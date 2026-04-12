<script lang="ts">
  import { onDestroy, onMount, tick } from "svelte";
  import QRCode from "qrcode";
  import QrScanner from "qr-scanner";
  import qrWorkerUrl from "qr-scanner/qr-scanner-worker.min?url";
  import {
    clearAuth,
    createMyLoginLink,
    exchangeLoginToken,
    fetchCurrentUser,
    fetchUserLibrary,
    initAuth,
    isPageUnloading,
    listEnsembles,
    logout,
    setOnSessionExpired,
    setOnTokenRefreshed,
    type AppUser,
    type LoginLinkResponse,
    type UserLibraryEnsemble,
  } from "./lib/api";
  import ListenPage from "./pages/ListenPage.svelte";
  import AdminPage from "./pages/AdminPage.svelte";
  import HomePage from "./pages/HomePage.svelte";
  import CredentialModal from "./components/CredentialModal.svelte";
  import AppConfigModal from "./components/AppConfigModal.svelte";
  import ScannerModal from "./components/ScannerModal.svelte";
  import AccountModal from "./components/AccountModal.svelte";
  import { parseJwtSub } from "./lib/utils";
  (QrScanner as unknown as { WORKER_PATH: string }).WORKER_PATH = qrWorkerUrl;

  type AppRoute =
    | { kind: "user" }
    | { kind: "admin" }
    | { kind: "public"; accessKey: string }
    | { kind: "connect"; token: string };

  const storedRefreshToken =
    typeof window !== "undefined"
      ? (window.localStorage.getItem("refresh-token") ?? "").replace(
          /^undefined$/,
          "",
        )
      : "";
  const storedAccessToken =
    typeof window !== "undefined"
      ? (window.localStorage.getItem("access-token") ?? "")
      : "";
  const storedUsername = parseJwtSub(storedAccessToken);

  if (storedRefreshToken) {
    initAuth(storedRefreshToken, storedAccessToken);
  }

  let route = $state(
    resolveRoute(
      typeof window !== "undefined" ? window.location.pathname : "/",
    ),
  );

  let refreshToken = $state(storedRefreshToken);
  let currentUser = $state<AppUser | null>(null);
  let userSessionExpiresAt = $state<string | null>(null);
  let userLoading = $state(!!storedRefreshToken);
  let userError = $state("");
  let userSuccess = $state("");
  let userLibrary = $state<UserLibraryEnsemble[]>([]);
  let connectionBusy = $state(false);

  let credentialModalOpen = $state(false);
  let credentialModalTitle = $state("");
  let credentialLink = $state("");
  let credentialExpiresAt = $state("");
  let credentialQrDataUrl = $state("");
  let credentialQrLoading = $state(false);
  let credentialEyebrow = $state("Temporary access");
  let credentialLinkLabel = $state("Connection link");

  let scannerOpen = $state(false);
  let scannerError = $state("");
  let scannerVideo = $state<HTMLVideoElement | null>(null);
  let qrScanner: QrScanner | null = null;

  let accountModalOpen = $state(false);
  let appConfigModalOpen = $state(false);
  const storedCountInEnabled =
    typeof window !== "undefined"
      ? window.localStorage.getItem("app-enable-count-in") === "true"
      : false;
  let enableCountIn = $state(storedCountInEnabled);

  function handleMyAccount() {
    accountModalOpen = true;
  }

  function handleAppConfig() {
    appConfigModalOpen = true;
  }

  function setEnableCountIn(value: boolean) {
    enableCountIn = value;
    if (typeof window !== "undefined") {
      window.localStorage.setItem("app-enable-count-in", String(value));
    }
  }

  onMount(() => {
    const handlePopState = () => {
      route = resolveRoute(window.location.pathname);
      void syncRoute();
    };

    const syncViewportHeight = () => {
      const viewportHeight =
        window.visualViewport?.height ?? window.innerHeight;
      document.documentElement.style.setProperty(
        "--app-height",
        `${viewportHeight}px`,
      );
    };

    window.addEventListener("popstate", handlePopState);
    window.addEventListener("resize", syncViewportHeight);
    window.visualViewport?.addEventListener("resize", syncViewportHeight);
    syncViewportHeight();

    setOnSessionExpired(() => {
      clearUserSession();
      userError = "Your session has expired. Please sign in again.";
    });

    setOnTokenRefreshed((accessToken) => {
      window.localStorage.setItem("access-token", accessToken);
    });

    void syncRoute();

    return () => {
      window.removeEventListener("popstate", handlePopState);
      window.removeEventListener("resize", syncViewportHeight);
      window.visualViewport?.removeEventListener(
        "resize",
        syncViewportHeight,
      );
    };
  });

  onDestroy(() => {
    closeScanner();
  });

  function resolveRoute(pathname: string): AppRoute {
    const publicMatch = pathname.match(/^\/listen\/([^/]+)$/);
    if (publicMatch) {
      return { kind: "public", accessKey: decodeURIComponent(publicMatch[1]) };
    }

    const connectMatch = pathname.match(/^\/connect\/([^/]+)$/);
    if (connectMatch) {
      return { kind: "connect", token: decodeURIComponent(connectMatch[1]) };
    }

    if (pathname === "/admin") {
      return { kind: "admin" };
    }

    return { kind: "user" };
  }

  async function syncRoute() {
    if (route.kind === "public") {
      return;
    }

    if (route.kind === "connect") {
      await completeConnectionFromToken(route.token, true);
      return;
    }

    if (route.kind === "admin") {
      if (refreshToken) {
        await loadCurrentUser();
      }
      return;
    }

    if (refreshToken) {
      await loadCurrentUser();
    }
  }

  async function loadCurrentUser() {
    if (!refreshToken) {
      clearUserSession();
      return;
    }

    userLoading = true;

    try {
      const response = await fetchCurrentUser();
      let library = await fetchUserLibrary();
      if (
        response.user.role === "admin" ||
        response.user.role === "superadmin"
      ) {
        const adminEnsembles = await listEnsembles();
        const libraryById = new Map(
          library.ensembles.map((ensemble) => [ensemble.id, ensemble]),
        );
        library = {
          ensembles: adminEnsembles.map((ensemble) => ({
            id: ensemble.id,
            name: ensemble.name,
            scores: libraryById.get(ensemble.id)?.scores ?? [],
          })),
        };
      }
      currentUser = response.user;
      userSessionExpiresAt = response.session_expires_at;
      userLibrary = library.ensembles;
      userError = "";
    } catch (error) {
      // Ignore errors caused by the page unloading — never touch stored tokens.
      if (isPageUnloading()) return;
      if (error instanceof Error && error.name === "AbortError") return;
      userError =
        error instanceof Error
          ? error.message
          : "Unable to restore your session";
    } finally {
      userLoading = false;
    }
  }

  function clearUserState() {
    refreshToken = "";
    currentUser = null;
    userSessionExpiresAt = null;
    userLibrary = [];
    clearAuth();
  }

  function clearStoredSession() {
    window.localStorage.removeItem("refresh-token");
    window.localStorage.removeItem("access-token");
  }

  function clearUserSession() {
    clearUserState();
    clearStoredSession();
  }

  function persistUserSession(
    newRefreshToken: string,
    newAccessToken: string,
    user: AppUser,
    expiresAt: string | null,
  ) {
    if (!newRefreshToken) {
      userError = "Sign-in failed: server did not return a session token.";
      return;
    }
    initAuth(newRefreshToken, newAccessToken);
    refreshToken = newRefreshToken;
    currentUser = user;
    userSessionExpiresAt = expiresAt;
    userLoading = false;
    window.localStorage.setItem("refresh-token", newRefreshToken);
    window.localStorage.setItem("access-token", newAccessToken);
  }

  async function completeConnectionFromToken(token: string, fromRoute = false) {
    userLoading = true;
    connectionBusy = true;
    userError = "";
    userSuccess = "";

    try {
      const response = await exchangeLoginToken(token);
      persistUserSession(
        response.refresh_token,
        response.access_token,
        response.user,
        response.access_token_expires_at,
      );
      closeScanner();
      if (fromRoute || route.kind !== "user") {
        navigate("/", true);
      }
    } catch (error) {
      userError =
        error instanceof Error
          ? error.message
          : "Unable to use this connection link";
    } finally {
      connectionBusy = false;
      userLoading = false;
    }
  }

  function extractConnectionToken(value: string): string | null {
    const trimmed = value.trim();
    if (!trimmed) {
      return null;
    }

    try {
      const parsed = new URL(trimmed, window.location.origin);
      const match = parsed.pathname.match(/^\/connect\/([^/]+)$/);
      if (match) {
        return decodeURIComponent(match[1]);
      }
    } catch {
      // Ignore malformed URLs and try local patterns below.
    }

    const pathMatch = trimmed.match(/^\/connect\/([^/]+)$/);
    if (pathMatch) {
      return decodeURIComponent(pathMatch[1]);
    }

    if (/^[a-zA-Z0-9]+$/.test(trimmed)) {
      return trimmed;
    }

    return null;
  }

  async function handleShowMyQr() {
    if (!refreshToken || !currentUser) {
      userError = "Sign in first.";
      return;
    }

    userError = "";
    userSuccess = "";

    try {
      await showCredentialModal(`QR code for ${currentUser.username}`, () =>
        createMyLoginLink(),
      );
      userSuccess = "QR code ready.";
    } catch (error) {
      userError =
        error instanceof Error ? error.message : "Unable to create QR code";
    }
  }

  async function handleCopyMyLink() {
    if (!refreshToken || !currentUser) {
      userError = "Sign in first.";
      return;
    }

    userError = "";
    userSuccess = "";

    try {
      const response = await createMyLoginLink();
      await navigator.clipboard.writeText(response.connection_url);
      userSuccess = "Connection link copied to clipboard.";
    } catch (error) {
      userError =
        error instanceof Error
          ? error.message
          : "Unable to create connection link";
    }
  }

  async function logoutUser() {
    try {
      await logout();
    } catch {
      // best-effort: clear locally regardless
    }
    clearUserSession();
    userSuccess = "";
    userError = "";
  }

  async function showCredentialModal(
    title: string,
    loadLink: () => Promise<LoginLinkResponse>,
    options?: { eyebrow?: string; linkLabel?: string },
  ) {
    credentialModalTitle = title;
    credentialEyebrow = options?.eyebrow ?? "Temporary access";
    credentialLinkLabel = options?.linkLabel ?? "Connection link";
    credentialModalOpen = true;
    credentialQrLoading = true;
    credentialQrDataUrl = "";
    credentialLink = "";
    credentialExpiresAt = "";

    try {
      const linkResponse = await loadLink();
      credentialLink = linkResponse.connection_url;
      credentialExpiresAt = linkResponse.expires_at;
      credentialQrDataUrl = await QRCode.toDataURL(linkResponse.connection_url, {
        width: 360,
        margin: 1,
        color: {
          dark: "#111111",
          light: "#0000",
        },
      });
    } finally {
      credentialQrLoading = false;
    }
  }

  function navigate(pathname: string, replace = false) {
    if (typeof window === "undefined") {
      return;
    }

    const currentPath = window.location.pathname;
    if (currentPath !== pathname) {
      const method = replace ? "replaceState" : "pushState";
      window.history[method]({}, "", pathname);
    }

    route = resolveRoute(pathname);
    void syncRoute();
  }

  async function openScanner() {
    scannerError = "";
    scannerOpen = true;
    await tick();

    if (!scannerVideo) {
      scannerError = "Camera preview is unavailable on this device.";
      return;
    }

    try {
      qrScanner?.destroy();
      qrScanner = new QrScanner(
        scannerVideo,
        (result) => {
          const value = typeof result === "string" ? result : result.data;
          void handleScannedValue(value);
        },
        {
          highlightScanRegion: true,
          highlightCodeOutline: true,
        },
      );
      await qrScanner.start();
    } catch (error) {
      scannerError =
        error instanceof Error ? error.message : "Unable to start the camera";
    }
  }

  async function handleScannedValue(value: string) {
    const token = extractConnectionToken(value);
    if (!token) {
      scannerError = "That QR code is not a valid Fumen connection link.";
      return;
    }

    closeScanner();
    await completeConnectionFromToken(token);
  }

  function closeScanner() {
    qrScanner?.stop();
    qrScanner?.destroy();
    qrScanner = null;
    scannerOpen = false;
  }
