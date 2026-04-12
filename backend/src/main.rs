mod app;
mod audio;
mod config;
mod models;
mod openapi;
mod routes;
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
use models::UserRecord;
use routes::api_routes;
use sqlx::{
    PgPool,
    postgres::{PgConnectOptions, PgPoolOptions},
};
use std::fs;
use std::str::FromStr;
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

    let db_admin = open_database_pool(&config.database_url_admin, 1, "admin").await?;
    ensure_schema(&db_admin).await?;
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

async fn open_database_pool(url: &str, max_connections: u32, role: &str) -> Result<PgPool> {
    let options = PgConnectOptions::from_str(url)
        .with_context(|| format!("invalid PostgreSQL connection string for {role} pool"))?
        .statement_cache_capacity(0);

    Ok(PgPoolOptions::new()
        .max_connections(max_connections)
        .connect_with(options)
        .await?)
}

async fn ensure_schema(db: &PgPool) -> Result<()> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS directories (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            created_at TEXT NOT NULL
        )
        "#,
    )
    .execute(db)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS musics (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            icon TEXT,
            filename TEXT NOT NULL,
            content_type TEXT NOT NULL,
            object_key TEXT NOT NULL,
            audio_object_key TEXT,
            audio_status TEXT NOT NULL DEFAULT 'unavailable',
            audio_error TEXT,
            midi_object_key TEXT,
            midi_status TEXT NOT NULL DEFAULT 'unavailable',
            midi_error TEXT,
            stems_status TEXT NOT NULL DEFAULT 'unavailable',
            stems_error TEXT,
            public_token TEXT NOT NULL UNIQUE,
            public_id TEXT UNIQUE,
            quality_profile TEXT NOT NULL DEFAULT 'standard',
            created_at TEXT NOT NULL,
            directory_id TEXT NOT NULL
        )
        "#,
    )
    .execute(db)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS stems (
            id BIGSERIAL PRIMARY KEY,
            music_id TEXT NOT NULL REFERENCES musics(id),
            track_index BIGINT NOT NULL,
            track_name TEXT NOT NULL,
            instrument_name TEXT NOT NULL,
            storage_key TEXT NOT NULL,
            size_bytes BIGINT NOT NULL DEFAULT 0,
            drum_map_json TEXT
        )
        "#,
    )
    .execute(db)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id TEXT PRIMARY KEY,
            username TEXT NOT NULL UNIQUE,
            created_at TEXT NOT NULL,
            is_superadmin BOOLEAN NOT NULL DEFAULT FALSE,
            role TEXT NOT NULL DEFAULT 'user',
            created_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL
        )
        "#,
    )
    .execute(db)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS ensembles (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            created_at TEXT NOT NULL,
            created_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL
        )
        "#,
    )
    .execute(db)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS user_ensemble_memberships (
            user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            ensemble_id TEXT NOT NULL REFERENCES ensembles(id) ON DELETE CASCADE,
            role TEXT NOT NULL DEFAULT 'user',
            PRIMARY KEY (user_id, ensemble_id)
        )
        "#,
    )
    .execute(db)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS directory_ensemble_permissions (
            directory_id TEXT NOT NULL REFERENCES directories(id) ON DELETE CASCADE,
            ensemble_id TEXT NOT NULL REFERENCES ensembles(id) ON DELETE CASCADE,
            PRIMARY KEY (directory_id, ensemble_id)
        )
        "#,
    )
    .execute(db)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS music_ensemble_links (
            music_id TEXT NOT NULL REFERENCES musics(id) ON DELETE CASCADE,
            ensemble_id TEXT NOT NULL REFERENCES ensembles(id) ON DELETE CASCADE,
            PRIMARY KEY (music_id, ensemble_id)
        )
        "#,
    )
    .execute(db)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS user_login_links (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            token TEXT NOT NULL UNIQUE,
            created_at TEXT NOT NULL,
            expires_at TEXT NOT NULL,
            consumed_at TEXT
        )
        "#,
    )
    .execute(db)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS user_sessions (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            session_token TEXT NOT NULL UNIQUE,
            created_at TEXT NOT NULL,
            expires_at TEXT
        )
        "#,
    )
    .execute(db)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS user_music_track_playtime (
            user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            music_id TEXT NOT NULL REFERENCES musics(id) ON DELETE CASCADE,
            track_index BIGINT NOT NULL,
            total_seconds DOUBLE PRECISION NOT NULL DEFAULT 0,
            updated_at TEXT NOT NULL,
            PRIMARY KEY (user_id, music_id, track_index)
        )
        "#,
    )
    .execute(db)
    .await?;

    sqlx::query("ALTER TABLE user_sessions ALTER COLUMN expires_at DROP NOT NULL")
        .execute(db)
        .await?;

    ensure_music_column(db, "audio_object_key", "TEXT").await?;
    ensure_music_column(db, "icon", "TEXT").await?;
    ensure_music_column(db, "icon_image_key", "TEXT").await?;
    ensure_music_column(db, "audio_status", "TEXT NOT NULL DEFAULT 'unavailable'").await?;
    ensure_music_column(db, "audio_error", "TEXT").await?;
    ensure_music_column(db, "midi_object_key", "TEXT").await?;
    ensure_music_column(db, "midi_status", "TEXT NOT NULL DEFAULT 'unavailable'").await?;
    ensure_music_column(db, "midi_error", "TEXT").await?;
    ensure_music_column(db, "stems_status", "TEXT NOT NULL DEFAULT 'unavailable'").await?;
    ensure_music_column(db, "stems_error", "TEXT").await?;
    ensure_music_column(db, "musicxml_object_key", "TEXT").await?;
    ensure_music_column(db, "musicxml_status", "TEXT NOT NULL DEFAULT 'unavailable'").await?;
    ensure_music_column(db, "musicxml_error", "TEXT").await?;
    ensure_music_column(
        db,
        "quality_profile",
        &format!(
            "TEXT NOT NULL DEFAULT '{}'",
            audio::DEFAULT_STEM_QUALITY_PROFILE
        ),
    )
    .await?;
    ensure_stems_column(db, "size_bytes", "BIGINT NOT NULL DEFAULT 0").await?;
    ensure_stems_column(db, "drum_map_json", "TEXT").await?;
    ensure_music_column(db, "directory_id", "TEXT NOT NULL DEFAULT ''").await?;
    ensure_music_column(
        db,
        "owner_user_id",
        "TEXT REFERENCES users(id) ON DELETE SET NULL",
    )
    .await?;
    ensure_user_column(db, "is_superadmin", "BOOLEAN NOT NULL DEFAULT FALSE").await?;
    ensure_user_column(db, "role", "TEXT NOT NULL DEFAULT 'user'").await?;
    ensure_user_column(db, "display_name", "TEXT").await?;
    ensure_user_column(db, "avatar_image_key", "TEXT").await?;
    ensure_user_column(db, "display_name", "TEXT").await?;
    ensure_user_column(db, "avatar_image_key", "TEXT").await?;
    ensure_user_column(
        db,
        "created_by_user_id",
        "TEXT REFERENCES users(id) ON DELETE SET NULL",
    )
    .await?;
    ensure_ensemble_column(
        db,
        "created_by_user_id",
        "TEXT REFERENCES users(id) ON DELETE SET NULL",
    )
    .await?;
    ensure_user_ensemble_membership_column(db, "role", "TEXT NOT NULL DEFAULT 'user'").await?;
    sqlx::query(
        "CREATE UNIQUE INDEX IF NOT EXISTS users_single_superadmin_idx ON users (is_superadmin) WHERE is_superadmin = TRUE",
    )
    .execute(db)
    .await?;
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS user_music_track_playtime_music_idx ON user_music_track_playtime (music_id, user_id)",
    )
    .execute(db)
    .await?;
    sqlx::query("UPDATE users SET role = 'superadmin' WHERE is_superadmin = TRUE")
        .execute(db)
        .await?;
    sqlx::query(
        "UPDATE users SET role = 'manager' WHERE role = 'user' AND is_superadmin = FALSE AND EXISTS (SELECT 1 FROM user_ensemble_memberships uem WHERE uem.user_id = users.id AND uem.role = 'admin')",
    )
    .execute(db)
    .await?;
    sqlx::query(
        "UPDATE users SET role = 'user' WHERE role IS NULL OR role NOT IN ('superadmin', 'admin', 'manager', 'editor', 'user')",
    )
    .execute(db)
    .await?;
    sqlx::query("UPDATE users SET is_superadmin = (role = 'superadmin')")
        .execute(db)
        .await?;
    sqlx::query("UPDATE user_ensemble_memberships SET role = 'manager' WHERE role = 'admin'")
        .execute(db)
        .await?;
    sqlx::query(
        "UPDATE user_ensemble_memberships SET role = 'user' WHERE role IS NULL OR role NOT IN ('user', 'manager', 'editor')",
    )
    .execute(db)
    .await?;

    backfill_music_ensemble_links(db).await?;

    Ok(())
}

