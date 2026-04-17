<script lang="ts">
    import { Users } from "@lucide/svelte";
    import type { AdminEnsembleResponse as Ensemble } from "$backend/models";
    import {
        STEM_QUALITY_PROFILES,
        type StemQualityProfile,
    } from "$lib/stem-quality";
    import BaseModal from "./BaseModal.svelte";
    import CustomSelect from "$components/CustomSelect.svelte";
    import ScoreEnsemblePickerModal from "./ScoreEnsemblePickerModal.svelte";
    import { closeModal, pushModal } from "./modalState";
    import type { UploadScoreDraft } from "./types";

    let {
        ensembles,
        onUpload,
        modalId,
    }: {
        ensembles: Ensemble[];
        onUpload: (draft: UploadScoreDraft) => void | Promise<void>;
        modalId?: string;
    } = $props();

    const qualityOptions = STEM_QUALITY_PROFILES.map((profile) => ({
        value: profile.value,
        label: profile.label,
        description: profile.description,
    }));

    let title = $state("");
    let subtitle = $state("");
    let publicId = $state("");
    let qualityProfile = $state<StemQualityProfile>("standard");
    let selectedFile = $state<File | null>(null);
    let selectedIconFile = $state<File | null>(null);
    let selectedEnsembleIds = $state(ensembles[0] ? [ensembles[0].id] : []);
    let errorMsg = $state("");

    function openEnsemblePicker() {
        pushModal({
            modal: ScoreEnsemblePickerModal,
            props: {
                mode: "upload",
                ensembles,
                initialSelectedEnsembleIds: selectedEnsembleIds,
                onApply: (ensembleIds: string[]) => {
                    selectedEnsembleIds = ensembleIds;
                },
            },
        });
    }

    function handleUpload() {
        if (!selectedFile) {
            errorMsg = "Choose an .mscz file first.";
            return;
        }

        errorMsg = "";
        void onUpload({
            title,
            subtitle,
            publicId,
            qualityProfile,
            file: selectedFile,
            iconFile: selectedIconFile,
            ensembleIds: selectedEnsembleIds,
        });
        closeModal();
    }
</script>

{#snippet footer()}
    <div class="actions admin-user-modal-actions">
        <button
            class="button ghost"
            type="button"
            onclick={closeModal}
        >
            Cancel
        </button>
        <button
            class="button"
            type="button"
            onclick={handleUpload}
        >
            Add score
        </button>
    </div>
{/snippet}

<BaseModal
    size="large"
    cardClass="admin-score-modal"
    title="Upload"
    subtitle="Add a MuseScore score"
    {footer}
    {modalId}
>
    <div class="upload-grid admin-score-modal-grid">
        <label class="field admin-score-modal-full">
            <span>Title</span>
            <input bind:value={title} placeholder="Optional display title" />
        </label>
        <label class="field admin-score-modal-full">
            <span>Subtitle</span>
            <input
                bind:value={subtitle}
                placeholder="Optional subtitle"
            />
        </label>
        <label class="field">
            <span>Public id</span>
            <input
                bind:value={publicId}
                placeholder="Optional friendly id"
            />
        </label>
        <label class="field admin-score-quality-field">
            <span>Stem quality</span>
            <CustomSelect
                bind:value={qualityProfile}
                options={qualityOptions}
                compact={true}
                showDescriptionInTrigger={false}
            />
            <small class="subtle">
                {STEM_QUALITY_PROFILES.find(
                    (option) => option.value === qualityProfile,
                )?.description}
            </small>
        </label>
        <label class="field file-field admin-score-file-field">
            <span>Icon image</span>
            <input
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
                type="file"
                accept=".mscz"
                onchange={(event) => {
                    const target = event.currentTarget as HTMLInputElement;
                    selectedFile = target.files?.[0] ?? null;
                }}
            />
        </label>
    </div>
    <button
        class="button ghost admin-score-ensemble-trigger"
        type="button"
        onclick={openEnsemblePicker}
    >
        <Users size={16} aria-hidden="true" />
        {selectedEnsembleIds.length > 0
            ? `Selected ensembles (${selectedEnsembleIds.length})`
            : "Choose ensembles"}
    </button>
    {#if errorMsg}
        <p class="admin-error">{errorMsg}</p>
    {/if}
</BaseModal>
