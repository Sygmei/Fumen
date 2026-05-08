use crate::schemas::{
    CreateScoreAnnotationRequest, ErrorResponse, PublicMusicResponse, ReportPlaytimeRequest,
    ScoreAnnotationListResponse, ScoreAnnotationResponse, StemInfo,
};
use crate::services::{auth, music};
use crate::{AppError, AppState, sanitize_content_disposition};
use anyhow::anyhow;
use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::Response,
};
use bytes::Bytes;
use flate2::{Compression, write::ZlibEncoder};
use fumen_core::models::MusicRecord;
use image::{GenericImageView, imageops::FilterType};
use std::path::PathBuf;
use std::{env, io::Write};
use tokio::io::{AsyncReadExt, AsyncSeekExt};

pub(super) fn routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route(
            "/public/{access_key}",
            crate::op_get!(state, "/public/{access_key}", public_music),
        )
        .route(
            "/public/{access_key}/audio",
            crate::op_get!(state, "/public/{access_key}/audio", public_music_audio),
        )
        .route(
            "/public/{access_key}/midi",
            crate::op_get!(state, "/public/{access_key}/midi", public_music_midi),
        )
        .route(
            "/public/{access_key}/musicxml",
            crate::op_get!(
                state,
                "/public/{access_key}/musicxml",
                public_music_musicxml
            ),
        )
        .route(
            "/public/{access_key}/stems",
            crate::op_get!(state, "/public/{access_key}/stems", public_music_stems),
        )
        .route(
            "/public/{access_key}/playtime",
            crate::op_post!(
                state,
                "/public/{access_key}/playtime",
                report_public_music_playtime
            ),
        )
        .route(
            "/public/{access_key}/stems/{track_index}",
            crate::op_get!(
                state,
                "/public/{access_key}/stems/{track_index}",
                public_music_stem_audio
            ),
        )
        .route(
            "/public/{access_key}/download",
            crate::op_get!(
                state,
                "/public/{access_key}/download",
                public_music_download
            ),
        )
        .route(
            "/public/{access_key}/icon",
            crate::op_get!(state, "/public/{access_key}/icon", public_music_icon),
        )
        .route(
            "/public/{access_key}/share-card.svg",
            crate::op_get!(
                state,
                "/public/{access_key}/share-card.svg",
                public_music_share_card
            ),
        )
        .route(
            "/public/{access_key}/share-card.png",
            crate::op_get!(
                state,
                "/public/{access_key}/share-card.png",
                public_music_share_card_png
            ),
        )
        .route(
            "/public/{access_key}/annotations",
            crate::op_get!(
                state,
                "/public/{access_key}/annotations",
                public_music_annotations
            ),
        )
        .route(
            "/public/{access_key}/annotations",
            crate::op_post!(
                state,
                "/public/{access_key}/annotations",
                create_public_music_annotation
            ),
        )
}

pub(crate) fn listen_routes(state: AppState) -> Router {
    Router::new()
        .route(
            "/listen/{access_key}",
            crate::op_get!(state, "/listen/{access_key}", public_listen_page),
        )
        .with_state(state)
}

#[utoipa::path(
    get,
    path = "/api/public/{access_key}",
    tag = "public",
    params(
        ("access_key" = String, Path, description = "Public score token or public id")
    ),
    responses(
        (status = 200, description = "Public score metadata", body = PublicMusicResponse),
        (status = 404, description = "Music not found", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse)
    )
)]
pub(crate) async fn public_music(
    State(state): State<AppState>,
    Path(access_key): Path<String>,
) -> Result<Json<PublicMusicResponse>, AppError> {
    let record = music::find_public_music_record(&state, &access_key)
        .await?
        .ok_or_else(|| AppError::not_found("Music not found"))?;

    Ok(Json(music::record_to_public_response(
        &state.storage,
        record,
        &access_key,
    )))
}

#[utoipa::path(
    get,
    path = "/api/public/{access_key}/audio",
    tag = "public",
    params(
        ("access_key" = String, Path, description = "Public score token or public id")
    ),
    responses(
        (status = 200, description = "Audio preview stream", content_type = "audio/mpeg"),
        (status = 404, description = "Audio preview or score not found", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse)
    )
)]
pub(crate) async fn public_music_audio(
    State(state): State<AppState>,
    Path(access_key): Path<String>,
) -> Result<Response, AppError> {
    let record = music::find_public_music_record(&state, &access_key)
        .await?
        .ok_or_else(|| AppError::not_found("Music not found"))?;

    let audio_key = record
        .audio_object_key
        .ok_or_else(|| AppError::not_found("Audio preview is not available for this score"))?;

    let (bytes, content_type, content_encoding) = state.storage.get_bytes(&audio_key).await?;
    Ok(binary_response(
        bytes,
        content_type.unwrap_or_else(|| "audio/mpeg".to_owned()),
        content_encoding,
        Some("inline; filename=\"preview.mp3\"".to_owned()),
    ))
}

#[utoipa::path(
    get,
    path = "/api/public/{access_key}/midi",
    tag = "public",
    params(
        ("access_key" = String, Path, description = "Public score token or public id")
    ),
    responses(
        (status = 200, description = "MIDI file", content_type = "audio/midi"),
        (status = 404, description = "MIDI export or score not found", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse)
    )
)]
pub(crate) async fn public_music_midi(
    State(state): State<AppState>,
    Path(access_key): Path<String>,
) -> Result<Response, AppError> {
    let record = music::find_public_music_record(&state, &access_key)
        .await?
        .ok_or_else(|| AppError::not_found("Music not found"))?;

    let midi_key = record
        .midi_object_key
        .ok_or_else(|| AppError::not_found("MIDI export is not available for this score"))?;

    let (bytes, content_type, content_encoding) = state.storage.get_bytes(&midi_key).await?;
    Ok(binary_response(
        bytes,
        content_type.unwrap_or_else(|| "audio/midi".to_owned()),
        content_encoding,
        Some(format!(
            "attachment; filename=\"{}\"",
            music::midi_filename_for(&record.filename)
        )),
    ))
}