async fn ensure_music_column(db: &PgPool, name: &str, definition: &str) -> Result<()> {
    let query = format!("ALTER TABLE musics ADD COLUMN IF NOT EXISTS {name} {definition}");
    sqlx::query(&query).execute(db).await?;
    Ok(())
}

async fn ensure_user_column(db: &PgPool, name: &str, definition: &str) -> Result<()> {
    let query = format!("ALTER TABLE users ADD COLUMN IF NOT EXISTS {name} {definition}");
    sqlx::query(&query).execute(db).await?;
    Ok(())
}

async fn ensure_ensemble_column(db: &PgPool, name: &str, definition: &str) -> Result<()> {
    let query = format!("ALTER TABLE ensembles ADD COLUMN IF NOT EXISTS {name} {definition}");
    sqlx::query(&query).execute(db).await?;
    Ok(())
}

async fn ensure_user_ensemble_membership_column(
    db: &PgPool,
    name: &str,
    definition: &str,
) -> Result<()> {
    let query = format!(
        "ALTER TABLE user_ensemble_memberships ADD COLUMN IF NOT EXISTS {name} {definition}"
    );
    sqlx::query(&query).execute(db).await?;
    Ok(())
}

async fn ensure_stems_column(db: &PgPool, name: &str, definition: &str) -> Result<()> {
    let query = format!("ALTER TABLE stems ADD COLUMN IF NOT EXISTS {name} {definition}");
    sqlx::query(&query).execute(db).await?;
    Ok(())
}

