<script lang="ts">
    import {
        createEnsemble,
        deleteEnsemble,
        addUserToEnsemble,
        removeUserFromEnsemble,
        addMusicToEnsemble,
        removeMusicFromEnsemble,
        type AdminMusic,
        type AppUser,
        type Ensemble,
        type EnsembleRole,
    } from "../lib/api";
    import BaseModal from "../components/BaseModal.svelte";
    import CustomSelect from "../components/CustomSelect.svelte";
    import ConfirmModal from "../components/ConfirmModal.svelte";
    import AdminRecordCard from "../components/AdminRecordCard.svelte";
    import {
        Search,
        Plus,
        Trash2,
        UserPlus,
        Music,
        Users,
        Pencil,
        User,
    } from "@lucide/svelte";
    import {
        canCreateEnsembles,
        canManageEnsembleMembers,
        canDeleteEnsembleRecord,
        canManageEnsembleScores,
        allowedEnsembleRolesForUser,
    } from "../lib/admin-permissions";

    type ManagedMemberDraftRole = "none" | EnsembleRole;

    let {
        currentUser,
        ensembles,
        allUsers,
        allMusics,
        onEnsembleCreated,
        onRefresh,
        onSuccess,
        onError,
    }: {
        currentUser: AppUser;
        ensembles: Ensemble[];
        allUsers: AppUser[];
        allMusics: AdminMusic[];
        onEnsembleCreated: (ensemble: Ensemble) => void;
        onRefresh: () => Promise<void>;
        onSuccess: (msg: string) => void;
        onError: (msg: string) => void;
    } = $props();

    // Search
    let ensembleSearchQuery = $state("");

    // Create modal
    let showCreateEnsembleModal = $state(false);
    let newEnsembleName = $state("");
    let creatingEnsemble = $state(false);
    let deletingEnsembleFor = $state("");

    // Manage members
    let managingEnsembleId = $state("");
    let currentMemberSearchQuery = $state("");
    let addMemberSearchQuery = $state("");
    let inviteRoles = $state<Record<string, EnsembleRole>>({});
    let originalManagedMemberRoles = $state<
        Record<string, ManagedMemberDraftRole>
    >({});
    let managedMemberDraftRoles = $state<
        Record<string, ManagedMemberDraftRole>
    >({});
    let savingManagedMembers = $state(false);

    // Manage ensemble scores
    let managingEnsembleScoresId = $state("");
    let currentEnsembleScoreSearchQuery = $state("");
    let addEnsembleScoreSearchQuery = $state("");
    let originalEnsembleScoreMusicIds = $state<string[]>([]);
    let stagedEnsembleScoreMusicIds = $state<string[]>([]);
    let savingEnsembleScores = $state(false);

    // Confirm
    let confirmMessage = $state("");
    let confirmLabel = $state("");
    let confirmAction = $state<(() => Promise<void>) | null>(null);
    let confirmBusy = $state(false);

    // Derived
    const filteredEnsembles = $derived.by(() => {
        const query = ensembleSearchQuery.trim().toLowerCase();
        const sorted = [...ensembles].sort((a, b) =>
            a.name.localeCompare(b.name),
        );
        if (!query) return sorted;
        return sorted.filter((e) =>
            [
                e.name,
                `${e.members.length} members`,
                `${e.score_count} scores`,
            ].some((v) => v.toLowerCase().includes(query)),
        );
    });

    const activeManagedEnsemble = $derived(
        ensembles.find((e) => e.id === managingEnsembleId) ?? null,
    );

    const activeManagedScoreEnsemble = $derived(
        ensembles.find((e) => e.id === managingEnsembleScoresId) ?? null,
    );

    function managedMemberRoleForUser(userId: string): ManagedMemberDraftRole {
        return managedMemberDraftRoles[userId] ?? "none";
    }

    const filteredManagedMembers = $derived.by(() => {
        if (!activeManagedEnsemble) return [];
        const query = currentMemberSearchQuery.trim().toLowerCase();
        return allUsers
            .map((user) => ({
                user_id: user.id,
                role: managedMemberRoleForUser(user.id),
                user,
            }))
            .filter(
                (
                    e,
                ): e is {
                    user_id: string;
                    role: EnsembleRole;
                    user: AppUser;
                } => e.role !== "none",
            )
            .sort((a, b) => a.user.username.localeCompare(b.user.username))
            .filter((e) =>
                !query
                    ? true
                    : [e.user.username, e.role]
                          .join(" ")
                          .toLowerCase()
                          .includes(query),
            );
    });

    const filteredAvailableEnsembleUsers = $derived.by(() => {
        if (!activeManagedEnsemble) return [];
        const query = addMemberSearchQuery.trim().toLowerCase();
        return [...allUsers]
            .filter(
                (u) =>
                    managedMemberRoleForUser(u.id) === "none" &&
                    allowedEnsembleRolesForUser(u, currentUser).length > 0,
            )
            .sort((a, b) => a.username.localeCompare(b.username))
            .filter((u) =>
                !query
                    ? true
                    : [u.username, u.role]
                          .join(" ")
                          .toLowerCase()
                          .includes(query),
            );
    });

    const filteredManagedEnsembleScores = $derived.by(() => {
        if (!activeManagedScoreEnsemble) return [];
        const query = currentEnsembleScoreSearchQuery.trim().toLowerCase();
        return [...allMusics]
            .filter((m) => stagedEnsembleScoreMusicIds.includes(m.id))
            .sort((a, b) => a.title.localeCompare(b.title))
            .filter((m) =>
                !query
                    ? true
                    : [m.title, m.public_id ?? "", ...m.ensemble_names]
                          .join(" ")
                          .toLowerCase()
                          .includes(query),
            );
    });

    const filteredAvailableEnsembleScores = $derived.by(() => {
        if (!activeManagedScoreEnsemble) return [];
        const query = addEnsembleScoreSearchQuery.trim().toLowerCase();
        return [...allMusics]
            .filter((m) => !stagedEnsembleScoreMusicIds.includes(m.id))
            .sort((a, b) => a.title.localeCompare(b.title))
            .filter((m) =>
                !query
                    ? true
                    : [m.title, m.public_id ?? "", ...m.ensemble_names]
                          .join(" ")
                          .toLowerCase()
                          .includes(query),
            );
    });

    // Role helpers
    function memberRoleLabel(role: EnsembleRole) {
        if (role === "manager") return "Manager";
        if (role === "editor") return "Editor";
        return "Member";
    }

    function ensembleRoleDescription(role: EnsembleRole) {
        if (role === "manager")
            return "Can manage ensemble members and score access.";
        if (role === "editor")
            return "Can add scores and manage only their own uploads.";
        return "Can access scores shared with the ensemble.";
    }

    function ensembleRoleIconComponent(role: EnsembleRole) {
        if (role === "manager") return Users;
        if (role === "editor") return Pencil;
        return User;
    }

    function ensembleRoleOptionsForUser(user: AppUser) {
        return allowedEnsembleRolesForUser(user, currentUser).map((role) => ({
            value: role,
            label: memberRoleLabel(role),
            description: ensembleRoleDescription(role),
            iconComponent: ensembleRoleIconComponent(role),
            tone: role,
        }));
    }

    // Confirm helpers
    function openConfirm(
        msg: string,
        label: string,
        action: () => Promise<void>,
    ) {
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

    // Create ensemble
    function openCreateEnsembleModal() {
        newEnsembleName = "";
        showCreateEnsembleModal = true;
    }

    function closeCreateEnsembleModal() {
        if (creatingEnsemble) return;
        showCreateEnsembleModal = false;
        newEnsembleName = "";
    }

    async function handleCreateEnsemble() {
        const trimmed = newEnsembleName.trim();
        if (!trimmed) {
            onError("Choose an ensemble name first.");
            return;
        }
        creatingEnsemble = true;
        try {
            const ensemble = await createEnsemble(trimmed);
            onEnsembleCreated(ensemble);
            newEnsembleName = "";
            showCreateEnsembleModal = false;
            onSuccess(`Ensemble ${ensemble.name} created.`);
        } catch (error) {
            onError(
                error instanceof Error
                    ? error.message
                    : "Unable to create ensemble",
            );
        } finally {
            creatingEnsemble = false;
        }
    }

    // Delete ensemble
    async function deleteEnsembleAccount(ensemble: Ensemble) {
        deletingEnsembleFor = ensemble.id;
        try {
            await deleteEnsemble(ensemble.id);
            await onRefresh();
            onSuccess(`Ensemble ${ensemble.name} deleted.`);
        } catch (error) {
            onError(
                error instanceof Error
                    ? error.message
                    : "Unable to delete ensemble",
            );
        } finally {
            deletingEnsembleFor = "";
        }
    }

    function handleDeleteEnsemble(ensemble: Ensemble) {
        openConfirm(`Delete ensemble ${ensemble.name}?`, "Delete", () =>
            deleteEnsembleAccount(ensemble),
        );
    }

    // Manage members
    function buildEnsembleMemberRoleMap(ensemble: Ensemble) {
        return Object.fromEntries(
            ensemble.members.map((m) => [m.user_id, m.role]),
        ) as Record<string, ManagedMemberDraftRole>;
    }

    function openManageMembersModal(ensemble: Ensemble) {
        managingEnsembleId = ensemble.id;
        currentMemberSearchQuery = "";
        addMemberSearchQuery = "";
        originalManagedMemberRoles = buildEnsembleMemberRoleMap(ensemble);
        managedMemberDraftRoles = buildEnsembleMemberRoleMap(ensemble);
        inviteRoles = {};
        savingManagedMembers = false;
    }

    function closeManageMembersModal() {
        if (savingManagedMembers) return;
        managingEnsembleId = "";
        currentMemberSearchQuery = "";
        addMemberSearchQuery = "";
        originalManagedMemberRoles = {};
        managedMemberDraftRoles = {};
        inviteRoles = {};
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

    function hasManagedMemberChanges(): boolean {
        const keys = new Set([
            ...Object.keys(originalManagedMemberRoles),
            ...Object.keys(managedMemberDraftRoles),
        ]);
        for (const userId of keys) {
            if (
                (originalManagedMemberRoles[userId] ?? "none") !==
                (managedMemberDraftRoles[userId] ?? "none")
            )
                return true;
        }
        return false;
    }

    async function saveManagedEnsembleChanges() {
        const ensembleId = managingEnsembleId;
        if (!ensembleId || !hasManagedMemberChanges()) return;
        savingManagedMembers = true;
        try {
            const keys = new Set([
                ...Object.keys(originalManagedMemberRoles),
                ...Object.keys(managedMemberDraftRoles),
            ]);
            for (const userId of keys) {
                const orig = originalManagedMemberRoles[userId] ?? "none";
                const next = managedMemberDraftRoles[userId] ?? "none";
                if (orig === next) continue;
                if (next === "none")
                    await removeUserFromEnsemble(ensembleId, userId);
                else await addUserToEnsemble(ensembleId, userId, next);
            }
            await onRefresh();
            originalManagedMemberRoles = { ...managedMemberDraftRoles };
            onSuccess("Ensemble members updated.");
        } catch (error) {
            onError(
                error instanceof Error
                    ? error.message
                    : "Unable to update ensemble members",
            );
        } finally {
            savingManagedMembers = false;
        }
    }

    async function handleAddUserToManagedEnsemble(userId: string) {
        stageManagedMemberRole(
            userId,
            inviteRoles[userId] ?? ("user" as EnsembleRole),
        );
    }

    // Manage ensemble scores
    function openManageScoresModal(ensemble: Ensemble) {
        managingEnsembleScoresId = ensemble.id;
        currentEnsembleScoreSearchQuery = "";
        addEnsembleScoreSearchQuery = "";
        originalEnsembleScoreMusicIds = allMusics
            .filter((m) => m.ensemble_ids.includes(ensemble.id))
            .map((m) => m.id);
        stagedEnsembleScoreMusicIds = [...originalEnsembleScoreMusicIds];
        savingEnsembleScores = false;
    }

    function closeManageScoresModal() {
        if (savingEnsembleScores) return;
        managingEnsembleScoresId = "";
        currentEnsembleScoreSearchQuery = "";
        addEnsembleScoreSearchQuery = "";
        originalEnsembleScoreMusicIds = [];
        stagedEnsembleScoreMusicIds = [];
    }

    function toggleStagedEnsembleScore(musicId: string, shouldAdd: boolean) {
        if (shouldAdd) {
            stagedEnsembleScoreMusicIds = [
                ...stagedEnsembleScoreMusicIds,
                musicId,
            ];
        } else {
            stagedEnsembleScoreMusicIds = stagedEnsembleScoreMusicIds.filter(
                (id) => id !== musicId,
            );
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
        try {
            for (const id of toAdd)
                await addMusicToEnsemble(id, managingEnsembleScoresId);
            for (const id of toRemove)
                await removeMusicFromEnsemble(id, managingEnsembleScoresId);
            await onRefresh();
            onSuccess("Ensemble scores updated.");
            closeManageScoresModal();
        } catch (error) {
            onError(
                error instanceof Error
                    ? error.message
                    : "Failed to update scores",
            );
        } finally {
            savingEnsembleScores = false;
        }
    }
</script>

<section
    class="grid gap-2 h-full min-h-full min-w-0 px-2 pt-2 pb-12 overflow-hidden"
>
    <div
        class="grid grid-rows-[auto_minmax(0,1fr)] gap-2 h-full min-h-0 overflow-hidden"
    >
        <div
            class="admin-section-toolbar music-card grid grid-cols-[auto_minmax(0,1fr)_auto] gap-3 items-center !p-3 !px-4"
        >
            <div class="admin-section-heading flex items-center min-h-full">
                <h3>Ensembles</h3>
            </div>
            <label class="field m-0 gap-0 min-w-0 self-center">
                <span class="sr-only">Search ensembles</span>
                <div class="relative">
                    <Search
                        size={15}
                        class="absolute top-1/2 left-2.5 -translate-y-1/2 text-(--text-dim) pointer-events-none"
                        aria-hidden="true"
                    />
                    <input
                        bind:value={ensembleSearchQuery}
                        placeholder="Search ensembles"
                        class="!py-2 !px-3 !pl-8 !min-h-[38px]"
                    />
                </div>
            </label>
            {#if canCreateEnsembles(currentUser)}
                <button
                    class="button admin-create-user-btn"
                    type="button"
                    onclick={openCreateEnsembleModal}
                >
                    <Plus size={15} aria-hidden="true" />
                    <span class="admin-create-label admin-create-label-full"
                        >Create ensemble</span
                    >
                    <span class="admin-create-label admin-create-label-short"
                        >Create</span
                    >
                </button>
            {/if}
        </div>

        {#if ensembles.length === 0}
            <div class="music-card"><p class="hint">No ensembles yet.</p></div>
        {:else if filteredEnsembles.length === 0}
            <div class="music-card">
                <p class="hint">
                    No ensembles match "{ensembleSearchQuery.trim()}".
                </p>
            </div>
        {:else}
            <div class="admin-ensemble-scroll-area">
                <div
                    class="grid grid-cols-3 gap-2 items-start content-start max-[1360px]:grid-cols-2 max-[760px]:grid-cols-1"
                >
                    {#each filteredEnsembles as ensemble}
                        {#snippet ensembleAvatar()}
                            <Users size={16} aria-hidden="true" />
                        {/snippet}

                        {#snippet ensembleMain()}
                            <h3>{ensemble.name}</h3>
                        {/snippet}

                        {#snippet ensembleActions()}
                            <button
                                class="button secondary admin-user-action"
                                type="button"
                                onclick={() => openManageScoresModal(ensemble)}
                                aria-label={`Manage scores for ${ensemble.name}`}
                                title="Manage scores"
                                disabled={!canManageEnsembleScores(
                                    ensemble,
                                    currentUser,
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
                                onclick={() => openManageMembersModal(ensemble)}
                                aria-label={`Manage members for ${ensemble.name}`}
                                title="Manage members"
                                disabled={!canManageEnsembleMembers(
                                    ensemble,
                                    currentUser,
                                )}
                            >
                                <UserPlus size={16} aria-hidden="true" />
                                <span
                                    class="admin-action-badge"
                                    aria-hidden="true"
                                    >{ensemble.members.length}</span
                                >
                            </button>
                            {#if canDeleteEnsembleRecord(ensemble, currentUser)}
                                <button
                                    class="button ghost danger admin-user-action"
                                    type="button"
                                    disabled={deletingEnsembleFor ===
                                        ensemble.id}
                                    onclick={() =>
                                        void handleDeleteEnsemble(ensemble)}
                                    aria-label={`Delete ${ensemble.name}`}
                                    title="Delete ensemble"
                                >
                                    <Trash2 size={16} aria-hidden="true" />
                                </button>
                            {/if}
                        {/snippet}

                        <AdminRecordCard
                            avatar={ensembleAvatar}
                            main={ensembleMain}
                            actions={ensembleActions}
                        />
                    {/each}
                </div>
            </div>
        {/if}
    </div>
</section>

{#if showCreateEnsembleModal}
    {#snippet createEnsembleFooter()}
        <div class="actions admin-user-modal-actions">
            <button
                class="button ghost"
                type="button"
                disabled={creatingEnsemble}
                onclick={closeCreateEnsembleModal}>Cancel</button
            >
            <button
                class="button"
                type="button"
                disabled={creatingEnsemble}
                onclick={() => void handleCreateEnsemble()}
            >
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
                onkeydown={(e) => {
                    if (e.key === "Enter") void handleCreateEnsemble();
                    else if (e.key === "Escape") closeCreateEnsembleModal();
                }}
            />
        </label>
    </BaseModal>
{/if}

{#if activeManagedEnsemble}
    {#snippet manageMembersFooter()}
        <div class="actions admin-user-modal-actions">
            <button
                class="button ghost"
                type="button"
                disabled={savingManagedMembers}
                onclick={closeManageMembersModal}>Cancel</button
            >
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
                                    {#if allowedEnsembleRolesForUser(member.user!, currentUser).length > 0}
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
                                                    currentUser,
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
                                                    [user.id]:
                                                        role as EnsembleRole,
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
                onclick={closeManageScoresModal}>Cancel</button
            >
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

{#if confirmAction}
    <ConfirmModal
        title={confirmMessage}
        {confirmLabel}
        busy={confirmBusy}
        onConfirm={executeConfirm}
        onClose={closeConfirm}
    />
{/if}
