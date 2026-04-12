use crate::schemas::{ErrorResponse, PublicMusicResponse, ReportPlaytimeRequest, StemInfo};
use crate::services::{auth, music};
use crate::{AppError, AppState, sanitize_content_disposition};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::Response,
};
use bytes::Bytes;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

pub(super) fn routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/public/{access_key}", crate::op_get!(state, "/public/{access_key}", public_music))
        .route("/public/{access_key}/audio", crate::op_get!(state, "/public/{access_key}/audio", public_music_audio))
        .route("/public/{access_key}/midi", crate::op_get!(state, "/public/{access_key}/midi", public_music_midi))
        .route("/public/{access_key}/musicxml", crate::op_get!(state, "/public/{access_key}/musicxml", public_music_musicxml))
        .route("/public/{access_key}/stems", crate::op_get!(state, "/public/{access_key}/stems", public_music_stems))
        .route(
            "/public/{access_key}/playtime",
            crate::op_post!(state, "/public/{access_key}/playtime", report_public_music_playtime),
        )
        .route(
            "/public/{access_key}/stems/{track_index}",
            crate::op_get!(state, "/public/{access_key}/stems/{track_index}", public_music_stem_audio),
        )
        .route("/public/{access_key}/download", crate::op_get!(state, "/public/{access_key}/download", public_music_download))
        .route("/public/{access_key}/icon", crate::op_get!(state, "/public/{access_key}/icon", public_music_icon))
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
    music::add_user_track_playtime(&state.db_rw, &auth_context.user.id, &record.id, &normalized)
        .await?;

    Ok(StatusCode::NO_CONTENT)
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
