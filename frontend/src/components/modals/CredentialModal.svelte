<script lang="ts">
    import { onMount } from "svelte";
    import BaseModal from "./BaseModal.svelte";
    import type { LoginLinkResponse } from "$backend/models";
    import { prettyDate } from "$lib/utils";
    import QRCode from "qrcode";

    let {
        title,
        loadLink,
        onClose = () => {},
        eyebrow = "Temporary access",
        linkLabel = "Connection link",
        modalId,
    }: {
        title: string;
        loadLink: () => Promise<LoginLinkResponse>;
        onClose?: () => void;
        eyebrow?: string;
        linkLabel?: string;
        modalId?: string;
    } = $props();

    let qrDataUrl = $state("");
    let isLoading = $state(true);
    let link = $state("");
    let expiresAt = $state("");

    onMount(() => {
        let cancelled = false;

        const run = async () => {
            isLoading = true;
            try {
                const linkResponse = await loadLink();
                if (cancelled) return;
                link = linkResponse.connection_url;
                expiresAt = linkResponse.expires_at;
                qrDataUrl = await QRCode.toDataURL(linkResponse.connection_url, {
                    width: 360,
                    margin: 1,
                    color: {
                        dark: "#111111",
                        light: "#0000",
                    },
                });
            } finally {
                if (!cancelled) {
                    isLoading = false;
                }
            }
        };

        void run();

        return () => {
            cancelled = true;
        };
    });

    function placeholderCell(index: number) {
        const hash = Array.from(`${link}:${index}`).reduce(
            (acc, char) => (acc * 33 + char.charCodeAt(0)) % 9973,
            17,
        );

        return {
            filled: hash % 5 !== 0,
            delay: `${(hash % 9) * 120}ms`,
            duration: `${2600 + (hash % 7) * 180}ms`,
        };
    }
</script>

<BaseModal
    {onClose}
    size="large"
    title={eyebrow}
    subtitle={title}
    {modalId}
>
    {#if qrDataUrl}
        <img class="qr-preview" src={qrDataUrl} alt={title} />
    {:else if isLoading}
        <div class="qr-preview qr-preview-placeholder" aria-hidden="true">
            <div class="qr-placeholder-grid">
                {#each Array.from({ length: 81 }) as _, index}
                    {@const cell = placeholderCell(index)}
                    <span
                        class:filled={cell.filled}
                        style={`animation-delay: ${cell.delay}; animation-duration: ${cell.duration};`}
                    ></span>
                {/each}
            </div>
        </div>
    {/if}
    <p class="hint">
        {#if expiresAt}
            Valid until {prettyDate(expiresAt)}
        {:else if isLoading}
            Generating a temporary connection link...
        {/if}
    </p>
    <div class="field">
        <span>{linkLabel}</span>
        <input
            value={link}
            readonly
            placeholder={isLoading ? "Preparing secure link..." : ""}
        />
    </div>
    <button
        class="button"
        disabled={!link}
        onclick={() => void navigator.clipboard.writeText(link)}
    >
        {link ? "Copy link" : "Preparing link..."}
    </button>
</BaseModal>

<style>
    .qr-preview-placeholder {
        display: grid;
        place-items: center;
        overflow: hidden;
        background: #fff;
    }

    .qr-placeholder-grid {
        display: grid;
        grid-template-columns: repeat(9, 1fr);
        gap: 10px;
        width: min(100%, 268px);
        aspect-ratio: 1;
        filter: blur(7px);
        transform: scale(1.05);
    }

    .qr-placeholder-grid span {
        border-radius: 0;
        background: rgba(17, 17, 17, 0.08);
        animation: qr-breathe 3s ease-in-out infinite;
    }

    .qr-placeholder-grid span.filled {
        background: rgba(17, 17, 17, 0.82);
    }

    @keyframes qr-breathe {
        0%,
        100% {
            opacity: 0.42;
            transform: scale(0.92);
        }

        50% {
            opacity: 0.92;
            transform: scale(1.04);
        }
    }
</style>