#[utoipa::path(
    get,
    path = "/api/public/{access_key}/musicxml",
    tag = "public",
    params(
        ("access_key" = String, Path, description = "Public score token or public id")
    ),
    responses(
        (status = 200, description = "MusicXML file", content_type = "application/xml"),
        (status = 404, description = "MusicXML export or score not found", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse)
    )
)]
pub(crate) async fn public_music_musicxml(
    State(state): State<AppState>,
    Path(access_key): Path<String>,
) -> Result<Response, AppError> {
    let record = music::find_public_music_record(&state, &access_key)
        .await?
        .ok_or_else(|| AppError::not_found("Music not found"))?;

    let musicxml_key = record
        .musicxml_object_key
        .ok_or_else(|| AppError::not_found("MusicXML export is not available for this score"))?;

    let (bytes, content_type, content_encoding) = state.storage.get_bytes(&musicxml_key).await?;
    Ok(binary_response(
        bytes,
        content_type.unwrap_or_else(|| "application/xml".to_owned()),
        content_encoding,
        Some(format!(
            "inline; filename=\"{}.musicxml\"",
            sanitize_content_disposition(record.filename.trim_end_matches(".mscz"))
        )),
    ))
}

#[utoipa::path(
    get,
    path = "/api/public/{access_key}/download",
    tag = "public",
    params(
        ("access_key" = String, Path, description = "Public score token or public id")
    ),
    responses(
        (status = 200, description = "Original score file", content_type = "application/octet-stream"),
        (status = 404, description = "Score not found", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse)
    )
)]
pub(crate) async fn public_music_download(
    State(state): State<AppState>,
    Path(access_key): Path<String>,
) -> Result<Response, AppError> {
    let record = music::find_public_music_record(&state, &access_key)
        .await?
        .ok_or_else(|| AppError::not_found("Music not found"))?;

    let (bytes, content_type, content_encoding) =
        state.storage.get_bytes(&record.object_key).await?;
    Ok(binary_response(
        bytes,
        content_type.unwrap_or(record.content_type),
        content_encoding,
        Some(format!(
            "attachment; filename=\"{}\"",
            sanitize_content_disposition(&record.filename)
        )),
    ))
}

#[utoipa::path(
    get,
    path = "/api/public/{access_key}/icon",
    tag = "public",
    params(
        ("access_key" = String, Path, description = "Public score token or public id")
    ),
    responses(
        (status = 200, description = "Score icon image", content_type = "image/*"),
        (status = 404, description = "Icon or score not found", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse)
    )
)]
pub(crate) async fn public_music_icon(
    State(state): State<AppState>,
    Path(access_key): Path<String>,
) -> Result<Response, AppError> {
    let record = music::find_public_music_record(&state, &access_key)
        .await?
        .ok_or_else(|| AppError::not_found("Music not found"))?;

    let icon_key = record
        .icon_image_key
        .ok_or_else(|| AppError::not_found("No icon for this score"))?;

    let (bytes, content_type, content_encoding) = state.storage.get_bytes(&icon_key).await?;
    Ok(binary_response(
        bytes,
        content_type.unwrap_or_else(|| "image/jpeg".to_owned()),
        content_encoding,
        None,
    ))
}

pub(crate) async fn public_music_share_card(
    State(state): State<AppState>,
    Path(access_key): Path<String>,
) -> Result<Response, AppError> {
    let record = music::find_public_music_record(&state, &access_key)
        .await?
        .ok_or_else(|| AppError::not_found("Music not found"))?;

    let svg = render_share_card_svg(&state, &access_key, &record);
    Ok(text_response(
        svg,
        "image/svg+xml; charset=utf-8",
        Some("public, max-age=300"),
    ))
}

pub(crate) async fn public_music_share_card_png(
    State(state): State<AppState>,
    Path(access_key): Path<String>,
) -> Result<Response, AppError> {
    let record = music::find_public_music_record(&state, &access_key)
        .await?
        .ok_or_else(|| AppError::not_found("Music not found"))?;

    let icon_bytes = match record.icon_image_key.as_deref() {
        Some(icon_key) => state
            .storage
            .get_bytes(icon_key)
            .await
            .ok()
            .map(|(bytes, _, _)| bytes),
        None => None,
    };
    let png = render_share_card_png(&record, icon_bytes.as_deref())?;
    let mut response = binary_response(Bytes::from(png), "image/png".to_owned(), None, None);
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=300"),
    );
    Ok(response)
}

pub(crate) async fn public_listen_page(
    State(state): State<AppState>,
    Path(access_key): Path<String>,
) -> Result<Response, AppError> {
    let record = music::find_public_music_record(&state, &access_key)
        .await?
        .ok_or_else(|| AppError::not_found("Music not found"))?;

    let html = render_listen_page_html(&state, &access_key, &record).await?;
    Ok(text_response(html, "text/html; charset=utf-8", None))
}

#[utoipa::path(
    get,
    path = "/api/public/{access_key}/annotations",
    tag = "public",
    params(
        ("access_key" = String, Path, description = "Public score token or public id")
    ),
    responses(
        (status = 200, description = "Score annotations visible to the current viewer", body = ScoreAnnotationListResponse),
        (status = 404, description = "Music not found", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse)
    )
)]
pub(crate) async fn public_music_annotations(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(access_key): Path<String>,
) -> Result<Json<ScoreAnnotationListResponse>, AppError> {
    let auth_context = auth::try_build_auth_context(&state, &headers).await?;
    Ok(Json(
        music::build_public_score_annotations_response(&state, &access_key, auth_context.as_ref())
            .await?,
    ))
}

#[utoipa::path(
    get,
    path = "/api/public/{access_key}/stems",
    tag = "public",
    params(
        ("access_key" = String, Path, description = "Public score token or public id")
    ),
    responses(
        (status = 200, description = "Available stems", body = [StemInfo]),
        (status = 404, description = "Music not found", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse)
    )
)]
pub(crate) async fn public_music_stems(
    State(state): State<AppState>,
    Path(access_key): Path<String>,
) -> Result<Json<Vec<StemInfo>>, AppError> {
    let record = music::find_public_music_record(&state, &access_key)
        .await?
        .ok_or_else(|| AppError::not_found("Music not found"))?;

    Ok(Json(
        music::build_public_stem_infos(&state, &access_key, &record.id).await?,
    ))
}

