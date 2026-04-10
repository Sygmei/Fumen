<script lang="ts">
    import type { AppUser } from "../lib/api";
    import { Menu, LayoutGrid, User, QrCode, House, LogOut, UserCog } from '@lucide/svelte';

    let {
        breadcrumbs,
        currentUser,
        onLogout,
        onShowQr,
        onMyAccount,
        adminHref,
        userHomeHref,
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
        /** If set, an admin-panel icon link appears */
        adminHref?: string;
        /** If set, "User homepage" appears in the user menu */
        userHomeHref?: string;
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

    let menuOpen = $state(false);
    let mobileMenuOpen = $state(false);

    function toggleMenu() {
        menuOpen = !menuOpen;
    }

    function closeMenu() {
        menuOpen = false;
    }

    function toggleMobileMenu() {
        mobileMenuOpen = !mobileMenuOpen;
    }

    function closeMobileMenu() {
        mobileMenuOpen = false;
    }

    function handleLogout() {
        closeMenu();
        onLogout?.();
    }

    function handleShowQr() {
        closeMenu();
        onShowQr?.();
    }

    function handleMyAccount() {
        closeMenu();
        onMyAccount?.();
    }

    function handleMobileMenuSelect(id: string) {
        closeMobileMenu();
        onMobileMenuSelect?.(id);
    }
</script>

<header class="flex items-stretch justify-between gap-6 min-h-[54px] px-7 border-b border-(--border-strong) bg-[linear-gradient(90deg,rgba(196,43,13,0.12),transparent_26%),var(--surface)]">
    <div class="flex items-center gap-5 py-2.5">
        <div class="flex items-center gap-2.5 font-bold text-[clamp(1.4rem,2vw,2rem)] leading-none" aria-label="Breadcrumb">
            {#each breadcrumbs as crumb, i}
                {#if i === 0}
                    {#if crumb.href}
                        <a class="font-brand text-[clamp(1.7rem,2.5vw,2.4rem)] leading-none text-[#c42b0d] no-underline flex items-center self-stretch pr-1" href={crumb.href}>{crumb.label}</a>
                    {:else}
                        <span class="font-brand text-[clamp(1.7rem,2.5vw,2.4rem)] leading-none text-[#c42b0d] flex items-center self-stretch pr-1">{crumb.label}</span>
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
            <div class="relative">
                <button
                    class="flex items-center justify-center w-[34px] h-[34px] border border-(--border-strong) bg-(--surface-alt) text-(--text-dim) cursor-pointer transition-[background,color] duration-150 shrink-0 hover:bg-(--surface) hover:text-(--text) p-0 overflow-hidden rounded-full"
                    onclick={toggleMenu}
                    aria-haspopup="true"
                    aria-expanded={menuOpen}
                    aria-label="User menu"
                    title="User menu"
                >
                    {#if currentUser.avatar_url}
                        <img src={currentUser.avatar_url} alt="" class="w-full h-full object-cover rounded-full block" />
                    {:else}
                        <User size={18} aria-hidden="true" />
                    {/if}
                </button>

                {#if menuOpen}
                    <!-- svelte-ignore a11y_click_events_have_key_events -->
                    <div
                        class="fixed inset-0 z-[99]"
                        onclick={closeMenu}
                        role="presentation"
                    ></div>
                    <div class="absolute top-[calc(100%+8px)] right-0 z-[100] min-w-[210px] bg-(--surface-alt) border border-(--border-strong) shadow-[0_8px_24px_rgba(0,0,0,0.18)] overflow-hidden [animation:fade-up-sm_0.12s_ease_both]" role="menu">
                        <div class="px-3.5 pt-2.5 pb-2 border-b border-(--border) flex flex-col gap-0.5">
                            <span class="meta-label">Signed in as</span>
                            <div class="flex items-center gap-2 min-w-0">
                                <strong class="text-[0.9rem] min-w-0 overflow-hidden text-ellipsis whitespace-nowrap">{currentUser.display_name ?? currentUser.username}</strong>
                                <span class="inline-flex items-center shrink-0 px-1.5 py-0.5 border border-(--border-strong) bg-(--surface) text-(--accent) font-mono text-[0.62rem] font-semibold leading-none tracking-[0.08em] uppercase">
                                    {currentUser.role}
                                </span>
                            </div>
                        </div>
                        {#if onMyAccount}
                            <button
                                class="flex items-center gap-2.5 w-full px-3.5 py-2.5 bg-transparent border-0 text-(--text) text-sm font-[inherit] cursor-pointer text-left no-underline transition-[background] duration-[0.12s] hover:bg-(--surface) [&_svg]:shrink-0 [&_svg]:opacity-70"
                                role="menuitem"
                                onclick={handleMyAccount}
                            >
                                <UserCog size={15} aria-hidden="true" />
                                My account
                            </button>
                        {/if}
                        {#if onShowQr}
                            <button
                                class="flex items-center gap-2.5 w-full px-3.5 py-2.5 bg-transparent border-0 text-(--text) text-sm font-[inherit] cursor-pointer text-left no-underline transition-[background] duration-[0.12s] hover:bg-(--surface) [&_svg]:shrink-0 [&_svg]:opacity-70"
                                role="menuitem"
                                onclick={handleShowQr}
                            >
                                <QrCode size={15} aria-hidden="true" />
                                Log in on another device
                            </button>
                        {/if}
                        {#if userHomeHref}
                            <a
                                class="flex items-center gap-2.5 w-full px-3.5 py-2.5 bg-transparent border-0 text-(--text) text-sm font-[inherit] cursor-pointer text-left no-underline transition-[background] duration-[0.12s] hover:bg-(--surface) [&_svg]:shrink-0 [&_svg]:opacity-70"
                                role="menuitem"
                                href={userHomeHref}
                                onclick={closeMenu}
                            >
                                <House size={15} aria-hidden="true" />
                                User homepage
                            </a>
                        {/if}
                        {#if onLogout}
                            <button
                                class="flex items-center gap-2.5 w-full px-3.5 py-2.5 bg-transparent border-0 text-(--accent) text-sm font-[inherit] cursor-pointer text-left no-underline transition-[background] duration-[0.12s] hover:bg-(--surface) [&_svg]:shrink-0 [&_svg]:opacity-70"
                                role="menuitem"
                                onclick={handleLogout}
                            >
                                <LogOut size={15} aria-hidden="true" />
                                Sign out
                            </button>
                        {/if}
                    </div>
                {/if}
            </div>
        {/if}
    </div>
</header>
