mod app;
mod audio;
mod config;
mod db;
mod models;
mod openapi;
mod routes;
mod schema;
mod schemas;
mod services;
mod storage;
mod telemetry;

pub(crate) use app::{
    ACCESS_TOKEN_TTL_SECONDS, AppError, AppRole, AppState, AuthContext, EnsembleRole,
    LOGIN_LINK_TTL_MINUTES, ensure_membership_entities_exist, format_timestamp,
    generate_auth_token, generate_public_token, normalize_music_icon, normalize_name,
    normalize_public_id, normalize_username, parse_quality_profile, sanitize_content_disposition,
    sanitize_filename, utc_now_string,
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
use diesel::OptionalExtension;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use models::{NewUser, UserRecord};
use routes::api_routes;
use schema::users::dsl as users_dsl;
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
use uuid::Uuid;

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
            let output_path = args
                .next()
                .map(PathBuf::from)
                .context("usage: cargo run --bin fumen-backend -- --dump-openapi <output-path>")?;

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
    let superadmin = ensure_superadmin_user(&db_admin, &config).await?;
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

    let frontend_dist = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../frontend/dist");
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

async fn ensure_superadmin_user(db: &db::DbPool, config: &AppConfig) -> Result<UserRecord> {
    let mut conn = db.get().await?;
    if let Some(existing) = users_dsl::users
        .filter(
            users_dsl::role
                .eq(AppRole::Superadmin.as_str())
                .or(users_dsl::is_superadmin.eq(true)),
        )
        .select(UserRecord::as_select())
        .first(&mut conn)
        .await
        .optional()?
    {
        return Ok(existing);
    }

    let base_username = normalize_username(&config.superadmin_username)
        .map_err(|error| anyhow::anyhow!(error.message))?;
    let mut username = base_username.clone();

    while users_dsl::users
        .filter(users_dsl::username.eq(&username))
        .select(users_dsl::id)
        .first::<String>(&mut conn)
        .await
        .optional()?
        .is_some()
    {
        username = format!(
            "{base_username}-{}",
            generate_auth_token(6).to_ascii_lowercase()
        );
    }

    let record = UserRecord {
        id: Uuid::new_v4().to_string(),
        username,
        created_at: utc_now_string(),
        is_superadmin: true,
        role: AppRole::Superadmin.as_str().to_owned(),
        display_name: None,
        avatar_image_key: None,
        created_by_user_id: None,
    };

    diesel::insert_into(users_dsl::users)
        .values(&NewUser {
            id: &record.id,
            username: &record.username,
            created_at: &record.created_at,
            is_superadmin: record.is_superadmin,
            role: &record.role,
            display_name: record.display_name.as_deref(),
            avatar_image_key: record.avatar_image_key.as_deref(),
            created_by_user_id: record.created_by_user_id.as_deref(),
        })
        .execute(&mut conn)
        .await?;

    info!("created superadmin user '{}'", record.username);
    Ok(record)
}

async fn root_message() -> impl IntoResponse {
    Json(serde_json::json!({
        "message": "Fumen backend is running. Build the frontend to serve it from this process."
    }))
}
