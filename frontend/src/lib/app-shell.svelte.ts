import { browser } from "$app/environment";
import { goto } from "$app/navigation";
import type {
    LoginLinkResponse,
    UserLibraryEnsembleResponse as UserLibraryEnsemble,
    UserResponse as AppUser,
} from "$backend/models";
import {
    authenticatedApiClient,
    clearAuth,
    initAuth,
    isPageUnloading,
    publicApiClient,
    setOnSessionExpired,
    setOnTokenRefreshed,
} from "$lib/auth-client";
import {
    showAccountModal,
    showAppConfigModal,
    showCredentialModal,
    showScannerModal,
} from "$components/modals";
import type { AnnotationDefaultPlacement } from "$components/modals/modalHelpers";
import { parseJwtSub } from "$lib/utils";

const ANNOTATION_DEFAULT_PLACEMENT_KEY =
    "app-annotation-default-placement";

class AppShellState {
    refreshToken = $state("");
    currentUser = $state<AppUser | null>(null);
    userSessionExpiresAt = $state<string | null>(null);
    userLoading = $state(false);
    userError = $state("");
    userSuccess = $state("");
    userLibrary = $state<UserLibraryEnsemble[]>([]);
    connectionBusy = $state(false);
    enableCountIn = $state(false);
    countInMeasures = $state(1);
    annotationDefaultPlacement = $state<AnnotationDefaultPlacement>("below");
    preloadedUsername = "";

    private mounted = false;
    private cleanup: (() => void) | null = null;

    constructor() {
        if (!browser) {
            return;
        }

        const storedRefreshToken = (
            window.localStorage.getItem("refresh-token") ?? ""
        ).replace(/^undefined$/, "");
        const storedAccessToken =
            window.localStorage.getItem("access-token") ?? "";
        const storedCountInEnabled =
            window.localStorage.getItem("app-enable-count-in") === "true";
        const storedCountInMeasures = Number(
            window.localStorage.getItem("app-count-in-measures") ?? "1",
        );
        const storedAnnotationDefaultPlacement =
            window.localStorage.getItem(
                ANNOTATION_DEFAULT_PLACEMENT_KEY,
            ) === "above"
                ? "above"
                : "below";

        this.preloadedUsername = parseJwtSub(storedAccessToken);
        this.refreshToken = storedRefreshToken;
        this.userLoading = !!storedRefreshToken;
        this.enableCountIn = storedCountInEnabled;
        this.countInMeasures =
            Number.isFinite(storedCountInMeasures) &&
            storedCountInMeasures > 0
                ? Math.floor(storedCountInMeasures)
                : 1;
        this.annotationDefaultPlacement = storedAnnotationDefaultPlacement;

        if (storedRefreshToken) {
            initAuth(storedRefreshToken, storedAccessToken);
        }
    }

    mount() {
        if (!browser || this.mounted) {
            return this.cleanup ?? (() => {});
        }

        this.mounted = true;

        const syncViewportHeight = () => {
            const viewportHeight =
                window.visualViewport?.height ?? window.innerHeight;
            document.documentElement.style.setProperty(
                "--app-height",
                `${viewportHeight}px`,
            );
        };

        window.addEventListener("resize", syncViewportHeight);
        window.visualViewport?.addEventListener("resize", syncViewportHeight);
        syncViewportHeight();

        setOnSessionExpired(() => {
            this.clearUserSession();
            this.userError = "Your session has expired. Please sign in again.";
        });

        setOnTokenRefreshed((accessToken) => {
            window.localStorage.setItem("access-token", accessToken);
        });

        if (this.refreshToken) {
            void this.loadCurrentUser();
        }

        this.cleanup = () => {
            window.removeEventListener("resize", syncViewportHeight);
            window.visualViewport?.removeEventListener(
                "resize",
                syncViewportHeight,
            );
            this.mounted = false;
            this.cleanup = null;
        };

        return this.cleanup;
    }

    handleMyAccount() {
        if (!this.currentUser) return;

        showAccountModal({
            currentUser: this.currentUser,
            onSaved: (user) => {
                this.currentUser = user;
            },
        });
    }

    handleAppConfig() {
        showAppConfigModal({
            enableCountIn: this.enableCountIn,
            countInMeasures: this.countInMeasures,
            annotationDefaultPlacement: this.annotationDefaultPlacement,
            onToggleCountIn: (value) => this.setEnableCountIn(value),
            onChangeCountInMeasures: (value) =>
                this.setCountInMeasures(value),
            onChangeAnnotationDefaultPlacement: (value) =>
                this.setAnnotationDefaultPlacement(value),
        });
    }

