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

    function isReadyMusic(item: AdminMusic) {
        return processingStatuses(item).every((status) => status === "ready");
    }

    function shouldAutoRefreshProcessing(item: AdminMusic) {
        return !isReadyMusic(item);
    }

    const processingSteps = $derived(
        processingProgress?.steps?.length
            ? processingProgress.steps.map((step) => ({
                  ...step,
                  status: step.status as ProcessingStepStatus,
                }))
            : [],
    );
    const processingGridSteps = $derived(buildProcessingGrid(processingSteps));
    const processingStallMessage = $derived(
        processingProgress?.state_message || "",
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
                <p class="hint">Loading processing data...</p>
            {:else if !processingProgressError && !processingLogError && !retryError && (processingLog || processingSteps.length)}
                <div class="processing-journey-shell">
                    {#if processingSteps.length}
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
                    {/if}
                </div>
                <LogBlock
                    logs={processingLog}
                    title=""
                    emptyMessage="No processing log has been recorded for this score yet."
                    showLevelFilter={false}
                    showDownloadButton={false}
                />
            {:else if !processingProgressError && !processingLogError && !retryError}
                <p class="hint">No processing data has been recorded for this score yet.</p>
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
