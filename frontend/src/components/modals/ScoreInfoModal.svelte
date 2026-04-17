<script lang="ts">
    import { onMount } from "svelte";
    import type {
        AdminMusicPlaytimeResponse as AdminMusicPlaytime,
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
    import BaseModal from "./BaseModal.svelte";

    let {
        music,
        currentUser,
        loadPlaytime,
        onRetryRender,
        modalId,
    }: {
        music: AdminMusic;
        currentUser: AppUser;
        loadPlaytime: (musicId: string) => Promise<AdminMusicPlaytime>;
        onRetryRender: (musicId: string) => Promise<AdminMusic>;
        modalId?: string;
    } = $props();

    let currentMusic = $state(music);
    let playtime = $state<AdminMusicPlaytime | null>(null);
    let playtimeLoading = $state(true);
    let playtimeError = $state("");
    let retrying = $state(false);
    let retryError = $state("");

    onMount(() => {
        let cancelled = false;

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
        };

        void run();

        return () => {
            cancelled = true;
        };
    });

    async function handleRetryRender() {
        retrying = true;
        retryError = "";
        try {
            currentMusic = await onRetryRender(currentMusic.id);
        } catch (error) {
            retryError =
                error instanceof Error ? error.message : "Retry failed.";
        } finally {
            retrying = false;
        }
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
    {#if retryError}
        <p class="status error">{retryError}</p>
    {/if}
    {#if currentMusic.stems_status !== "ready" && canEditOwnedScore(currentMusic, currentUser)}
        <div class="actions admin-user-modal-actions">
            <button
                class="button ghost"
                type="button"
                disabled={retrying}
                onclick={() => void handleRetryRender()}
            >
                {retrying ? "Retrying render..." : "Retry render"}
            </button>
        </div>
    {/if}
</BaseModal>
