<script lang="ts">
    import {
        Search,
        User,
        Users,
        Pencil,
        UserCog,
    } from "@lucide/svelte";
    import type {
        AdminEnsembleResponse as Ensemble,
        UserResponse as AppUser,
    } from "../../adapters/fumen-backend/src/models";
    import { allowedEnsembleRolesForUser } from "../../lib/admin-permissions";
    import type { EnsembleRole } from "../../lib/roles";
    import BaseModal from "./BaseModal.svelte";
    import CustomSelect from "../CustomSelect.svelte";
    import EnsembleRolePickerModal from "./EnsembleRolePickerModal.svelte";
    import { closeModal, pushModal } from "./modalState";
    import type {
        EnsembleMemberAssignment,
        EnsembleRoleOption,
    } from "./types";

    type ManagedMemberDraftRole = "none" | EnsembleRole;

    let {
        ensemble,
        allUsers,
        currentUser,
        onSave,
        modalId,
    }: {
        ensemble: Ensemble;
        allUsers: AppUser[];
        currentUser: AppUser;
        onSave: (
            ensembleId: string,
            members: EnsembleMemberAssignment[],
        ) => Promise<void>;
        modalId?: string;
    } = $props();

    let currentMemberSearchQuery = $state("");
    let addMemberSearchQuery = $state("");
    let inviteRoles = $state<Record<string, EnsembleRole>>({});
    let originalManagedMemberRoles = $state(
        Object.fromEntries(
            ensemble.members.map((member) => [member.user_id, member.role]),
        ) as Record<string, ManagedMemberDraftRole>,
    );
    let managedMemberDraftRoles = $state({ ...originalManagedMemberRoles });
    let saving = $state(false);
    let errorMsg = $state("");

    function memberRoleLabel(role: EnsembleRole) {
        if (role === "manager") return "Manager";
        if (role === "editor") return "Editor";
        return "Member";
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

    function ensembleRoleIconComponent(role: EnsembleRole) {
        if (role === "manager") return Users;
        if (role === "editor") return Pencil;
        return User;
    }

    function ensembleRoleOptionsForUser(user: AppUser): EnsembleRoleOption[] {
        return allowedEnsembleRolesForUser(user, currentUser).map((role) => ({
            value: role,
            label: memberRoleLabel(role),
            description: ensembleRoleDescription(role),
            iconComponent: ensembleRoleIconComponent(role),
            tone: role,
        }));
    }

    function managedMemberRoleForUser(userId: string): ManagedMemberDraftRole {
        return managedMemberDraftRoles[userId] ?? "none";
    }

    const filteredManagedMembers = $derived.by(() =>
        allUsers
            .map((user) => ({
                user_id: user.id,
                role: managedMemberRoleForUser(user.id),
                user,
            }))
            .filter(
                (
                    member,
                ): member is {
                    user_id: string;
                    role: EnsembleRole;
                    user: AppUser;
                } => member.role !== "none",
            )
            .sort((left, right) =>
                left.user.username.localeCompare(right.user.username),
            )
            .filter((member) => {
                const query = currentMemberSearchQuery.trim().toLowerCase();
                if (!query) return true;
                return [member.user.username, member.role]
                    .join(" ")
                    .toLowerCase()
                    .includes(query);
            }),
    );

    const filteredAvailableEnsembleUsers = $derived.by(() =>
        [...allUsers]
            .filter(
                (user) =>
                    managedMemberRoleForUser(user.id) === "none" &&
                    allowedEnsembleRolesForUser(user, currentUser).length > 0,
            )
            .sort((left, right) => left.username.localeCompare(right.username))
            .filter((user) => {
                const query = addMemberSearchQuery.trim().toLowerCase();
                if (!query) return true;
                return [user.username, user.role]
                    .join(" ")
                    .toLowerCase()
                    .includes(query);
            }),
    );

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
            ) {
                return true;
            }
        }
        return false;
    }

    function inviteRoleForUser(user: AppUser): EnsembleRole {
        return (
            inviteRoles[user.id] ??
            allowedEnsembleRolesForUser(user, currentUser)[0] ??
            "user"
        );
    }

    function openRolePicker(
        kind: "member" | "invite",
        user: AppUser,
        role: EnsembleRole,
    ) {
        pushModal({
            modal: EnsembleRolePickerModal,
            props: {
                subtitle: user.display_name ?? `@${user.username}`,
                initialValue: role,
                options: ensembleRoleOptionsForUser(user),
                onApply: (nextRole: EnsembleRole) => {
                    if (kind === "member") {
                        stageManagedMemberRole(user.id, nextRole);
                    } else {
                        inviteRoles = {
                            ...inviteRoles,
                            [user.id]: nextRole,
                        };
                    }
                },
            },
        });
    }

    async function handleSave() {
        if (!hasManagedMemberChanges()) {
            closeModal();
            return;
        }

        saving = true;
        errorMsg = "";
        try {
            const members = Object.entries(managedMemberDraftRoles)
                .filter(([, role]) => role !== "none")
                .map(([userId, role]) => ({
                    userId,
                    role: role as EnsembleRole,
                }))
                .sort((left, right) => left.userId.localeCompare(right.userId));

            await onSave(ensemble.id, members);
            closeModal();
        } catch (error) {
            errorMsg =
                error instanceof Error
                    ? error.message
                    : "Unable to update ensemble members.";
        } finally {
            saving = false;
        }
    }

    function handleAddUser(userId: string) {
        stageManagedMemberRole(
            userId,
            inviteRoles[userId] ?? ("user" as EnsembleRole),
        );
    }
