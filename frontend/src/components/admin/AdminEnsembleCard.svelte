<script lang="ts">
    import type { AdminEnsembleResponse as Ensemble } from "$backend/models";
    import AdminCard from "./AdminCard.svelte";
    import {
        Trash2,
        UserPlus,
        Music,
        Users,
    } from "@lucide/svelte";

    let {
        ensemble,
        creating = false,
        saving = false,
        deleting = false,
        canManageScores = false,
        canManageMembers = false,
        canDelete = false,
        onManageScores,
        onManageMembers,
        onDelete,
    }: {
        ensemble: Ensemble;
        creating?: boolean;
        saving?: boolean;
        deleting?: boolean;
        canManageScores?: boolean;
        canManageMembers?: boolean;
        canDelete?: boolean;
        onManageScores: () => void;
        onManageMembers: () => void;
        onDelete: () => void;
    } = $props();

    const cardState = $derived(
        deleting ? "deleting" : saving ? "saving" : creating ? "creating" : "",
    );
    const actionsDisabled = $derived(creating || saving || deleting);
</script>

{#snippet body()}
    <div class="admin-record-main">
        <div class="admin-record-avatar admin-record-avatar-gradient" aria-hidden="true">
            <Users size={16} aria-hidden="true" />
        </div>
        <div class="admin-record-copy">
            <h3>{ensemble.name}</h3>
            <div class="admin-user-state-row">
                {#if creating}
                    <span class="status-pill admin-user-creating-pill">
                        CREATING
                    </span>
                {:else if saving}
                    <span class="status-pill admin-user-saving-pill">
                        SAVING
                    </span>
                {:else if deleting}
                    <span class="status-pill admin-user-deleting-pill">
                        DELETING
                    </span>
                {/if}
            </div>
        </div>
    </div>
{/snippet}

{#snippet footer()}
    <div class="admin-record-actions admin-record-actions-compact">
        <button
            class="button secondary admin-user-action"
            type="button"
            onclick={onManageScores}
            aria-label={`Manage scores for ${ensemble.name}`}
            title="Manage scores"
            disabled={actionsDisabled || !canManageScores}
        >
            <Music size={16} aria-hidden="true" />
            <span class="admin-action-badge" aria-hidden="true">{ensemble.score_count}</span>
        </button>
        <button
            class="button secondary admin-user-action"
            type="button"
            onclick={onManageMembers}
            aria-label={`Manage members for ${ensemble.name}`}
            title="Manage members"
            disabled={actionsDisabled || !canManageMembers}
        >
            <UserPlus size={16} aria-hidden="true" />
            <span class="admin-action-badge" aria-hidden="true">{ensemble.members.length}</span>
        </button>
        {#if canDelete}
            <button
                class="button ghost danger admin-user-action"
                type="button"
                disabled={actionsDisabled}
                onclick={onDelete}
                aria-label={`Delete ${ensemble.name}`}
                title="Delete ensemble"
            >
                <Trash2 size={16} aria-hidden="true" />
            </button>
        {/if}
    </div>
{/snippet}

<AdminCard
    cardClass={`admin-record-card admin-ensemble-card${cardState ? ` is-${cardState}` : ""}`}
    {body}
    {footer}
/>
