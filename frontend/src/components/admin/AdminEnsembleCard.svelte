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
        deleting = false,
        canManageScores = false,
        canManageMembers = false,
        canDelete = false,
        onManageScores,
        onManageMembers,
        onDelete,
    }: {
        ensemble: Ensemble;
        deleting?: boolean;
        canManageScores?: boolean;
        canManageMembers?: boolean;
        canDelete?: boolean;
        onManageScores: () => void;
        onManageMembers: () => void;
        onDelete: () => void;
    } = $props();
</script>

{#snippet body()}
    <div class="admin-record-main">
        <div class="admin-record-avatar admin-record-avatar-gradient" aria-hidden="true">
            <Users size={16} aria-hidden="true" />
        </div>
        <div class="admin-record-copy">
            <h3>{ensemble.name}</h3>
        </div>
    </div>
{/snippet}

{#snippet footer()}
    <div class="admin-record-actions">
        <button
            class="button secondary admin-user-action"
            type="button"
            onclick={onManageScores}
            aria-label={`Manage scores for ${ensemble.name}`}
            title="Manage scores"
            disabled={!canManageScores}
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
            disabled={!canManageMembers}
        >
            <UserPlus size={16} aria-hidden="true" />
            <span class="admin-action-badge" aria-hidden="true">{ensemble.members.length}</span>
        </button>
        {#if canDelete}
            <button
                class="button ghost danger admin-user-action"
                type="button"
                disabled={deleting}
                onclick={onDelete}
                aria-label={`Delete ${ensemble.name}`}
                title="Delete ensemble"
            >
                <Trash2 size={16} aria-hidden="true" />
            </button>
        {/if}
    </div>
{/snippet}

<AdminCard cardClass="admin-record-card admin-ensemble-card" {body} {footer} />
