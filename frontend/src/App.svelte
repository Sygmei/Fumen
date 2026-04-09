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
  import ScannerModal from "./components/ScannerModal.svelte";
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

  function readCachedUser(): AppUser | null {
    try {
      const raw = window.localStorage.getItem("cached-user");
      return raw ? (JSON.parse(raw) as AppUser) : null;
    } catch {
      return null;
    }
  }

  function readCachedLibrary(): UserLibraryEnsemble[] {
    try {
      const raw = window.localStorage.getItem("cached-library");
      return raw ? (JSON.parse(raw) as UserLibraryEnsemble[]) : [];
    } catch {
      return [];
    }
  }

  const cachedUser = typeof window !== "undefined" ? readCachedUser() : null;
  const cachedLibrary =
    typeof window !== "undefined" ? readCachedLibrary() : [];

  if (storedRefreshToken) {
    initAuth(storedRefreshToken, storedAccessToken);
  }

  let route = $state(
    resolveRoute(
      typeof window !== "undefined" ? window.location.pathname : "/",
    ),
  );

  let refreshToken = $state(storedRefreshToken);
  let currentUser = $state<AppUser | null>(cachedUser);
  let userSessionExpiresAt = $state<string | null>(null);
  let userLoading = $state(!!storedRefreshToken);
  let userError = $state("");
  let userSuccess = $state("");
  let userLibrary = $state<UserLibraryEnsemble[]>(cachedLibrary);
  let connectionBusy = $state(false);

  let credentialModalOpen = $state(false);
  let credentialModalTitle = $state("");
  let credentialLink = $state("");
  let credentialExpiresAt = $state("");
  let credentialQrDataUrl = $state("");

  let scannerOpen = $state(false);
  let scannerError = $state("");
  let scannerVideo = $state<HTMLVideoElement | null>(null);
  let qrScanner: QrScanner | null = null;

  onMount(() => {
    const handlePopState = () => {
      route = resolveRoute(window.location.pathname);
      void syncRoute();
    };

    window.addEventListener("popstate", handlePopState);

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
      const library = await fetchUserLibrary();
      currentUser = response.user;
      userSessionExpiresAt = response.session_expires_at;
      userLibrary = library.ensembles;
      userError = "";
      window.localStorage.setItem("cached-user", JSON.stringify(response.user));
      window.localStorage.setItem(
        "cached-library",
        JSON.stringify(library.ensembles),
      );
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
    window.localStorage.removeItem("cached-user");
    window.localStorage.removeItem("cached-library");
    window.localStorage.removeItem("cached-admin-musics");
    window.localStorage.removeItem("cached-admin-users");
    window.localStorage.removeItem("cached-admin-ensembles");
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
    window.localStorage.setItem("cached-user", JSON.stringify(user));
    window.localStorage.setItem("cached-library", JSON.stringify([]));
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
      const response = await createMyLoginLink();
      await showCredentialModal(
        `QR code for ${currentUser.username}`,
        response,
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
    linkResponse: LoginLinkResponse,
  ) {
    credentialModalTitle = title;
    credentialLink = linkResponse.connection_url;
    credentialExpiresAt = linkResponse.expires_at;
    credentialQrDataUrl = await QRCode.toDataURL(linkResponse.connection_url, {
      width: 280,
      margin: 1,
      color: {
        dark: "#111111",
        light: "#0000",
      },
    });
    credentialModalOpen = true;
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
  <ListenPage accessKey={route.accessKey} />
{:else if route.kind === "admin"}
  <main class="page admin-shell">
    <AdminPage
      {currentUser}
      {userLoading}
      {userError}
      preloadedUsername={storedUsername}
      onShowCredential={showCredentialModal}
      onLogout={logoutUser}
    />
  </main>
{:else}
  <HomePage
    routeKind={route.kind}
    {currentUser}
    {userLoading}
    preloadedUsername={storedUsername}
    {userError}
    {userSuccess}
    {userLibrary}
    {connectionBusy}
    onLogout={logoutUser}
    onShowQr={handleShowMyQr}
    onOpenScanner={openScanner}
  />
{/if}

{#if credentialModalOpen}
  <CredentialModal
    title={credentialModalTitle}
    qrDataUrl={credentialQrDataUrl}
    link={credentialLink}
    expiresAt={credentialExpiresAt}
    onClose={() => (credentialModalOpen = false)}
  />
{/if}

{#if scannerOpen}
  <ScannerModal
    error={scannerError}
    onClose={closeScanner}
    bind:videoEl={scannerVideo}
  />
{/if}
