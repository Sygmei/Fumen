<script lang="ts">
    import type { EnsembleRole } from "$lib/roles";
    import BaseModal from "./BaseModal.svelte";
    import CustomSelect from "../CustomSelect.svelte";
    import { closeModal } from "./modalState";
    import type { EnsembleRoleOption } from "./types";

    let {
        subtitle,
        initialValue,
        options,
        onApply,
        modalId,
    }: {
        subtitle: string;
        initialValue: EnsembleRole;
        options: EnsembleRoleOption[];
        onApply: (role: EnsembleRole) => void | Promise<void>;
        modalId?: string;
    } = $props();

    let value = $state(initialValue);
    let applying = $state(false);

    async function handleApply() {
        applying = true;
        try {
            await onApply(value);
            closeModal();
        } finally {
            applying = false;
        }
    }
</script>

{#snippet footer()}
    <div class="actions admin-user-modal-actions">
        <button class="button ghost" type="button" onclick={closeModal}>
            Cancel
        </button>
        <button
            class="button"
            type="button"
            onclick={() => void handleApply()}
            disabled={applying}
        >
            Apply
        </button>
    </div>
{/snippet}

<BaseModal
    size="small"
    cardClass="admin-role-picker-modal"
    title="Role"
    {subtitle}
    {footer}
    canClose={!applying}
    {modalId}
>
    <CustomSelect
        label="Role"
        bind:value
        options={options}
        compact={true}
        showDescriptionInTrigger={false}
    />
</BaseModal>
