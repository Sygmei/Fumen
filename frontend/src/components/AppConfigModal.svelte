<script lang="ts">
    import { Music2 } from "@lucide/svelte";
    import BaseModal from "./BaseModal.svelte";

    let {
        enableCountIn,
        countInMeasures,
        onToggleCountIn,
        onChangeCountInMeasures,
        onClose,
    }: {
        enableCountIn: boolean;
        countInMeasures: number;
        onToggleCountIn: (value: boolean) => void;
        onChangeCountInMeasures: (value: number) => void;
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
            <div class="toggle-row">
                <span class="grid gap-0.5 min-w-0">
                    <strong>Enable count-in</strong>
                    <small>
                        Play a count-in before playback starts, matched to the
                        score tempo and beat.
                    </small>
                </span>
                <div class="flex items-center gap-2">
                    <label class="flex items-center gap-2">
                        <span class="sr-only">Enable count-in</span>
                        <input
                            type="checkbox"
                            checked={enableCountIn}
                            onchange={(event) =>
                                onToggleCountIn(
                                    (event.currentTarget as HTMLInputElement).checked,
                                )}
                        />
                    </label>
                    <label class="flex items-center gap-2 text-sm text-(--text-dim)">
                        <span>Count-in measures</span>
                        <input
                            type="number"
                            min="1"
                            step="1"
                            value={countInMeasures}
                            disabled={!enableCountIn}
                            onchange={(event) =>
                                onChangeCountInMeasures(
                                    Math.max(
                                        1,
                                        Math.floor(
                                            Number(
                                                (event.currentTarget as HTMLInputElement)
                                                    .value,
                                            ) || 1,
                                        ),
                                    ),
                                )}
                            class="w-20 rounded-md border border-(--border-strong) bg-(--surface-alt) px-2 py-1 text-sm text-(--text) outline-none disabled:opacity-50"
                        />
                    </label>
                </div>
            </div>
        </div>
    </section>
</BaseModal>
