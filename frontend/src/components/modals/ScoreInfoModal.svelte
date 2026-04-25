<script lang="ts">
    import { onMount } from "svelte";
    import type {
        AdminMusicPlaytimeResponse as AdminMusicPlaytime,
        AdminMusicProcessingLogResponse,
        AdminMusicResponse as AdminMusic,
        UserResponse as AppUser,
    } from "$backend/models";
    import { canEditOwnedScore } from "$lib/admin-permissions";
    import {
        formatBytes,
        formatPlaytimeDuration,
        prettyDate,
        qualityProfileLabel,
    } from "$lib/utils";
    import LogBlock from "$components/log-block/LogBlock.svelte";
    import BaseModal from "./BaseModal.svelte";

    let {
        music,
        currentUser,
        loadPlaytime,
        loadProcessingLog,
        reloadMusic,
        canViewProcessingLog = false,
        onRetryRender,
        modalId,
    }: {
        music: AdminMusic;
        currentUser: AppUser;
        loadPlaytime: (musicId: string) => Promise<AdminMusicPlaytime>;
        loadProcessingLog: (
            musicId: string,
        ) => Promise<AdminMusicProcessingLogResponse>;
        reloadMusic: (musicId: string) => Promise<AdminMusic>;
        canViewProcessingLog?: boolean;
        onRetryRender: (musicId: string) => Promise<AdminMusic>;
        modalId?: string;
    } = $props();

    let currentMusic = $state(music);
    let playtime = $state<AdminMusicPlaytime | null>(null);
    let playtimeLoading = $state(true);
    let playtimeError = $state("");
    let processingLog = $state("");
    let processingLogLoading = $state(false);
    let processingLogError = $state("");
    let retrying = $state(false);
    let retryError = $state("");

    type ProcessingStepStatus = "done" | "active" | "pending" | "failed";
    type ProcessingStep = {
        key: string;
        label: string;
        detail: string;
        status: ProcessingStepStatus;
    };

    function processingStatuses(item: AdminMusic) {
        return [
            item.audio_status,
            item.midi_status,
            item.musicxml_status,
            item.stems_status,
        ];
    }

    function isReadyMusic(item: AdminMusic) {
        return processingStatuses(item).every((status) => status === "ready");
    }

    function shouldAutoRefreshProcessing(item: AdminMusic) {
        return !isReadyMusic(item);
    }

    function hasProcessingFailure(item: AdminMusic) {
        return processingStatuses(item).includes("failed");
    }

    function deriveProcessingSteps(item: AdminMusic, logText: string): ProcessingStep[] {
        const lower = logText.toLowerCase();
        const storageLabel = lower.includes(" s3") ? "S3" : "Storage";
        const failed =
            hasProcessingFailure(item) ||
            lower.includes("processing failed") ||
            lower.includes(" failed.");

        const hasInputActivity =
            lower.includes("processing restart requested") ||
            lower.includes("writing temporary input file") ||
            lower.includes("temporary input file written") ||
            lower.includes("score upload");
        const hasMusicxmlActivity =
            lower.includes("musicxml") ||
            lower.includes("application/xml") ||
            lower.includes("score.musicxml") ||
            lower.includes("audio, midi, and musicxml conversion finished");
        const hasStorageActivity =
            lower.includes("uploading ") ||
            lower.includes(" uploaded ") ||
            lower.includes("upload to s3") ||
            lower.includes("upload to storage") ||
            lower.includes("database state updated");
        const isReady = isReadyMusic(item);
        const isComplete =
            lower.includes("processing completed. database state updated.") ||
            (isReady &&
                !hasProcessingFailure(item) &&
                processingStatuses(item).every((status) => status === "ready"));

        const inputDone =
            hasInputActivity ||
            logText.trim().length > 0 ||
            processingStatuses(item).some((status) => status !== "processing");
        const musicxmlDone =
            item.musicxml_status !== "processing" || hasMusicxmlActivity;
        const stemsDone =
            item.stems_status !== "processing" ||
            lower.includes("stem generation finished") ||
            lower.includes("stems: upload") ||
            lower.includes("stems status:");
        const storageStarted = hasStorageActivity || (stemsDone && !isComplete);
        const storageDone = isComplete;

        let failedIndex: number | null = null;
        if (failed) {
            if (item.musicxml_status === "failed") {
                failedIndex = 1;
            } else if (item.stems_status === "failed") {
                failedIndex = 2;
            } else if (item.audio_status === "failed" || item.midi_status === "failed") {
                failedIndex = stemsDone ? 3 : 1;
            } else if (!musicxmlDone) {
                failedIndex = 1;
            } else if (!stemsDone) {
                failedIndex = 2;
            } else {
                failedIndex = 3;
            }
        }

        const steps = [
            {
                key: "input",
                label: "Input",
                detail: "staged",
            },
            {
                key: "musicxml",
                label: "MusicXML",
                detail: "notation export",
            },
            {
                key: "stems",
                label: "Stems",
                detail: "track render",
            },
            {
                key: "storage",
                label: storageLabel,
                detail: "asset upload",
            },
            {
                key: "ready",
                label: "Ready",
                detail: "published",
            },
        ];

        const statuses: ProcessingStepStatus[] = [
            inputDone ? "done" : "active",
            musicxmlDone ? "done" : inputDone ? "active" : "pending",
            stemsDone ? "done" : musicxmlDone ? "active" : "pending",
            storageDone ? "done" : storageStarted ? "active" : "pending",
            isComplete ? "done" : "pending",
        ];

        if (failedIndex !== null) {
            for (let index = 0; index < statuses.length; index++) {
                if (index < failedIndex) {
                    statuses[index] = "done";
                } else if (index === failedIndex) {
                    statuses[index] = "failed";
                } else {
                    statuses[index] = "pending";
                }
            }
        }

        return steps.map((step, index) => ({
            ...step,
            status: statuses[index],
        }));
    }

    const processingSteps = $derived(deriveProcessingSteps(currentMusic, processingLog));

    async function refreshCurrentMusic() {
        currentMusic = await reloadMusic(currentMusic.id);
    }

    async function refreshProcessingLog(showLoading = true) {
        if (!canViewProcessingLog) return;

        if (showLoading) {
            processingLogLoading = true;
        }
        processingLogError = "";
        try {
            const response = await loadProcessingLog(currentMusic.id);
            processingLog = response.content.trim();
        } catch (error) {
            processingLogError =
                error instanceof Error
                    ? error.message
                    : "Unable to load processing log";
        } finally {
            if (showLoading) {
                processingLogLoading = false;
            }
        }
    }

    onMount(() => {
        let cancelled = false;
        let intervalId: number | undefined;

        const run = async () => {
            playtimeLoading = true;
            playtimeError = "";
            try {
                const response = await loadPlaytime(currentMusic.id);
                if (!cancelled) {
                    playtime = response;
                }
            } catch (error) {
                if (!cancelled) {
                    playtimeError =
                        error instanceof Error
                            ? error.message
                            : "Unable to load playtime";
                }
            } finally {
                if (!cancelled) {
                    playtimeLoading = false;
                }
            }

            if (!cancelled && canViewProcessingLog) {
                await refreshProcessingLog();
            }
        };

        void run();

        intervalId = window.setInterval(() => {
            if (!shouldAutoRefreshProcessing(currentMusic)) {
                return;
            }

            void (async () => {
                try {
                    await refreshCurrentMusic();
                    if (canViewProcessingLog) {
                        await refreshProcessingLog(false);
                    }
                } catch {
                    // Ignore polling errors and keep the modal interactive.
                }
            })();
        }, 5000);

        return () => {
            cancelled = true;
            if (intervalId) {
                window.clearInterval(intervalId);
            }
        };
    });

    async function handleRetryRender() {
        retrying = true;
        retryError = "";
        try {
            currentMusic = await onRetryRender(currentMusic.id);
            await refreshProcessingLog();
        } catch (error) {
            retryError =
                error instanceof Error
                    ? error.message
                    : "Restart failed.";
        } finally {
            retrying = false;
        }
    }

    function resolveProcessingLogFileName(): string {
        const fallback = (currentMusic.title || "score")
            .trim()
            .toLowerCase()
            .replace(/[^a-z0-9]+/g, "-")
            .replace(/^-+|-+$/g, "");

        return `${fallback || "score"}-processing.log`;
    }

    function downloadProcessingLog() {
        if (!processingLog) {
            return;
        }

        const blob = new Blob([processingLog], {
            type: "text/plain;charset=utf-8",
        });
        const objectUrl = URL.createObjectURL(blob);
        const anchor = document.createElement("a");

        anchor.href = objectUrl;
        anchor.download = resolveProcessingLogFileName();
        anchor.style.display = "none";
        document.body.append(anchor);
        anchor.click();
        anchor.remove();

        setTimeout(() => URL.revokeObjectURL(objectUrl), 0);
    }
