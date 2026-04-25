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
        hasGlobalPower,
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
    let creatingMusicIds = $state<string[]>([]);
    let savingMusicIds = $state<string[]>([]);
    let deletingMusicIds = $state<string[]>([]);
    let restartingMusicIds = $state<string[]>([]);
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
                music.subtitle ?? "",
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

    function canRestartProcessing(music: AdminMusic) {
        return [
            music.audio_status,
            music.midi_status,
            music.musicxml_status,
            music.stems_status,
        ].some((status) => status !== "ready");
    }

    async function reloadMusic(musicId: string) {
        await adminState.refresh();
        const music = adminState.musics.find((item) => item.id === musicId);
        if (!music) {
            throw new Error("Score not found");
        }
        return music;
    }

    function openCreateScoreModal() {
        showModal(UploadScoreModal, {
            ensembles: adminState.ensembles,
            onUpload: handleUpload,
        });
    }

    function displayTitleForDraft(draft: UploadScoreDraft) {
        const titled = draft.title.trim();
        if (titled) return titled;

        const filename = draft.file?.name?.trim() ?? "";
        return filename.replace(/\.mscz$/i, "") || "Untitled score";
    }

    function buildPendingMusic(
        id: string,
        draft: UploadScoreDraft,
    ): AdminMusic {
        const ensembleNames = draft.ensembleIds
            .map((ensembleId) =>
                adminState.ensembles.find((ensemble) => ensemble.id === ensembleId)?.name,
            )
            .filter((name): name is string => !!name)
            .sort((left, right) => left.localeCompare(right));

        return {
            id,
            title: displayTitleForDraft(draft),
            subtitle: draft.subtitle.trim() || null,
            icon: "",
            icon_image_url: null,
            filename: draft.file?.name ?? "upload.mscz",
            content_type: draft.file?.type || "application/octet-stream",
            audio_status: "processing",
            audio_error: null,
            midi_status: "processing",
            midi_error: null,
            musicxml_status: "processing",
            musicxml_error: null,
            stems_status: "processing",
            stems_error: null,
            public_token: "",
            public_id: draft.publicId.trim() || null,
            public_url: "",
            public_id_url: null,
            download_url: "",
            midi_download_url: null,
            quality_profile: draft.qualityProfile,
            created_at: new Date().toISOString(),
            stems_total_bytes: 0,
            ensemble_ids: [...draft.ensembleIds].sort((left, right) =>
                left.localeCompare(right),
            ),
            ensemble_names: ensembleNames,
            owner_user_id: appShell.currentUser?.id ?? null,
        };
    }

    async function handleUpload(draft: UploadScoreDraft) {
        const optimisticId = `creating-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
        const optimisticMusic = buildPendingMusic(optimisticId, draft);
        creatingMusicIds = [...creatingMusicIds, optimisticId];
        adminState.updateMusic(optimisticMusic);

        try {
            const payload = {
                file: draft.file,
                title: draft.title,
                subtitle: draft.subtitle,
                icon: "",
                icon_file: draft.iconFile ?? undefined,
                public_id: draft.publicId,
                quality_profile: draft.qualityProfile,
                ensemble_id: draft.ensembleIds,
            } as unknown as AdminUploadMusicMultipartRequest;
            const uploaded = await authenticatedApiClient.adminUploadMusic(payload);
            adminState.removeMusic(optimisticId);
            adminState.updateMusic(uploaded);
            void adminState.refresh();
            adminState.setSuccess(
                "Upload started. The score will keep processing in the background.",
            );
        } catch (error) {
            adminState.removeMusic(optimisticId);
            const message = error instanceof Error ? error.message : "Upload failed";
            adminState.setError(message);
        } finally {
            creatingMusicIds = creatingMusicIds.filter((id) => id !== optimisticId);
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
        const optimisticEnsembleIds = [...staged].sort((left, right) =>
            left.localeCompare(right),
        );
        const optimisticEnsembleNames = optimisticEnsembleIds
            .map((ensembleId) =>
                adminState.ensembles.find((ensemble) => ensemble.id === ensembleId)?.name,
            )
            .filter((name): name is string => !!name)
            .sort((left, right) => left.localeCompare(right));
        savingMusicIds = [...savingMusicIds, music.id];
        adminState.updateMusic({
            ...music,
            ensemble_ids: optimisticEnsembleIds,
            ensemble_names: optimisticEnsembleNames,
        });
        try {
            await authenticatedApiClient.adminUpdateMusicEnsembles(music.id, {
                ensemble_ids: optimisticEnsembleIds,
            });
            await adminState.refresh();
            adminState.setSuccess("Score ensembles updated.");
        } catch (error) {
            adminState.updateMusic(music);
            const message =
                error instanceof Error
                    ? error.message
                    : "Failed to update ensembles";
            adminState.setError(message);
        } finally {
            savingMusicIds = savingMusicIds.filter((id) => id !== music.id);
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
        const optimisticMusic: AdminMusic = {
            ...music,
            title: draft.title.trim(),
            subtitle: draft.subtitle.trim() || null,
            public_id: draft.publicId.trim() || null,
            icon: draft.icon,
        };
        savingMusicIds = [...savingMusicIds, music.id];
        adminState.updateMusic(optimisticMusic);
        try {
            const payload = {
                title: draft.title,
                subtitle: draft.subtitle,
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
            adminState.updateMusic(music);
            const message =
                error instanceof Error
                    ? error.message
                    : "Unable to update score metadata";
            adminState.setError(message);
        } finally {
            savingMusicIds = savingMusicIds.filter((id) => id !== music.id);
        }
    }

    function openScoreInfoModal(music: AdminMusic) {
        if (!appShell.currentUser) return;
        showModal(ScoreInfoModal, {
            music,
            currentUser: appShell.currentUser,
            loadPlaytime: loadScorePlaytime,
            loadProcessingLog: (musicId: string) =>
                authenticatedApiClient.adminMusicProcessingLog(musicId),
            reloadMusic,
            canViewProcessingLog: hasGlobalPower(appShell.currentUser),
            onRetryRender: handleRestartProcessing,
        });
        openDownloadMenuFor = "";
    }

    async function loadScorePlaytime(musicId: string) {
        return authenticatedApiClient.adminMusicPlaytime(musicId);
    }

    async function handleRestartProcessing(musicId: string) {
        restartingMusicIds = [...restartingMusicIds, musicId];
        try {
            const updated = await authenticatedApiClient.adminRetryRender(musicId);
            adminState.updateMusic(updated);
            adminState.setSuccess("Processing restarted.");
            return updated;
        } catch (error) {
            const message =
                error instanceof Error
                    ? error.message
                    : "Unable to restart processing";
            adminState.setError(message);
            throw error instanceof Error ? error : new Error(message);
        } finally {
            restartingMusicIds = restartingMusicIds.filter((id) => id !== musicId);
        }
    }

    $effect(() => {
        if (!appShell.currentUser) return;
        if (!adminState.musics.some((music) => isProcessingMusic(music))) return;

        const interval = window.setInterval(() => {
            void adminState.refresh();
        }, 5000);

        return () => {
            window.clearInterval(interval);
        };
    });

    async function deleteMusicAccount(musicId: string) {
        deletingMusicIds = [...deletingMusicIds, musicId];
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
            deletingMusicIds = deletingMusicIds.filter((id) => id !== musicId);
        }
    }

    function handleDeleteMusic(musicId: string) {
        showConfirmModal({
            title: "Delete score",
            message: "Delete this score permanently?",
            confirmText: "Delete",
            variant: "danger",
            onConfirm: () => {
                void deleteMusicAccount(musicId);
            },
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
                <div class="admin-score-scroll-area">
                    <div
                        class="admin-score-list grid grid-cols-3 gap-2 items-start content-start max-[1360px]:grid-cols-2 max-[760px]:grid-cols-1"
                    >
                        {#each filteredMusics as music}
                            <AdminScoreCard
                                {music}
                                creating={creatingMusicIds.includes(music.id)}
                                saving={savingMusicIds.includes(music.id)}
                                restarting={restartingMusicIds.includes(music.id)}
                                processing={isProcessingMusic(music)}
                                downloadOpen={openDownloadMenuFor === music.id}
                                deleting={deletingMusicIds.includes(music.id)}
                                canManageEnsembles={canManageScoreEnsembles(
                                    music,
                                    appShell.currentUser,
                                )}
                                canEdit={canEditOwnedScore(music, appShell.currentUser)}
                                canDelete={canDeleteScore(music, appShell.currentUser)}
                                showRestartAction={canRestartProcessing(music)}
                                onToggleDownloadMenu={() => toggleDownloadMenu(music.id)}
                                onManageEnsembles={() => openScoreEnsembleModal(music)}
                                onEdit={() => openScoreMetadataModal(music)}
                                onShowQr={() => void handleShowScoreQr(music)}
                                onShowInfo={() => openScoreInfoModal(music)}
                                onRestartProcessing={() =>
                                    void handleRestartProcessing(music.id)}
                                onDelete={() => handleDeleteMusic(music.id)}
                                onCloseDownloadMenu={() => (openDownloadMenuFor = "")}
                            />
                        {/each}
                    </div>
                </div>
            {/if}
        </div>
    </section>
{/if}
