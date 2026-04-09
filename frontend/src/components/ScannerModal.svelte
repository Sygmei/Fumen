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

<BaseModal {onClose} title="Scan" subtitle="Scan a QR code">
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
