<script lang="ts">
    import type { Snippet } from "svelte";
    import { onMount } from "svelte";
    import "../app.css";
    import ModalStore from "../components/modals/ModalStore.svelte";
    import { appShell } from "../lib/app-shell.svelte";

    let { children }: { children: Snippet } = $props();

    onMount(() => appShell.mount());
</script>

{@render children()}

{#if appShell.userError || appShell.userSuccess}
    <div class="toast-stack" aria-live="polite" aria-atomic="true">
        {#if appShell.userError}
            <p class="status error toast">{appShell.userError}</p>
        {/if}
        {#if appShell.userSuccess}
            <p class="status success toast">{appShell.userSuccess}</p>
        {/if}
    </div>
{/if}

<ModalStore />
