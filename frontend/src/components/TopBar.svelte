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

<header class="admin-topbar">
    <div class="admin-topbar-title">
        <div class="admin-breadcrumb" aria-label="Breadcrumb">
            {#each breadcrumbs as crumb, i}
                {#if i === 0}
                    {#if crumb.href}
                        <a class="admin-brand" href={crumb.href}
                            >{crumb.label}</a
                        >
                    {:else}
                        <span class="admin-brand">{crumb.label}</span>
                    {/if}
                {:else if i === 1}
                    <span
                        class="admin-breadcrumb-separator admin-breadcrumb-dash"
                        >—</span
                    >
                    {#if crumb.href}
                        <a href={crumb.href}>{crumb.label}</a>
                    {:else}
                        <span>{crumb.label}</span>
                    {/if}
                {:else}
                    <span class="admin-breadcrumb-separator">/</span>
                    {#if crumb.href}
                        <strong><a href={crumb.href}>{crumb.label}</a></strong>
                    {:else}
                        <strong>{crumb.label}</strong>
                    {/if}
                {/if}
            {/each}
        </div>
    </div>

    <div class="admin-topbar-actions">
        {#if mobileMenuItems.length > 0}
            <div class="topbar-menu-wrap topbar-mobile-only">
                <button
                    class="topbar-icon-btn"
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
                        class="topbar-menu-backdrop"
                        onclick={closeMobileMenu}
                        role="presentation"
                    ></div>
                    <div class="topbar-dropdown topbar-mobile-dropdown" role="menu">
                        {#each mobileMenuItems as item}
                            <button
                                class="topbar-dropdown-item"
                                class:topbar-dropdown-item--active={mobileMenuActiveId ===
                                    item.id}
                                role="menuitem"
                                onclick={() => handleMobileMenuSelect(item.id)}
                            >
                                <div class="topbar-dropdown-copy">
                                    {#if item.eyebrow}
                                        <span class="meta-label">{item.eyebrow}</span>
                                    {/if}
                                    <strong>{item.label}</strong>
                                    {#if item.meta}
                                        <small>{item.meta}</small>
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
                class="topbar-icon-btn"
                href={adminHref}
                title="Admin panel"
                aria-label="Admin panel"
            >
                <LayoutGrid size={18} aria-hidden="true" />
            </a>
        {/if}

        {#if currentUser && (onLogout || onShowQr || onMyAccount || userHomeHref)}
            <div class="topbar-menu-wrap">
                <button
                    class="topbar-icon-btn topbar-user-btn"
                    onclick={toggleMenu}
                    aria-haspopup="true"
                    aria-expanded={menuOpen}
                    aria-label="User menu"
                    title="User menu"
                >
                    {#if currentUser.avatar_url}
                        <img src={currentUser.avatar_url} alt="" class="topbar-avatar-img" />
                    {:else}
                        <User size={18} aria-hidden="true" />
                    {/if}
                </button>

                {#if menuOpen}
                    <!-- svelte-ignore a11y_click_events_have_key_events -->
                    <div
                        class="topbar-menu-backdrop"
                        onclick={closeMenu}
                        role="presentation"
                    ></div>
                    <div class="topbar-dropdown" role="menu">
                        <div class="topbar-dropdown-user">
                            <span class="meta-label">Signed in as</span>
                            <div class="topbar-dropdown-userline">
                                <strong>{currentUser.display_name ?? currentUser.username}</strong>
                                <span class="topbar-role-badge">
                                    {currentUser.role}
                                </span>
                            </div>
                        </div>
                        {#if onMyAccount}
                            <button
                                class="topbar-dropdown-item"
                                role="menuitem"
                                onclick={handleMyAccount}
                            >
                                <UserCog size={15} aria-hidden="true" />
                                My account
                            </button>
                        {/if}
                        {#if onShowQr}
                            <button
                                class="topbar-dropdown-item"
                                role="menuitem"
                                onclick={handleShowQr}
                            >
                                <!-- QR icon -->
                                <QrCode size={15} aria-hidden="true" />
                                Log in on another device
                            </button>
                        {/if}
                        {#if userHomeHref}
                            <a
                                class="topbar-dropdown-item"
                                role="menuitem"
                                href={userHomeHref}
                                onclick={closeMenu}
                            >
                                <!-- House icon -->
                                <House size={15} aria-hidden="true" />
                                User homepage
                            </a>
                        {/if}
                        {#if onLogout}
                            <button
                                class="topbar-dropdown-item topbar-dropdown-item--danger"
                                role="menuitem"
                                onclick={handleLogout}
                            >
                                <!-- Sign-out arrow icon -->
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

<style>
    .topbar-icon-btn {
        display: flex;
        align-items: center;
        justify-content: center;
        width: 34px;
        height: 34px;
        border-radius: var(--radius-md);
        border: 1px solid var(--border-strong);
        background: var(--surface-alt);
        color: var(--text-muted);
        cursor: pointer;
        transition:
            background 0.15s,
            color 0.15s,
            border-color 0.15s;
        text-decoration: none;
        flex-shrink: 0;
    }

    .topbar-icon-btn:hover {
        background: var(--surface);
        color: var(--text);
        border-color: var(--border-strong);
    }

    .topbar-user-btn {
        padding: 0;
        overflow: hidden;
        border-radius: 50%;
    }

    .topbar-avatar-img {
        width: 100%;
        height: 100%;
        object-fit: cover;
        border-radius: 50%;
        display: block;
    }

    .topbar-menu-wrap {
        position: relative;
    }

    .topbar-menu-backdrop {
        position: fixed;
        inset: 0;
        z-index: 99;
    }

    .topbar-dropdown {
        position: absolute;
        top: calc(100% + 8px);
        right: 0;
        z-index: 100;
        min-width: 210px;
        background: var(--surface-alt);
        border: 1px solid var(--border-strong);
        border-radius: var(--radius-lg);
        box-shadow: 0 8px 24px rgba(0, 0, 0, 0.18);
        overflow: hidden;
        animation: fade-up-sm 0.12s ease both;
    }

    @keyframes fade-up-sm {
        from {
            opacity: 0;
            transform: translateY(4px);
        }
        to {
            opacity: 1;
            transform: translateY(0);
        }
    }

    .topbar-dropdown-user {
        padding: 10px 14px 8px;
        border-bottom: 1px solid var(--border);
        display: flex;
        flex-direction: column;
        gap: 2px;
    }

    .topbar-dropdown-userline {
        display: flex;
        align-items: center;
        gap: 8px;
        min-width: 0;
    }

    .topbar-dropdown-user strong {
        font-size: 0.9rem;
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .topbar-role-badge {
        display: inline-flex;
        align-items: center;
        flex-shrink: 0;
        padding: 2px 6px;
        border: 1px solid var(--border-strong);
        background: var(--surface);
        color: var(--accent);
        font-family: 'Fira Code', 'Cascadia Code', monospace;
        font-size: 0.62rem;
        font-weight: 600;
        line-height: 1;
        letter-spacing: 0.08em;
        text-transform: uppercase;
    }

    .topbar-dropdown-item {
        display: flex;
        align-items: center;
        gap: 9px;
        width: 100%;
        padding: 9px 14px;
        background: transparent;
        border: none;
        color: var(--text);
        font-size: 0.875rem;
        font-family: inherit;
        cursor: pointer;
        text-align: left;
        text-decoration: none;
        transition: background 0.12s;
    }

    .topbar-dropdown-item:hover {
        background: var(--surface);
    }

    .topbar-dropdown-item--danger {
        color: var(--accent);
    }

    .topbar-dropdown-item--active {
        background: var(--surface);
        color: var(--accent);
    }

    .topbar-dropdown-copy {
        display: grid;
        gap: 2px;
        min-width: 0;
    }

    .topbar-dropdown-copy strong,
    .topbar-dropdown-copy small {
        text-align: left;
    }

    .topbar-dropdown-item :global(svg) {
        flex-shrink: 0;
        opacity: 0.7;
    }

    .topbar-mobile-only {
        display: none;
    }

    @media (max-width: 1199px) {
        .topbar-mobile-only {
            display: block;
        }

        .topbar-mobile-dropdown {
            min-width: min(280px, calc(100vw - 32px));
        }
    }
</style>
