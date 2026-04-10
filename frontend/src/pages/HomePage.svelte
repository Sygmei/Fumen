<script lang="ts">
    import ScoreIcon from "../components/ScoreIcon.svelte";
    import type { AppUser, UserLibraryEnsemble } from "../lib/api";
    import TopBar from "../components/TopBar.svelte";
    import { QrCode } from '@lucide/svelte';

    let {
        routeKind,
        currentUser,
        userLoading,
        userError,
        userSuccess,
        userLibrary,
        connectionBusy,
        preloadedUsername,
        onLogout,
        onShowQr,
        onOpenScanner,
        onMyAccount,
    }: {
        routeKind: string;
        currentUser: AppUser | null;
        userLoading: boolean;
        userError: string;
        preloadedUsername: string;
        userSuccess: string;
        userLibrary: UserLibraryEnsemble[];
        connectionBusy: boolean;
        onLogout: () => Promise<void>;
        onShowQr: () => Promise<void>;
        onOpenScanner: () => void;
        onMyAccount?: () => void;
    } = $props();

    function canAccessAdmin() {
        return currentUser?.role !== undefined && currentUser.role !== "user";
    }

    function ensembleAccent(name: string) {
        let hash = 0;

        for (let i = 0; i < name.length; i += 1) {
            hash = (hash * 31 + name.charCodeAt(i)) >>> 0;
        }

        const hue = hash % 360;
        const hueAlt = (hue + 32) % 360;

        return [
            `--ensemble-hue: ${hue}`,
            `--ensemble-hue-alt: ${hueAlt}`,
            `--ensemble-accent: hsl(${hue} 68% 46%)`,
            `--ensemble-accent-soft: hsl(${hue} 76% 95%)`,
            `--ensemble-accent-border: hsl(${hue} 52% 78%)`,
        ].join("; ");
    }

    function scoreLabel(count: number) {
        return `${count} score${count === 1 ? "" : "s"}`;
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
            <p class="loading-eq-label">
                {preloadedUsername ? `Hello, ${preloadedUsername}` : "Fumen"}
            </p>
        </div>
    {:else}
        <TopBar
            breadcrumbs={[{ label: "Fumen", href: "/" }]}
            {currentUser}
            adminHref={canAccessAdmin() ? "/admin" : undefined}
            onShowQr={() => void onShowQr()}
            onLogout={() => void onLogout()}
            {onMyAccount}
        />
        <section
            class="content-panel home-grid"
            class:home-landing-stage={!currentUser && routeKind !== "connect"}
            class:home-library-stage={!!currentUser}
        >
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
                <div class="music-card library-card">
                    <div class="card-header library-header">
                        <div class="library-header-copy">
                            <p class="meta-label">Library</p>
                            <h2>Your ensembles</h2>
                            <p class="lede">
                                Jump between ensembles and open any score in one
                                click.
                            </p>
                        </div>
                    </div>
                    {#if userError}<p class="status error">{userError}</p>{/if}
                    {#if userSuccess}<p class="status success">
                            {userSuccess}
                        </p>{/if}
                    {#if userLibrary.length === 0}
                        <p class="hint">
                            No scores are available for your ensembles yet.
                        </p>
                    {:else}
                        <div class="directory-stack library-accordion">
                            {#each userLibrary as ensemble, index}
                                <details
                                    class="directory-panel ensemble-accordion"
                                    open={index === 0}
                                    style={ensembleAccent(ensemble.name)}
                                >
                                    <summary class="ensemble-summary">
                                        <div class="ensemble-summary-main">
                                            <span class="ensemble-pill">
                                                {ensemble.name.slice(0, 1)}
                                            </span>
                                            <div class="ensemble-summary-copy">
                                                <h3>{ensemble.name}</h3>
                                                <p class="subtle">
                                                    {scoreLabel(
                                                        ensemble.scores.length,
                                                    )}
                                                </p>
                                            </div>
                                        </div>
                                        <span
                                            class="ensemble-summary-icon"
                                            aria-hidden="true"
                                        ></span>
                                    </summary>
                                    <div
                                        class="score-link-list library-score-grid"
                                    >
                                        {#if ensemble.scores.length === 0}
                                            <p class="hint">
                                                No scores are available for this
                                                ensemble yet.
                                            </p>
                                        {:else}
                                            {#each ensemble.scores as score}
                                                <a
                                                    class="score-link-row"
                                                    href={score.public_url}
                                                >
                                                    <span class="score-link-title">
                                                        <ScoreIcon
                                                            variant="library"
                                                            icon={score.icon}
                                                            imageUrl={score.icon_image_url}
                                                        />
                                                        <span>{score.title}</span>
                                                    </span>
                                                </a>
                                            {/each}
                                        {/if}
                                    </div>
                                </details>
                            {/each}
                        </div>
                    {/if}
                </div>
            {:else}
                <div class="music-card landing-card">
                    <div class="landing-card-copy">
                        <p class="meta-label">Instant Access</p>
                        <h1>Open-source score manager.</h1>
                        <p class="landing-lede">
                            Scan the QR code from your conductor or admin and
                            jump straight into your library. If someone sends
                            you a Fumen link, opening it here signs you in
                            automatically.
                        </p>
                        <div class="landing-actions">
                            <button
                                class="button landing-cta"
                                onclick={() => onOpenScanner()}
                            >
                                <QrCode size={18} aria-hidden="true" />
                                {connectionBusy
                                    ? "Opening camera..."
                                    : "Scan QR code"}
                            </button>
                            <p class="landing-note">
                                Best on mobile. Camera access opens in a secure
                                scanner window.
                            </p>
                        </div>
                    </div>
                    <div class="landing-orbit" aria-hidden="true">
                        <div class="landing-orbit-ring"></div>
                        <div
                            class="landing-orbit-ring landing-orbit-ring-alt"
                        ></div>
                        <div class="landing-orbit-core">
                            <span>Fumen</span>
                            <strong>Scan to enter</strong>
                        </div>
                        <div class="landing-chip landing-chip-a">Orchestra</div>
                        <div class="landing-chip landing-chip-b">Library</div>
                        <div class="landing-chip landing-chip-c">
                            Live score
                        </div>
                    </div>
                    {#if userError}<p class="status error landing-status">
                            {userError}
                        </p>{/if}
                    {#if userSuccess}<p class="status success landing-status">
                            {userSuccess}
                        </p>{/if}
                </div>
            {/if}
        </section>
    {/if}
</main>