#[utoipa::path(
    get,
    path = "/api/public/{access_key}/stems/{track_index}",
    tag = "public",
    params(
        ("access_key" = String, Path, description = "Public score token or public id"),
        ("track_index" = i64, Path, description = "Stem track index")
    ),
    responses(
        (status = 200, description = "Stem audio stream", content_type = "audio/ogg"),
        (status = 206, description = "Partial stem audio stream", content_type = "audio/ogg"),
        (status = 404, description = "Stem or score not found", body = ErrorResponse),
        (status = 416, description = "Invalid range", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse)
    )
)]
pub(crate) async fn public_music_stem_audio(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((access_key, track_index)): Path<(String, i64)>,
) -> Result<Response, AppError> {
    let record = music::find_public_music_record(&state, &access_key)
        .await?
        .ok_or_else(|| AppError::not_found("Music not found"))?;

    let stem = music::find_public_stem(&state.db_ro, &state.db_rw, &record.id, track_index)
        .await?
        .ok_or_else(|| AppError::not_found("Stem not found"))?;

    if let Some(path) = state.storage.local_path_for_key(&stem.storage_key) {
        return local_file_response(
            &path,
            "audio/ogg",
            Some(format!("inline; filename=\"{}.ogg\"", stem.track_name)),
            headers.get(header::RANGE),
        )
        .await;
    }

    let (bytes, content_type, content_encoding) =
        state.storage.get_bytes(&stem.storage_key).await?;
    Ok(binary_response(
        bytes,
        content_type.unwrap_or_else(|| "audio/ogg".to_owned()),
        content_encoding,
        Some(format!("inline; filename=\"{}.ogg\"", stem.track_name)),
    ))
}

#[utoipa::path(
    post,
    path = "/api/public/{access_key}/playtime",
    tag = "public",
    security(("bearer_auth" = [])),
    params(
        ("access_key" = String, Path, description = "Public score token or public id")
    ),
    request_body = ReportPlaytimeRequest,
    responses(
        (status = 204, description = "Playtime recorded"),
        (status = 400, description = "Invalid playtime payload", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "Stem or score not found", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse)
    )
)]
pub(crate) async fn report_public_music_playtime(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(access_key): Path<String>,
    Json(payload): Json<ReportPlaytimeRequest>,
) -> Result<StatusCode, AppError> {
    let auth_context = auth::build_auth_context(&state, &headers).await?;
    let record = music::find_public_music_record(&state, &access_key)
        .await?
        .ok_or_else(|| AppError::not_found("Music not found"))?;

    if payload.tracks.is_empty() {
        return Err(AppError::bad_request(
            "No playtime increments were provided",
        ));
    }

    let stems = music::find_public_stems(&state.db_ro, &state.db_rw, &record.id).await?;
    let valid_track_indices = stems
        .into_iter()
        .map(|stem| stem.track_index)
        .collect::<std::collections::HashSet<_>>();
    let mut normalized = std::collections::HashMap::<i64, f64>::new();

    for track in payload.tracks {
        if !track.seconds.is_finite() || track.seconds <= 0.0 {
            return Err(AppError::bad_request(
                "Playtime increments must be positive numbers",
            ));
        }
        if track.seconds > 300.0 {
            return Err(AppError::bad_request(
                "Playtime increments cannot exceed 300 seconds at once",
            ));
        }
        if !valid_track_indices.contains(&track.track_index) {
            return Err(AppError::bad_request(
                "Unknown track index in playtime report",
            ));
        }

        *normalized.entry(track.track_index).or_insert(0.0) += track.seconds;
    }

    let normalized = normalized.into_iter().collect::<Vec<_>>();
    let mut track_indices = normalized
        .iter()
        .map(|(track_index, _)| *track_index)
        .collect::<Vec<_>>();
    track_indices.sort_unstable();
    music::add_user_track_playtime(&state.db_rw, &auth_context.user.id, &record.id, &normalized)
        .await
        .map_err(|error| {
            AppError::from(anyhow!(
                "failed to record playtime for music {} and user {} on tracks {:?}: {}",
                record.id,
                auth_context.user.id,
                track_indices,
                error.message,
            ))
        })?;

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/api/public/{access_key}/annotations",
    tag = "public",
    security(("bearer_auth" = [])),
    params(
        ("access_key" = String, Path, description = "Public score token or public id")
    ),
    request_body = CreateScoreAnnotationRequest,
    responses(
        (status = 200, description = "Created annotation", body = ScoreAnnotationResponse),
        (status = 400, description = "Invalid annotation payload", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "Music not found", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse)
    )
)]
pub(crate) async fn create_public_music_annotation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(access_key): Path<String>,
    Json(payload): Json<CreateScoreAnnotationRequest>,
) -> Result<Json<ScoreAnnotationResponse>, AppError> {
    let auth_context = auth::build_auth_context(&state, &headers).await?;
    Ok(Json(
        music::create_public_score_annotation(&state, &access_key, &auth_context, payload).await?,
    ))
}

async fn render_listen_page_html(
    state: &AppState,
    access_key: &str,
    record: &MusicRecord,
) -> Result<String, AppError> {
    let meta_tags = build_listen_meta_tags(state, access_key, record);
    let index_path = frontend_dist_path()
        .map(|path| path.join("index.html"))
        .ok_or_else(|| {
            AppError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Frontend build is missing; cannot render listen page",
            )
        })?;
    let index_html = tokio::fs::read_to_string(&index_path)
        .await
        .map_err(AppError::from)?;

    if let Some(head_end) = index_html.find("</head>") {
        let mut html = String::with_capacity(index_html.len() + meta_tags.len() + 1);
        html.push_str(&index_html[..head_end]);
        html.push_str(&meta_tags);
        html.push_str(&index_html[head_end..]);
        return Ok(html);
    }

    Ok(format!(
        "<!doctype html><html lang=\"en\"><head>{}</head><body><main id=\"app\"></main></body></html>",
        meta_tags
    ))
}

pub(crate) fn frontend_dist_path() -> Option<PathBuf> {
    frontend_dist_candidates()
        .into_iter()
        .find(|path| path.join("index.html").exists())
}

