<script lang="ts">
    import type { AdminMusicResponse as AdminMusic } from "$backend/models";
    import BaseModal from "./BaseModal.svelte";
    import { closeModal } from "./modalState";
    import type { EditScoreDraft } from "./types";

    let {
        music,
        onSave,
        modalId,
    }: {
        music: AdminMusic;
        onSave: (draft: EditScoreDraft) => Promise<void>;
        modalId?: string;
    } = $props();

    let title = $state(music.title);
    let publicId = $state(music.public_id ?? "");
    let icon = $state(music.icon ?? "");
    let iconFile = $state<File | null>(null);
    let saving = $state(false);
    let errorMsg = $state("");

    async function handleSave() {
        if (!title.trim()) {
            errorMsg = "Title cannot be empty.";
            return;
        }

        saving = true;
        errorMsg = "";
        try {
            await onSave({
                title,
                publicId,
                icon,
                iconFile,
            });
            closeModal();
        } catch (error) {
            errorMsg =
                error instanceof Error
                    ? error.message
                    : "Unable to update score metadata.";
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
            onclick={() => void handleSave()}
        >
            {saving ? "Saving..." : "Save changes"}
        </button>
    </div>
{/snippet}

<BaseModal
    size="medium"
    cardClass="admin-score-modal"
    title="Edit score"
    subtitle={title}
    {footer}
    canClose={!saving}
    {modalId}
>
    <div class="upload-grid admin-score-modal-grid">
        <label class="field">
            <span>Title</span>
            <input bind:value={title} />
        </label>
        <label class="field file-field admin-score-file-field">
            <span>Icon image</span>
            <input
                type="file"
                accept="image/*"
                onchange={(event) => {
                    const target = event.currentTarget as HTMLInputElement;
                    iconFile = target.files?.[0] ?? null;
                }}
            />
        </label>
        <label class="field admin-score-modal-full">
            <span>Friendly public id</span>
            <input
                bind:value={publicId}
                placeholder="example: moonlight-sonata"
            />
        </label>
    </div>
    {#if errorMsg}
        <p class="admin-error">{errorMsg}</p>
    {/if}
</BaseModal>
