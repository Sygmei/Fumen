<script lang="ts">
    import BaseModal from "./BaseModal.svelte";
    import { prettyDate } from "../lib/utils";

    const {
        title,
        qrDataUrl,
        isLoading = false,
        link,
        expiresAt,
        onClose,
    }: {
        title: string;
        qrDataUrl: string;
        isLoading?: boolean;
        link: string;
        expiresAt: string;
        onClose: () => void;
    } = $props();

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

{#snippet modalHeader()}
    <div class="card-header">
        <div>
            <p class="meta-label">Temporary access</p>
            <h2>{title}</h2>
        </div>
        <button
            class="button ghost admin-modal-close"
            onclick={onClose}
            aria-label="Close credential modal"
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

<BaseModal {onClose} size="large" header={modalHeader}>
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
        <span>Connection link</span>
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
        background:
            radial-gradient(circle at 30% 20%, rgba(196, 43, 13, 0.16), transparent 42%),
            radial-gradient(circle at 70% 80%, rgba(26, 23, 18, 0.12), transparent 46%),
            #fff;
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
