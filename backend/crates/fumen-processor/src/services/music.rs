use crate::audio::{ConversionOutcome, ProgressLogSender, StemResult, emit_progress_with_step};
use crate::{AppError, AppState, processing};
use bytes::Bytes;
use diesel_async::RunQueryDsl;
use flate2::{Compression, write::GzEncoder};
use fumen_core::models::NewStem;
pub use fumen_core::music::{find_public_stems, processing_log_key};
use fumen_core::schema::stems;
use std::io::Write;
use tracing::warn;

#[tracing::instrument(
    skip(state, stems, error),
    fields(music_id = %music_id, stem_count = stems.len(), stems_status = status)
)]
pub async fn store_stems(
    state: &AppState,
    music_id: &str,
    stems: Vec<StemResult>,
    status: String,
    error: Option<String>,
    progress_log: Option<&ProgressLogSender>,
) -> Result<(String, Option<String>), AppError> {
    let mut conn = state.db_rw.get().await?;
    let storage_target = if state.storage.is_s3() {
        "S3"
    } else {
        "storage"
    };
    let total = stems.len();

    emit_storage_progress(
        progress_log,
        processing::LOG_STEP_UPLOAD,
        format!("stems: uploading {total} rendered stem file(s) to {storage_target}."),
    );

    for (index, stem) in stems.into_iter().enumerate() {
        let size_bytes = stem.bytes.len() as i64;
        let storage_key = format!("stems/{music_id}/{}.ogg", stem.track_index);
        let drum_map_json = stem
            .drum_map
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| AppError::from(anyhow::Error::from(error)))?;
        emit_storage_progress(
            progress_log,
            processing::LOG_STEP_UPLOAD,
            format!(
                "stems: uploading [{}/{}] '{}' ({} KB) to {storage_target} as {}.",
                index + 1,
                total,
                stem.track_name,
                size_bytes / 1024,
                storage_key,
            ),
        );
        state
            .storage
            .upload_bytes(&storage_key, stem.bytes.clone(), "audio/ogg")
            .await?;
        emit_storage_progress(
            progress_log,
            processing::LOG_STEP_UPLOAD,
            format!(
                "stems: uploaded [{}/{}] '{}' to {storage_target} ({} KB).",
                index + 1,
                total,
                stem.track_name,
                size_bytes / 1024,
            ),
        );

        diesel::insert_into(stems::table)
            .values(NewStem {
                music_id,
                track_index: stem.track_index as i64,
                track_name: &stem.track_name,
                instrument_name: &stem.instrument_name,
                storage_key: &storage_key,
                size_bytes,
                drum_map_json: drum_map_json.as_deref(),
            })
            .execute(&mut conn)
            .await?;
    }

    emit_storage_progress(
        progress_log,
        processing::LOG_STEP_UPLOAD,
        format!("stems: upload to {storage_target} completed."),
    );
    Ok((status, error))
}

#[tracing::instrument(skip(state, outcome), fields(music_id = %music_id, kind = kind))]
pub async fn store_conversion(
    state: &AppState,
    music_id: &str,
    kind: &str,
    outcome: ConversionOutcome,
    progress_log: Option<&ProgressLogSender>,
) -> Result<(Option<String>, String, Option<String>), AppError> {
    match outcome {
        ConversionOutcome::Ready {
            bytes,
            content_type,
            extension,
        } => {
            let object_key = format!("{kind}/{music_id}.{extension}");
            let storage_target = if state.storage.is_s3() {
                "S3"
            } else {
                "storage"
            };
            let (stored_bytes, content_encoding) = if kind == "musicxml" && state.storage.is_s3() {
                (gzip_bytes(&bytes)?, Some("gzip"))
            } else {
                (bytes, None)
            };
            emit_storage_progress(
                progress_log,
                processing::LOG_STEP_UPLOAD,
                format!(
                    "{kind}: uploading {} KB to {storage_target} as {}.",
                    stored_bytes.len() / 1024,
                    object_key,
                ),
            );
            state
                .storage
                .upload_bytes_with_encoding(
                    &object_key,
                    stored_bytes,
                    content_type,
                    content_encoding,
                )
                .await?;
            emit_storage_progress(
                progress_log,
                processing::LOG_STEP_UPLOAD,
                format!("{kind}: upload to {storage_target} completed."),
            );
            Ok((Some(object_key), "ready".to_owned(), None))
        }
        ConversionOutcome::Unavailable { reason } => {
            Ok((None, "unavailable".to_owned(), Some(reason)))
        }
        ConversionOutcome::Failed { reason } => {
            warn!("{kind} conversion failed for {music_id}: {reason}");
            Ok((None, "failed".to_owned(), Some(reason)))
        }
    }
}

pub fn estimated_upload_bytes_for_conversion(
    storage_is_s3: bool,
    kind: &str,
    outcome: &ConversionOutcome,
) -> Result<Option<usize>, AppError> {
    match outcome {
        ConversionOutcome::Ready { bytes, .. } => {
            if kind == "musicxml" && storage_is_s3 {
                Ok(Some(gzip_bytes(bytes)?.len()))
            } else {
                Ok(Some(bytes.len()))
            }
        }
        ConversionOutcome::Unavailable { .. } | ConversionOutcome::Failed { .. } => Ok(None),
    }
}

pub fn estimated_upload_bytes_for_stems(stems: &[StemResult]) -> usize {
    stems.iter().map(|stem| stem.bytes.len()).sum()
}

fn emit_storage_progress(
    progress_log: Option<&ProgressLogSender>,
    step: &'static str,
    message: impl Into<String>,
) {
    emit_progress_with_step(progress_log, step, message);
}

fn gzip_bytes(bytes: &Bytes) -> Result<Bytes, AppError> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes).map_err(AppError::from)?;
    let compressed = encoder.finish().map_err(AppError::from)?;
    Ok(Bytes::from(compressed))
}
