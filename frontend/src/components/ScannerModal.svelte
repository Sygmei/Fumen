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

<BaseModal {onClose}>
    <div class="card-header">
        <div>
            <p class="meta-label">Scan</p>
            <h2>Scan a QR code</h2>
        </div>
        <button class="button ghost" onclick={onClose}>Close</button>
    </div>
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
