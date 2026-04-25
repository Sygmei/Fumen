<script lang="ts">
    import LogBlockEntry from "./LogBlockEntry.svelte";
    import type { LogEntry, LogGroupEntry } from "./types";

    interface Props {
        entry: LogEntry;
        depth?: number;
        collapsedGroupIds?: Record<string, boolean>;
        toggleGroup?(groupId: string): void;
        displayLevel?(level: string): string;
    }

    let {
        entry,
        depth = 0,
        collapsedGroupIds = {},
        toggleGroup = () => {},
        displayLevel = (level: string) => level,
    }: Props = $props();

    function getGroupState(groupEntry: LogGroupEntry): boolean {
        return Boolean(collapsedGroupIds[groupEntry.id]);
    }

    function levelClass(level: string | undefined): string {
        const normalized = (level || "").toUpperCase();

        if (normalized === "ERROR" || normalized === "CRITICAL") {
            return "is-error";
        }

        if (normalized === "WARNING" || normalized === "WARN") {
            return "is-warning";
        }

        if (normalized === "INFO") {
            return "is-info";
        }

        if (normalized === "DEBUG") {
            return "is-debug";
        }

        return "is-other";
    }

    interface ParsedTimestamp {
        year: string;
        month: string;
        day: string;
        separator: string;
        hour: string;
        minute: string;
        second: string;
        fractional?: string;
        timezone?: string;
    }

    function parseTimestamp(timestamp: string | undefined): ParsedTimestamp | null {
        if (!timestamp) {
            return null;
        }

        const match = timestamp.match(
            /^(?<year>\d{4})-(?<month>\d{2})-(?<day>\d{2})(?<separator>[T ])(?<hour>\d{2}):(?<minute>\d{2}):(?<second>\d{2})(?<fractional>\.\d+)?(?<timezone>Z|[+-]\d{2}:\d{2})?$/,
        );

        if (!match?.groups) {
            return null;
        }

        return {
            year: match.groups.year,
            month: match.groups.month,
            day: match.groups.day,
            separator: match.groups.separator,
            hour: match.groups.hour,
            minute: match.groups.minute,
            second: match.groups.second,
            fractional: match.groups.fractional,
            timezone: match.groups.timezone,
        };
    }

    type HighlightTokenKind = "text" | "string" | "number" | "path";

    interface HighlightToken {
        kind: HighlightTokenKind;
        text: string;
    }

    const HIGHLIGHT_PATTERN =
        /`(?:\\.|[^`\\])*`|"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'|(?:[A-Za-z]:\\[^\s"'`]+|(?:\.{1,2}\/|\/)[^\s"'`]+)|\b-?\d+(?:\.\d+)?\b/g;
    const PATH_PATTERN = /^(?:[A-Za-z]:\\[^\s"'`]+|(?:\.{1,2}\/|\/)[^\s"'`]+)$/;
    const NUMBER_PATTERN = /^-?\d+(?:\.\d+)?$/;

    function tokenizeSyntax(text: string): HighlightToken[] {
        if (!text) {
            return [{ kind: "text", text: "" }];
        }

        const tokens: HighlightToken[] = [];
        let lastIndex = 0;
        HIGHLIGHT_PATTERN.lastIndex = 0;

        let match: RegExpExecArray | null;
        while ((match = HIGHLIGHT_PATTERN.exec(text)) !== null) {
            const matchedText = match[0];
            const start = match.index;
            const end = start + matchedText.length;

            if (start > lastIndex) {
                tokens.push({
                    kind: "text",
                    text: text.slice(lastIndex, start),
                });
            }

            let kind: HighlightTokenKind = "text";
            if (
                (matchedText.startsWith("'") && matchedText.endsWith("'")) ||
                (matchedText.startsWith('"') && matchedText.endsWith('"')) ||
                (matchedText.startsWith("`") && matchedText.endsWith("`"))
            ) {
                kind = "string";
            } else if (PATH_PATTERN.test(matchedText)) {
                kind = "path";
            } else if (NUMBER_PATTERN.test(matchedText)) {
                kind = "number";
            }

            tokens.push({ kind, text: matchedText });
            lastIndex = end;
        }

        if (lastIndex < text.length) {
            tokens.push({
                kind: "text",
                text: text.slice(lastIndex),
            });
        }

        return tokens.length > 0 ? tokens : [{ kind: "text", text }];
    }

    function syntaxTokenClass(kind: HighlightTokenKind): string {
        if (kind === "string") {
            return "log-token-string";
        }

        if (kind === "number") {
            return "log-token-number";
        }

        if (kind === "path") {
            return "log-token-path";
        }

        return "";
    }
</script>

<style>
    .log-group {
        margin: 4px 0;
    }

    .log-group-toggle {
        display: flex;
        align-items: center;
        gap: 10px;
        width: 100%;
        padding: 8px 10px;
        border: 1px solid var(--border);
        background: var(--surface);
        color: var(--text);
        text-align: left;
        transition: background 150ms ease, border-color 150ms ease;
    }

    .log-group-toggle:hover {
        background: color-mix(in srgb, var(--surface) 88%, white);
        border-color: var(--border-strong);
    }

    .log-group-caret {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        width: 16px;
        height: 16px;
        border: 1px solid var(--border);
        background: var(--surface-alt);
        color: var(--text-dim);
        font-size: 0.65rem;
        line-height: 1;
        flex-shrink: 0;
    }

    .log-group-label {
        color: var(--text-dim);
        font-size: 0.64rem;
        font-weight: 700;
        letter-spacing: 0.09em;
        text-transform: uppercase;
    }

    .log-group-title {
        min-width: 0;
        color: var(--text);
        font-size: 0.76rem;
        font-weight: 700;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }

    .log-group-count {
        margin-left: auto;
        display: inline-flex;
        align-items: center;
        min-height: 18px;
        padding: 0 6px;
        border: 1px solid var(--border);
        background: var(--surface-alt);
        color: var(--text-dim);
        font-size: 0.66rem;
        font-family: 'Fira Code', 'Cascadia Code', monospace;
    }

    .log-group-children {
        position: relative;
        padding-top: 4px;
    }

    .log-group-children::before {
        content: "";
        position: absolute;
        left: 0;
        top: 6px;
        bottom: 8px;
        width: 1px;
        background: color-mix(in srgb, var(--accent) 16%, var(--border));
    }

    .log-row {
        display: grid;
        grid-template-columns: max-content minmax(0, 1fr);
        align-items: start;
        gap: 10px;
        padding: 8px 10px;
        border-bottom: 1px solid color-mix(in srgb, var(--border) 86%, transparent);
        transition: background 120ms ease;
    }

    .log-row:hover {
        background: color-mix(in srgb, var(--surface-alt) 62%, transparent);
    }

    .log-row:last-child {
        border-bottom: 0;
    }

    .log-timestamp {
        display: inline-flex;
        align-items: center;
        min-height: 18px;
        padding: 0;
        border: 0;
        background: transparent;
        color: var(--text-dim);
        font-size: 0.68rem;
        font-family: 'Fira Code', 'Cascadia Code', monospace;
        line-height: 1;
        white-space: nowrap;
        letter-spacing: 0.01em;
    }

    .log-timestamp.is-placeholder {
        visibility: hidden;
        user-select: none;
    }

    .log-ts-year,
    .log-ts-time {
        color: var(--accent);
    }

    .log-ts-month,
    .log-ts-day {
        color: color-mix(in srgb, var(--accent) 68%, var(--text));
    }

    .log-ts-fraction {
        color: var(--amber);
    }

    .log-ts-zone {
        color: var(--text-dim);
    }

    .log-level {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        min-height: 20px;
        margin-right: 6px;
        margin-bottom: 1px;
        padding: 0 7px;
        font-size: 0.64rem;
        font-weight: 700;
        font-family: 'Fira Code', 'Cascadia Code', monospace;
        letter-spacing: 0.08em;
        text-transform: uppercase;
    }

    .log-level.is-error {
        border: 1px solid color-mix(in srgb, var(--danger) 28%, var(--border));
        background: color-mix(in srgb, var(--danger) 10%, var(--surface));
        color: var(--danger);
    }

    .log-level.is-warning {
        border: 1px solid color-mix(in srgb, var(--amber) 30%, var(--border));
        background: color-mix(in srgb, var(--amber) 12%, var(--surface));
        color: var(--amber);
    }

    .log-level.is-info {
        border: 1px solid color-mix(in srgb, var(--accent) 24%, var(--border));
        background: color-mix(in srgb, var(--accent) 8%, var(--surface));
        color: var(--accent);
    }

    .log-level.is-debug {
        border: 1px solid var(--border);
        background: var(--surface-alt);
        color: var(--text-dim);
    }

    .log-level.is-other {
        border: 1px solid color-mix(in srgb, var(--success) 28%, var(--border));
        background: color-mix(in srgb, var(--success) 11%, var(--surface));
        color: var(--success);
    }

    .log-message {
        min-width: 0;
        color: var(--text);
        font-size: 0.78rem;
        line-height: 1.5;
        word-break: break-word;
    }

    .log-link {
        border: 0;
        padding: 0;
        background: transparent;
        color: var(--accent);
        text-decoration: underline;
        text-underline-offset: 2px;
        cursor: pointer;
    }

    .log-link:hover {
        color: var(--accent-hover);
    }

    .log-metadata {
        color: var(--text-dim);
    }

    .log-token-string {
        color: color-mix(in srgb, var(--accent) 70%, var(--text));
    }

    .log-token-number {
        color: var(--amber);
    }

    .log-token-path {
        color: var(--accent);
        font-weight: 500;
        text-decoration: underline;
        text-decoration-color: color-mix(in srgb, var(--accent) 42%, transparent);
        text-underline-offset: 0.12em;
    }
</style>

{#if entry.kind === "group"}
    <div class="log-group" style={`margin-left: ${depth * 16}px;`}>
        <button
            type="button"
            class="log-group-toggle"
            onclick={() => toggleGroup(entry.id)}
            aria-expanded={!getGroupState(entry)}
            aria-label={getGroupState(entry) ? `Expand ${entry.title}` : `Collapse ${entry.title}`}
        >
            <span class="log-group-caret">
                {getGroupState(entry) ? ">" : "v"}
            </span>
            <span class="log-group-label">group</span>
            <span class="log-group-title">{entry.title}</span>
            <span class="log-group-count">
                {entry.entries.length}
            </span>
        </button>

        {#if !getGroupState(entry)}
            <div class="log-group-children">
                {#each entry.entries as childEntry (childEntry.id)}
                    <LogBlockEntry
                        entry={childEntry}
                        depth={depth + 1}
                        {collapsedGroupIds}
                        {toggleGroup}
                        {displayLevel}
                    />
                {/each}
            </div>
        {/if}
    </div>
{:else}
    {@const parsedTimestamp = parseTimestamp(entry.timestamp)}
    <div
        class="log-row"
        style={`margin-left: ${depth * 16}px;`}
    >
        {#if parsedTimestamp}
            <span class="log-timestamp"><span class="log-ts-year">{parsedTimestamp.year}</span><span>-</span><span class="log-ts-month">{parsedTimestamp.month}</span><span>-</span><span class="log-ts-day">{parsedTimestamp.day}</span><span>{parsedTimestamp.separator}</span><span class="log-ts-time">{parsedTimestamp.hour}</span><span>:</span><span class="log-ts-time">{parsedTimestamp.minute}</span><span>:</span><span class="log-ts-time">{parsedTimestamp.second}</span>{#if parsedTimestamp.fractional}<span class="log-ts-fraction">{parsedTimestamp.fractional}</span>{/if}{#if parsedTimestamp.timezone}<span class="log-ts-zone">{parsedTimestamp.timezone}</span>{/if}</span>
        {:else if entry.timestamp}
            <span class="log-timestamp">
                {entry.timestamp}
            </span>
        {:else}
            <span
                class="log-timestamp is-placeholder"
                aria-hidden="true"
            >
                0000-00-00T00:00:00.000+00:00
            </span>
        {/if}

        <span class="log-message">
            {#if entry.level}
                <span
                    class={`log-level ${levelClass(entry.level)}`}
                >
                    {displayLevel(entry.level)}
                </span>
            {/if}
            {#each entry.messageParts as part, partIndex (`${part.kind}-${part.text}-${partIndex}`)}
                {#if part.kind === "link" && part.href}
                    <button
                        type="button"
                        onclick={() => window.open(part.href, "_blank", "noopener,noreferrer")}
                        class="log-link"
                    >
                        {part.text}
                    </button>
                {:else}
                    {@const syntaxTokens = tokenizeSyntax(part.text)}
                    {#each syntaxTokens as syntaxToken, syntaxTokenIndex (`${syntaxToken.kind}-${syntaxToken.text}-${syntaxTokenIndex}`)}
                        {#if syntaxToken.kind === "text"}
                            {syntaxToken.text}
                        {:else}
                            <span class={syntaxTokenClass(syntaxToken.kind)}>{syntaxToken.text}</span>
                        {/if}
                    {/each}
                {/if}
            {/each}

            {#if entry.metadata}
                <span class="log-metadata">
                    {#each tokenizeSyntax(entry.metadata) as metadataToken, metadataTokenIndex (`${metadataToken.kind}-${metadataToken.text}-${metadataTokenIndex}`)}
                        {#if metadataToken.kind === "text"}
                            {metadataToken.text}
                        {:else}
                            <span class={syntaxTokenClass(metadataToken.kind)}>{metadataToken.text}</span>
                        {/if}
                    {/each}
                </span>
            {/if}
        </span>
    </div>
{/if}
