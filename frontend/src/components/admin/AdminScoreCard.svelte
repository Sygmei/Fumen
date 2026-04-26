<script lang="ts">
    import { tick } from "svelte";
    import type { AdminMusicResponse as AdminMusic } from "$backend/models";
    import { portal } from "$lib/portal";
    import AdminCard from "./AdminCard.svelte";
    import ScoreIcon from "$components/ScoreIcon.svelte";
    import {
        QrCode,
        Trash2,
        Users,
        Pencil,
        Info,
        Download,
        Music,
        RotateCcw,
    } from "@lucide/svelte";

    let {
        music,
        creating = false,
        saving = false,
        restarting = false,
        processing = false,
        downloadOpen = false,
        deleting = false,
        canManageEnsembles = false,
        canEdit = false,
        canDelete = false,
        showRestartAction = false,
        onToggleDownloadMenu,
        onManageEnsembles,
        onEdit,
        onShowQr,
        onShowInfo,
        onRestartProcessing,
        onDelete,
        onCloseDownloadMenu,
    }: {
        music: AdminMusic;
        creating?: boolean;
        saving?: boolean;
        restarting?: boolean;
        processing?: boolean;
        downloadOpen?: boolean;
        deleting?: boolean;
        canManageEnsembles?: boolean;
        canEdit?: boolean;
        canDelete?: boolean;
        showRestartAction?: boolean;
        onToggleDownloadMenu: () => void;
        onManageEnsembles: () => void;
        onEdit: () => void;
        onShowQr: () => void;
        onShowInfo: () => void;
        onRestartProcessing: () => void;
        onDelete: () => void;
        onCloseDownloadMenu: () => void;
    } = $props();

    const cardState = $derived(
        deleting
            ? "deleting"
            : restarting
              ? "restarting"
              : saving
                ? "saving"
                : creating
                  ? "creating"
                  : "",
    );
    const actionsDisabled = $derived(creating || saving || restarting || deleting);
    const failed = $derived(
        music.processing_job_status === "failed" ||
            [
                music.audio_status,
                music.midi_status,
                music.musicxml_status,
                music.stems_status,
            ].includes("failed"),
    );

    let downloadMenuRoot = $state<HTMLElement | null>(null);
    let downloadMenuButton = $state<HTMLButtonElement | null>(null);
    let downloadMenuPanel = $state<HTMLElement | null>(null);
    let downloadMenuLeft = $state(0);
    let downloadMenuTop = $state<number | null>(null);
    let downloadMenuBottom = $state<number | null>(null);
    let downloadMenuWidth = $state(208);

    function handleWindowPointerDown(event: PointerEvent) {
        if (!downloadOpen) {
            return;
        }

        const target = event.target;
        if (
            target instanceof Node &&
            !downloadMenuRoot?.contains(target) &&
            !downloadMenuPanel?.contains(target)
        ) {
            onCloseDownloadMenu();
        }
    }

    async function updateDownloadMenuPlacement() {
        if (!downloadOpen || !downloadMenuButton) {
            return;
        }

        await tick();

        const rect = downloadMenuButton.getBoundingClientRect();
        const panelHeight = downloadMenuPanel?.offsetHeight ?? 0;
        const viewport = window.visualViewport;
        const viewportWidth = viewport?.width ?? window.innerWidth;
        const viewportHeight = viewport?.height ?? window.innerHeight;
        const margin = viewportWidth <= 640 ? 12 : 16;
        const gap = 6;

        downloadMenuWidth = Math.max(208, rect.width);
        downloadMenuLeft = Math.min(
            Math.max(margin, rect.right - downloadMenuWidth),
            Math.max(margin, viewportWidth - downloadMenuWidth - margin),
        );

        const fitsBelow =
            rect.bottom + gap + panelHeight <= viewportHeight - margin;
        if (fitsBelow || rect.top < panelHeight + gap + margin) {
            downloadMenuTop = rect.bottom + gap;
            downloadMenuBottom = null;
        } else {
            downloadMenuTop = null;
            downloadMenuBottom = viewportHeight - rect.top + gap;
        }
    }

    function handleViewportChange() {
        if (downloadOpen) {
            void updateDownloadMenuPlacement();
        }
    }

    $effect(() => {
        if (downloadOpen) {
            void updateDownloadMenuPlacement();
        }
    });

    $effect(() => {
        const viewport = window.visualViewport;
        viewport?.addEventListener("resize", handleViewportChange);
        viewport?.addEventListener("scroll", handleViewportChange);
        document.addEventListener("scroll", handleViewportChange, true);

        return () => {
            viewport?.removeEventListener("resize", handleViewportChange);
            viewport?.removeEventListener("scroll", handleViewportChange);
            document.removeEventListener("scroll", handleViewportChange, true);
        };
    });
</script>

<svelte:window
    onpointerdown={handleWindowPointerDown}
    onresize={handleViewportChange}
    onscroll={handleViewportChange}
/>

