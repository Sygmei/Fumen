<script lang="ts">
    import type {
        AdminUpdateUserMultipartRequest,
        UserResponse as AppUser,
    } from "../adapters/fumen-backend/src/models";
    import { authenticatedApiClient } from "../lib/auth-client";
    import type { GlobalRole } from "../lib/roles";
    import CustomSelect from "../components/CustomSelect.svelte";
    import AdminRecordCard from "../components/AdminRecordCard.svelte";
    import {
        showConfirmModal,
        showModal,
    } from "../components/modals";
    import CreateUserModal from "../components/modals/CreateUserModal.svelte";
    import EditUserModal from "../components/modals/EditUserModal.svelte";
    import UserMetadataModal from "../components/modals/UserMetadataModal.svelte";
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
    import { prettyDate } from "../lib/utils";
    import type { UserEditDraft } from "../components/modals/types";

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

    let savingEditUserFor = $state("");

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
        showModal(CreateUserModal, {
            defaultRole: defaultCreateRole(currentUser),
            roleOptions: createRoleOptions,
            onCreate: handleCreateUser,
        });
    }

    async function handleCreateUser(
        username: string,
        role: Exclude<GlobalRole, "superadmin">,
    ) {
        try {
            const user = await authenticatedApiClient.adminCreateUser({
                username,
                role,
            });
            onUserCreated(user);
            onSuccess(`User ${user.username} created.`);
        } catch (error) {
            const message =
                error instanceof Error ? error.message : "Unable to create user";
            onError(message);
            throw error instanceof Error ? error : new Error(message);
        }
    }

    // Edit user
    function openUserEditModal(user: AppUser) {
        showModal(EditUserModal, {
            user,
            currentUser,
            roleOptions: createRoleOptions,
            onSave: (draft: UserEditDraft) => handleSaveUserEdit(user, draft),
        });
    }

    async function handleSaveUserEdit(
        user: AppUser,
        draft: UserEditDraft,
    ) {
        const originalUser = user;
        const optimisticUser: AppUser = {
            ...originalUser,
            display_name: draft.displayName || null,
            role: draft.role as GlobalRole,
            avatar_url: draft.clearAvatar
                ? null
                : draft.avatarPreview ?? originalUser.avatar_url,
        };
        savingEditUserFor = originalUser.id;
        onUserUpdated(optimisticUser);
        try {
            const payload = {
                role: draft.role,
                display_name: draft.displayName || undefined,
                avatar_file: draft.avatarFile ?? undefined,
                clear_avatar: draft.clearAvatar || undefined,
            } as unknown as AdminUpdateUserMultipartRequest;
            const updated = await authenticatedApiClient.adminUpdateUser(originalUser.id, payload);
            onUserUpdated(updated);
            onSuccess(`User ${updated.username} updated.`);
        } catch (error) {
            onUserUpdated(originalUser);
            const message =
                error instanceof Error ? error.message : "Failed to save user";
            onError(message);
            throw error instanceof Error ? error : new Error(message);
        } finally {
            savingEditUserFor = "";
        }
    }

    // User metadata
    async function loadUserMetadata(userId: string) {
        return authenticatedApiClient.adminUserMetadata(userId);
    }

    function openUserMetadataModal(user: AppUser) {
        if (!hasGlobalPower(currentUser)) return;
        showModal(UserMetadataModal, {
            user,
            loadMetadata: loadUserMetadata,
        });
    }

    async function deleteUserAccount(user: AppUser) {
        deletingUserFor = user.id;
        try {
            await authenticatedApiClient.adminDeleteUser(user.id);
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
        showConfirmModal({
            title: `Delete ${user.username}`,
            message: `Delete ${user.username} permanently?`,
            confirmText: "Delete",
            variant: "danger",
            onConfirm: () => deleteUserAccount(user),
        });
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
                            <div class="admin-user-state-row">
                                <p class="admin-user-role-pill">{user.role}</p>
                                {#if savingEditUserFor === user.id}
                                    <span class="status-pill admin-user-saving-pill"
                                        >Saving...</span
                                    >
                                {/if}
                            </div>
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
