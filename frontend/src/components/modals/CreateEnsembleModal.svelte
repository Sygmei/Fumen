<script lang="ts">
    import BaseModal from "./BaseModal.svelte";
    import { closeModal } from "./modalState";

    let {
        onCreate,
        modalId,
    }: {
        onCreate: (name: string) => Promise<void>;
        modalId?: string;
    } = $props();

    let name = $state("");
    let creating = $state(false);
    let errorMsg = $state("");

    async function handleSubmit() {
        const trimmed = name.trim();
        if (!trimmed) {
            errorMsg = "Choose an ensemble name first.";
            return;
        }

        creating = true;
        errorMsg = "";
        try {
            await onCreate(trimmed);
            closeModal();
        } catch (error) {
            errorMsg =
                error instanceof Error
                    ? error.message
                    : "Unable to create ensemble.";
        } finally {
            creating = false;
        }
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
            disabled={creating}
            onclick={closeModal}
        >
            Cancel
        </button>
        <button
            class="button"
            type="button"
            disabled={creating}
            onclick={() => void handleSubmit()}
        >
            {creating ? "Creating..." : "Create ensemble"}
        </button>
    </div>
{/snippet}

<BaseModal
    size="medium"
    cardClass="admin-user-modal"
    title="Create"
    subtitle="New ensemble"
    {footer}
    canClose={!creating}
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
