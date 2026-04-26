use crate::audio::{self, ProgressLogEvent, StemQualityProfile};
use crate::db::DbPool;
use crate::models::{MusicRecord, ProcessingJobRecord, UpdateMusicProcessing};
use crate::schema::{musics, processing_jobs, stems};
use crate::services::music;
use crate::{AppError, AppState, format_timestamp, sanitize_filename};
use bytes::Bytes;
use diesel::OptionalExtension;
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{BigInt, Text};
use diesel_async::RunQueryDsl;
use std::path::Path;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::{Mutex, Semaphore, mpsc, watch};
use tokio::task::JoinSet;

const PROCESSING_LOG_CONTENT_TYPE: &str = "text/plain; charset=utf-8";
const DEFAULT_MAX_ATTEMPTS: i64 = 25;

pub(crate) const JOB_STATUS_QUEUED: &str = "queued";
pub(crate) const JOB_STATUS_RUNNING: &str = "running";
pub(crate) const JOB_STATUS_COMPLETED: &str = "completed";
pub(crate) const JOB_STATUS_FAILED: &str = "failed";

pub(crate) const JOB_STEP_QUEUED: &str = "queued";
pub(crate) const JOB_STEP_FETCHING_INPUT: &str = "fetching_input";
pub(crate) const JOB_STEP_GENERATING_CORE: &str = "generating_core";
pub(crate) const JOB_STEP_GENERATING_STEMS: &str = "generating_stems";
pub(crate) const JOB_STEP_UPLOADING_ASSETS: &str = "uploading_assets";
pub(crate) const JOB_STEP_FINALIZING: &str = "finalizing";
pub(crate) const JOB_STEP_COMPLETED: &str = "completed";
pub(crate) const JOB_STEP_FAILED: &str = "failed";

pub(crate) const LOG_STEP_QUEUE: &str = "queue";
const LOG_STEP_INPUT: &str = "input";
const LOG_STEP_MUSICXML: &str = "musicxml";
const LOG_STEP_MIDI: &str = "midi";
const LOG_STEP_PREVIEW_MP3: &str = "preview_mp3";
const LOG_STEP_STEMS: &str = "stems";
const LOG_STEP_COMPRESS_STEMS: &str = "compress_stems";
pub(crate) const LOG_STEP_UPLOAD: &str = "upload_assets";
const LOG_STEP_DONE: &str = "done";

pub struct QueueProcessingJobRequest<'a> {
    pub music_id: &'a str,
    pub source_object_key: &'a str,
    pub source_filename: &'a str,
    pub quality_profile: &'a str,
}

#[derive(Clone)]
pub struct MusicProcessingLog {
    state: AppState,
    log_key: String,
    content: Arc<Mutex<String>>,
    current_step: Arc<Mutex<Option<String>>>,
}

