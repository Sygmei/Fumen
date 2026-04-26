<script lang="ts">
    import LogBlockEntry from "./LogBlockEntry.svelte";
    import processingLogProvider from "./processingLogProvider";
    import { defaultLinkify } from "./shared";
    import type {
        LogBlockProps,
        LogEntry,
        LogGroupEntry,
        LogLineEntry,
        LogProvider,
    } from "./types";

    type Props = LogBlockProps;

    let {
        logs = "",
        provider = processingLogProvider,
        title = "Logs",
        emptyMessage = "No logs to display.",
        showLevelFilter = true,
        showDownloadButton = true,
        downloadFileName = undefined,
        initialGroupsCollapsed = false,
        selectedLevels = $bindable<string[]>([]),
        className = "",
    }: Props = $props();

    let collapsedGroupIds = $state<Record<string, boolean>>({});
    let levelDropdownOpen = $state(false);
    let levelDropdownElement = $state<HTMLDivElement | undefined>(undefined);

    function splitInputLines(input: string | string[]): string[] {
        const lines = Array.isArray(input)
            ? input.flatMap((line) => line.split(/\r?\n/))
            : !input
              ? []
              : input.split(/\r?\n/);

        let start = 0;
        let end = lines.length - 1;

        while (start <= end && lines[start].trim().length === 0) {
            start += 1;
        }

        while (end >= start && lines[end].trim().length === 0) {
            end -= 1;
        }

        return lines.slice(start, end + 1);
    }

    function makeEntryId(prefix: string, lineIndex: number, depth: number): string {
        return `${prefix}-${lineIndex}-${depth}`;
    }

    function parseEntries(
        rawLogs: string | string[],
        logProvider: LogProvider,
    ): {
        entries: LogEntry[];
        levels: string[];
        groupIds: string[];
    } {
        const entries: LogEntry[] = [];
        const groupStack: LogGroupEntry[] = [];
        const levels: string[] = [];

        const lines = splitInputLines(rawLogs);

        const appendEntry = (entry: LogEntry): void => {
            if (groupStack.length === 0) {
                entries.push(entry);
                return;
            }

            groupStack[groupStack.length - 1].entries.push(entry);
        };

        for (let index = 0; index < lines.length; index++) {
            const line = lines[index];
            const parsed = logProvider.parseLine(line, index);

            if (parsed.type === "skip") {
                continue;
            }

            if (parsed.type === "groupStart") {
                const groupEntry: LogGroupEntry = {
                    kind: "group",
                    id: makeEntryId("group", index, groupStack.length),
                    raw: parsed.raw,
                    sourceLine: index + 1,
                    title: parsed.title,
                    entries: [],
                };

                appendEntry(groupEntry);
                groupStack.push(groupEntry);
                continue;
            }

            if (parsed.type === "groupEnd") {
                if (groupStack.length > 0) {
                    groupStack.pop();
                }
                continue;
            }

            const normalizedLevel = parsed.line.level
                ? (logProvider.normalizeLevel?.(parsed.line.level) ?? parsed.line.level)
                : undefined;

            if (normalizedLevel && !levels.includes(normalizedLevel)) {
                levels.push(normalizedLevel);
            }

            const linkify = logProvider.linkify ?? ((text: string) => defaultLinkify(text));
            const lineEntry: LogLineEntry = {
                kind: "line",
                id: makeEntryId("line", index, groupStack.length),
                raw: parsed.line.raw,
                sourceLine: index + 1,
                timestamp: parsed.line.timestamp,
                level: parsed.line.level,
                normalizedLevel,
                message: parsed.line.message,
                metadata: parsed.line.metadata,
                group: parsed.line.group,
                messageParts: linkify(parsed.line.message, parsed.line),
            };

            appendEntry(lineEntry);
        }

        const groupedEntries = logProvider.groupEntries?.(entries) ?? entries;

        return { entries: groupedEntries, levels, groupIds: collectGroupIds(groupedEntries) };
    }

    function collectGroupIds(entries: LogEntry[]): string[] {
        const groupIds: string[] = [];
        for (const entry of entries) {
            if (entry.kind !== "group") {
                continue;
            }

            groupIds.push(entry.id);
            groupIds.push(...collectGroupIds(entry.entries));
        }
        return groupIds;
    }

    const parsedData = $derived(parseEntries(logs, provider));
    const rawLogsText = $derived.by(() => {
        if (Array.isArray(logs)) {
            return logs.join("\n");
        }

        return logs || "";
    });
    const hasRawLogs = $derived(rawLogsText.length > 0);

    const levelOptions = $derived.by(() => {
        const levels = [...parsedData.levels];
        const orderedLevels = provider.levelOrder ?? [];

        levels.sort((a, b) => {
            const leftIndex = orderedLevels.indexOf(a);
            const rightIndex = orderedLevels.indexOf(b);

            if (leftIndex === -1 && rightIndex === -1) {
                return a.localeCompare(b);
            }

            if (leftIndex === -1) {
                return 1;
            }

            if (rightIndex === -1) {
                return -1;
            }

            return leftIndex - rightIndex;
        });

        return levels;
    });

    $effect(() => {
        const cleanedLevels = selectedLevels.filter((level) => levelOptions.includes(level));
        const normalizedLevels =
            levelOptions.length > 0 && cleanedLevels.length === levelOptions.length
                ? []
                : cleanedLevels;

        if (
            normalizedLevels.length !== selectedLevels.length ||
            normalizedLevels.some((level, index) => level !== selectedLevels[index])
        ) {
            selectedLevels = normalizedLevels;
        }
    });

    $effect(() => {
        if (!levelDropdownOpen) {
            return;
        }

        const handlePointerDown = (event: PointerEvent): void => {
            const target = event.target;
            if (!(target instanceof Node)) {
                return;
            }

            if (levelDropdownElement && !levelDropdownElement.contains(target)) {
                levelDropdownOpen = false;
            }
        };

        const handleEscape = (event: KeyboardEvent): void => {
            if (event.key === "Escape") {
                levelDropdownOpen = false;
            }
        };

        window.addEventListener("pointerdown", handlePointerDown);
        window.addEventListener("keydown", handleEscape);

        return () => {
            window.removeEventListener("pointerdown", handlePointerDown);
            window.removeEventListener("keydown", handleEscape);
        };
    });

    $effect(() => {
        const previousState = collapsedGroupIds;
        const nextState: Record<string, boolean> = {};
        let hasChanged = parsedData.groupIds.length !== Object.keys(previousState).length;

        for (const groupId of parsedData.groupIds) {
            const nextValue =
                previousState[groupId] !== undefined
                    ? previousState[groupId]
                    : initialGroupsCollapsed;

            nextState[groupId] = nextValue;

            if (previousState[groupId] !== nextValue) {
                hasChanged = true;
            }
        }

        if (hasChanged) {
            collapsedGroupIds = nextState;
        }
    });

    function filterEntriesByLevel(entries: LogEntry[], activeLevels: string[]): LogEntry[] {
        if (activeLevels.length === 0) {
            return entries;
        }

        const filteredEntries: LogEntry[] = [];

        for (const entry of entries) {
            if (entry.kind === "line") {
                if (!entry.normalizedLevel || activeLevels.includes(entry.normalizedLevel)) {
                    filteredEntries.push(entry);
                }
                continue;
            }

            const filteredChildren = filterEntriesByLevel(entry.entries, activeLevels);
            if (filteredChildren.length === 0) {
                continue;
            }

            filteredEntries.push({
                ...entry,
                entries: filteredChildren,
            });
        }

        return filteredEntries;
    }

    const visibleEntries = $derived(filterEntriesByLevel(parsedData.entries, selectedLevels));
    const hasHeader = $derived(
        title.trim().length > 0 || showDownloadButton || (showLevelFilter && levelOptions.length > 0),
    );

    function toggleGroup(groupId: string): void {
        collapsedGroupIds = {
            ...collapsedGroupIds,
            [groupId]: !collapsedGroupIds[groupId],
        };
    }

    function toggleLevel(level: string): void {
        if (selectedLevels.includes(level)) {
            selectedLevels = selectedLevels.filter((selectedLevel) => selectedLevel !== level);
            return;
        }

        selectedLevels = [...selectedLevels, level];
    }

    function resetLevelFilter(): void {
        selectedLevels = [];
    }

    function isLevelChecked(level: string): boolean {
        return selectedLevels.length === 0 || selectedLevels.includes(level);
    }

    function toggleLevelFromDropdown(level: string): void {
        if (selectedLevels.length === 0) {
            selectedLevels = levelOptions.filter((levelOption) => levelOption !== level);
            return;
        }

        toggleLevel(level);
    }

    function levelFilterLabel(): string {
        if (selectedLevels.length === 0) {
            return "All levels";
        }

        if (selectedLevels.length === 1) {
            return displayLevel(selectedLevels[0]);
        }

        return `${selectedLevels.length} levels`;
    }

    function displayLevel(level: string): string {
        const normalized = provider.normalizeLevel?.(level) ?? level;
        return provider.displayLevel?.(normalized) ?? normalized;
    }

    function resolveDownloadFileName(): string {
        if (downloadFileName?.trim()) {
            return downloadFileName.trim();
        }

        const fallback = title
            .trim()
            .toLowerCase()
            .replace(/[^a-z0-9]+/g, "-")
            .replace(/^-+|-+$/g, "");

        return `${fallback || "logs"}.log`;
    }

    function downloadRawLogs(): void {
        if (!rawLogsText) {
            return;
        }

        const blob = new Blob([rawLogsText], { type: "text/plain;charset=utf-8" });
        const objectUrl = URL.createObjectURL(blob);
        const anchor = document.createElement("a");

        anchor.href = objectUrl;
        anchor.download = resolveDownloadFileName();
        anchor.style.display = "none";
        document.body.append(anchor);
        anchor.click();
        anchor.remove();

        setTimeout(() => URL.revokeObjectURL(objectUrl), 0);
    }
