<script lang="ts">
    import type { Snippet } from "svelte";
    import { goto } from "$app/navigation";
    import { page } from "$app/state";
    import TopBar from "$components/TopBar.svelte";
    import { canAccessAdmin } from "$lib/admin-permissions";
    import { appShell } from "$lib/app-shell.svelte";
    import {
        AdminState,
        getAdminSectionFromPath,
        getAdminSectionPath,
        getPreferredAdminSection,
        getVisibleAdminSectionItems,
        setAdminStateContext,
        type AdminSection,
    } from "$lib/admin-state.svelte";

    let { children }: { children: Snippet } = $props();

    const adminState = setAdminStateContext(new AdminState());

    const currentSection = $derived.by(() =>
        getAdminSectionFromPath(page.url.pathname),
    );

    const visibleAdminSectionItems = $derived(
        getVisibleAdminSectionItems(appShell.currentUser),
    );

    const sectionCounts = $derived({
        users: adminState.adminUsers.length,
        ensembles: adminState.ensembles.length,
        scores: adminState.musics.length,
    });

    const currentSectionLabel = $derived(
        visibleAdminSectionItems.find((section) => section.id === currentSection)
            ?.label ??
            visibleAdminSectionItems[0]?.label ??
            "Admin",
    );

    const mobileMenuItems = $derived(
        visibleAdminSectionItems.map((section) => ({
            id: section.id,
            label: section.label,
            eyebrow: section.eyebrow,
            meta:
                section.id === "users"
                    ? `${sectionCounts.users} total`
                    : section.id === "ensembles"
                      ? `${sectionCounts.ensembles} groups`
                      : `${sectionCounts.scores} scores`,
        })),
    );

    const isInitialLoading = $derived(
        (appShell.userLoading && !appShell.currentUser) ||
            (adminState.adminLoading &&
                sectionCounts.users === 0 &&
                sectionCounts.ensembles === 0 &&
                sectionCounts.scores === 0),
    );

    const adminPageTitle = $derived.by(() => {
        if (!appShell.currentUser || !canAccessAdmin(appShell.currentUser)) {
            return "Admin | Fumen";
        }

        return `${currentSectionLabel} | Admin | Fumen`;
    });

    $effect(() => {
        const currentUser = appShell.currentUser;

        if (!currentUser || !canAccessAdmin(currentUser)) {
            adminState.clear();
            return;
        }

        void adminState.ensureLoadedFor(currentUser.id);
    });

    $effect(() => {
        const currentUser = appShell.currentUser;

        if (!currentUser || !canAccessAdmin(currentUser)) {
            return;
        }

        if (visibleAdminSectionItems.some((section) => section.id === currentSection)) {
            return;
        }

        void goto(getAdminSectionPath(getPreferredAdminSection(currentUser)), {
            replaceState: true,
            noScroll: true,
            keepFocus: true,
        });
    });

    $effect(() => {
        const shouldPollForProcessing =
            currentSection === "scores" &&
            adminState.musics.some((music) =>
                [
                    music.audio_status,
                    music.midi_status,
                    music.musicxml_status,
                    music.stems_status,
                ].includes("processing"),
            );

        if (!shouldPollForProcessing) {
            return;
        }

        const timer = window.setInterval(() => {
            if (currentSection === "scores") {
                void adminState.refresh();
            }
        }, 5000);

        return () => window.clearInterval(timer);
    });

    function navigateToSection(section: AdminSection) {
        void goto(getAdminSectionPath(section), {
            noScroll: true,
            keepFocus: true,
        });
    }
</script>

<svelte:head>
    <title>{adminPageTitle}</title>
</svelte:head>

