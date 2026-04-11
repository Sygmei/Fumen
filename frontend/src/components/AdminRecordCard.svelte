<script lang="ts">
    import type { Snippet } from "svelte";

    let {
        avatar,
        main,
        actions,
    }: {
        avatar?: Snippet;
        main: Snippet;
        actions: Snippet;
    } = $props();
</script>

<article class="music-card admin-record-card">
    <div class="admin-record-main">
        <div class="admin-record-avatar" aria-hidden="true">
            {#if avatar}
                {@render avatar()}
            {/if}
        </div>
        <div class="admin-record-copy">
            {@render main()}
        </div>
    </div>

    <div class="admin-record-actions">
        {@render actions()}
    </div>
</article>

<style>
    .admin-record-card {
        display: grid;
        grid-template-columns: minmax(0, 1fr) auto;
        grid-template-areas: "main actions";
        gap: 12px;
        align-items: center;
        width: 100%;
        height: 114px;
        min-height: 114px;
        max-height: 114px;
        padding: 10px 14px !important;
        overflow: hidden;
        align-self: stretch;
        justify-self: stretch;
        min-width: 0;
    }

    .admin-record-main {
        grid-area: main;
        display: flex;
        align-items: center;
        gap: 10px;
        min-width: 0;
        flex: 1 1 auto;
    }

    .admin-record-avatar {
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

    .admin-record-copy {
        display: grid;
        gap: 4px;
        min-width: 0;
    }

    .admin-record-copy :global(h3) {
        margin: 0;
        font-size: 0.95rem;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .admin-record-copy :global(.admin-user-role-pill) {
        width: fit-content;
        max-width: 100%;
    }

    .admin-record-actions {
        grid-area: actions;
        display: grid;
        grid-template-columns: repeat(3, 34px);
        gap: 8px;
        width: 118px;
        min-width: 118px;
        justify-content: end;
        align-self: center;
    }

    .admin-record-actions :global(.button) {
        width: 34px;
        min-width: 34px;
        height: 34px;
        min-height: 34px;
        padding: 0;
        border-radius: 0 !important;
        line-height: 1;
    }

    .admin-record-actions :global(.button svg) {
        display: block;
        flex-shrink: 0;
    }

    @media (max-width: 760px) {
        .admin-record-card {
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

        .admin-record-actions {
            width: 100%;
            min-width: 0;
            justify-content: stretch;
            align-self: stretch;
            grid-template-columns: repeat(3, minmax(0, 1fr));
        }

        .admin-record-actions :global(.button) {
            width: 100%;
            min-width: 0;
        }
    }
</style>
