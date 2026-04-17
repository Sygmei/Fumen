<script lang="ts">
    import type {
        AdminEnsembleResponse as Ensemble,
        EnsembleMemberResponse,
    } from "$backend/models";
    import { authenticatedApiClient } from "$lib/auth-client";
    import AdminEnsembleCard from "$components/admin/AdminEnsembleCard.svelte";
    import {
        showConfirmModal,
        showModal,
    } from "$components/modals";
    import CreateEnsembleModal from "$components/modals/CreateEnsembleModal.svelte";
    import ManageEnsembleMembersModal from "$components/modals/ManageEnsembleMembersModal.svelte";
    import ManageEnsembleScoresModal from "$components/modals/ManageEnsembleScoresModal.svelte";
    import {
        Search,
        Plus,
    } from "@lucide/svelte";
    import {
        canCreateEnsembles,
        canManageEnsembleMembers,
        canDeleteEnsembleRecord,
        canManageEnsembleScores,
    } from "$lib/admin-permissions";
    import type { EnsembleMemberAssignment } from "$components/modals/types";
    import { appShell } from "$lib/app-shell.svelte";
    import { getAdminStateContext } from "$lib/admin-state.svelte";

    const adminState = getAdminStateContext();

    let ensembleSearchQuery = $state("");
    let creatingEnsembleIds = $state<string[]>([]);
    let savingEnsembleIds = $state<string[]>([]);
    let deletingEnsembleIds = $state<string[]>([]);

    const filteredEnsembles = $derived.by(() => {
        const query = ensembleSearchQuery.trim().toLowerCase();
        const sorted = [...adminState.ensembles].sort((a, b) =>
            a.name.localeCompare(b.name),
        );
        if (!query) return sorted;
        return sorted.filter((ensemble) =>
            [
                ensemble.name,
                `${ensemble.members.length} members`,
                `${ensemble.score_count} scores`,
            ].some((v) => v.toLowerCase().includes(query)),
        );
    });

    function openCreateEnsembleModal() {
        showModal(CreateEnsembleModal, {
            onCreate: handleCreateEnsemble,
        });
    }

    function buildPendingEnsemble(id: string, name: string): Ensemble {
        return {
            id,
            name,
            created_at: new Date().toISOString(),
            created_by_user_id: appShell.currentUser?.id ?? null,
            members: [],
            score_count: 0,
        };
    }

    function findEnsembleById(ensembleId: string) {
        return adminState.ensembles.find((item) => item.id === ensembleId) ?? null;
    }

    async function handleCreateEnsemble(name: string) {
        const optimisticId = `creating-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
        const optimisticEnsemble = buildPendingEnsemble(optimisticId, name);
        creatingEnsembleIds = [...creatingEnsembleIds, optimisticId];
        adminState.addEnsemble(optimisticEnsemble);

        try {
            const ensemble = await authenticatedApiClient.adminCreateEnsemble({ name });
            adminState.removeEnsemble(optimisticId);
            adminState.addEnsemble(ensemble);
            adminState.setSuccess(`Ensemble ${ensemble.name} created.`);
        } catch (error) {
            adminState.removeEnsemble(optimisticId);
            const message =
                error instanceof Error
                    ? error.message
                    : "Unable to create ensemble";
            adminState.setError(message);
        } finally {
            creatingEnsembleIds = creatingEnsembleIds.filter((id) => id !== optimisticId);
        }
    }

    async function deleteEnsembleAccount(ensemble: Ensemble) {
        deletingEnsembleIds = [...deletingEnsembleIds, ensemble.id];
        try {
            await authenticatedApiClient.adminDeleteEnsemble(ensemble.id);
            await adminState.refresh();
            adminState.setSuccess(`Ensemble ${ensemble.name} deleted.`);
        } catch (error) {
            adminState.setError(
                error instanceof Error
                    ? error.message
                    : "Unable to delete ensemble",
            );
        } finally {
            deletingEnsembleIds = deletingEnsembleIds.filter((id) => id !== ensemble.id);
        }
    }

    function handleDeleteEnsemble(ensemble: Ensemble) {
        showConfirmModal({
            title: `Delete ensemble ${ensemble.name}`,
            message: `Delete ensemble ${ensemble.name}?`,
            confirmText: "Delete",
            variant: "danger",
            onConfirm: () => {
                void deleteEnsembleAccount(ensemble);
            },
        });
    }

    function openManageMembersModal(ensemble: Ensemble) {
        if (!appShell.currentUser) return;
        showModal(ManageEnsembleMembersModal, {
            ensemble,
            allUsers: adminState.adminUsers,
            currentUser: appShell.currentUser,
            onSave: saveManagedEnsembleChanges,
        });
    }

    async function saveManagedEnsembleChanges(
        ensembleId: string,
        members: EnsembleMemberAssignment[],
    ) {
        const originalEnsemble = findEnsembleById(ensembleId);
        if (!originalEnsemble) return;

        const optimisticMembers: EnsembleMemberResponse[] = members.map((member) => ({
            user_id: member.userId,
            role: member.role,
        }));
        savingEnsembleIds = [...savingEnsembleIds, ensembleId];
        adminState.updateEnsemble({
            ...originalEnsemble,
            members: optimisticMembers,
        });

        try {
            await authenticatedApiClient.adminUpdateEnsembleMembers(ensembleId, {
                members: optimisticMembers,
            });
            await adminState.refresh();
            adminState.setSuccess("Ensemble members updated.");
        } catch (error) {
            adminState.updateEnsemble(originalEnsemble);
            const message =
                error instanceof Error
                    ? error.message
                    : "Unable to update ensemble members";
            adminState.setError(message);
        } finally {
            savingEnsembleIds = savingEnsembleIds.filter((id) => id !== ensembleId);
        }
    }

    function openManageScoresModal(ensemble: Ensemble) {
        showModal(ManageEnsembleScoresModal, {
            ensemble,
            allMusics: adminState.musics,
            onSave: saveEnsembleScores,
        });
    }

    async function saveEnsembleScores(ensembleId: string, musicIds: string[]) {
        const originalEnsemble = findEnsembleById(ensembleId);
        if (!originalEnsemble) return;

        const uniqueMusicIds = [...new Set(musicIds)].sort((left, right) =>
            left.localeCompare(right),
        );
        savingEnsembleIds = [...savingEnsembleIds, ensembleId];
        adminState.updateEnsemble({
            ...originalEnsemble,
            score_count: uniqueMusicIds.length,
        });

        try {
            await authenticatedApiClient.adminUpdateEnsembleScores(ensembleId, {
                music_ids: uniqueMusicIds,
            });
            await adminState.refresh();
            adminState.setSuccess("Ensemble scores updated.");
        } catch (error) {
            adminState.updateEnsemble(originalEnsemble);
            const message =
                error instanceof Error
                    ? error.message
                    : "Failed to update scores";
            adminState.setError(message);
        } finally {
            savingEnsembleIds = savingEnsembleIds.filter((id) => id !== ensembleId);
        }
    }
</script>

{#if appShell.currentUser}
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
                {#if canCreateEnsembles(appShell.currentUser)}
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

            {#if adminState.ensembles.length === 0}
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
                            <AdminEnsembleCard
                                {ensemble}
                                creating={creatingEnsembleIds.includes(ensemble.id)}
                                saving={savingEnsembleIds.includes(ensemble.id)}
                                deleting={deletingEnsembleIds.includes(ensemble.id)}
                                canManageScores={canManageEnsembleScores(
                                    ensemble,
                                    appShell.currentUser,
                                )}
                                canManageMembers={canManageEnsembleMembers(
                                    ensemble,
                                    appShell.currentUser,
                                )}
                                canDelete={canDeleteEnsembleRecord(
                                    ensemble,
                                    appShell.currentUser,
                                )}
                                onManageScores={() => openManageScoresModal(ensemble)}
                                onManageMembers={() => openManageMembersModal(ensemble)}
                                onDelete={() => void handleDeleteEnsemble(ensemble)}
                            />
                        {/each}
                    </div>
                </div>
            {/if}
        </div>
    </section>
{/if}
