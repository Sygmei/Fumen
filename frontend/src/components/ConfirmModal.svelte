<script lang="ts">
    import BaseModal from "./BaseModal.svelte";

    const {
        title = "Are you sure?",
        confirmLabel = "Confirm",
        danger = true,
        busy = false,
        onConfirm,
        onClose,
    }: {
        title?: string;
        confirmLabel?: string;
        danger?: boolean;
        busy?: boolean;
        onConfirm: () => void | Promise<void>;
        onClose: () => void;
    } = $props();
</script>

{#snippet footer()}
    <div class="actions admin-user-modal-actions">
        <button
            class="button ghost"
            type="button"
            disabled={busy}
            onclick={onClose}
        >
            Cancel
        </button>
        <button
            class={`button${danger ? " danger" : ""}`}
            type="button"
            disabled={busy}
            onclick={() => void onConfirm()}
        >
            {busy ? "Please wait..." : confirmLabel}
        </button>
    </div>
{/snippet}

<BaseModal {onClose} size="small" title="Confirm" subtitle={title} {footer} />
