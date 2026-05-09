import fumenLogoSvg from "../../public/favicon.svg?raw";
import { existsSync } from "node:fs";
import { join } from "node:path";

const A4_SPEED_FONT_PATH = "fonts/a4-speed-bold.otf";
const FIRA_CODE_MEDIUM_FONT_PATH = "fonts/fira-code-medium.ttf";
const FIRA_CODE_BOLD_FONT_PATH = "fonts/fira-code-bold.ttf";

let fontFiles: string[] | null = null;

export function loadScoreCardFontFiles() {
    fontFiles ??= resolveFontFiles();
    return fontFiles;
}

function resolveFontFiles() {
    const fira500Path = resolvePublicAssetPath(FIRA_CODE_MEDIUM_FONT_PATH);
    const fira700Path = resolvePublicAssetPath(FIRA_CODE_BOLD_FONT_PATH);
    const a4SpeedPath = resolvePublicAssetPath(A4_SPEED_FONT_PATH);

    if (!fira500Path || !fira700Path) {
        console.warn("Unable to resolve score card Fira Code font assets");
    }

    if (!a4SpeedPath) {
        console.warn("Unable to resolve score card A4 Speed font asset");
    }

    return [
        ...(fira500Path ? [fira500Path] : []),
        ...(fira700Path ? [fira700Path] : []),
        ...(a4SpeedPath ? [a4SpeedPath] : []),
    ];
}

function resolvePublicAssetPath(relativePath: string) {
    const candidates = [
        join(process.cwd(), "build", "client", relativePath),
        join(process.cwd(), "public", relativePath),
    ];

    for (const candidate of candidates) {
        if (existsSync(candidate)) {
            return candidate;
        }
    }

    return null;
}

export function renderScoreCardSvg({
    title,
    subtitle,
    icon,
    scoreIconDataUri,
    ariaLabel,
}: {
    title: string;
    subtitle: string;
    icon: string;
    scoreIconDataUri: string | null;
    ariaLabel: string;
}) {
    const titleLines = wrapText(title, 23, 3);
    const subtitleLines = subtitle.trim() ? wrapText(subtitle, 38, 2) : [];
    const titleFontSize = titleLines.length >= 3 ? 48 : titleLines.length === 2 ? 56 : 66;
    const titleLineHeight = titleLines.length >= 3 ? 56 : 66;
    const titleCenterY = 284;
    const subtitleY = titleLines.length >= 3 ? 412 : 404;
    const subtitleFontSize = subtitleLines.length >= 2 ? 29 : 34;
    const subtitleLineHeight = subtitleLines.length >= 2 ? 38 : 46;
    const scoreIcon = scoreBadge(title, icon);
    const fumenLogoDataUri = svgDataUri(fumenLogoSvg);
    const iconMarkup = scoreIconDataUri
        ? `<image href="${htmlEscape(scoreIconDataUri)}" x="24" y="24" width="198" height="198" preserveAspectRatio="xMidYMid slice" clip-path="url(#scoreIconClip)"/>`
        : `<rect x="30" y="30" width="186" height="186" rx="30" fill="#10231f" opacity="0.94"/>
    <text x="123" y="129" text-anchor="middle" dominant-baseline="middle" fill="#f2c14e" font-family="'Fira Code', monospace" font-size="76" font-weight="700">${htmlEscape(scoreIcon)}</text>`;

    return `<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="630" viewBox="0 0 1200 630" role="img" aria-label="${htmlEscape(ariaLabel)}">
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#101820"/>
      <stop offset="0.48" stop-color="#17352f"/>
      <stop offset="1" stop-color="#f2c14e"/>
    </linearGradient>
    <radialGradient id="glow" cx="23%" cy="18%" r="72%">
      <stop offset="0" stop-color="#e9fff4" stop-opacity="0.34"/>
      <stop offset="0.5" stop-color="#2dbb83" stop-opacity="0.2"/>
      <stop offset="1" stop-color="#101820" stop-opacity="0"/>
    </radialGradient>
    <filter id="shadow" x="-10%" y="-10%" width="120%" height="130%">
      <feDropShadow dx="0" dy="28" stdDeviation="26" flood-color="#06100d" flood-opacity="0.38"/>
    </filter>
    <clipPath id="scoreIconClip"><rect x="24" y="24" width="198" height="198" rx="32"/></clipPath>
  </defs>
  <rect width="1200" height="630" fill="url(#bg)"/>
  <rect width="1200" height="630" fill="url(#glow)"/>
  <path d="M0 476 C184 416 290 540 460 484 C650 420 736 346 920 376 C1058 398 1124 340 1200 300 L1200 630 L0 630 Z" fill="#0b1413" opacity="0.32"/>
  <g opacity="0.24" stroke="#fff5d1" stroke-width="4">
    <path d="M142 202 H1064"/>
    <path d="M142 243 H1064"/>
    <path d="M142 284 H1064"/>
    <path d="M142 325 H1064"/>
    <path d="M142 366 H1064"/>
  </g>
  <g transform="translate(142 64)">
    <image href="${htmlEscape(fumenLogoDataUri)}" x="10" y="0" width="66" height="68"/>
    <text x="92" y="52" textLength="144" lengthAdjust="spacingAndGlyphs" fill="#fff9e7" font-family="'A4 SPEED', sans-serif" font-size="56" font-weight="500">Fumen</text>
  </g>
  <g transform="translate(142 154)" filter="url(#shadow)">
    <rect width="246" height="246" rx="44" fill="#f9f3df"/>
    ${iconMarkup}
  </g>
  <text fill="#fffdf2" font-family="'Fira Code', monospace" font-size="${titleFontSize}" font-weight="700" dominant-baseline="middle">${svgCenteredTspans(titleLines, 450, titleCenterY, titleLineHeight)}</text>
  <text fill="#d7f5e6" font-family="'Fira Code', monospace" font-size="${subtitleFontSize}" font-weight="500">${svgTspans(subtitleLines, 452, subtitleY, subtitleLineHeight)}</text>
</svg>`;
}

