<script lang="ts">
    import type { AppUser } from "../lib/api";
    import { Menu, LayoutGrid } from "@lucide/svelte";
    import UserMenu from "./UserMenu.svelte";

    let {
        breadcrumbs,
        currentUser,
        onLogout,
        onShowQr,
        onMyAccount,
        onAppConfig,
        adminHref,
        userHomeHref,
        showBrandTitleOnMobile = false,
        mobileMenuItems = [],
        mobileMenuActiveId,
        mobileMenuAriaLabel = "Section menu",
        onMobileMenuSelect,
    }: {
        breadcrumbs: Array<{ label: string; href?: string }>;
        currentUser: AppUser | null;
        onLogout?: () => void;
        /** If set, "Log in on another device" appears in the user menu */
        onShowQr?: () => void;
        /** If set, "My account" appears in the user menu */
        onMyAccount?: () => void;
        /** If set, "App settings" appears in the user menu */
        onAppConfig?: () => void;
        /** If set, an admin-panel icon link appears */
        adminHref?: string;
        /** If set, "User homepage" appears in the user menu */
        userHomeHref?: string;
        /** If set, the logo/title brand stays visible on mobile */
        showBrandTitleOnMobile?: boolean;
        mobileMenuItems?: Array<{
            id: string;
            label: string;
            eyebrow?: string;
            meta?: string;
        }>;
        mobileMenuActiveId?: string;
        mobileMenuAriaLabel?: string;
        onMobileMenuSelect?: (id: string) => void;
    } = $props();

    let mobileMenuOpen = $state(false);

    function toggleMobileMenu() {
        mobileMenuOpen = !mobileMenuOpen;
    }

    function closeMobileMenu() {
        mobileMenuOpen = false;
    }

    function handleMobileMenuSelect(id: string) {
        closeMobileMenu();
        onMobileMenuSelect?.(id);
    }
</script>

<header
    class="flex items-stretch justify-between gap-6 min-h-[54px] px-7 border-b border-(--border-strong) bg-[var(--surface)]"
    class:topbar-brand-mobile-visible={showBrandTitleOnMobile}
>
    <div class="flex items-center gap-5 py-2.5">
        <div class="topbar-breadcrumbs flex items-center gap-2.5 font-bold text-[clamp(1.4rem,2vw,2rem)] leading-none" aria-label="Breadcrumb">
            {#each breadcrumbs as crumb, i}
                {#if i === 0}
                    {#if crumb.href}
                        <a
                            class="topbar-brand-link flex items-center gap-[13px] no-underline leading-none self-stretch pr-1"
                            href={crumb.href}
                        >
                            <span class="topbar-home-mark" aria-hidden="true"></span>
                            <span class="topbar-brand-title">
                                {crumb.label}
                            </span>
                        </a>
                    {:else}
                        <span class="topbar-brand-link flex items-center gap-[13px] leading-none self-stretch pr-1">
                            <span class="topbar-home-mark" aria-hidden="true"></span>
                            <span class="topbar-brand-title">
                                {crumb.label}
                            </span>
                        </span>
                    {/if}
                {:else if i === 1}
                    <span class="opacity-55">—</span>
                    {#if crumb.href}
                        <a href={crumb.href}>{crumb.label}</a>
                    {:else}
                        <span class="text-(--text-dim)">{crumb.label}</span>
                    {/if}
                {:else}
                    <span class="opacity-55">/</span>
                    {#if crumb.href}
                        <strong><a href={crumb.href}>{crumb.label}</a></strong>
                    {:else}
                        <strong>{crumb.label}</strong>
                    {/if}
                {/if}
            {/each}
        </div>
    </div>

    <div class="flex items-center gap-5 py-2.5">
        {#if mobileMenuItems.length > 0}
            <div class="relative hidden max-[1199px]:block">
                <button
                    class="flex items-center justify-center w-[34px] h-[34px] border border-(--border-strong) bg-(--surface-alt) text-(--text-dim) cursor-pointer transition-[background,color] duration-150 shrink-0 hover:bg-(--surface) hover:text-(--text)"
                    onclick={toggleMobileMenu}
                    aria-haspopup="true"
                    aria-expanded={mobileMenuOpen}
                    aria-label={mobileMenuAriaLabel}
                    title={mobileMenuAriaLabel}
                >
                    <Menu size={18} aria-hidden="true" />
                </button>

                {#if mobileMenuOpen}
                    <div
                        class="fixed inset-0 z-[99]"
                        onclick={closeMobileMenu}
                        role="presentation"
                    ></div>
                    <div class="absolute top-[calc(100%+8px)] right-0 z-[100] min-w-[min(280px,calc(100vw-32px))] bg-(--surface-alt) border border-(--border-strong) shadow-[0_8px_24px_rgba(0,0,0,0.18)] overflow-hidden [animation:fade-up-sm_0.12s_ease_both]" role="menu">
                        {#each mobileMenuItems as item}
                            <button
                                class="flex items-center gap-2.5 w-full px-3.5 py-2.5 bg-transparent border-0 text-(--text) text-sm font-[inherit] cursor-pointer text-left no-underline transition-[background] duration-[0.12s] hover:bg-(--surface)"
                                style:background-color={mobileMenuActiveId === item.id ? 'var(--surface)' : undefined}
                                style:color={mobileMenuActiveId === item.id ? 'var(--accent)' : undefined}
                                role="menuitem"
                                onclick={() => handleMobileMenuSelect(item.id)}
                            >
                                <div class="grid gap-0.5 min-w-0">
                                    {#if item.eyebrow}
                                        <span class="meta-label">{item.eyebrow}</span>
                                    {/if}
                                    <strong class="text-left">{item.label}</strong>
                                    {#if item.meta}
                                        <small class="text-left">{item.meta}</small>
                                    {/if}
                                </div>
                            </button>
                        {/each}
                    </div>
                {/if}
            </div>
        {/if}

        {#if adminHref}
            <a
                class="flex items-center justify-center w-[34px] h-[34px] border border-(--border-strong) bg-(--surface-alt) text-(--text-dim) cursor-pointer transition-[background,color] duration-150 no-underline shrink-0 hover:bg-(--surface) hover:text-(--text)"
                href={adminHref}
                title="Admin panel"
                aria-label="Admin panel"
            >
                <LayoutGrid size={18} aria-hidden="true" />
            </a>
        {/if}

        {#if currentUser && (onLogout || onShowQr || onMyAccount || userHomeHref)}
            <UserMenu
                {currentUser}
                {onLogout}
                {onShowQr}
                {onMyAccount}
                {onAppConfig}
                {userHomeHref}
            />
        {/if}
    </div>
</header>
