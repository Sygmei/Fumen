<script lang="ts">
    import type { Snippet } from "svelte";

    type ModalSize = "small" | "medium" | "large" | "full";

    const {
        onClose,
        children,
        header,
        footer,
        size = "small",
        cardClass = "",
        labelledBy,
    }: {
        onClose: () => void;
        children: Snippet;
        header?: Snippet;
        footer?: Snippet;
        size?: ModalSize;
        cardClass?: string;
        labelledBy?: string;
    } = $props();

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
        class={`modal-card modal-card--${size} ${header ? "modal-card--with-header" : ""} ${footer ? "modal-card--with-footer" : ""} ${cardClass}`.trim()}
        role="dialog"
        aria-modal="true"
        aria-labelledby={labelledBy}
        tabindex="-1"
    >
        {#if header}
            <div class="modal-header">
                {@render header()}
            </div>
        {/if}
        <div class="modal-main">
            {@render children()}
        </div>
        {#if footer}
            <div class="modal-footer">
                {@render footer()}
            </div>
        {/if}
    </div>
</div>