</script>

<style>
    .log-block {
        display: grid;
        gap: 10px;
    }

    .log-block-header {
        display: flex;
        flex-wrap: wrap;
        align-items: center;
        justify-content: space-between;
        gap: 10px;
    }

    .log-block-title {
        margin: 0;
        color: var(--accent);
        font-size: 0.72rem;
        font-weight: 600;
        letter-spacing: 0.14em;
        text-transform: uppercase;
    }

    .log-block-actions {
        display: flex;
        align-items: center;
        gap: 8px;
        flex-wrap: wrap;
    }

    .log-block-filter {
        position: relative;
    }

    .log-block-button {
        min-height: 32px;
        padding: 0 12px;
        font-size: 0.74rem;
        font-weight: 700;
        letter-spacing: 0.03em;
        box-shadow: none;
    }

    .log-block-button:disabled {
        opacity: 0.5;
    }

    .log-block-button.is-active {
        border-color: color-mix(in srgb, var(--accent) 28%, var(--border-strong));
        background: var(--accent-faint);
        color: var(--accent);
    }

    .log-block-button.is-active:hover {
        background: color-mix(in srgb, var(--accent-faint) 74%, var(--surface));
        color: var(--accent);
        box-shadow: none;
    }

    .log-block-button-tag {
        color: var(--text-dim);
        font-size: 0.66rem;
        font-weight: 700;
        letter-spacing: 0.12em;
        text-transform: uppercase;
    }

    .log-block-button-label {
        color: var(--text-dim);
        font-weight: 600;
        letter-spacing: 0;
        text-transform: none;
    }

    .log-block-chevron {
        color: var(--text-dim);
        transition: transform 150ms ease;
    }

    .log-block-chevron.is-open {
        transform: rotate(180deg);
    }

    .log-block-menu {
        position: absolute;
        right: 0;
        top: calc(100% + 8px);
        width: 208px;
        border: 1px solid var(--border-strong);
        background: var(--surface);
        box-shadow: var(--shadow-lg);
        z-index: 20;
        overflow: hidden;
        isolation: isolate;
    }

    .log-block-menu-header {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 8px;
        padding: 8px 10px;
        border-bottom: 1px solid var(--border);
        background: color-mix(in srgb, var(--surface-alt) 74%, var(--surface));
    }

    .log-block-menu-link {
        border: 0;
        padding: 0;
        background: transparent;
        color: var(--accent);
        font-size: 0.72rem;
        font-weight: 700;
        cursor: pointer;
    }

    .log-block-menu-link:disabled {
        opacity: 0.4;
        cursor: not-allowed;
    }

    .log-block-menu-body {
        display: grid;
        gap: 4px;
        max-height: 224px;
        overflow: auto;
        padding: 6px;
        background:
            linear-gradient(180deg, color-mix(in srgb, var(--surface) 82%, var(--surface-alt)), var(--surface));
    }

    .log-block-menu-option {
        display: flex;
        align-items: center;
        gap: 8px;
        width: 100%;
        padding: 7px 8px;
        border: 1px solid transparent;
        background: transparent;
        color: var(--text);
        font-size: 0.72rem;
        font-weight: 600;
        text-align: left;
        transition: background 150ms ease, border-color 150ms ease, color 150ms ease;
    }

    .log-block-menu-option:hover {
        background: var(--surface-alt);
        border-color: var(--border);
    }

    .log-block-menu-option.is-checked {
        background: var(--accent-faint);
        border-color: color-mix(in srgb, var(--accent) 18%, var(--border));
        color: var(--accent);
    }

    .log-block-check {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        width: 14px;
        height: 14px;
        border: 1px solid var(--border-strong);
        background: var(--surface-alt);
        font-size: 0.62rem;
        line-height: 1;
        color: transparent;
    }

    .log-block-check.is-checked {
        border-color: color-mix(in srgb, var(--accent) 24%, var(--border-strong));
        background: var(--accent-faint);
        color: var(--accent);
    }

    .log-block-level-name {
        font-family: 'Fira Code', 'Cascadia Code', monospace;
        letter-spacing: 0.04em;
    }

    .log-block-body {
        position: relative;
        border: 1px solid var(--border);
        background:
            linear-gradient(180deg, color-mix(in srgb, var(--surface-alt) 58%, var(--surface)), var(--surface));
        max-height: 32rem;
        overflow: auto;
        font-family: 'Fira Code', 'Cascadia Code', monospace;
        box-shadow: var(--shadow-sm);
    }

    .log-block-body::before {
        content: "";
        position: sticky;
        top: 0;
        display: block;
        width: 100%;
        height: 2px;
        background: linear-gradient(90deg, var(--accent), color-mix(in srgb, var(--amber) 72%, var(--accent)));
        z-index: 1;
    }

    .log-block-empty {
        margin: 0;
        padding: 12px;
        color: var(--text-dim);
        font-size: 0.8rem;
    }