{#if isInitialLoading}
    <div class="home-loading-overlay">
        <div class="loading-eq" aria-label="Loading">
            <span></span>
            <span></span>
            <span></span>
            <span></span>
            <span></span>
        </div>
        <p class="loading-eq-label">
            {appShell.preloadedUsername ? `Hello, ${appShell.preloadedUsername}` : "Fumen"}
        </p>
    </div>
{:else if !appShell.currentUser}
    <section class="min-h-dvh grid place-items-center p-6">
        <div class="music-card w-[min(680px,100%)] grid gap-6">
            <div>
                <p class="eyebrow">Fumen • Admin</p>
                <h1>Control room</h1>
                <p class="lede">
                    Sign in as the seeded superadmin or another admin-enabled
                    user to open the full-screen control room.
                </p>
            </div>
            <div class="flex items-center gap-4 flex-wrap">
                <a class="button ghost" href="/">User homepage</a>
            </div>
            <p class="hint">
                Use the backend CLI to generate a temporary connection link for
                the superadmin account, then open it here.
            </p>
        </div>
    </section>
{:else if !canAccessAdmin(appShell.currentUser)}
    <section class="min-h-dvh grid place-items-center p-6">
        <div class="music-card w-[min(680px,100%)] grid gap-6">
            <div>
                <p class="eyebrow">Fumen • Admin</p>
                <h1>Control room</h1>
                <p class="lede">
                    {appShell.currentUser.username} is signed in, but this
                    account does not have admin access.
                </p>
            </div>
            <div class="flex items-center gap-4 flex-wrap">
                <a class="button ghost" href="/">User homepage</a>
            </div>
        </div>
    </section>
{:else}
    <section class="h-[var(--app-height,100dvh)] min-h-[var(--app-height,100dvh)] grid grid-rows-[auto_minmax(0,1fr)] overflow-hidden">
        <TopBar
            breadcrumbs={[
                { label: "Fumen", href: "/" },
                { label: currentSectionLabel },
            ]}
            currentUser={appShell.currentUser}
            userHomeHref="/"
            onShowQr={() => void appShell.handleShowMyQr()}
            onLogout={() => void appShell.logoutUser()}
            onMyAccount={() => appShell.handleMyAccount()}
            onAppConfig={() => appShell.handleAppConfig()}
            mobileMenuItems={mobileMenuItems}
            mobileMenuActiveId={currentSection}
            mobileMenuAriaLabel="Admin sections"
            onMobileMenuSelect={(sectionId) =>
                navigateToSection(sectionId as AdminSection)}
        />

        <div
            class="h-full min-h-0 grid grid-cols-[280px_minmax(0,1fr)] overflow-hidden max-[1199px]:grid-cols-1"
        >
            <aside class="admin-sidebar">
                <nav
                    class="grid gap-0 border-t border-(--border-strong) !mx-0"
                    aria-label="Admin sections"
                >
                    {#each visibleAdminSectionItems as section}
                        <button
                            class="admin-nav-button"
                            class:is-active={currentSection === section.id}
                            onclick={() => navigateToSection(section.id)}
                        >
                            <span class="admin-nav-eyebrow">{section.eyebrow}</span>
                            <strong>{section.label}</strong>
                            {#if section.id === "users"}
                                <small>{sectionCounts.users} total</small>
                            {:else if section.id === "ensembles"}
                                <small>{sectionCounts.ensembles} groups</small>
                            {:else}
                                <small>{sectionCounts.scores} scores</small>
                            {/if}
                        </button>
                    {/each}
                </nav>
            </aside>

            <div class="min-h-0 overflow-y-auto bg-(--bg) pb-[env(safe-area-inset-bottom,0px)]">
                {@render children()}
            </div>
        </div>
    </section>
{/if}

{#if adminState.adminError || adminState.adminSuccess}
    <div class="toast-stack" aria-live="polite" aria-atomic="true">
        {#if adminState.adminError}
            <p class="status error toast">{adminState.adminError}</p>
        {/if}
        {#if adminState.adminSuccess}
            <p class="status success toast">{adminState.adminSuccess}</p>
        {/if}
    </div>
{/if}
