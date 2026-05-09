import type { PublicMusicResponse } from "$backend/models";
import process from "node:process";

export function resolveServerApiBaseUrl(origin = "http://localhost:5173") {
    const configuredValue =
        process.env.API_BASE_URL?.trim() || process.env.VITE_API_BASE_URL?.trim();

    if (configuredValue) {
        return new URL(configuredValue.replace(/\/+$/, ""), origin)
            .toString()
            .replace(/\/+$/, "");
    }

    if (process.env.NODE_ENV !== "production") {
        return "http://127.0.0.1:3000/api";
    }

    return new URL("/api", origin).toString().replace(/\/+$/, "");
}

export async function fetchPublicMusic(
    fetchFn: typeof fetch,
    accessKey: string,
    origin?: string,
): Promise<PublicMusicResponse | null> {
    const apiBaseUrl = resolveServerApiBaseUrl(origin);
    const response = await fetchFn(
        `${apiBaseUrl}/public/${encodeURIComponent(accessKey)}`,
    );

    if (response.status === 404) {
        return null;
    }

    if (!response.ok) {
        throw new Error(`Unable to load public score: ${response.status}`);
    }

    return (await response.json()) as PublicMusicResponse;
}

export function scoreShareTitle(music: PublicMusicResponse) {
    const subtitle = music.subtitle?.trim();
    return subtitle ? `${music.title} - ${subtitle}` : music.title;
}

export function scoreShareDescription(music: PublicMusicResponse) {
    if (music.can_stream_audio && music.musicxml_url) {
        return "Open the interactive score and listen with Fumen.";
    }

    if (music.can_stream_audio) {
        return "Listen to this score on Fumen.";
    }

    return "Open this score on Fumen.";
}
