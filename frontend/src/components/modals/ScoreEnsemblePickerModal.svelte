<script lang="ts">
    import { Search } from "@lucide/svelte";
    import type {
        AdminEnsembleResponse as Ensemble,
        AdminMusicResponse as AdminMusic,
    } from "../../adapters/fumen-backend/src/models";
    import BaseModal from "./BaseModal.svelte";
    import { closeModal } from "./modalState";

    let {
        mode,
        ensembles,
        initialSelectedEnsembleIds,
        music,
        onApply,
        modalId,
    }: {
        mode: "upload" | "score";
        ensembles: Ensemble[];
        initialSelectedEnsembleIds: string[];
        music?: AdminMusic;
        onApply: (ensembleIds: string[]) => void | Promise<void>;
        modalId?: string;
    } = $props();

    let searchQuery = $state("");
    let selectedEnsembleIds = $state([...initialSelectedEnsembleIds]);
    let saving = $state(false);
    let errorMsg = $state("");

    const filteredEnsembles = $derived.by(() =>
        [...ensembles]
            .sort((left, right) => left.name.localeCompare(right.name))
            .filter((ensemble) => {
                const query = searchQuery.trim().toLowerCase();
                if (!query) return true;
                return ensemble.name.toLowerCase().includes(query);
            }),
    );

    const subtitle = $derived(
        mode === "upload"
            ? "Choose ensembles for the new score"
            : `Manage ensembles for ${music?.title ?? "score"}`,
    );

    const confirmText = $derived(
        mode === "upload" ? "Apply" : "Save changes",
    );

    function isSelected(ensembleId: string) {
        return selectedEnsembleIds.includes(ensembleId);
    }

    function toggleSelection(ensembleId: string, checked: boolean) {
        if (checked) {
            if (!selectedEnsembleIds.includes(ensembleId)) {
                selectedEnsembleIds = [...selectedEnsembleIds, ensembleId];
            }
            return;
        }

        selectedEnsembleIds = selectedEnsembleIds.filter(
            (id) => id !== ensembleId,
        );
    }

    async function handleApply() {
        saving = true;
        errorMsg = "";
        try {
            await onApply([...selectedEnsembleIds].sort((a, b) => a.localeCompare(b)));
            closeModal();
        } catch (error) {
            errorMsg =
                error instanceof Error
                    ? error.message
                    : "Unable to save ensembles.";
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
            disabled={saving}
            onclick={() => void handleApply()}
        >
            {saving ? "Saving..." : confirmText}
        </button>
    </div>
{/snippet}

<BaseModal
    size="medium"
    cardClass="admin-selector-modal"
    title="Ensembles"
    {subtitle}
    {footer}
    canClose={!saving}
    {modalId}
>
    <label class="field admin-user-search">
        <span class="sr-only">Search ensembles</span>
        <div class="admin-user-search-input-wrap">
            <Search size={15} aria-hidden="true" />
            <input bind:value={searchQuery} placeholder="Search ensembles" />
        </div>
    </label>
    <div class="admin-inline-list admin-selector-list">
        {#if filteredEnsembles.length === 0}
            <p class="hint">No ensembles match this search.</p>
        {:else}
            {#each filteredEnsembles as ensemble}
                <label class="admin-selector-row">
                    <div class="admin-inline-copy">
                        <strong>{ensemble.name}</strong>
                        <span class="admin-user-role-pill">
                            {ensemble.members.length} members
                        </span>
                    </div>
                    <input
                        type="checkbox"
                        checked={isSelected(ensemble.id)}
                        onchange={(event) =>
                            toggleSelection(
                                ensemble.id,
                                (event.currentTarget as HTMLInputElement).checked,
                            )}
                    />
                </label>
            {/each}
        {/if}
    </div>
    {#if errorMsg}
        <p class="admin-error">{errorMsg}</p>
    {/if}
</BaseModal>
