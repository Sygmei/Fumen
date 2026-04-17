<script lang="ts">
    import BaseModal from "./BaseModal.svelte";
    import { closeModal } from "./modalState";

    let {
        title = "Confirm Action",
        message = "",
        confirmText = "Confirm",
        cancelText = "Cancel",
        variant = "default",
        busy = false,
        onConfirm = () => {},
        onCancel = () => {},
        modalId,
    }: {
        title?: string;
        message?: string;
        confirmText?: string;
        cancelText?: string;
        variant?: "default" | "danger" | "warning" | "success";
        busy?: boolean;
        onConfirm?: () => void | Promise<void>;
        onCancel?: () => void | Promise<void>;
        modalId?: string;
    } = $props();

    const confirmButtonClass = $derived(
        `button${variant === "danger" || variant === "warning" ? " danger" : ""}`,
    );

    async function handleCancel() {
        try {
            await onCancel();
        } finally {
            closeModal();
        }
    }

    async function handleConfirm() {
        try {
            await onConfirm();
        } finally {
            closeModal();
        }
    }
</script>

{#snippet children()}
    {#if message}
        <p class="hint">{message}</p>
    {/if}
{/snippet}

{#snippet footer()}
    <div class="actions admin-user-modal-actions">
        <button
            class="button ghost"
            type="button"
            disabled={busy}
            onclick={() => void handleCancel()}
        >
            {cancelText}
        </button>
        <button
            class={confirmButtonClass}
            type="button"
            disabled={busy}
            onclick={() => void handleConfirm()}
        >
            {busy ? "Please wait..." : confirmText}
        </button>
    </div>
{/snippet}

<BaseModal
    title="Confirm"
    subtitle={title}
    size="small"
    {children}
    {footer}
    {modalId}
/>