</style>

<div class={`log-block ${className}`}>
    {#if hasHeader}
        <header class="log-block-header">
            <h2 class="log-block-title">{title}</h2>

            <div class="log-block-actions">
                {#if showDownloadButton}
                    <button
                        type="button"
                        class="button secondary log-block-button"
                        onclick={downloadRawLogs}
                        disabled={!hasRawLogs}
                    >
                        <span>Download raw</span>
                    </button>
                {/if}

                {#if showLevelFilter && levelOptions.length > 0}
                    <div class="log-block-filter" bind:this={levelDropdownElement}>
                        <button
                            type="button"
                            class={`button secondary log-block-button ${selectedLevels.length === 0 ? "is-active" : ""}`}
                            aria-haspopup="menu"
                            aria-expanded={levelDropdownOpen}
                            onclick={() => (levelDropdownOpen = !levelDropdownOpen)}
                        >
                            <span class="log-block-button-tag">Levels</span>
                            <span class="log-block-button-label">{levelFilterLabel()}</span>
                            <span class={`log-block-chevron ${levelDropdownOpen ? "is-open" : ""}`}>v</span>
                        </button>

                        {#if levelDropdownOpen}
                            <div
                                class="log-block-menu"
                                role="menu"
                                aria-label="Filter levels"
                            >
                                <div class="log-block-menu-header">
                                    <button
                                        type="button"
                                        class="log-block-menu-link"
                                        onclick={() => {
                                            resetLevelFilter();
                                            levelDropdownOpen = false;
                                        }}
                                    >
                                        All levels
                                    </button>
                                    <button
                                        type="button"
                                        class="log-block-menu-link"
                                        onclick={() => {
                                            selectedLevels = [];
                                            levelDropdownOpen = false;
                                        }}
                                        disabled={selectedLevels.length === 0}
                                    >
                                        Clear
                                    </button>
                                </div>

                                <div class="log-block-menu-body">
                                    {#each levelOptions as level (level)}
                                        <button
                                            type="button"
                                            role="menuitemcheckbox"
                                            aria-checked={isLevelChecked(level)}
                                            class={`log-block-menu-option ${isLevelChecked(level) ? "is-checked" : ""}`}
                                            onclick={() => toggleLevelFromDropdown(level)}
                                        >
                                            <span class={`log-block-check ${isLevelChecked(level) ? "is-checked" : ""}`}>
                                                x
                                            </span>
                                            <span class="log-block-level-name">{displayLevel(level)}</span>
                                        </button>
                                    {/each}
                                </div>
                            </div>
                        {/if}
                    </div>
                {/if}
            </div>
        </header>
    {/if}

    <div class="log-block-body">
        {#if visibleEntries.length === 0}
            <p class="log-block-empty">{emptyMessage}</p>
        {:else}
            {#each visibleEntries as entry (entry.id)}
                <LogBlockEntry
                    {entry}
                    collapsedGroupIds={collapsedGroupIds}
                    {toggleGroup}
                    {displayLevel}
                />
            {/each}
        {/if}
    </div>
</div>
