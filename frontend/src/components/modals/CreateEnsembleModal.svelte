<script lang="ts">
    import BaseModal from "./BaseModal.svelte";
    import { closeModal } from "./modalState";

    let {
        onCreate,
        modalId,
    }: {
        onCreate: (name: string) => void | Promise<void>;
        modalId?: string;
    } = $props();

    let name = $state("");
    let errorMsg = $state("");

    function handleSubmit() {
        const trimmed = name.trim();
        if (!trimmed) {
            errorMsg = "Choose an ensemble name first.";
            return;
        }

        errorMsg = "";
        void onCreate(trimmed);
        closeModal();
    }

    function handleKeydown(event: KeyboardEvent) {
        if (event.key === "Enter") {
            event.preventDefault();
            void handleSubmit();
        }
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
            onclick={handleSubmit}
        >
            Create ensemble
        </button>
    </div>
{/snippet}

<BaseModal
    size="medium"
    cardClass="admin-user-modal"
    title="Create"
    subtitle="New ensemble"
    {footer}
    {modalId}
>
    <label class="field">
        <span>Ensemble name</span>
        <input
            bind:value={name}
            placeholder="example: Strings"
            onkeydown={handleKeydown}
        />
    </label>
    {#if errorMsg}
        <p class="admin-error">{errorMsg}</p>
    {/if}
</BaseModal>
