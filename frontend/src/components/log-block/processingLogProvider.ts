import type { LogProvider } from "./types";
import { defaultLinkify, parseGroupMarkers } from "./shared";

const BRACKET_TIMESTAMP_PATTERN =
    /^\[(?<timestamp>\d{4}-\d{2}-\d{2}T[^\]]+)\]\s*(?<message>.*)$/;
const INLINE_LEVEL_PATTERN =
    /^(?:(?<timestamp>\d{4}-\d{2}-\d{2}[T ][^\s]+)\s+)?(?<level>TRACE|DEBUG|INFO|WARN|WARNING|ERROR|CRITICAL)\s+(?<message>.*)$/i;

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

const processingLogProvider: LogProvider = {
    parseLine: (line: string) => {
        const groupedLine = parseGroupMarkers(line);
        if (groupedLine) {
            return groupedLine;
        }

        const match = line.match(BRACKET_TIMESTAMP_PATTERN);
        if (match?.groups) {
            const parsedMessage = extractLevelAndMessage(match.groups.message || "");

            return {
                type: "line",
                line: {
                    raw: line,
                    timestamp: normalizeTimestampPrecision(match.groups.timestamp),
                    level: parsedMessage.level ?? inferLevel(parsedMessage.message) ?? "INFO",
                    message: parsedMessage.message,
                },
            };
        }

        const parsedLine = extractLevelAndMessage(line);

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
            },
        };
    },
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
