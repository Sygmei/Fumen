<script lang="ts">
    import { onMount } from "svelte";
    import BaseModal from "./BaseModal.svelte";
    import { closeModal } from "./modalState";

    let {
        positionLabel,
        instrumentName = null,
        initialComment = "",
        onSave,
        modalId,
    }: {
        positionLabel: string;
        instrumentName?: string | null;
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
    subtitle="Review target"
    {footer}
    canClose={!saving}
    {modalId}
>
    <div class="annotation-modal-body">
        <section class="annotation-preview" aria-label="Annotation target preview">
            <p class="annotation-preview-eyebrow">Target preview</p>
            <dl class="annotation-preview-grid">
                <div class="annotation-preview-item">
                    <dt>Bar / beat</dt>
                    <dd>{positionLabel}</dd>
                </div>
                <div class="annotation-preview-item">
                    <dt>Instrument</dt>
                    <dd>{instrumentName ?? "Instrument not detected"}</dd>
                </div>
            </dl>
        </section>

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

<style>
    .annotation-modal-body {
        display: grid;
        gap: 16px;
    }

    .annotation-preview {
        display: grid;
        gap: 12px;
        padding: 14px 16px;
        border: 1px solid var(--border);
        border-radius: 16px;
        background:
            linear-gradient(135deg, rgba(224, 61, 30, 0.08), rgba(224, 61, 30, 0.03)),
            var(--surface-alt);
    }

    .annotation-preview-eyebrow {
        margin: 0;
        font-size: 0.72rem;
        font-weight: 700;
        letter-spacing: 0.12em;
        text-transform: uppercase;
        color: var(--text-dim);
    }

    .annotation-preview-grid {
        display: grid;
        grid-template-columns: repeat(2, minmax(0, 1fr));
        gap: 12px;
        margin: 0;
    }

    .annotation-preview-item {
        display: grid;
        gap: 3px;
        min-width: 0;
    }

    .annotation-preview-item dt {
        font-size: 0.72rem;
        font-weight: 700;
        letter-spacing: 0.08em;
        text-transform: uppercase;
        color: var(--text-dim);
    }

    .annotation-preview-item dd {
        margin: 0;
        font-size: 0.98rem;
        font-weight: 700;
        color: var(--text);
    }

    .annotation-modal-hint {
        margin: 0;
    }

    @media (max-width: 640px) {
        .annotation-preview-grid {
            grid-template-columns: 1fr;
        }
    }
</style>