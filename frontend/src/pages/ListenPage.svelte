<script lang="ts">
    import { onDestroy, onMount, tick } from "svelte";
    import {
        ApiError,
        fetchPublicMusic,
        fetchStems,
        hasAuth,
        reportPublicMusicPlaytime,
        type PublicMusic,
    } from "../lib/api";
    import { StemMixerPlayer, type StemTrack } from "../lib/stem-mixer";
    import { ScoreViewer } from "../lib/score-viewer";
    import { formatTime } from "../lib/utils";
    import Mixer from "../components/Mixer.svelte";
    import ScoreIcon from "../components/ScoreIcon.svelte";
    import { Download, ChevronDown, Pause, Play, Square } from '@lucide/svelte';

    let {
        accessKey,
        enableCountIn = false,
        countInMeasures = 1,
    }: {
        accessKey: string;
        enableCountIn?: boolean;
        countInMeasures?: number;
    } = $props();

    let publicMusic = $state<PublicMusic | null>(null);
    let publicLoading = $state(false);
    let publicError = $state("");
    let downloadMenuOpen = $state(false);
    let mixerRequested = $state(false);

    let scoreViewer = $state<ScoreViewer | null>(null);
    let scoreContainer = $state<HTMLElement | null>(null);
    let scoreLoading = $state(false);
    let scoreLoaded = $state(false);
    let scoreError = $state("");

    let stemPlayer = $state<StemMixerPlayer | null>(null);
    let stemPlaybackReady = $state(false);
    let mixerTracks = $state<StemTrack[]>([]);
    let midiLoading = $state(false);
    let midiPlayerError = $state("");
    let stemLoadProgress = $state(0);
    let playbackState = $state<"stopped" | "playing" | "paused" | "counting-in">("stopped");
    let playbackPosition = $state(0);
    let playbackDuration = $state(0);
    let pct = $derived(
        playbackDuration > 0 ? (playbackPosition / playbackDuration) * 100 : 0,
    );
    let loadPct = $derived(stemLoadProgress * 100);
    let playbackFrame = $state<number | null>(null);
    let globalVolume = $state(1.0);
    let trackLevels = $state<Record<string, number>>({});
    let soloedTrackIds = $state<Set<string>>(new Set());
    const PLAYTIME_ACCOUNTING_INTERVAL_MS = 1000;
    const PLAYTIME_FLUSH_INTERVAL_MS = 5000;
    let playtimePending: Record<string, number> = {};
    let playtimeLastMeasuredAt = 0;
    let playtimeLastFlushAt = 0;
    let playtimeInterval: number | null = null;
    let playtimeReportingDisabled = false;

    onMount(() => {
        const handlePageHide = () => {
            capturePlaytimeSlice();
            void flushPlaytime(true);
        };
        window.addEventListener("pagehide", handlePageHide);
        void loadPublicMusic(accessKey);

        return () => {
            window.removeEventListener("pagehide", handlePageHide);
        };
    });

    onDestroy(() => {
        stopPlaybackLoop();
        capturePlaytimeSlice();
        void flushPlaytime(true);
        void stopPlaytimeTracking(false);
        if (stemPlayer) {
            void stemPlayer.dispose();
            stemPlayer = null;
        }
        if (scoreViewer) {
            scoreViewer.dispose();
            scoreViewer = null;
        }
    });

    async function loadPublicMusic(key: string) {
        publicLoading = true;
        publicError = "";
        downloadMenuOpen = false;
        playtimePending = {};
        playtimeReportingDisabled = false;

        try {
            const music = await fetchPublicMusic(key);
            publicMusic = music;
            publicLoading = false;
            await tick();
            await resetMixers();
            mixerRequested = false;

            let scoreTask: Promise<void> = Promise.resolve();
            if (music.musicxml_url && scoreContainer) {
                scoreLoading = true;
                const sv = new ScoreViewer(scoreContainer);
                sv.onClickSeek = (seconds: number) => handleScoreSeek(seconds);
                scoreViewer = sv;
                scoreTask = sv
                    .load(music.musicxml_url)
                    .then(() => {
                        scoreLoaded = true;
                    })
                    .catch((err: unknown) => {
                        console.error("[ScoreViewer] load failed:", err);
                        scoreError =
                            err instanceof Error
                                ? `${err.message}\n${err.stack ?? ""}`
                                : String(err);
                    })
                    .finally(() => {
                        scoreLoading = false;
                    });
            }

            await scoreTask;

            mixerRequested = true;
            if (music.stems_status === "ready") {
                await loadStemMixer(key);
            }
        } catch (error) {
            publicError =
                error instanceof Error
                    ? error.message
                    : "Unable to load this score";
        } finally {
            publicLoading = false;
        }
    }

    async function resetMixers() {
        stopPlaybackLoop();
        void stopPlaytimeTracking(false);
        playbackState = "stopped";
        playbackPosition = 0;
        playbackDuration = 0;
        globalVolume = 1.0;
        mixerTracks = [];
        stemPlaybackReady = false;
        midiLoading = false;
        midiPlayerError = "";
        soloedTrackIds = new Set();

        if (stemPlayer) {
            await stemPlayer.dispose();
            stemPlayer = null;
        }
        if (scoreViewer) {
            scoreViewer.dispose();
            scoreViewer = null;
        }
        scoreLoading = false;
        scoreLoaded = false;
        scoreError = "";
        mixerRequested = false;
    }

    async function loadStemMixer(key: string) {
        midiLoading = true;
        midiPlayerError = "";
        stemPlaybackReady = false;
        stemLoadProgress = 0;

        try {
            const stems = await fetchStems(key);
            if (stems.length === 0) {
                midiPlayerError = "No stems available for this score";
                return;
            }

            stemPlayer = new StemMixerPlayer();
            const loaded = await stemPlayer.loadStems(
                stems.map((stem) => ({
                    id: String(stem.track_index),
                    name: stem.track_name,
                    instrumentName: stem.instrument_name,
                    fullStemUrl: stem.full_stem_url,
                    durationSeconds: stem.duration_seconds,
                })),
                (progress) => { stemLoadProgress = progress; },
            );
            stemPlayer.setLevelMultiplier(15);
            stemPlaybackReady = stemPlayer.isReadyToPlay();
            mixerTracks = loaded.tracks;
            playbackDuration = loaded.duration;
            playbackPosition = 0;
            playbackState = "stopped";
        } catch (error) {
            midiPlayerError =
                error instanceof Error
                    ? error.message
                    : "Unable to prepare stem playback";
        } finally {
            midiLoading = false;
        }
    }

    async function togglePlayback() {
        const player = stemPlayer;
        if (!player || playbackDuration <= 0) {
            return;
        }

        if (!stemPlaybackReady) {
            midiPlayerError =
                "Stems are still buffering. Wait a moment before starting playback.";
            return;
        }

        try {
            if (playbackState === "playing" || playbackState === "counting-in") {
                capturePlaytimeSlice();
                player.pause();
                playbackState = "paused";
                playbackPosition = player.getCurrentTime();
                await stopPlaytimeTracking();
                stopPlaybackLoop();
                return;
            }

            if (playbackPosition >= playbackDuration - 0.01) {
                player.seek(0);
                playbackPosition = 0;
            }

            const shouldCountIn =
                enableCountIn &&
                playbackPosition <= 0.01;
            if (shouldCountIn) {
                const countInInfo = scoreViewer?.getCountInInfo() ?? {
                    bpm: 120,
                    beatsPerBar: 4,
                };
                const countInBars = Math.max(1, Math.floor(countInMeasures || 1));
                const beatSeconds =
                    countInInfo.bpm > 0 ? 60 / countInInfo.bpm : 0.5;
                await player.play({
                    startDelaySeconds:
                        beatSeconds * countInInfo.beatsPerBar * countInBars,
                    countInBeats: countInInfo.beatsPerBar * countInBars,
                    beatSeconds,
                });
                playbackState = "counting-in";
            } else {
                await player.play();
                playbackState = "playing";
                startPlaytimeTracking();
            }
            startPlaybackLoop();
        } catch (error) {
            midiPlayerError =
                error instanceof Error
                    ? error.message
                    : "Unable to start playback";
        }
    }

    function stopPlayback() {
        const player = stemPlayer;
        if (!player) {
            return;
        }

        capturePlaytimeSlice();
        player.stop();
        playbackState = "stopped";
        playbackPosition = 0;
        void stopPlaytimeTracking();
        stopPlaybackLoop();
    }

    function handleSeek(event: Event) {
        const player = stemPlayer;
        if (!player) {
            return;
        }

        const target = event.currentTarget as HTMLInputElement;
        const seconds = Number(target.value);
        void handleScoreSeek(seconds);
    }

    async function handleScoreSeek(seconds: number) {
        capturePlaytimeSlice();
        scoreViewer?.seek(seconds);
        playbackPosition = seconds;
        const player = stemPlayer;
        if (!player) return;
        const wasPlaying = playbackState === "playing";
        const wasCountingIn = playbackState === "counting-in";
        if (wasPlaying || wasCountingIn) {
            player.pause();
            stopPlaybackLoop();
        }
        player.seek(seconds);
        playtimeLastMeasuredAt = performance.now();
        if (wasPlaying) {
            await player.play();
            playbackState = "playing";
            startPlaytimeTracking();
            startPlaybackLoop();
        } else if (wasCountingIn) {
            playbackState = "paused";
        }
    }

    function updateTrackVolume(trackId: string, volume: number) {
        capturePlaytimeSlice();
        mixerTracks = mixerTracks.map((track) =>
            track.id === trackId ? { ...track, volume } : track,
        );
        if (stemPlayer) {
            stemPlayer.setTrackVolume(trackId, volume);
        }
    }

    function updateGlobalVolume(volume: number) {
        capturePlaytimeSlice();
        globalVolume = volume;
        mixerTracks = mixerTracks.map((track) => ({
            ...track,
            volume: globalVolume,
        }));
        if (stemPlayer) {
            for (const track of mixerTracks) {
                stemPlayer.setTrackVolume(track.id, globalVolume);
            }
        }
    }

    function toggleTrackMute(trackId: string) {
        capturePlaytimeSlice();
        mixerTracks = mixerTracks.map((track) => {
            if (track.id !== trackId) {
                return track;
            }

            const muted = !track.muted;
            const anySoloed = soloedTrackIds.size > 0;
            const effectiveMuted = muted || (anySoloed && !soloedTrackIds.has(trackId));
            if (stemPlayer) {
                stemPlayer.setTrackMuted(trackId, effectiveMuted);
            }
            return { ...track, muted };
        });
    }

    function toggleTrackSolo(trackId: string) {
        capturePlaytimeSlice();
        const newSoloedIds = new Set(soloedTrackIds);
        if (newSoloedIds.has(trackId)) {
            newSoloedIds.delete(trackId);
        } else {
            newSoloedIds.add(trackId);
        }
        soloedTrackIds = newSoloedIds;

        const anySoloed = newSoloedIds.size > 0;
        for (const track of mixerTracks) {
            const effectiveMuted = track.muted || (anySoloed && !newSoloedIds.has(track.id));
            if (stemPlayer) {
                stemPlayer.setTrackMuted(track.id, effectiveMuted);
            }
        }
    }

    function startPlaybackLoop() {
        stopPlaybackLoop();
        const tickPlayback = () => {
            const player = stemPlayer;
            if (!player) return;
            playbackPosition = player.getCurrentTime();
            scoreViewer?.seek(playbackPosition);
            if (stemPlayer) {
                stemPlayer.synchronizePlayback();
                const levels: Record<string, number> = {};
                for (const track of mixerTracks)
                    levels[track.id] = stemPlayer.getLevel(track.id);
                trackLevels = levels;
            }
            if (playbackState === "counting-in" && playbackPosition > 0.01) {
                playbackState = "playing";
                startPlaytimeTracking();
            }
            if (playbackState === "playing" || playbackState === "counting-in") {
                if (
                    playbackState === "playing" &&
                    playbackDuration > 0 &&
                    playbackPosition >= playbackDuration - 0.03
                ) {
                    capturePlaytimeSlice();
                    player.pause();
                    player.seek(playbackDuration);
                    playbackState = "paused";
                    playbackPosition = playbackDuration;
                    void stopPlaytimeTracking();
                    stopPlaybackLoop();
                    return;
                }
                playbackFrame = requestAnimationFrame(tickPlayback);
            }
        };
        playbackFrame = requestAnimationFrame(tickPlayback);
    }

    function stopPlaybackLoop() {
        if (playbackFrame !== null) {
            cancelAnimationFrame(playbackFrame);
            playbackFrame = null;
        }
        trackLevels = {};
    }

    function startPlaytimeTracking() {
        if (!hasAuth() || playtimeReportingDisabled) {
            return;
        }

        void stopPlaytimeTracking(false);
        playtimeLastMeasuredAt = performance.now();
        playtimeLastFlushAt = playtimeLastMeasuredAt;
        playtimeInterval = window.setInterval(() => {
            capturePlaytimeSlice();
            const now = performance.now();
            if (now - playtimeLastFlushAt >= PLAYTIME_FLUSH_INTERVAL_MS) {
                void flushPlaytime();
            }
        }, PLAYTIME_ACCOUNTING_INTERVAL_MS);
    }

    async function stopPlaytimeTracking(flush = true) {
        if (playtimeInterval !== null) {
            clearInterval(playtimeInterval);
            playtimeInterval = null;
        }
        playtimeLastMeasuredAt = 0;
        if (flush) {
            await flushPlaytime();
        }
    }

    function capturePlaytimeSlice() {
        if (
            playbackState !== "playing" ||
            !publicMusic ||
            !hasAuth() ||
            playtimeReportingDisabled
        ) {
            playtimeLastMeasuredAt = performance.now();
            return;
        }

        const now = performance.now();
        if (!playtimeLastMeasuredAt) {
            playtimeLastMeasuredAt = now;
            return;
        }

        const deltaSeconds = (now - playtimeLastMeasuredAt) / 1000;
        playtimeLastMeasuredAt = now;
        if (deltaSeconds <= 0) {
            return;
        }

        const audibleTrackIndices = getAudibleTrackIndices();
        for (const trackIndex of audibleTrackIndices) {
            playtimePending[trackIndex] =
                (playtimePending[trackIndex] ?? 0) + deltaSeconds;
        }
    }

    function getAudibleTrackIndices(): number[] {
        if (globalVolume <= 0) {
            return [];
        }

        const anySoloed = soloedTrackIds.size > 0;
        return mixerTracks
            .filter((track) => {
                if (track.muted || track.volume <= 0) {
                    return false;
                }
                return !anySoloed || soloedTrackIds.has(track.id);
            })
            .map((track) => Number(track.id))
            .filter((trackIndex) => Number.isFinite(trackIndex));
    }

    async function flushPlaytime(keepalive = false) {
        if (!publicMusic || !hasAuth() || playtimeReportingDisabled) {
            playtimePending = {};
            return;
        }

        const tracks = Object.entries(playtimePending)
            .map(([trackIndex, seconds]) => ({
                track_index: Number(trackIndex),
                seconds: Number(seconds.toFixed(3)),
            }))
            .filter(
                (track) =>
                    Number.isFinite(track.track_index) &&
                    Number.isFinite(track.seconds) &&
                    track.seconds > 0,
            );
        if (tracks.length === 0) {
            return;
        }

        playtimePending = {};

        try {
            await reportPublicMusicPlaytime(
                accessKey,
                { tracks },
                { keepalive },
            );
            playtimeLastFlushAt = performance.now();
        } catch (error) {
            if (error instanceof ApiError && error.status < 500) {
                playtimeReportingDisabled = true;
                return;
            }

            for (const track of tracks) {
                playtimePending[track.track_index] =
                    (playtimePending[track.track_index] ?? 0) + track.seconds;
            }
        }
    }
