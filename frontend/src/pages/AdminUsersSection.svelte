<script lang="ts">
    import {
        createUser,
        deleteUser,
        adminUpdateUser,
        createAdminUserLoginLink,
        fetchAdminUserMetadata,
        compressImageToJpeg,
        type AppUser,
        type AdminUserMetadata,
        type GlobalRole,
    } from "../lib/api";
    import BaseModal from "../components/BaseModal.svelte";
    import CustomSelect from "../components/CustomSelect.svelte";
    import ConfirmModal from "../components/ConfirmModal.svelte";
    import AdminRecordCard from "../components/AdminRecordCard.svelte";
    import {
        Search,
        Plus,
        QrCode,
        Trash2,
        Pencil,
        Shield,
        Users,
        User,
        Info,
    } from "@lucide/svelte";
    import {
        hasGlobalPower,
        canManageUsers,
        canDeleteUserAccount,
        isSuperadmin,
        allowedCreateRoles,
        defaultCreateRole,
    } from "../lib/admin-permissions";
    import { formatPlaytimeDuration, prettyDate } from "../lib/utils";

    let {
        currentUser,
        users,
        onUserCreated,
        onUserUpdated,
        onRefresh,
        onShowQr,
        onSuccess,
        onError,
    }: {
        currentUser: AppUser;
        users: AppUser[];
        onUserCreated: (user: AppUser) => void;
        onUserUpdated: (user: AppUser) => void;
        onRefresh: () => Promise<void>;
        onShowQr: (user: AppUser) => Promise<void>;
        onSuccess: (msg: string) => void;
        onError: (msg: string) => void;
    } = $props();

    // Search
    let userSearchQuery = $state("");

    // Create modal
    let showCreateUserModal = $state(false);
    let newUsername = $state("");
    let newUserRole = $state<Exclude<GlobalRole, "superadmin">>("user");
    let creatingUser = $state(false);

    // Edit modal
    let editingUser = $state<AppUser | null>(null);
    let editingDisplayName = $state("");
    let editingRole = $state<Exclude<GlobalRole, "superadmin">>("user");
    let editingAvatarFile = $state<File | null>(null);
    let editingAvatarPreview = $state<string | null>(null);
    let editingClearAvatar = $state(false);
    let savingEditUser = $state(false);
    let editUserError = $state("");

    // Metadata modal
    let metadataUser = $state<AppUser | null>(null);
    let metadataLoading = $state(false);
    let metadataError = $state("");
    let metadata = $state<AdminUserMetadata | null>(null);

    // Delete confirm
    let confirmMessage = $state("");
    let confirmLabel = $state("");
    let confirmAction = $state<(() => Promise<void>) | null>(null);
    let confirmBusy = $state(false);
    let deletingUserFor = $state("");

    // Filtered list
    const filteredUsers = $derived.by(() => {
        const query = userSearchQuery.trim().toLowerCase();
        const sorted = [...users].sort((a, b) =>
            a.username.localeCompare(b.username),
        );
        if (!query) return sorted;
        return sorted.filter((user) =>
            [user.username, user.role, prettyDate(user.created_at)].some((v) =>
                v.toLowerCase().includes(query),
            ),
        );
    });

    // Role option helpers
    function roleLabel(role: Exclude<GlobalRole, "superadmin">) {
        return (
            {
                admin: "Admin",
                manager: "Manager",
                editor: "Editor",
                user: "User",
            }[role] ?? "User"
        );
    }

    function roleDescription(role: Exclude<GlobalRole, "superadmin">) {
        if (role === "admin")
            return "Full access, except removing other admins.";
        if (role === "manager")
            return "Can create ensembles and users, and manage assigned ensembles.";
        if (role === "editor")
            return "Standard access plus score uploads on assigned ensembles.";
        return "Can sign in and listen to scores they can access.";
    }

    function roleIconComponent(role: Exclude<GlobalRole, "superadmin">) {
        if (role === "admin") return Shield;
        if (role === "manager") return Users;
        if (role === "editor") return Pencil;
        return User;
    }

    const createRoleOptions = $derived(
        allowedCreateRoles(currentUser).map((role) => ({
            value: role,
            label: roleLabel(role),
            description: roleDescription(role),
            iconComponent: roleIconComponent(role),
            tone: role,
        })),
    );

    // Create user
    function openCreateUserModal() {
        newUsername = "";
        newUserRole = defaultCreateRole(currentUser);
        showCreateUserModal = true;
    }

    function closeCreateUserModal() {
        if (creatingUser) return;
        showCreateUserModal = false;
        newUsername = "";
        newUserRole = defaultCreateRole(currentUser);
    }

    async function handleCreateUser() {
        const trimmed = newUsername.trim();
        if (!trimmed) {
            onError("Choose a username first.");
            return;
        }
        creatingUser = true;
        try {
            const user = await createUser(trimmed, newUserRole);
            onUserCreated(user);
            newUsername = "";
            newUserRole = defaultCreateRole(currentUser);
            showCreateUserModal = false;
            onSuccess(`User ${user.username} created.`);
        } catch (error) {
            onError(
                error instanceof Error
                    ? error.message
                    : "Unable to create user",
            );
        } finally {
            creatingUser = false;
        }
    }

    // Edit user
    function openUserEditModal(user: AppUser) {
        editingUser = user;
        editingDisplayName = user.display_name ?? "";
        editingRole = (
            user.role === "superadmin" ? "admin" : user.role
        ) as Exclude<GlobalRole, "superadmin">;
        editingAvatarFile = null;
        editingAvatarPreview = user.avatar_url;
        editingClearAvatar = false;
        editUserError = "";
    }

    function closeUserEditModal() {
        editingUser = null;
        editingAvatarFile = null;
        editingAvatarPreview = null;
    }

    const MAX_AVATAR_BYTES = 1 * 1024 * 1024;

    async function handleEditAvatarChange(event: Event) {
        const input = event.currentTarget as HTMLInputElement;
        const file = input.files?.[0];
        if (!file) return;
        if (file.size > MAX_AVATAR_BYTES) {
            editUserError = "Image must be under 1 MB.";
            input.value = "";
            return;
        }
        editUserError = "";
        const compressed = await compressImageToJpeg(file, 256);
        editingAvatarFile = compressed;
        editingClearAvatar = false;
        const reader = new FileReader();
        reader.onload = () => {
            editingAvatarPreview = reader.result as string;
        };
        reader.readAsDataURL(compressed);
    }

    function handleEditRemoveAvatar() {
        editingAvatarFile = null;
        editingClearAvatar = true;
        editingAvatarPreview = null;
    }

    async function handleSaveUserEdit() {
        if (!editingUser) return;
        savingEditUser = true;
        editUserError = "";
        try {
            const updated = await adminUpdateUser(editingUser.id, {
                role: editingRole,
                displayName: editingDisplayName.trim() || null,
                avatarFile: editingAvatarFile,
                clearAvatar: editingClearAvatar,
            });
            onUserUpdated(updated);
            closeUserEditModal();
            onSuccess(`User ${updated.username} updated.`);
        } catch (error) {
            editUserError =
                error instanceof Error ? error.message : "Failed to save.";
        } finally {
            savingEditUser = false;
        }
    }

    // User metadata
    function closeUserMetadataModal() {
        metadataUser = null;
        metadataLoading = false;
        metadataError = "";
        metadata = null;
    }

    async function loadUserMetadata(userId: string) {
        metadataLoading = true;
        metadataError = "";
        try {
            const response = await fetchAdminUserMetadata(userId);
            if (metadataUser?.id === userId) {
                metadata = response;
            }
        } catch (error) {
            if (metadataUser?.id === userId) {
                metadataError =
                    error instanceof Error
                        ? error.message
                        : "Unable to load metadata";
            }
        } finally {
            if (metadataUser?.id === userId) {
                metadataLoading = false;
            }
        }
    }

    function openUserMetadataModal(user: AppUser) {
        if (!hasGlobalPower(currentUser)) return;
        metadataUser = user;
        metadata = null;
        metadataError = "";
        metadataLoading = true;
        void loadUserMetadata(user.id);
    }

    // Delete user
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

    async function deleteUserAccount(user: AppUser) {
        deletingUserFor = user.id;
        try {
            await deleteUser(user.id);
            await onRefresh();
            onSuccess(`User ${user.username} deleted.`);
        } catch (error) {
            onError(
                error instanceof Error
                    ? error.message
                    : "Unable to delete user",
            );
        } finally {
            deletingUserFor = "";
        }
    }

    function handleDeleteUser(user: AppUser) {
        openConfirm(`Delete ${user.username} permanently?`, "Delete", () =>
            deleteUserAccount(user),
        );
    }

    // QR
    async function handleShowUserQr(user: AppUser) {
        try {
            await onShowQr(user);
            onSuccess(`QR code ready for ${user.username}.`);
        } catch (error) {
            onError(
                error instanceof Error
                    ? error.message
                    : "Unable to create QR code",
            );
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
                <h3>User accounts</h3>
            </div>
            <label class="field m-0 gap-0 min-w-0 self-center">
                <span class="sr-only">Search users</span>
                <div class="relative">
                    <Search
                        size={15}
                        class="absolute top-1/2 left-2.5 -translate-y-1/2 text-(--text-dim) pointer-events-none"
                        aria-hidden="true"
                    />
                    <input
                        bind:value={userSearchQuery}
                        placeholder="Search users"
                        class="!py-2 !px-3 !pl-8 !min-h-[38px]"
                    />
                </div>
            </label>
            {#if canManageUsers(currentUser)}
                <button
                    class="button admin-create-user-btn"
                    onclick={openCreateUserModal}
                >
                    <Plus size={15} aria-hidden="true" />
                    <span class="admin-create-label admin-create-label-full"
                        >Create user</span
                    >
                    <span class="admin-create-label admin-create-label-short"
                        >Create</span
                    >
                </button>
            {/if}
        </div>

        {#if users.length === 0}
            <div class="music-card"><p class="hint">No users yet.</p></div>
        {:else if filteredUsers.length === 0}
            <div class="music-card">
                <p class="hint">No users match "{userSearchQuery.trim()}".</p>
            </div>
        {:else}
            <div class="admin-user-scroll-area">
                <div
                    class="admin-user-list grid grid-cols-3 gap-2 items-start content-start max-[1360px]:grid-cols-2 max-[760px]:grid-cols-1"
                >
                    {#each filteredUsers as user}
                        {#snippet userAvatar()}
                            {#if user.avatar_url}
                                <img
                                    src={user.avatar_url}
                                    alt=""
                                    class="admin-user-avatar-img"
                                />
                            {:else}
                                {user.username.slice(0, 1).toUpperCase()}
                            {/if}
                        {/snippet}

                        {#snippet userMain()}
                            <h3>
                                {#if user.display_name}
                                    {user.display_name} —
                                    <span class="admin-user-handle"
                                        >@{user.username}</span
                                    >
                                {:else}
                                    @{user.username}
                                {/if}
                            </h3>
                            <p class="admin-user-role-pill">{user.role}</p>
                        {/snippet}

                        {#snippet userActions()}
                            {#if canDeleteUserAccount(user, currentUser) || isSuperadmin(currentUser)}
                                <button
                                    class="button secondary admin-user-action"
                                    type="button"
                                    onclick={() => openUserEditModal(user)}
                                    aria-label={`Edit ${user.username}`}
                                    title="Edit user"
                                    >
                                        <Pencil size={15} aria-hidden="true" />
                                    </button>
                            {/if}
                            {#if hasGlobalPower(currentUser)}
                                <button
                                    class="button secondary admin-user-action"
                                    type="button"
                                    onclick={() => openUserMetadataModal(user)}
                                    aria-label={`View metadata for ${user.username}`}
                                    title="User metadata"
                                >
                                    <Info size={15} aria-hidden="true" />
                                </button>
                            {/if}
                            <button
                                class="button secondary admin-user-action"
                                type="button"
                                onclick={() => void handleShowUserQr(user)}
                                aria-label={`Show QR code for ${user.username}`}
                                title="Show QR code"
                            >
                                <QrCode size={15} aria-hidden="true" />
                            </button>
                            {#if canDeleteUserAccount(user, currentUser)}
                                <button
                                    class="button ghost danger admin-user-action"
                                    type="button"
                                    disabled={deletingUserFor === user.id}
                                    onclick={() => handleDeleteUser(user)}
                                    aria-label={`Delete ${user.username}`}
                                    title="Delete user"
                                >
                                    <Trash2 size={16} aria-hidden="true" />
                                </button>
                            {/if}
                        {/snippet}

                        <AdminRecordCard
                            avatar={userAvatar}
                            main={userMain}
                            actions={userActions}
                        />
                    {/each}
                </div>
            </div>
        {/if}
    </div>
</section>

{#if showCreateUserModal}
    {#snippet createUserFooter()}
        <div class="actions admin-user-modal-actions">
            <button
                class="button ghost"
                type="button"
                disabled={creatingUser}
                onclick={closeCreateUserModal}>Cancel</button
            >
            <button
                class="button"
                type="button"
                disabled={creatingUser}
                onclick={() => void handleCreateUser()}
            >
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
                onkeydown={(e) => {
                    if (e.key === "Enter") void handleCreateUser();
                    else if (e.key === "Escape") closeCreateUserModal();
                }}
            />
        </label>
        <CustomSelect
            label="Global role"
            bind:value={newUserRole}
            options={createRoleOptions}
        />
    </BaseModal>
{/if}

{#if editingUser}
    <BaseModal
        title="Edit user"
        subtitle={editingUser.display_name ?? editingUser.username}
        size="medium"
        onClose={closeUserEditModal}
    >
        {#snippet children()}
            <form
                class="edit-user-form"
                onsubmit={(e) => {
                    e.preventDefault();
                    void handleSaveUserEdit();
                }}
            >
                <div class="edit-user-avatar-row">
                    <div class="edit-user-avatar-preview admin-user-avatar">
                        {#if editingAvatarPreview}
                            <img
                                src={editingAvatarPreview}
                                alt=""
                                class="admin-user-avatar-img"
                            />
                        {:else}
                            {editingUser.username.slice(0, 1).toUpperCase()}
                        {/if}
                    </div>
                    <div class="edit-user-avatar-actions">
                        <label
                            class="button secondary small edit-user-avatar-btn"
                        >
                            <Pencil size={13} aria-hidden="true" />
                            {editingAvatarPreview
                                ? "Change photo"
                                : "Upload photo"}
                            <input
                                type="file"
                                accept="image/jpeg,image/png,image/webp,image/gif"
                                onchange={handleEditAvatarChange}
                                style="display:none"
                            />
                        </label>
                        {#if editingAvatarPreview}
                            <button
                                type="button"
                                class="button ghost small"
                                onclick={handleEditRemoveAvatar}>Remove</button
                            >
                        {/if}
                    </div>
                </div>
                <div class="edit-user-field">
                    <label class="edit-user-label" for="edit-display-name"
                        >Display name</label
                    >
                    <input
                        id="edit-display-name"
                        class="admin-input"
                        type="text"
                        placeholder={editingUser.username}
                        bind:value={editingDisplayName}
                        maxlength={80}
                    />
                </div>
                {#if canDeleteUserAccount(editingUser, currentUser) || isSuperadmin(currentUser)}
                    <div class="edit-user-field">
                        <CustomSelect
                            label="Role"
                            bind:value={editingRole}
                            options={createRoleOptions}
                            onValueChange={(role) => {
                                editingRole = role as Exclude<
                                    GlobalRole,
                                    "superadmin"
                                >;
                            }}
                        />
                    </div>
                {/if}
                {#if editUserError}
                    <p class="admin-error">{editUserError}</p>
                {/if}
            </form>
        {/snippet}
        {#snippet footer()}
            <div class="actions admin-user-modal-actions">
                <button
                    class="button ghost"
                    type="button"
                    onclick={closeUserEditModal}
                    disabled={savingEditUser}>Cancel</button
                >
                <button
                    class="button"
                    type="button"
                    onclick={() => void handleSaveUserEdit()}
                    disabled={savingEditUser}
                    >{savingEditUser ? "Saving…" : "Save"}</button
                >
            </div>
        {/snippet}
    </BaseModal>
{/if}

{#if metadataUser}
    <BaseModal
        title="User metadata"
        subtitle={metadataUser.display_name ?? `@${metadataUser.username}`}
        size="large"
        onClose={closeUserMetadataModal}
        cardClass="admin-user-modal admin-user-metadata-modal"
    >
        <div class="admin-user-metadata-summary">
            <article class="admin-user-metadata-stat">
                <span>Last login</span>
                <strong>
                    {metadata?.last_login_at
                        ? prettyDate(metadata.last_login_at)
                        : metadataLoading
                            ? "Loading..."
                            : "Never"}
                </strong>
            </article>
            <article class="admin-user-metadata-stat">
                <span>Total playtime</span>
                <strong>
                    {metadata
                        ? formatPlaytimeDuration(metadata.total_playtime_seconds)
                        : metadataLoading
                            ? "Loading..."
                            : "0s"}
                </strong>
            </article>
            <article class="admin-user-metadata-stat">
                <span>Scores played</span>
                <strong>
                    {metadata
                        ? metadata.score_playtimes.length
                        : metadataLoading
                            ? "Loading..."
                            : 0}
                </strong>
            </article>
        </div>

        {#if metadataLoading}
            <p class="hint">Loading metadata...</p>
        {:else if metadataError}
            <p class="status error">{metadataError}</p>
        {:else if metadata}
            <section class="admin-user-metadata-section">
                <div class="admin-playtime-header">
                    <div>
                        <p class="meta-label">Scores</p>
                        <h3>Playtime by score</h3>
                    </div>
                </div>

                {#if metadata.score_playtimes.length === 0}
                    <p class="hint">No score playtime has been recorded yet.</p>
                {:else}
                    <div class="admin-user-metadata-score-list">
                        {#each metadata.score_playtimes as score}
                            <a
                                class="admin-user-metadata-score-row"
                                href={score.public_url}
                                target="_blank"
                                rel="noreferrer"
                            >
                                <span class="admin-user-metadata-score-icon" aria-hidden="true">
                                    {#if score.icon_image_url}
                                        <img
                                            src={score.icon_image_url}
                                            alt=""
                                            class="admin-user-metadata-score-icon-img"
                                        />
                                    {:else}
                                        {score.icon ?? ""}
                                    {/if}
                                </span>
                                <span class="admin-user-metadata-score-copy">
                                    <strong>{score.title}</strong>
                                    <span class="subtle">Public score link</span>
                                </span>
                                <span class="status-pill">
                                    {formatPlaytimeDuration(score.total_seconds)}
                                </span>
                            </a>
                        {/each}
                    </div>
                {/if}
            </section>
        {/if}
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
