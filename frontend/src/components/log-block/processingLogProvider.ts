import type { LogEntry, LogGroupEntry, LogLineEntry, LogProvider } from "./types";
import { defaultLinkify, parseGroupMarkers } from "./shared";

const BRACKET_TIMESTAMP_PATTERN =
    /^\[(?<timestamp>\d{4}-\d{2}-\d{2}T[^\]]+)\]\s*(?<message>.*)$/;
const STEP_TAG_PATTERN = /^\[step=(?<step>[a-z0-9_-]+)\]\s*(?<message>.*)$/i;
const INLINE_LEVEL_PATTERN =
    /^(?:(?<timestamp>\d{4}-\d{2}-\d{2}[T ][^\s]+)\s+)?(?<level>TRACE|DEBUG|INFO|WARN|WARNING|ERROR|CRITICAL)\s+(?<message>.*)$/i;

const STEP_ORDER = [
    "upload",
    "queue",
    "input",
    "musicxml",
    "midi",
    "preview_mp3",
    "stems",
    "compress_stems",
    "upload_assets",
    "done",
    "other",
];

const STEP_LABELS: Record<string, string> = {
    upload: "Upload",
    queue: "Queue",
    input: "Input",
    musicxml: "MusicXML",
    midi: "MIDI",
    preview_mp3: "Audio",
    stems: "Stems",
    compress_stems: "Compress",
    upload_assets: "Asset Upload",
    done: "Done",
    other: "Other",
};

function normalizeTimestampPrecision(timestamp: string): string {
    const match = timestamp.match(
        /^(?<base>\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2})(?<fractional>\.\d+)?(?<timezone>Z|[+-]\d{2}:\d{2})$/,
    );

    if (!match?.groups) {
        return timestamp;
    }

    const fractionalDigits = (match.groups.fractional ?? ".000000000")
        .slice(1)
        .padEnd(9, "0")
        .slice(0, 9);

    return `${match.groups.base}.${fractionalDigits}${match.groups.timezone}`;
}

function extractLevelAndMessage(message: string): {
    timestamp?: string;
    level?: string;
    message: string;
} {
    const match = message.match(INLINE_LEVEL_PATTERN);

    if (!match?.groups) {
        return { message };
    }

    return {
        timestamp: match.groups.timestamp
            ? normalizeTimestampPrecision(match.groups.timestamp.replace(" ", "T"))
            : undefined,
        level: match.groups.level.toUpperCase(),
        message: match.groups.message,
    };
}

function inferLevel(message: string): string | undefined {
    const normalized = message.trim().toLowerCase();

    if (
        normalized.startsWith("warning") ||
        normalized.includes(" failed") ||
        normalized.startsWith("failed") ||
        normalized.startsWith("processing failed") ||
        normalized.includes(" unable to ") ||
        normalized.startsWith("error")
    ) {
        if (normalized.startsWith("warning")) {
            return "WARNING";
        }
        return "ERROR";
    }

    return undefined;
}

function extractStepAndMessage(message: string): { step?: string; message: string } {
    const match = message.match(STEP_TAG_PATTERN);
    if (!match?.groups) {
        return { message };
    }

    return {
        step: match.groups.step.toLowerCase(),
        message: match.groups.message,
    };
}

