<script lang="ts">
    import type { Snippet } from "svelte";
    import { X } from "@lucide/svelte";
    import { activeModal, closeModal } from "./modalState";

    type ModalSize = "small" | "medium" | "large" | "full";

    let {
        children,
        header,
        footer,
        title,
        subtitle,
        showCloseButton = true,
        showDefaultHeader = true,
        size = "small",
        showDefaultFooter = false,
        confirmText = "Confirm",
        cancelText = "Cancel",
        confirmButtonClass = "button",
        cancelButtonClass = "button ghost",
        onConfirm = () => {},
        onCancel = () => {},
        onClose = () => {},
        canClose = true,
        cardClass = "",
        modalId,
    }: {
        children?: Snippet;
        header?: Snippet;
        footer?: Snippet;
        title?: string;
        subtitle?: string;
        showCloseButton?: boolean;
        showDefaultHeader?: boolean;
        size?: ModalSize;
        showDefaultFooter?: boolean;
        confirmText?: string;
        cancelText?: string;
        confirmButtonClass?: string;
        cancelButtonClass?: string;
        onConfirm?: () => void | Promise<void>;
        onCancel?: () => void | Promise<void>;
        onClose?: () => void;
        canClose?: boolean;
        cardClass?: string;
        modalId?: string;
    } = $props();

    const hasDefaultHeader = $derived(
        showDefaultHeader && !!(title || subtitle || showCloseButton),
    );
    const bodyless = $derived(!children);
    const isTopModal = $derived(!modalId || $activeModal?.id === modalId);

    function cleanup() {
        if (!isTopModal) return;
        onClose();
        closeModal();
    }

    function handleBackdropClick(event: MouseEvent) {
        if (!canClose || !isTopModal) return;
        if (event.target === event.currentTarget) {
            cleanup();
        }
    }

    function handleKeydown(event: KeyboardEvent) {
        if (isTopModal && canClose && event.key === "Escape") {
            cleanup();
        }
    }

    async function handleConfirm() {
        await onConfirm();
        closeModal();
    }

    async function handleCancel() {
        await onCancel();
        closeModal();
    }
</script>

<style>
    .modal-header-row {
        position: relative;
        padding-right: 56px;
    }

    .modal-close-button {
        position: absolute;
        top: 50%;
        right: 0;
        width: 40px;
        min-width: 40px;
        height: 40px;
        min-height: 40px;
        padding: 0;
        justify-content: center;
        border-radius: var(--radius-md) !important;
        transform: translateY(-50%);
    }

    .modal-close-button:hover,
    .modal-close-button:active {
        transform: translateY(-50%);
    }
</style>

<svelte:window onkeydown={handleKeydown} />

<!-- svelte-ignore a11y_no_noninteractive_tabindex a11y_click_events_have_key_events -->
<div
    class="modal-backdrop"
    role="presentation"
    tabindex="0"
    onclick={handleBackdropClick}
>
    <div
        class={`modal-card modal-card--${size} ${hasDefaultHeader || header ? "modal-card--with-header" : ""} ${footer || showDefaultFooter ? "modal-card--with-footer" : ""} ${bodyless ? "modal-card--bodyless" : ""} ${cardClass}`.trim()}
        role="dialog"
        aria-modal="true"
        aria-labelledby={hasDefaultHeader ? "modal-title" : undefined}
        tabindex="-1"
    >
        {#if header}
            <div class="modal-header">
                {@render header()}
            </div>
        {:else if hasDefaultHeader}
            <div class="modal-header">
                <div class="card-header items-center modal-header-row">
                    <div>
                        {#if title}<p class="meta-label">{title}</p>{/if}
                        {#if subtitle}<h2 id="modal-title">{subtitle}</h2>{/if}
                    </div>
                    {#if canClose && showCloseButton}
                        <button
                            class="button ghost modal-close-button"
                            type="button"
                            aria-label="Close modal"
                            onclick={cleanup}
                        >
                            <X size={16} aria-hidden="true" />
                        </button>
                    {/if}
                </div>
            </div>
        {/if}

        {#if children}
            <div class="modal-main">
                {@render children()}
            </div>
        {/if}

        {#if footer}
            <div class="modal-footer">
                {@render footer()}
            </div>
        {:else if showDefaultFooter}
            <div class="modal-footer">
                <div class="actions admin-user-modal-actions">
                    <button
                        class={cancelButtonClass}
                        type="button"
                        onclick={() => void handleCancel()}
                    >
                        {cancelText}
                    </button>
                    <button
                        class={confirmButtonClass}
                        type="button"
                        onclick={() => void handleConfirm()}
                    >
                        {confirmText}
                    </button>
                </div>
            </div>
        {/if}
    </div>
</div>
