<script lang="ts">
    import BaseModal from "./BaseModal.svelte";
    import CustomSelect from "../CustomSelect.svelte";
    import { closeModal } from "./modalState";
    import type { GlobalRole } from "$lib/roles";
    import type { GlobalRoleOption } from "./types";

    let {
        defaultRole = "user",
        roleOptions,
        onCreate,
        modalId,
    }: {
        defaultRole?: Exclude<GlobalRole, "superadmin">;
        roleOptions: GlobalRoleOption[];
        onCreate: (
            username: string,
            role: Exclude<GlobalRole, "superadmin">,
        ) => void | Promise<void>;
        modalId?: string;
    } = $props();

    let username = $state("");
    let role = $state(defaultRole);
    let errorMsg = $state("");

    function handleSubmit() {
        const trimmed = username.trim();
        if (!trimmed) {
            errorMsg = "Choose a username first.";
            return;
        }

        errorMsg = "";
        void onCreate(trimmed, role);
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
            Create user
        </button>
    </div>
{/snippet}

<BaseModal
    size="medium"
    cardClass="admin-user-modal"
    title="Create"
    subtitle="New account"
    {footer}
    {modalId}
>
    <p class="subtle">
        Create a username-only account, assign its global role, then generate a
        QR code or connection link from the list.
    </p>
    <label class="field">
        <span>Username</span>
        <input
            bind:value={username}
            placeholder="example: lucas"
            onkeydown={handleKeydown}
        />
    </label>
    <CustomSelect
        label="Global role"
        bind:value={role}
        options={roleOptions}
    />
    {#if errorMsg}
        <p class="admin-error">{errorMsg}</p>
    {/if}
</BaseModal>
