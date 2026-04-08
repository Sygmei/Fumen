<script lang="ts">
    import type { AppUser } from "../lib/api";

    let {
        breadcrumbs,
        currentUser,
        onLogout,
        onShowQr,
        adminHref,
        userHomeHref,
    }: {
        breadcrumbs: Array<{ label: string; href?: string }>;
        currentUser: AppUser | null;
        onLogout?: () => void;
        /** If set, "Log in on another device" appears in the user menu */
        onShowQr?: () => void;
        /** If set, an admin-panel icon link appears */
        adminHref?: string;
        /** If set, "User homepage" appears in the user menu */
        userHomeHref?: string;
    } = $props();

    let menuOpen = $state(false);

    function toggleMenu() {
        menuOpen = !menuOpen;
    }

    function closeMenu() {
        menuOpen = false;
    }

    function handleLogout() {
        closeMenu();
        onLogout?.();
    }

    function handleShowQr() {
        closeMenu();
        onShowQr?.();
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
        {#if currentUser}
            <span class="status-pill">{currentUser.role}</span>
        {/if}

        {#if adminHref}
            <a
                class="topbar-icon-btn"
                href={adminHref}
                title="Admin panel"
                aria-label="Admin panel"
            >
                <!-- 2×2 grid icon -->
                <svg
                    width="18"
                    height="18"
                    viewBox="0 0 18 18"
                    fill="none"
                    xmlns="http://www.w3.org/2000/svg"
                    aria-hidden="true"
                >
                    <rect
                        x="1"
                        y="1"
                        width="6"
                        height="6"
                        rx="1"
                        stroke="currentColor"
                        stroke-width="1.5"
                    />
                    <rect
                        x="11"
                        y="1"
                        width="6"
                        height="6"
                        rx="1"
                        stroke="currentColor"
                        stroke-width="1.5"
                    />
                    <rect
                        x="1"
                        y="11"
                        width="6"
                        height="6"
                        rx="1"
                        stroke="currentColor"
                        stroke-width="1.5"
                    />
                    <rect
                        x="11"
                        y="11"
                        width="6"
                        height="6"
                        rx="1"
                        stroke="currentColor"
                        stroke-width="1.5"
                    />
                </svg>
            </a>
        {/if}

        {#if currentUser && (onLogout || onShowQr || userHomeHref)}
            <div class="topbar-menu-wrap">
                <button
                    class="topbar-icon-btn"
                    onclick={toggleMenu}
                    aria-haspopup="true"
                    aria-expanded={menuOpen}
                    aria-label="User menu"
                    title="User menu"
                >
                    <!-- Person icon -->
                    <svg
                        width="18"
                        height="18"
                        viewBox="0 0 18 18"
                        fill="none"
                        xmlns="http://www.w3.org/2000/svg"
                        aria-hidden="true"
                    >
                        <circle
                            cx="9"
                            cy="5.5"
                            r="2.5"
                            stroke="currentColor"
                            stroke-width="1.5"
                        />
                        <path
                            d="M3 15.5c0-3.314 2.686-5.5 6-5.5s6 2.186 6 5.5"
                            stroke="currentColor"
                            stroke-width="1.5"
                            stroke-linecap="round"
                        />
                    </svg>
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
                            <strong>{currentUser.username}</strong>
                        </div>
                        {#if onShowQr}
                            <button
                                class="topbar-dropdown-item"
                                role="menuitem"
                                onclick={handleShowQr}
                            >
                                <!-- QR icon -->
                                <svg
                                    width="15"
                                    height="15"
                                    viewBox="0 0 15 15"
                                    fill="none"
                                    aria-hidden="true"
                                >
                                    <rect
                                        x="1"
                                        y="1"
                                        width="5"
                                        height="5"
                                        rx="0.5"
                                        stroke="currentColor"
                                        stroke-width="1.3"
                                    />
                                    <rect
                                        x="9"
                                        y="1"
                                        width="5"
                                        height="5"
                                        rx="0.5"
                                        stroke="currentColor"
                                        stroke-width="1.3"
                                    />
                                    <rect
                                        x="1"
                                        y="9"
                                        width="5"
                                        height="5"
                                        rx="0.5"
                                        stroke="currentColor"
                                        stroke-width="1.3"
                                    />
                                    <rect
                                        x="2.5"
                                        y="2.5"
                                        width="2"
                                        height="2"
                                        fill="currentColor"
                                    />
                                    <rect
                                        x="10.5"
                                        y="2.5"
                                        width="2"
                                        height="2"
                                        fill="currentColor"
                                    />
                                    <rect
                                        x="2.5"
                                        y="10.5"
                                        width="2"
                                        height="2"
                                        fill="currentColor"
                                    />
                                    <path
                                        d="M9 9h2v2H9zM11 11h2v2h-2zM13 9h1v1h-1zM9 11h1v3H9zM11 13h3v1h-3z"
                                        fill="currentColor"
                                    />
                                </svg>
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
                                <svg
                                    width="15"
                                    height="15"
                                    viewBox="0 0 15 15"
                                    fill="none"
                                    aria-hidden="true"
                                >
                                    <path
                                        d="M1.5 6.5L7.5 1.5L13.5 6.5V13.5H9.5V9.5H5.5V13.5H1.5V6.5Z"
                                        stroke="currentColor"
                                        stroke-width="1.3"
                                        stroke-linejoin="round"
                                    />
                                </svg>
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
                                <svg
                                    width="15"
                                    height="15"
                                    viewBox="0 0 15 15"
                                    fill="none"
                                    aria-hidden="true"
                                >
                                    <path
                                        d="M5.5 2.5H2.5V12.5H5.5"
                                        stroke="currentColor"
                                        stroke-width="1.3"
                                        stroke-linecap="round"
                                        stroke-linejoin="round"
                                    />
                                    <path
                                        d="M10.5 5L13.5 7.5L10.5 10"
                                        stroke="currentColor"
                                        stroke-width="1.3"
                                        stroke-linecap="round"
                                        stroke-linejoin="round"
                                    />
                                    <path
                                        d="M13.5 7.5H6.5"
                                        stroke="currentColor"
                                        stroke-width="1.3"
                                        stroke-linecap="round"
                                    />
                                </svg>
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
        border-radius: 8px;
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
        border-radius: 10px;
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

    .topbar-dropdown-user strong {
        font-size: 0.9rem;
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

    .topbar-dropdown-item svg {
        flex-shrink: 0;
        opacity: 0.7;
    }
</style>
