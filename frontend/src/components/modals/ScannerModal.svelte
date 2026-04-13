<script lang="ts">
    import { onDestroy, onMount } from "svelte";
    import QrScanner from "qr-scanner";
    import qrWorkerUrl from "qr-scanner/qr-scanner-worker.min?url";
    import BaseModal from "./BaseModal.svelte";
    import { closeModal } from "./modalState";

    let {
        onConnectToken,
        onClose = () => {},
        videoEl = $bindable<HTMLVideoElement | null>(null),
        modalId,
    }: {
        onConnectToken: (token: string) => Promise<void>;
        onClose?: () => void;
        videoEl?: HTMLVideoElement | null;
        modalId?: string;
    } = $props();

    (QrScanner as unknown as { WORKER_PATH: string }).WORKER_PATH = qrWorkerUrl;

    let error = $state("");
    let scanner: QrScanner | null = null;

    function extractConnectionToken(value: string): string | null {
        const trimmed = value.trim();
        if (!trimmed) {
            return null;
        }

        try {
            const parsed = new URL(trimmed, window.location.origin);
            const match = parsed.pathname.match(/^\/connect\/([^/]+)$/);
            if (match) {
                return decodeURIComponent(match[1]);
            }
        } catch {
            // Ignore malformed URLs and try local patterns below.
        }

        const pathMatch = trimmed.match(/^\/connect\/([^/]+)$/);
        if (pathMatch) {
            return decodeURIComponent(pathMatch[1]);
        }

        if (/^[a-zA-Z0-9]+$/.test(trimmed)) {
            return trimmed;
        }

        return null;
    }

    onMount(() => {
        const start = async () => {
            if (!videoEl) {
                error = "Camera preview is unavailable on this device.";
                return;
            }

            try {
                scanner?.destroy();
                scanner = new QrScanner(
                    videoEl,
                    async (result) => {
                        const value =
                            typeof result === "string" ? result : result.data;
                        const token = extractConnectionToken(value);
                        if (!token) {
                            error = "That QR code is not a valid Fumen connection link.";
                            return;
                        }

                        try {
                            await onConnectToken(token);
                            closeModal();
                        } catch (scanError) {
                            error =
                                scanError instanceof Error
                                    ? scanError.message
                                    : "Unable to use this connection link";
                        }
                    },
                    {
                        highlightScanRegion: true,
                        highlightCodeOutline: true,
                    },
                );
                await scanner.start();
            } catch (scanError) {
                error =
                    scanError instanceof Error
                        ? scanError.message
                        : "Unable to start the camera";
            }
        };

        void start();
    });

    onDestroy(() => {
        scanner?.stop();
        scanner?.destroy();
        scanner = null;
    });
</script>

<BaseModal
    {onClose}
    size="large"
    cardClass="scanner-modal"
    title="Scan"
    subtitle="Scan QR code"
    {modalId}
>
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
