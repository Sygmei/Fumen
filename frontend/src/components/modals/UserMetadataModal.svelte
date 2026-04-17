<script lang="ts">
    import { onMount } from "svelte";
    import type {
        AdminUserMetadataResponse as AdminUserMetadata,
        UserResponse as AppUser,
    } from "$backend/models";
    import { formatPlaytimeDuration, prettyDate } from "$lib/utils";
    import BaseModal from "./BaseModal.svelte";

    let {
        user,
        loadMetadata,
        modalId,
    }: {
        user: AppUser;
        loadMetadata: (userId: string) => Promise<AdminUserMetadata>;
        modalId?: string;
    } = $props();

    let metadataLoading = $state(true);
    let metadataError = $state("");
    let metadata = $state<AdminUserMetadata | null>(null);

    onMount(() => {
        let cancelled = false;

        const run = async () => {
            metadataLoading = true;
            metadataError = "";
            try {
                const response = await loadMetadata(user.id);
                if (!cancelled) {
                    metadata = response;
                }
            } catch (error) {
                if (!cancelled) {
                    metadataError =
                        error instanceof Error
                            ? error.message
                            : "Unable to load metadata";
                }
            } finally {
                if (!cancelled) {
                    metadataLoading = false;
                }
            }
        };

        void run();

        return () => {
            cancelled = true;
        };
    });
</script>

<BaseModal
    title="User metadata"
    subtitle={user.display_name ?? `@${user.username}`}
    size="large"
    cardClass="admin-user-modal admin-user-metadata-modal"
    {modalId}
>
    <div class="admin-user-metadata-summary">
        <article class="admin-user-metadata-stat">
            <span>Last login</span>
            <strong>
                {metadata?.last_login_at
                    ? prettyDate(metadata.last_login_at)
                    : metadataLoading
                        ? "Loading..."
                        : "Never"}
            </strong>
        </article>
        <article class="admin-user-metadata-stat">
            <span>Total playtime</span>
            <strong>
                {metadata
                    ? formatPlaytimeDuration(metadata.total_playtime_seconds)
                    : metadataLoading
                        ? "Loading..."
                        : "0s"}
            </strong>
        </article>
        <article class="admin-user-metadata-stat">
            <span>Scores played</span>
            <strong>
                {metadata
                    ? metadata.score_playtimes.length
                    : metadataLoading
                        ? "Loading..."
                        : 0}
            </strong>
        </article>
    </div>

    {#if metadataLoading}
        <p class="hint">Loading metadata...</p>
    {:else if metadataError}
        <p class="status error">{metadataError}</p>
    {:else if metadata}
        <section class="admin-user-metadata-section">
            <div class="admin-playtime-header">
                <div>
                    <p class="meta-label">Scores</p>
                    <h3>Playtime by score</h3>
                </div>
            </div>

            {#if metadata.score_playtimes.length === 0}
                <p class="hint">No score playtime has been recorded yet.</p>
            {:else}
                <div class="admin-user-metadata-score-list">
                    {#each metadata.score_playtimes as score}
                        <a
                            class="admin-user-metadata-score-row"
                            href={score.public_url}
                            target="_blank"
                            rel="noreferrer"
                        >
                            <span
                                class="admin-user-metadata-score-icon"
                                aria-hidden="true"
                            >
                                {#if score.icon_image_url}
                                    <img
                                        src={score.icon_image_url}
                                        alt=""
                                        class="admin-user-metadata-score-icon-img"
                                    />
                                {:else}
                                    {score.icon ?? ""}
                                {/if}
                            </span>
                            <span class="admin-user-metadata-score-copy">
                                <strong>{score.title}</strong>
                                {#if score.subtitle}
                                    <span class="subtle">{score.subtitle}</span>
                                {:else}
                                    <span class="subtle">Public score link</span>
                                {/if}
                            </span>
                            <span class="status-pill">
                                {formatPlaytimeDuration(score.total_seconds)}
                            </span>
                        </a>
                    {/each}
                </div>
            {/if}
        </section>
    {/if}
</BaseModal>
