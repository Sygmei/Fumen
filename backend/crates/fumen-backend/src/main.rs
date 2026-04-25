mod app;
mod openapi;
mod routes;
mod schemas;
mod services;
mod telemetry;

pub mod audio {
    pub use fumen_processor::audio::*;
}

pub mod config {
    pub use fumen_core::config::*;
}

pub mod db {
    pub use fumen_core::db::*;
}

pub mod models {
    pub use fumen_core::models::*;
}

pub mod processing {
    use crate::{AppError, AppState, models::MusicRecord};
    use axum::http::StatusCode;
    pub(crate) use fumen_processor::processing::{
        QueueProcessingJobRequest, build_processing_log_header, processing_statuses,
    };

    fn processor_state(state: &AppState) -> fumen_processor::AppState {
        fumen_processor::AppState {
            config: state.config.clone(),
            db_rw: state.db_rw.clone(),
            db_ro: state.db_ro.clone(),
            storage: state.storage.clone(),
        }
    }

    fn map_processor_error(error: fumen_processor::AppError) -> AppError {
        AppError::new(StatusCode::INTERNAL_SERVER_ERROR, error.message)
    }

    pub(crate) struct MusicProcessingLog {
        inner: fumen_processor::processing::MusicProcessingLog,
    }

    impl MusicProcessingLog {
        pub(crate) fn new(state: AppState, music_id: impl Into<String>) -> Self {
            Self {
                inner: fumen_processor::processing::MusicProcessingLog::new(
                    processor_state(&state),
                    music_id,
                ),
            }
        }

        pub(crate) async fn reset(&mut self, lines: &[String]) {
            self.inner.reset(lines).await;
        }

        pub(crate) async fn append(&mut self, message: impl AsRef<str>) {
            self.inner.append(message).await;
        }
    }

    pub(crate) async fn load_music_processing_log(state: &AppState, music_id: &str) -> String {
        fumen_processor::processing::load_music_processing_log(&processor_state(state), music_id)
            .await
    }

    pub(crate) async fn reset_music_processing_state(
        state: &AppState,
        record: &MusicRecord,
        log: &mut MusicProcessingLog,
    ) -> Result<(), AppError> {
        fumen_processor::processing::reset_music_processing_state(
            &processor_state(state),
            record,
            &mut log.inner,
        )
        .await
        .map_err(map_processor_error)
    }

    pub(crate) async fn enqueue_music_processing_job(
        db: &fumen_core::db::DbPool,
        request: QueueProcessingJobRequest<'_>,
    ) -> Result<(), AppError> {
        fumen_processor::processing::enqueue_music_processing_job(db, request)
            .await
            .map_err(map_processor_error)
    }
}

pub mod schema {
    pub use fumen_core::schema::*;
}

pub mod storage {
    pub use fumen_core::storage::*;
}

pub(crate) use app::{
    ACCESS_TOKEN_TTL_SECONDS, AppError, AppRole, AppState, AuthContext, EnsembleRole,
    LOGIN_LINK_TTL_MINUTES, ensure_membership_entities_exist, generate_public_token,
    normalize_music_icon, normalize_name, normalize_public_id, parse_quality_profile,
    sanitize_content_disposition,
};
pub(crate) use fumen_core::{
    generate_auth_token, normalize_username, sanitize_filename, utc_now_string,
};

use anyhow::{Context, Result};
use axum::{
    Json, Router,
    extract::DefaultBodyLimit,
    http::{
        HeaderValue,
        header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE},
    },
    response::IntoResponse,
};
use config::{AppConfig, StorageConfig};
use db::{open_database_pool, run_migrations};
use routes::api_routes;
use std::fs;
use std::{net::SocketAddr, path::PathBuf};
use storage::Storage;
use tower_http::{
    compression::CompressionLayer,
    cors::{AllowOrigin, Any, CorsLayer},
    services::{ServeDir, ServeFile},
};
use tracing::info;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;
fn main() -> Result<()> {
    if let Some(output_path) = dump_openapi_arg()? {
        dump_openapi_to_file(&output_path)?;
        return Ok(());
    }

    let _telemetry = telemetry::init()?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to build Tokio runtime")?;
    let result = runtime.block_on(async_main());
    drop(runtime);
    result
}

