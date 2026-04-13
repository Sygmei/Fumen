<script lang="ts">
  import { onMount } from "svelte";
  import {
    authenticatedApiClient,
    clearAuth,
    initAuth,
    isPageUnloading,
    publicApiClient,
    setOnSessionExpired,
    setOnTokenRefreshed,
  } from "./lib/auth-client";
  import ModalStore from "./components/modals/ModalStore.svelte";
  import {
    showAccountModal,
    showAppConfigModal,
    showCredentialModal,
    showScannerModal,
  } from "./components/modals";
  import type {
    LoginLinkResponse,
    UserLibraryEnsembleResponse as UserLibraryEnsemble,
    UserResponse as AppUser,
  } from "./adapters/fumen-backend/src/models";
  import ListenPage from "./pages/ListenPage.svelte";
  import AdminPage from "./pages/AdminPage.svelte";
  import HomePage from "./pages/HomePage.svelte";
  import { parseJwtSub } from "./lib/utils";

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
  const storedCountInEnabled =
    typeof window !== "undefined"
      ? window.localStorage.getItem("app-enable-count-in") === "true"
      : false;
  const storedCountInMeasures =
    typeof window !== "undefined"
      ? Number(window.localStorage.getItem("app-count-in-measures") ?? "1")
      : 1;
  let enableCountIn = $state(storedCountInEnabled);
  let countInMeasures = $state(
    Number.isFinite(storedCountInMeasures) && storedCountInMeasures > 0
      ? Math.floor(storedCountInMeasures)
      : 1,
  );

  function handleMyAccount() {
    if (!currentUser) return;
    showAccountModal({
      currentUser,
      onSaved: (user) => {
        currentUser = user;
      },
    });
  }

  function handleAppConfig() {
    showAppConfigModal({
      enableCountIn,
      countInMeasures,
      onToggleCountIn: setEnableCountIn,
      onChangeCountInMeasures: setCountInMeasures,
    });
  }

  function setEnableCountIn(value: boolean) {
    enableCountIn = value;
    if (typeof window !== "undefined") {
      window.localStorage.setItem("app-enable-count-in", String(value));
    }
  }

  function setCountInMeasures(value: number) {
    const normalized = Math.max(1, Math.floor(value || 1));
    countInMeasures = normalized;
    if (typeof window !== "undefined") {
      window.localStorage.setItem("app-count-in-measures", String(normalized));
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
      const response = await authenticatedApiClient.currentUser();
      let library = await authenticatedApiClient.currentUserLibrary();
      if (
        response.user.role === "admin" ||
        response.user.role === "superadmin"
      ) {
        const adminEnsembles = await authenticatedApiClient.adminListEnsembles();
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
      userSessionExpiresAt = response.session_expires_at ?? null;
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
      const response = await publicApiClient.exchangeLoginToken({ token });
      persistUserSession(
        response.refresh_token,
        response.access_token,
        response.user,
        response.access_token_expires_at,
      );
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

  async function handleShowMyQr() {
    if (!refreshToken || !currentUser) {
      userError = "Sign in first.";
      return;
    }

    userError = "";
    userSuccess = "";

    try {
      showCredentialModal({
        title: `QR code for ${currentUser.username}`,
        loadLink: () => authenticatedApiClient.createMyLoginLink(),
      });
      userSuccess = "QR code ready.";
    } catch (error) {
      userError =
        error instanceof Error ? error.message : "Unable to create QR code";
    }
  }

  async function logoutUser() {
    try {
      await authenticatedApiClient.meLogout();
    } catch {
      // best-effort: clear locally regardless
    }
    clearUserSession();
    userSuccess = "";
    userError = "";
  }

  async function openCredentialModal(
    title: string,
    loadLink: () => Promise<LoginLinkResponse>,
    options?: { eyebrow?: string; linkLabel?: string },
  ) {
    showCredentialModal({
      title,
      loadLink,
      eyebrow: options?.eyebrow,
      linkLabel: options?.linkLabel,
    });
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
    showScannerModal({
      onConnectToken: (token) => completeConnectionFromToken(token),
    });
  }
</script>

{#if route.kind === "public"}
  <ListenPage accessKey={route.accessKey} {enableCountIn} {countInMeasures} />
{:else if route.kind === "admin"}
  <main class="page admin-shell">
  <AdminPage
      {currentUser}
      {userLoading}
      preloadedUsername={storedUsername}
      onShowQr={handleShowMyQr}
      onShowCredential={openCredentialModal}
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

<ModalStore />