impl MusicProcessingLog {
    pub fn new(state: AppState, music_id: impl Into<String>) -> Self {
        let music_id = music_id.into();
        Self {
            state,
            log_key: music::processing_log_key(&music_id),
            content: Arc::new(Mutex::new(String::new())),
            current_step: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn set_step(&self, step: impl Into<String>) {
        *self.current_step.lock().await = Some(step.into());
    }

    pub async fn reset(&mut self, lines: &[String]) {
        let mut content = self.content.lock().await;
        content.clear();
        for line in lines {
            Self::push_line(&mut content, None, &format!("INFO {line}"));
        }
        self.persist(content.clone()).await;
    }

    pub async fn append(&mut self, message: impl AsRef<str>) {
        self.append_with_level_and_step("INFO", message, None).await;
    }

    pub async fn append_warning(&mut self, message: impl AsRef<str>) {
        self.append_with_level_and_step("WARNING", message, None)
            .await;
    }

    pub async fn append_error(&mut self, message: impl AsRef<str>) {
        self.append_with_level_and_step("ERROR", message, None)
            .await;
    }

    pub async fn append_progress(&mut self, message: impl AsRef<str>) {
        self.append_progress_with_step(message, None).await;
    }

    pub async fn append_progress_with_step(
        &mut self,
        message: impl AsRef<str>,
        explicit_step: Option<&str>,
    ) {
        if let Some(step) = explicit_step {
            self.append_with_level_and_step("INFO", message, Some(step))
                .await;
            return;
        }

        let current_step = self.current_step.lock().await.clone();
        let inferred_step = infer_log_step(message.as_ref(), current_step.as_deref());
        self.append_with_level_and_step("INFO", message, inferred_step.as_deref())
            .await;
    }

    async fn append_with_level_and_step(
        &mut self,
        level: &str,
        message: impl AsRef<str>,
        explicit_step: Option<&str>,
    ) {
        debug_marker("processing-log-append-enter");
        let current_step = if explicit_step.is_some() {
            None
        } else {
            self.current_step.lock().await.clone()
        };
        let step = explicit_step
            .or(current_step.as_deref())
            .map(ToOwned::to_owned);
        let content_to_persist = {
            let mut content = self.content.lock().await;
            debug_marker("processing-log-append-after-lock");
            Self::push_line(
                &mut content,
                step.as_deref(),
                &format!("{level} {}", message.as_ref()),
            );
            content.clone()
        };
        debug_marker("processing-log-append-after-push-line");
        self.persist(content_to_persist).await;
        debug_marker("processing-log-append-after-persist");
    }

    fn push_line(content: &mut String, step: Option<&str>, message: &str) {
        let timestamp = chrono::Utc::now().to_rfc3339();
        for line in message.lines() {
            let line = line.trim_end();
            if line.is_empty() {
                continue;
            }
            content.push('[');
            content.push_str(&timestamp);
            content.push_str("] ");
            if let Some(step) = step.filter(|value| !value.trim().is_empty()) {
                content.push_str("[step=");
                content.push_str(step);
                content.push_str("] ");
            }
            content.push_str(line);
            content.push('\n');
        }
    }

    async fn persist(&self, content: String) {
        debug_marker("processing-log-persist-enter");
        let storage = self.state.storage.clone();
        let log_key = self.log_key.clone();
        let upload = tokio::spawn(async move {
            storage
                .upload_bytes_quiet(
                    &log_key,
                    Bytes::from(content.into_bytes()),
                    PROCESSING_LOG_CONTENT_TYPE,
                )
                .await
        });

        if let Err(error) = match upload.await {
            Ok(result) => result,
            Err(error) => Err(error.into()),
        } {
            tracing::warn!(
                storage_key = %self.log_key,
                error = ?error,
                "failed to persist score processing log"
            );
        }
        debug_marker("processing-log-persist-exit");
    }
}

pub(crate) fn spawn_processing_log_bridge(
    log: MusicProcessingLog,
) -> (audio::ProgressLogSender, tokio::task::JoinHandle<()>) {
    let (sender, mut receiver) = mpsc::unbounded_channel::<ProgressLogEvent>();
    let handle = tokio::spawn(async move {
        let mut log = log;
        while let Some(event) = receiver.recv().await {
            log.append_progress_with_step(event.message, event.step)
                .await;
        }
    });
    (sender, handle)
}

fn infer_log_step(message: &str, current_step: Option<&str>) -> Option<String> {
    let normalized = message.trim().to_ascii_lowercase();

    if normalized.contains("score.musicxml") || normalized.contains("application/xml") {
        return Some(LOG_STEP_MUSICXML.to_owned());
    }

    if normalized.contains("preview.mid") || normalized.contains("audio/midi") {
        return Some(LOG_STEP_MIDI.to_owned());
    }

    if normalized.contains("preview.mp3")
        || normalized.contains("audio/mpeg")
        || normalized.starts_with("audio: ")
    {
        return Some(LOG_STEP_PREVIEW_MP3.to_owned());
    }

    if normalized.starts_with("stems: compressing [")
        || normalized.starts_with("stems: compressed [")
    {
        return Some(LOG_STEP_COMPRESS_STEMS.to_owned());
    }

    if normalized.starts_with("upload: ")
        || normalized.contains(" uploading ")
        || normalized.contains(" uploaded ")
        || normalized.contains("upload to s3")
        || normalized.contains("upload to storage")
    {
        return Some(LOG_STEP_UPLOAD.to_owned());
    }

    if normalized.starts_with("stems: ") || normalized.contains("musescore-direct-stems") {
        return Some(LOG_STEP_STEMS.to_owned());
    }

    current_step.map(ToOwned::to_owned)
}

pub async fn load_music_processing_log(state: &AppState, music_id: &str) -> String {
    match state
        .storage
        .get_bytes(&music::processing_log_key(music_id))
        .await
    {
        Ok((bytes, _, _)) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(error) => {
            tracing::warn!(
                music_id = %music_id,
                error = ?error,
                "failed to load score processing log"
            );
            String::new()
        }
    }
}

pub fn processing_statuses(record: &MusicRecord) -> [&str; 4] {
    [
        record.audio_status.as_str(),
        record.midi_status.as_str(),
        record.musicxml_status.as_str(),
        record.stems_status.as_str(),
    ]
}

pub fn build_processing_log_header(
    action: &str,
    filename: &str,
    quality_profile: &StemQualityProfile,
) -> Vec<String> {
    vec![
        format!("{action} requested."),
        format!("Input file: {filename}"),
        format!("Stem quality: {}", quality_profile.as_str()),
    ]
}

pub async fn reset_music_processing_state(
    state: &AppState,
    record: &MusicRecord,
    log: &mut MusicProcessingLog,
) -> Result<(), AppError> {
    log.append("Resetting score processing state and clearing previous derived assets.")
        .await;

    let existing_stems = music::find_public_stems(&state.db_rw, &state.db_rw, &record.id).await?;
    let mut keys_to_delete = Vec::new();
    if let Some(value) = record.audio_object_key.as_ref() {
        keys_to_delete.push(value.clone());
    }
    if let Some(value) = record.midi_object_key.as_ref() {
        keys_to_delete.push(value.clone());
    }
    if let Some(value) = record.musicxml_object_key.as_ref() {
        keys_to_delete.push(value.clone());
    }
    for stem in &existing_stems {
        keys_to_delete.push(stem.storage_key.clone());
    }

    let mut conn = state.db_rw.get().await?;
    diesel::delete(stems::table.filter(stems::music_id.eq(&record.id)))
        .execute(&mut conn)
        .await?;
    diesel::update(musics::table.find(&record.id))
        .set(UpdateMusicProcessing {
            audio_object_key: None,
            audio_status: "processing",
            audio_error: None,
            midi_object_key: None,
            midi_status: "processing",
            midi_error: None,
            musicxml_object_key: None,
            musicxml_status: "processing",
            musicxml_error: None,
            stems_status: "processing",
            stems_error: None,
        })
        .execute(&mut conn)
        .await?;
    drop(conn);

    for key in keys_to_delete {
        if let Err(error) = state.storage.delete_key(&key).await {
            tracing::warn!(
                music_id = %record.id,
                storage_key = %key,
                error = ?error,
                "failed to delete derived asset during restart"
            );
            log.append_warning(format!("Failed to remove previous asset {key}: {error}"))
                .await;
        }
    }

    log.append("Processing state reset. The score has been queued again.")
        .await;
    Ok(())
}

pub async fn enqueue_music_processing_job(
    db: &DbPool,
    request: QueueProcessingJobRequest<'_>,
) -> Result<(), AppError> {
    let queued_at = crate::utc_now_string();
    let mut conn = db.get().await?;
    sql_query(
        "INSERT INTO processing_jobs (
            music_id,
            source_object_key,
            source_filename,
            quality_profile,
            status,
            current_step,
            attempt,
            max_attempts,
            worker_id,
            lease_expires_at,
            heartbeat_at,
            queued_at,
            started_at,
            finished_at,
            error_message
        ) VALUES (
            $1, $2, $3, $4, $5, $6, 1, $7, NULL, NULL, NULL, $8, NULL, NULL, NULL
        )
        ON CONFLICT (music_id) DO UPDATE
        SET source_object_key = EXCLUDED.source_object_key,
            source_filename = EXCLUDED.source_filename,
            quality_profile = EXCLUDED.quality_profile,
            status = EXCLUDED.status,
            current_step = EXCLUDED.current_step,
            attempt = processing_jobs.attempt + 1,
            max_attempts = EXCLUDED.max_attempts,
            worker_id = NULL,
            lease_expires_at = NULL,
            heartbeat_at = NULL,
            queued_at = EXCLUDED.queued_at,
            started_at = NULL,
            finished_at = NULL,
            error_message = NULL",
    )
    .bind::<Text, _>(request.music_id)
    .bind::<Text, _>(request.source_object_key)
    .bind::<Text, _>(request.source_filename)
    .bind::<Text, _>(request.quality_profile)
    .bind::<Text, _>(JOB_STATUS_QUEUED)
    .bind::<Text, _>(JOB_STEP_QUEUED)
    .bind::<BigInt, _>(DEFAULT_MAX_ATTEMPTS)
    .bind::<Text, _>(&queued_at)
    .execute(&mut conn)
    .await?;

    Ok(())
}

pub(crate) async fn claim_next_processing_job(
    db: &DbPool,
    worker_id: &str,
    lease_seconds: i64,
) -> Result<Option<ProcessingJobRecord>, AppError> {
    let now = chrono::Utc::now();
    let heartbeat_at = format_timestamp(now);
    let lease_expires_at = format_timestamp(now + chrono::Duration::seconds(lease_seconds));
    let mut conn = db.get().await?;

    Ok(sql_query(
        "WITH candidate AS (
            SELECT music_id
            FROM processing_jobs
            WHERE status = $1
               OR (status = $2 AND lease_expires_at IS NOT NULL AND lease_expires_at < $3)
            ORDER BY queued_at ASC
            LIMIT 1
            FOR UPDATE SKIP LOCKED
        )
        UPDATE processing_jobs AS jobs
        SET status = $2,
            current_step = $4,
            worker_id = $5,
            lease_expires_at = $6,
            heartbeat_at = $3,
            started_at = COALESCE(jobs.started_at, $3),
            finished_at = NULL,
            error_message = NULL
        FROM candidate
        WHERE jobs.music_id = candidate.music_id
        RETURNING
            jobs.music_id,
            jobs.source_object_key,
            jobs.source_filename,
            jobs.quality_profile,
            jobs.status,
            jobs.current_step,
            jobs.attempt,
            jobs.max_attempts,
            jobs.worker_id,
            jobs.lease_expires_at,
            jobs.heartbeat_at,
            jobs.queued_at,
            jobs.started_at,
            jobs.finished_at,
            jobs.error_message",
    )
    .bind::<Text, _>(JOB_STATUS_QUEUED)
    .bind::<Text, _>(JOB_STATUS_RUNNING)
    .bind::<Text, _>(&heartbeat_at)
    .bind::<Text, _>(JOB_STEP_FETCHING_INPUT)
    .bind::<Text, _>(worker_id)
    .bind::<Text, _>(&lease_expires_at)
    .get_result::<ProcessingJobRecord>(&mut conn)
    .await
    .optional()?)
}

