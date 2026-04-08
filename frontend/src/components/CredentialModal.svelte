<script lang="ts">
    import BaseModal from "./BaseModal.svelte";
    import { prettyDate } from "../lib/utils";

    const {
        title,
        qrDataUrl,
        link,
        expiresAt,
        onClose,
    }: {
        title: string;
        qrDataUrl: string;
        link: string;
        expiresAt: string;
        onClose: () => void;
    } = $props();
</script>

<BaseModal {onClose}>
    <div class="card-header">
        <div>
            <p class="meta-label">Temporary access</p>
            <h2>{title}</h2>
        </div>
        <button class="button ghost" onclick={onClose}>Close</button>
    </div>
    {#if qrDataUrl}
        <img class="qr-preview" src={qrDataUrl} alt={title} />
    {/if}
    <p class="hint">Valid until {prettyDate(expiresAt)}</p>
    <div class="field">
        <span>Connection link</span>
        <input value={link} readonly />
    </div>
    <button
        class="button"
        onclick={() => void navigator.clipboard.writeText(link)}
    >
        Copy link
    </button>
</BaseModal>
