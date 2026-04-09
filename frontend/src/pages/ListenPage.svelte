<script lang="ts">
    import { onDestroy, onMount, tick } from "svelte";
    import { fetchPublicMusic, fetchStems, type PublicMusic } from "../lib/api";
    import { MidiMixerPlayer, type MixerTrack } from "../lib/midi-player";
    import { StemMixerPlayer, type StemTrack } from "../lib/stem-mixer";
    import { ScoreViewer } from "../lib/score-viewer";
    import { formatTime } from "../lib/utils";
    import Mixer from "../components/Mixer.svelte";
    import ScoreIcon from "../components/ScoreIcon.svelte";

    const { accessKey }: { accessKey: string } = $props();

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

    let midiPlayer = $state<MidiMixerPlayer | null>(null);
    let stemPlayer = $state<StemMixerPlayer | null>(null);
    let stemPlaybackReady = $state(false);
    let mixerTracks = $state<(MixerTrack | StemTrack)[]>([]);
    let playerMode = $state<"stems" | "midi" | null>(null);
    let midiLoading = $state(false);
    let midiPlayerError = $state("");
    let playbackState = $state<"stopped" | "playing" | "paused">("stopped");
    let playbackPosition = $state(0);
    let playbackDuration = $state(0);
    let pct = $derived(
        playbackDuration > 0 ? (playbackPosition / playbackDuration) * 100 : 0,
    );
    let playbackFrame = $state<number | null>(null);
    let globalVolume = $state(1.0);
    let trackLevels = $state<Record<string, number>>({});
    let soloedTrackIds = $state<Set<string>>(new Set());

    onMount(() => {
        void loadPublicMusic(accessKey);
    });

    onDestroy(() => {
        stopPlaybackLoop();
        if (stemPlayer) {
            void stemPlayer.dispose();
            stemPlayer = null;
        }
        if (midiPlayer) {
            void midiPlayer.dispose();
            midiPlayer = null;
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
        playbackState = "stopped";
        playbackPosition = 0;
        playbackDuration = 0;
        globalVolume = 1.0;
        mixerTracks = [];
        playerMode = null;
        stemPlaybackReady = false;
        midiLoading = false;
        midiPlayerError = "";
        soloedTrackIds = new Set();

        if (stemPlayer) {
            await stemPlayer.dispose();
            stemPlayer = null;
        }
        if (midiPlayer) {
            await midiPlayer.dispose();
            midiPlayer = null;
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
            );
            stemPlayer.setLevelMultiplier(15);
            stemPlaybackReady = stemPlayer.isReadyToPlay();
            mixerTracks = loaded.tracks;
            playbackDuration = loaded.duration;
            playbackPosition = 0;
            playbackState = "stopped";
            playerMode = "stems";
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
        const player = stemPlayer ?? midiPlayer;
        if (!player || playbackDuration <= 0) {
            return;
        }

        if (playerMode === "stems" && !stemPlaybackReady) {
            midiPlayerError =
                "Stems are still buffering. Wait a moment before starting playback.";
            return;
        }

        try {
            if (playbackState === "playing") {
                player.pause();
                playbackState = "paused";
                playbackPosition = player.getCurrentTime();
                stopPlaybackLoop();
                return;
            }

            if (playbackPosition >= playbackDuration - 0.01) {
                player.seek(0);
                playbackPosition = 0;
            }

            await player.play();
            playbackState = "playing";
            startPlaybackLoop();
        } catch (error) {
            midiPlayerError =
                error instanceof Error
                    ? error.message
                    : "Unable to start playback";
        }
    }

    function stopPlayback() {
        const player = stemPlayer ?? midiPlayer;
        if (!player) {
            return;
        }

        player.stop();
        playbackState = "stopped";
        playbackPosition = 0;
        stopPlaybackLoop();
    }

    function handleSeek(event: Event) {
        const player = stemPlayer ?? midiPlayer;
        if (!player) {
            return;
        }

        const target = event.currentTarget as HTMLInputElement;
        const seconds = Number(target.value);
        void handleScoreSeek(seconds);
    }

    async function handleScoreSeek(seconds: number) {
        scoreViewer?.seek(seconds);
        playbackPosition = seconds;
        const player = stemPlayer ?? midiPlayer;
        if (!player) return;
        const wasPlaying = playbackState === "playing";
        if (wasPlaying) {
            player.pause();
            stopPlaybackLoop();
        }
        player.seek(seconds);
        if (wasPlaying) {
            await player.play();
            startPlaybackLoop();
        }
    }

    function updateTrackVolume(trackId: string, volume: number) {
        mixerTracks = mixerTracks.map((track) =>
            track.id === trackId ? { ...track, volume } : track,
        );
        if (stemPlayer && playerMode === "stems") {
            stemPlayer.setTrackVolume(trackId, volume);
        } else if (midiPlayer && playerMode === "midi") {
            midiPlayer.setTrackVolume(trackId, volume);
        }
    }

    function updateGlobalVolume(volume: number) {
        globalVolume = volume;
        mixerTracks = mixerTracks.map((track) => ({
            ...track,
            volume: globalVolume,
        }));
        if (stemPlayer && playerMode === "stems") {
            for (const track of mixerTracks) {
                stemPlayer.setTrackVolume(track.id, globalVolume);
            }
        } else if (midiPlayer && playerMode === "midi") {
            for (const track of mixerTracks) {
                midiPlayer.setTrackVolume(track.id, globalVolume);
            }
        }
    }

    function toggleTrackMute(trackId: string) {
        mixerTracks = mixerTracks.map((track) => {
            if (track.id !== trackId) {
                return track;
            }

            const muted = !track.muted;
            const anySoloed = soloedTrackIds.size > 0;
            const effectiveMuted = muted || (anySoloed && !soloedTrackIds.has(trackId));
            if (stemPlayer && playerMode === "stems") {
                stemPlayer.setTrackMuted(trackId, effectiveMuted);
            } else if (midiPlayer && playerMode === "midi") {
                midiPlayer.setTrackMuted(trackId, effectiveMuted);
            }
            return { ...track, muted };
        });
    }

    function toggleTrackSolo(trackId: string) {
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
            if (stemPlayer && playerMode === "stems") {
                stemPlayer.setTrackMuted(track.id, effectiveMuted);
            } else if (midiPlayer && playerMode === "midi") {
                midiPlayer.setTrackMuted(track.id, effectiveMuted);
            }
        }
    }

    function startPlaybackLoop() {
        stopPlaybackLoop();
        const tickPlayback = () => {
            const player = stemPlayer ?? midiPlayer;
            if (!player) return;
            playbackPosition = player.getCurrentTime();
            scoreViewer?.seek(playbackPosition);
            if (stemPlayer && playerMode === "stems") {
                stemPlayer.synchronizePlayback();
                const levels: Record<string, number> = {};
                for (const track of mixerTracks)
                    levels[track.id] = stemPlayer.getLevel(track.id);
                trackLevels = levels;
            }
            if (playbackState === "playing") {
                if (
                    playbackDuration > 0 &&
                    playbackPosition >= playbackDuration - 0.03
                ) {
                    player.pause();
                    player.seek(playbackDuration);
                    playbackState = "paused";
                    playbackPosition = playbackDuration;
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
</script>

<main class="page public-shell">
    <section class="content-panel public-content-panel">
        {#if publicError}
            <p class="status error">{publicError}</p>
        {:else}
            <div class="public-card public-workspace">
                <div class="public-score-pane">
                    <div class="score-title-row score-title-bar">
                        <a class="listen-brand" href="/">Fumen</a>
                        <span class="listen-title-separator" aria-hidden="true"
                            >-</span
                        >
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
                                <svg
                                    width="15"
                                    height="15"
                                    viewBox="0 0 24 24"
                                    fill="none"
                                    stroke="currentColor"
                                    stroke-width="2.2"
                                    stroke-linecap="round"
                                    stroke-linejoin="round"
                                    ><path d="M12 3v12M7 11l5 5 5-5" /><path
                                        d="M4 20h16"
                                    /></svg
                                >
                                Download
                                <svg
                                    class="chevron"
                                    width="12"
                                    height="12"
                                    viewBox="0 0 24 24"
                                    fill="none"
                                    stroke="currentColor"
                                    stroke-width="2.5"
                                    ><polyline
                                        points="6 9 12 15 18 9"
                                    /></svg
                                    >
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
                        class:is-playing={playbackState === "playing"}
                    >
                        <button
                            class="playbar-btn playbar-play"
                            onclick={() => void togglePlayback()}
                            disabled={mixerTracks.length === 0 ||
                                midiLoading ||
                                (playerMode === "stems" && !stemPlaybackReady)}
                            aria-label={playbackState === "playing"
                                ? "Pause"
                                : "Play"}
                        >
                            {#if playbackState === "playing"}
                                <svg
                                    width="18"
                                    height="18"
                                    viewBox="0 0 24 24"
                                    fill="currentColor"
                                    ><rect
                                        x="5"
                                        y="4"
                                        width="4"
                                        height="16"
                                        rx="1.5"
                                    /><rect
                                        x="15"
                                        y="4"
                                        width="4"
                                        height="16"
                                        rx="1.5"
                                    /></svg
                                >
                            {:else}
                                <svg
                                    width="18"
                                    height="18"
                                    viewBox="0 0 24 24"
                                    fill="currentColor"
                                    ><path d="M7 4.5 L7 19.5 L20 12 Z" /></svg
                                >
                            {/if}
                        </button>
                        <button
                            class="playbar-btn playbar-stop"
                            onclick={stopPlayback}
                            disabled={mixerTracks.length === 0 ||
                                midiLoading ||
                                (playerMode === "stems" && !stemPlaybackReady)}
                            aria-label="Stop"
                            ><svg
                                width="14"
                                height="14"
                                viewBox="0 0 24 24"
                                fill="currentColor"
                                ><rect
                                    x="4"
                                    y="4"
                                    width="16"
                                    height="16"
                                    rx="2"
                                /></svg
                            ></button
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
                                    (playerMode === "stems" &&
                                        !stemPlaybackReady)}
                                style="--pct: {pct}%"
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