pub(crate) async fn heartbeat_processing_job(
    db: &DbPool,
    music_id: &str,
    worker_id: &str,
    current_step: &str,
    lease_seconds: i64,
) -> Result<(), AppError> {
    let now = chrono::Utc::now();
    let heartbeat_at = format_timestamp(now);
    let lease_expires_at = format_timestamp(now + chrono::Duration::seconds(lease_seconds));
    let mut conn = db.get().await?;
    diesel::update(
        processing_jobs::table
            .filter(processing_jobs::music_id.eq(music_id))
            .filter(processing_jobs::worker_id.eq(Some(worker_id))),
    )
    .set((
        processing_jobs::status.eq(JOB_STATUS_RUNNING),
        processing_jobs::current_step.eq(current_step),
        processing_jobs::heartbeat_at.eq(Some(heartbeat_at)),
        processing_jobs::lease_expires_at.eq(Some(lease_expires_at)),
    ))
    .execute(&mut conn)
    .await?;

    Ok(())
}

pub(crate) async fn mark_processing_job_completed(
    db: &DbPool,
    music_id: &str,
    worker_id: &str,
) -> Result<(), AppError> {
    let finished_at = crate::utc_now_string();
    let mut conn = db.get().await?;
    diesel::update(
        processing_jobs::table
            .filter(processing_jobs::music_id.eq(music_id))
            .filter(processing_jobs::worker_id.eq(Some(worker_id))),
    )
    .set((
        processing_jobs::status.eq(JOB_STATUS_COMPLETED),
        processing_jobs::current_step.eq(JOB_STEP_COMPLETED),
        processing_jobs::lease_expires_at.eq::<Option<String>>(None),
        processing_jobs::heartbeat_at.eq(Some(finished_at.clone())),
        processing_jobs::finished_at.eq(Some(finished_at)),
        processing_jobs::error_message.eq::<Option<String>>(None),
    ))
    .execute(&mut conn)
    .await?;

    Ok(())
}