    setEnableCountIn(value: boolean) {
        this.enableCountIn = value;
        if (browser) {
            window.localStorage.setItem("app-enable-count-in", String(value));
        }
    }

    setCountInMeasures(value: number) {
        const normalized = Math.max(1, Math.floor(value || 1));
        this.countInMeasures = normalized;
        if (browser) {
            window.localStorage.setItem(
                "app-count-in-measures",
                String(normalized),
            );
        }
    }

    setAnnotationDefaultPlacement(value: AnnotationDefaultPlacement) {
        this.annotationDefaultPlacement = value;
        if (browser) {
            window.localStorage.setItem(
                ANNOTATION_DEFAULT_PLACEMENT_KEY,
                value,
            );
        }
    }

    async loadCurrentUser() {
        if (!this.refreshToken) {
            this.clearUserSession();
            return;
        }

        this.userLoading = true;

        try {
            const response = await authenticatedApiClient.currentUser();
            let library = await authenticatedApiClient.currentUserLibrary();

            if (
                response.user.role === "admin" ||
                response.user.role === "superadmin"
            ) {
                const adminEnsembles =
                    await authenticatedApiClient.adminListEnsembles();
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

            this.currentUser = response.user;
            this.userSessionExpiresAt = response.session_expires_at ?? null;
            this.userLibrary = library.ensembles;
            this.userError = "";
        } catch (error) {
            if (isPageUnloading()) return;
            if (error instanceof Error && error.name === "AbortError") return;

            this.userError =
                error instanceof Error
                    ? error.message
                    : "Unable to restore your session";
        } finally {
            this.userLoading = false;
        }
    }

    clearUserState() {
        this.refreshToken = "";
        this.currentUser = null;
        this.userSessionExpiresAt = null;
        this.userLibrary = [];
        clearAuth();
    }

    clearStoredSession() {
        if (!browser) return;
        window.localStorage.removeItem("refresh-token");
        window.localStorage.removeItem("access-token");
    }

    clearUserSession() {
        this.clearUserState();
        this.clearStoredSession();
    }

    persistUserSession(
        newRefreshToken: string,
        newAccessToken: string,
        user: AppUser,
        expiresAt: string | null,
    ) {
        if (!newRefreshToken) {
            this.userError =
                "Sign-in failed: server did not return a session token.";
            return;
        }

        initAuth(newRefreshToken, newAccessToken);
        this.refreshToken = newRefreshToken;
        this.currentUser = user;
        this.userSessionExpiresAt = expiresAt;
        this.userLoading = false;

        if (browser) {
            window.localStorage.setItem("refresh-token", newRefreshToken);
            window.localStorage.setItem("access-token", newAccessToken);
        }
    }

    async completeConnectionFromToken(token: string, fromRoute = false) {
        this.userLoading = true;
        this.connectionBusy = true;
        this.userError = "";
        this.userSuccess = "";

        try {
            const response = await publicApiClient.exchangeLoginToken({ token });
            this.persistUserSession(
                response.refresh_token,
                response.access_token,
                response.user,
                response.access_token_expires_at,
            );

        } catch (error) {
            this.userError =
                error instanceof Error
                    ? error.message
                    : "Unable to use this connection link";
        } finally {
            this.connectionBusy = false;
            this.userLoading = false;

            if (fromRoute && browser) {
                await goto("/", { replaceState: true });
            }
        }
    }

    async handleShowMyQr() {
        if (!this.refreshToken || !this.currentUser) {
            this.userError = "Sign in first.";
            return;
        }

        this.userError = "";
        this.userSuccess = "";

        try {
            showCredentialModal({
                title: `QR code for ${this.currentUser.username}`,
                loadLink: () => authenticatedApiClient.createMyLoginLink(),
            });
            this.userSuccess = "QR code ready.";
        } catch (error) {
            this.userError =
                error instanceof Error
                    ? error.message
                    : "Unable to create QR code";
        }
    }

    async logoutUser() {
        try {
            await authenticatedApiClient.meLogout();
        } catch {
            // Best-effort: clear locally regardless.
        }

        this.clearUserSession();
        this.userSuccess = "";
        this.userError = "";
    }

    async openCredentialModal(
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

    openScanner() {
        showScannerModal({
            onConnectToken: (token) => this.completeConnectionFromToken(token),
        });
    }
}

export const appShell = new AppShellState();