</script>

<BaseModal
    size="large"
    cardClass="admin-score-modal"
    title="Score info"
    subtitle={currentMusic.title}
    {modalId}
>
    <div class="upload-grid admin-score-modal-grid">
        <label class="field admin-score-modal-full">
            <span>Subtitle</span>
            <input value={currentMusic.subtitle ?? ""} readonly />
        </label>
        <label class="field">
            <span>MSCZ filename</span>
            <input value={currentMusic.filename} readonly />
        </label>
        <label class="field">
            <span>Stem quality</span>
            <input
                value={qualityProfileLabel(currentMusic.quality_profile)}
                readonly
            />
        </label>
        <label class="field admin-score-modal-full">
            <span>Ensembles</span>
            <input
                value={currentMusic.ensemble_names.join(", ") || "No ensemble"}
                readonly
            />
        </label>
        <label class="field">
            <span>Stem files size</span>
            <input
                value={`${formatBytes(currentMusic.stems_total_bytes)} total`}
                readonly
            />
        </label>
        <label class="field">
            <span>Uploaded</span>
            <input value={prettyDate(currentMusic.created_at)} readonly />
        </label>
        <label class="field">
            <span>Stems status</span>
            <input value={currentMusic.stems_status} readonly />
        </label>
        <label class="field">
            <span>Audio status</span>
            <input value={currentMusic.audio_status} readonly />
        </label>
        <label class="field">
            <span>MIDI status</span>
            <input value={currentMusic.midi_status} readonly />
        </label>
        <label class="field">
            <span>MusicXML status</span>
            <input value={currentMusic.musicxml_status} readonly />
        </label>
    </div>
    {#if currentMusic.audio_error}
        <p class="hint">{currentMusic.audio_error}</p>
    {/if}
    {#if currentMusic.stems_error}
        <p class="hint">{currentMusic.stems_error}</p>
    {/if}
    {#if currentMusic.midi_error}
        <p class="hint">{currentMusic.midi_error}</p>
    {/if}
    {#if canViewProcessingLog}
        <section class="admin-playtime-section">
            <div class="admin-playtime-header">
                <div>
                    <p class="meta-label">Processing</p>
                    <h3>Backend log</h3>
                </div>
                <div class="actions admin-playtime-actions">
                    <button
                        class="button secondary"
                        type="button"
                        disabled={processingLogLoading}
                        onclick={() => void refreshProcessingLog()}
                    >
                        {processingLogLoading ? "Refreshing..." : "Refresh log"}
                    </button>
                    <button
                        class="button secondary"
                        type="button"
                        disabled={!processingLog}
                        onclick={downloadProcessingLog}
                    >
                        Download log
                    </button>
                    {#if currentMusic.stems_status !== "ready" && canEditOwnedScore(currentMusic, currentUser)}
                        <button
                            class="button ghost"
                            type="button"
                            disabled={retrying}
                            onclick={() => void handleRetryRender()}
                        >
                            {retrying ? "Restarting processing..." : "Restart processing"}
                        </button>
                    {/if}
                </div>
            </div>

            {#if processingLogError}
                <p class="status error">{processingLogError}</p>
            {:else if retryError}
                <p class="status error">{retryError}</p>
            {:else if processingLogLoading}
                <p class="hint">Loading processing log...</p>
            {:else if processingLog}
                <div class="processing-journey" aria-label="Processing progress">
                    {#each processingSteps as step, index (step.key)}
                        <article class={`processing-stop is-${step.status}`}>
                            <div class="processing-stop-rail" aria-hidden="true">
                                <span class="processing-stop-node"></span>
                                {#if index < processingSteps.length - 1}
                                    <span class="processing-stop-connector"></span>
                                {/if}
                            </div>
                            <div class="processing-stop-copy">
                                <strong>{step.label}</strong>
                                <span>{step.detail}</span>
                            </div>
                        </article>
                    {/each}
                </div>
                <LogBlock
                    logs={processingLog}
                    title=""
                    emptyMessage="No processing log has been recorded for this score yet."
                    showLevelFilter={false}
                    showDownloadButton={false}
                />
            {:else}
                <p class="hint">No processing log has been recorded for this score yet.</p>
            {/if}
        </section>
    {/if}
    <section class="admin-playtime-section">
        <div class="admin-playtime-header">
            <div>
                <p class="meta-label">Playtime</p>
                <h3>Listening activity</h3>
            </div>
            {#if playtime}
                <span class="status-pill admin-playtime-total-badge">
                    {formatPlaytimeDuration(playtime.total_seconds)} total
                </span>
            {/if}
        </div>

        {#if playtimeLoading}
            <p class="hint">Loading playtime...</p>
        {:else if playtimeError}
            <p class="status error">{playtimeError}</p>
        {:else if playtime}
            <div class="admin-playtime-layout">
                <section class="admin-playtime-card">
                    <h3>User leaderboard</h3>
                    {#if playtime.leaderboard.length === 0}
                        <p class="hint">
                            No user playtime has been recorded yet.
                        </p>
                    {:else}
                        <div class="admin-playtime-user-list">
                            {#each playtime.leaderboard as entry, index}
                                <details class="admin-playtime-user-row">
                                    <summary class="admin-playtime-user-head">
                                        <div
                                            class="admin-user-avatar admin-playtime-avatar"
                                        >
                                            {#if entry.avatar_url}
                                                <img
                                                    src={entry.avatar_url}
                                                    alt=""
                                                    class="admin-user-avatar-img"
                                                />
                                            {:else}
                                                {(entry.display_name ??
                                                    entry.username)
                                                    .slice(0, 1)
                                                    .toUpperCase()}
                                            {/if}
                                        </div>
                                        <div
                                            class="admin-playtime-user-copy"
                                        >
                                            <strong>
                                                #{index + 1}
                                                {entry.display_name ??
                                                    entry.username}
                                            </strong>
                                            <span class="subtle">
                                                @{entry.username}
                                            </span>
                                        </div>
                                        <span class="status-pill">
                                            {formatPlaytimeDuration(
                                                entry.best_track_seconds,
                                            )}
                                        </span>
                                    </summary>
                                    <div class="admin-playtime-track-list">
                                        {#each entry.track_totals as track}
                                            <article
                                                class="admin-playtime-track-row"
                                            >
                                                <div
                                                    class="admin-playtime-track-copy"
                                                >
                                                    <strong>
                                                        {track.track_name}
                                                    </strong>
                                                    <span class="subtle">
                                                        {track.instrument_name}
                                                    </span>
                                                </div>
                                                <strong>
                                                    {formatPlaytimeDuration(
                                                        track.total_seconds,
                                                    )}
                                                </strong>
                                            </article>
                                        {/each}
                                    </div>
                                </details>
                            {/each}
                        </div>
                    {/if}
                </section>
            </div>
        {/if}
    </section>
    <div class="admin-score-links">
        <a href={currentMusic.public_url} target="_blank" rel="noreferrer">
            Public link
        </a>
    </div>
</BaseModal>