</script>

<main class="page public-shell public-listen-shell">
    <section class="content-panel public-content-panel">
        {#if publicError}
            <p class="status error">{publicError}</p>
        {:else}
            <div class="public-card public-workspace">
                <div class="public-score-pane">
                    <div class="score-title-row score-title-bar listen-score-title-bar">
                        <a class="listen-home-link" href="/" aria-label="Fumen home">
                            <span class="listen-home-mark" aria-hidden="true"></span>
                        </a>
                        <h2>
                            <ScoreIcon
                                variant="listen"
                                icon={publicMusic?.icon ?? null}
                                imageUrl={publicMusic?.icon_image_url ?? null}
                            />
                            <span>{publicMusic?.title ?? "Loading score"}</span>
                        </h2>
                        <div class="download-menu" class:open={downloadMenuOpen}>
                            <button
                                class="download-menu-btn"
                                onclick={() =>
                                    publicMusic &&
                                    (downloadMenuOpen = !downloadMenuOpen)}
                                aria-haspopup="true"
                                aria-expanded={downloadMenuOpen}
                                disabled={!publicMusic}
                            >
                                <Download size={15} strokeWidth={2.2} />
                                <span class="download-menu-label">Download</span>
                                <ChevronDown class="chevron" size={12} strokeWidth={2.5} />
                                </button>
                            {#if downloadMenuOpen && publicMusic}
                                <div class="download-dropdown">
                                    {#if publicMusic.audio_stream_url}
                                        <a
                                            class="download-item"
                                            href={publicMusic.audio_stream_url}
                                            download
                                            onclick={() =>
                                                (downloadMenuOpen = false)}
                                            >Download Audio</a
                                        >
                                    {/if}
                                    {#if publicMusic.midi_download_url}
                                        <a
                                            class="download-item"
                                            href={publicMusic.midi_download_url}
                                            download
                                            onclick={() =>
                                                (downloadMenuOpen = false)}
                                            >Download MIDI</a
                                        >
                                    {/if}
                                    <a
                                        class="download-item"
                                        href={publicMusic.download_url}
                                        download
                                        onclick={() =>
                                            (downloadMenuOpen = false)}
                                        >Download MuseScore</a
                                    >
                                </div>
                            {/if}
                        </div>
                    </div>
                    <div
                        class="score-scroll-area"
                        class:score-scroll-area-loading={scoreLoading &&
                            !scoreError}
                    >
                        <div class="score-scroll-inner">
                            <div
                                class="score-container"
                                class:loaded={scoreLoaded}
                                bind:this={scoreContainer}
                            ></div>
                            {#if scoreLoading}
                                <div class="score-loading-state" aria-label="Loading score">
                                    <div class="loading-eq" aria-hidden="true">
                                        <span></span>
                                        <span></span>
                                        <span></span>
                                        <span></span>
                                        <span></span>
                                    </div>
                                    <p class="loading-eq-label">Fumen</p>
                                    <p class="score-loading-copy">Loading score</p>
                                </div>
                            {:else if scoreError}
                                <p class="status error">Score: {scoreError}</p>
                            {/if}
                        </div>
                    </div>
                    <div
                        class="playbar"
                        class:is-playing={playbackState === "playing" ||
                            playbackState === "counting-in"}
                    >
                        <button
                            class="playbar-btn playbar-play"
                            onclick={() => void togglePlayback()}
                            disabled={mixerTracks.length === 0 ||
                                midiLoading ||
                                !stemPlaybackReady}
                            aria-label={playbackState === "playing" ||
                                playbackState === "counting-in"
                                ? "Pause"
                                : "Play"}
                        >
                            {#if playbackState === "playing" ||
                                playbackState === "counting-in"}
                                <Pause size={18} fill="currentColor" strokeWidth={0} />
                            {:else}
                                <Play size={18} fill="currentColor" strokeWidth={0} />
                            {/if}
                        </button>
                        <button
                            class="playbar-btn playbar-stop"
                            onclick={stopPlayback}
                            disabled={mixerTracks.length === 0 ||
                                midiLoading ||
                                !stemPlaybackReady}
                            aria-label="Stop"
                            ><Square size={14} fill="currentColor" strokeWidth={0} /></button
                        >
                        <div class="playbar-progress">
                            <input
                                class="playbar-track"
                                type="range"
                                min="0"
                                max={playbackDuration || 0}
                                step="0.01"
                                value={playbackPosition}
                                oninput={handleSeek}
                                disabled={mixerTracks.length === 0 ||
                                    midiLoading ||
                                    !stemPlaybackReady}
                                style="--pct: {pct}%; --load-pct: {loadPct}%"
                                aria-label="Playback position"
                            />
                        </div>
                        <span class="playbar-time"
                            >{formatTime(playbackPosition)}<span
                                class="playbar-sep"
                            >
                                /
                            </span>{formatTime(playbackDuration)}</span
                        >
                    </div>
                </div>
                <div class="public-mixer-pane">
                    <Mixer
                        {midiLoading}
                        {mixerTracks}
                        {mixerRequested}
                        {globalVolume}
                        {trackLevels}
                        {midiPlayerError}
                        {soloedTrackIds}
                        stemsError={publicMusic?.stems_error ?? null}
                        onGlobalVolumeChange={updateGlobalVolume}
                        onTrackVolumeChange={updateTrackVolume}
                        onTrackMuteToggle={toggleTrackMute}
                        onTrackSoloToggle={toggleTrackSolo}
                    />
                </div>
            </div>
        {/if}
    </section>
</main>
