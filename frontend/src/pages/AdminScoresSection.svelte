<script lang="ts">
    import {
        fetchAdminMusicPlaytime,
        uploadMusic,
        deleteMusic,
        updateMusicMetadata,
        addMusicToEnsemble,
        removeMusicFromEnsemble,
        retryRender,
        STEM_QUALITY_PROFILES,
        type AdminMusic,
        type AdminMusicPlaytime,
        type AppUser,
        type Ensemble,
        type StemQualityProfile,
    } from "../lib/api";
    import BaseModal from "../components/BaseModal.svelte";
    import CustomSelect from "../components/CustomSelect.svelte";
    import ConfirmModal from "../components/ConfirmModal.svelte";
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
    import {
        formatBytes,
        formatPlaytimeDuration,
        prettyDate,
        qualityProfileLabel,
    } from "../lib/utils";

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

    // Upload state
    let showCreateScoreModal = $state(false);
    let uploadTitle = $state("");
    let uploadPublicId = $state("");
    let uploadQualityProfile = $state<StemQualityProfile>("standard");
    let selectedFile = $state<File | null>(null);
    let selectedIconFile = $state<File | null>(null);
    let uploadBusy = $state(false);
    let uploadEnsembleIds = $state<string[]>([]);

    // Ensemble picker (for upload and score ensemble management)
    let ensemblePickerMode = $state<"" | "upload" | "score">("");
    let ensemblePickerMusicId = $state("");
    let ensemblePickerSearchQuery = $state("");
    let stagedScoreEnsembleIds = $state<string[]>([]);
    let savingScoreEnsembles = $state(false);

    // Score metadata
    let metadataMusicId = $state("");
    let metadataTitle = $state("");
    let metadataIcon = $state("");
    let metadataIconFile = $state<File | null>(null);
    let metadataPublicId = $state("");
    let savingMetadataFor = $state("");

    // Score info
    let infoMusicId = $state("");
    let infoPlaytime = $state<AdminMusicPlaytime | null>(null);
    let infoPlaytimeLoading = $state(false);
    let infoPlaytimeError = $state("");

    // Other
    let retryingFor = $state("");
    let deletingMusicFor = $state("");
    let openDownloadMenuFor = $state("");

    // Confirm
    let confirmMessage = $state("");
    let confirmLabel = $state("");
    let confirmAction = $state<(() => Promise<void>) | null>(null);
    let confirmBusy = $state(false);

    // Quality options
    const stemQualityOptions = STEM_QUALITY_PROFILES.map((p) => ({
        value: p.value,
        label: p.label,
        description: p.description,
    }));

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

    const activeEnsemblePickerMusic = $derived(
        musics.find((m) => m.id === ensemblePickerMusicId) ?? null,
    );

    const activeMetadataMusic = $derived(
        musics.find((m) => m.id === metadataMusicId) ?? null,
    );

    const activeInfoMusic = $derived(
        musics.find((m) => m.id === infoMusicId) ?? null,
    );

    const filteredPickerEnsembles = $derived.by(() => {
        const query = ensemblePickerSearchQuery.trim().toLowerCase();
        const sorted = [...ensembles].sort((a, b) =>
            a.name.localeCompare(b.name),
        );
        if (!query) return sorted;
        return sorted.filter((e) => e.name.toLowerCase().includes(query));
    });

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

    // Upload
    function openCreateScoreModal() {
        showCreateScoreModal = true;
    }

    function closeCreateScoreModal(force = false) {
        if (uploadBusy && !force) return;
        showCreateScoreModal = false;
        closeEnsemblePickerModal();
        uploadTitle = "";
        uploadPublicId = "";
        uploadQualityProfile = "standard";
        selectedFile = null;
        selectedIconFile = null;
        uploadEnsembleIds = ensembles[0] ? [ensembles[0].id] : [];
        const scoreInput = document.getElementById(
            "mscz-input",
        ) as HTMLInputElement | null;
        if (scoreInput) scoreInput.value = "";
        const iconInput = document.getElementById(
            "icon-file-input",
        ) as HTMLInputElement | null;
        if (iconInput) iconInput.value = "";
    }

    function handleFileSelection(event: Event) {
        const target = event.currentTarget as HTMLInputElement;
        selectedFile = target.files?.[0] ?? null;
    }

    async function handleUpload() {
        if (!selectedFile) {
            onError("Choose an .mscz file first.");
            return;
        }
        uploadBusy = true;
        try {
            const uploaded = await uploadMusic({
                file: selectedFile,
                title: uploadTitle,
                icon: "",
                iconFile: selectedIconFile,
                publicId: uploadPublicId,
                qualityProfile: uploadQualityProfile,
                ensembleId: uploadEnsembleIds[0] ?? "",
            });
            for (const ensembleId of uploadEnsembleIds.slice(1)) {
                if (!uploaded.ensemble_ids.includes(ensembleId)) {
                    await addMusicToEnsemble(uploaded.id, ensembleId);
                }
            }
            await onRefresh();
            uploadBusy = false;
            closeCreateScoreModal(true);
            onSuccess("Upload completed.");
        } catch (error) {
            onError(error instanceof Error ? error.message : "Upload failed");
        } finally {
            if (uploadBusy) uploadBusy = false;
        }
    }

    // Ensemble picker
    function isUploadEnsembleSelected(ensembleId: string) {
        return uploadEnsembleIds.includes(ensembleId);
    }

    function toggleUploadEnsembleSelection(
        ensembleId: string,
        checked: boolean,
    ) {
        if (checked) {
            if (!uploadEnsembleIds.includes(ensembleId)) {
                uploadEnsembleIds = [...uploadEnsembleIds, ensembleId];
            }
        } else {
            uploadEnsembleIds = uploadEnsembleIds.filter(
                (id) => id !== ensembleId,
            );
        }
    }

    function openUploadEnsembleModal() {
        ensemblePickerMode = "upload";
        ensemblePickerMusicId = "";
        ensemblePickerSearchQuery = "";
    }

    function openScoreEnsembleModal(music: AdminMusic) {
        ensemblePickerMode = "score";
        ensemblePickerMusicId = music.id;
        ensemblePickerSearchQuery = "";
        stagedScoreEnsembleIds = [...music.ensemble_ids];
        openDownloadMenuFor = "";
    }

    function closeEnsemblePickerModal() {
        if (savingScoreEnsembles) return;
        ensemblePickerMode = "";
        ensemblePickerMusicId = "";
        ensemblePickerSearchQuery = "";
        stagedScoreEnsembleIds = [];
    }

    function toggleStagedEnsembleForScore(ensembleId: string) {
        if (stagedScoreEnsembleIds.includes(ensembleId)) {
            stagedScoreEnsembleIds = stagedScoreEnsembleIds.filter(
                (id) => id !== ensembleId,
            );
        } else {
            stagedScoreEnsembleIds = [...stagedScoreEnsembleIds, ensembleId];
        }
    }

    async function saveScoreEnsembles() {
        if (!activeEnsemblePickerMusic) return;
        const original = new Set(activeEnsemblePickerMusic.ensemble_ids);
        const staged = new Set(stagedScoreEnsembleIds);
        const toAdd = [...staged].filter((id) => !original.has(id));
        const toRemove = [...original].filter((id) => !staged.has(id));
        if (toAdd.length === 0 && toRemove.length === 0) {
            closeEnsemblePickerModal();
            return;
        }
        savingScoreEnsembles = true;
        try {
            for (const id of toAdd)
                await addMusicToEnsemble(activeEnsemblePickerMusic.id, id);
            for (const id of toRemove)
                await removeMusicFromEnsemble(activeEnsemblePickerMusic.id, id);
            await onRefresh();
            onSuccess("Score ensembles updated.");
            ensemblePickerMode = "";
            ensemblePickerMusicId = "";
            ensemblePickerSearchQuery = "";
            stagedScoreEnsembleIds = [];
        } catch (error) {
            onError(
                error instanceof Error
                    ? error.message
                    : "Failed to update ensembles",
            );
        } finally {
            savingScoreEnsembles = false;
        }
    }

    // Score metadata
    function openScoreMetadataModal(music: AdminMusic) {
        metadataMusicId = music.id;
        metadataTitle = music.title;
        metadataIcon = music.icon ?? "";
        metadataIconFile = null;
        metadataPublicId = music.public_id ?? "";
        openDownloadMenuFor = "";
    }

    function closeScoreMetadataModal() {
        if (savingMetadataFor) return;
        metadataMusicId = "";
        metadataTitle = "";
        metadataIcon = "";
        metadataIconFile = null;
        metadataPublicId = "";
    }

    async function handleSaveScoreMetadata() {
        if (!metadataMusicId) return;
        if (!metadataTitle.trim()) {
            onError("Title cannot be empty.");
            return;
        }
        savingMetadataFor = metadataMusicId;
        try {
            const updated = await updateMusicMetadata(metadataMusicId, {
                title: metadataTitle,
                publicId: metadataPublicId,
                icon: metadataIcon,
                iconFile: metadataIconFile,
            });
            onMusicUpdated(updated);
            closeScoreMetadataModal();
            onSuccess("Score metadata updated.");
        } catch (error) {
            onError(
                error instanceof Error
                    ? error.message
                    : "Unable to update score metadata",
            );
        } finally {
            savingMetadataFor = "";
        }
    }

    // Score info
    function openScoreInfoModal(music: AdminMusic) {
        infoMusicId = music.id;
        infoPlaytime = null;
        infoPlaytimeError = "";
        void loadScorePlaytime(music.id);
        openDownloadMenuFor = "";
    }

    function closeScoreInfoModal() {
        infoMusicId = "";
        infoPlaytime = null;
        infoPlaytimeLoading = false;
        infoPlaytimeError = "";
    }

    async function loadScorePlaytime(musicId: string) {
        infoPlaytimeLoading = true;
        infoPlaytimeError = "";
        try {
            const playtime = await fetchAdminMusicPlaytime(musicId);
            if (infoMusicId === musicId) {
                infoPlaytime = playtime;
            }
        } catch (error) {
            if (infoMusicId === musicId) {
                infoPlaytimeError =
                    error instanceof Error
                        ? error.message
                        : "Unable to load playtime";
            }
        } finally {
            if (infoMusicId === musicId) {
                infoPlaytimeLoading = false;
            }
        }
    }

    // Retry render
    async function handleRetryRender(musicId: string) {
        retryingFor = musicId;
        try {
            const updated = await retryRender(musicId);
            onMusicUpdated(updated);
            onSuccess("Render retried successfully.");
        } catch (error) {
            onError(error instanceof Error ? error.message : "Retry failed");
        } finally {
            retryingFor = "";
        }
    }

    // Delete
    async function deleteMusicAccount(musicId: string) {
        deletingMusicFor = musicId;
        try {
            await deleteMusic(musicId);
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
        openConfirm("Delete this score permanently?", "Delete", () =>
            deleteMusicAccount(musicId),
        );
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
                        class:download-open={openDownloadMenuFor === music.id}
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

{#if showCreateScoreModal}
    {#snippet uploadScoreFooter()}
        <div class="actions admin-user-modal-actions">
            <button
                class="button ghost"
                type="button"
                disabled={uploadBusy}
                onclick={() => closeCreateScoreModal()}>Cancel</button
            >
            <button
                class="button"
                type="button"
                disabled={uploadBusy}
                onclick={() => void handleUpload()}
            >
                {uploadBusy ? "Uploading..." : "Add score"}
            </button>
        </div>
    {/snippet}
    <BaseModal
        onClose={closeCreateScoreModal}
        size="large"
        cardClass="admin-score-modal"
        title="Upload"
        subtitle="Add a MuseScore score"
        footer={uploadScoreFooter}
    >
        <div class="upload-grid admin-score-modal-grid">
            <label class="field admin-score-modal-full">
                <span>Title</span>
                <input
                    bind:value={uploadTitle}
                    placeholder="Optional display title"
                />
            </label>
            <label class="field">
                <span>Public id</span>
                <input
                    bind:value={uploadPublicId}
                    placeholder="Optional friendly id"
                />
            </label>
            <label class="field admin-score-quality-field">
                <span>Stem quality</span>
                <CustomSelect
                    bind:value={uploadQualityProfile}
                    options={stemQualityOptions}
                    compact={true}
                    showDescriptionInTrigger={false}
                />
                <small class="subtle">
                    {STEM_QUALITY_PROFILES.find(
                        (o) => o.value === uploadQualityProfile,
                    )?.description}
                </small>
            </label>
            <label class="field file-field admin-score-file-field">
                <span>Icon image</span>
                <input
                    id="icon-file-input"
                    type="file"
                    accept="image/*"
                    onchange={(event) => {
                        const target = event.currentTarget as HTMLInputElement;
                        selectedIconFile = target.files?.[0] ?? null;
                    }}
                />
            </label>
            <label class="field file-field admin-score-file-field">
                <span>MSCZ file</span>
                <input
                    id="mscz-input"
                    type="file"
                    accept=".mscz"
                    onchange={handleFileSelection}
                />
            </label>
        </div>
        <button
            class="button ghost admin-score-ensemble-trigger"
            type="button"
            onclick={openUploadEnsembleModal}
        >
            <Users size={16} aria-hidden="true" />
            {uploadEnsembleIds.length > 0
                ? `Selected ensembles (${uploadEnsembleIds.length})`
                : "Choose ensembles"}
        </button>
    </BaseModal>
{/if}

{#if ensemblePickerMode}
    {#snippet ensemblePickerFooter()}
        {#if ensemblePickerMode === "score"}
            <div class="actions admin-user-modal-actions">
                <button
                    class="button ghost"
                    type="button"
                    disabled={savingScoreEnsembles}
                    onclick={closeEnsemblePickerModal}>Cancel</button
                >
                <button
                    class="button"
                    type="button"
                    disabled={savingScoreEnsembles}
                    onclick={() => void saveScoreEnsembles()}
                >
                    {savingScoreEnsembles ? "Saving..." : "Save changes"}
                </button>
            </div>
        {/if}
    {/snippet}
    <BaseModal
        onClose={closeEnsemblePickerModal}
        size="medium"
        cardClass="admin-selector-modal"
        title="Ensembles"
        subtitle={ensemblePickerMode === "upload"
            ? "Choose ensembles for the new score"
            : `Manage ensembles for ${activeEnsemblePickerMusic?.title ?? "score"}`}
        footer={ensemblePickerFooter}
    >
        <label class="field admin-user-search">
            <span class="sr-only">Search ensembles</span>
            <div class="admin-user-search-input-wrap">
                <Search size={15} aria-hidden="true" />
                <input
                    bind:value={ensemblePickerSearchQuery}
                    placeholder="Search ensembles"
                />
            </div>
        </label>
        <div class="admin-inline-list admin-selector-list">
            {#if filteredPickerEnsembles.length === 0}
                <p class="hint">No ensembles match this search.</p>
            {:else}
                {#each filteredPickerEnsembles as ensemble}
                    <label class="admin-selector-row">
                        <div class="admin-inline-copy">
                            <strong>{ensemble.name}</strong>
                            <span class="admin-user-role-pill">
                                {ensemble.members.length} members
                            </span>
                        </div>
                        {#if ensemblePickerMode === "upload"}
                            <input
                                type="checkbox"
                                checked={isUploadEnsembleSelected(ensemble.id)}
                                onchange={(event) =>
                                    toggleUploadEnsembleSelection(
                                        ensemble.id,
                                        (
                                            event.currentTarget as HTMLInputElement
                                        ).checked,
                                    )}
                            />
                        {:else if activeEnsemblePickerMusic}
                            <input
                                type="checkbox"
                                checked={stagedScoreEnsembleIds.includes(
                                    ensemble.id,
                                )}
                                onchange={() =>
                                    toggleStagedEnsembleForScore(ensemble.id)}
                            />
                        {/if}
                    </label>
                {/each}
            {/if}
        </div>
    </BaseModal>
{/if}

{#if activeMetadataMusic}
    {#snippet editScoreFooter()}
        <div class="actions admin-user-modal-actions">
            <button
                class="button ghost"
                type="button"
                disabled={!!savingMetadataFor}
                onclick={closeScoreMetadataModal}>Cancel</button
            >
            <button
                class="button"
                type="button"
                disabled={!!savingMetadataFor}
                onclick={() => void handleSaveScoreMetadata()}
            >
                {savingMetadataFor ? "Saving..." : "Save changes"}
            </button>
        </div>
    {/snippet}
    <BaseModal
        onClose={closeScoreMetadataModal}
        size="medium"
        cardClass="admin-score-modal"
        title="Edit score"
        subtitle={metadataTitle}
        footer={editScoreFooter}
    >
        <div class="upload-grid admin-score-modal-grid">
            <label class="field">
                <span>Title</span>
                <input bind:value={metadataTitle} />
            </label>
            <label class="field file-field admin-score-file-field">
                <span>Icon image</span>
                <input
                    id="metadata-icon-file-input"
                    type="file"
                    accept="image/*"
                    onchange={(event) => {
                        const target = event.currentTarget as HTMLInputElement;
                        metadataIconFile = target.files?.[0] ?? null;
                    }}
                />
            </label>
            <label class="field admin-score-modal-full">
                <span>Friendly public id</span>
                <input
                    bind:value={metadataPublicId}
                    placeholder="example: moonlight-sonata"
                />
            </label>
        </div>
    </BaseModal>
{/if}

{#if activeInfoMusic}
    <BaseModal
        onClose={closeScoreInfoModal}
        size="large"
        cardClass="admin-score-modal"
        title="Score info"
        subtitle={activeInfoMusic.title}
    >
        <div class="upload-grid admin-score-modal-grid">
            <label class="field">
                <span>MSCZ filename</span>
                <input value={activeInfoMusic.filename} readonly />
            </label>
            <label class="field">
                <span>Stem quality</span>
                <input
                    value={qualityProfileLabel(activeInfoMusic.quality_profile)}
                    readonly
                />
            </label>
            <label class="field admin-score-modal-full">
                <span>Ensembles</span>
                <input
                    value={activeInfoMusic.ensemble_names.join(", ") ||
                        "No ensemble"}
                    readonly
                />
            </label>
            <label class="field">
                <span>Stem files size</span>
                <input
                    value={`${formatBytes(activeInfoMusic.stems_total_bytes)} total`}
                    readonly
                />
            </label>
            <label class="field">
                <span>Uploaded</span>
                <input
                    value={prettyDate(activeInfoMusic.created_at)}
                    readonly
                />
            </label>
            <label class="field">
                <span>Stems status</span>
                <input value={activeInfoMusic.stems_status} readonly />
            </label>
            <label class="field">
                <span>Audio status</span>
                <input value={activeInfoMusic.audio_status} readonly />
            </label>
            <label class="field">
                <span>MIDI status</span>
                <input value={activeInfoMusic.midi_status} readonly />
            </label>
        </div>
        {#if activeInfoMusic.audio_error}
            <p class="hint">{activeInfoMusic.audio_error}</p>
        {/if}
        {#if activeInfoMusic.stems_error}
            <p class="hint">{activeInfoMusic.stems_error}</p>
        {/if}
        {#if activeInfoMusic.midi_error}
            <p class="hint">{activeInfoMusic.midi_error}</p>
        {/if}
        <section class="admin-playtime-section">
            <div class="admin-playtime-header">
                <div>
                    <p class="meta-label">Playtime</p>
                    <h3>Listening activity</h3>
                </div>
                {#if infoPlaytime}
                    <span class="status-pill admin-playtime-total-badge"
                        >{formatPlaytimeDuration(infoPlaytime.total_seconds)}
                        total</span
                    >
                {/if}
            </div>

            {#if infoPlaytimeLoading}
                <p class="hint">Loading playtime...</p>
            {:else if infoPlaytimeError}
                <p class="status error">{infoPlaytimeError}</p>
            {:else if infoPlaytime}
                <div class="admin-playtime-layout">
                    <section class="admin-playtime-card">
                        <h3>User leaderboard</h3>
                        {#if infoPlaytime.leaderboard.length === 0}
                            <p class="hint">
                                No user playtime has been recorded yet.
                            </p>
                        {:else}
                            <div class="admin-playtime-user-list">
                                {#each infoPlaytime.leaderboard as entry, index}
                                    <details
                                        class="admin-playtime-user-row"
                                    >
                                        <summary
                                            class="admin-playtime-user-head"
                                        >
                                            <div
                                                class="admin-user-avatar admin-playtime-avatar"
                                            >
                                                {#if entry.avatar_url}
                                                    <img
                                                        src={entry.avatar_url}
                                                        alt=""
                                                        class="admin-user-avatar-img"
                                                    />
                                                {:else}
                                                    {(entry.display_name ??
                                                        entry.username)
                                                        .slice(0, 1)
                                                        .toUpperCase()}
                                                {/if}
                                            </div>
                                            <div
                                                class="admin-playtime-user-copy"
                                            >
                                                <strong
                                                    >#{index + 1}
                                                    {entry.display_name ??
                                                        entry.username}</strong
                                                >
                                                <span class="subtle"
                                                    >@{entry.username}</span
                                                >
                                            </div>
                                            <span class="status-pill"
                                                >{formatPlaytimeDuration(
                                                    entry.best_track_seconds,
                                                )}</span
                                            >
                                        </summary>
                                        <div class="admin-playtime-track-list">
                                            {#each entry.track_totals as track}
                                                <article
                                                    class="admin-playtime-track-row"
                                                >
                                                    <div
                                                        class="admin-playtime-track-copy"
                                                    >
                                                        <strong
                                                            >{track.track_name}</strong
                                                        >
                                                        <span class="subtle"
                                                            >{track.instrument_name}</span
                                                        >
                                                    </div>
                                                    <strong
                                                        >{formatPlaytimeDuration(
                                                            track.total_seconds,
                                                        )}</strong
                                                    >
                                                </article>
                                            {/each}
                                        </div>
                                    </details>
                                {/each}
                            </div>
                        {/if}
                    </section>
                </div>
            {/if}
        </section>
        <div class="admin-score-links">
            <a
                href={activeInfoMusic.public_url}
                target="_blank"
                rel="noreferrer">Public link</a
            >
        </div>
        {#if activeInfoMusic.stems_status !== "ready" && canEditOwnedScore(activeInfoMusic, currentUser)}
            <div class="actions admin-user-modal-actions">
                <button
                    class="button ghost"
                    type="button"
                    disabled={retryingFor === activeInfoMusic.id}
                    onclick={() => void handleRetryRender(activeInfoMusic.id)}
                >
                    {retryingFor === activeInfoMusic.id
                        ? "Retrying render..."
                        : "Retry render"}
                </button>
            </div>
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
