<script lang="ts">
    import { MessageSquare, Music2 } from "@lucide/svelte";
    import BaseModal from "./BaseModal.svelte";

    type AnnotationDefaultPlacement = "above" | "below";

    let {
        enableCountIn,
        countInMeasures,
        annotationDefaultPlacement = "below",
        onToggleCountIn,
        onChangeCountInMeasures,
        onChangeAnnotationDefaultPlacement,
        onClose = () => {},
        modalId,
    }: {
        enableCountIn: boolean;
        countInMeasures: number;
        annotationDefaultPlacement?: AnnotationDefaultPlacement;
        onToggleCountIn: (value: boolean) => void;
        onChangeCountInMeasures: (value: number) => void;
        onChangeAnnotationDefaultPlacement?: (
            value: AnnotationDefaultPlacement,
        ) => void;
        onClose?: () => void;
        modalId?: string;
    } = $props();
</script>

<BaseModal
    title="App settings"
    subtitle="Playback and annotation preferences"
    size="small"
    cardClass="app-config-modal"
    {onClose}
    {modalId}
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

        <div class="flex items-center gap-2.5 pt-1">
            <span class="flex items-center justify-center w-8 h-8 border border-(--border-strong) bg-(--surface-alt) text-(--accent)">
                <MessageSquare size={16} aria-hidden="true" />
            </span>
            <div class="grid gap-0.5 min-w-0">
                <strong class="text-sm">Annotations</strong>
                <p class="text-sm text-(--text-dim)">
                    Control the default bubble direction for instruments in the
                    middle of the score.
                </p>
            </div>
        </div>

        <div class="toggle-grid">
            <div class="toggle-row">
                <span class="grid gap-0.5 min-w-0">
                    <strong>Default annotation position</strong>
                    <small>
                        Top two staves always open below the note, bottom two
                        always open above it.
                    </small>
                </span>
                <select
                    class="w-full min-w-[14rem] max-w-[18rem] rounded-md border border-(--border-strong) bg-(--surface-alt) px-2.5 py-1.5 text-sm text-(--text) outline-none"
                    value={annotationDefaultPlacement}
                    onchange={(event) =>
                        onChangeAnnotationDefaultPlacement?.(
                            (event.currentTarget as HTMLSelectElement).value as AnnotationDefaultPlacement,
                        )}
                >
                    <option value="below">Below the staff (arrow on top)</option>
                    <option value="above">Above the staff (arrow on bottom)</option>
                </select>
            </div>
        </div>
    </section>
</BaseModal>
