import type { Handle } from "@sveltejs/kit";
import {
    fetchPublicMusic,
    scoreShareDescription,
    scoreShareTitle,
} from "$lib/server-public-api";
import { scoreShareImageVersion } from "$lib/share-card";

export const handle: Handle = async ({ event, resolve }) => {
    const accessKey = listenAccessKey(event.url.pathname);
    if (!accessKey) {
        return resolve(event);
    }

    let metaTags = "";
    try {
        const music = await fetchPublicMusic(
            event.fetch,
            accessKey,
            event.url.origin,
        );

        if (music) {
            const title = scoreShareTitle(music);
            const description = scoreShareDescription(music);
            const pageUrl = `${event.url.origin}/listen/${encodeURIComponent(accessKey)}`;
            const imageUrl = `${event.url.origin}/share-card/${encodeURIComponent(accessKey)}.png?v=${scoreShareImageVersion(music)}`;
            metaTags = buildListenMetaTags({
                title,
                description,
                pageUrl,
                imageUrl,
            });
        }
    } catch (error) {
        console.warn("Unable to build listen preview metadata", error);
    }

    return resolve(event, {
        transformPageChunk: ({ html }) =>
            metaTags ? html.replace("</head>", `${metaTags}</head>`) : html,
    });
};

function listenAccessKey(pathname: string) {
    const match = /^\/listen\/([^/?#]+)\/?$/.exec(pathname);
    return match ? decodeURIComponent(match[1]) : null;
}

function buildListenMetaTags({
    title,
    description,
    pageUrl,
    imageUrl,
}: {
    title: string;
    description: string;
    pageUrl: string;
    imageUrl: string;
}) {
    const safeTitle = htmlEscape(title);
    const safeDescription = htmlEscape(description);
    const safePageUrl = htmlEscape(pageUrl);
    const safeImageUrl = htmlEscape(imageUrl);

    return `
<title>${safeTitle}</title>
<meta name="description" content="${safeDescription}" />
<link rel="canonical" href="${safePageUrl}" />
<meta property="og:type" content="music.song" />
<meta property="og:site_name" content="Fumen" />
<meta property="og:title" content="${safeTitle}" />
<meta property="og:description" content="${safeDescription}" />
<meta property="og:url" content="${safePageUrl}" />
<meta property="og:image" content="${safeImageUrl}" />
<meta property="og:image:secure_url" content="${safeImageUrl}" />
<meta property="og:image:type" content="image/png" />
<meta property="og:image:width" content="1200" />
<meta property="og:image:height" content="630" />
<meta property="og:image:alt" content="${safeTitle} on Fumen" />
<meta name="twitter:card" content="summary_large_image" />
<meta name="twitter:title" content="${safeTitle}" />
<meta name="twitter:description" content="${safeDescription}" />
<meta name="twitter:image" content="${safeImageUrl}" />`;
}

function htmlEscape(value: string) {
    return value
        .replaceAll("&", "&amp;")
        .replaceAll('"', "&quot;")
        .replaceAll("<", "&lt;")
        .replaceAll(">", "&gt;");
}
