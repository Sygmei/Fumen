<script lang="ts">
    import type { Snippet } from "svelte";

    const {
        onClose,
        children,
    }: {
        onClose: () => void;
        children: Snippet;
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
    <div class="modal-card" role="dialog" aria-modal="true" tabindex="-1">
        {@render children()}
    </div>
</div>
