<script lang="ts">
    import {
        addMusicToEnsemble,
        addUserToEnsemble,
        createAdminUserLoginLink,
        createEnsemble,
        createUser,
        deleteEnsemble,
        deleteMusic,
        deleteUser,
        listEnsembles,
        listMusics,
        listUsers,
        removeMusicFromEnsemble,
        removeUserFromEnsemble,
        retryRender,
        STEM_QUALITY_PROFILES,
        updateMusicMetadata,
        uploadMusic,
        type AdminMusic,
        type AppUser,
        type Ensemble,
        type EnsembleRole,
        type GlobalRole,
        type LoginLinkResponse,
        type StemQualityProfile,
    } from "../lib/api";
    import ScoreIcon from "../components/ScoreIcon.svelte";
    import CustomSelect from "../components/CustomSelect.svelte";
    import BaseModal from "../components/BaseModal.svelte";
    import ConfirmModal from "../components/ConfirmModal.svelte";
    import { formatBytes, prettyDate, qualityProfileLabel } from "../lib/utils";
    import TopBar from "../components/TopBar.svelte";
    import { Search, Plus, QrCode, Trash2, UserPlus, Music, Users, Pencil, Info, Download, ChevronDown } from '@lucide/svelte';

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
        onShowCredential,
        onLogout,
    }: {
        currentUser: AppUser | null;
        userLoading: boolean;
        userError: string;
        preloadedUsername: string;
        onShowCredential: (
            title: string,
            loadLink: () => Promise<LoginLinkResponse>,
        ) => Promise<void>;
        onLogout: () => Promise<void>;
    } = $props();

    type AdminSection = "users" | "ensembles" | "scores";
    type ManagedMemberDraftRole = "none" | EnsembleRole;

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
    let uploadTitle = $state("");
    let selectedIconFile = $state<File | null>(null);
    let uploadPublicId = $state("");
    let uploadQualityProfile = $state<StemQualityProfile>("standard");
    let selectedFile = $state<File | null>(null);
    let uploadBusy = $state(false);
    let musics = $state<AdminMusic[]>(cachedMusics);
    let adminUsers = $state<AppUser[]>(cachedAdminUsers);
    let ensembles = $state<Ensemble[]>(cachedEnsembles);
    let newUsername = $state("");
    let newUserRole = $state<Exclude<GlobalRole, "superadmin">>("user");
    let userSearchQuery = $state("");
    let showCreateUserModal = $state(false);
    let creatingUser = $state(false);
    let deletingUserFor = $state("");
    let ensembleSearchQuery = $state("");
    let newEnsembleName = $state("");
    let showCreateEnsembleModal = $state(false);
    let creatingEnsemble = $state(false);
    let deletingEnsembleFor = $state("");
    let scoreSearchQuery = $state("");
    let showCreateScoreModal = $state(false);
    let uploadEnsembleIds = $state<string[]>(
        cachedEnsembles[0] ? [cachedEnsembles[0].id] : [],
    );
    let managingEnsembleId = $state("");
    let managingEnsembleScoresId = $state("");
    let currentMemberSearchQuery = $state("");
    let addMemberSearchQuery = $state("");
    let currentEnsembleScoreSearchQuery = $state("");
    let addEnsembleScoreSearchQuery = $state("");
    let inviteRoles = $state<Record<string, EnsembleRole>>({});
    let originalManagedMemberRoles = $state<
        Record<string, ManagedMemberDraftRole>
    >({});
    let managedMemberDraftRoles = $state<
        Record<string, ManagedMemberDraftRole>
    >({});
    let savingManagedMembers = $state(false);
    let originalEnsembleScoreMusicIds = $state<string[]>([]);
    let stagedEnsembleScoreMusicIds = $state<string[]>([]);
    let savingEnsembleScores = $state(false);
    let ensemblePickerMode = $state<"upload" | "score" | "">("");
    let ensemblePickerMusicId = $state("");
    let ensemblePickerSearchQuery = $state("");
    let stagedScoreEnsembleIds = $state<string[]>([]);
    let savingScoreEnsembles = $state(false);
    let metadataMusicId = $state("");
    let metadataTitle = $state("");
    let metadataIcon = $state("");
    let metadataPublicId = $state("");
    let savingMetadataFor = $state("");
    let infoMusicId = $state("");
    let retryingFor = $state("");
    let deletingMusicFor = $state("");
    let openDownloadMenuFor = $state("");
    let confirmMessage = $state("");
    let confirmLabel = $state("Delete");
    let confirmAction = $state<(() => Promise<void>) | null>(null);
    let confirmBusy = $state(false);

    const stemQualityOptions = STEM_QUALITY_PROFILES.map((option) => ({
        value: option.value,
        label: option.label,
        description: option.description,
    }));

    function canAccessAdmin(user = currentUser) {
        return !!user && user.role !== "user";
    }

    function isSuperadmin(user = currentUser) {
        return user?.role === "superadmin";
    }

    function hasGlobalPower(user = currentUser) {
        return user?.role === "superadmin" || user?.role === "admin";
    }

    function canManageUsers(user = currentUser) {
        return (
            user?.role === "superadmin" ||
            user?.role === "admin" ||
            user?.role === "manager"
        );
    }

    function canCreateEnsembles(user = currentUser) {
        return canManageUsers(user);
    }

    function canManageEnsembleMembers(ensemble: Ensemble, user = currentUser) {
        return (
            !!user &&
            (hasGlobalPower(user) ||
                user.managed_ensemble_ids.includes(ensemble.id))
        );
    }

    function canDeleteUserAccount(user: AppUser, actor = currentUser) {
        if (!actor || user.id === actor.id) {
            return false;
        }
        if (actor.role === "superadmin") {
            return user.role !== "superadmin";
        }
        if (actor.role === "admin") {
            return user.role !== "admin" && user.role !== "superadmin";
        }
        if (actor.role === "manager") {
            return user.role === "user" && user.created_by_user_id === actor.id;
        }
        return false;
    }

    function canDeleteEnsembleRecord(ensemble: Ensemble, actor = currentUser) {
        if (!actor) {
            return false;
        }
        return (
            hasGlobalPower(actor) ||
            ensemble.created_by_user_id === actor.id ||
            actor.managed_ensemble_ids.includes(ensemble.id)
        );
    }

    function canDeleteScore(music: AdminMusic, actor = currentUser) {
        if (!actor) {
            return false;
        }
        return hasGlobalPower(actor) || music.owner_user_id === actor.id;
    }

    function canEditOwnedScore(music: AdminMusic, actor = currentUser) {
        return canDeleteScore(music, actor);
    }

    function canManageScoreEnsembles(music: AdminMusic, actor = currentUser) {
        if (!actor) {
            return false;
        }
        return (
            hasGlobalPower(actor) ||
            actor.role === "manager" ||
            music.owner_user_id === actor.id
        );
    }

    function canManageEnsembleScores(ensemble: Ensemble, actor = currentUser) {
        if (!actor) {
            return false;
        }
        return (
            hasGlobalPower(actor) ||
            actor.role === "manager" ||
            actor.managed_ensemble_ids.includes(ensemble.id) ||
            actor.editable_ensemble_ids.includes(ensemble.id)
        );
    }

    function canUseUsersSection(user = currentUser) {
        return canManageUsers(user);
    }

    function allowedCreateRoles(
        actor = currentUser,
    ): Array<Exclude<GlobalRole, "superadmin">> {
        if (!actor) {
            return ["user"];
        }
        if (actor.role === "superadmin" || actor.role === "admin") {
            return ["admin", "manager", "editor", "user"];
        }
        if (actor.role === "manager") {
            return ["user"];
        }
        return ["user"];
    }

    function defaultCreateRole(
        actor = currentUser,
    ): Exclude<GlobalRole, "superadmin"> {
        const roles = allowedCreateRoles(actor);
        return roles.includes("user") ? "user" : (roles[0] ?? "user");
    }

    function createRoleLabel(role: Exclude<GlobalRole, "superadmin">) {
        if (role === "admin") {
            return "Admin";
        }
        if (role === "manager") {
            return "Manager";
        }
        if (role === "editor") {
            return "Editor";
        }
        return "User";
    }

    function createRoleIcon(role: Exclude<GlobalRole, "superadmin">) {
        if (role === "admin") {
            return "ðŸ›¡";
        }
        if (role === "manager") {
            return "ðŸ‘¥";
        }
        if (role === "editor") {
            return "âœŽ";
        }
        return "â€¢";
    }

    let createUserRoleOptions = $derived.by(() =>
        allowedCreateRoles().map((role) => ({
            value: role,
            label: createRoleLabel(role),
            description: createRoleDescription(role),
            icon: createRoleIcon(role),
            tone: role,
        })),
    );

    function createRoleDescription(role: Exclude<GlobalRole, "superadmin">) {
        if (role === "admin") {
            return "Full access, except removing other admins.";
        }
        if (role === "manager") {
            return "Can create ensembles and users, and manage assigned ensembles.";
        }
        if (role === "editor") {
            return "Standard access plus score uploads on assigned ensembles.";
        }
        return "Can sign in and listen to scores they can access.";
    }

    function ensembleRoleIcon(role: EnsembleRole) {
        if (role === "manager") {
            return "ðŸ‘¥";
        }
        if (role === "editor") {
            return "âœŽ";
        }
        return "â€¢";
    }

    function ensembleRoleDescription(role: EnsembleRole) {
        if (role === "manager") {
            return "Can manage ensemble members and score access.";
        }
        if (role === "editor") {
            return "Can add scores and manage only their own uploads.";
        }
        return "Can access scores shared with the ensemble.";
    }

    function ensembleRoleOptionsForUser(user: AppUser) {
        return allowedEnsembleRolesForUser(user).map((role) => ({
            value: role,
            label: memberRoleLabel(role),
            description: ensembleRoleDescription(role),
            icon: ensembleRoleIcon(role),
            tone: role,
        }));
    }

    function allowedEnsembleRolesForUser(user: AppUser): EnsembleRole[] {
        if (user.role === "superadmin" || user.role === "admin") {
            return [];
        }
        if (user.role === "manager") {
            return currentUser?.role === "manager"
                ? []
                : ["manager", "editor", "user"];
        }
        if (user.role === "editor") {
            return ["editor", "user"];
        }
        return ["user"];
    }

    const visibleAdminSectionItems = $derived.by(() =>
        adminSectionItems.filter((section) => {
            if (section.id === "users") {
                return canUseUsersSection();
            }
            return canAccessAdmin();
        }),
    );

    function currentAdminSectionItem() {
        return (
            visibleAdminSectionItems.find(
                (section) => section.id === adminSection,
            ) ??
            visibleAdminSectionItems[0] ??
            adminSectionItems[0]
        );
    }

    function mobileAdminSectionItems() {
        return visibleAdminSectionItems.map((section) => ({
            id: section.id,
            label: section.label,
            eyebrow: section.eyebrow,
            meta:
                section.id === "users"
                    ? `${adminUsers.length} total`
                    : section.id === "ensembles"
                      ? `${ensembles.length} groups`
                      : `${musics.length} scores`,
        }));
    }

    function normalizeQuery(value: string) {
        return value.trim().toLowerCase();
    }

    let filteredAdminUsers = $derived.by(() => {
        const query = normalizeQuery(userSearchQuery);
        const sorted = [...adminUsers].sort((left, right) =>
            left.username.localeCompare(right.username),
        );

        if (!query) {
            return sorted;
        }

        return sorted.filter((user) =>
            [user.username, user.role, prettyDate(user.created_at)].some(
                (value) => value.toLowerCase().includes(query),
            ),
        );
    });

    let filteredEnsembles = $derived.by(() => {
        const query = normalizeQuery(ensembleSearchQuery);
        const sorted = [...ensembles].sort((left, right) =>
            left.name.localeCompare(right.name),
        );

        if (!query) {
            return sorted;
        }

        return sorted.filter((ensemble) =>
            [
                ensemble.name,
                `${ensemble.members.length} members`,
                `${ensemble.score_count} scores`,
            ].some((value) => value.toLowerCase().includes(query)),
        );
    });

    let filteredMusics = $derived.by(() => {
        const query = normalizeQuery(scoreSearchQuery);
        const sorted = [...musics].sort((left, right) =>
            left.title.localeCompare(right.title),
        );

        if (!query) {
            return sorted;
        }

        return sorted.filter((music) =>
            [
                music.title,
                music.filename,
                music.public_id ?? "",
                ...music.ensemble_names,
                qualityProfileLabel(music.quality_profile),
            ].some((value) => value.toLowerCase().includes(query)),
        );
    });

    let activeManagedEnsemble = $derived.by(
        () =>
            ensembles.find((ensemble) => ensemble.id === managingEnsembleId) ??
            null,
    );

    let activeManagedScoreEnsemble = $derived.by(
        () =>
            ensembles.find(
                (ensemble) => ensemble.id === managingEnsembleScoresId,
            ) ?? null,
    );

    function managedMemberRoleForUser(userId: string): ManagedMemberDraftRole {
        return managedMemberDraftRoles[userId] ?? "none";
    }

    let filteredManagedMembers = $derived.by(() => {
        const ensemble = activeManagedEnsemble;
        if (!ensemble) {
            return [];
        }

        const query = normalizeQuery(currentMemberSearchQuery);

        return adminUsers
            .map((user) => ({
                user_id: user.id,
                role: managedMemberRoleForUser(user.id),
                user,
            }))
            .filter(
                (
                    entry,
                ): entry is {
                    user_id: string;
                    role: EnsembleRole;
                    user: AppUser;
                } => entry.role !== "none",
            )
            .sort((left, right) =>
                left.user!.username.localeCompare(right.user!.username),
            )
            .filter((entry) =>
                !query
                    ? true
                    : [entry.user!.username, entry.role]
                          .join(" ")
                          .toLowerCase()
                          .includes(query),
            );
    });

    let filteredAvailableEnsembleUsers = $derived.by(() => {
        const ensemble = activeManagedEnsemble;
        if (!ensemble) {
            return [];
        }

        const query = normalizeQuery(addMemberSearchQuery);

        return [...adminUsers]
            .filter(
                (user) =>
                    managedMemberRoleForUser(user.id) === "none" &&
                    allowedEnsembleRolesForUser(user).length > 0,
            )
            .sort((left, right) => left.username.localeCompare(right.username))
            .filter((user) =>
                !query
                    ? true
                    : [user.username, user.role]
                          .join(" ")
                          .toLowerCase()
                          .includes(query),
            );
    });

    let filteredManagedEnsembleScores = $derived.by(() => {
        if (!activeManagedScoreEnsemble) {
            return [];
        }

        const query = normalizeQuery(currentEnsembleScoreSearchQuery);

        return [...musics]
            .filter((music) => stagedEnsembleScoreMusicIds.includes(music.id))
            .sort((left, right) => left.title.localeCompare(right.title))
            .filter((music) =>
                !query
                    ? true
                    : [
                          music.title,
                          music.public_id ?? "",
                          ...music.ensemble_names,
                      ]
                          .join(" ")
                          .toLowerCase()
                          .includes(query),
            );
    });

    let filteredAvailableEnsembleScores = $derived.by(() => {
        if (!activeManagedScoreEnsemble) {
            return [];
        }

        const query = normalizeQuery(addEnsembleScoreSearchQuery);

        return [...musics]
            .filter((music) => !stagedEnsembleScoreMusicIds.includes(music.id))
            .sort((left, right) => left.title.localeCompare(right.title))
            .filter((music) =>
                !query
                    ? true
                    : [
                          music.title,
                          music.public_id ?? "",
                          ...music.ensemble_names,
                      ]
                          .join(" ")
                          .toLowerCase()
                          .includes(query),
            );
    });

    $effect(() => {
        if (
            !visibleAdminSectionItems.some(
                (section) => section.id === adminSection,
            )
        ) {
            adminSection = visibleAdminSectionItems[0]?.id ?? "scores";
        }
    });

    let activeMetadataMusic = $derived.by(
        () => musics.find((music) => music.id === metadataMusicId) ?? null,
    );

    let activeInfoMusic = $derived.by(
        () => musics.find((music) => music.id === infoMusicId) ?? null,
    );

    let activeEnsemblePickerMusic = $derived.by(
        () =>
            musics.find((music) => music.id === ensemblePickerMusicId) ?? null,
    );

    let filteredPickerEnsembles = $derived.by(() => {
        const query = normalizeQuery(ensemblePickerSearchQuery);
        const sorted = [...ensembles].sort((left, right) =>
            left.name.localeCompare(right.name),
        );

        if (!query) {
            return sorted;
        }

        return sorted.filter((ensemble) =>
            ensemble.name.toLowerCase().includes(query),
        );
    });

    $effect(() => {
        if (currentUser && canAccessAdmin()) {
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
            uploadEnsembleIds = uploadEnsembleIds.filter((ensembleId) =>
                ensembleItems.some((ensemble) => ensemble.id === ensembleId),
            );
            if (uploadEnsembleIds.length === 0 && ensembleItems[0]) {
                uploadEnsembleIds = [ensembleItems[0].id];
            }
            if (
                managingEnsembleId &&
                !ensembleItems.some(
                    (ensemble) => ensemble.id === managingEnsembleId,
                )
            ) {
                managingEnsembleId = "";
            }
            if (
                managingEnsembleScoresId &&
                !ensembleItems.some(
                    (ensemble) => ensemble.id === managingEnsembleScoresId,
                )
            ) {
                closeManageScoresModal();
            }
            if (
                metadataMusicId &&
                !musicItems.some((music) => music.id === metadataMusicId)
            ) {
                closeScoreMetadataModal();
            }
            if (
                ensemblePickerMusicId &&
                !musicItems.some((music) => music.id === ensemblePickerMusicId)
            ) {
                closeEnsemblePickerModal();
            }
        } catch (error) {
            adminError =
                error instanceof Error
                    ? error.message
                    : "Unable to load admin data";
        } finally {
            adminLoading = false;
        }
    }

    async function handleCreateUser() {
        const trimmed = newUsername.trim();
        if (!trimmed) {
            adminError = "Choose a username first.";
            return;
        }

        creatingUser = true;
        adminError = "";
        adminSuccess = "";

        try {
            const user = await createUser(trimmed, newUserRole);
            adminUsers = [...adminUsers, user].sort((left, right) =>
                left.username.localeCompare(right.username),
            );
            newUsername = "";
            newUserRole = defaultCreateRole();
            showCreateUserModal = false;
            adminSuccess = `User ${user.username} created.`;
        } catch (error) {
            adminError =
                error instanceof Error
                    ? error.message
                    : "Unable to create user";
        } finally {
            creatingUser = false;
        }
    }

    function openCreateUserModal() {
        adminError = "";
        newUsername = "";
        newUserRole = defaultCreateRole();
        showCreateUserModal = true;
    }

    function closeCreateUserModal() {
        if (creatingUser) {
            return;
        }
        showCreateUserModal = false;
        newUsername = "";
        newUserRole = defaultCreateRole();
    }

    function openCreateEnsembleModal() {
        adminError = "";
        newEnsembleName = "";
        showCreateEnsembleModal = true;
    }

    function closeCreateEnsembleModal() {
        if (creatingEnsemble) {
            return;
        }
        showCreateEnsembleModal = false;
        newEnsembleName = "";
    }

    async function handleCreateEnsemble() {
        const trimmed = newEnsembleName.trim();
        if (!trimmed) {
            adminError = "Choose an ensemble name first.";
            return;
        }

        creatingEnsemble = true;
        adminError = "";
        adminSuccess = "";

        try {
            const ensemble = await createEnsemble(trimmed);
            ensembles = [...ensembles, ensemble].sort((left, right) =>
                left.name.localeCompare(right.name),
            );
            newEnsembleName = "";
            showCreateEnsembleModal = false;
            adminSuccess = `Ensemble ${ensemble.name} created.`;
        } catch (error) {
            adminError =
                error instanceof Error
                    ? error.message
                    : "Unable to create ensemble";
        } finally {
            creatingEnsemble = false;
        }
    }

    function ensembleMemberRole(
        ensemble: Ensemble,
        userId: string,
    ): "none" | EnsembleRole {
        return (
            ensemble.members.find((member) => member.user_id === userId)
                ?.role ?? "none"
        );
    }

    function buildEnsembleMemberRoleMap(ensemble: Ensemble) {
        return Object.fromEntries(
            ensemble.members.map((member) => [member.user_id, member.role]),
        ) as Record<string, ManagedMemberDraftRole>;
    }

    function stageManagedMemberRole(
        userId: string,
        role: ManagedMemberDraftRole,
    ) {
        managedMemberDraftRoles = {
            ...managedMemberDraftRoles,
            [userId]: role,
        };
    }

    function hasManagedMemberChanges() {
        const keys = new Set([
            ...Object.keys(originalManagedMemberRoles),
            ...Object.keys(managedMemberDraftRoles),
        ]);

        for (const userId of keys) {
            if (
                (originalManagedMemberRoles[userId] ?? "none") !==
                (managedMemberDraftRoles[userId] ?? "none")
            ) {
                return true;
            }
        }

        return false;
    }

    async function saveManagedEnsembleChanges() {
        const ensembleId = managingEnsembleId;
        if (!ensembleId || !hasManagedMemberChanges()) {
            return;
        }

        savingManagedMembers = true;
        adminError = "";
        adminSuccess = "";

        try {
            const keys = new Set([
                ...Object.keys(originalManagedMemberRoles),
                ...Object.keys(managedMemberDraftRoles),
            ]);

            for (const userId of keys) {
                const originalRole =
                    originalManagedMemberRoles[userId] ?? "none";
                const nextRole = managedMemberDraftRoles[userId] ?? "none";

                if (originalRole === nextRole) {
                    continue;
                }

                if (nextRole === "none") {
                    await removeUserFromEnsemble(ensembleId, userId);
                } else {
                    await addUserToEnsemble(ensembleId, userId, nextRole);
                }
            }

            await refreshAdminData();
            originalManagedMemberRoles = { ...managedMemberDraftRoles };
            adminSuccess = "Ensemble members updated.";
        } catch (error) {
            adminError =
                error instanceof Error
                    ? error.message
                    : "Unable to update ensemble members";
        } finally {
            savingManagedMembers = false;
        }
    }

    function musicHasEnsemble(music: AdminMusic, ensembleId: string) {
        return music.ensemble_ids.includes(ensembleId);
    }

    function scoreLinkForCopy(music: AdminMusic) {
        return music.public_id_url ?? music.public_url;
    }

    function isUploadEnsembleSelected(ensembleId: string) {
        return uploadEnsembleIds.includes(ensembleId);
    }

    function toggleUploadEnsembleSelection(
        ensembleId: string,
        checked: boolean,
    ) {
        if (checked) {
            if (!uploadEnsembleIds.includes(ensembleId)) {
                uploadEnsembleIds = [...uploadEnsembleIds, ensembleId];
            }
            return;
        }

        uploadEnsembleIds = uploadEnsembleIds.filter(
            (value) => value !== ensembleId,
        );
    }

    function openCreateScoreModal() {
        adminError = "";
        showCreateScoreModal = true;
    }

    function closeCreateScoreModal(force = false) {
        if (uploadBusy && !force) {
            return;
        }

        showCreateScoreModal = false;
        closeEnsemblePickerModal();
        uploadTitle = "";
        uploadPublicId = "";
        uploadQualityProfile = "standard";
        selectedFile = null;
        selectedIconFile = null;
        uploadEnsembleIds = ensembles[0] ? [ensembles[0].id] : [];
        const scoreInput = document.getElementById(
            "mscz-input",
        ) as HTMLInputElement | null;
        if (scoreInput) {
            scoreInput.value = "";
        }
        const iconInput = document.getElementById(
            "icon-file-input",
        ) as HTMLInputElement | null;
        if (iconInput) {
            iconInput.value = "";
        }
    }

    function openUploadEnsembleModal() {
        ensemblePickerMode = "upload";
        ensemblePickerMusicId = "";
        ensemblePickerSearchQuery = "";
    }

    function openScoreEnsembleModal(music: AdminMusic) {
        ensemblePickerMode = "score";
        ensemblePickerMusicId = music.id;
        ensemblePickerSearchQuery = "";
        stagedScoreEnsembleIds = [...music.ensemble_ids];
        openDownloadMenuFor = "";
    }

    function closeEnsemblePickerModal() {
        if (savingScoreEnsembles) return;
        ensemblePickerMode = "";
        ensemblePickerMusicId = "";
        ensemblePickerSearchQuery = "";
        stagedScoreEnsembleIds = [];
    }

    function toggleStagedEnsembleForScore(ensembleId: string) {
        if (stagedScoreEnsembleIds.includes(ensembleId)) {
            stagedScoreEnsembleIds = stagedScoreEnsembleIds.filter((id) => id !== ensembleId);
        } else {
            stagedScoreEnsembleIds = [...stagedScoreEnsembleIds, ensembleId];
        }
    }

    async function saveScoreEnsembles() {
        if (!activeEnsemblePickerMusic) return;
        const original = new Set(activeEnsemblePickerMusic.ensemble_ids);
        const staged = new Set(stagedScoreEnsembleIds);
        const toAdd = [...staged].filter((id) => !original.has(id));
        const toRemove = [...original].filter((id) => !staged.has(id));
        if (toAdd.length === 0 && toRemove.length === 0) {
            closeEnsemblePickerModal();
            return;
        }
        savingScoreEnsembles = true;
        adminError = "";
        try {
            for (const id of toAdd) {
                await addMusicToEnsemble(activeEnsemblePickerMusic.id, id);
            }
            for (const id of toRemove) {
                await removeMusicFromEnsemble(activeEnsemblePickerMusic.id, id);
            }
            await refreshAdminData();
            adminSuccess = "Score ensembles updated.";
            ensemblePickerMode = "";
            ensemblePickerMusicId = "";
            ensemblePickerSearchQuery = "";
            stagedScoreEnsembleIds = [];
        } catch (error) {
            adminError = error instanceof Error ? error.message : "Failed to update ensembles";
        } finally {
            savingScoreEnsembles = false;
        }
    }

    function openScoreInfoModal(music: AdminMusic) {
        infoMusicId = music.id;
        openDownloadMenuFor = "";
    }

    function closeScoreInfoModal() {
        infoMusicId = "";
    }

    function openConfirm(msg: string, label: string, action: () => Promise<void>) {
        confirmMessage = msg;
        confirmLabel = label;
        confirmAction = action;
    }

    function closeConfirm() {
        if (confirmBusy) return;
        confirmMessage = "";
        confirmAction = null;
    }

    async function executeConfirm() {
        if (!confirmAction) return;
        confirmBusy = true;
        try {
            await confirmAction();
        } finally {
            confirmBusy = false;
            closeConfirm();
        }
    }

    async function handleShowScoreQr(music: AdminMusic) {
        const url = scoreLinkForCopy(music);
        try {
            await onShowCredential(
                `Share link for ${music.title}`,
                () => Promise.resolve({ connection_url: url, expires_at: "" }),
            );
        } catch (error) {
            adminError = error instanceof Error ? error.message : "Failed to show QR";
        }
    }

    function openManageMembersModal(ensemble: Ensemble) {
        managingEnsembleId = ensemble.id;
        currentMemberSearchQuery = "";
        addMemberSearchQuery = "";
        originalManagedMemberRoles = buildEnsembleMemberRoleMap(ensemble);
        managedMemberDraftRoles = buildEnsembleMemberRoleMap(ensemble);
        inviteRoles = {};
        savingManagedMembers = false;
        adminError = "";
    }

    function closeManageMembersModal() {
        if (savingManagedMembers) {
            return;
        }
        managingEnsembleId = "";
        currentMemberSearchQuery = "";
        addMemberSearchQuery = "";
        originalManagedMemberRoles = {};
        managedMemberDraftRoles = {};
        inviteRoles = {};
    }

    function openManageScoresModal(ensemble: Ensemble) {
        managingEnsembleScoresId = ensemble.id;
        currentEnsembleScoreSearchQuery = "";
        addEnsembleScoreSearchQuery = "";
        originalEnsembleScoreMusicIds = musics
            .filter((m) => musicHasEnsemble(m, ensemble.id))
            .map((m) => m.id);
        stagedEnsembleScoreMusicIds = [...originalEnsembleScoreMusicIds];
        savingEnsembleScores = false;
        adminError = "";
    }

    function closeManageScoresModal() {
        if (savingEnsembleScores) {
            return;
        }
        managingEnsembleScoresId = "";
        currentEnsembleScoreSearchQuery = "";
        addEnsembleScoreSearchQuery = "";
        originalEnsembleScoreMusicIds = [];
        stagedEnsembleScoreMusicIds = [];
    }

    function openScoreMetadataModal(music: AdminMusic) {
        metadataMusicId = music.id;
        metadataTitle = music.title;
        metadataIcon = music.icon ?? "";
        metadataPublicId = music.public_id ?? "";
        adminError = "";
        openDownloadMenuFor = "";
    }

    function closeScoreMetadataModal() {
        if (savingMetadataFor) {
            return;
        }
        metadataMusicId = "";
        metadataTitle = "";
        metadataIcon = "";
        metadataPublicId = "";
    }

    function toggleDownloadMenu(musicId: string) {
        openDownloadMenuFor = openDownloadMenuFor === musicId ? "" : musicId;
    }

    async function toggleMusicEnsembleAssignment(
        musicId: string,
        ensembleId: string,
        shouldAdd: boolean,
    ) {
        adminError = "";
        adminSuccess = "";

        try {
            if (shouldAdd) {
                await addMusicToEnsemble(musicId, ensembleId);
            } else {
                await removeMusicFromEnsemble(musicId, ensembleId);
            }
            await refreshAdminData();
            adminSuccess = "Score ensembles updated.";
        } catch (error) {
            adminError =
                error instanceof Error
                    ? error.message
                    : "Unable to update score ensembles";
        }
    }

    function toggleStagedEnsembleScore(musicId: string, shouldAdd: boolean) {
        if (shouldAdd) {
            stagedEnsembleScoreMusicIds = [...stagedEnsembleScoreMusicIds, musicId];
        } else {
            stagedEnsembleScoreMusicIds = stagedEnsembleScoreMusicIds.filter((id) => id !== musicId);
        }
    }

    function hasManagedScoreChanges(): boolean {
        const orig = new Set(originalEnsembleScoreMusicIds);
        const staged = new Set(stagedEnsembleScoreMusicIds);
        if (orig.size !== staged.size) return true;
        return [...staged].some((id) => !orig.has(id));
    }

    async function saveEnsembleScores() {
        if (!managingEnsembleScoresId) return;
        const orig = new Set(originalEnsembleScoreMusicIds);
        const staged = new Set(stagedEnsembleScoreMusicIds);
        const toAdd = [...staged].filter((id) => !orig.has(id));
        const toRemove = [...orig].filter((id) => !staged.has(id));
        if (toAdd.length === 0 && toRemove.length === 0) {
            closeManageScoresModal();
            return;
        }
        savingEnsembleScores = true;
        adminError = "";
        try {
            for (const id of toAdd) {
                await addMusicToEnsemble(id, managingEnsembleScoresId);
            }
            for (const id of toRemove) {
                await removeMusicFromEnsemble(id, managingEnsembleScoresId);
            }
            await refreshAdminData();
            adminSuccess = "Ensemble scores updated.";
            closeManageScoresModal();
        } catch (error) {
            adminError = error instanceof Error ? error.message : "Failed to update scores";
        } finally {
            savingEnsembleScores = false;
        }
    }

    async function handleUpload() {
        if (!selectedFile) {
            adminError = "Choose an .mscz file first.";
            return;
        }

        uploadBusy = true;
        adminError = "";
        adminSuccess = "";

        try {
            const uploaded = await uploadMusic({
                file: selectedFile,
                title: uploadTitle,
                icon: "",
                iconFile: selectedIconFile,
                publicId: uploadPublicId,
                qualityProfile: uploadQualityProfile,
                ensembleId: uploadEnsembleIds[0] ?? "",
            });

            for (const ensembleId of uploadEnsembleIds.slice(1)) {
                if (!uploaded.ensemble_ids.includes(ensembleId)) {
                    await addMusicToEnsemble(uploaded.id, ensembleId);
                }
            }

            await refreshAdminData();
            uploadBusy = false;
            closeCreateScoreModal(true);
            adminSuccess = "Upload completed.";
        } catch (error) {
            adminError =
                error instanceof Error ? error.message : "Upload failed";
        } finally {
            if (uploadBusy) {
                uploadBusy = false;
            }
        }
    }

    async function handleSaveScoreMetadata() {
        if (!metadataMusicId) {
            return;
        }
        if (!metadataTitle.trim()) {
            adminError = "Title cannot be empty.";
            return;
        }

        savingMetadataFor = metadataMusicId;
        adminError = "";
        adminSuccess = "";

        try {
            const updated = await updateMusicMetadata(metadataMusicId, {
                title: metadataTitle,
                publicId: metadataPublicId,
                icon: metadataIcon,
            });
            musics = musics.map((music) =>
                music.id === metadataMusicId ? updated : music,
            );
            closeScoreMetadataModal();
            adminSuccess = "Score metadata updated.";
        } catch (error) {
            adminError =
                error instanceof Error
                    ? error.message
                    : "Unable to update score metadata";
        } finally {
            savingMetadataFor = "";
        }
    }

    async function deleteMusicAccount(musicId: string) {
        deletingMusicFor = musicId;
        adminError = "";
        adminSuccess = "";
        try {
            await deleteMusic(musicId);
            musics = musics.filter((music) => music.id !== musicId);
            await refreshAdminData();
            adminSuccess = "Score deleted.";
        } catch (error) {
            adminError = error instanceof Error ? error.message : "Unable to delete score";
        } finally {
            deletingMusicFor = "";
        }
    }

    function handleDeleteMusic(musicId: string) {
        openConfirm("Delete this score permanently?", "Delete", () => deleteMusicAccount(musicId));
    }

    async function deleteUserAccount(user: AppUser) {
        deletingUserFor = user.id;
        adminError = "";
        adminSuccess = "";
        try {
            await deleteUser(user.id);
            adminUsers = adminUsers.filter((candidate) => candidate.id !== user.id);
            await refreshAdminData();
            adminSuccess = `User ${user.username} deleted.`;
        } catch (error) {
            adminError = error instanceof Error ? error.message : "Unable to delete user";
        } finally {
            deletingUserFor = "";
        }
    }

    function handleDeleteUser(user: AppUser) {
        openConfirm(`Delete ${user.username} permanently?`, "Delete", () => deleteUserAccount(user));
    }

    async function deleteEnsembleAccount(ensemble: Ensemble) {
        deletingEnsembleFor = ensemble.id;
        adminError = "";
        adminSuccess = "";
        try {
            await deleteEnsemble(ensemble.id);
            ensembles = ensembles.filter((candidate) => candidate.id !== ensemble.id);
            await refreshAdminData();
            adminSuccess = `Ensemble ${ensemble.name} deleted.`;
        } catch (error) {
            adminError = error instanceof Error ? error.message : "Unable to delete ensemble";
        } finally {
            deletingEnsembleFor = "";
        }
    }

    function handleDeleteEnsemble(ensemble: Ensemble) {
        openConfirm(`Delete ensemble ${ensemble.name}?`, "Delete", () => deleteEnsembleAccount(ensemble));
    }

    async function handleRetryRender(musicId: string) {
        retryingFor = musicId;
        adminError = "";
        adminSuccess = "";

        try {
            const updated = await retryRender(musicId);
            musics = musics.map((music) =>
                music.id === musicId ? updated : music,
            );
            adminSuccess = "Render retried successfully.";
        } catch (error) {
            adminError =
                error instanceof Error ? error.message : "Retry failed";
        } finally {
            retryingFor = "";
        }
    }

    async function copyText(value: string, successMessage: string) {
        await navigator.clipboard.writeText(value);
        adminSuccess = successMessage;
        adminError = "";
    }

    async function handleShowUserQr(user: AppUser) {
        adminError = "";
        adminSuccess = "";

        try {
            await onShowCredential(
                `QR code for ${user.username}`,
                () => createAdminUserLoginLink(user.id),
            );
            adminSuccess = `QR code ready for ${user.username}.`;
        } catch (error) {
            adminError =
                error instanceof Error
                    ? error.message
                    : "Unable to create QR code";
        }
    }

    function handleFileSelection(event: Event) {
        const target = event.currentTarget as HTMLInputElement;
        selectedFile = target.files?.[0] ?? null;
    }

    function memberRoleLabel(role: EnsembleRole) {
        if (role === "manager") {
            return "Manager";
        }
        if (role === "editor") {
            return "Editor";
        }
        return "Member";
    }

    async function handleAddUserToManagedEnsemble(userId: string) {
        stageManagedMemberRole(userId, inviteRoles[userId] ?? "user");
    }
</script>

{#if (userLoading && !currentUser) || (adminLoading && musics.length === 0 && adminUsers.length === 0 && ensembles.length === 0)}
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
{:else if !currentUser}
    <section class="admin-login-shell">
        <div class="music-card auth-card admin-auth-card">
            <div>
                <p class="eyebrow">Fumen â€¢ Admin</p>
                <h1>Control room</h1>
                <p class="lede">
                    Sign in as the seeded superadmin or another admin-enabled
                    user to open the full-screen control room.
                </p>
            </div>
            <div class="actions">
                <a class="button ghost" href="/">User homepage</a>
            </div>
            <p class="hint">
                Use the backend CLI to generate a temporary connection link for
                the superadmin account, then open it here.
            </p>
            {#if userError}<p class="status error">{userError}</p>{/if}
        </div>
    </section>
{:else if !canAccessAdmin()}
    <section class="admin-login-shell">
        <div class="music-card auth-card admin-auth-card">
            <div>
                <p class="eyebrow">Fumen â€¢ Admin</p>
                <h1>Control room</h1>
                <p class="lede">
                    {currentUser.username} is signed in, but this account does not
                    have admin access.
                </p>
            </div>
            <div class="actions">
                <a class="button ghost" href="/">User homepage</a>
            </div>
            {#if adminError}<p class="status error">{adminError}</p>{/if}
        </div>
    </section>
{:else}
    <section class="admin-app-shell">
        <TopBar
            breadcrumbs={[
                { label: "Fumen", href: "/" },
                { label: "Admin" },
                { label: currentAdminSectionItem().label },
            ]}
            {currentUser}
            userHomeHref="/"
            onLogout={() => void onLogout()}
            mobileMenuItems={mobileAdminSectionItems()}
            mobileMenuActiveId={adminSection}
            mobileMenuAriaLabel="Admin sections"
            onMobileMenuSelect={(sectionId) =>
                (adminSection = sectionId as AdminSection)}
        />

        <div class="admin-shell-body">
            <aside class="admin-sidebar">
                <nav class="admin-nav-list" aria-label="Admin sections">
                    {#each visibleAdminSectionItems as section}
                        <button
                            class="admin-nav-button"
                            class:is-active={adminSection === section.id}
                            onclick={() => (adminSection = section.id)}
                        >
                            <span class="admin-nav-eyebrow"
                                >{section.eyebrow}</span
                            >
                            <strong>{section.label}</strong>
                            {#if section.id === "users"}
                                <small>{adminUsers.length} total</small>
                            {:else if section.id === "ensembles"}
                                <small>{ensembles.length} groups</small>
                            {:else if section.id === "scores"}
                                <small>{musics.length} scores</small>
                            {:else}
                                <small>Everything at a glance</small>
                            {/if}
                        </button>
                    {/each}
                </nav>

                {#if adminError}<p class="status error">{adminError}</p>{/if}
                {#if adminSuccess}<p class="status success">
                        {adminSuccess}
                    </p>{/if}
            </aside>

            <div class="admin-main">
                {#if adminSection === "users"}
                    <section class="list-section admin-stage">
                        <div class="admin-panel-stack admin-user-panel-stack">
                            <div class="music-card admin-users-toolbar">
                                <div class="admin-users-toolbar-copy">
                                    <h3>User accounts</h3>
                                </div>
                                <label class="field admin-user-search">
                                    <span class="sr-only">Search users</span>
                                    <div class="admin-user-search-input-wrap">
                                        <Search size={15} aria-hidden="true" />
                                        <input
                                            bind:value={userSearchQuery}
                                            placeholder="Search users"
                                        />
                                    </div>
                                </label>
                                {#if canManageUsers()}
                                    <button
                                        class="button admin-create-user-btn"
                                        onclick={openCreateUserModal}
                                    >
                                        <Plus size={15} aria-hidden="true" />
                                        Create user
                                    </button>
                                {/if}
                            </div>

                            {#if adminUsers.length === 0}
                                <div class="music-card">
                                    <p class="hint">No users yet.</p>
                                </div>
                            {:else if filteredAdminUsers.length === 0}
                                <div class="music-card">
                                    <p class="hint">
                                        No users match â€œ{userSearchQuery.trim()}â€.
                                    </p>
                                </div>
                            {:else}
                                <div class="music-list admin-user-list">
                                    {#each filteredAdminUsers as user}
                                        <article
                                            class="music-card admin-user-row admin-account-card"
                                        >
                                            <div class="admin-user-row-main">
                                                <div
                                                    class="admin-user-avatar"
                                                    aria-hidden="true"
                                                >
                                                    {user.username
                                                        .slice(0, 1)
                                                        .toUpperCase()}
                                                </div>
                                                <div class="admin-user-copy">
                                                    <h3>{user.username}</h3>
                                                    <p
                                                        class="admin-user-role-pill"
                                                    >
                                                        {user.role}
                                                    </p>
                                                </div>
                                            </div>
                                            <div
                                                class="actions admin-user-actions"
                                            >
                                                <button
                                                    class="button secondary admin-user-action"
                                                    onclick={() =>
                                                        void handleShowUserQr(
                                                            user,
                                                        )}
                                                    aria-label={`Show QR code for ${user.username}`}
                                                    title="Show QR code"
                                                >
                                                    <QrCode size={15} aria-hidden="true" />
                                                </button>
                                                {#if canDeleteUserAccount(user)}
                                                    <button
                                                        class="button ghost danger admin-user-action"
                                                        type="button"
                                                        disabled={deletingUserFor ===
                                                            user.id}
                                                        onclick={() =>
                                                            void handleDeleteUser(
                                                                user,
                                                            )}
                                                        aria-label={`Delete ${user.username}`}
                                                        title="Delete user"
                                                    >
                                                        <Trash2 size={16} aria-hidden="true" />
                                                    </button>
                                                {/if}
                                            </div>
                                        </article>
                                    {/each}
                                </div>
                            {/if}
                        </div>
                    </section>
                {:else if adminSection === "ensembles"}
                    <section class="list-section admin-stage">
                        <div class="admin-panel-stack admin-user-panel-stack">
                            <div class="music-card admin-users-toolbar">
                                <div class="admin-users-toolbar-copy">
                                    <h3>Ensembles</h3>
                                </div>
                                <label class="field admin-user-search">
                                    <span class="sr-only">Search ensembles</span
                                    >
                                    <div class="admin-user-search-input-wrap">
                                        <Search size={15} aria-hidden="true" />
                                        <input
                                            bind:value={ensembleSearchQuery}
                                            placeholder="Search ensembles"
                                        />
                                    </div>
                                </label>
                                {#if canCreateEnsembles()}
                                    <button
                                        class="button admin-create-user-btn"
                                        type="button"
                                        onclick={openCreateEnsembleModal}
                                    >
                                        <Plus size={15} aria-hidden="true" />
                                        Create ensemble
                                    </button>
                                {/if}
                            </div>

                            {#if ensembles.length === 0}
                                <div class="music-card">
                                    <p class="hint">No ensembles yet.</p>
                                </div>
                            {:else if filteredEnsembles.length === 0}
                                <div class="music-card">
                                    <p class="hint">
                                        No ensembles match "{ensembleSearchQuery.trim()}".
                                    </p>
                                </div>
                            {:else}
                                <div
                                    class="music-list admin-user-list admin-ensemble-list"
                                >
                                    {#each filteredEnsembles as ensemble}
                                        <article
                                            class="music-card admin-user-row admin-ensemble-card"
                                        >
                                            <div class="admin-user-row-main">
                                                <div
                                                    class="admin-user-avatar admin-ensemble-avatar"
                                                    aria-hidden="true"
                                                >
                                                    <Users size={16} aria-hidden="true" />
                                                </div>
                                                <div
                                                    class="admin-user-copy admin-ensemble-copy"
                                                >
                                                    <h3>{ensemble.name}</h3>
                                                </div>
                                            </div>
                                            <div
                                                class="actions admin-user-actions"
                                            >
                                                <button
                                                    class="button secondary admin-user-action"
                                                    type="button"
                                                    onclick={() =>
                                                        openManageScoresModal(
                                                            ensemble,
                                                        )}
                                                    aria-label={`Manage scores for ${ensemble.name}`}
                                                    title="Manage scores"
                                                    disabled={!canManageEnsembleScores(
                                                        ensemble,
                                                    )}
                                                >
                                                    <Music size={16} aria-hidden="true" />
                                                    <span
                                                        class="admin-action-badge"
                                                        aria-hidden="true"
                                                        >{ensemble.score_count}</span
                                                    >
                                                </button>
                                                <button
                                                    class="button secondary admin-user-action"
                                                    type="button"
                                                    onclick={() =>
                                                        openManageMembersModal(
                                                            ensemble,
                                                        )}
                                                    aria-label={`Manage members for ${ensemble.name}`}
                                                    title="Manage members"
                                                    disabled={!canManageEnsembleMembers(
                                                        ensemble,
                                                    )}
                                                >
                                                    <UserPlus size={16} aria-hidden="true" />
                                                    <span
                                                        class="admin-action-badge"
                                                        aria-hidden="true"
                                                        >{ensemble.members.length}</span
                                                    >
                                                </button>
                                                {#if canDeleteEnsembleRecord(ensemble)}
                                                    <button
                                                        class="button ghost danger admin-user-action"
                                                        type="button"
                                                        disabled={deletingEnsembleFor ===
                                                            ensemble.id}
                                                        onclick={() =>
                                                            void handleDeleteEnsemble(
                                                                ensemble,
                                                            )}
                                                        aria-label={`Delete ${ensemble.name}`}
                                                        title="Delete ensemble"
                                                    >
                                                        <Trash2 size={16} aria-hidden="true" />
                                                    </button>
                                                {/if}
                                            </div>
                                        </article>
                                    {/each}
                                </div>
                            {/if}
                        </div>
                    </section>
                {:else if adminSection === "scores"}
                    <section class="list-section admin-stage">
                        <div class="admin-panel-stack admin-user-panel-stack">
                            <div class="music-card admin-users-toolbar">
                                <div class="admin-users-toolbar-copy">
                                    <h3>Scores</h3>
                                </div>
                                <label class="field admin-user-search">
                                    <span class="sr-only">Search scores</span>
                                    <div class="admin-user-search-input-wrap">
                                        <Search size={15} aria-hidden="true" />
                                        <input
                                            bind:value={scoreSearchQuery}
                                            placeholder="Search scores"
                                        />
                                    </div>
                                </label>
                                <button
                                    class="button admin-create-user-btn"
                                    type="button"
                                    onclick={openCreateScoreModal}
                                >
                                    <Plus size={15} aria-hidden="true" />
                                    Add a score
                                </button>
                            </div>

                            {#if musics.length === 0}
                                <div class="music-card">
                                    <p class="hint">No uploads yet.</p>
                                </div>
                            {:else if filteredMusics.length === 0}
                                <div class="music-card">
                                    <p class="hint">
                                        No scores match "{scoreSearchQuery.trim()}".
                                    </p>
                                </div>
                            {:else}
                                <div class="music-list admin-score-list">
                                    {#each filteredMusics as music}
                                        <article
                                            class="music-card admin-score-card"
                                            class:download-open={openDownloadMenuFor === music.id}
                                        >
                                            <div class="admin-score-header">
                                                <h3 class="admin-score-title">
                                                    <ScoreIcon
                                                        variant="admin"
                                                        icon={music.icon}
                                                        imageUrl={music.icon_image_url}
                                                    />
                                                    <span>{music.title}</span>
                                                </h3>
                                                <div
                                                    class="download-menu admin-score-download-menu"
                                                    class:open={openDownloadMenuFor ===
                                                        music.id}
                                                >
                                                    <button
                                                        class="download-menu-btn admin-score-download-btn"
                                                        type="button"
                                                        onclick={() =>
                                                            toggleDownloadMenu(
                                                                music.id,
                                                            )}
                                                        aria-label={`Download files for ${music.title}`}
                                                        title="Downloads"
                                                        aria-haspopup="true"
                                                        aria-expanded={openDownloadMenuFor ===
                                                            music.id}
                                                    >
                                                        <Download size={15} strokeWidth={2.2} />
                                                        <span>Download</span>
                                                        <ChevronDown class="chevron" size={12} strokeWidth={2.5} />
                                                    </button>
                                                    {#if openDownloadMenuFor === music.id}
                                                        <div
                                                            class="download-dropdown"
                                                        >
                                                            <a
                                                                class="download-item"
                                                                href={music.download_url}
                                                                target="_blank"
                                                                rel="noreferrer"
                                                                onclick={() =>
                                                                    (openDownloadMenuFor =
                                                                        "")}
                                                            >
                                                                <Download size={18} strokeWidth={2.2} aria-hidden="true" />
                                                                <span>Download MuseScore</span>
                                                            </a>
                                                            {#if music.midi_download_url}
                                                                <a
                                                                    class="download-item"
                                                                    href={music.midi_download_url}
                                                                    target="_blank"
                                                                    rel="noreferrer"
                                                                    onclick={() =>
                                                                        (openDownloadMenuFor =
                                                                            "")}
                                                                >
                                                                    <Music size={18} strokeWidth={2.2} aria-hidden="true" />
                                                                    <span>Download MIDI</span>
                                                                </a>
                                                            {/if}
                                                        </div>
                                                    {/if}
                                                </div>
                                            </div>
                                            <div
                                                class="actions admin-score-actions"
                                            >
                                                {#if canManageScoreEnsembles(music)}
                                                    <button
                                                        class="button secondary admin-user-action"
                                                        type="button"
                                                        onclick={() =>
                                                            openScoreEnsembleModal(
                                                                music,
                                                            )}
                                                        aria-label={`Manage ensembles for ${music.title}`}
                                                        title="Manage ensembles"
                                                    >
                                                        <Users size={16} aria-hidden="true" />
                                                        <span
                                                            class="admin-action-badge"
                                                            aria-hidden="true"
                                                            >{music.ensemble_names
                                                                .length}</span
                                                        >
                                                    </button>
                                                {/if}
                                                {#if canEditOwnedScore(music)}
                                                    <button
                                                        class="button secondary admin-user-action"
                                                        type="button"
                                                        onclick={() =>
                                                            openScoreMetadataModal(
                                                                music,
                                                            )}
                                                        aria-label={`Edit metadata for ${music.title}`}
                                                        title="Edit metadata"
                                                    >
                                                        <Pencil size={16} aria-hidden="true" />
                                                    </button>
                                                {/if}
                                                <button
                                                    class="button secondary admin-user-action"
                                                    type="button"
                                                    onclick={() => void handleShowScoreQr(music)}
                                                    aria-label={`Share QR for ${music.title}`}
                                                    title="Share QR code"
                                                >
                                                    <QrCode size={16} aria-hidden="true" />
                                                </button>
                                                <button
                                                    class="button secondary admin-user-action"
                                                    type="button"
                                                    onclick={() =>
                                                        openScoreInfoModal(
                                                            music,
                                                        )}
                                                    aria-label={`View metadata for ${music.title}`}
                                                    title="View metadata"
                                                >
                                                    <Info size={16} aria-hidden="true" />
                                                </button>
                                                {#if canDeleteScore(music)}
                                                    <button
                                                        class="button ghost danger admin-user-action"
                                                        type="button"
                                                        aria-label={`Delete ${music.title}`}
                                                        title="Delete score"
                                                        disabled={deletingMusicFor ===
                                                            music.id}
                                                        onclick={() =>
                                                            void handleDeleteMusic(
                                                                music.id,
                                                            )}
                                                    >
                                                        <Trash2 size={16} aria-hidden="true" />
                                                    </button>
                                                {/if}
                                            </div>
                                        </article>
                                    {/each}
                                </div>
                            {/if}
                        </div>
                    </section>
                {/if}
            </div>
        </div>
    </section>
{/if}

{#if showCreateUserModal}
    {#snippet createUserFooter()}
        <div class="actions admin-user-modal-actions">
            <button class="button ghost" type="button" disabled={creatingUser} onclick={closeCreateUserModal}>Cancel</button>
            <button class="button" type="button" disabled={creatingUser} onclick={() => void handleCreateUser()}>
                {creatingUser ? "Creating..." : "Create user"}
            </button>
        </div>
    {/snippet}
    <BaseModal
        onClose={closeCreateUserModal}
        size="medium"
        cardClass="admin-user-modal"
        title="Create"
        subtitle="New account"
        footer={createUserFooter}
    >
        <p class="subtle">
            Create a username-only account, assign its global role, then
            generate a QR code or connection link from the list.
        </p>
        <label class="field">
            <span>Username</span>
            <input
                bind:value={newUsername}
                placeholder="example: lucas"
                onkeydown={(event) => {
                    if (event.key === "Enter") {
                        void handleCreateUser();
                    } else if (event.key === "Escape") {
                        closeCreateUserModal();
                    }
                }}
            />
        </label>
        <CustomSelect
            label="Global role"
            bind:value={newUserRole}
            options={createUserRoleOptions}
        />
    </BaseModal>
{/if}

{#if showCreateEnsembleModal}
    {#snippet createEnsembleFooter()}
        <div class="actions admin-user-modal-actions">
            <button class="button ghost" type="button" disabled={creatingEnsemble} onclick={closeCreateEnsembleModal}>Cancel</button>
            <button class="button" type="button" disabled={creatingEnsemble} onclick={() => void handleCreateEnsemble()}>
                {creatingEnsemble ? "Creating..." : "Create ensemble"}
            </button>
        </div>
    {/snippet}
    <BaseModal
        onClose={closeCreateEnsembleModal}
        size="medium"
        cardClass="admin-user-modal"
        title="Create"
        subtitle="New ensemble"
        footer={createEnsembleFooter}
    >
        <label class="field">
            <span>Ensemble name</span>
            <input
                bind:value={newEnsembleName}
                placeholder="example: Strings"
                onkeydown={(event) => {
                    if (event.key === "Enter") {
                        void handleCreateEnsemble();
                    } else if (event.key === "Escape") {
                        closeCreateEnsembleModal();
                    }
                }}
            />
        </label>
    </BaseModal>
{/if}

{#if showCreateScoreModal}
    {#snippet uploadScoreFooter()}
        <div class="actions admin-user-modal-actions">
            <button
                class="button ghost"
                type="button"
                disabled={uploadBusy}
                onclick={() => closeCreateScoreModal()}
            >
                Cancel
            </button>
            <button
                class="button"
                type="button"
                disabled={uploadBusy}
                onclick={() => void handleUpload()}
            >
                {uploadBusy ? "Uploading..." : "Add score"}
            </button>
        </div>
    {/snippet}
    <BaseModal
        onClose={closeCreateScoreModal}
        size="large"
        cardClass="admin-score-modal"
        title="Upload"
        subtitle="Add a MuseScore score"
        footer={uploadScoreFooter}
    >
        <div class="upload-grid admin-score-modal-grid">
            <label class="field admin-score-modal-full">
                <span>Title</span>
                <input
                    bind:value={uploadTitle}
                    placeholder="Optional display title"
                />
            </label>
            <label class="field">
                <span>Public id</span>
                <input
                    bind:value={uploadPublicId}
                    placeholder="Optional friendly id"
                />
            </label>
            <label class="field admin-score-quality-field">
                <span>Stem quality</span>
                <CustomSelect
                    bind:value={uploadQualityProfile}
                    options={stemQualityOptions}
                    compact={true}
                    showDescriptionInTrigger={false}
                />
                <small class="subtle">
                    {STEM_QUALITY_PROFILES.find(
                        (option) => option.value === uploadQualityProfile,
                    )?.description}
                </small>
            </label>
            <label class="field file-field admin-score-file-field">
                <span>Icon image</span>
                <input
                    id="icon-file-input"
                    type="file"
                    accept="image/*"
                    onchange={(event) => {
                        const target = event.currentTarget as HTMLInputElement;
                        selectedIconFile = target.files?.[0] ?? null;
                    }}
                />
            </label>
            <label class="field file-field admin-score-file-field">
                <span>MSCZ file</span>
                <input
                    id="mscz-input"
                    type="file"
                    accept=".mscz"
                    onchange={handleFileSelection}
                />
            </label>
        </div>
        <button
            class="button ghost admin-score-ensemble-trigger"
            type="button"
            onclick={openUploadEnsembleModal}
        >
            <Users size={16} aria-hidden="true" />
            {uploadEnsembleIds.length > 0
                ? `Selected ensembles (${uploadEnsembleIds.length})`
                : "Choose ensembles"}
        </button>
    </BaseModal>
{/if}

{#if activeManagedEnsemble}
    {#snippet manageMembersFooter()}
        <div class="actions admin-user-modal-actions">
            <button
                class="button ghost"
                type="button"
                disabled={savingManagedMembers}
                onclick={closeManageMembersModal}
            >
                Cancel
            </button>
            <button
                class="button"
                type="button"
                disabled={savingManagedMembers || !hasManagedMemberChanges()}
                onclick={() => void saveManagedEnsembleChanges()}
            >
                {savingManagedMembers ? "Saving..." : "Save changes"}
            </button>
        </div>
    {/snippet}
    <BaseModal
        onClose={closeManageMembersModal}
        size="full"
        cardClass="admin-split-modal"
        title="Members"
        subtitle={activeManagedEnsemble.name}
        footer={manageMembersFooter}
    >
        <div class="admin-split-pane">
            <section class="admin-split-column">
                <div class="admin-split-header">
                    <div class="admin-split-header-main">
                        <h4>Current members</h4>
                        <span class="admin-user-role-pill">
                            {filteredManagedMembers.length}
                        </span>
                    </div>
                    <label class="field admin-user-search admin-split-search">
                        <span class="sr-only">Search current members</span>
                        <div class="admin-user-search-input-wrap">
                            <Search size={15} aria-hidden="true" />
                            <input
                                bind:value={currentMemberSearchQuery}
                                placeholder="Search current members"
                            />
                        </div>
                    </label>
                </div>
                <div class="admin-inline-list">
                    {#if filteredManagedMembers.length === 0}
                        <p class="hint">No matching members.</p>
                    {:else}
                        {#each filteredManagedMembers as member}
                            <div class="admin-inline-row">
                                <div class="admin-inline-copy">
                                    <strong>{member.user!.username}</strong>
                                    <span class="admin-user-role-pill">
                                        {memberRoleLabel(member.role)}
                                    </span>
                                </div>
                                <div class="admin-inline-actions">
                                    {#if allowedEnsembleRolesForUser(member.user!).length > 0}
                                        <div class="admin-member-role-select">
                                            <CustomSelect
                                                value={member.role}
                                                options={ensembleRoleOptionsForUser(
                                                    member.user!,
                                                )}
                                                compact={true}
                                                showDescriptionInTrigger={false}
                                                onValueChange={(role) =>
                                                    stageManagedMemberRole(
                                                        member.user_id,
                                                        role as EnsembleRole,
                                                    )}
                                            />
                                        </div>
                                        <button
                                            class="button ghost danger admin-inline-icon-btn admin-inline-symbol-btn"
                                            type="button"
                                            aria-label={`Remove ${member.user!.username}`}
                                            title={`Remove ${member.user!.username}`}
                                            onclick={() =>
                                                stageManagedMemberRole(
                                                    member.user_id,
                                                    "none",
                                                )}
                                        >
                                            <span aria-hidden="true">-</span>
                                        </button>
                                    {:else}
                                        <span class="hint">Locked</span>
                                    {/if}
                                </div>
                            </div>
                        {/each}
                    {/if}
                </div>
            </section>
            <section class="admin-split-column">
                <div class="admin-split-header">
                    <div class="admin-split-header-main">
                        <h4>Add members</h4>
                        <span class="admin-user-role-pill">
                            {filteredAvailableEnsembleUsers.length}
                        </span>
                    </div>
                    <label class="field admin-user-search admin-split-search">
                        <span class="sr-only">Search available users</span>
                        <div class="admin-user-search-input-wrap">
                            <Search size={15} aria-hidden="true" />
                            <input
                                bind:value={addMemberSearchQuery}
                                placeholder="Search available users"
                            />
                        </div>
                    </label>
                </div>
                <div class="admin-inline-list">
                    {#if filteredAvailableEnsembleUsers.length === 0}
                        <p class="hint">No available users.</p>
                    {:else}
                        {#each filteredAvailableEnsembleUsers as user}
                            <div class="admin-inline-row">
                                <div class="admin-inline-copy">
                                    <strong>{user.username}</strong>
                                    <span class="admin-user-role-pill"
                                        >{user.role}</span
                                    >
                                </div>
                                <div class="admin-inline-actions">
                                    <div class="admin-member-role-select">
                                        <CustomSelect
                                            value={inviteRoles[user.id] ??
                                                allowedEnsembleRolesForUser(
                                                    user,
                                                )[0] ??
                                                "user"}
                                            options={ensembleRoleOptionsForUser(
                                                user,
                                            )}
                                            compact={true}
                                            showDescriptionInTrigger={false}
                                            onValueChange={(role) => {
                                                inviteRoles = {
                                                    ...inviteRoles,
                                                    [user.id]: role as EnsembleRole,
                                                };
                                            }}
                                        />
                                    </div>
                                    <button
                                        class="button secondary admin-inline-icon-btn admin-inline-symbol-btn"
                                        type="button"
                                        aria-label={`Add ${user.username}`}
                                        title={`Add ${user.username}`}
                                        onclick={() =>
                                            void handleAddUserToManagedEnsemble(
                                                user.id,
                                            )}
                                    >
                                        <span aria-hidden="true">+</span>
                                    </button>
                                </div>
                            </div>
                        {/each}
                    {/if}
                </div>
            </section>
        </div>
    </BaseModal>
{/if}

{#if activeManagedScoreEnsemble}
    {#snippet manageScoresFooter()}
        <div class="actions admin-user-modal-actions">
            <button
                class="button ghost"
                type="button"
                disabled={savingEnsembleScores}
                onclick={closeManageScoresModal}
            >
                Cancel
            </button>
            <button
                class="button"
                type="button"
                disabled={savingEnsembleScores || !hasManagedScoreChanges()}
                onclick={() => void saveEnsembleScores()}
            >
                {savingEnsembleScores ? "Saving..." : "Save changes"}
            </button>
        </div>
    {/snippet}
    <BaseModal
        onClose={closeManageScoresModal}
        size="full"
        cardClass="admin-split-modal"
        title="Scores"
        subtitle={activeManagedScoreEnsemble.name}
        footer={manageScoresFooter}
    >
        <div class="admin-split-pane">
            <section class="admin-split-column">
                <div class="admin-split-header">
                    <div class="admin-split-header-main">
                        <h4>Current scores</h4>
                        <span class="admin-user-role-pill">
                            {filteredManagedEnsembleScores.length}
                        </span>
                    </div>
                    <label class="field admin-user-search admin-split-search">
                        <span class="sr-only">Search current scores</span>
                        <div class="admin-user-search-input-wrap">
                            <Search size={15} aria-hidden="true" />
                            <input
                                bind:value={currentEnsembleScoreSearchQuery}
                                placeholder="Search current scores"
                            />
                        </div>
                    </label>
                </div>
                <div class="admin-inline-list">
                    {#if filteredManagedEnsembleScores.length === 0}
                        <p class="hint">No matching scores in this ensemble.</p>
                    {:else}
                        {#each filteredManagedEnsembleScores as music}
                            <div class="admin-inline-row">
                                <div class="admin-inline-copy">
                                    <strong>{music.title}</strong>
                                    {#if music.public_id}
                                        <span class="status-pill">
                                            {music.public_id}
                                        </span>
                                    {/if}
                                </div>
                                <div class="admin-inline-actions">
                                    <button
                                        class="button ghost danger admin-inline-icon-btn admin-inline-symbol-btn"
                                        type="button"
                                        disabled={savingEnsembleScores}
                                        aria-label={`Remove ${music.title}`}
                                        title={`Remove ${music.title}`}
                                        onclick={() =>
                                            toggleStagedEnsembleScore(
                                                music.id,
                                                false,
                                            )}
                                    >
                                        <span aria-hidden="true">-</span>
                                    </button>
                                </div>
                            </div>
                        {/each}
                    {/if}
                </div>
            </section>
            <section class="admin-split-column">
                <div class="admin-split-header">
                    <div class="admin-split-header-main">
                        <h4>Add scores</h4>
                        <span class="admin-user-role-pill">
                            {filteredAvailableEnsembleScores.length}
                        </span>
                    </div>
                    <label class="field admin-user-search admin-split-search">
                        <span class="sr-only">Search available scores</span>
                        <div class="admin-user-search-input-wrap">
                            <Search size={15} aria-hidden="true" />
                            <input
                                bind:value={addEnsembleScoreSearchQuery}
                                placeholder="Search available scores"
                            />
                        </div>
                    </label>
                </div>
                <div class="admin-inline-list">
                    {#if filteredAvailableEnsembleScores.length === 0}
                        <p class="hint">No available scores.</p>
                    {:else}
                        {#each filteredAvailableEnsembleScores as music}
                            <div class="admin-inline-row">
                                <div class="admin-inline-copy">
                                    <strong>{music.title}</strong>
                                    {#if music.public_id}
                                        <span class="status-pill">
                                            {music.public_id}
                                        </span>
                                    {/if}
                                </div>
                                <div class="admin-inline-actions">
                                    <button
                                        class="button secondary admin-inline-icon-btn admin-inline-symbol-btn"
                                        type="button"
                                        disabled={savingEnsembleScores}
                                        aria-label={`Add ${music.title}`}
                                        title={`Add ${music.title}`}
                                        onclick={() =>
                                            toggleStagedEnsembleScore(
                                                music.id,
                                                true,
                                            )}
                                    >
                                        <span aria-hidden="true">+</span>
                                    </button>
                                </div>
                            </div>
                        {/each}
                    {/if}
                </div>
            </section>
        </div>
    </BaseModal>
{/if}

{#if ensemblePickerMode}
    {#snippet ensemblePickerFooter()}
        {#if ensemblePickerMode === "score"}
            <div class="actions admin-user-modal-actions">
                <button class="button ghost" type="button" disabled={savingScoreEnsembles} onclick={closeEnsemblePickerModal}>Cancel</button>
                <button class="button" type="button" disabled={savingScoreEnsembles} onclick={() => void saveScoreEnsembles()}>
                    {savingScoreEnsembles ? "Saving..." : "Save changes"}
                </button>
            </div>
        {/if}
    {/snippet}
    <BaseModal
        onClose={closeEnsemblePickerModal}
        size="medium"
        cardClass="admin-selector-modal"
        title="Ensembles"
        subtitle={ensemblePickerMode === 'upload'
            ? 'Choose ensembles for the new score'
            : `Manage ensembles for ${activeEnsemblePickerMusic?.title ?? 'score'}`}
        footer={ensemblePickerFooter}
    >
        <label class="field admin-user-search">
            <span class="sr-only">Search ensembles</span>
            <div class="admin-user-search-input-wrap">
                <Search size={15} aria-hidden="true" />
                <input
                    bind:value={ensemblePickerSearchQuery}
                    placeholder="Search ensembles"
                />
            </div>
        </label>
        <div class="admin-inline-list admin-selector-list">
            {#if filteredPickerEnsembles.length === 0}
                <p class="hint">No ensembles match this search.</p>
            {:else}
                {#each filteredPickerEnsembles as ensemble}
                    <label class="admin-selector-row">
                        <div class="admin-inline-copy">
                            <strong>{ensemble.name}</strong>
                            <span class="admin-user-role-pill">
                                {ensemble.members.length} members
                            </span>
                        </div>
                        {#if ensemblePickerMode === "upload"}
                            <input
                                type="checkbox"
                                checked={isUploadEnsembleSelected(ensemble.id)}
                                onchange={(event) =>
                                    toggleUploadEnsembleSelection(
                                        ensemble.id,
                                        (
                                            event.currentTarget as HTMLInputElement
                                        ).checked,
                                    )}
                            />
                        {:else if activeEnsemblePickerMusic}
                            <input
                                type="checkbox"
                                checked={stagedScoreEnsembleIds.includes(ensemble.id)}
                                onchange={() => toggleStagedEnsembleForScore(ensemble.id)}
                            />
                        {/if}
                    </label>
                {/each}
            {/if}
        </div>
    </BaseModal>
{/if}

{#if activeMetadataMusic}
    {#snippet editScoreFooter()}
        <div class="actions admin-user-modal-actions">
            <button
                class="button ghost"
                type="button"
                disabled={!!savingMetadataFor}
                onclick={closeScoreMetadataModal}
            >
                Cancel
            </button>
            <button
                class="button"
                type="button"
                disabled={!!savingMetadataFor}
                onclick={() => void handleSaveScoreMetadata()}
            >
                {savingMetadataFor ? "Saving..." : "Save changes"}
            </button>
        </div>
    {/snippet}
    <BaseModal
        onClose={closeScoreMetadataModal}
        size="medium"
        cardClass="admin-score-modal"
        title="Edit score"
        subtitle={metadataTitle}
        footer={editScoreFooter}
    >
        <div class="upload-grid admin-score-modal-grid">
            <label class="field">
                <span>Title</span>
                <input bind:value={metadataTitle} />
            </label>
            <label class="field">
                <span>Score icon</span>
                <input
                    bind:value={metadataIcon}
                    maxlength="2"
                    placeholder="Optional emoji or 2-char mark"
                />
            </label>
            <label class="field admin-score-modal-full">
                <span>Friendly public id</span>
                <input
                    bind:value={metadataPublicId}
                    placeholder="example: moonlight-sonata"
                />
            </label>
        </div>
    </BaseModal>
{/if}

{#if activeInfoMusic}
    <BaseModal
        onClose={closeScoreInfoModal}
        size="large"
        cardClass="admin-score-modal"
        title="Score info"
        subtitle={activeInfoMusic.title}
    >
        <div class="upload-grid admin-score-modal-grid">
            <label class="field">
                <span>MSCZ filename</span>
                <input value={activeInfoMusic.filename} readonly />
            </label>
            <label class="field">
                <span>Stem quality</span>
                <input
                    value={qualityProfileLabel(
                        activeInfoMusic.quality_profile,
                    )}
                    readonly
                />
            </label>
            <label class="field admin-score-modal-full">
                <span>Ensembles</span>
                <input
                    value={activeInfoMusic.ensemble_names.join(", ") ||
                        "No ensemble"}
                    readonly
                />
            </label>
            <label class="field">
                <span>Stem files size</span>
                <input
                    value={`${formatBytes(activeInfoMusic.stems_total_bytes)} total`}
                    readonly
                />
            </label>
            <label class="field">
                <span>Uploaded</span>
                <input
                    value={prettyDate(activeInfoMusic.created_at)}
                    readonly
                />
            </label>
            <label class="field">
                <span>Stems status</span>
                <input value={activeInfoMusic.stems_status} readonly />
            </label>
            <label class="field">
                <span>Audio status</span>
                <input value={activeInfoMusic.audio_status} readonly />
            </label>
            <label class="field">
                <span>MIDI status</span>
                <input value={activeInfoMusic.midi_status} readonly />
            </label>
        </div>
        {#if activeInfoMusic.audio_error}
            <p class="hint">{activeInfoMusic.audio_error}</p>
        {/if}
        {#if activeInfoMusic.stems_error}
            <p class="hint">{activeInfoMusic.stems_error}</p>
        {/if}
        {#if activeInfoMusic.midi_error}
            <p class="hint">{activeInfoMusic.midi_error}</p>
        {/if}
        <div class="admin-score-links">
            <a
                href={activeInfoMusic.public_url}
                target="_blank"
                rel="noreferrer"
            >
                Random link
            </a>
            {#if activeInfoMusic.public_id_url}
                <a
                    href={activeInfoMusic.public_id_url}
                    target="_blank"
                    rel="noreferrer"
                >
                    Public id link
                </a>
            {/if}
        </div>
        {#if activeInfoMusic.stems_status !== "ready" && canEditOwnedScore(activeInfoMusic)}
            <div class="actions admin-user-modal-actions">
                <button
                    class="button ghost"
                    type="button"
                    disabled={retryingFor === activeInfoMusic.id}
                    onclick={() => void handleRetryRender(activeInfoMusic.id)}
                >
                    {retryingFor === activeInfoMusic.id
                        ? "Retrying render..."
                        : "Retry render"}
                </button>
            </div>
        {/if}
    </BaseModal>
{/if}

{#if confirmAction}
    <ConfirmModal
        title={confirmMessage}
        confirmLabel={confirmLabel}
        busy={confirmBusy}
        onConfirm={executeConfirm}
        onClose={closeConfirm}
    />
{/if}