export async function fetchBinaryDataUri(fetchFn: typeof fetch, url: string) {
    try {
        const response = await fetchFn(url);
        if (!response.ok) {
            return null;
        }

        const contentType = response.headers.get("content-type") ?? "image/png";
        const bytes = Buffer.from(await response.arrayBuffer());
        return `data:${contentType};base64,${bytes.toString("base64")}`;
    } catch (error) {
        console.warn("Unable to load score card image", error);
        return null;
    }
}

function svgDataUri(svg: string) {
    return `data:image/svg+xml;base64,${Buffer.from(svg).toString("base64")}`;
}

function scoreBadge(title: string, icon: string) {
    const trimmedIcon = icon.trim();
    if (trimmedIcon) {
        return Array.from(trimmedIcon).slice(0, 2).join("");
    }

    const initials = title
        .trim()
        .split(/\s+/)
        .map((word) => Array.from(word)[0])
        .filter(Boolean)
        .slice(0, 2)
        .join("")
        .toUpperCase();

    return initials || "F";
}

function wrapText(value: string, maxChars: number, maxLines: number) {
    const lines: string[] = [];
    let current = "";

    const words = value
        .trim()
        .split(/\s+/)
        .flatMap((word) => splitLongWord(word, maxChars));

    for (const word of words) {
        const separator = current ? 1 : 0;
        if (current && current.length + separator + word.length > maxChars) {
            lines.push(current);
            current = "";
            if (lines.length === maxLines) {
                break;
            }
        }

        current = current ? `${current} ${word}` : word;
    }

    if (lines.length < maxLines && current) {
        lines.push(current);
    }

    if (lines.length === maxLines && value.length > lines.join(" ").length) {
        lines[lines.length - 1] =
            `${lines[lines.length - 1].slice(0, Math.max(0, maxChars - 3))}...`;
    }

    return lines.length > 0 ? lines : ["Untitled score"];
}

function splitLongWord(word: string, maxChars: number) {
    if (word.length <= maxChars) {
        return [word];
    }

    const chunks: string[] = [];
    let rest = word;
    while (rest.length > maxChars) {
        chunks.push(rest.slice(0, maxChars - 1));
        rest = rest.slice(maxChars - 1);
    }

    if (rest) {
        chunks.push(rest);
    }

    return chunks;
}

function svgCenteredTspans(
    lines: string[],
    x: number,
    centerY: number,
    lineHeight: number,
) {
    const firstY = centerY - ((lines.length - 1) * lineHeight) / 2;
    return svgTspans(lines, x, firstY, lineHeight);
}

function svgTspans(lines: string[], x: number, y: number, lineHeight: number) {
    return lines
        .map(
            (line, index) =>
                `<tspan x="${x}" y="${y + index * lineHeight}">${htmlEscape(line)}</tspan>`,
        )
        .join("");
}

function htmlEscape(value: string) {
    return value
        .replaceAll("&", "&amp;")
        .replaceAll('"', "&quot;")
        .replaceAll("<", "&lt;")
        .replaceAll(">", "&gt;");
}
