const SCORE_SHARE_CARD_VERSION = "6";

type ScoreShareMetadata = {
    title: string;
    subtitle?: string | null;
    icon?: string | null;
    icon_image_url?: string | null;
};

export function scoreShareImageVersion(music: ScoreShareMetadata) {
    const source = [
        SCORE_SHARE_CARD_VERSION,
        music.title,
        music.subtitle ?? "",
        music.icon ?? "",
        music.icon_image_url ?? "",
    ].join("\n");

    let hash = 5381;
    for (let index = 0; index < source.length; index += 1) {
        hash = (hash * 33) ^ source.charCodeAt(index);
    }

    return `${SCORE_SHARE_CARD_VERSION}-${(hash >>> 0).toString(36)}`;
}
