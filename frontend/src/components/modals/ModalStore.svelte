<script lang="ts">
    import { modalStack } from "./modalState";
</script>

{#each $modalStack as modalSettings}
    {@const CurrentModalComponent = modalSettings.modal}
    <CurrentModalComponent {...modalSettings.props} modalId={modalSettings.id} />
{/each}