{#snippet body()}
    <div class="admin-score-header">
        <div class="admin-score-main">
            <ScoreIcon
                variant="admin"
                icon={music.icon}
                imageUrl={music.icon_image_url}
            />
            <h3 class="admin-score-title">
                {#if creating || !music.public_url}
                    <span class="admin-score-copy">
                        <span class="admin-score-primary">{music.title}</span>
                        {#if music.subtitle}
                            <span class="admin-score-subtitle">{music.subtitle}</span>
                        {/if}
                    </span>
                {:else}
                    <a
                        class="admin-score-copy"
                        href={music.public_url}
                        target="_blank"
                        rel="noreferrer"
                    >
                        <span class="admin-score-primary">{music.title}</span>
                        {#if music.subtitle}
                            <span class="admin-score-subtitle">{music.subtitle}</span>
                        {/if}
                    </a>
                {/if}
            </h3>
            {#if creating || saving || restarting || deleting || processing || failed}
                <div class="admin-score-badges">
                    {#if creating}
                        <span class="status-pill admin-user-creating-pill">CREATING</span>
                    {:else if restarting}
                        <span class="status-pill admin-user-saving-pill">RESTARTING</span>
                    {:else if saving}
                        <span class="status-pill admin-user-saving-pill">SAVING</span>
                    {:else if deleting}
                        <span class="status-pill admin-user-deleting-pill">DELETING</span>
                    {/if}
                    {#if failed}
                        <span class="status-pill admin-score-failed-pill">Failed</span>
                    {:else if processing}
                        <span class="status-pill admin-score-processing-pill">Processing</span>
                    {/if}
                </div>
            {/if}
        </div>
        <div
            class="download-menu admin-score-download-menu"
            class:open={downloadOpen}
            bind:this={downloadMenuRoot}
        >
            <button
                class="download-menu-btn admin-score-download-btn"
                type="button"
                disabled={actionsDisabled}
                onclick={onToggleDownloadMenu}
                aria-label={`Download files for ${music.title}`}
                title="Downloads"
                aria-haspopup="true"
                aria-expanded={downloadOpen}
                bind:this={downloadMenuButton}
            >
                <Download size={15} strokeWidth={2.2} />
            </button>
            {#if downloadOpen}
                <div
                    class="download-dropdown admin-score-download-dropdown"
                    bind:this={downloadMenuPanel}
                    use:portal
                    style={`position: fixed; left: ${downloadMenuLeft}px; width: ${downloadMenuWidth}px; ${
                        downloadMenuTop === null ? "" : `top: ${downloadMenuTop}px;`
                    } ${downloadMenuBottom === null ? "" : `bottom: ${downloadMenuBottom}px;`}`}
                >
                    <a
                        class="download-item"
                        href={music.download_url}
                        target="_blank"
                        rel="noreferrer"
                        onclick={onCloseDownloadMenu}
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
                            onclick={onCloseDownloadMenu}
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
{/snippet}

{#snippet footer()}
    <div class="actions admin-score-actions">
        {#if canManageEnsembles}
            <button
                class="button secondary admin-user-action"
                type="button"
                disabled={actionsDisabled}
                onclick={onManageEnsembles}
                aria-label={`Manage ensembles for ${music.title}`}
                title="Manage ensembles"
            >
                <Users size={16} aria-hidden="true" />
                <span class="admin-action-badge" aria-hidden="true"
                    >{music.ensemble_names.length}</span
                >
            </button>
        {/if}
        {#if canEdit}
            <button
                class="button secondary admin-user-action"
                type="button"
                disabled={actionsDisabled}
                onclick={onEdit}
                aria-label={`Edit metadata for ${music.title}`}
                title="Edit metadata"
            >
                <Pencil size={16} aria-hidden="true" />
            </button>
        {/if}
        <button
            class="button secondary admin-user-action"
            type="button"
            disabled={actionsDisabled}
            onclick={onShowQr}
            aria-label={`Share QR for ${music.title}`}
            title="Share QR code"
        >
            <QrCode size={16} aria-hidden="true" />
        </button>
        <button
            class="button secondary admin-user-action"
            type="button"
            disabled={actionsDisabled}
            onclick={onShowInfo}
            aria-label={`View metadata for ${music.title}`}
            title="View metadata"
        >
            <Info size={16} aria-hidden="true" />
        </button>
        {#if showRestartAction && canEdit}
            <button
                class="button secondary admin-user-action"
                type="button"
                disabled={actionsDisabled}
                onclick={onRestartProcessing}
                aria-label={`Restart processing for ${music.title}`}
                title="Restart processing"
            >
                <RotateCcw size={16} aria-hidden="true" />
            </button>
        {/if}
        {#if canDelete}
            <button
                class="button secondary danger admin-user-action"
                type="button"
                aria-label={`Delete ${music.title}`}
                title="Delete score"
                disabled={actionsDisabled}
                onclick={onDelete}
            >
                <Trash2 size={16} aria-hidden="true" />
            </button>
        {/if}
    </div>
{/snippet}

<AdminCard
    cardClass={`admin-score-card${downloadOpen ? " download-open" : ""}${processing ? " processing" : ""}${failed ? " is-failed" : ""}${cardState ? ` is-${cardState}` : ""}`}
    {body}
    {footer}
    footerClass="admin-score-footer"
/>
