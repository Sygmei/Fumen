<script lang="ts">
    import BaseModal from "./BaseModal.svelte";
    import type { AppUser } from "../lib/api";
    import { updateMyProfile, compressImageToJpeg } from "../lib/api";
    import { Camera } from "@lucide/svelte";

    const {
        currentUser,
        onClose,
        onSaved,
    }: {
        currentUser: AppUser;
        onClose: () => void;
        onSaved: (user: AppUser) => void;
    } = $props();

    let displayName = $state(currentUser.display_name ?? "");
    let avatarPreview = $state<string | null>(currentUser.avatar_url);
    let avatarFile = $state<File | null>(null);
    let clearAvatar = $state(false);
    let saving = $state(false);
    let errorMsg = $state("");

    const MAX_FILE_SIZE = 1 * 1024 * 1024; // 1 MB

    async function handleAvatarChange(event: Event) {
        const input = event.currentTarget as HTMLInputElement;
        const file = input.files?.[0];
        if (!file) return;

        if (file.size > MAX_FILE_SIZE) {
            errorMsg = "Image must be under 1 MB.";
            input.value = "";
            return;
        }

        errorMsg = "";
        const compressed = await compressImageToJpeg(file, 256);
        avatarFile = compressed;
        clearAvatar = false;
        const reader = new FileReader();
        reader.onload = () => { avatarPreview = reader.result as string; };
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
            const resp = await updateMyProfile({
                displayName: displayName.trim() || null,
                avatarFile: avatarFile,
                clearAvatar,
            });
            onSaved(resp.user);
        } catch (err) {
            errorMsg = err instanceof Error ? err.message : "Failed to save.";
        } finally {
            saving = false;
        }
    }
</script>

<BaseModal title="Profile" subtitle="My account" size="small" {onClose}>
    {#snippet children()}
        <form class="account-form" onsubmit={(e) => { e.preventDefault(); handleSave(); }}>
            <div class="account-avatar-section">
                <div class="account-avatar-preview">
                    {#if avatarPreview}
                        <img src={avatarPreview} alt="" class="account-avatar-img" />
                    {:else}
                        <span class="account-avatar-initials">
                            {(currentUser.display_name ?? currentUser.username).charAt(0).toUpperCase()}
                        </span>
                    {/if}
                </div>
                <div class="account-avatar-actions">
                    <label class="btn btn--secondary btn--small account-avatar-upload-btn">
                        <Camera size={14} aria-hidden="true" />
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
                            class="btn btn--ghost btn--small"
                            onclick={handleRemoveAvatar}
                        >Remove photo</button>
                    {/if}
                </div>
            </div>

            <div class="account-field">
                <label class="account-label" for="account-username">Username</label>
                <input
                    id="account-username"
                    class="account-input account-input--readonly"
                    type="text"
                    value={currentUser.username}
                    readonly
                    aria-label="Username (cannot be changed)"
                />
            </div>

            <div class="account-field">
                <label class="account-label" for="account-display-name">Display name</label>
                <input
                    id="account-display-name"
                    class="account-input"
                    type="text"
                    placeholder={currentUser.username}
                    bind:value={displayName}
                    maxlength={80}
                    autocomplete="name"
                />
                <p class="account-hint">Shown instead of your username when set. Letters, accents, and spaces are all fine.</p>
            </div>

            {#if errorMsg}
                <p class="account-error">{errorMsg}</p>
            {/if}
        </form>
    {/snippet}

    {#snippet footer()}
        <div class="modal-footer-actions">
            <button class="btn btn--ghost" type="button" onclick={onClose} disabled={saving}>
                Cancel
            </button>
            <button class="btn btn--primary" type="button" onclick={handleSave} disabled={saving}>
                {saving ? "Saving…" : "Save"}
            </button>
        </div>
    {/snippet}
</BaseModal>

<style>
    .account-form {
        display: flex;
        flex-direction: column;
        gap: 20px;
        padding: 24px;
    }

    .account-avatar-section {
        display: flex;
        align-items: center;
        gap: 16px;
    }

    .account-avatar-preview {
        width: 64px;
        height: 64px;
        border-radius: 50%;
        background: var(--color-accent, #6366f1);
        display: flex;
        align-items: center;
        justify-content: center;
        overflow: hidden;
        flex-shrink: 0;
    }

    .account-avatar-img {
        width: 100%;
        height: 100%;
        object-fit: cover;
        border-radius: 50%;
    }

    .account-avatar-initials {
        color: #fff;
        font-size: 24px;
        font-weight: 600;
        line-height: 1;
        user-select: none;
    }

    .account-avatar-actions {
        display: flex;
        flex-direction: column;
        gap: 6px;
    }

    .account-avatar-upload-btn {
        display: inline-flex;
        align-items: center;
        gap: 6px;
        cursor: pointer;
    }

    .account-field {
        display: flex;
        flex-direction: column;
        gap: 6px;
    }

    .account-label {
        font-size: 13px;
        font-weight: 500;
        color: var(--color-text-muted, #64748b);
    }

    .account-input {
        height: 36px;
        padding: 0 10px;
        border: 1px solid var(--color-border, #e2e8f0);
        border-radius: 6px;
        font-size: 14px;
        background: var(--color-bg, #fff);
        color: var(--color-text, #0f172a);
        outline: none;
        transition: border-color 0.15s;
    }

    .account-input:focus {
        border-color: var(--color-accent, #6366f1);
    }

    .account-input--readonly {
        background: var(--color-bg-subtle, #f8fafc);
        color: var(--color-text-muted, #64748b);
        cursor: default;
    }

    .account-hint {
        font-size: 12px;
        color: var(--color-text-muted, #64748b);
        margin: 0;
        line-height: 1.4;
    }

    .account-error {
        color: var(--color-error, #ef4444);
        font-size: 13px;
        margin: 0;
    }

    .modal-footer-actions {
        display: flex;
        justify-content: flex-end;
        gap: 8px;
        padding: 16px 24px;
    }
</style>
