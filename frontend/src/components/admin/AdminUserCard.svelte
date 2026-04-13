<script lang="ts">
    import type { UserResponse as AppUser } from "$backend/models";
    import AdminCard from "./AdminCard.svelte";
    import {
        QrCode,
        Trash2,
        Pencil,
        Info,
    } from "@lucide/svelte";

    let {
        user,
        saving = false,
        deleting = false,
        canEdit = false,
        canViewMetadata = false,
        canDelete = false,
        onEdit,
        onShowMetadata,
        onShowQr,
        onDelete,
    }: {
        user: AppUser;
        saving?: boolean;
        deleting?: boolean;
        canEdit?: boolean;
        canViewMetadata?: boolean;
        canDelete?: boolean;
        onEdit: () => void;
        onShowMetadata: () => void;
        onShowQr: () => void;
        onDelete: () => void;
    } = $props();
</script>

{#snippet body()}
    <div class="admin-record-main">
        <div class="admin-record-avatar admin-record-avatar-gradient" aria-hidden="true">
            {#if user.avatar_url}
                <img
                    src={user.avatar_url}
                    alt=""
                    class="admin-user-avatar-img"
                />
            {:else}
                {user.username.slice(0, 1).toUpperCase()}
            {/if}
        </div>
        <div class="admin-record-copy">
            <h3>
                {#if user.display_name}
                    {user.display_name} -
                    <span class="admin-user-handle">@{user.username}</span>
                {:else}
                    @{user.username}
                {/if}
            </h3>
            <div class="admin-user-state-row">
                <p class="admin-user-role-pill">{user.role}</p>
                {#if saving}
                    <span class="status-pill admin-user-saving-pill">Saving...</span>
                {/if}
            </div>
        </div>
    </div>
{/snippet}

{#snippet footer()}
    <div class="admin-record-actions">
        {#if canEdit}
            <button
                class="button secondary admin-user-action"
                type="button"
                onclick={onEdit}
                aria-label={`Edit ${user.username}`}
                title="Edit user"
            >
                <Pencil size={15} aria-hidden="true" />
            </button>
        {/if}
        {#if canViewMetadata}
            <button
                class="button secondary admin-user-action"
                type="button"
                onclick={onShowMetadata}
                aria-label={`View metadata for ${user.username}`}
                title="User metadata"
            >
                <Info size={15} aria-hidden="true" />
            </button>
        {/if}
        <button
            class="button secondary admin-user-action"
            type="button"
            onclick={onShowQr}
            aria-label={`Show QR code for ${user.username}`}
            title="Show QR code"
        >
            <QrCode size={15} aria-hidden="true" />
        </button>
        {#if canDelete}
            <button
                class="button ghost danger admin-user-action"
                type="button"
                disabled={deleting}
                onclick={onDelete}
                aria-label={`Delete ${user.username}`}
                title="Delete user"
            >
                <Trash2 size={16} aria-hidden="true" />
            </button>
        {/if}
    </div>
{/snippet}

<AdminCard cardClass="admin-record-card admin-user-card" {body} {footer} />