fn frontend_dist_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(path) = env::var("FRONTEND_DIST_DIR")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
    {
        candidates.push(path);
    }

    candidates.push(PathBuf::from("/app/frontend/dist"));
    candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../frontend/dist"));
    candidates
}

fn build_listen_meta_tags(state: &AppState, access_key: &str, record: &MusicRecord) -> String {
    let title = score_share_title(record);
    let description = score_share_description(record);
    let page_url = state.config.public_url_for(access_key);
    let escaped_access_key = percent_encode_path_segment(access_key);
    let image_url = format!(
        "{}/api/public/{}/share-card.png",
        state.config.app_base_url.trim_end_matches('/'),
        escaped_access_key
    );

    format!(
        "\n<title>{title}</title>\
         \n<meta name=\"description\" content=\"{description}\" />\
         \n<link rel=\"canonical\" href=\"{page_url}\" />\
         \n<meta property=\"og:type\" content=\"music.song\" />\
         \n<meta property=\"og:site_name\" content=\"Fumen\" />\
         \n<meta property=\"og:title\" content=\"{title}\" />\
         \n<meta property=\"og:description\" content=\"{description}\" />\
         \n<meta property=\"og:url\" content=\"{page_url}\" />\
         \n<meta property=\"og:image\" content=\"{image_url}\" />\
         \n<meta property=\"og:image:secure_url\" content=\"{image_url}\" />\
         \n<meta property=\"og:image:type\" content=\"image/png\" />\
         \n<meta property=\"og:image:width\" content=\"1200\" />\
         \n<meta property=\"og:image:height\" content=\"630\" />\
         \n<meta property=\"og:image:alt\" content=\"{title} on Fumen\" />\
         \n<meta name=\"twitter:card\" content=\"summary_large_image\" />\
         \n<meta name=\"twitter:title\" content=\"{title}\" />\
         \n<meta name=\"twitter:description\" content=\"{description}\" />\
         \n<meta name=\"twitter:image\" content=\"{image_url}\" />\n",
        title = html_escape(&title),
        description = html_escape(&description),
        page_url = html_escape(&page_url),
        image_url = html_escape(&image_url),
    )
}

fn score_share_title(record: &MusicRecord) -> String {
    match record
        .subtitle
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(subtitle) => format!("{} - {}", record.title, subtitle),
        None => record.title.clone(),
    }
}

fn score_share_description(record: &MusicRecord) -> String {
    if record.audio_object_key.is_some() && record.musicxml_object_key.is_some() {
        return "Open the interactive score and listen with Fumen.".to_owned();
    }

    if record.audio_object_key.is_some() {
        return "Listen to this score on Fumen.".to_owned();
    }

    "Open this score on Fumen.".to_owned()
}

fn render_share_card_svg(state: &AppState, access_key: &str, record: &MusicRecord) -> String {
    let title_lines = wrap_svg_text(&record.title, 31, 2);
    let subtitle_lines = record
        .subtitle
        .as_deref()
        .map(|subtitle| wrap_svg_text(subtitle, 38, 1))
        .unwrap_or_default();
    let badge = score_badge(record);
    let title_start_y = if title_lines.len() > 1 { 224 } else { 260 };
    let title_tspans = svg_tspans(&title_lines, 415, title_start_y, 68);
    let subtitle_tspans = svg_tspans(&subtitle_lines, 416, 388, 42);
    let escaped_access_key = percent_encode_path_segment(access_key);
    let icon_markup = if record.icon_image_key.is_some() {
        format!(
            r##"<image href="{}/api/public/{}/icon" x="24" y="24" width="186" height="186" preserveAspectRatio="xMidYMid slice" clip-path="url(#scoreIconClip)"/>"##,
            html_escape(state.config.app_base_url.trim_end_matches('/')),
            html_escape(&escaped_access_key),
        )
    } else {
        format!(
            r##"<rect x="30" y="30" width="174" height="174" rx="28" fill="#10231f" opacity="0.94"/>
    <text x="117" y="128" text-anchor="middle" dominant-baseline="middle" fill="#f2c14e" font-family="Georgia, 'Times New Roman', serif" font-size="72" font-weight="700">{}</text>"##,
            svg_escape(&badge),
        )
    };

    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="630" viewBox="0 0 1200 630" role="img" aria-label="{aria_label}">
  <defs>
    <style>
      @import url('https://fonts.cdnfonts.com/css/a4-speed');
      @import url('https://fonts.googleapis.com/css2?family=Fira+Code:wght@500;700&amp;display=swap');
    </style>
    <linearGradient id="bg" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#101820"/>
      <stop offset="0.52" stop-color="#17352f"/>
      <stop offset="1" stop-color="#f2c14e"/>
    </linearGradient>
    <radialGradient id="glow" cx="24%" cy="20%" r="70%">
      <stop offset="0" stop-color="#e9fff4" stop-opacity="0.34"/>
      <stop offset="0.48" stop-color="#2dbb83" stop-opacity="0.2"/>
      <stop offset="1" stop-color="#101820" stop-opacity="0"/>
    </radialGradient>
    <filter id="shadow" x="-10%" y="-10%" width="120%" height="130%">
      <feDropShadow dx="0" dy="28" stdDeviation="26" flood-color="#06100d" flood-opacity="0.38"/>
    </filter>
    <clipPath id="scoreIconClip"><rect x="24" y="24" width="186" height="186" rx="30"/></clipPath>
  </defs>
  <rect width="1200" height="630" fill="url(#bg)"/>
  <rect width="1200" height="630" fill="url(#glow)"/>
  <path d="M0 476 C184 416 290 540 460 484 C650 420 736 346 920 376 C1058 398 1124 340 1200 300 L1200 630 L0 630 Z" fill="#0b1413" opacity="0.32"/>
  <g opacity="0.24" stroke="#fff5d1" stroke-width="4">
    <path d="M116 164 H1038"/>
    <path d="M116 205 H1038"/>
    <path d="M116 246 H1038"/>
    <path d="M116 287 H1038"/>
    <path d="M116 328 H1038"/>
  </g>
  <g transform="translate(72 52)">
    <image href="{favicon_url}" x="0" y="0" width="52" height="54"/>
    <text x="70" y="39" fill="#fff9e7" font-family="'A4 SPEED', 'A4 Speed', sans-serif" font-size="39" font-weight="500" letter-spacing="0">Fumen</text>
  </g>
  <g transform="translate(112 112)" filter="url(#shadow)">
    <rect width="234" height="234" rx="42" fill="#f9f3df"/>
    {icon_markup}
  </g>
  <text fill="#fffdf2" font-family="'Fira Code', 'Cascadia Code', monospace" font-size="58" font-weight="700">{title_tspans}</text>
  <text fill="#d7f5e6" font-family="'Fira Code', 'Cascadia Code', monospace" font-size="29" font-weight="500">{subtitle_tspans}</text>
</svg>"##,
        aria_label = html_escape(&score_share_title(record)),
        favicon_url = html_escape(&format!(
            "{}/favicon.svg",
            state.config.app_base_url.trim_end_matches('/')
        )),
        icon_markup = icon_markup,
        title_tspans = title_tspans,
        subtitle_tspans = subtitle_tspans,
    )
}

