<script lang="ts">
    import type {
        AdminUpdateMusicMultipartRequest,
        AdminUploadMusicMultipartRequest,
        AdminEnsembleResponse as Ensemble,
        AdminMusicResponse as AdminMusic,
        UserResponse as AppUser,
    } from "../adapters/fumen-backend/src/models";
    import { authenticatedApiClient } from "../lib/auth-client";
    import { showConfirmModal, showModal } from "../components/modals";
    import UploadScoreModal from "../components/modals/UploadScoreModal.svelte";
    import ScoreEnsemblePickerModal from "../components/modals/ScoreEnsemblePickerModal.svelte";
    import EditScoreModal from "../components/modals/EditScoreModal.svelte";
    import ScoreInfoModal from "../components/modals/ScoreInfoModal.svelte";
    import ScoreIcon from "../components/ScoreIcon.svelte";
    import {
        Search,
        Plus,
        QrCode,
        Trash2,
        Users,
        Pencil,
        Info,
        Download,
        ChevronDown,
        Music,
    } from "@lucide/svelte";
    import {
        canDeleteScore,
        canEditOwnedScore,
        canManageScoreEnsembles,
    } from "../lib/admin-permissions";
    import { qualityProfileLabel } from "../lib/utils";
    import type {
        EditScoreDraft,
        UploadScoreDraft,
    } from "../components/modals/types";

    let {
        currentUser,
        musics,
        ensembles,
        onMusicUpdated,
        onRefresh,
        onShowQr,
        onSuccess,
        onError,
    }: {
        currentUser: AppUser;
        musics: AdminMusic[];
        ensembles: Ensemble[];
        onMusicUpdated: (music: AdminMusic) => void;
        onRefresh: () => Promise<void>;
        onShowQr: (music: AdminMusic) => Promise<void>;
        onSuccess: (msg: string) => void;
        onError: (msg: string) => void;
    } = $props();

    // Search
    let scoreSearchQuery = $state("");

    // Other
    let deletingMusicFor = $state("");
    let openDownloadMenuFor = $state("");

    // Derived
    const filteredMusics = $derived.by(() => {
        const query = scoreSearchQuery.trim().toLowerCase();
        const sorted = [...musics].sort((a, b) =>
            a.title.localeCompare(b.title),
        );
        if (!query) return sorted;
        return sorted.filter((m) =>
            [
                m.title,
                m.filename,
                m.public_id ?? "",
                ...m.ensemble_names,
                qualityProfileLabel(m.quality_profile),
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

    // Upload
    function openCreateScoreModal() {
        showModal(UploadScoreModal, {
            ensembles,
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
            onMusicUpdated(uploaded);
            void onRefresh();
            onSuccess("Upload started. The score will keep processing in the background.");
        } catch (error) {
            const message = error instanceof Error ? error.message : "Upload failed";
            onError(message);
            throw error instanceof Error ? error : new Error(message);
        }
    }

    // Ensemble picker
    function openScoreEnsembleModal(music: AdminMusic) {
        showModal(ScoreEnsemblePickerModal, {
            mode: "score",
            ensembles,
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
            await authenticatedApiClient.adminUpdateMusicEnsembles(
                music.id,
                { ensemble_ids: [...staged].sort((left, right) => left.localeCompare(right)) },
            );
            await onRefresh();
            onSuccess("Score ensembles updated.");
        } catch (error) {
            const message =
                error instanceof Error
                    ? error.message
                    : "Failed to update ensembles";
            onError(message);
            throw error instanceof Error ? error : new Error(message);
        }
    }

    // Score metadata
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
            const updated = await authenticatedApiClient.adminUpdateMusic(music.id, payload);
            onMusicUpdated(updated);
            onSuccess("Score metadata updated.");
        } catch (error) {
            const message =
                error instanceof Error
                    ? error.message
                    : "Unable to update score metadata";
            onError(message);
            throw error instanceof Error ? error : new Error(message);
        }
    }

    // Score info
    function openScoreInfoModal(music: AdminMusic) {
        showModal(ScoreInfoModal, {
            music,
            currentUser,
            loadPlaytime: loadScorePlaytime,
            onRetryRender: handleRetryRender,
        });
        openDownloadMenuFor = "";
    }

    async function loadScorePlaytime(musicId: string) {
        return authenticatedApiClient.adminMusicPlaytime(musicId);
    }

    // Retry render
    async function handleRetryRender(musicId: string) {
        try {
            const updated = await authenticatedApiClient.adminRetryRender(musicId);
            onMusicUpdated(updated);
            onSuccess("Render retried successfully.");
            return updated;
        } catch (error) {
            const message = error instanceof Error ? error.message : "Retry failed";
            onError(message);
            throw error instanceof Error ? error : new Error(message);
        }
    }

    // Delete
    async function deleteMusicAccount(musicId: string) {
        deletingMusicFor = musicId;
        try {
            await authenticatedApiClient.adminDeleteMusic(musicId);
            await onRefresh();
            onSuccess("Score deleted.");
        } catch (error) {
            onError(
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

    // Download menu
    function toggleDownloadMenu(musicId: string) {
        openDownloadMenuFor = openDownloadMenuFor === musicId ? "" : musicId;
    }

    // QR
    async function handleShowScoreQr(music: AdminMusic) {
        try {
            await onShowQr(music);
        } catch (error) {
            onError(
                error instanceof Error ? error.message : "Failed to show QR",
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

        {#if musics.length === 0}
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
                    <article
                        class="music-card admin-score-card"
                        class:processing={isProcessingMusic(music)}
                        class:download-open={openDownloadMenuFor === music.id}
                        aria-busy={isProcessingMusic(music)}
                    >
                        <div class="admin-score-header">
                            <h3 class="admin-score-title">
                                <ScoreIcon
                                    variant="admin"
                                    icon={music.icon}
                                    imageUrl={music.icon_image_url}
                                />
                                <a
                                    href={music.public_url}
                                    target="_blank"
                                    rel="noreferrer">{music.title}</a
                                >
                                {#if isProcessingMusic(music)}
                                    <span class="status-pill admin-score-processing-pill"
                                        >Processing</span
                                    >
                                {/if}
                            </h3>
                            <div
                                class="download-menu admin-score-download-menu"
                                class:open={openDownloadMenuFor === music.id}
                            >
                                <button
                                    class="download-menu-btn admin-score-download-btn"
                                    type="button"
                                    onclick={() => toggleDownloadMenu(music.id)}
                                    aria-label={`Download files for ${music.title}`}
                                    title="Downloads"
                                    aria-haspopup="true"
                                    aria-expanded={openDownloadMenuFor ===
                                        music.id}
                                >
                                    <Download size={15} strokeWidth={2.2} />
                                    <span>Download</span>
                                    <ChevronDown
                                        class="chevron"
                                        size={12}
                                        strokeWidth={2.5}
                                    />
                                </button>
                                {#if openDownloadMenuFor === music.id}
                                    <div class="download-dropdown">
                                        <a
                                            class="download-item"
                                            href={music.download_url}
                                            target="_blank"
                                            rel="noreferrer"
                                            onclick={() =>
                                                (openDownloadMenuFor = "")}
                                        >
                                            <Download
                                                size={18}
                                                strokeWidth={2.2}
                                                aria-hidden="true"
                                            />
                                            <span>Download MuseScore</span>
                                        </a>
                                        {#if music.midi_download_url}
                                            <a
                                                class="download-item"
                                                href={music.midi_download_url}
                                                target="_blank"
                                                rel="noreferrer"
                                                onclick={() =>
                                                    (openDownloadMenuFor = "")}
                                            >
                                                <Music
                                                    size={18}
                                                    strokeWidth={2.2}
                                                    aria-hidden="true"
                                                />
                                                <span>Download MIDI</span>
                                            </a>
                                        {/if}
                                    </div>
                                {/if}
                            </div>
                        </div>
                        <div class="actions admin-score-actions">
                            {#if canManageScoreEnsembles(music, currentUser)}
                                <button
                                    class="button secondary admin-user-action"
                                    type="button"
                                    onclick={() =>
                                        openScoreEnsembleModal(music)}
                                    aria-label={`Manage ensembles for ${music.title}`}
                                    title="Manage ensembles"
                                >
                                    <Users size={16} aria-hidden="true" />
                                    <span
                                        class="admin-action-badge"
                                        aria-hidden="true"
                                        >{music.ensemble_names.length}</span
                                    >
                                </button>
                            {/if}
                            {#if canEditOwnedScore(music, currentUser)}
                                <button
                                    class="button secondary admin-user-action"
                                    type="button"
                                    onclick={() =>
                                        openScoreMetadataModal(music)}
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
                                onclick={() => openScoreInfoModal(music)}
                                aria-label={`View metadata for ${music.title}`}
                                title="View metadata"
                            >
                                <Info size={16} aria-hidden="true" />
                            </button>
                            {#if canDeleteScore(music, currentUser)}
                                <button
                                    class="button ghost danger admin-user-action"
                                    type="button"
                                    aria-label={`Delete ${music.title}`}
                                    title="Delete score"
                                    disabled={deletingMusicFor === music.id}
                                    onclick={() => handleDeleteMusic(music.id)}
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