</script>

{#snippet footer()}
    <div class="actions admin-user-modal-actions">
        <button
            class="button ghost"
            type="button"
            disabled={saving}
            onclick={closeModal}
        >
            Cancel
        </button>
        <button
            class="button"
            type="button"
            disabled={saving || !hasManagedMemberChanges()}
            onclick={() => void handleSave()}
        >
            {saving ? "Saving..." : "Save changes"}
        </button>
    </div>
{/snippet}

<BaseModal
    size="full"
    cardClass="admin-split-modal"
    title="Members"
    subtitle={ensemble.name}
    {footer}
    canClose={!saving}
    {modalId}
>
    <div class="admin-split-pane">
        <section class="admin-split-column">
            <div class="admin-split-header">
                <div class="admin-split-header-main">
                    <h4>Current members</h4>
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
                    <span class="admin-user-role-pill">
                        {filteredManagedMembers.length}
                    </span>
                </div>
            </div>
            <div class="admin-inline-list">
                {#if filteredManagedMembers.length === 0}
                    <p class="hint">No matching members.</p>
                {:else}
                    {#each filteredManagedMembers as member}
                        <div class="admin-inline-row">
                            <div class="admin-inline-copy">
                                <strong>{member.user.username}</strong>
                                <span class="admin-user-role-pill">
                                    {memberRoleLabel(member.role)}
                                </span>
                            </div>
                            <div class="admin-inline-actions">
                                {#if allowedEnsembleRolesForUser(member.user, currentUser).length > 0}
                                    <button
                                        class="button secondary admin-inline-role-btn admin-inline-role-btn-mobile"
                                        type="button"
                                        aria-label={`Change role for ${member.user.username}`}
                                        title={`Change role for ${member.user.username}`}
                                        onclick={() =>
                                            openRolePicker(
                                                "member",
                                                member.user,
                                                member.role,
                                            )}
                                    >
                                        <UserCog
                                            size={17}
                                            strokeWidth={2.25}
                                            aria-hidden="true"
                                        />
                                    </button>
                                    <div
                                        class="admin-member-role-select admin-inline-role-select-desktop"
                                    >
                                        <CustomSelect
                                            value={member.role}
                                            options={ensembleRoleOptionsForUser(
                                                member.user,
                                            )}
                                            compact={true}
                                            showDescriptionInTrigger={false}
                                            onValueChange={(nextRole) =>
                                                stageManagedMemberRole(
                                                    member.user_id,
                                                    nextRole as EnsembleRole,
                                                )}
                                        />
                                    </div>
                                    <button
                                        class="button ghost danger admin-inline-icon-btn admin-inline-symbol-btn"
                                        type="button"
                                        aria-label={`Remove ${member.user.username}`}
                                        title={`Remove ${member.user.username}`}
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
                    <span class="admin-user-role-pill">
                        {filteredAvailableEnsembleUsers.length}
                    </span>
                </div>
            </div>
            <div class="admin-inline-list">
                {#if filteredAvailableEnsembleUsers.length === 0}
                    <p class="hint">No available users.</p>
                {:else}
                    {#each filteredAvailableEnsembleUsers as user}
                        <div class="admin-inline-row">
                            <div class="admin-inline-copy">
                                <strong>{user.username}</strong>
                                <span class="admin-user-role-pill">
                                    {user.role}
                                </span>
                            </div>
                            <div class="admin-inline-actions">
                                <button
                                    class="button secondary admin-inline-role-btn admin-inline-role-btn-mobile"
                                    type="button"
                                    aria-label={`Change role for ${user.username}`}
                                    title={`Change role for ${user.username}`}
                                    onclick={() =>
                                        openRolePicker(
                                            "invite",
                                            user,
                                            inviteRoleForUser(user),
                                        )}
                                >
                                    <UserCog size={15} aria-hidden="true" />
                                </button>
                                <div
                                    class="admin-member-role-select admin-inline-role-select-desktop"
                                >
                                    <CustomSelect
                                        value={inviteRoleForUser(user)}
                                        options={ensembleRoleOptionsForUser(
                                            user,
                                        )}
                                        compact={true}
                                        showDescriptionInTrigger={false}
                                        onValueChange={(nextRole) => {
                                            inviteRoles = {
                                                ...inviteRoles,
                                                [user.id]:
                                                    nextRole as EnsembleRole,
                                            };
                                        }}
                                    />
                                </div>
                                <button
                                    class="button secondary admin-inline-icon-btn admin-inline-symbol-btn"
                                    type="button"
                                    aria-label={`Add ${user.username}`}
                                    title={`Add ${user.username}`}
                                    onclick={() => handleAddUser(user.id)}
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
    {#if errorMsg}
        <p class="admin-error">{errorMsg}</p>
    {/if}
</BaseModal>