fn render_share_card_png(
    record: &MusicRecord,
    icon_bytes: Option<&[u8]>,
) -> Result<Vec<u8>, AppError> {
    let mut canvas = RgbCanvas::new(1200, 630);
    let badge = score_badge(record);
    canvas.paint_background();
    canvas.paint_staff_lines();
    canvas.paint_brand();
    canvas.paint_score_tile(record, &badge, icon_bytes);
    canvas.paint_score_copy(record);
    encode_png_rgb(canvas.width, canvas.height, &canvas.pixels)
}

struct RgbCanvas {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl RgbCanvas {
    fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; (width * height * 3) as usize],
        }
    }

    fn paint_background(&mut self) {
        let width = self.width as f32;
        let height = self.height as f32;

        for y in 0..self.height {
            for x in 0..self.width {
                let tx = x as f32 / width;
                let ty = y as f32 / height;
                let glow = (1.0
                    - (((tx - 0.24).powi(2) / 0.42) + ((ty - 0.2).powi(2) / 0.28)).sqrt())
                .clamp(0.0, 1.0);
                let sweep = ((tx * 0.7) + (ty * 0.35)).clamp(0.0, 1.0);
                let base = mix_rgb((16, 24, 32), (23, 53, 47), sweep);
                let warm = mix_rgb(base, (242, 193, 78), (tx * ty * 0.55).clamp(0.0, 0.55));
                let lit = mix_rgb(warm, (221, 255, 236), glow * 0.34);
                self.set_pixel(x, y, lit);
            }
        }

        self.fill_wave_band();
    }

    fn paint_staff_lines(&mut self) {
        for y in [164, 205, 246, 287, 328] {
            self.fill_rect(116, y, 922, 4, (255, 245, 209), 0.24);
        }

        for offset in [0, 220, 440, 660] {
            self.fill_rect(176 + offset, 148, 5, 198, (255, 245, 209), 0.12);
        }
    }

    fn paint_brand(&mut self) {
        self.fill_circle(98, 79, 36, (249, 243, 223), 0.16);
        self.fill_circle(96, 80, 24, (242, 193, 78), 0.95);
        self.fill_circle(86, 85, 13, (16, 24, 32), 0.92);
        self.fill_rounded_rect(106, 48, 9, 54, 5, (16, 24, 32), 0.92);
        self.fill_rounded_rect(114, 48, 30, 8, 4, (16, 24, 32), 0.92);
        self.draw_a4_speed_wordmark(156, 54, 5, (255, 249, 231), 0.98);
    }

    fn paint_score_tile(&mut self, record: &MusicRecord, badge: &str, icon_bytes: Option<&[u8]>) {
        self.fill_rounded_rect(112, 112, 234, 234, 42, (249, 243, 223), 1.0);
        self.fill_rounded_rect(136, 136, 186, 186, 30, (16, 35, 31), 0.96);

        if let Some(icon_bytes) = icon_bytes {
            if self.paint_uploaded_icon(icon_bytes, 136, 136, 186, 186, 30) {
                return;
            }
        }

        self.fill_circle(206, 258, 29, (242, 193, 78), 0.95);
        self.fill_rounded_rect(226, 168, 14, 102, 4, (242, 193, 78), 0.95);
        self.fill_rounded_rect(238, 168, 48, 12, 4, (242, 193, 78), 0.95);

        let icon_text = normalize_card_text(
            if record
                .icon
                .as_deref()
                .is_some_and(|icon| !icon.trim().is_empty())
            {
                badge
            } else {
                &record.title
            },
        );
        let icon_text = icon_text.chars().take(2).collect::<String>();
        self.draw_centered_text(&icon_text, 136, 210, 186, 15, (255, 249, 231), 1.0);
    }

    fn paint_uploaded_icon(
        &mut self,
        icon_bytes: &[u8],
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        radius: i32,
    ) -> bool {
        let Ok(image) = image::load_from_memory(icon_bytes) else {
            return false;
        };
        let resized = image.resize_to_fill(width as u32, height as u32, FilterType::Lanczos3);

        for py in 0..height {
            for px in 0..width {
                if !rounded_rect_contains(px, py, width, height, radius) {
                    continue;
                }

                let pixel = resized.get_pixel(px as u32, py as u32);
                let alpha = f32::from(pixel.0[3]) / 255.0;
                self.blend_pixel(x + px, y + py, (pixel.0[0], pixel.0[1], pixel.0[2]), alpha);
            }
        }

        true
    }

    fn paint_score_copy(&mut self, record: &MusicRecord) {
        let title = normalize_card_text(&record.title);
        let title_lines = wrap_card_text(&title, 23, 2);
        let line_height = 62;
        let glyph_height = 7 * 7;
        let total_height = glyph_height + ((title_lines.len() as i32 - 1).max(0) * line_height);
        let mut y = 246 - total_height / 2;
        for line in title_lines {
            self.draw_mono_text(&line, 414, y, 7, (255, 253, 242), 0.98);
            y += line_height;
        }

        if let Some(subtitle) = record
            .subtitle
            .as_deref()
            .map(normalize_card_text)
            .filter(|value| !value.is_empty())
        {
            for line in wrap_card_text(&subtitle, 31, 1) {
                self.draw_mono_text(&line, 416, 400, 4, (215, 245, 230), 0.95);
            }
        }
    }

    fn fill_wave_band(&mut self) {
        for y in 300..self.height as i32 {
            for x in 0..self.width as i32 {
                let wave = 455.0
                    + ((x as f32 / 75.0).sin() * 22.0)
                    + ((x as f32 - 340.0).abs() / 13.0).sin() * 10.0;
                if y as f32 > wave {
                    self.blend_pixel(x, y, (11, 20, 19), 0.32);
                }
            }
        }
    }

    fn draw_centered_text(
        &mut self,
        text: &str,
        x: i32,
        y: i32,
        width: i32,
        scale: i32,
        color: (u8, u8, u8),
        alpha: f32,
    ) {
        let text_width = text_pixel_width(text, scale);
        self.draw_text(
            text,
            x + ((width - text_width) / 2).max(0),
            y,
            scale,
            color,
            alpha,
        );
    }

    fn draw_a4_speed_wordmark(
        &mut self,
        x: i32,
        y: i32,
        scale: i32,
        color: (u8, u8, u8),
        alpha: f32,
    ) {
        for index in 0..3 {
            let slash_x = x + index * 12;
            self.fill_slanted_bar(slash_x, y + 4, scale, 28, color, alpha * 0.9);
        }
        self.draw_text("FUMEN", x + 42, y, scale, color, alpha);
    }

    fn fill_slanted_bar(
        &mut self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        color: (u8, u8, u8),
        alpha: f32,
    ) {
        for row in 0..height {
            self.fill_rect(x + row / 3, y + row, width, 1, color, alpha);
        }
    }

    fn draw_text(
        &mut self,
        text: &str,
        x: i32,
        y: i32,
        scale: i32,
        color: (u8, u8, u8),
        alpha: f32,
    ) {
        let mut cursor_x = x;
        for character in text.chars() {
            let glyph = glyph_rows(character);
            for (row_index, row) in glyph.iter().enumerate() {
                for col in 0..5 {
                    if row & (1 << (4 - col)) != 0 {
                        self.fill_rect(
                            cursor_x + (col * scale),
                            y + (row_index as i32 * scale),
                            scale,
                            scale,
                            color,
                            alpha,
                        );
                    }
                }
            }
            cursor_x += 6 * scale;
        }
    }

    fn draw_mono_text(
        &mut self,
        text: &str,
        x: i32,
        y: i32,
        scale: i32,
        color: (u8, u8, u8),
        alpha: f32,
    ) {
        let mut cursor_x = x;
        for character in text.chars() {
            let glyph = glyph_rows(character);
            for (row_index, row) in glyph.iter().enumerate() {
                for col in 0..5 {
                    if row & (1 << (4 - col)) != 0 {
                        self.fill_rect(
                            cursor_x + (col * scale),
                            y + (row_index as i32 * scale),
                            scale,
                            scale,
                            color,
                            alpha,
                        );
                    }
                }
            }
            cursor_x += 7 * scale;
        }
    }

    fn fill_rect(
        &mut self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        color: (u8, u8, u8),
        alpha: f32,
    ) {
        for py in y..(y + height) {
            for px in x..(x + width) {
                self.blend_pixel(px, py, color, alpha);
            }
        }
    }

    fn fill_rounded_rect(
        &mut self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        radius: i32,
        color: (u8, u8, u8),
        alpha: f32,
    ) {
        for py in y..(y + height) {
            for px in x..(x + width) {
                let dx = if px < x + radius {
                    x + radius - px
                } else if px >= x + width - radius {
                    px - (x + width - radius - 1)
                } else {
                    0
                };
                let dy = if py < y + radius {
                    y + radius - py
                } else if py >= y + height - radius {
                    py - (y + height - radius - 1)
                } else {
                    0
                };

                if dx == 0 || dy == 0 || dx * dx + dy * dy <= radius * radius {
                    self.blend_pixel(px, py, color, alpha);
                }
            }
        }
    }

    fn fill_circle(
        &mut self,
        center_x: i32,
        center_y: i32,
        radius: i32,
        color: (u8, u8, u8),
        alpha: f32,
    ) {
        let radius_sq = radius * radius;
        for py in (center_y - radius)..=(center_y + radius) {
            for px in (center_x - radius)..=(center_x + radius) {
                let dx = px - center_x;
                let dy = py - center_y;
                if dx * dx + dy * dy <= radius_sq {
                    self.blend_pixel(px, py, color, alpha);
                }
            }
        }
    }

    fn set_pixel(&mut self, x: u32, y: u32, color: (u8, u8, u8)) {
        let index = ((y * self.width + x) * 3) as usize;
        self.pixels[index] = color.0;
        self.pixels[index + 1] = color.1;
        self.pixels[index + 2] = color.2;
    }

    fn blend_pixel(&mut self, x: i32, y: i32, color: (u8, u8, u8), alpha: f32) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return;
        }

        let index = (((y as u32) * self.width + x as u32) * 3) as usize;
        self.pixels[index] = blend_channel(self.pixels[index], color.0, alpha);
        self.pixels[index + 1] = blend_channel(self.pixels[index + 1], color.1, alpha);
        self.pixels[index + 2] = blend_channel(self.pixels[index + 2], color.2, alpha);
    }
}