pub(crate) async fn mark_processing_job_failed(
    db: &DbPool,
    music_id: &str,
    worker_id: &str,
    error_message: &str,
) -> Result<(), AppError> {
    let finished_at = crate::utc_now_string();
    let mut conn = db.get().await?;
    diesel::update(
        processing_jobs::table
            .filter(processing_jobs::music_id.eq(music_id))
            .filter(processing_jobs::worker_id.eq(Some(worker_id))),
    )
    .set((
        processing_jobs::status.eq(JOB_STATUS_FAILED),
        processing_jobs::current_step.eq(JOB_STEP_FAILED),
        processing_jobs::lease_expires_at.eq::<Option<String>>(None),
        processing_jobs::heartbeat_at.eq(Some(finished_at.clone())),
        processing_jobs::finished_at.eq(Some(finished_at)),
        processing_jobs::error_message.eq(Some(error_message.to_owned())),
    ))
    .execute(&mut conn)
    .await?;

    Ok(())
}

pub(crate) async fn mark_music_processing_failed(
    state: &AppState,
    music_id: &str,
    error: String,
) -> Result<(), AppError> {
    let mut conn = state.db_rw.get().await?;
    diesel::update(musics::table.find(music_id))
        .set(crate::models::MarkMusicProcessingFailed {
            audio_status: "failed",
            audio_error: Some(error.as_str()),
            midi_status: "failed",
            midi_error: Some(error.as_str()),
            musicxml_status: "failed",
            musicxml_error: Some(error.as_str()),
            stems_status: "failed",
            stems_error: Some(error.as_str()),
        })
        .execute(&mut conn)
        .await?;

    Ok(())
}

