<script lang="ts">
    import type { UserResponse as AppUser } from "$backend/models";
    import {
        House,
        LogOut,
        QrCode,
        Settings2,
        User,
        UserCog,
    } from "@lucide/svelte";

    let {
        currentUser,
        onLogout,
        onShowQr,
        onMyAccount,
        onAppConfig,
        userHomeHref,
    }: {
        currentUser: AppUser;
        onLogout?: () => void;
        onShowQr?: () => void;
        onMyAccount?: () => void;
        onAppConfig?: () => void;
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

    function handleMyAccount() {
        closeMenu();
        onMyAccount?.();
    }

    function handleAppConfig() {
        closeMenu();
        onAppConfig?.();
    }
</script>

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
            <img
                src={currentUser.avatar_url}
                alt=""
                class="w-full h-full object-cover rounded-full block"
            />
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
        <div
            class="absolute top-[calc(100%+8px)] right-0 z-[100] min-w-[210px] bg-(--surface-alt) border border-(--border-strong) shadow-[0_8px_24px_rgba(0,0,0,0.18)] overflow-hidden [animation:fade-up-sm_0.12s_ease_both]"
            role="menu"
        >
            <div
                class="px-3.5 pt-2.5 pb-2 border-b border-(--border) flex flex-col gap-0.5"
            >
                <span class="meta-label">Signed in as</span>
                <div class="flex items-center gap-2 min-w-0">
                    <strong
                        class="text-[0.9rem] min-w-0 overflow-hidden text-ellipsis whitespace-nowrap"
                    >
                        {currentUser.display_name ?? currentUser.username}
                    </strong>
                    <span
                        class="inline-flex items-center shrink-0 px-1.5 py-0.5 border border-(--border-strong) bg-(--surface) text-(--accent) font-mono text-[0.62rem] font-semibold leading-none tracking-[0.08em] uppercase"
                    >
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
            {#if onAppConfig}
                <button
                    class="flex items-center gap-2.5 w-full px-3.5 py-2.5 bg-transparent border-0 text-(--text) text-sm font-[inherit] cursor-pointer text-left no-underline transition-[background] duration-[0.12s] hover:bg-(--surface) [&_svg]:shrink-0 [&_svg]:opacity-70"
                    role="menuitem"
                    onclick={handleAppConfig}
                >
                    <Settings2 size={15} aria-hidden="true" />
                    App settings
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
                <button
                    class="flex items-center gap-2.5 w-full px-3.5 py-2.5 bg-transparent border-0 text-(--text) text-sm font-[inherit] cursor-pointer text-left no-underline transition-[background] duration-[0.12s] hover:bg-(--surface) [&_svg]:shrink-0 [&_svg]:opacity-70"
                    role="menuitem"
                    onclick={() => {
                        window.location.href = userHomeHref;
                        closeMenu();
                    }}
                >
                    <House size={15} aria-hidden="true" />
                    User homepage
                </button>
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
