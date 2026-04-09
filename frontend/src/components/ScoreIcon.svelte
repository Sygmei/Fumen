<script lang="ts">
    type ScoreIconVariant = "library" | "admin" | "listen";

    let {
        icon = null,
        imageUrl = null,
        variant = "admin",
    }: {
        icon?: string | null;
        imageUrl?: string | null;
        variant?: ScoreIconVariant;
    } = $props();

    function textClasses(currentVariant: ScoreIconVariant, isEmpty: boolean) {
        if (currentVariant === "library") {
            return `score-link-icon${isEmpty ? " is-empty" : ""}`;
        }

        if (currentVariant === "listen") {
            return `listen-score-icon${isEmpty ? " is-empty" : ""}`;
        }

        return "admin-score-icon";
    }

    function imageClasses(currentVariant: ScoreIconVariant) {
        if (currentVariant === "library") {
            return "score-link-icon score-link-icon-img";
        }

        if (currentVariant === "listen") {
            return "listen-score-icon listen-score-icon-img";
        }

        return "admin-score-icon-img";
    }
</script>

{#if imageUrl}
    <img
        class={imageClasses(variant)}
        src={imageUrl}
        alt=""
        aria-hidden="true"
    />
{:else if icon?.trim()}
    <span class={textClasses(variant, false)} aria-hidden="true">{icon?.trim() ?? ""}</span>
{:else}
    <span class={textClasses(variant, true)} aria-hidden="true"></span>
{/if}