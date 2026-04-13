<script lang="ts">
    import type { AdminMusicResponse as AdminMusic } from "$backend/models";
    import AdminCard from "./AdminCard.svelte";
    import ScoreIcon from "$components/ScoreIcon.svelte";
    import {
        QrCode,
        Trash2,
        Users,
        Pencil,
        Info,
        Download,
        ChevronDown,
        Music,
    } from "@lucide/svelte";

    let {
        music,
        processing = false,
        downloadOpen = false,
        deleting = false,
        canManageEnsembles = false,
        canEdit = false,
        canDelete = false,
        onToggleDownloadMenu,
        onManageEnsembles,
        onEdit,
        onShowQr,
        onShowInfo,
        onDelete,
        onCloseDownloadMenu,
    }: {
        music: AdminMusic;
        processing?: boolean;
        downloadOpen?: boolean;
        deleting?: boolean;
        canManageEnsembles?: boolean;
        canEdit?: boolean;
        canDelete?: boolean;
        onToggleDownloadMenu: () => void;
        onManageEnsembles: () => void;
        onEdit: () => void;
        onShowQr: () => void;
        onShowInfo: () => void;
        onDelete: () => void;
        onCloseDownloadMenu: () => void;
    } = $props();
</script>

{#snippet body()}
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
            {#if processing}
                <span class="status-pill admin-score-processing-pill">Processing</span>
            {/if}
        </h3>
        <div
            class="download-menu admin-score-download-menu"
            class:open={downloadOpen}
        >
            <button
                class="download-menu-btn admin-score-download-btn"
                type="button"
                onclick={onToggleDownloadMenu}
                aria-label={`Download files for ${music.title}`}
                title="Downloads"
                aria-haspopup="true"
                aria-expanded={downloadOpen}
            >
                <Download size={15} strokeWidth={2.2} />
                <span>Download</span>
                <ChevronDown
                    class="chevron"
                    size={12}
                    strokeWidth={2.5}
                />
            </button>
            {#if downloadOpen}
                <div class="download-dropdown">
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
            onclick={onShowQr}
            aria-label={`Share QR for ${music.title}`}
            title="Share QR code"
        >
            <QrCode size={16} aria-hidden="true" />
        </button>
        <button
            class="button secondary admin-user-action"
            type="button"
            onclick={onShowInfo}
            aria-label={`View metadata for ${music.title}`}
            title="View metadata"
        >
            <Info size={16} aria-hidden="true" />
        </button>
        {#if canDelete}
            <button
                class="button ghost danger admin-user-action"
                type="button"
                aria-label={`Delete ${music.title}`}
                title="Delete score"
                disabled={deleting}
                onclick={onDelete}
            >
                <Trash2 size={16} aria-hidden="true" />
            </button>
        {/if}
    </div>
{/snippet}

<AdminCard
    cardClass={`admin-score-card${downloadOpen ? " download-open" : ""}${processing ? " processing" : ""}`}
    {body}
    {footer}
    footerClass="admin-score-footer"
/>
