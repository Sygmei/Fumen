<script lang="ts">
    import type {
        AdminEnsembleResponse as Ensemble,
        AdminMusicResponse as AdminMusic,
        LoginLinkResponse,
        UserResponse as AppUser,
    } from "../adapters/fumen-backend/src/models";
    import { authenticatedApiClient } from "../lib/auth-client";
    import AdminLayout from "../components/AdminLayout.svelte";
    import AdminUsersSection from "./AdminUsersSection.svelte";
    import AdminEnsemblesSection from "./AdminEnsemblesSection.svelte";
    import AdminScoresSection from "./AdminScoresSection.svelte";
    import { canAccessAdmin, canUseUsersSection } from "../lib/admin-permissions";

    const {
        currentUser,
        userLoading,
        preloadedUsername,
        onShowQr,
        onShowCredential,
        onLogout,
        onMyAccount,
        onAppConfig,
    }: {
        currentUser: AppUser | null;
        userLoading: boolean;
        preloadedUsername: string;
        onShowQr: () => Promise<void>;
        onShowCredential: (
            title: string,
            loadLink: () => Promise<LoginLinkResponse>,
            options?: { eyebrow?: string; linkLabel?: string },
        ) => Promise<void>;
        onLogout: () => Promise<void>;
        onMyAccount?: () => void;
        onAppConfig?: () => void;
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
    let musics = $state<AdminMusic[]>([]);
    let adminUsers = $state<AppUser[]>([]);
    let ensembles = $state<Ensemble[]>([]);

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

    $effect(() => {
        const shouldPollForProcessing =
            adminSection === "scores" &&
            musics.some((music) =>
                [
                    music.audio_status,
                    music.midi_status,
                    music.musicxml_status,
                    music.stems_status,
                ].includes("processing"),
            );

        if (!shouldPollForProcessing) {
            return;
        }

        const timer = window.setInterval(() => {
            if (adminSection === "scores") {
                void refreshAdminData();
            }
        }, 5000);

        return () => window.clearInterval(timer);
    });

    async function refreshAdminData() {
        adminLoading = true;
        adminError = "";
        try {
            const [musicItems, userItems, ensembleItems] = await Promise.all([
                authenticatedApiClient.adminListMusics(),
                authenticatedApiClient.adminListUsers(),
                authenticatedApiClient.adminListEnsembles(),
            ]);
            musics = musicItems;
            adminUsers = userItems;
            ensembles = ensembleItems;
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
            () => authenticatedApiClient.adminCreateUserLoginLink(user.id),
        );
    }

    async function handleShowScoreQr(music: AdminMusic) {
        const url = music.public_url;
        await onShowCredential(
            `Share link for ${music.title}`,
            () => Promise.resolve({ connection_url: url, expires_at: "" }),
            { eyebrow: "Share score", linkLabel: "Share link" },
        );
    }

</script>

<AdminLayout
    {currentUser}
    {userLoading}
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
    {onAppConfig}
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
                }}
                onUserUpdated={(user) => {
                    adminUsers = adminUsers.map((u) =>
                        u.id === user.id ? user : u,
                    );
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
                    musics = [...musics.filter((m) => m.id !== music.id), music].sort(
                        (a, b) => b.created_at.localeCompare(a.created_at),
                    );
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
