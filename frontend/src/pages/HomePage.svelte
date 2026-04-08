<script lang="ts">
    import type { AppUser, UserLibraryEnsemble } from "../lib/api";
    import { prettyDate } from "../lib/utils";

    let {
        routeKind,
        currentUser,
        userLoading,
        userError,
        userSuccess,
        userLibrary,
        userSessionExpiresAt,
        connectionBusy,
        preloadedUsername,
        manualConnectionLink = $bindable(),
        onLogout,
        onShowQr,
        onCopyLink,
        onOpenScanner,
        onManualConnect,
    }: {
        routeKind: string;
        currentUser: AppUser | null;
        userLoading: boolean;
        userError: string;
        preloadedUsername: string;
        userSuccess: string;
        userLibrary: UserLibraryEnsemble[];
        userSessionExpiresAt: string | null;
        connectionBusy: boolean;
        manualConnectionLink: string;
        onLogout: () => Promise<void>;
        onShowQr: () => Promise<void>;
        onCopyLink: () => Promise<void>;
        onOpenScanner: () => void;
        onManualConnect: () => Promise<void>;
    } = $props();

    function canAccessAdmin() {
        return (
            currentUser?.role === "admin" || currentUser?.role === "superadmin"
        );
    }
</script>

<main class="page home-shell">
    {#if userLoading && !currentUser}
        <div class="home-loading-overlay">
            <div class="loading-eq" aria-label="Loading">
                <span></span>
                <span></span>
                <span></span>
                <span></span>
                <span></span>
            </div>
            <p class="loading-eq-label">{preloadedUsername ? `Hello, ${preloadedUsername}` : 'Fumen'}</p>
        </div>
    {:else}
        <section class="hero-panel">
            <div class="hero-actions">
                <div>
                    <p class="eyebrow">Fumen • Users</p>
                    <h1>
                        {currentUser
                            ? `Welcome, ${currentUser.username}`
                            : "Connect a device"}
                    </h1>
                    <p class="lede">
                        {#if currentUser}You are signed in on this device.
                            Generate a QR code or a short-lived connection link
                            to open your session somewhere else.{:else}Scan a QR
                            code from the admin panel or another signed-in
                            device, or paste a 5-minute connection link to log
                            in.{/if}
                    </p>
                </div>
                <div class="hero-actions-stack">
                    {#if currentUser}
                        <button
                            class="button secondary"
                            onclick={() => void onShowQr()}>Show QR code</button
                        >
                        <button
                            class="button ghost"
                            onclick={() => void onCopyLink()}
                            >Copy connection link</button
                        >
                        {#if canAccessAdmin()}
                            <a class="button ghost" href="/admin">Admin panel</a
                            >
                        {/if}
                        <button
                            class="button ghost"
                            onclick={() => void onLogout()}>Log out</button
                        >
                    {:else}
                        <button class="button" onclick={() => onOpenScanner()}
                            >Scan QR code</button
                        >
                    {/if}
                </div>
            </div>
        </section>
        <section class="content-panel home-grid">
            {#if routeKind === "connect"}
                <div class="music-card connect-card">
                    <p class="meta-label">Connection</p>
                    <h2>Finishing sign-in</h2>
                    <p class="lede">
                        We are validating your temporary connection link.
                    </p>
                    {#if connectionBusy || userLoading}<p class="status">
                            Connecting...
                        </p>{/if}{#if userError}<p class="status error">
                            {userError}
                        </p>{/if}
                </div>
            {:else if currentUser}
                <div class="music-card">
                    <div class="card-header">
                        <div>
                            <p class="meta-label">Session</p>
                            <h2>{currentUser.username}</h2>
                        </div>
                        <p class="status-pill">signed in</p>
                    </div>
                    <div class="meta-grid">
                        <div>
                            <p class="meta-label">Username</p>
                            <p>{currentUser.username}</p>
                        </div>
                        <div>
                            <p class="meta-label">Access token until</p>
                            <p>
                                {userSessionExpiresAt
                                    ? prettyDate(userSessionExpiresAt)
                                    : "—"}
                            </p>
                        </div>
                        <div>
                            <p class="meta-label">Accessible ensembles</p>
                            <p>{userLibrary.length}</p>
                        </div>
                    </div>
                    <p class="hint">
                        Every QR code and connection link is single-use and
                        valid for 5 minutes.
                    </p>
                    {#if userError}<p class="status error">{userError}</p>{/if}
                    {#if userSuccess}<p class="status success">
                            {userSuccess}
                        </p>{/if}
                </div>
                <div class="music-card">
                    <div class="card-header">
                        <div>
                            <p class="meta-label">Library</p>
                            <h2>Your ensembles</h2>
                        </div>
                    </div>
                    {#if userLibrary.length === 0}
                        <p class="hint">
                            No scores are available for your ensembles yet.
                        </p>
                    {:else}
                        <div class="directory-stack">
                            {#each userLibrary as ensemble}
                                <section class="directory-panel">
                                    <div class="music-topline">
                                        <div>
                                            <h3>{ensemble.name}</h3>
                                            <p class="subtle">
                                                {ensemble.scores.length} scores
                                            </p>
                                        </div>
                                    </div>
                                    <div class="score-link-list">
                                        {#each ensemble.scores as score}
                                            <a
                                                class="score-link-row"
                                                href={score.public_url}
                                            >
                                                <span>{score.title}</span>
                                                <small>{score.filename}</small>
                                            </a>
                                        {/each}
                                    </div>
                                </section>
                            {/each}
                        </div>
                    {/if}
                </div>
            {:else}
                <div class="music-card">
                    <div class="card-header">
                        <div>
                            <p class="meta-label">Connect</p>
                            <h2>Use a connection link</h2>
                        </div>
                        <button
                            class="button secondary"
                            onclick={() => onOpenScanner()}>Scan QR code</button
                        >
                    </div>
                    <label class="field"
                        ><span>Connection link</span><input
                            bind:value={manualConnectionLink}
                            placeholder="Paste a link like https://.../connect/..."
                        /></label
                    >
                    <div class="actions">
                        <button
                            class="button"
                            onclick={() => void onManualConnect()}
                            disabled={connectionBusy}
                            >{connectionBusy
                                ? "Connecting..."
                                : "Connect this device"}</button
                        ><a class="button ghost" href="/admin"
                            >Open admin panel</a
                        >
                    </div>
                    {#if userError}<p class="status error">{userError}</p>{/if}
                    {#if userSuccess}<p class="status success">
                            {userSuccess}
                        </p>{/if}
                </div>
                <div class="music-card">
                    <p class="meta-label">How it works</p>
                    <h2>Username-only access</h2>
                    <p class="lede">
                        An admin creates your username once. After that, any
                        signed-in device can generate a temporary QR code or
                        link for another device.
                    </p>
                    <p class="hint">
                        If your browser blocks camera access, paste the
                        connection link here instead.
                    </p>
                </div>
            {/if}
        </section>
    {/if}
</main>