</script>

{#if route.kind === "public"}
  <ListenPage accessKey={route.accessKey} {enableCountIn} />
{:else if route.kind === "admin"}
  <main class="page admin-shell">
  <AdminPage
      {currentUser}
      {userLoading}
      preloadedUsername={storedUsername}
      onShowQr={handleShowMyQr}
      onShowCredential={showCredentialModal}
      onLogout={logoutUser}
      onMyAccount={handleMyAccount}
      onAppConfig={handleAppConfig}
    />
  </main>
{:else}
  <HomePage
    routeKind={route.kind}
    {currentUser}
    {userLoading}
    preloadedUsername={storedUsername}
    {userLibrary}
    {connectionBusy}
    onLogout={logoutUser}
    onShowQr={handleShowMyQr}
    onOpenScanner={openScanner}
    onMyAccount={handleMyAccount}
    onAppConfig={handleAppConfig}
  />
{/if}

{#if userError || userSuccess}
  <div class="toast-stack" aria-live="polite" aria-atomic="true">
    {#if userError}
      <p class="status error toast">{userError}</p>
    {/if}
    {#if userSuccess}
      <p class="status success toast">{userSuccess}</p>
    {/if}
  </div>
{/if}

{#if credentialModalOpen}
  <CredentialModal
    title={credentialModalTitle}
    eyebrow={credentialEyebrow}
    linkLabel={credentialLinkLabel}
    qrDataUrl={credentialQrDataUrl}
    isLoading={credentialQrLoading}
    link={credentialLink}
    expiresAt={credentialExpiresAt}
    onClose={() => {
      credentialModalOpen = false;
      credentialQrLoading = false;
      credentialQrDataUrl = "";
      credentialLink = "";
      credentialExpiresAt = "";
    }}
  />
{/if}

{#if scannerOpen}
  <ScannerModal
    error={scannerError}
    onClose={closeScanner}
    bind:videoEl={scannerVideo}
  />
{/if}

{#if accountModalOpen && currentUser}
  <AccountModal
    {currentUser}
    onClose={() => { accountModalOpen = false; }}
    onSaved={(user) => {
      currentUser = user;
      accountModalOpen = false;
    }}
  />
{/if}

{#if appConfigModalOpen}
  <AppConfigModal
    enableCountIn={enableCountIn}
    onToggleCountIn={setEnableCountIn}
    onClose={() => {
      appConfigModalOpen = false;
    }}
  />
{/if}
