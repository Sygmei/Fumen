<script lang="ts">
    import type {
        AdminUpdateUserMultipartRequest,
        UserResponse as AppUser,
    } from "$backend/models";
    import { authenticatedApiClient } from "$lib/auth-client";
    import type { GlobalRole } from "$lib/roles";
    import AdminUserCard from "$components/admin/AdminUserCard.svelte";
    import {
        showConfirmModal,
        showModal,
    } from "$components/modals";
    import CreateUserModal from "$components/modals/CreateUserModal.svelte";
    import EditUserModal from "$components/modals/EditUserModal.svelte";
    import UserMetadataModal from "$components/modals/UserMetadataModal.svelte";
    import {
        Search,
        Plus,
        Pencil,
        Shield,
        Users,
        User,
    } from "@lucide/svelte";
    import {
        hasGlobalPower,
        canManageUsers,
        canDeleteUserAccount,
        isSuperadmin,
        allowedCreateRoles,
        defaultCreateRole,
        canUseUsersSection,
    } from "$lib/admin-permissions";
    import { prettyDate } from "$lib/utils";
    import type { UserEditDraft } from "$components/modals/types";
    import { appShell } from "$lib/app-shell.svelte";
    import { getAdminStateContext } from "$lib/admin-state.svelte";

    const adminState = getAdminStateContext();

    let userSearchQuery = $state("");
    let creatingUserIds = $state<string[]>([]);
    let savingEditUserIds = $state<string[]>([]);
    let deletingUserIds = $state<string[]>([]);

    const filteredUsers = $derived.by(() => {
        const query = userSearchQuery.trim().toLowerCase();
        const sorted = [...adminState.adminUsers].sort((a, b) =>
            a.username.localeCompare(b.username),
        );
        if (!query) return sorted;
        return sorted.filter((user) =>
            [user.username, user.role, prettyDate(user.created_at)].some((v) =>
                v.toLowerCase().includes(query),
            ),
        );
    });

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
        if (role === "admin") return "Full access, except removing other admins.";
        if (role === "manager") {
            return "Can create ensembles and users, and manage assigned ensembles.";
        }
        if (role === "editor") {
            return "Standard access plus score uploads on assigned ensembles.";
        }
        return "Can sign in and listen to scores they can access.";
    }

    function roleIconComponent(role: Exclude<GlobalRole, "superadmin">) {
        if (role === "admin") return Shield;
        if (role === "manager") return Users;
        if (role === "editor") return Pencil;
        return User;
    }

    const createRoleOptions = $derived(
        allowedCreateRoles(appShell.currentUser).map((role) => ({
            value: role,
            label: roleLabel(role),
            description: roleDescription(role),
            iconComponent: roleIconComponent(role),
            tone: role,
        })),
    );

    function openCreateUserModal() {
        if (!appShell.currentUser) return;
        showModal(CreateUserModal, {
            defaultRole: defaultCreateRole(appShell.currentUser),
            roleOptions: createRoleOptions,
            onCreate: handleCreateUser,
        });
    }

    function buildPendingUser(
        id: string,
        username: string,
        role: Exclude<GlobalRole, "superadmin">,
    ): AppUser {
        return {
            id,
            username,
            role,
            created_at: new Date().toISOString(),
            created_by_user_id: appShell.currentUser?.id ?? null,
            display_name: null,
            avatar_url: null,
            editable_ensemble_ids: [],
            managed_ensemble_ids: [],
        };
    }

    async function handleCreateUser(
        username: string,
        role: Exclude<GlobalRole, "superadmin">,
    ) {
        const optimisticId = `creating-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
        const optimisticUser = buildPendingUser(optimisticId, username, role);
        creatingUserIds = [...creatingUserIds, optimisticId];
        adminState.addUser(optimisticUser);

        try {
            const user = await authenticatedApiClient.adminCreateUser({
                username,
                role,
            });
            adminState.removeUser(optimisticId);
            adminState.addUser(user);
            adminState.setSuccess(`User ${user.username} created.`);
        } catch (error) {
            adminState.removeUser(optimisticId);
            const message =
                error instanceof Error ? error.message : "Unable to create user";
            adminState.setError(message);
        } finally {
            creatingUserIds = creatingUserIds.filter((id) => id !== optimisticId);
        }
    }

    function openUserEditModal(user: AppUser) {
        if (!appShell.currentUser) return;
        showModal(EditUserModal, {
            user,
            currentUser: appShell.currentUser,
            roleOptions: createRoleOptions,
            onSave: (draft: UserEditDraft) => handleSaveUserEdit(user, draft),
        });
    }

    async function handleSaveUserEdit(user: AppUser, draft: UserEditDraft) {
        const originalUser = user;
        const optimisticUser: AppUser = {
            ...originalUser,
            display_name: draft.displayName || null,
            role: draft.role as GlobalRole,
            avatar_url: draft.clearAvatar
                ? null
                : draft.avatarPreview ?? originalUser.avatar_url,
        };
        savingEditUserIds = [...savingEditUserIds, originalUser.id];
        adminState.updateUser(optimisticUser);
        try {
            const payload = {
                role: draft.role,
                display_name: draft.displayName || undefined,
                avatar_file: draft.avatarFile ?? undefined,
                clear_avatar: draft.clearAvatar || undefined,
            } as unknown as AdminUpdateUserMultipartRequest;
            const updated = await authenticatedApiClient.adminUpdateUser(
                originalUser.id,
                payload,
            );
            adminState.updateUser(updated);
            adminState.setSuccess(`User ${updated.username} updated.`);
        } catch (error) {
            adminState.updateUser(originalUser);
            const message =
                error instanceof Error ? error.message : "Failed to save user";
            adminState.setError(message);
        } finally {
            savingEditUserIds = savingEditUserIds.filter((id) => id !== originalUser.id);
        }
    }

    async function loadUserMetadata(userId: string) {
        return authenticatedApiClient.adminUserMetadata(userId);
    }

    function openUserMetadataModal(user: AppUser) {
        if (!appShell.currentUser || !hasGlobalPower(appShell.currentUser)) return;
        showModal(UserMetadataModal, {
            user,
            loadMetadata: loadUserMetadata,
        });
    }

    async function deleteUserAccount(user: AppUser) {
        deletingUserIds = [...deletingUserIds, user.id];
        try {
            await authenticatedApiClient.adminDeleteUser(user.id);
            await adminState.refresh();
            adminState.setSuccess(`User ${user.username} deleted.`);
        } catch (error) {
            adminState.setError(
                error instanceof Error ? error.message : "Unable to delete user",
            );
        } finally {
            deletingUserIds = deletingUserIds.filter((id) => id !== user.id);
        }
    }

    function handleDeleteUser(user: AppUser) {
        showConfirmModal({
            title: `Delete ${user.username}`,
            message: `Delete ${user.username} permanently?`,
            confirmText: "Delete",
            variant: "danger",
            onConfirm: () => {
                void deleteUserAccount(user);
            },
        });
    }

    async function handleShowUserQr(user: AppUser) {
        try {
            await appShell.openCredentialModal(
                `QR code for ${user.username}`,
                () => authenticatedApiClient.adminCreateUserLoginLink(user.id),
            );
            adminState.setSuccess(`QR code ready for ${user.username}.`);
        } catch (error) {
            adminState.setError(
                error instanceof Error
                    ? error.message
                    : "Unable to create QR code",
            );
        }
    }
</script>

{#if appShell.currentUser && canUseUsersSection(appShell.currentUser)}
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
                {#if canManageUsers(appShell.currentUser)}
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

            {#if adminState.adminUsers.length === 0}
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
                            <AdminUserCard
                                {user}
                                creating={creatingUserIds.includes(user.id)}
                                saving={savingEditUserIds.includes(user.id)}
                                deleting={deletingUserIds.includes(user.id)}
                                canEdit={canDeleteUserAccount(user, appShell.currentUser) ||
                                    isSuperadmin(appShell.currentUser)}
                                canViewMetadata={hasGlobalPower(appShell.currentUser)}
                                canDelete={canDeleteUserAccount(user, appShell.currentUser)}
                                onEdit={() => openUserEditModal(user)}
                                onShowMetadata={() => openUserMetadataModal(user)}
                                onShowQr={() => void handleShowUserQr(user)}
                                onDelete={() => handleDeleteUser(user)}
                            />
                        {/each}
                    </div>
                </div>
            {/if}
        </div>
    </section>
{/if}