fn blend_channel(base: u8, overlay: u8, alpha: f32) -> u8 {
    ((base as f32 * (1.0 - alpha)) + (overlay as f32 * alpha)).round() as u8
}

fn mix_rgb(left: (u8, u8, u8), right: (u8, u8, u8), amount: f32) -> (u8, u8, u8) {
    (
        blend_channel(left.0, right.0, amount),
        blend_channel(left.1, right.1, amount),
        blend_channel(left.2, right.2, amount),
    )
}

fn encode_png_rgb(width: u32, height: u32, pixels: &[u8]) -> Result<Vec<u8>, AppError> {
    let mut scanlines = Vec::with_capacity((height * (1 + width * 3)) as usize);
    let row_len = (width * 3) as usize;
    for row in pixels.chunks_exact(row_len) {
        scanlines.push(0);
        scanlines.extend_from_slice(row);
    }

    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(&scanlines).map_err(AppError::from)?;
    let compressed = encoder.finish().map_err(AppError::from)?;

    let mut png = Vec::new();
    png.extend_from_slice(b"\x89PNG\r\n\x1a\n");

    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    write_png_chunk(&mut png, b"IHDR", &ihdr);
    write_png_chunk(&mut png, b"IDAT", &compressed);
    write_png_chunk(&mut png, b"IEND", &[]);

    Ok(png)
}

