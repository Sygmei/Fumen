<script lang="ts">
    import {
        createAdminUserLoginLink,
        listEnsembles,
        listMusics,
        listUsers,
        type AdminMusic,
        type AppUser,
        type Ensemble,
        type LoginLinkResponse,
    } from "../lib/api";
    import AdminLayout from "../components/AdminLayout.svelte";
    import AdminUsersSection from "./AdminUsersSection.svelte";
    import AdminEnsemblesSection from "./AdminEnsemblesSection.svelte";
    import AdminScoresSection from "./AdminScoresSection.svelte";
    import { canAccessAdmin, canUseUsersSection } from "../lib/admin-permissions";

    function readAdminCache<T>(key: string): T[] {
        try {
            const raw = localStorage.getItem(key);
            return raw ? (JSON.parse(raw) as T[]) : [];
        } catch {
            return [];
        }
    }

    const cachedMusics = readAdminCache<AdminMusic>("cached-admin-musics");
    const cachedAdminUsers = readAdminCache<AppUser>("cached-admin-users");
    const cachedEnsembles = readAdminCache<Ensemble>("cached-admin-ensembles");

    const {
        currentUser,
        userLoading,
        userError,
        preloadedUsername,
        onShowQr,
        onShowCredential,
        onLogout,
        onMyAccount,
    }: {
        currentUser: AppUser | null;
        userLoading: boolean;
        userError: string;
        preloadedUsername: string;
        onShowQr: () => Promise<void>;
        onShowCredential: (
            title: string,
            loadLink: () => Promise<LoginLinkResponse>,
            options?: { eyebrow?: string; linkLabel?: string },
        ) => Promise<void>;
        onLogout: () => Promise<void>;
        onMyAccount?: () => void;
    } = $props();

    type AdminSection = "users" | "ensembles" | "scores";

    const adminSectionItems: Array<{
        id: AdminSection;
        label: string;
        eyebrow: string;
    }> = [
        { id: "users", label: "Users", eyebrow: "Accounts" },
        { id: "ensembles", label: "Ensembles", eyebrow: "Groups" },
        { id: "scores", label: "Scores", eyebrow: "Library" },
    ];

    let adminSection = $state<AdminSection>("users");
    let adminLoading = $state(false);
    let adminError = $state("");
    let adminSuccess = $state("");
    let musics = $state<AdminMusic[]>(cachedMusics);
    let adminUsers = $state<AppUser[]>(cachedAdminUsers);
    let ensembles = $state<Ensemble[]>(cachedEnsembles);

    const visibleAdminSectionItems = $derived.by(() =>
        adminSectionItems.filter((section) => {
            if (section.id === "users") return canUseUsersSection(currentUser);
            return canAccessAdmin(currentUser);
        }),
    );

    $effect(() => {
        if (!visibleAdminSectionItems.some((s) => s.id === adminSection)) {
            adminSection = visibleAdminSectionItems[0]?.id ?? "scores";
        }
    });

    $effect(() => {
        if (currentUser && canAccessAdmin(currentUser)) {
            void refreshAdminData();
        }
    });

    async function refreshAdminData() {
        adminLoading = true;
        adminError = "";
        try {
            const [musicItems, userItems, ensembleItems] = await Promise.all([
                listMusics(),
                listUsers(),
                listEnsembles(),
            ]);
            musics = musicItems;
            adminUsers = userItems;
            ensembles = ensembleItems;
            localStorage.setItem(
                "cached-admin-musics",
                JSON.stringify(musicItems),
            );
            localStorage.setItem(
                "cached-admin-users",
                JSON.stringify(userItems),
            );
            localStorage.setItem(
                "cached-admin-ensembles",
                JSON.stringify(ensembleItems),
            );
        } catch (error) {
            adminError =
                error instanceof Error
                    ? error.message
                    : "Unable to load admin data";
        } finally {
            adminLoading = false;
        }
    }

    async function handleShowUserQr(user: AppUser) {
        await onShowCredential(
            `QR code for ${user.username}`,
            () => createAdminUserLoginLink(user.id),
        );
    }

    async function handleShowScoreQr(music: AdminMusic) {
        const url = music.public_id_url ?? music.public_url;
        await onShowCredential(
            `Share link for ${music.title}`,
            () => Promise.resolve({ connection_url: url, expires_at: "" }),
            { eyebrow: "Share score", linkLabel: "Share link" },
        );
    }

    function cacheAdminUsers(users: AppUser[]) {
        localStorage.setItem("cached-admin-users", JSON.stringify(users));
    }
</script>

<AdminLayout
    {currentUser}
    {userLoading}
    {userError}
    {preloadedUsername}
    {adminLoading}
    {adminError}
    {adminSuccess}
    {adminSection}
    visibleSectionItems={visibleAdminSectionItems}
    sectionCounts={{ users: adminUsers.length, ensembles: ensembles.length, scores: musics.length }}
    {onShowQr}
    onLogout={() => void onLogout()}
    {onMyAccount}
    onSectionChange={(section) => (adminSection = section)}
>
    {#snippet children()}
        {#if adminSection === "users"}
            <AdminUsersSection
                currentUser={currentUser!}
                users={adminUsers}
                onUserCreated={(user) => {
                    adminUsers = [...adminUsers, user].sort((a, b) =>
                        a.username.localeCompare(b.username),
                    );
                    cacheAdminUsers(adminUsers);
                }}
                onUserUpdated={(user) => {
                    adminUsers = adminUsers.map((u) =>
                        u.id === user.id ? user : u,
                    );
                    cacheAdminUsers(adminUsers);
                }}
                onRefresh={refreshAdminData}
                onShowQr={handleShowUserQr}
                onSuccess={(msg) => {
                    adminSuccess = msg;
                    adminError = "";
                }}
                onError={(msg) => {
                    adminError = msg;
                    adminSuccess = "";
                }}
            />
        {:else if adminSection === "ensembles"}
            <AdminEnsemblesSection
                currentUser={currentUser!}
                {ensembles}
                allUsers={adminUsers}
                allMusics={musics}
                onEnsembleCreated={(ensemble) => {
                    ensembles = [...ensembles, ensemble].sort((a, b) =>
                        a.name.localeCompare(b.name),
                    );
                }}
                onRefresh={refreshAdminData}
                onSuccess={(msg) => {
                    adminSuccess = msg;
                    adminError = "";
                }}
                onError={(msg) => {
                    adminError = msg;
                    adminSuccess = "";
                }}
            />
        {:else if adminSection === "scores"}
            <AdminScoresSection
                currentUser={currentUser!}
                {musics}
                {ensembles}
                onMusicUpdated={(music) => {
                    musics = musics.map((m) => (m.id === music.id ? music : m));
                }}
                onRefresh={refreshAdminData}
                onShowQr={handleShowScoreQr}
                onSuccess={(msg) => {
                    adminSuccess = msg;
                    adminError = "";
                }}
                onError={(msg) => {
                    adminError = msg;
                    adminSuccess = "";
                }}
            />
        {/if}
    {/snippet}
</AdminLayout>
