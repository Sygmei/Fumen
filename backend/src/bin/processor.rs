#[path = "../app.rs"]
mod app;
#[path = "../audio.rs"]
mod audio;
#[path = "../config.rs"]
mod config;
#[path = "../db.rs"]
mod db;
#[path = "../models.rs"]
mod models;
#[path = "../openapi.rs"]
mod openapi;
#[path = "../routes/mod.rs"]
mod routes;
#[path = "../schema.rs"]
mod schema;
#[path = "../schemas.rs"]
mod schemas;
#[path = "../services/mod.rs"]
mod services;
#[path = "../storage.rs"]
mod storage;
#[path = "../telemetry.rs"]
mod telemetry;

pub(crate) use app::{
    ACCESS_TOKEN_TTL_SECONDS, AppError, AppRole, AppState, AuthContext, EnsembleRole,
    LOGIN_LINK_TTL_MINUTES, ensure_membership_entities_exist, format_timestamp,
    generate_auth_token, generate_public_token, normalize_music_icon, normalize_name,
    normalize_public_id, normalize_username, parse_quality_profile, sanitize_content_disposition,
    sanitize_filename, utc_now_string,
};

use anyhow::{Context, Result};
use config::AppConfig;
use db::{open_database_pool, run_migrations};
use services::processing;
use std::time::Duration;
use storage::Storage;
use tracing::{error, info, warn};
use uuid::Uuid;

fn main() -> Result<()> {
    let _telemetry = telemetry::init()?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to build Tokio runtime")?;
    let result = runtime.block_on(async_main());
    drop(runtime);
    result
}

async fn async_main() -> Result<()> {
    let config = AppConfig::from_env()?;
    run_migrations(&config.database_url_admin).await?;

    let db_rw = open_database_pool(&config.database_url, 5, "processor read-write").await?;
    let db_ro =
        open_database_pool(&config.database_url_read_only, 1, "processor read-only").await?;
    let storage = Storage::new(&config).await?;
    let state = AppState {
        config,
        db_rw,
        db_ro,
        storage,
    };

    let worker_id = state
        .config
        .processor_worker_id
        .clone()
        .unwrap_or_else(default_worker_id);

    info!(
        worker_id = %worker_id,
        poll_interval_ms = state.config.processor_poll_interval_ms,
        lease_seconds = state.config.processor_lease_seconds,
        heartbeat_interval_ms = state.config.processor_heartbeat_interval_ms,
        max_parallel_stem_renders = state.config.processor_max_parallel_stem_renders,
        "score processor worker started"
    );

    let poll_interval = Duration::from_millis(state.config.processor_poll_interval_ms);
    loop {
        match processing::claim_next_processing_job(
            &state.db_rw,
            &worker_id,
            state.config.processor_lease_seconds,
        )
        .await
        {
            Ok(Some(job)) => {
                if let Err(error) = run_claimed_job(&state, &worker_id, job).await {
                    error!(worker_id = %worker_id, error = ?error, "claimed processing job failed");
                }
            }
            Ok(None) => {
                tokio::time::sleep(poll_interval).await;
            }
            Err(error) => {
                warn!(
                    worker_id = %worker_id,
                    error = ?error,
                    "failed to claim processing job"
                );
                tokio::time::sleep(poll_interval).await;
            }
        }
    }
}

async fn run_claimed_job(
    state: &AppState,
    worker_id: &str,
    job: models::ProcessingJobRecord,
) -> Result<(), AppError> {
    info!(
        worker_id = %worker_id,
        music_id = %job.music_id,
        attempt = job.attempt,
        "claimed processing job"
    );

    let mut log = processing::MusicProcessingLog::new(state.clone(), job.music_id.clone());
    log.append(format!(
        "Processor worker {worker_id} claimed attempt {}.",
        job.attempt
    ))
    .await;

    let (step_sender, step_receiver) = tokio::sync::watch::channel(job.current_step.clone());
    let heartbeat_state = state.clone();
    let heartbeat_music_id = job.music_id.clone();
    let heartbeat_worker_id = worker_id.to_owned();
    let heartbeat_interval_ms = state.config.processor_heartbeat_interval_ms;
    let lease_seconds = state.config.processor_lease_seconds;
    let heartbeat_handle = tokio::spawn(async move {
        let receiver = step_receiver;
        loop {
            tokio::time::sleep(Duration::from_millis(heartbeat_interval_ms)).await;
            let step = receiver.borrow().clone();
            if let Err(error) = processing::heartbeat_processing_job(
                &heartbeat_state.db_rw,
                &heartbeat_music_id,
                &heartbeat_worker_id,
                &step,
                lease_seconds,
            )
            .await
            {
                warn!(
                    worker_id = %heartbeat_worker_id,
                    music_id = %heartbeat_music_id,
                    error = ?error,
                    "failed to refresh processing job lease"
                );
            }
        }
    });

    let result =
        processing::execute_processing_job(state, &job, &mut log, Some(&step_sender)).await;

    heartbeat_handle.abort();
    let _ = heartbeat_handle.await;

    match result {
        Ok(()) => {
            processing::mark_processing_job_completed(&state.db_rw, &job.music_id, worker_id)
                .await?;
            info!(
                worker_id = %worker_id,
                music_id = %job.music_id,
                "processing job completed"
            );
            Ok(())
        }
        Err(error) => {
            error!(
                worker_id = %worker_id,
                music_id = %job.music_id,
                error = ?error,
                "processing job execution failed"
            );
            log.append_error(format!("Processing failed: {}", error.message))
                .await;
            processing::mark_music_processing_failed(state, &job.music_id, error.message.clone())
                .await?;
            processing::mark_processing_job_failed(
                &state.db_rw,
                &job.music_id,
                worker_id,
                &error.message,
            )
            .await?;
            Ok(())
        }
    }
}

fn default_worker_id() -> String {
    let hostname = std::env::var("HOSTNAME")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "processor".to_owned());
    format!("{hostname}-{}", Uuid::new_v4().simple())
}
