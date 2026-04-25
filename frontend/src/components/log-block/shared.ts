import type { LogProviderParseResult, LogTextPart } from "./types";

const TRAILING_PUNCTUATION = /[.,!?;:)]$/;

function splitUrlToken(url: string): { cleaned: string; trailing: string } {
    let cleaned = url;
    let trailing = "";

    while (cleaned.length > 0 && TRAILING_PUNCTUATION.test(cleaned)) {
        trailing = cleaned.slice(-1) + trailing;
        cleaned = cleaned.slice(0, -1);
    }

    return { cleaned, trailing };
}

export function defaultLinkify(text: string): LogTextPart[] {
    const parts: LogTextPart[] = [];
    const urlRegex = /\bhttps?:\/\/[^\s<>"']+/gi;

    let lastIndex = 0;
    let match: RegExpExecArray | null;

    while ((match = urlRegex.exec(text)) !== null) {
        const matchedUrl = match[0];
        const start = match.index;

        if (start > lastIndex) {
            parts.push({
                kind: "text",
                text: text.slice(lastIndex, start),
            });
        }

        const { cleaned, trailing } = splitUrlToken(matchedUrl);

        if (cleaned.length > 0) {
            parts.push({
                kind: "link",
                text: cleaned,
                href: cleaned,
            });
        }

        if (trailing.length > 0) {
            parts.push({
                kind: "text",
                text: trailing,
            });
        }

        lastIndex = start + matchedUrl.length;
    }

    if (lastIndex < text.length) {
        parts.push({
            kind: "text",
            text: text.slice(lastIndex),
        });
    }

    if (parts.length === 0) {
        return [{ kind: "text", text }];
    }

    return parts;
}

export function parseGroupMarkers(line: string): LogProviderParseResult | null {
    const groupStartMatch = line.match(/^::group::\s*(.*)$/);
    if (groupStartMatch) {
        return {
            type: "groupStart",
            title: groupStartMatch[1]?.trim() || "Group",
            raw: line,
        };
    }

    if (/^::endgroup::\s*$/.test(line)) {
        return {
            type: "groupEnd",
            raw: line,
        };
    }

    return null;
}
