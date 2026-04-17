<script lang="ts">
    import { Pencil, User } from "@lucide/svelte";
    import type { UserResponse as AppUser } from "$backend/models";
    import { canDeleteUserAccount, isSuperadmin } from "$lib/admin-permissions";
    import {
        AVATAR_IMAGE_SIZE,
        AVATAR_UPLOAD_MAX_BYTES,
        compressImageToJpeg,
    } from "$lib/image";
    import type { GlobalRole } from "$lib/roles";
    import BaseModal from "./BaseModal.svelte";
    import CustomSelect from "$components/CustomSelect.svelte";
    import { closeModal } from "./modalState";
    import type { GlobalRoleOption, UserEditDraft } from "./types";

    let {
        user,
        currentUser,
        roleOptions,
        onSave,
        modalId,
    }: {
        user: AppUser;
        currentUser: AppUser;
        roleOptions: GlobalRoleOption[];
        onSave: (draft: UserEditDraft) => void | Promise<void>;
        modalId?: string;
    } = $props();

    const initialRole = (
        user.role === "superadmin" ? "admin" : user.role
    ) as Exclude<GlobalRole, "superadmin">;

    let displayName = $state(user.display_name ?? "");
    let role = $state(initialRole);
    let avatarFile = $state<File | null>(null);
    let avatarPreview = $state<string | null>(user.avatar_url ?? null);
    let clearAvatar = $state(false);
    let errorMsg = $state("");

    const canEditRole = $derived(
        canDeleteUserAccount(user, currentUser) || isSuperadmin(currentUser),
    );

    async function handleAvatarChange(event: Event) {
        const input = event.currentTarget as HTMLInputElement;
        const file = input.files?.[0];
        if (!file) return;
        if (file.size > AVATAR_UPLOAD_MAX_BYTES) {
            errorMsg = "Image must be under 10 MB.";
            input.value = "";
            return;
        }

        errorMsg = "";
        const compressed = await compressImageToJpeg(file, AVATAR_IMAGE_SIZE);
        avatarFile = compressed;
        clearAvatar = false;
        const reader = new FileReader();
        reader.onload = () => {
            avatarPreview = reader.result as string;
        };
        reader.readAsDataURL(compressed);
    }

    function handleRemoveAvatar() {
        avatarFile = null;
        clearAvatar = true;
        avatarPreview = null;
    }

    function handleSave() {
        errorMsg = "";
        void onSave({
            displayName: displayName.trim(),
            role,
            avatarFile,
            avatarPreview,
            clearAvatar,
        });
        closeModal();
    }
</script>

{#snippet children()}
    <form
        class="edit-user-form"
        onsubmit={(event) => {
            event.preventDefault();
            handleSave();
        }}
    >
        <div class="edit-user-avatar-row">
            <div class="edit-user-avatar-preview admin-user-avatar">
                {#if avatarPreview}
                    <img
                        src={avatarPreview}
                        alt=""
                        class="admin-user-avatar-img"
                    />
                {:else}
                    <User size={18} aria-hidden="true" />
                {/if}
            </div>
            <div class="edit-user-avatar-actions">
                <label class="button secondary small edit-user-avatar-btn">
                    <Pencil size={13} aria-hidden="true" />
                    {avatarPreview ? "Change photo" : "Upload photo"}
                    <input
                        type="file"
                        accept="image/jpeg,image/png,image/webp,image/gif"
                        onchange={handleAvatarChange}
                        style="display:none"
                    />
                </label>
                {#if avatarPreview}
                    <button
                        type="button"
                        class="button ghost small"
                        onclick={handleRemoveAvatar}
                    >
                        Remove
                    </button>
                {/if}
            </div>
        </div>
        <div class="edit-user-field">
            <label class="edit-user-label" for="edit-display-name">
                Display name
            </label>
            <input
                id="edit-display-name"
                class="admin-input"
                type="text"
                placeholder={user.username}
                bind:value={displayName}
                maxlength={80}
            />
        </div>
        {#if canEditRole}
            <div class="edit-user-field">
                <CustomSelect
                    label="Role"
                    bind:value={role}
                    options={roleOptions}
                    onValueChange={(nextRole) => {
                        role = nextRole as Exclude<GlobalRole, "superadmin">;
                    }}
                />
            </div>
        {/if}
        {#if errorMsg}
            <p class="admin-error">{errorMsg}</p>
        {/if}
    </form>
{/snippet}

{#snippet footer()}
    <div class="actions admin-user-modal-actions">
        <button
            class="button ghost"
            type="button"
            onclick={closeModal}
        >
            Cancel
        </button>
        <button
            class="button"
            type="button"
            onclick={handleSave}
        >
            Save
        </button>
    </div>
{/snippet}

<BaseModal
    title="Edit user"
    subtitle={user.display_name ?? user.username}
    size="medium"
    {children}
    {footer}
    {modalId}
/>
