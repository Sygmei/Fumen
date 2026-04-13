<script lang="ts">
    import { goto } from "$app/navigation";
    import { appShell } from "$lib/app-shell.svelte";
    import {
        getAdminSectionPath,
        getPreferredAdminSection,
    } from "$lib/admin-state.svelte";

    $effect(() => {
        if (appShell.userLoading && !appShell.currentUser) {
            return;
        }

        void goto(getAdminSectionPath(getPreferredAdminSection(appShell.currentUser)), {
            replaceState: true,
            noScroll: true,
            keepFocus: true,
        });
    });
</script>
