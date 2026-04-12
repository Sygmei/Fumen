<script lang="ts">
    import { Music2 } from "@lucide/svelte";
    import BaseModal from "./BaseModal.svelte";

    let {
        enableCountIn,
        onToggleCountIn,
        onClose,
    }: {
        enableCountIn: boolean;
        onToggleCountIn: (value: boolean) => void;
        onClose: () => void;
    } = $props();
</script>

<BaseModal
    title="App settings"
    subtitle="Playback preferences"
    size="small"
    cardClass="app-config-modal"
    {onClose}
>
    <section class="grid gap-3">
        <div class="flex items-center gap-2.5">
            <span class="flex items-center justify-center w-8 h-8 border border-(--border-strong) bg-(--surface-alt) text-(--accent)">
                <Music2 size={16} aria-hidden="true" />
            </span>
            <div class="grid gap-0.5 min-w-0">
                <strong class="text-sm">Listen</strong>
                <p class="text-sm text-(--text-dim)">
                    Playback preferences for the listening experience.
                </p>
            </div>
        </div>

        <div class="toggle-grid">
            <label class="toggle-row">
                <span class="grid gap-0.5 min-w-0">
                    <strong>Enable count-in</strong>
                    <small>
                        Play a one-bar count-in before playback starts, matched
                        to the score tempo and beat.
                    </small>
                </span>
                <input
                    type="checkbox"
                    checked={enableCountIn}
                    onchange={(event) =>
                        onToggleCountIn(
                            (event.currentTarget as HTMLInputElement).checked,
                        )}
                />
            </label>
        </div>
    </section>
</BaseModal>
