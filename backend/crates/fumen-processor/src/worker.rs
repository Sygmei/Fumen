use crate::{AppError, AppState, processing};
use anyhow::Result;
use fumen_core::{
    config::AppConfig,
    db::{open_database_pool, run_migrations},
    models,
    storage::Storage,
};
use std::time::Duration;
use tracing::{error, info, warn};
use uuid::Uuid;

pub async fn run() -> Result<()> {
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
        max_parallel_core_conversions = state.config.processor_max_parallel_core_conversions,
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
    log.set_step(processing::LOG_STEP_QUEUE).await;
    log.append(format!(
        "Processor worker {worker_id} claimed attempt {}.",
        job.attempt
    ))
    .await;
    debug_marker("after-claim-log-append");

    let (step_sender, step_receiver) = tokio::sync::watch::channel(job.current_step.clone());
    debug_marker("after-step-channel");
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
    debug_marker("after-heartbeat-spawn");

    debug_marker("before-execute-processing-job");
    let result =
        processing::execute_processing_job(state, &job, &mut log, Some(&step_sender)).await;
    debug_marker("after-execute-processing-job");

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

fn debug_marker(message: &str) {
    if std::env::var("PROCESSOR_DEBUG_MARKERS")
        .ok()
        .is_some_and(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "on"))
    {
        eprintln!("[processor-debug] {message}");
    }
}
