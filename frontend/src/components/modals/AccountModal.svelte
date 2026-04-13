<script lang="ts">
    import BaseModal from "./BaseModal.svelte";
    import type {
        UpdateMyProfileMultipartRequest,
        UserResponse as AppUser,
    } from "../../adapters/fumen-backend/src/models";
    import { authenticatedApiClient } from "../../lib/auth-client";
    import { compressImageToJpeg } from "../../lib/image";
    import { closeModal } from "./modalState";
    import { Camera } from "@lucide/svelte";

    const {
        currentUser,
        onSaved,
        onClose = () => {},
        modalId,
    }: {
        currentUser: AppUser;
        onClose?: () => void;
        onSaved: (user: AppUser) => void;
        modalId?: string;
    } = $props();

    const initialDisplayName = currentUser.display_name ?? "";
    const initialAvatarPreview = currentUser.avatar_url ?? null;

    let displayName = $state(initialDisplayName);
    let avatarPreview = $state<string | null>(initialAvatarPreview);
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
            const payload = {
                display_name: displayName.trim() || undefined,
                avatar_file: avatarFile ?? undefined,
                clear_avatar: clearAvatar || undefined,
            } as unknown as UpdateMyProfileMultipartRequest;
            const resp = await authenticatedApiClient.updateMyProfile(payload);
            onSaved(resp.user);
            closeModal();
        } catch (err) {
            errorMsg = err instanceof Error ? err.message : "Failed to save.";
        } finally {
            saving = false;
        }
    }
</script>

<BaseModal
    title="Profile"
    subtitle="My account"
    size="small"
    {onClose}
    {modalId}
>
    {#snippet children()}
        <form class="flex flex-col gap-5 p-6" onsubmit={(e) => { e.preventDefault(); handleSave(); }}>
            <!-- Avatar row -->
            <div class="flex items-center gap-4">
                <div class="size-16 rounded-full bg-(--accent) flex items-center justify-center overflow-hidden shrink-0">
                    {#if avatarPreview}
                        <img src={avatarPreview} alt="" class="w-full h-full object-cover" />
                    {:else}
                        <span class="text-white text-2xl font-semibold leading-none select-none">
                            {(currentUser.display_name ?? currentUser.username).charAt(0).toUpperCase()}
                        </span>
                    {/if}
                </div>
                <div class="flex flex-col gap-1.5">
                    <label class="button ghost inline-flex items-center gap-1.5 cursor-pointer text-sm px-3 py-1.5">
                        <Camera size={14} aria-hidden="true" />
                        {avatarPreview ? "Change photo" : "Upload photo"}
                        <input
                            type="file"
                            accept="image/jpeg,image/png,image/webp,image/gif"
                            onchange={handleAvatarChange}
                            class="hidden"
                        />
                    </label>
                    {#if avatarPreview}
                        <button
                            type="button"
                            class="button ghost text-sm px-3 py-1.5"
                            onclick={handleRemoveAvatar}
                        >Remove photo</button>
                    {/if}
                </div>
            </div>

            <!-- Username (read-only) -->
            <label class="field">
                <span>Username</span>
                <input
                    id="account-username"
                    type="text"
                    value={currentUser.username}
                    readonly
                    aria-label="Username (cannot be changed)"
                    class="opacity-60 cursor-default"
                />
            </label>

            <!-- Display name -->
            <label class="field">
                <span>Display name</span>
                <input
                    id="account-display-name"
                    type="text"
                    placeholder={currentUser.username}
                    bind:value={displayName}
                    maxlength={80}
                    autocomplete="name"
                />
                <p class="text-xs text-(--text-dim) m-0 leading-[1.4]">
                    Shown instead of your username when set. Letters, accents, and spaces are all fine.
                </p>
            </label>

            {#if errorMsg}
                <p class="text-(--danger) text-[13px] m-0">{errorMsg}</p>
            {/if}
        </form>
    {/snippet}

    {#snippet footer()}
        <div class="actions admin-user-modal-actions">
            <button class="button ghost" type="button" onclick={onClose} disabled={saving}>Cancel</button>
            <button class="button" type="button" onclick={handleSave} disabled={saving}>
                {saving ? "Saving…" : "Save"}
            </button>
        </div>
    {/snippet}
</BaseModal>
