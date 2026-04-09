use crate::schemas::{PublicMusicResponse, StemInfo};
use crate::services::music;
use crate::{AppError, AppState, sanitize_content_disposition};
use axum::{
    Json, Router,
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::Response,
    extract::{Path, State},
    routing::get,
};
use bytes::Bytes;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/public/{access_key}", get(public_music))
        .route("/public/{access_key}/audio", get(public_music_audio))
        .route("/public/{access_key}/midi", get(public_music_midi))
        .route("/public/{access_key}/musicxml", get(public_music_musicxml))
        .route("/public/{access_key}/stems", get(public_music_stems))
        .route(
            "/public/{access_key}/stems/{track_index}",
            get(public_music_stem_audio),
        )
        .route("/public/{access_key}/download", get(public_music_download))
        .route("/public/{access_key}/icon", get(public_music_icon))
}

async fn public_music(
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

async fn public_music_audio(
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

async fn public_music_midi(
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

async fn public_music_musicxml(
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

async fn public_music_download(
    State(state): State<AppState>,
    Path(access_key): Path<String>,
) -> Result<Response, AppError> {
    let record = music::find_public_music_record(&state, &access_key)
        .await?
        .ok_or_else(|| AppError::not_found("Music not found"))?;

    let (bytes, content_type, content_encoding) = state.storage.get_bytes(&record.object_key).await?;
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

async fn public_music_icon(
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

async fn public_music_stems(
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

async fn public_music_stem_audio(
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

    let (bytes, content_type, content_encoding) = state.storage.get_bytes(&stem.storage_key).await?;
    Ok(binary_response(
        bytes,
        content_type.unwrap_or_else(|| "audio/ogg".to_owned()),
        content_encoding,
        Some(format!("inline; filename=\"{}.ogg\"", stem.track_name)),
    ))
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
            response.headers_mut().insert(header::CONTENT_ENCODING, value);
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