fn write_png_chunk(png: &mut Vec<u8>, chunk_type: &[u8; 4], data: &[u8]) {
    png.extend_from_slice(&(data.len() as u32).to_be_bytes());
    png.extend_from_slice(chunk_type);
    png.extend_from_slice(data);

    let mut crc_data = Vec::with_capacity(chunk_type.len() + data.len());
    crc_data.extend_from_slice(chunk_type);
    crc_data.extend_from_slice(data);
    png.extend_from_slice(&crc32(&crc_data).to_be_bytes());
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for byte in data {
        crc ^= *byte as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

fn rounded_rect_contains(px: i32, py: i32, width: i32, height: i32, radius: i32) -> bool {
    let dx = if px < radius {
        radius - px
    } else if px >= width - radius {
        px - (width - radius - 1)
    } else {
        0
    };
    let dy = if py < radius {
        radius - py
    } else if py >= height - radius {
        py - (height - radius - 1)
    } else {
        0
    };

    dx == 0 || dy == 0 || dx * dx + dy * dy <= radius * radius
}

fn normalize_card_text(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_uppercase()
            } else if matches!(character, ' ' | '-' | '_' | ':' | '/' | '&' | '+') {
                character
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn wrap_card_text(value: &str, max_chars: usize, max_lines: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();

    for word in value.split_whitespace() {
        let separator = if current.is_empty() { 0 } else { 1 };
        if !current.is_empty() && current.len() + separator + word.len() > max_chars {
            lines.push(current);
            current = String::new();
            if lines.len() == max_lines {
                break;
            }
        }

        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }

    if lines.len() < max_lines && !current.is_empty() {
        lines.push(current);
    }

    if lines.is_empty() {
        lines.push("UNTITLED SCORE".to_owned());
    }

    if lines.len() == max_lines {
        let original = value.split_whitespace().collect::<Vec<_>>().join(" ");
        let displayed = lines.join(" ");
        if original.len() > displayed.len() {
            if let Some(last) = lines.last_mut() {
                *last = ellipsize_card_text(last, max_chars);
            }
        }
    }

    lines
}

fn ellipsize_card_text(value: &str, max_chars: usize) -> String {
    let mut result = value
        .chars()
        .take(max_chars.saturating_sub(3))
        .collect::<String>();
    result.push_str("...");
    result
}

fn text_pixel_width(text: &str, scale: i32) -> i32 {
    text.chars().count() as i32 * 6 * scale
}

fn glyph_rows(character: char) -> [u8; 7] {
    match character.to_ascii_uppercase() {
        'A' => [
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'B' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
        ],
        'C' => [
            0b01111, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b01111,
        ],
        'D' => [
            0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
        ],
        'E' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
        ],
        'F' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'G' => [
            0b01111, 0b10000, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111,
        ],
        'H' => [
            0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'I' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b11111,
        ],
        'J' => [
            0b00111, 0b00010, 0b00010, 0b00010, 0b10010, 0b10010, 0b01100,
        ],
        'K' => [
            0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
        ],
        'L' => [
            0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
        ],
        'M' => [
            0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
        ],
        'N' => [
            0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001,
        ],
        'O' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'P' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'Q' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101,
        ],
        'R' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ],
        'S' => [
            0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        'T' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'U' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'V' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100,
        ],
        'W' => [
            0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010,
        ],
        'X' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001,
        ],
        'Y' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'Z' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111,
        ],
        '0' => [
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ],
        '1' => [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        '2' => [
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
        ],
        '3' => [
            0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        '4' => [
            0b10010, 0b10010, 0b10010, 0b11111, 0b00010, 0b00010, 0b00010,
        ],
        '5' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b00001, 0b00001, 0b11110,
        ],
        '6' => [
            0b01110, 0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ],
        '7' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ],
        '8' => [
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ],
        '9' => [
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110,
        ],
        '-' => [
            0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000,
        ],
        '_' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b11111,
        ],
        ':' => [
            0b00000, 0b00100, 0b00100, 0b00000, 0b00100, 0b00100, 0b00000,
        ],
        '/' => [
            0b00001, 0b00010, 0b00010, 0b00100, 0b01000, 0b01000, 0b10000,
        ],
        '&' => [
            0b01100, 0b10010, 0b10100, 0b01000, 0b10101, 0b10010, 0b01101,
        ],
        '+' => [
            0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000,
        ],
        '.' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100,
        ],
        ' ' => [0; 7],
        _ => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b00100, 0b00000, 0b00100,
        ],
    }
}

fn score_badge(record: &MusicRecord) -> String {
    if let Some(icon) = record
        .icon
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return icon.chars().take(2).collect();
    }

    let badge = record
        .title
        .split_whitespace()
        .filter_map(|word| word.chars().next())
        .take(2)
        .collect::<String>()
        .to_uppercase()
        .chars()
        .take(2)
        .collect::<String>();

    if badge.is_empty() {
        "F".to_owned()
    } else {
        badge
    }
}

fn wrap_svg_text(value: &str, max_chars: usize, max_lines: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();

    for word in value.split_whitespace() {
        let separator = if current.is_empty() { 0 } else { 1 };
        if !current.is_empty()
            && current.chars().count() + separator + word.chars().count() > max_chars
        {
            lines.push(current);
            current = String::new();
            if lines.len() == max_lines {
                break;
            }
        }

        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }

    if lines.len() < max_lines && !current.is_empty() {
        lines.push(current);
    }

    if lines.is_empty() {
        lines.push("Untitled score".to_owned());
    }

    lines = lines
        .into_iter()
        .map(|line| {
            if line.chars().count() > max_chars {
                ellipsize(&line, max_chars)
            } else {
                line
            }
        })
        .collect();

    if lines.len() == max_lines {
        let original = value.split_whitespace().collect::<Vec<_>>().join(" ");
        let displayed = lines.join(" ");
        if let Some(last) = lines.last_mut() {
            if original.chars().count() > displayed.chars().count() {
                *last = ellipsize(last, max_chars);
            }
        }
    }

    lines
}