fn dump_openapi_arg() -> Result<Option<PathBuf>> {
    let mut args = std::env::args().skip(1);

    match args.next().as_deref() {
        None => Ok(None),
        Some("--dump-openapi") => {
            let output_path = args.next().map(PathBuf::from).context(
                "usage: cargo run --package fumen-backend -- --dump-openapi <output-path>",
            )?;

            if let Some(extra) = args.next() {
                anyhow::bail!("unexpected extra argument: {extra}");
            }

            Ok(Some(output_path))
        }
        Some(other) => anyhow::bail!("unknown argument: {other}"),
    }
}

fn dump_openapi_to_file(output_path: &PathBuf) -> Result<()> {
    let openapi = openapi::ApiDoc::openapi();
    let json = serde_json::to_string_pretty(&openapi)?;

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }

    fs::write(output_path, json)
        .with_context(|| format!("failed to write OpenAPI JSON to {}", output_path.display()))?;
    println!("wrote OpenAPI JSON to {}", output_path.display());
    Ok(())
}

async fn async_main() -> Result<()> {
    let config = AppConfig::from_env()?;
    match &config.storage {
        StorageConfig::Local { root } => {
            info!("using local storage at {}", root.display());
        }
        StorageConfig::S3(s3) => {
            info!("using s3 storage bucket {}", s3.bucket);
        }
    }

    run_migrations(&config.database_url_admin).await?;
    let db_admin = open_database_pool(&config.database_url_admin, 1, "admin").await?;
    let superadmin = fumen_core::auth::ensure_superadmin_user(&db_admin, &config).await?;
    let db_rw = open_database_pool(&config.database_url, 5, "read-write").await?;
    let db_ro = open_database_pool(&config.database_url_read_only, 5, "read-only").await?;
    let storage = Storage::new(&config).await?;
    let cors_layer = build_cors_layer(&config)?;

    let state = AppState {
        config,
        db_rw,
        db_ro,
        storage,
    };
    let mut app = Router::new()
        .nest("/api", api_routes(state.clone()))
        .merge(SwaggerUi::new("/api/docs").url("/api/openapi.json", openapi::ApiDoc::openapi()))
        .layer(DefaultBodyLimit::max(50 * 1024 * 1024))
        .layer(CompressionLayer::new())
        .layer(cors_layer);

    let frontend_dist = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../frontend/dist");
    if frontend_dist.exists() {
        app = app.fallback_service(
            ServeDir::new(&frontend_dist)
                .not_found_service(ServeFile::new(frontend_dist.join("index.html"))),
        );
    } else {
        app = app.route("/", crate::op_get!(state, "/", root_message));
    }

    let address: SocketAddr = state
        .config
        .bind_address
        .parse()
        .with_context(|| format!("invalid BIND_ADDRESS '{}'", state.config.bind_address))?;

    info!("listening on http://{}", address);
    info!(
        "superadmin account ready: {} ({})",
        superadmin.username, superadmin.id
    );
    let listener = tokio::net::TcpListener::bind(address).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

fn build_cors_layer(config: &AppConfig) -> Result<CorsLayer> {
    let base = CorsLayer::new()
        .allow_headers([AUTHORIZATION, ACCEPT, CONTENT_TYPE])
        .expose_headers([telemetry::TRACE_ID_HEADER_NAME])
        .allow_methods(Any);

    match &config.cors_allowed_origins {
        None => Ok(base.allow_origin(Any)),
        Some(list) => {
            let origins = list
                .iter()
                .map(|origin| {
                    HeaderValue::from_str(origin)
                        .with_context(|| format!("invalid CORS origin '{}'", origin))
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(base.allow_origin(AllowOrigin::list(origins)))
        }
    }
}

async fn root_message() -> impl IntoResponse {
    Json(serde_json::json!({
        "message": "Fumen backend is running. Build the frontend to serve it from this process."
    }))
}
