<script lang="ts">
    import { onMount } from "svelte";
    import BaseModal from "./BaseModal.svelte";
    import { closeModal } from "./modalState";

    let {
        positionLabel,
        initialComment = "",
        onSave,
        modalId,
    }: {
        positionLabel: string;
        initialComment?: string;
        onSave: (comment: string) => void | Promise<void>;
        modalId?: string;
    } = $props();

    let comment = $state("");
    let saving = $state(false);
    let error = $state("");

    onMount(() => {
        comment = initialComment;
    });

    async function handleSave() {
        const nextComment = comment.trim();
        if (!nextComment) {
            error = "Write an annotation before saving.";
            return;
        }

        saving = true;
        error = "";

        try {
            await onSave(nextComment);
            closeModal();
        } catch (saveError) {
            error = saveError instanceof Error ? saveError.message : "Unable to save annotation.";
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
            onclick={() => closeModal()}
        >
            Cancel
        </button>
        <button
            class="button"
            type="button"
            disabled={saving || !comment.trim()}
            onclick={() => void handleSave()}
        >
            {saving ? "Adding..." : "Add annotation"}
        </button>
    </div>
{/snippet}

<BaseModal
    size="medium"
    title="Add annotation"
    subtitle={positionLabel}
    {footer}
    canClose={!saving}
    {modalId}
>
    <div class="annotation-modal-body">
        <p class="hint annotation-modal-hint">
            Pin a note to this point on the score.
        </p>

        <label class="field annotation-field">
            <span>Comment</span>
            <textarea
                bind:value={comment}
                rows="6"
                placeholder="Write your annotation..."
                disabled={saving}
            ></textarea>
        </label>

        {#if error}
            <p class="status error">{error}</p>
        {/if}
    </div>
</BaseModal>