fn publish_step(step_sender: Option<&watch::Sender<String>>, step: &str) {
    if let Some(step_sender) = step_sender {
        let _ = step_sender.send(step.to_owned());
    }
}

enum CoreConversionKind {
    Audio,
    Midi,
    MusicXml,
}

struct CoreConversionOutputs {
    audio: audio::ConversionOutcome,
    midi: audio::ConversionOutcome,
    musicxml: audio::ConversionOutcome,
}

async fn run_core_conversions(
    state: &AppState,
    input_path: &Path,
    output_dir: &Path,
    progress_sender: &audio::ProgressLogSender,
) -> Result<CoreConversionOutputs, AppError> {
    let max_parallel = state.config.processor_max_parallel_core_conversions.max(1);
    let semaphore = Arc::new(Semaphore::new(max_parallel));
    let mut jobs = JoinSet::new();
    let input_path = input_path.to_path_buf();
    let output_dir = output_dir.to_path_buf();

    for kind in [
        CoreConversionKind::Audio,
        CoreConversionKind::Midi,
        CoreConversionKind::MusicXml,
    ] {
        let config = state.config.clone();
        let input_path = input_path.clone();
        let output_dir = output_dir.clone();
        let progress_sender = progress_sender.clone();
        let semaphore = semaphore.clone();

        jobs.spawn(async move {
            let _permit = semaphore
                .acquire_owned()
                .await
                .map_err(|_| AppError::new("core conversion semaphore closed"))?;

            let outcome = match kind {
                CoreConversionKind::Audio => {
                    audio::generate_audio(&config, &input_path, &output_dir, Some(&progress_sender))
                        .await
                        .map_err(AppError::from)?
                }
                CoreConversionKind::Midi => {
                    audio::generate_midi(&config, &input_path, &output_dir, Some(&progress_sender))
                        .await
                        .map_err(AppError::from)?
                }
                CoreConversionKind::MusicXml => audio::generate_musicxml(
                    &config,
                    &input_path,
                    &output_dir,
                    Some(&progress_sender),
                )
                .await
                .map_err(AppError::from)?,
            };

            Ok::<_, AppError>((kind, outcome))
        });
    }

    let mut audio_outcome = None;
    let mut midi_outcome = None;
    let mut musicxml_outcome = None;

    while let Some(result) = jobs.join_next().await {
        let (kind, outcome) = result.map_err(|error| {
            AppError::new(format!("core conversion task join failed: {error}"))
        })??;

        match kind {
            CoreConversionKind::Audio => audio_outcome = Some(outcome),
            CoreConversionKind::Midi => midi_outcome = Some(outcome),
            CoreConversionKind::MusicXml => musicxml_outcome = Some(outcome),
        }
    }

    Ok(CoreConversionOutputs {
        audio: audio_outcome
            .ok_or_else(|| AppError::new("audio conversion task did not return a result"))?,
        midi: midi_outcome
            .ok_or_else(|| AppError::new("MIDI conversion task did not return a result"))?,
        musicxml: musicxml_outcome
            .ok_or_else(|| AppError::new("MusicXML conversion task did not return a result"))?,
    })
}

