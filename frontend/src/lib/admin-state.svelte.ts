import { getContext, setContext } from "svelte";
import type {
    AdminEnsembleResponse as Ensemble,
    AdminMusicResponse as AdminMusic,
    UserResponse as AppUser,
} from "$backend/models";
import { authenticatedApiClient } from "$lib/auth-client";
import {
    canAccessAdmin,
    canUseUsersSection,
} from "$lib/admin-permissions";

export type AdminSection = "users" | "ensembles" | "scores";

export const adminSectionItems: Array<{
    id: AdminSection;
    label: string;
    eyebrow: string;
}> = [
    { id: "users", label: "Users", eyebrow: "Accounts" },
    { id: "ensembles", label: "Ensembles", eyebrow: "Groups" },
    { id: "scores", label: "Scores", eyebrow: "Library" },
];

const adminStateContextKey = Symbol("admin-state");

export function getVisibleAdminSectionItems(user: AppUser | null) {
    return adminSectionItems.filter((section) => {
        if (section.id === "users") {
            return canUseUsersSection(user);
        }

        return canAccessAdmin(user);
    });
}

export function getPreferredAdminSection(user: AppUser | null): AdminSection {
    return getVisibleAdminSectionItems(user)[0]?.id ?? "scores";
}

export function getAdminSectionPath(section: AdminSection) {
    return `/admin/${section}`;
}

export function getAdminSectionFromPath(pathname: string): AdminSection {
    if (pathname.endsWith("/users")) return "users";
    if (pathname.endsWith("/ensembles")) return "ensembles";
    return "scores";
}

export class AdminState {
    adminLoading = $state(false);
    adminError = $state("");
    adminSuccess = $state("");
    musics = $state<AdminMusic[]>([]);
    adminUsers = $state<AppUser[]>([]);
    ensembles = $state<Ensemble[]>([]);
    loadedForUserId = $state("");

    private refreshPromise: Promise<void> | null = null;

    clear() {
        this.adminLoading = false;
        this.adminError = "";
        this.adminSuccess = "";
        this.musics = [];
        this.adminUsers = [];
        this.ensembles = [];
        this.loadedForUserId = "";
    }

    setSuccess(message: string) {
        this.adminSuccess = message;
        this.adminError = "";
    }

    setError(message: string) {
        this.adminError = message;
        this.adminSuccess = "";
    }

    async ensureLoadedFor(userId: string) {
        if (
            this.loadedForUserId === userId &&
            (this.musics.length > 0 ||
                this.adminUsers.length > 0 ||
                this.ensembles.length > 0)
        ) {
            return;
        }

        await this.refresh(userId);
    }

    async refresh(userId = this.loadedForUserId) {
        if (this.refreshPromise) {
            return this.refreshPromise;
        }

        this.refreshPromise = this.doRefresh(userId);

        try {
            await this.refreshPromise;
        } finally {
            this.refreshPromise = null;
        }
    }

    private async doRefresh(userId: string) {
        this.adminLoading = true;
        this.adminError = "";

        try {
            const [musicItems, userItems, ensembleItems] = await Promise.all([
                authenticatedApiClient.adminListMusics(),
                authenticatedApiClient.adminListUsers(),
                authenticatedApiClient.adminListEnsembles(),
            ]);

            this.musics = musicItems;
            this.adminUsers = userItems;
            this.ensembles = ensembleItems;
            this.loadedForUserId = userId;
        } catch (error) {
            this.adminError =
                error instanceof Error
                    ? error.message
                    : "Unable to load admin data";
        } finally {
            this.adminLoading = false;
        }
    }

    addUser(user: AppUser) {
        this.adminUsers = [...this.adminUsers, user].sort((left, right) =>
            left.username.localeCompare(right.username),
        );
    }

    removeUser(userId: string) {
        this.adminUsers = this.adminUsers.filter((item) => item.id !== userId);
    }

    updateUser(user: AppUser) {
        this.adminUsers = this.adminUsers.map((item) =>
            item.id === user.id ? user : item,
        );
    }

    addEnsemble(ensemble: Ensemble) {
        this.ensembles = [...this.ensembles, ensemble].sort((left, right) =>
            left.name.localeCompare(right.name),
        );
    }

    removeEnsemble(ensembleId: string) {
        this.ensembles = this.ensembles.filter((item) => item.id !== ensembleId);
    }

    updateEnsemble(ensemble: Ensemble) {
        this.ensembles = this.ensembles
            .map((item) => (item.id === ensemble.id ? ensemble : item))
            .sort((left, right) => left.name.localeCompare(right.name));
    }

    updateMusic(music: AdminMusic) {
        this.musics = [...this.musics.filter((item) => item.id !== music.id), music]
            .sort((left, right) => right.created_at.localeCompare(left.created_at));
    }

    removeMusic(musicId: string) {
        this.musics = this.musics.filter((item) => item.id !== musicId);
    }
}

export function setAdminStateContext(state: AdminState) {
    setContext(adminStateContextKey, state);
    return state;
}

export function getAdminStateContext() {
    return getContext<AdminState>(adminStateContextKey);
}
