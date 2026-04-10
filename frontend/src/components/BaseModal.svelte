<script lang="ts">
    import type { Snippet } from "svelte";
    import { X } from "@lucide/svelte";

    type ModalSize = "small" | "medium" | "large" | "full";

    const {
        onClose,
        children,
        footer,
        title,
        subtitle,
        canClose = true,
        size = "small",
        cardClass = "",
    }: {
        onClose: () => void;
        children?: Snippet;
        footer?: Snippet;
        title?: string;
        subtitle?: string;
        canClose?: boolean;
        size?: ModalSize;
        cardClass?: string;
    } = $props();

    const hasHeader = $derived(!!(title || subtitle));
    const bodyless = $derived(!children);

    function handleBackdropClick(event: MouseEvent) {
        if (event.target === event.currentTarget) {
            onClose();
        }
    }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex a11y_click_events_have_key_events -->
<div
    class="modal-backdrop"
    role="presentation"
    tabindex="0"
    onclick={handleBackdropClick}
    onkeydown={(event) => event.key === "Escape" && onClose()}
>
    <div
        class={`modal-card modal-card--${size} ${hasHeader ? "modal-card--with-header" : ""} ${footer ? "modal-card--with-footer" : ""} ${bodyless ? "modal-card--bodyless" : ""} ${cardClass}`.trim()}
        role="dialog"
        aria-modal="true"
        aria-labelledby={hasHeader ? "modal-title" : undefined}
        tabindex="-1"
    >
        {#if hasHeader}
            <div class="modal-header">
                <div class="card-header items-start">
                    <div>
                        {#if title}<p class="meta-label">{title}</p>{/if}
                        {#if subtitle}<h2 id="modal-title">{subtitle}</h2>{/if}
                    </div>
                    {#if canClose}
                        <button
                            class="button ghost admin-modal-close"
                            type="button"
                            aria-label="Close modal"
                            onclick={onClose}
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
        {/if}
    </div>
</div>

