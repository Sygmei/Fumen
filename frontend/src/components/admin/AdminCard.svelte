<script lang="ts">
    import type { Snippet } from "svelte";

    let {
        cardClass = "",
        body,
        footer,
        footerClass = "",
    }: {
        cardClass?: string;
        body: Snippet;
        footer?: Snippet;
        footerClass?: string;
    } = $props();
</script>

<article class={`music-card admin-card ${cardClass}`.trim()}>
    <div class="admin-card-accent" aria-hidden="true"></div>
    <div class="admin-card-body">
        {@render body()}
    </div>
    {#if footer}
        <div class={`admin-card-footer ${footerClass}`.trim()}>
            {@render footer()}
        </div>
    {/if}
</article>

<style>
    .admin-card {
        position: relative;
        overflow: hidden;
    }

    .admin-card-accent {
        position: absolute;
        inset: 0 auto auto 0;
        width: 100%;
        height: 3px;
        background: linear-gradient(90deg, var(--accent), var(--accent-hover));
        opacity: 0.98;
        pointer-events: none;
    }

    .admin-card-body,
    .admin-card-footer {
        position: relative;
        z-index: 1;
    }

    :global(.admin-record-card) {
        display: grid;
        grid-template-columns: minmax(0, 1fr) auto;
        grid-template-areas: "main actions";
        gap: 12px;
        align-items: center;
        width: 100%;
        height: 114px;
        min-height: 114px;
        max-height: 114px;
        padding: 14px !important;
        overflow: hidden;
        align-self: stretch;
        justify-self: stretch;
        min-width: 0;
    }

    :global(.admin-record-card .admin-card-body) {
        grid-area: main;
        min-width: 0;
    }

    :global(.admin-record-card .admin-card-footer) {
        grid-area: actions;
    }

    :global(.admin-record-main) {
        display: flex;
        align-items: center;
        gap: 10px;
        min-width: 0;
        flex: 1 1 auto;
    }

    :global(.admin-record-avatar) {
        display: grid;
        place-items: center;
        width: 2rem;
        height: 2rem;
        flex-shrink: 0;
        border: 1px solid var(--border-strong);
        background: var(--surface-alt);
        color: var(--accent);
        font-family: 'Plus Jakarta Sans', system-ui, sans-serif;
        font-size: 0.9rem;
        font-weight: 800;
        overflow: hidden;
        border-radius: inherit;
    }

    :global(.admin-record-avatar-gradient) {
        background:
            linear-gradient(135deg, rgba(255, 255, 255, 0.14), transparent 60%),
            linear-gradient(90deg, var(--accent), var(--accent-hover));
        color: white;
        box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.16);
    }

    :global(.admin-record-copy) {
        display: grid;
        gap: 4px;
        min-width: 0;
    }

    :global(.admin-record-copy h3) {
        margin: 0;
        font-size: 0.95rem;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    :global(.admin-record-actions) {
        display: grid;
        grid-template-columns: repeat(4, 34px);
        gap: 8px;
        width: 152px;
        min-width: 152px;
        justify-content: end;
        align-self: center;
    }

    :global(.admin-record-actions .button) {
        width: 34px;
        min-width: 34px;
        height: 34px;
        min-height: 34px;
        padding: 0;
        border-radius: 0 !important;
        line-height: 1;
    }

    :global(.admin-record-actions .button svg) {
        display: block;
        flex-shrink: 0;
    }

    @media (max-width: 760px) {
        :global(.admin-record-card) {
            grid-template-columns: 1fr;
            grid-template-rows: minmax(0, 1fr) auto;
            grid-template-areas:
                "main"
                "actions";
            align-items: stretch;
            height: auto;
            min-height: 114px;
            max-height: none;
        }

        :global(.admin-record-actions) {
            width: 100%;
            min-width: 0;
            justify-content: stretch;
            align-self: stretch;
            grid-template-columns: repeat(4, minmax(0, 1fr));
        }

        :global(.admin-record-actions .button) {
            width: 100%;
            min-width: 0;
        }
    }
</style>