pub(crate) async fn execute_processing_job(
    state: &AppState,
    job: &ProcessingJobRecord,
    log: &mut MusicProcessingLog,
    step_sender: Option<&watch::Sender<String>>,
) -> Result<(), AppError> {
    debug_marker("execute-processing-job-enter");
    tracing::info!(
        music_id = %job.music_id,
        quality_profile = %job.quality_profile,
        filename = %job.source_filename,
        "starting processing job execution"
    );
    let quality_profile = StemQualityProfile::from_stored_or_default(&job.quality_profile);
    debug_marker("execute-processing-job-after-quality-profile");
    log.set_step(LOG_STEP_INPUT).await;
    publish_step(step_sender, JOB_STEP_FETCHING_INPUT);
    debug_marker("execute-processing-job-after-publish-step");
    debug_marker("execute-processing-job-before-fetch-log-format");
    let fetch_log_message = format!(
        "Fetching source score from storage key {}.",
        job.source_object_key
    );
    debug_marker("execute-processing-job-after-fetch-log-format");
    log.append(fetch_log_message).await;
    debug_marker("execute-processing-job-after-fetch-log");
    let (bytes, _, _) = state.storage.get_bytes(&job.source_object_key).await?;
    debug_marker("execute-processing-job-after-get-bytes");

    let (progress_sender, progress_handle) = spawn_processing_log_bridge(log.clone());
    let processing_result = async {
        let temp_dir = tempfile::tempdir()?;
        let safe_filename = sanitize_filename(&job.source_filename);
        let temp_input_path = temp_dir.path().join(&safe_filename);

        log.append(format!(
            "Writing temporary input file to {}.",
            temp_input_path.display()
        ))
        .await;
        fs::write(&temp_input_path, &bytes).await?;
        log.append("Temporary input file written. Starting conversion pipeline.")
            .await;

        log.set_step(LOG_STEP_MUSICXML).await;
        publish_step(step_sender, JOB_STEP_GENERATING_CORE);
        let CoreConversionOutputs {
            audio: audio_outcome,
            midi: midi_outcome,
            musicxml: musicxml_outcome,
        } = run_core_conversions(state, &temp_input_path, temp_dir.path(), &progress_sender)
            .await?;
        log.append("Audio, MIDI, and MusicXML conversion finished. Storing generated files.")
            .await;

        log.set_step(LOG_STEP_STEMS).await;
        publish_step(step_sender, JOB_STEP_GENERATING_STEMS);
        let (stem_results, stems_status, stems_error) = audio::generate_stems(
            &state.config,
            &temp_input_path,
            temp_dir.path(),
            quality_profile,
            Some(&progress_sender),
        )
        .await?;
        log.append(format!(
            "Stem generation finished with {} rendered stem file(s).",
            stem_results.len()
        ))
        .await;

        let mut upload_asset_count = stem_results.len();
        let mut upload_total_bytes = music::estimated_upload_bytes_for_stems(&stem_results);
        if let Some(bytes) =
            music::estimated_upload_bytes_for_conversion(state.storage.is_s3(), "audio", &audio_outcome)?
        {
            upload_asset_count += 1;
            upload_total_bytes += bytes;
        }
        if let Some(bytes) =
            music::estimated_upload_bytes_for_conversion(state.storage.is_s3(), "midi", &midi_outcome)?
        {
            upload_asset_count += 1;
            upload_total_bytes += bytes;
        }
        if let Some(bytes) = music::estimated_upload_bytes_for_conversion(
            state.storage.is_s3(),
            "musicxml",
            &musicxml_outcome,
        )? {
            upload_asset_count += 1;
            upload_total_bytes += bytes;
        }

        log.append(format!(
            "Upload phase prepared {upload_asset_count} asset(s) totaling {upload_total_bytes} bytes."
        ))
        .await;
        let _ = progress_sender.send(ProgressLogEvent {
            message: format!(
                "upload: prepared {upload_asset_count} asset(s) totaling {upload_total_bytes} bytes."
            ),
            step: Some(LOG_STEP_UPLOAD),
        });

        log.set_step(LOG_STEP_UPLOAD).await;
        publish_step(step_sender, JOB_STEP_UPLOADING_ASSETS);
        tokio::try_join!(
            music::store_conversion(
                state,
                &job.music_id,
                "audio",
                audio_outcome,
                Some(&progress_sender),
            ),
            music::store_conversion(
                state,
                &job.music_id,
                "midi",
                midi_outcome,
                Some(&progress_sender),
            ),
            music::store_conversion(
                state,
                &job.music_id,
                "musicxml",
                musicxml_outcome,
                Some(&progress_sender),
            ),
            music::store_stems(
                state,
                &job.music_id,
                stem_results,
                stems_status,
                stems_error,
                Some(&progress_sender),
            ),
        )
    }
    .await;

    drop(progress_sender);
    let _ = progress_handle.await;

    let (
        (audio_object_key, audio_status, audio_error),
        (midi_object_key, midi_status, midi_error),
        (musicxml_object_key, musicxml_status, musicxml_error),
        (stems_status, stems_error),
    ) = processing_result?;

    log.set_step(LOG_STEP_DONE).await;
    log.append(format!("Audio status: {audio_status}.")).await;
    if let Some(error) = audio_error.as_deref() {
        log.append(format!("Audio detail: {error}")).await;
    }
    log.append(format!("MIDI status: {midi_status}.")).await;
    if let Some(error) = midi_error.as_deref() {
        log.append(format!("MIDI detail: {error}")).await;
    }
    log.append(format!("MusicXML status: {musicxml_status}."))
        .await;
    if let Some(error) = musicxml_error.as_deref() {
        log.append(format!("MusicXML detail: {error}")).await;
    }
    log.append(format!("Stems status: {stems_status}.")).await;
    if let Some(error) = stems_error.as_deref() {
        log.append(format!("Stems detail: {error}")).await;
    }

    publish_step(step_sender, JOB_STEP_FINALIZING);
    let mut conn = state.db_rw.get().await?;
    diesel::update(musics::table.find(&job.music_id))
        .set(UpdateMusicProcessing {
            audio_object_key: audio_object_key.as_deref(),
            audio_status: &audio_status,
            audio_error: audio_error.as_deref(),
            midi_object_key: midi_object_key.as_deref(),
            midi_status: &midi_status,
            midi_error: midi_error.as_deref(),
            musicxml_object_key: musicxml_object_key.as_deref(),
            musicxml_status: &musicxml_status,
            musicxml_error: musicxml_error.as_deref(),
            stems_status: &stems_status,
            stems_error: stems_error.as_deref(),
        })
        .execute(&mut conn)
        .await?;

    log.append("Processing completed. Database state updated.")
        .await;
    Ok(())
}

fn debug_marker(message: &str) {
    if std::env::var("PROCESSOR_DEBUG_MARKERS")
        .ok()
        .is_some_and(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "on"))
    {
        eprintln!("[processor-debug] {message}");
    }
}
