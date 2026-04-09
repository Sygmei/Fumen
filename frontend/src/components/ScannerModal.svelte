<script lang="ts">
    import BaseModal from "./BaseModal.svelte";

    let {
        error,
        onClose,
        videoEl = $bindable(),
    }: {
        error: string;
        onClose: () => void;
        videoEl?: HTMLVideoElement | null;
    } = $props();
</script>

{#snippet modalHeader()}
    <div class="card-header">
        <div>
            <p class="meta-label">Scan</p>
            <h2>Scan a QR code</h2>
        </div>
        <button
            class="button ghost admin-modal-close"
            onclick={onClose}
            aria-label="Close scanner modal"
        >
            <svg
                width="16"
                height="16"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
                aria-hidden="true"
            >
                <path d="M18 6 6 18" />
                <path d="m6 6 12 12" />
            </svg>
        </button>
    </div>
{/snippet}

<BaseModal {onClose} header={modalHeader}>
    <div class="scanner-frame">
        <video class="scanner-video" bind:this={videoEl} muted playsinline
        ></video>
    </div>
    <p class="hint">
        Point the camera at a Fumen QR code to connect this device.
    </p>
    {#if error}
        <p class="status error">{error}</p>
    {/if}
</BaseModal>
