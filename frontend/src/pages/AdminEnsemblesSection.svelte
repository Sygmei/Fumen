<script lang="ts">
    import type {
        AdminEnsembleResponse as Ensemble,
        AdminMusicResponse as AdminMusic,
        UserResponse as AppUser,
    } from "../adapters/fumen-backend/src/models";
    import { authenticatedApiClient } from "../lib/auth-client";
    import AdminRecordCard from "../components/AdminRecordCard.svelte";
    import {
        showConfirmModal,
        showModal,
    } from "../components/modals";
    import CreateEnsembleModal from "../components/modals/CreateEnsembleModal.svelte";
    import ManageEnsembleMembersModal from "../components/modals/ManageEnsembleMembersModal.svelte";
    import ManageEnsembleScoresModal from "../components/modals/ManageEnsembleScoresModal.svelte";
    import {
        Search,
        Plus,
        Trash2,
        UserPlus,
        Music,
        Users,
    } from "@lucide/svelte";
    import {
        canCreateEnsembles,
        canManageEnsembleMembers,
        canDeleteEnsembleRecord,
        canManageEnsembleScores,
    } from "../lib/admin-permissions";
    import type { EnsembleMemberAssignment } from "../components/modals/types";

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

    let deletingEnsembleFor = $state("");

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

    // Create ensemble
    function openCreateEnsembleModal() {
        showModal(CreateEnsembleModal, {
            onCreate: handleCreateEnsemble,
        });
    }

    async function handleCreateEnsemble(name: string) {
        try {
            const ensemble = await authenticatedApiClient.adminCreateEnsemble({ name });
            onEnsembleCreated(ensemble);
            onSuccess(`Ensemble ${ensemble.name} created.`);
        } catch (error) {
            const message =
                error instanceof Error
                    ? error.message
                    : "Unable to create ensemble";
            onError(message);
            throw error instanceof Error ? error : new Error(message);
        }
    }

    // Delete ensemble
    async function deleteEnsembleAccount(ensemble: Ensemble) {
        deletingEnsembleFor = ensemble.id;
        try {
            await authenticatedApiClient.adminDeleteEnsemble(ensemble.id);
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
        showConfirmModal({
            title: `Delete ensemble ${ensemble.name}`,
            message: `Delete ensemble ${ensemble.name}?`,
            confirmText: "Delete",
            variant: "danger",
            onConfirm: () => deleteEnsembleAccount(ensemble),
        });
    }

    // Manage members
    function openManageMembersModal(ensemble: Ensemble) {
        showModal(ManageEnsembleMembersModal, {
            ensemble,
            allUsers,
            currentUser,
            onSave: saveManagedEnsembleChanges,
        });
    }

    async function saveManagedEnsembleChanges(
        ensembleId: string,
        members: EnsembleMemberAssignment[],
    ) {
        try {
            await authenticatedApiClient.adminUpdateEnsembleMembers(ensembleId, {
                members: members.map((member) => ({
                    user_id: member.userId,
                    role: member.role,
                })),
            });
            await onRefresh();
            onSuccess("Ensemble members updated.");
        } catch (error) {
            const message =
                error instanceof Error
                    ? error.message
                    : "Unable to update ensemble members";
            onError(message);
            throw error instanceof Error ? error : new Error(message);
        }
    }

    // Manage ensemble scores
    function openManageScoresModal(ensemble: Ensemble) {
        showModal(ManageEnsembleScoresModal, {
            ensemble,
            allMusics,
            onSave: saveEnsembleScores,
        });
    }

    async function saveEnsembleScores(
        ensembleId: string,
        musicIds: string[],
    ) {
        try {
            await authenticatedApiClient.adminUpdateMusicEnsembles(ensembleId, {
                ensemble_ids: musicIds,
            });
            await onRefresh();
            onSuccess("Ensemble scores updated.");
        } catch (error) {
            const message =
                error instanceof Error
                    ? error.message
                    : "Failed to update scores";
            onError(message);
            throw error instanceof Error ? error : new Error(message);
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
