<script lang="ts">
    import { page } from "$app/state";
    import { appShell } from "$lib/app-shell.svelte";

    const token = $derived(page.params.token);
    let attemptedToken = $state("");

    $effect(() => {
        if (token && attemptedToken !== token) {
            attemptedToken = token;
            void appShell.completeConnectionFromToken(token, true);
        }
    });
</script>

<main class="page home-shell">
    <section class="content-panel home-grid">
        <div class="music-card connect-card">
            <p class="meta-label">Connection</p>
            <h2>Finishing sign-in</h2>
            <p class="lede">
                We are validating your temporary connection link and sending
                you to your library.
            </p>
            <p class="status">Connecting...</p>
        </div>
    </section>
</main>