fn ellipsize(value: &str, max_chars: usize) -> String {
    let limit = max_chars.saturating_sub(3);
    let mut result = value.chars().take(limit).collect::<String>();
    result.push_str("...");
    result
}

fn svg_tspans(lines: &[String], x: i32, y: i32, line_height: i32) -> String {
    lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            format!(
                r#"<tspan x="{x}" y="{}">{}</tspan>"#,
                y + (index as i32 * line_height),
                svg_escape(line)
            )
        })
        .collect::<Vec<_>>()
        .join("")
}

fn percent_encode_path_segment(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                vec![byte as char]
            }
            _ => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn svg_escape(value: &str) -> String {
    html_escape(value).replace('\'', "&apos;")
}

fn binary_response(
    bytes: Bytes,
    content_type: String,
    content_encoding: Option<String>,
    content_disposition: Option<String>,
) -> Response {
    let mut response = Response::new(axum::body::Body::from(bytes));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&content_type)
            .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
    );

    if let Some(content_disposition) = content_disposition {
        if let Ok(value) = HeaderValue::from_str(&content_disposition) {
            response
                .headers_mut()
                .insert(header::CONTENT_DISPOSITION, value);
        }
    }

    if let Some(content_encoding) = content_encoding {
        if let Ok(value) = HeaderValue::from_str(&content_encoding) {
            response
                .headers_mut()
                .insert(header::CONTENT_ENCODING, value);
        }
    }

    response
}

fn text_response(body: String, content_type: &str, cache_control: Option<&str>) -> Response {
    let mut response = Response::new(axum::body::Body::from(body));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(content_type)
            .unwrap_or_else(|_| HeaderValue::from_static("text/plain; charset=utf-8")),
    );
    response.headers_mut().insert(
        header::HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );

    if let Some(cache_control) = cache_control {
        if let Ok(value) = HeaderValue::from_str(cache_control) {
            response.headers_mut().insert(header::CACHE_CONTROL, value);
        }
    }

    response
}

async fn local_file_response(
    path: &std::path::Path,
    content_type: &str,
    content_disposition: Option<String>,
    range_header: Option<&HeaderValue>,
) -> Result<Response, AppError> {
    let metadata = tokio::fs::metadata(path).await.map_err(AppError::from)?;
    let file_len = metadata.len();

    let parsed_range = range_header
        .map(|value| parse_byte_range_header(value, file_len))
        .transpose()?
        .flatten();

    let (start, end, status) = match parsed_range {
        Some((start, end)) => (start, end, StatusCode::PARTIAL_CONTENT),
        None if file_len == 0 => (0, 0, StatusCode::OK),
        None => (0, file_len - 1, StatusCode::OK),
    };

    let byte_count = if file_len == 0 {
        0usize
    } else {
        (end - start + 1) as usize
    };

    let mut file = tokio::fs::File::open(path).await.map_err(AppError::from)?;
    if byte_count > 0 {
        file.seek(std::io::SeekFrom::Start(start))
            .await
            .map_err(AppError::from)?;
    }

    let mut bytes = vec![0u8; byte_count];
    if byte_count > 0 {
        file.read_exact(&mut bytes).await.map_err(AppError::from)?;
    }

    let mut response = binary_response(
        Bytes::from(bytes),
        content_type.to_owned(),
        None,
        content_disposition,
    );
    *response.status_mut() = status;
    response
        .headers_mut()
        .insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    response.headers_mut().insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&byte_count.to_string())
            .unwrap_or_else(|_| HeaderValue::from_static("0")),
    );

    if status == StatusCode::PARTIAL_CONTENT {
        let content_range = format!("bytes {start}-{end}/{file_len}");
        response.headers_mut().insert(
            header::CONTENT_RANGE,
            HeaderValue::from_str(&content_range)
                .unwrap_or_else(|_| HeaderValue::from_static("bytes */0")),
        );
    }

    Ok(response)
}

fn parse_byte_range_header(
    value: &HeaderValue,
    file_len: u64,
) -> Result<Option<(u64, u64)>, AppError> {
    if file_len == 0 {
        return Ok(None);
    }

    let value = value
        .to_str()
        .map_err(|_| AppError::bad_request("Invalid Range header"))?
        .trim();

    let range_spec = value
        .strip_prefix("bytes=")
        .ok_or_else(|| AppError::bad_request("Only bytes ranges are supported"))?;

    if range_spec.contains(',') {
        return Err(AppError::bad_request(
            "Multiple byte ranges are not supported",
        ));
    }

    let (start_raw, end_raw) = range_spec
        .split_once('-')
        .ok_or_else(|| AppError::bad_request("Invalid Range header"))?;

    let invalid_range = || {
        AppError::new(
            StatusCode::RANGE_NOT_SATISFIABLE,
            format!("Requested range is not satisfiable for a {file_len}-byte file"),
        )
    };

    let range = if start_raw.is_empty() {
        let suffix_len = end_raw
            .parse::<u64>()
            .map_err(|_| AppError::bad_request("Invalid Range header"))?;
        if suffix_len == 0 {
            return Err(invalid_range());
        }
        let start = file_len.saturating_sub(suffix_len);
        (start, file_len - 1)
    } else {
        let start = start_raw
            .parse::<u64>()
            .map_err(|_| AppError::bad_request("Invalid Range header"))?;
        if start >= file_len {
            return Err(invalid_range());
        }

        let end = if end_raw.is_empty() {
            file_len - 1
        } else {
            let parsed_end = end_raw
                .parse::<u64>()
                .map_err(|_| AppError::bad_request("Invalid Range header"))?;
            if parsed_end < start {
                return Err(invalid_range());
            }
            parsed_end.min(file_len - 1)
        };

        (start, end)
    };

    Ok(Some(range))
}