function inferStep(message: string): string | undefined {
    const normalized = message.trim().toLowerCase();

    if (normalized.includes("score.musicxml") || normalized.includes("application/xml")) {
        return "musicxml";
    }

    if (normalized.includes("preview.mid") || normalized.includes("audio/midi")) {
        return "midi";
    }

    if (
        normalized.includes("preview.mp3") ||
        normalized.includes("audio/mpeg") ||
        normalized.startsWith("audio: ")
    ) {
        return "preview_mp3";
    }

    if (normalized.startsWith("stems: compressing [") || normalized.startsWith("stems: compressed [")) {
        return "compress_stems";
    }

    if (
        normalized.startsWith("upload: ") ||
        normalized.includes(" uploading ") ||
        normalized.includes(" uploaded ") ||
        normalized.includes("upload to s3") ||
        normalized.includes("upload to storage")
    ) {
        return "upload_assets";
    }

    if (normalized.startsWith("stems: ") || normalized.includes("musescore-direct-stems")) {
        return "stems";
    }

    if (
        normalized.includes("fetching source score") ||
        normalized.includes("temporary input file") ||
        normalized.includes("writing temporary input")
    ) {
        return "input";
    }

    if (normalized.includes("processor worker") || normalized.includes("queued")) {
        return "queue";
    }

    if (normalized.includes("processing completed") || normalized.includes("status:")) {
        return "done";
    }

    return undefined;
}

function groupProcessingEntries(entries: LogEntry[]): LogEntry[] {
    const grouped = new Map<string, LogLineEntry[]>();
    const passthroughGroups: LogGroupEntry[] = [];

    for (const entry of entries) {
        if (entry.kind === "group") {
            passthroughGroups.push(entry);
            continue;
        }

        const group = entry.group ?? inferStep(entry.message) ?? "other";
        const bucket = grouped.get(group) ?? [];
        bucket.push({ ...entry, group });
        grouped.set(group, bucket);
    }

    const result: LogEntry[] = [];
    for (const step of STEP_ORDER) {
        const bucket = grouped.get(step);
        if (!bucket?.length) {
            continue;
        }

        result.push({
            kind: "group",
            id: `processing-step-${step}`,
            raw: "",
            sourceLine: bucket[0].sourceLine,
            title: STEP_LABELS[step] ?? step,
            entries: bucket,
        });
    }

    for (const [step, bucket] of grouped) {
        if (STEP_ORDER.includes(step)) {
            continue;
        }

        result.push({
            kind: "group",
            id: `processing-step-${step}`,
            raw: "",
            sourceLine: bucket[0].sourceLine,
            title: STEP_LABELS[step] ?? step,
            entries: bucket,
        });
    }

    return [...result, ...passthroughGroups];
}

const processingLogProvider: LogProvider = {
    parseLine: (line: string) => {
        const groupedLine = parseGroupMarkers(line);
        if (groupedLine) {
            return groupedLine;
        }

        const match = line.match(BRACKET_TIMESTAMP_PATTERN);
        if (match?.groups) {
            const parsedStep = extractStepAndMessage(match.groups.message || "");
            const parsedMessage = extractLevelAndMessage(parsedStep.message);

            return {
                type: "line",
                line: {
                    raw: line,
                    timestamp: normalizeTimestampPrecision(match.groups.timestamp),
                    level: parsedMessage.level ?? inferLevel(parsedMessage.message) ?? "INFO",
                    message: parsedMessage.message,
                    group: parsedStep.step ?? inferStep(parsedMessage.message),
                },
            };
        }

        const parsedStep = extractStepAndMessage(line);
        const parsedLine = extractLevelAndMessage(parsedStep.message);

        return {
            type: "line",
            line: {
                raw: line,
                timestamp: parsedLine.timestamp,
                level:
                    parsedLine.level ??
                    (parsedLine.timestamp
                        ? inferLevel(parsedLine.message) ?? "INFO"
                        : inferLevel(parsedLine.message)),
                message: parsedLine.message,
                group: parsedStep.step ?? inferStep(parsedLine.message),
            },
        };
    },
    groupEntries: groupProcessingEntries,
    normalizeLevel: (level: string) => {
        const normalized = level.toUpperCase();
        if (normalized === "WARN") {
            return "WARNING";
        }
        return normalized;
    },
    levelOrder: ["CRITICAL", "ERROR", "WARNING", "INFO", "DEBUG", "TRACE"],
    linkify: (text: string) => defaultLinkify(text),
};

export default processingLogProvider;
