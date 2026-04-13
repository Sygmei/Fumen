<script lang="ts">
    import { Pencil, User } from "@lucide/svelte";
    import type { UserResponse as AppUser } from "../../adapters/fumen-backend/src/models";
    import { canDeleteUserAccount, isSuperadmin } from "../../lib/admin-permissions";
    import { compressImageToJpeg } from "../../lib/image";
    import type { GlobalRole } from "../../lib/roles";
    import BaseModal from "./BaseModal.svelte";
    import CustomSelect from "../CustomSelect.svelte";
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
        onSave: (draft: UserEditDraft) => Promise<void>;
        modalId?: string;
    } = $props();

    const MAX_AVATAR_BYTES = 1 * 1024 * 1024;
    const initialRole = (
        user.role === "superadmin" ? "admin" : user.role
    ) as Exclude<GlobalRole, "superadmin">;

    let displayName = $state(user.display_name ?? "");
    let role = $state(initialRole);
    let avatarFile = $state<File | null>(null);
    let avatarPreview = $state<string | null>(user.avatar_url ?? null);
    let clearAvatar = $state(false);
    let saving = $state(false);
    let errorMsg = $state("");

    const canEditRole = $derived(
        canDeleteUserAccount(user, currentUser) || isSuperadmin(currentUser),
    );

    async function handleAvatarChange(event: Event) {
        const input = event.currentTarget as HTMLInputElement;
        const file = input.files?.[0];
        if (!file) return;
        if (file.size > MAX_AVATAR_BYTES) {
            errorMsg = "Image must be under 1 MB.";
            input.value = "";
            return;
        }

        errorMsg = "";
        const compressed = await compressImageToJpeg(file, 256);
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

    async function handleSave() {
        saving = true;
        errorMsg = "";
        try {
            await onSave({
                displayName: displayName.trim(),
                role,
                avatarFile,
                avatarPreview,
                clearAvatar,
            });
            closeModal();
        } catch (error) {
            errorMsg =
                error instanceof Error ? error.message : "Failed to save.";
        } finally {
            saving = false;
        }
    }
</script>

{#snippet children()}
    <form
        class="edit-user-form"
        onsubmit={(event) => {
            event.preventDefault();
            void handleSave();
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
            disabled={saving}
        >
            Cancel
        </button>
        <button
            class="button"
            type="button"
            onclick={() => void handleSave()}
            disabled={saving}
        >
            {saving ? "Saving..." : "Save"}
        </button>
    </div>
{/snippet}

<BaseModal
    title="Edit user"
    subtitle={user.display_name ?? user.username}
    size="medium"
    {children}
    {footer}
    canClose={!saving}
    {modalId}
/>