async fn backfill_music_ensemble_links(db: &PgPool) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO music_ensemble_links (music_id, ensemble_id)
        SELECT m.id, dep.ensemble_id
        FROM musics m
        JOIN directory_ensemble_permissions dep ON dep.directory_id = m.directory_id
        ON CONFLICT DO NOTHING
        "#,
    )
    .execute(db)
    .await?;

    Ok(())
}

async fn ensure_superadmin_user(db: &PgPool, config: &AppConfig) -> Result<UserRecord> {
    if let Some(existing) = sqlx::query_as::<_, UserRecord>(
        "SELECT id, username, display_name, avatar_image_key, created_at, is_superadmin, role, created_by_user_id FROM users WHERE role = 'superadmin' OR is_superadmin = TRUE LIMIT 1",
    )
    .fetch_optional(db)
    .await?
    {
        return Ok(existing);
    }

    let base_username = normalize_username(&config.superadmin_username)
        .map_err(|error| anyhow::anyhow!(error.message))?;
    let mut username = base_username.clone();

    while sqlx::query_scalar::<_, String>("SELECT id FROM users WHERE username = $1")
        .bind(&username)
        .fetch_optional(db)
        .await?
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
        display_name: None,
        avatar_image_key: None,
        created_at: utc_now_string(),
        is_superadmin: true,
        role: AppRole::Superadmin.as_str().to_owned(),
        created_by_user_id: None,
    };

    sqlx::query(
        "INSERT INTO users (id, username, created_at, is_superadmin, role, created_by_user_id) VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(&record.id)
    .bind(&record.username)
    .bind(&record.created_at)
    .bind(record.is_superadmin)
    .bind(&record.role)
    .bind(&record.created_by_user_id)
    .execute(db)
    .await?;

    info!("created superadmin user '{}'", record.username);
    Ok(record)
}

async fn root_message() -> impl IntoResponse {
    Json(serde_json::json!({
        "message": "Fumen backend is running. Build the frontend to serve it from this process."
    }))
}
