<script lang="ts">
    import type { Snippet } from "svelte";
    import type { AppUser } from "../lib/api";
    import TopBar from "./TopBar.svelte";
    import { canAccessAdmin } from "../lib/admin-permissions";

    type AdminSection = "users" | "ensembles" | "scores";

    let {
        currentUser,
        userLoading,
        preloadedUsername,
        adminLoading,
        adminError,
        adminSuccess,
        adminSection,
        visibleSectionItems,
        sectionCounts,
        onShowQr,
        onLogout,
        onMyAccount,
        onAppConfig,
        onSectionChange,
        children,
    }: {
        currentUser: AppUser | null;
        userLoading: boolean;
        preloadedUsername: string;
        adminLoading: boolean;
        adminError: string;
        adminSuccess: string;
        adminSection: AdminSection;
        visibleSectionItems: Array<{ id: AdminSection; label: string; eyebrow: string }>;
        sectionCounts: { users: number; ensembles: number; scores: number };
        onShowQr?: () => void;
        onLogout: () => void;
        onMyAccount?: () => void;
        onAppConfig?: () => void;
        onSectionChange: (section: AdminSection) => void;
        children: Snippet;
    } = $props();

    const currentSectionLabel = $derived(
        visibleSectionItems.find((s) => s.id === adminSection)?.label ??
            visibleSectionItems[0]?.label ??
            "Admin"
    );

    const mobileMenuItems = $derived(
        visibleSectionItems.map((section) => ({
            id: section.id,
            label: section.label,
            eyebrow: section.eyebrow,
            meta:
                section.id === "users"
                    ? `${sectionCounts.users} total`
                    : section.id === "ensembles"
                      ? `${sectionCounts.ensembles} groups`
                      : `${sectionCounts.scores} scores`,
        }))
    );

    const isInitialLoading = $derived(
        (userLoading && !currentUser) ||
            (adminLoading &&
                sectionCounts.users === 0 &&
                sectionCounts.ensembles === 0 &&
                sectionCounts.scores === 0)
    );
</script>

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
            {preloadedUsername ? `Hello, ${preloadedUsername}` : "Fumen"}
        </p>
    </div>
{:else if !currentUser}
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
{:else if !canAccessAdmin(currentUser)}
    <section class="min-h-dvh grid place-items-center p-6">
        <div class="music-card w-[min(680px,100%)] grid gap-6">
            <div>
                <p class="eyebrow">Fumen • Admin</p>
                <h1>Control room</h1>
                <p class="lede">
                    {currentUser.username} is signed in, but this account does not
                    have admin access.
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
            {currentUser}
            userHomeHref="/"
            {onShowQr}
            {onLogout}
            {onMyAccount}
            {onAppConfig}
            mobileMenuItems={mobileMenuItems}
            mobileMenuActiveId={adminSection}
            mobileMenuAriaLabel="Admin sections"
            onMobileMenuSelect={(sectionId) =>
                onSectionChange(sectionId as AdminSection)}
        />

        <div
            class="h-full min-h-0 grid grid-cols-[280px_minmax(0,1fr)] overflow-hidden max-[1199px]:grid-cols-1"
        >
            <aside class="admin-sidebar">
                <nav
                    class="grid gap-0 border-t border-(--border-strong) !mx-0"
                    aria-label="Admin sections"
                >
                    {#each visibleSectionItems as section}
                        <button
                            class="admin-nav-button"
                            class:is-active={adminSection === section.id}
                            onclick={() => onSectionChange(section.id)}
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

{#if adminError || adminSuccess}
    <div class="toast-stack" aria-live="polite" aria-atomic="true">
        {#if adminError}
            <p class="status error toast">{adminError}</p>
        {/if}
        {#if adminSuccess}
            <p class="status success toast">{adminSuccess}</p>
        {/if}
    </div>
{/if}
