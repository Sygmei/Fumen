import type { RequestHandler } from "./$types";
import { Resvg, type ResvgRenderOptions } from "@resvg/resvg-js";
import {
    fetchPublicMusic,
    resolveServerApiBaseUrl,
    scoreShareTitle,
} from "$lib/server-public-api";
import {
    fetchBinaryDataUri,
    loadScoreCardFontFiles,
    renderScoreCardSvg,
} from "$lib/server-score-card";

export const prerender = false;

export const GET: RequestHandler = async ({ fetch, params, url }) => {
    const music = await fetchPublicMusic(fetch, params.accessKey, url.origin);

    if (!music) {
        return new Response("Score not found", { status: 404 });
    }

    const iconUrl = music.icon_image_url
        ? `${resolveServerApiBaseUrl(url.origin)}/public/${encodeURIComponent(params.accessKey)}/icon`
        : null;
    const scoreIconDataUri = iconUrl
        ? await fetchBinaryDataUri(fetch, iconUrl)
        : null;
    const svg = renderScoreCardSvg({
        title: music.title,
        subtitle: music.subtitle ?? "",
        icon: music.icon ?? "",
        scoreIconDataUri,
        ariaLabel: scoreShareTitle(music),
    });

    const resvgOptions: ResvgRenderOptions & {
        font: NonNullable<ResvgRenderOptions["font"]> & {
            fontFiles: string[];
        };
    } = {
        fitTo: { mode: "width", value: 1200 },
        font: {
            fontFiles: loadScoreCardFontFiles(),
            loadSystemFonts: false,
        },
    };
    const resvg = new Resvg(svg, resvgOptions);
    const png = resvg.render().asPng();

    return new Response(Uint8Array.from(png), {
        headers: {
            "content-type": "image/png",
            "cache-control": "public, max-age=300",
            "x-content-type-options": "nosniff",
        },
    });
};
