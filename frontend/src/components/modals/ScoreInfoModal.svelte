<script lang="ts">
    import { onMount } from "svelte";
    import type {
        AdminMusicPlaytimeResponse as AdminMusicPlaytime,
        AdminMusicProcessingLogResponse,
        AdminMusicProcessingProgressResponse,
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
    import {
        STEM_QUALITY_PROFILES,
        type StemQualityProfile,
    } from "$lib/stem-quality";
    import CustomSelect from "$components/CustomSelect.svelte";
    import LogBlock from "$components/log-block/LogBlock.svelte";
    import BaseModal from "./BaseModal.svelte";

    let {
        music,
        currentUser,
        loadPlaytime,
        loadProcessingLog,
        loadProcessingProgress,
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
        loadProcessingProgress: (
            musicId: string,
        ) => Promise<AdminMusicProcessingProgressResponse>;
        reloadMusic: (musicId: string) => Promise<AdminMusic>;
        canViewProcessingLog?: boolean;
        onRetryRender: (
            musicId: string,
            qualityProfile?: StemQualityProfile,
        ) => Promise<AdminMusic>;
        modalId?: string;
    } = $props();

    const qualityOptions = STEM_QUALITY_PROFILES.map((profile) => ({
        value: profile.value,
        label: profile.label,
        description: profile.description,
    }));

    function normalizeStemQualityProfile(value?: string | null): StemQualityProfile {
        return (
            STEM_QUALITY_PROFILES.find((profile) => profile.value === value)
                ?.value ?? "balanced"
        );
    }

    let currentMusic = $state(music);
    let retryQualityProfile = $state<StemQualityProfile>(
        normalizeStemQualityProfile(music.quality_profile),
    );
    let playtime = $state<AdminMusicPlaytime | null>(null);
    let playtimeLoading = $state(true);
    let playtimeError = $state("");
    let processingLog = $state("");
    let processingProgress = $state<AdminMusicProcessingProgressResponse | null>(
        null,
    );
    let processingLogLoading = $state(false);
    let processingLogError = $state("");
    let processingProgressError = $state("");
    let retrying = $state(false);
    let retryError = $state("");

    type ProcessingStepStatus =
        | "done"
        | "active"
        | "pending"
        | "failed"
        | "stalled";
    type ProcessingStep = {
        key: string;
        label: string;
        detail?: string | null;
        last_updated_at?: string | null;
        status: ProcessingStepStatus;
        tooltip?: string | null;
        group?: string | null;
    };
    type ProcessingGridStep = {
        step: ProcessingStep;
        column: number;
        row: number;
    };

    function processingStatuses(item: AdminMusic) {
        return [
            item.audio_status,
            item.midi_status,
            item.musicxml_status,
            item.stems_status,
        ];
    }

    function processingJobStatus(item: AdminMusic) {
        return item.processing_job_status ?? null;
    }

    function processingJobStep(item: AdminMusic) {
        return item.processing_job_step ?? null;
    }

    function parseTimestampMs(value?: string | null) {
        if (!value) return null;
        const timestamp = Date.parse(value);
        return Number.isFinite(timestamp) ? timestamp : null;
    }

    function processingJobLeaseExpired(item: AdminMusic) {
        if (processingJobStatus(item) !== "running") {
            return false;
        }

        const leaseExpiresAt = parseTimestampMs(
            item.processing_job_lease_expires_at,
        );
        return leaseExpiresAt !== null && leaseExpiresAt <= Date.now();
    }

    function processingJobStallMessage(item: AdminMusic) {
        if (!processingJobLeaseExpired(item) || hasProcessingFailure(item)) {
            return "";
        }

        const heartbeatAt = item.processing_job_heartbeat_at;
        return heartbeatAt
            ? `Processor worker heartbeat stopped after ${prettyDate(heartbeatAt)}. Waiting for another worker to reclaim the job.`
            : "Processor worker lease expired. Waiting for another worker to reclaim the job.";
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

    function logHasLine(logText: string, matcher: (line: string) => boolean) {
        return logText.split(/\r?\n/).some((line) => matcher(line.toLowerCase()));
    }

    function deriveProcessingSteps(item: AdminMusic, logText: string): ProcessingStep[] {
        const lower = logText.toLowerCase();
        const storageLabel = lower.includes(" s3") ? "S3" : "Storage";
        const jobStatus = processingJobStatus(item);
        const jobStep = processingJobStep(item);
        const failed =
            jobStatus === "failed" ||
            hasProcessingFailure(item) ||
            lower.includes("processing failed") ||
            lower.includes(" failed.");

        if (jobStatus === "queued") {
            return [
                {
                    key: "queue",
                    label: "Queue",
                    detail: "waiting worker",
                    status: "active",
                },
                {
                    key: "input",
                    label: "Input",
                    detail: "staged",
                    status: "pending",
                },
                {
                    key: "musicxml",
                    label: "MusicXML",
                    detail: "notation export",
                    status: "pending",
                    group: "core_exports",
                },
                {
                    key: "midi",
                    label: "MIDI",
                    status: "pending",
                    group: "core_exports",
                },
                {
                    key: "preview_mp3",
                    label: "Audio",
                    status: "pending",
                    group: "core_exports",
                },
                {
                    key: "stems",
                    label: "Stems",
                    detail: "track render",
                    status: "pending",
                },
                {
                    key: "compress_stems",
                    label: "Compress",
                    status: "pending",
                },
                {
                    key: "storage",
                    label: storageLabel,
                    detail: "asset upload",
                    status: "pending",
                },
                {
                    key: "ready",
                    label: "Ready",
                    detail: "published",
                    status: "pending",
                },
            ];
        }

        const hasInputActivity =
            lower.includes("processing restart requested") ||
            lower.includes("processor worker") ||
            lower.includes("fetching source score") ||
            lower.includes("writing temporary input file") ||
            lower.includes("temporary input file written") ||
            lower.includes("score upload");
        const hasMusicxmlCompletion =
            lower.includes("audio, midi, and musicxml conversion finished");
        const hasMusicxmlDoneLine =
            logHasLine(
                logText,
                (line) =>
                    line.includes("score.musicxml") &&
                    line.includes("musescore: done"),
            ) ||
            logHasLine(
                logText,
                (line) =>
                    line.includes("application/xml") &&
                    line.includes("musescore: done"),
            );
        const hasMidiDoneLine =
            logHasLine(
                logText,
                (line) =>
                    line.includes("preview.mid") &&
                    line.includes("musescore: done"),
            ) ||
            logHasLine(
                logText,
                (line) =>
                    line.includes("audio/midi") &&
                    line.includes("musescore: done"),
            );
        const hasPreviewDoneLine =
            logHasLine(
                logText,
                (line) =>
                    line.includes("preview.mp3") &&
                    line.includes("musescore: done"),
            ) ||
            logHasLine(
                logText,
                (line) =>
                    line.includes("audio/mpeg") &&
                    line.includes("musescore: done"),
            );
        const hasStemRenderActivity =
            lower.includes("stems: exporting midi from score") ||
            lower.includes("stems: found ") ||
            lower.includes("stems: [") ||
            lower.includes("rendering via musescore") ||
            lower.includes("stem generation finished");
        const hasStorageActivity =
            lower.includes("uploading ") ||
            lower.includes(" uploaded ") ||
            lower.includes("upload to s3") ||
            lower.includes("upload to storage") ||
            lower.includes("database state updated");
        const hasCompressionActivity =
            lower.includes("stems: compressing [") ||
            lower.includes("stems: compressed [");
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
            item.musicxml_status !== "processing" ||
            hasMusicxmlCompletion ||
            hasMusicxmlDoneLine;
        const midiDone =
            item.midi_status !== "processing" ||
            hasMusicxmlCompletion ||
            hasMidiDoneLine;
        const previewDone =
            item.audio_status !== "processing" ||
            hasMusicxmlCompletion ||
            hasPreviewDoneLine;
        const stemsStarted = jobStep === "generating_stems" || hasStemRenderActivity;
        const stemsDone =
            item.stems_status !== "processing" ||
            lower.includes("stem generation finished") ||
            lower.includes("stems: upload") ||
            lower.includes("stems status:");
        const compressionDone =
            item.stems_status !== "processing" ||
            (hasCompressionActivity && lower.includes("stem generation finished"));
        const storageStarted = hasStorageActivity || (stemsDone && !isComplete);
        const storageDone = isComplete;
        const leaseExpired = processingJobLeaseExpired(item) && !isComplete && !failed;

        let failedIndex: number | null = null;
        if (failed) {
            if (item.musicxml_status === "failed") {
                failedIndex = 2;
            } else if (item.midi_status === "failed") {
                failedIndex = 3;
            } else if (item.audio_status === "failed") {
                failedIndex = 4;
            } else if (item.stems_status === "failed") {
                failedIndex = hasCompressionActivity ? 6 : 5;
            } else if (!musicxmlDone) {
                failedIndex = 2;
            } else if (!midiDone) {
                failedIndex = 3;
            } else if (!previewDone) {
                failedIndex = 4;
            } else if (!stemsDone) {
                failedIndex = 5;
            } else if (hasCompressionActivity && !compressionDone) {
                failedIndex = 6;
            } else {
                failedIndex = 7;
            }
        }

        const steps = [
            {
                key: "queue",
                label: "Queue",
                detail: "claimed",
            },
            {
                key: "input",
                label: "Input",
                detail: "staged",
            },
            {
                key: "musicxml",
                label: "MusicXML",
                group: "core_exports",
            },
            {
                key: "midi",
                label: "MIDI",
                group: "core_exports",
            },
            {
                key: "preview_mp3",
                label: "Audio",
                group: "core_exports",
            },
            {
                key: "stems",
                label: "Stems",
                detail: "track render",
            },
            {
                key: "compress_stems",
                label: "Compress",
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

        const stalledDetails = [
            "worker lost",
            "worker lease expired",
            "worker lease expired",
            "worker lease expired",
            "worker lease expired",
            "worker lease expired",
            "worker lease expired",
            "worker lease expired",
            "worker lease expired",
        ];

        const statuses: ProcessingStepStatus[] = [
            jobStatus === "running" || isComplete || failed ? "done" : "active",
            inputDone
                ? "done"
                : jobStep === "fetching_input" || jobStep === "generating_core"
                  ? "active"
                  : "pending",
            musicxmlDone
                ? "done"
                : jobStep === "generating_core" || inputDone
                  ? "active"
                  : "pending",
            midiDone
                ? "done"
                : jobStep === "generating_core" || inputDone
                  ? "active"
                  : "pending",
            previewDone
                ? "done"
                : jobStep === "generating_core" || inputDone
                  ? "active"
                  : "pending",
            stemsDone
                ? "done"
                : stemsStarted
                  ? "active"
                  : "pending",
            compressionDone
                ? "done"
                : hasCompressionActivity || (jobStep === "generating_stems" && stemsDone)
                  ? "active"
                  : "pending",
            storageDone
                ? "done"
                : jobStep === "uploading_assets" || storageStarted
                  ? "active"
                  : "pending",
            isComplete || jobStatus === "completed"
                ? "done"
                : jobStep === "finalizing"
                  ? "active"
                  : "pending",
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
        } else if (leaseExpired) {
            const stalledIndex =
                jobStep === "finalizing"
                    ? 8
                    : jobStep === "uploading_assets"
                      ? 7
                      : jobStep === "generating_stems"
                        ? hasCompressionActivity
                            ? 6
                            : 5
                        : jobStep === "generating_core"
                          ? 2
                          : 1;

            for (let index = 0; index < statuses.length; index++) {
                if (index < stalledIndex) {
                    statuses[index] = "done";
                } else if (index === stalledIndex) {
                    statuses[index] = "stalled";
                    steps[index].detail = stalledDetails[index];
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

    const processingSteps = $derived(
        processingProgress?.steps?.length
            ? processingProgress.steps.map((step) => ({
                  ...step,
                  status: step.status as ProcessingStepStatus,
                }))
            : deriveProcessingSteps(currentMusic, processingLog),
    );
    const processingGridSteps = $derived(buildProcessingGrid(processingSteps));
    const processingStallMessage = $derived(
        processingProgress?.state_message || processingJobStallMessage(currentMusic),
    );

    function stepTooltip(step: ProcessingStep) {
        return step.tooltip || (step.last_updated_at
            ? `Last update: ${step.last_updated_at}`
            : "");
    }

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

    async function refreshProcessingProgress() {
        if (!canViewProcessingLog) return;

        processingProgressError = "";
        try {
            processingProgress = await loadProcessingProgress(currentMusic.id);
        } catch (error) {
            processingProgress = null;
            processingProgressError =
                error instanceof Error
                    ? error.message
                    : "Unable to load processing progress";
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
                await Promise.all([
                    refreshProcessingLog(),
                    refreshProcessingProgress(),
                ]);
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
                        await Promise.all([
                            refreshProcessingLog(false),
                            refreshProcessingProgress(),
                        ]);
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
            currentMusic = await onRetryRender(
                currentMusic.id,
                retryQualityProfile,
            );
            retryQualityProfile = normalizeStemQualityProfile(
                currentMusic.quality_profile,
            );
            await Promise.all([
                refreshProcessingLog(),
                refreshProcessingProgress(),
            ]);
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

    function buildProcessingGrid(steps: ProcessingStep[]): ProcessingGridStep[] {
        const slots: Array<{
            column: number;
            row: number;
            keys: string[];
        }> = [
            { column: 1, row: 2, keys: ["upload", "input"] },
            { column: 2, row: 2, keys: ["queue"] },
            { column: 3, row: 1, keys: ["musicxml"] },
            { column: 3, row: 2, keys: ["midi"] },
            { column: 3, row: 3, keys: ["preview_mp3"] },
            { column: 4, row: 2, keys: ["stems"] },
            { column: 5, row: 2, keys: ["compress_stems"] },
            { column: 6, row: 2, keys: ["storage", "upload_assets"] },
            { column: 7, row: 2, keys: ["ready", "done"] },
        ];

        const used = new Set<string>();
        const placed: ProcessingGridStep[] = [];

        for (const slot of slots) {
            const step = steps.find(
                (candidate) =>
                    !used.has(candidate.key) && slot.keys.includes(candidate.key),
            );
            if (!step) {
                continue;
            }

            used.add(step.key);
            placed.push({
                step,
                column: slot.column,
                row: slot.row,
            });
        }

        return placed;
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
                    {#if canEditOwnedScore(currentMusic, currentUser)}
                        <label class="processing-retry-controls">
                            <span class="processing-retry-inline-label">Re-process as</span>
                            <div class="processing-quality-select">
                                <CustomSelect
                                    bind:value={retryQualityProfile}
                                    options={qualityOptions}
                                    compact={true}
                                    showDescriptionInTrigger={false}
                                    disabled={retrying}
                                />
                            </div>
                        </label>
                        <button
                            class="button ghost"
                            type="button"
                            disabled={retrying}
                            onclick={() => void handleRetryRender()}
                        >
                            {retrying ? "Re-processing..." : "Re-process"}
                        </button>
                    {/if}
                </div>
            </div>

            {#if processingProgressError}
                <p class="status error">{processingProgressError}</p>
            {/if}
            {#if processingLogError}
                <p class="status error">{processingLogError}</p>
            {/if}
            {#if retryError}
                <p class="status error">{retryError}</p>
            {/if}
            {#if processingStallMessage}
                <p class="status warning">{processingStallMessage}</p>
            {/if}

            {#if !processingProgressError && !processingLogError && !retryError && processingLogLoading}
                <p class="hint">Loading processing log...</p>
            {:else if !processingProgressError && !processingLogError && !retryError && (processingLog || processingSteps.length)}
                <div class="processing-journey-shell">
                    <div class="processing-journey" aria-label="Processing progress">
                        {#each processingGridSteps as cell (cell.step.key)}
                            {@const step = cell.step}
                        <article
                            class={`processing-stop is-${step.status} processing-cell row-${cell.row} ${stepTooltip(step) ? "has-tooltip" : ""}`}
                            title={stepTooltip(step)}
                            style={`grid-column: ${cell.column}; grid-row: ${cell.row};`}
                        >
                            <span class="processing-stop-node" aria-hidden="true"></span>
                            <div class="processing-stop-copy">
                                <strong>{step.label}</strong>
                                {#if step.detail}
                                    <span>{step.detail}</span>
                                {/if}
                            </div>
                        </article>
                        {/each}
                    </div>
                </div>
                <LogBlock
                    logs={processingLog}
                    title=""
                    emptyMessage="No processing log has been recorded for this score yet."
                    showLevelFilter={false}
                    showDownloadButton={false}
                />
            {:else if !processingProgressError && !processingLogError && !retryError}
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
