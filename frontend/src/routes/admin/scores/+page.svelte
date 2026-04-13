<script lang="ts">
    import type {
        AdminUpdateMusicMultipartRequest,
        AdminUploadMusicMultipartRequest,
        AdminMusicResponse as AdminMusic,
    } from "$backend/models";
    import { authenticatedApiClient } from "$lib/auth-client";
    import {
        showConfirmModal,
        showModal,
    } from "$components/modals";
    import UploadScoreModal from "$components/modals/UploadScoreModal.svelte";
    import ScoreEnsemblePickerModal from "$components/modals/ScoreEnsemblePickerModal.svelte";
    import EditScoreModal from "$components/modals/EditScoreModal.svelte";
    import ScoreInfoModal from "$components/modals/ScoreInfoModal.svelte";
    import AdminScoreCard from "$components/admin/AdminScoreCard.svelte";
    import {
        Search,
        Plus,
    } from "@lucide/svelte";
    import {
        canDeleteScore,
        canEditOwnedScore,
        canManageScoreEnsembles,
    } from "$lib/admin-permissions";
    import { qualityProfileLabel } from "$lib/utils";
    import type {
        EditScoreDraft,
        UploadScoreDraft,
    } from "$components/modals/types";
    import { appShell } from "$lib/app-shell.svelte";
    import { getAdminStateContext } from "$lib/admin-state.svelte";

    const adminState = getAdminStateContext();

    let scoreSearchQuery = $state("");
    let deletingMusicFor = $state("");
    let openDownloadMenuFor = $state("");

    const filteredMusics = $derived.by(() => {
        const query = scoreSearchQuery.trim().toLowerCase();
        const sorted = [...adminState.musics].sort((a, b) =>
            a.title.localeCompare(b.title),
        );
        if (!query) return sorted;
        return sorted.filter((music) =>
            [
                music.title,
                music.filename,
                music.public_id ?? "",
                ...music.ensemble_names,
                qualityProfileLabel(music.quality_profile),
            ].some((v) => v.toLowerCase().includes(query)),
        );
    });

    function isProcessingMusic(music: AdminMusic) {
        return [
            music.audio_status,
            music.midi_status,
            music.musicxml_status,
            music.stems_status,
        ].includes("processing");
    }

    function openCreateScoreModal() {
        showModal(UploadScoreModal, {
            ensembles: adminState.ensembles,
            onUpload: handleUpload,
        });
    }

    async function handleUpload(draft: UploadScoreDraft) {
        try {
            const payload = {
                file: draft.file,
                title: draft.title,
                icon: "",
                icon_file: draft.iconFile ?? undefined,
                public_id: draft.publicId,
                quality_profile: draft.qualityProfile,
                ensemble_id: draft.ensembleIds,
            } as unknown as AdminUploadMusicMultipartRequest;
            const uploaded = await authenticatedApiClient.adminUploadMusic(payload);
            adminState.updateMusic(uploaded);
            void adminState.refresh();
            adminState.setSuccess(
                "Upload started. The score will keep processing in the background.",
            );
        } catch (error) {
            const message = error instanceof Error ? error.message : "Upload failed";
            adminState.setError(message);
            throw error instanceof Error ? error : new Error(message);
        }
    }

    function openScoreEnsembleModal(music: AdminMusic) {
        showModal(ScoreEnsemblePickerModal, {
            mode: "score",
            ensembles: adminState.ensembles,
            music,
            initialSelectedEnsembleIds: music.ensemble_ids,
            onApply: (ensembleIds: string[]) =>
                saveScoreEnsembles(music, ensembleIds),
        });
        openDownloadMenuFor = "";
    }

    async function saveScoreEnsembles(
        music: AdminMusic,
        ensembleIds: string[],
    ) {
        const original = new Set(music.ensemble_ids);
        const staged = new Set(ensembleIds);
        const toAdd = [...staged].filter((id) => !original.has(id));
        const toRemove = [...original].filter((id) => !staged.has(id));
        if (toAdd.length === 0 && toRemove.length === 0) {
            return;
        }
        try {
            await authenticatedApiClient.adminUpdateMusicEnsembles(music.id, {
                ensemble_ids: [...staged].sort((left, right) =>
                    left.localeCompare(right),
                ),
            });
            await adminState.refresh();
            adminState.setSuccess("Score ensembles updated.");
        } catch (error) {
            const message =
                error instanceof Error
                    ? error.message
                    : "Failed to update ensembles";
            adminState.setError(message);
            throw error instanceof Error ? error : new Error(message);
        }
    }

    function openScoreMetadataModal(music: AdminMusic) {
        showModal(EditScoreModal, {
            music,
            onSave: (draft: EditScoreDraft) =>
                handleSaveScoreMetadata(music, draft),
        });
        openDownloadMenuFor = "";
    }

    async function handleSaveScoreMetadata(
        music: AdminMusic,
        draft: EditScoreDraft,
    ) {
        try {
            const payload = {
                title: draft.title,
                public_id: draft.publicId,
                icon: draft.icon,
                icon_file: draft.iconFile ?? undefined,
            } as unknown as AdminUpdateMusicMultipartRequest;
            const updated = await authenticatedApiClient.adminUpdateMusic(
                music.id,
                payload,
            );
            adminState.updateMusic(updated);
            adminState.setSuccess("Score metadata updated.");
        } catch (error) {
            const message =
                error instanceof Error
                    ? error.message
                    : "Unable to update score metadata";
            adminState.setError(message);
            throw error instanceof Error ? error : new Error(message);
        }
    }

    function openScoreInfoModal(music: AdminMusic) {
        if (!appShell.currentUser) return;
        showModal(ScoreInfoModal, {
            music,
            currentUser: appShell.currentUser,
            loadPlaytime: loadScorePlaytime,
            onRetryRender: handleRetryRender,
        });
        openDownloadMenuFor = "";
    }

    async function loadScorePlaytime(musicId: string) {
        return authenticatedApiClient.adminMusicPlaytime(musicId);
    }

    async function handleRetryRender(musicId: string) {
        try {
            const updated = await authenticatedApiClient.adminRetryRender(musicId);
            adminState.updateMusic(updated);
            adminState.setSuccess("Render retried successfully.");
            return updated;
        } catch (error) {
            const message = error instanceof Error ? error.message : "Retry failed";
            adminState.setError(message);
            throw error instanceof Error ? error : new Error(message);
        }
    }

    async function deleteMusicAccount(musicId: string) {
        deletingMusicFor = musicId;
        try {
            await authenticatedApiClient.adminDeleteMusic(musicId);
            await adminState.refresh();
            adminState.setSuccess("Score deleted.");
        } catch (error) {
            adminState.setError(
                error instanceof Error
                    ? error.message
                    : "Unable to delete score",
            );
        } finally {
            deletingMusicFor = "";
        }
    }

    function handleDeleteMusic(musicId: string) {
        showConfirmModal({
            title: "Delete score",
            message: "Delete this score permanently?",
            confirmText: "Delete",
            variant: "danger",
            onConfirm: () => deleteMusicAccount(musicId),
        });
    }

    function toggleDownloadMenu(musicId: string) {
        openDownloadMenuFor = openDownloadMenuFor === musicId ? "" : musicId;
    }

    async function handleShowScoreQr(music: AdminMusic) {
        try {
            await appShell.openCredentialModal(
                `Share link for ${music.title}`,
                () =>
                    Promise.resolve({
                        connection_url: music.public_url,
                        expires_at: "",
                    }),
                {
                    eyebrow: "Share score",
                    linkLabel: "Share link",
                },
            );
        } catch (error) {
            adminState.setError(
                error instanceof Error ? error.message : "Failed to show QR",
            );
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
                    <h3>Scores</h3>
                </div>
                <label class="field m-0 gap-0 min-w-0 self-center">
                    <span class="sr-only">Search scores</span>
                    <div class="relative">
                        <Search
                            size={15}
                            class="absolute top-1/2 left-2.5 -translate-y-1/2 text-(--text-dim) pointer-events-none"
                            aria-hidden="true"
                        />
                        <input
                            bind:value={scoreSearchQuery}
                            placeholder="Search scores"
                            class="!py-2 !px-3 !pl-8 !min-h-[38px]"
                        />
                    </div>
                </label>
                <button
                    class="button admin-create-user-btn"
                    type="button"
                    onclick={openCreateScoreModal}
                >
                    <Plus size={15} aria-hidden="true" />
                    <span class="admin-create-label admin-create-label-full"
                        >Add a score</span
                    >
                    <span class="admin-create-label admin-create-label-short"
                        >Create</span
                    >
                </button>
            </div>

            {#if adminState.musics.length === 0}
                <div class="music-card"><p class="hint">No uploads yet.</p></div>
            {:else if filteredMusics.length === 0}
                <div class="music-card">
                    <p class="hint">
                        No scores match "{scoreSearchQuery.trim()}".
                    </p>
                </div>
            {:else}
                <div
                    class="grid grid-cols-3 gap-2 items-start content-start max-[1360px]:grid-cols-2 max-[760px]:grid-cols-1"
                >
                    {#each filteredMusics as music}
                        <AdminScoreCard
                            {music}
                            processing={isProcessingMusic(music)}
                            downloadOpen={openDownloadMenuFor === music.id}
                            deleting={deletingMusicFor === music.id}
                            canManageEnsembles={canManageScoreEnsembles(
                                music,
                                appShell.currentUser,
                            )}
                            canEdit={canEditOwnedScore(music, appShell.currentUser)}
                            canDelete={canDeleteScore(music, appShell.currentUser)}
                            onToggleDownloadMenu={() => toggleDownloadMenu(music.id)}
                            onManageEnsembles={() => openScoreEnsembleModal(music)}
                            onEdit={() => openScoreMetadataModal(music)}
                            onShowQr={() => void handleShowScoreQr(music)}
                            onShowInfo={() => openScoreInfoModal(music)}
                            onDelete={() => handleDeleteMusic(music.id)}
                            onCloseDownloadMenu={() => (openDownloadMenuFor = "")}
                        />
                    {/each}
                </div>
            {/if}
        </div>
    </section>
{/if}
