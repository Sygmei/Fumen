<script lang="ts">
    import { Search } from "@lucide/svelte";
    import type {
        AdminEnsembleResponse as Ensemble,
        AdminMusicResponse as AdminMusic,
    } from "$backend/models";
    import BaseModal from "./BaseModal.svelte";
    import { closeModal } from "./modalState";

    let {
        ensemble,
        allMusics,
        onSave,
        modalId,
    }: {
        ensemble: Ensemble;
        allMusics: AdminMusic[];
        onSave: (ensembleId: string, musicIds: string[]) => Promise<void>;
        modalId?: string;
    } = $props();

    const initialMusicIds = allMusics
        .filter((music) => music.ensemble_ids.includes(ensemble.id))
        .map((music) => music.id);

    let currentSearchQuery = $state("");
    let addSearchQuery = $state("");
    let originalMusicIds = $state([...initialMusicIds]);
    let stagedMusicIds = $state([...initialMusicIds]);
    let saving = $state(false);
    let errorMsg = $state("");

    const filteredManagedScores = $derived.by(() =>
        [...allMusics]
            .filter((music) => stagedMusicIds.includes(music.id))
            .sort((left, right) => left.title.localeCompare(right.title))
            .filter((music) => {
                const query = currentSearchQuery.trim().toLowerCase();
                if (!query) return true;
                return [music.title, music.public_id ?? "", ...music.ensemble_names]
                    .join(" ")
                    .toLowerCase()
                    .includes(query);
            }),
    );

    const filteredAvailableScores = $derived.by(() =>
        [...allMusics]
            .filter((music) => !stagedMusicIds.includes(music.id))
            .sort((left, right) => left.title.localeCompare(right.title))
            .filter((music) => {
                const query = addSearchQuery.trim().toLowerCase();
                if (!query) return true;
                return [music.title, music.public_id ?? "", ...music.ensemble_names]
                    .join(" ")
                    .toLowerCase()
                    .includes(query);
            }),
    );

    function toggleStagedEnsembleScore(musicId: string, shouldAdd: boolean) {
        if (shouldAdd) {
            stagedMusicIds = [...stagedMusicIds, musicId];
            return;
        }
        stagedMusicIds = stagedMusicIds.filter((id) => id !== musicId);
    }

    function hasManagedScoreChanges(): boolean {
        const original = new Set(originalMusicIds);
        const staged = new Set(stagedMusicIds);
        if (original.size !== staged.size) return true;
        return [...staged].some((id) => !original.has(id));
    }

    async function handleSave() {
        if (!hasManagedScoreChanges()) {
            closeModal();
            return;
        }

        saving = true;
        errorMsg = "";
        try {
            await onSave(
                ensemble.id,
                [...new Set(stagedMusicIds)].sort((left, right) =>
                    left.localeCompare(right),
                ),
            );
            originalMusicIds = [...stagedMusicIds];
            closeModal();
        } catch (error) {
            errorMsg =
                error instanceof Error
                    ? error.message
                    : "Failed to update scores.";
        } finally {
            saving = false;
        }
    }
</script>

{#snippet footer()}
    <div class="actions admin-user-modal-actions">
        <button
            class="button ghost"
            type="button"
            disabled={saving}
            onclick={closeModal}
        >
            Cancel
        </button>
        <button
            class="button"
            type="button"
            disabled={saving || !hasManagedScoreChanges()}
            onclick={() => void handleSave()}
        >
            {saving ? "Saving..." : "Save changes"}
        </button>
    </div>
{/snippet}

<BaseModal
    size="full"
    cardClass="admin-split-modal"
    title="Scores"
    subtitle={ensemble.name}
    {footer}
    canClose={!saving}
    {modalId}
>
    <div class="admin-split-pane">
        <section class="admin-split-column">
            <div class="admin-split-header">
                <div class="admin-split-header-main">
                    <h4>Current scores</h4>
                    <label class="field admin-user-search admin-split-search">
                        <span class="sr-only">Search current scores</span>
                        <div class="admin-user-search-input-wrap">
                            <Search size={15} aria-hidden="true" />
                            <input
                                bind:value={currentSearchQuery}
                                placeholder="Search current scores"
                            />
                        </div>
                    </label>
                    <span class="admin-user-role-pill">
                        {filteredManagedScores.length}
                    </span>
                </div>
            </div>
            <div class="admin-inline-list">
                {#if filteredManagedScores.length === 0}
                    <p class="hint">No matching scores in this ensemble.</p>
                {:else}
                    {#each filteredManagedScores as music}
                        <div class="admin-inline-row admin-inline-row-score">
                            <div class="admin-inline-copy">
                                <strong>{music.title}</strong>
                            </div>
                            <div class="admin-inline-actions">
                                <button
                                    class="button ghost danger admin-inline-icon-btn admin-inline-symbol-btn"
                                    type="button"
                                    disabled={saving}
                                    aria-label={`Remove ${music.title}`}
                                    title={`Remove ${music.title}`}
                                    onclick={() =>
                                        toggleStagedEnsembleScore(
                                            music.id,
                                            false,
                                        )}
                                >
                                    <span aria-hidden="true">-</span>
                                </button>
                            </div>
                        </div>
                    {/each}
                {/if}
            </div>
        </section>
        <section class="admin-split-column">
            <div class="admin-split-header">
                <div class="admin-split-header-main">
                    <h4>Add scores</h4>
                    <label class="field admin-user-search admin-split-search">
                        <span class="sr-only">Search available scores</span>
                        <div class="admin-user-search-input-wrap">
                            <Search size={15} aria-hidden="true" />
                            <input
                                bind:value={addSearchQuery}
                                placeholder="Search available scores"
                            />
                        </div>
                    </label>
                    <span class="admin-user-role-pill">
                        {filteredAvailableScores.length}
                    </span>
                </div>
            </div>
            <div class="admin-inline-list">
                {#if filteredAvailableScores.length === 0}
                    <p class="hint">No available scores.</p>
                {:else}
                    {#each filteredAvailableScores as music}
                        <div class="admin-inline-row admin-inline-row-score">
                            <div class="admin-inline-copy">
                                <strong>{music.title}</strong>
                            </div>
                            <div class="admin-inline-actions">
                                <button
                                    class="button secondary admin-inline-icon-btn admin-inline-symbol-btn"
                                    type="button"
                                    disabled={saving}
                                    aria-label={`Add ${music.title}`}
                                    title={`Add ${music.title}`}
                                    onclick={() =>
                                        toggleStagedEnsembleScore(
                                            music.id,
                                            true,
                                        )}
                                >
                                    <span aria-hidden="true">+</span>
                                </button>
                            </div>
                        </div>
                    {/each}
                {/if}
            </div>
        </section>
    </div>
    {#if errorMsg}
        <p class="admin-error">{errorMsg}</p>
    {/if}
</BaseModal>
