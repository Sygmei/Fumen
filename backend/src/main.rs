mod audio;
mod config;
mod models;
mod storage;

use anyhow::{Context, Result};
use axum::{
    Json, Router,
    extract::multipart::MultipartError,
    extract::{DefaultBodyLimit, Multipart, Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, patch, post},
};
use bytes::Bytes;
use chrono::{Duration, SecondsFormat, Utc};
use config::{AppConfig, StorageConfig};
use flate2::{Compression, write::GzEncoder};
use models::{
    AdminDirectoryResponse, AdminEnsembleResponse, AdminMusicResponse, AuthSessionResponse,
    CreateDirectoryRequest, CreateEnsembleRequest, CreateUserRequest, CurrentUserResponse,
    DirectoryEnsemblePermissionRecord, DirectoryRecord, EnsembleRecord, ExchangeLoginTokenRequest,
    ExportMixerGainsRequest, LoginLinkResponse, LoginRequest, LoginResponse, MoveMusicRequest,
    MusicRecord, PublicMusicResponse, StemInfo, StemRecord, UpdateMusicRequest, UserLibraryResponse,
    UserLibraryDirectoryResponse, UserLibraryScoreResponse, UserEnsembleMembershipRecord,
    UserRecord, UserResponse, UserSessionRecord,
};
use rand::{Rng, distr::Alphanumeric};
use sqlx::{
    PgPool,
    postgres::{PgConnectOptions, PgPoolOptions},
};
use std::io::Write;
use std::str::FromStr;
use std::{net::SocketAddr, path::PathBuf};
use storage::Storage;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::process::Command;
use tower_http::{
    compression::CompressionLayer,
    cors::{AllowOrigin, Any, CorsLayer},
    services::{ServeDir, ServeFile},
};
use tracing::{info, warn};
use uuid::Uuid;

const LOGIN_LINK_TTL_MINUTES: i64 = 5;
const USER_SESSION_TTL_DAYS: i64 = 30;
const DEFAULT_DIRECTORY_ID: &str = "general";
const DEFAULT_DIRECTORY_NAME: &str = "General";

#[derive(Clone)]
struct AppState {
    config: AppConfig,
    db_rw: PgPool,
    db_ro: PgPool,
    storage: Storage,
}

#[derive(Debug)]
struct AppError {
    status: StatusCode,
    message: String,
}

impl AppError {
    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, message)
    }

    fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, message)
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, message)
    }

    fn conflict(message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, message)
    }
}

impl From<anyhow::Error> for AppError {
    fn from(error: anyhow::Error) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
    }
}

impl From<sqlx::Error> for AppError {
    fn from(error: sqlx::Error) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
    }
}

impl From<MultipartError> for AppError {
    fn from(error: MultipartError) -> Self {
        Self::new(StatusCode::BAD_REQUEST, error.to_string())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(serde_json::json!({ "error": self.message })),
        )
            .into_response()
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "fumen_backend=info,tower_http=info".to_owned()),
        )
        .init();

    let config = AppConfig::from_env()?;
    let cors_layer = build_cors_layer(&config)?;
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
    let db_rw = open_database_pool(&config.database_url, 5, "read-write").await?;
    let db_ro = open_database_pool(&config.database_url_read_only, 5, "read-only").await?;
    let storage = Storage::new(&config).await?;

    let state = AppState {
        config,
        db_rw,
        db_ro,
        storage,
    };
    let api_routes = Router::new()
        .route("/health", get(health))
        .route("/admin/login", post(admin_login))
        .route("/admin/users", get(admin_list_users).post(admin_create_user))
        .route(
            "/admin/users/{id}/login-link",
            post(admin_create_user_login_link),
        )
        .route(
            "/admin/ensembles",
            get(admin_list_ensembles).post(admin_create_ensemble),
        )
        .route(
            "/admin/ensembles/{id}/users/{user_id}",
            post(admin_add_user_to_ensemble).delete(admin_remove_user_from_ensemble),
        )
        .route(
            "/admin/directories",
            get(admin_list_directories).post(admin_create_directory),
        )
        .route(
            "/admin/directories/{id}/ensembles/{ensemble_id}",
            post(admin_grant_directory_to_ensemble).delete(admin_revoke_directory_from_ensemble),
        )
        .route(
            "/admin/musics",
            get(admin_list_musics).post(admin_upload_music),
        )
        .route("/admin/musics/{id}", patch(admin_update_music))
        .route("/admin/musics/{id}/move", post(admin_move_music))
        .route("/admin/musics/{id}/delete", post(admin_delete_music))
        .route("/admin/musics/{id}/gains", get(admin_export_score_gains))
        .route(
            "/admin/public/{access_key}/gains",
            get(admin_export_public_score_gains).post(admin_export_public_mixer_gains),
        )
        .route("/admin/musics/{id}/retry", post(admin_retry_render))
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
        .route("/auth/exchange", post(exchange_login_token))
        .route("/me", get(current_user))
        .route("/me/library", get(current_user_library))
        .route("/me/login-link", post(create_my_login_link))
        .with_state(state.clone());

    let mut app = Router::new()
        .nest("/api", api_routes)
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
        app = app.route("/", get(root_message));
    }

    let address: SocketAddr = state
        .config
        .bind_address
        .parse()
        .with_context(|| format!("invalid BIND_ADDRESS '{}'", state.config.bind_address))?;

    info!("listening on http://{}", address);
    let listener = tokio::net::TcpListener::bind(address).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

fn build_cors_layer(config: &AppConfig) -> Result<CorsLayer> {
    let origins = config
        .cors_allowed_origins
        .iter()
        .map(|origin| {
            HeaderValue::from_str(origin)
                .with_context(|| format!("invalid CORS origin '{}'", origin))
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_headers(Any)
        .allow_methods(Any))
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
            directory_id TEXT NOT NULL DEFAULT 'general'
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
            created_at TEXT NOT NULL
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
            created_at TEXT NOT NULL
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
            expires_at TEXT NOT NULL
        )
        "#,
    )
    .execute(db)
    .await?;

    ensure_music_column(db, "audio_object_key", "TEXT").await?;
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
    ensure_music_column(
        db,
        "directory_id",
        &format!("TEXT NOT NULL DEFAULT '{}'", DEFAULT_DIRECTORY_ID),
    )
    .await?;
    ensure_stems_column(db, "size_bytes", "BIGINT NOT NULL DEFAULT 0").await?;
    ensure_stems_column(db, "drum_map_json", "TEXT").await?;

    ensure_default_directory(db).await?;
    sqlx::query("UPDATE musics SET directory_id = $1 WHERE directory_id IS NULL OR directory_id = ''")
        .bind(DEFAULT_DIRECTORY_ID)
        .execute(db)
        .await?;

    Ok(())
}

async fn ensure_music_column(db: &PgPool, name: &str, definition: &str) -> Result<()> {
    let query = format!("ALTER TABLE musics ADD COLUMN IF NOT EXISTS {name} {definition}");
    sqlx::query(&query).execute(db).await?;
    Ok(())
}

async fn ensure_stems_column(db: &PgPool, name: &str, definition: &str) -> Result<()> {
    let query = format!("ALTER TABLE stems ADD COLUMN IF NOT EXISTS {name} {definition}");
    sqlx::query(&query).execute(db).await?;
    Ok(())
}

async fn ensure_default_directory(db: &PgPool) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO directories (id, name, created_at)
        VALUES ($1, $2, $3)
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .bind(DEFAULT_DIRECTORY_ID)
    .bind(DEFAULT_DIRECTORY_NAME)
    .bind(utc_now_string())
    .execute(db)
    .await?;
    Ok(())
}

async fn root_message() -> impl IntoResponse {
    Json(serde_json::json!({
        "message": "Fumen backend is running. Build the frontend to serve it from this process."
    }))
}

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({ "ok": true }))
}

async fn admin_login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, AppError> {
    if payload.password != state.config.admin_password {
        return Err(AppError::unauthorized("Invalid admin password"));
    }

    Ok(Json(LoginResponse { ok: true }))
}

async fn admin_list_users(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<UserResponse>>, AppError> {
    require_admin(&headers, &state.config)?;

    let rows = sqlx::query_as::<_, UserRecord>(
        "SELECT id, username, created_at FROM users ORDER BY username ASC",
    )
    .fetch_all(&state.db_rw)
    .await?;

    Ok(Json(rows.into_iter().map(user_record_to_response).collect()))
}

async fn admin_create_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateUserRequest>,
) -> Result<Json<UserResponse>, AppError> {
    require_admin(&headers, &state.config)?;

    let username = normalize_username(&payload.username)?;
    if find_user_by_username(&state.db_rw, &username).await?.is_some() {
        return Err(AppError::conflict("That username already exists"));
    }

    let record = UserRecord {
        id: Uuid::new_v4().to_string(),
        username,
        created_at: utc_now_string(),
    };

    sqlx::query("INSERT INTO users (id, username, created_at) VALUES ($1, $2, $3)")
        .bind(&record.id)
        .bind(&record.username)
        .bind(&record.created_at)
        .execute(&state.db_rw)
        .await?;

    Ok(Json(user_record_to_response(record)))
}

async fn admin_create_user_login_link(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<LoginLinkResponse>, AppError> {
    require_admin(&headers, &state.config)?;

    let user = find_user_by_id(&state.db_rw, &id)
        .await?
        .ok_or_else(|| AppError::not_found("User not found"))?;

    Ok(Json(create_login_link_response(&state, &user.id).await?))
}

async fn admin_list_ensembles(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<AdminEnsembleResponse>>, AppError> {
    require_admin(&headers, &state.config)?;

    let ensembles = sqlx::query_as::<_, EnsembleRecord>(
        "SELECT id, name, created_at FROM ensembles ORDER BY name ASC",
    )
    .fetch_all(&state.db_rw)
    .await?;
    let memberships = fetch_user_ensemble_memberships(&state.db_rw).await?;
    let mut member_map: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    for membership in memberships {
        member_map
            .entry(membership.ensemble_id)
            .or_default()
            .push(membership.user_id);
    }

    Ok(Json(
        ensembles
            .into_iter()
            .map(|ensemble| AdminEnsembleResponse {
                id: ensemble.id.clone(),
                name: ensemble.name,
                created_at: ensemble.created_at,
                member_user_ids: member_map.remove(&ensemble.id).unwrap_or_default(),
            })
            .collect(),
    ))
}

async fn admin_create_ensemble(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateEnsembleRequest>,
) -> Result<Json<AdminEnsembleResponse>, AppError> {
    require_admin(&headers, &state.config)?;

    let name = normalize_name(&payload.name, "Ensemble names", 2, 64)?;
    if find_ensemble_by_name(&state.db_rw, &name).await?.is_some() {
        return Err(AppError::conflict("That ensemble already exists"));
    }

    let record = EnsembleRecord {
        id: Uuid::new_v4().to_string(),
        name,
        created_at: utc_now_string(),
    };

    sqlx::query("INSERT INTO ensembles (id, name, created_at) VALUES ($1, $2, $3)")
        .bind(&record.id)
        .bind(&record.name)
        .bind(&record.created_at)
        .execute(&state.db_rw)
        .await?;

    Ok(Json(AdminEnsembleResponse {
        id: record.id,
        name: record.name,
        created_at: record.created_at,
        member_user_ids: Vec::new(),
    }))
}

async fn admin_add_user_to_ensemble(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((id, user_id)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    require_admin(&headers, &state.config)?;
    ensure_membership_entities_exist(&state.db_rw, &id, &user_id).await?;

    sqlx::query(
        "INSERT INTO user_ensemble_memberships (user_id, ensemble_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(&user_id)
    .bind(&id)
    .execute(&state.db_rw)
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

async fn admin_remove_user_from_ensemble(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((id, user_id)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    require_admin(&headers, &state.config)?;

    sqlx::query("DELETE FROM user_ensemble_memberships WHERE user_id = $1 AND ensemble_id = $2")
        .bind(&user_id)
        .bind(&id)
        .execute(&state.db_rw)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

async fn admin_list_directories(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<AdminDirectoryResponse>>, AppError> {
    require_admin(&headers, &state.config)?;

    let directories = sqlx::query_as::<_, DirectoryRecord>(
        "SELECT id, name, created_at FROM directories ORDER BY name ASC",
    )
    .fetch_all(&state.db_rw)
    .await?;
    let permissions = fetch_directory_permissions(&state.db_rw).await?;
    let counts = sqlx::query_as::<_, DirectoryScoreCountRow>(
        "SELECT directory_id, COUNT(*)::BIGINT AS score_count FROM musics GROUP BY directory_id",
    )
    .fetch_all(&state.db_rw)
    .await?;

    let mut permission_map: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    for permission in permissions {
        permission_map
            .entry(permission.directory_id)
            .or_default()
            .push(permission.ensemble_id);
    }
    let mut count_map: std::collections::HashMap<String, i64> = counts
        .into_iter()
        .map(|count| (count.directory_id, count.score_count))
        .collect();

    Ok(Json(
        directories
            .into_iter()
            .map(|directory| AdminDirectoryResponse {
                id: directory.id.clone(),
                name: directory.name,
                created_at: directory.created_at,
                permitted_ensemble_ids: permission_map.remove(&directory.id).unwrap_or_default(),
                score_count: count_map.remove(&directory.id).unwrap_or(0),
            })
            .collect(),
    ))
}

async fn admin_create_directory(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateDirectoryRequest>,
) -> Result<Json<AdminDirectoryResponse>, AppError> {
    require_admin(&headers, &state.config)?;

    let name = normalize_name(&payload.name, "Directory names", 2, 64)?;
    if find_directory_by_name(&state.db_rw, &name).await?.is_some() {
        return Err(AppError::conflict("That directory already exists"));
    }

    let record = DirectoryRecord {
        id: slugify_name(&name),
        name,
        created_at: utc_now_string(),
    };
    if find_directory_by_id(&state.db_rw, &record.id).await?.is_some() {
        return Err(AppError::conflict("A similar directory id already exists"));
    }

    sqlx::query("INSERT INTO directories (id, name, created_at) VALUES ($1, $2, $3)")
        .bind(&record.id)
        .bind(&record.name)
        .bind(&record.created_at)
        .execute(&state.db_rw)
        .await?;

    Ok(Json(AdminDirectoryResponse {
        id: record.id,
        name: record.name,
        created_at: record.created_at,
        permitted_ensemble_ids: Vec::new(),
        score_count: 0,
    }))
}

async fn admin_grant_directory_to_ensemble(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((id, ensemble_id)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    require_admin(&headers, &state.config)?;
    ensure_directory_and_ensemble_exist(&state.db_rw, &id, &ensemble_id).await?;

    sqlx::query(
        "INSERT INTO directory_ensemble_permissions (directory_id, ensemble_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(&id)
    .bind(&ensemble_id)
    .execute(&state.db_rw)
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

async fn admin_revoke_directory_from_ensemble(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((id, ensemble_id)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    require_admin(&headers, &state.config)?;

    sqlx::query(
        "DELETE FROM directory_ensemble_permissions WHERE directory_id = $1 AND ensemble_id = $2",
    )
    .bind(&id)
    .bind(&ensemble_id)
    .execute(&state.db_rw)
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

#[derive(sqlx::FromRow)]
struct StemsTotalRow {
    music_id: String,
    total_bytes: i64,
}

#[derive(sqlx::FromRow)]
struct DirectoryScoreCountRow {
    directory_id: String,
    score_count: i64,
}

async fn fetch_stems_total(db: &PgPool, music_id: &str) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE(SUM(size_bytes), 0)::BIGINT FROM stems WHERE music_id = $1",
    )
    .bind(music_id)
    .fetch_one(db)
    .await
    .unwrap_or(0)
}

async fn find_directory_by_id(db: &PgPool, id: &str) -> Result<Option<DirectoryRecord>, AppError> {
    Ok(sqlx::query_as::<_, DirectoryRecord>(
        "SELECT id, name, created_at FROM directories WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(db)
    .await?)
}

async fn find_directory_by_name(
    db: &PgPool,
    name: &str,
) -> Result<Option<DirectoryRecord>, AppError> {
    Ok(sqlx::query_as::<_, DirectoryRecord>(
        "SELECT id, name, created_at FROM directories WHERE name = $1",
    )
    .bind(name)
    .fetch_optional(db)
    .await?)
}

async fn find_ensemble_by_id(db: &PgPool, id: &str) -> Result<Option<EnsembleRecord>, AppError> {
    Ok(sqlx::query_as::<_, EnsembleRecord>(
        "SELECT id, name, created_at FROM ensembles WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(db)
    .await?)
}

async fn find_ensemble_by_name(db: &PgPool, name: &str) -> Result<Option<EnsembleRecord>, AppError> {
    Ok(sqlx::query_as::<_, EnsembleRecord>(
        "SELECT id, name, created_at FROM ensembles WHERE name = $1",
    )
    .bind(name)
    .fetch_optional(db)
    .await?)
}

async fn fetch_user_ensemble_memberships(
    db: &PgPool,
) -> Result<Vec<UserEnsembleMembershipRecord>, AppError> {
    Ok(sqlx::query_as::<_, UserEnsembleMembershipRecord>(
        "SELECT user_id, ensemble_id FROM user_ensemble_memberships",
    )
    .fetch_all(db)
    .await?)
}

async fn fetch_directory_permissions(
    db: &PgPool,
) -> Result<Vec<DirectoryEnsemblePermissionRecord>, AppError> {
    Ok(sqlx::query_as::<_, DirectoryEnsemblePermissionRecord>(
        "SELECT directory_id, ensemble_id FROM directory_ensemble_permissions",
    )
    .fetch_all(db)
    .await?)
}

async fn find_user_by_id(db: &PgPool, id: &str) -> Result<Option<UserRecord>, AppError> {
    Ok(sqlx::query_as::<_, UserRecord>(
        "SELECT id, username, created_at FROM users WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(db)
    .await?)
}

async fn find_user_by_username(
    db: &PgPool,
    username: &str,
) -> Result<Option<UserRecord>, AppError> {
    Ok(sqlx::query_as::<_, UserRecord>(
        "SELECT id, username, created_at FROM users WHERE username = $1",
    )
    .bind(username)
    .fetch_optional(db)
    .await?)
}

async fn find_session_by_token(
    db: &PgPool,
    session_token: &str,
) -> Result<Option<UserSessionRecord>, AppError> {
    Ok(sqlx::query_as::<_, UserSessionRecord>(
        "SELECT id, user_id, session_token, created_at, expires_at FROM user_sessions WHERE session_token = $1",
    )
    .bind(session_token)
    .fetch_optional(db)
    .await?)
}

async fn require_user_session(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<(UserRecord, UserSessionRecord), AppError> {
    let Some(header_value) = headers.get(header::AUTHORIZATION) else {
        return Err(AppError::unauthorized("Missing Authorization header"));
    };

    let authorization = header_value
        .to_str()
        .map_err(|_| AppError::unauthorized("Invalid Authorization header"))?;
    let session_token = authorization
        .strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::unauthorized("Expected a Bearer token"))?;

    let session = find_session_by_token(&state.db_rw, session_token)
        .await?
        .ok_or_else(|| AppError::unauthorized("Unknown session"))?;

    if session.expires_at <= utc_now_string() {
        return Err(AppError::unauthorized("Session expired"));
    }

    let user = find_user_by_id(&state.db_rw, &session.user_id)
        .await?
        .ok_or_else(|| AppError::unauthorized("User not found"))?;

    Ok((user, session))
}

async fn create_login_link_response(
    state: &AppState,
    user_id: &str,
) -> Result<LoginLinkResponse, AppError> {
    let now = Utc::now();
    let created_at = format_timestamp(now);
    let expires_at = format_timestamp(now + Duration::minutes(LOGIN_LINK_TTL_MINUTES));
    let token = generate_auth_token(48);

    sqlx::query(
        r#"
        INSERT INTO user_login_links (id, user_id, token, created_at, expires_at)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(user_id)
    .bind(&token)
    .bind(&created_at)
    .bind(&expires_at)
    .execute(&state.db_rw)
    .await?;

    Ok(LoginLinkResponse {
        connection_url: state.config.connection_url_for(&token),
        expires_at,
    })
}

async fn find_public_music_record(
    state: &AppState,
    access_key: &str,
) -> Result<Option<MusicRecord>, AppError> {
    if let Some(record) = find_music_by_access_key(&state.db_ro, access_key).await? {
        return Ok(Some(record));
    }

    Ok(find_music_by_access_key(&state.db_rw, access_key).await?)
}

async fn find_accessible_music_for_user(
    db: &PgPool,
    user_id: &str,
) -> Result<Vec<MusicRecord>, AppError> {
    Ok(sqlx::query_as::<_, MusicRecord>(
        r#"
        SELECT DISTINCT m.id, m.title, m.filename, m.content_type, m.object_key, m.audio_object_key, m.audio_status, m.audio_error, m.midi_object_key, m.midi_status, m.midi_error, m.musicxml_object_key, m.musicxml_status, m.musicxml_error, m.stems_status, m.stems_error, m.public_token, m.public_id, m.quality_profile, m.created_at, m.directory_id, COALESCE(d.name, 'General') AS directory_name
        FROM musics m
        JOIN directories d ON d.id = m.directory_id
        JOIN directory_ensemble_permissions dep ON dep.directory_id = d.id
        JOIN user_ensemble_memberships uem ON uem.ensemble_id = dep.ensemble_id
        WHERE uem.user_id = $1
        ORDER BY d.name ASC, m.title ASC
        "#,
    )
    .bind(user_id)
    .fetch_all(db)
    .await?)
}

async fn find_public_stems(
    db_primary: &PgPool,
    db_fallback: &PgPool,
    music_id: &str,
) -> Result<Vec<StemRecord>, AppError> {
    let query = "SELECT id, music_id, track_index, track_name, instrument_name, storage_key, drum_map_json \
         FROM stems WHERE music_id = $1 ORDER BY track_index";

    let stems = sqlx::query_as::<_, StemRecord>(query)
        .bind(music_id)
        .fetch_all(db_primary)
        .await?;

    if !stems.is_empty() {
        return Ok(stems);
    }

    Ok(sqlx::query_as::<_, StemRecord>(query)
        .bind(music_id)
        .fetch_all(db_fallback)
        .await?)
}

async fn find_public_stem(
    db_primary: &PgPool,
    db_fallback: &PgPool,
    music_id: &str,
    track_index: i64,
) -> Result<Option<StemRecord>, AppError> {
    let query = "SELECT id, music_id, track_index, track_name, instrument_name, storage_key, drum_map_json \
         FROM stems WHERE music_id = $1 AND track_index = $2";

    if let Some(stem) = sqlx::query_as::<_, StemRecord>(query)
        .bind(music_id)
        .bind(track_index)
        .fetch_optional(db_primary)
        .await?
    {
        return Ok(Some(stem));
    }

    Ok(sqlx::query_as::<_, StemRecord>(query)
        .bind(music_id)
        .bind(track_index)
        .fetch_optional(db_fallback)
        .await?)
}

async fn admin_list_musics(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<AdminMusicResponse>>, AppError> {
    require_admin(&headers, &state.config)?;

    let rows = sqlx::query_as::<_, MusicRecord>(
        r#"
        SELECT m.id, m.title, m.filename, m.content_type, m.object_key, m.audio_object_key, m.audio_status, m.audio_error, m.midi_object_key, m.midi_status, m.midi_error, m.musicxml_object_key, m.musicxml_status, m.musicxml_error, m.stems_status, m.stems_error, m.public_token, m.public_id, m.quality_profile, m.created_at, m.directory_id, COALESCE(d.name, 'General') AS directory_name
        FROM musics m
        LEFT JOIN directories d ON d.id = m.directory_id
        ORDER BY m.created_at DESC
        "#,
    )
    .fetch_all(&state.db_rw)
    .await?;

    // Fetch per-music stem totals in one query
    let total_rows = sqlx::query_as::<_, StemsTotalRow>(
        "SELECT music_id, COALESCE(SUM(size_bytes), 0)::BIGINT AS total_bytes FROM stems GROUP BY music_id",
    )
    .fetch_all(&state.db_rw)
    .await?;
    let totals: std::collections::HashMap<String, i64> = total_rows
        .into_iter()
        .map(|r| (r.music_id, r.total_bytes))
        .collect();

    let items = rows
        .into_iter()
        .map(|record| {
            let total = totals.get(&record.id).copied().unwrap_or(0);
            record_to_admin_response(&state.config, &state.storage, record, total)
        })
        .collect();

    Ok(Json(items))
}

async fn admin_upload_music(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Json<AdminMusicResponse>, AppError> {
    require_admin(&headers, &state.config)?;

    let mut title: Option<String> = None;
    let mut requested_public_id: Option<String> = None;
    let mut requested_quality_profile: Option<String> = None;
    let mut requested_directory_id: Option<String> = None;
    let mut upload: Option<(String, String, Bytes)> = None;

    while let Some(field) = multipart.next_field().await? {
        match field.name() {
            Some("title") => {
                title = Some(field.text().await?.trim().to_owned());
            }
            Some("public_id") => {
                requested_public_id = Some(field.text().await?.trim().to_owned());
            }
            Some("quality_profile") => {
                requested_quality_profile = Some(field.text().await?.trim().to_owned());
            }
            Some("directory_id") => {
                requested_directory_id = Some(field.text().await?.trim().to_owned());
            }
            Some("file") => {
                let filename = field.file_name().map(ToOwned::to_owned).ok_or_else(|| {
                    AppError::bad_request("The uploaded file is missing a filename")
                })?;
                let content_type = field
                    .content_type()
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| "application/octet-stream".to_owned());
                upload = Some((filename, content_type, field.bytes().await?));
            }
            _ => {}
        }
    }

    let (filename, content_type, bytes) =
        upload.ok_or_else(|| AppError::bad_request("Please attach an .mscz file"))?;

    if !filename.to_lowercase().ends_with(".mscz") {
        return Err(AppError::bad_request("Only .mscz uploads are supported"));
    }

    let public_id = normalize_public_id(requested_public_id.as_deref())?;
    ensure_public_id_available(&state.db_rw, public_id.as_deref(), None).await?;
    let quality_profile = parse_quality_profile(requested_quality_profile.as_deref())?;
    let directory_id = requested_directory_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::bad_request("Choose a directory for this score"))?
        .to_owned();
    ensure_directory_exists(&state.db_rw, &directory_id).await?;

    let music_id = Uuid::new_v4().to_string();
    let public_token = generate_public_token();
    let resolved_title = title
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| filename.trim_end_matches(".mscz").to_owned());
    let safe_filename = sanitize_filename(&filename);
    let object_key = format!("scores/{music_id}/{safe_filename}");

    state
        .storage
        .upload_bytes(&object_key, bytes.clone(), &content_type)
        .await?;

    let temp_dir = tempfile::tempdir()?;
    let temp_input_path = temp_dir.path().join(&safe_filename);
    fs::write(&temp_input_path, &bytes).await?;

    // Pipeline:
    //   t=0      → MIDI export and MusicXML export run in parallel (both MuseScore passes)
    //   t=T_midi → stems render (parallel internally, reuse preview.mid)
    //   t=T_midi+T_stems → three-way parallel:
    //                      • upload MIDI
    //                      • upload MusicXML
    //                      • upload stem assets
    let (midi_outcome, musicxml_outcome) = tokio::try_join!(
        async {
            audio::generate_midi(&state.config, &temp_input_path, temp_dir.path())
                .await
                .map_err(AppError::from)
        },
        async {
            audio::generate_musicxml(&state.config, &temp_input_path, temp_dir.path())
                .await
                .map_err(AppError::from)
        },
    )?;

    let (stem_results, stems_status, stems_error) = audio::generate_stems(
        &state.config,
        &temp_input_path,
        temp_dir.path(),
        quality_profile,
    )
    .await?;

    // Insert the musics row BEFORE running store_stems so the FK constraint is satisfied.
    // Conversion-result columns have DEFAULT values and will be updated below.
    let created_at = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO musics (id, title, filename, content_type, object_key, public_token, public_id, quality_profile, created_at, directory_id)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        "#,
    )
    .bind(&music_id)
    .bind(&resolved_title)
    .bind(&filename)
    .bind(&content_type)
    .bind(&object_key)
    .bind(&public_token)
    .bind(&public_id)
    .bind(quality_profile.as_str())
    .bind(&created_at)
    .bind(&directory_id)
    .execute(&state.db_rw)
    .await?;

    let (
        (midi_object_key, midi_status, midi_error),
        (musicxml_object_key, musicxml_status, musicxml_error),
        (stems_status, stems_error),
    ) = tokio::try_join!(
        store_conversion(&state, &music_id, "midi", midi_outcome),
        store_conversion(&state, &music_id, "musicxml", musicxml_outcome),
        store_stems(&state, &music_id, stem_results, stems_status, stems_error),
    )?;

    let audio_object_key = None;
    let audio_status = "disabled".to_owned();
    let audio_error = None;

    // Update conversion results onto the row we just inserted.
    sqlx::query(
        r#"
        UPDATE musics SET
            audio_object_key   = $1, audio_status   = $2, audio_error   = $3,
            midi_object_key    = $4, midi_status    = $5, midi_error    = $6,
            musicxml_object_key = $7, musicxml_status = $8, musicxml_error = $9,
            stems_status       = $10, stems_error    = $11
        WHERE id = $12
        "#,
    )
    .bind(&audio_object_key)
    .bind(&audio_status)
    .bind(&audio_error)
    .bind(&midi_object_key)
    .bind(&midi_status)
    .bind(&midi_error)
    .bind(&musicxml_object_key)
    .bind(&musicxml_status)
    .bind(&musicxml_error)
    .bind(&stems_status)
    .bind(&stems_error)
    .bind(&music_id)
    .execute(&state.db_rw)
    .await?;

    let record = MusicRecord {
        id: music_id,
        title: resolved_title,
        filename,
        content_type,
        object_key,
        audio_object_key,
        audio_status,
        audio_error,
        midi_object_key,
        midi_status,
        midi_error,
        musicxml_object_key,
        musicxml_status,
        musicxml_error,
        stems_status,
        stems_error,
        public_token,
        public_id,
        quality_profile: quality_profile.as_str().to_owned(),
        created_at,
        directory_id: directory_id.clone(),
        directory_name: find_directory_by_id(&state.db_rw, &directory_id)
            .await?
            .map(|directory| directory.name)
            .unwrap_or_else(|| DEFAULT_DIRECTORY_NAME.to_owned()),
    };

    let stems_total = fetch_stems_total(&state.db_rw, &record.id).await;
    Ok(Json(record_to_admin_response(
        &state.config,
        &state.storage,
        record,
        stems_total,
    )))
}

async fn admin_retry_render(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<AdminMusicResponse>, AppError> {
    require_admin(&headers, &state.config)?;

    let record = find_music_by_id(&state.db_rw, &id)
        .await?
        .ok_or_else(|| AppError::not_found("Music not found"))?;
    let quality_profile =
        audio::StemQualityProfile::from_stored_or_default(&record.quality_profile);

    // Fetch the original score bytes from storage.
    let (score_bytes, _, _) = state.storage.get_bytes(&record.object_key).await?;

    let safe_filename = sanitize_filename(&record.filename);
    let temp_dir = tempfile::tempdir()?;
    let temp_input_path = temp_dir.path().join(&safe_filename);
    fs::write(&temp_input_path, &score_bytes).await?;

    // Re-run MIDI and MusicXML exports in parallel.
    let (midi_outcome, musicxml_outcome) = tokio::try_join!(
        async {
            audio::generate_midi(&state.config, &temp_input_path, temp_dir.path())
                .await
                .map_err(AppError::from)
        },
        async {
            audio::generate_musicxml(&state.config, &temp_input_path, temp_dir.path())
                .await
                .map_err(AppError::from)
        },
    )?;
    let (midi_object_key, midi_status, midi_error) =
        store_conversion(&state, &id, "midi", midi_outcome).await?;
    let (musicxml_object_key, musicxml_status, musicxml_error) =
        store_conversion(&state, &id, "musicxml", musicxml_outcome).await?;

    // Delete old stems then re-render.
    sqlx::query("DELETE FROM stems WHERE music_id = $1")
        .bind(&id)
        .execute(&state.db_rw)
        .await?;

    let (stem_results, stems_status, stems_error) = audio::generate_stems(
        &state.config,
        &temp_input_path,
        temp_dir.path(),
        quality_profile,
    )
    .await?;

    let (stems_status, stems_error) =
        store_stems(&state, &id, stem_results, stems_status, stems_error).await?;

    sqlx::query(
        "UPDATE musics SET \
         audio_object_key = NULL, audio_status = 'disabled', audio_error = NULL, \
         midi_object_key = $1, midi_status = $2, midi_error = $3, \
         musicxml_object_key = $4, musicxml_status = $5, musicxml_error = $6, \
         stems_status = $7, stems_error = $8 WHERE id = $9",
    )
    .bind(&midi_object_key)
    .bind(&midi_status)
    .bind(&midi_error)
    .bind(&musicxml_object_key)
    .bind(&musicxml_status)
    .bind(&musicxml_error)
    .bind(&stems_status)
    .bind(&stems_error)
    .bind(&id)
    .execute(&state.db_rw)
    .await?;

    let updated = find_music_by_id(&state.db_rw, &id)
        .await?
        .ok_or_else(|| AppError::not_found("Music not found"))?;

    let stems_total = fetch_stems_total(&state.db_rw, &id).await;
    Ok(Json(record_to_admin_response(
        &state.config,
        &state.storage,
        updated,
        stems_total,
    )))
}

async fn store_stems(
    state: &AppState,
    music_id: &str,
    stems: Vec<audio::StemResult>,
    status: String,
    error: Option<String>,
) -> Result<(String, Option<String>), AppError> {
    for stem in stems {
        let size_bytes = stem.bytes.len() as i64;
        let storage_key = stem_full_key(music_id, stem.track_index);
        let drum_map_json = stem
            .drum_map
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| AppError::from(anyhow::Error::from(error)))?;
        state
            .storage
            .upload_bytes(&storage_key, stem.bytes.clone(), "audio/ogg")
            .await?;

        sqlx::query(
            "INSERT INTO stems (music_id, track_index, track_name, instrument_name, storage_key, size_bytes, drum_map_json) \
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(music_id)
        .bind(stem.track_index as i64)
        .bind(&stem.track_name)
        .bind(&stem.instrument_name)
        .bind(&storage_key)
        .bind(size_bytes)
        .bind(&drum_map_json)
        .execute(&state.db_rw)
        .await?;
    }
    Ok((status, error))
}

async fn probe_audio_duration_seconds(path: &std::path::Path) -> Result<f64, AppError> {
    let output = Command::new("ffprobe")
        .arg("-v")
        .arg("error")
        .arg("-show_entries")
        .arg("format=duration")
        .arg("-of")
        .arg("default=noprint_wrappers=1:nokey=1")
        .arg(path)
        .output()
        .await
        .map_err(AppError::from)?;

    if !output.status.success() {
        return Err(AppError::from(anyhow::anyhow!(
            "ffprobe failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    let duration = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<f64>()
        .map_err(|error| AppError::from(anyhow::anyhow!("invalid ffprobe duration: {error}")))?;
    Ok(duration)
}

async fn public_music_stems(
    State(state): State<AppState>,
    Path(access_key): Path<String>,
) -> Result<Json<Vec<StemInfo>>, AppError> {
    let record = find_public_music_record(&state, &access_key)
        .await?
        .ok_or_else(|| AppError::not_found("Music not found"))?;

    let stems = find_public_stems(&state.db_ro, &state.db_rw, &record.id).await?;

    let mut resolved_infos = Vec::new();
    for stem in stems {
        let full_stem_url = state
            .storage
            .public_url(&stem.storage_key)
            .unwrap_or_else(|| format!("/api/public/{}/stems/{}", access_key, stem.track_index));
        let duration_seconds =
            if let Some(path) = state.storage.local_path_for_key(&stem.storage_key) {
                probe_audio_duration_seconds(&path).await?
            } else {
                let (stem_bytes, _, _) = state.storage.get_bytes(&stem.storage_key).await?;
                let temp_dir = tempfile::tempdir()?;
                let full_stem_path = temp_dir.path().join("stem.ogg");
                fs::write(&full_stem_path, stem_bytes).await?;
                probe_audio_duration_seconds(&full_stem_path).await?
            };

        resolved_infos.push(StemInfo {
            track_index: stem.track_index,
            track_name: stem.track_name,
            instrument_name: stem.instrument_name,
            full_stem_url,
            duration_seconds,
            drum_map: stem
                .drum_map_json
                .as_deref()
                .map(serde_json::from_str)
                .transpose()
                .map_err(|error| AppError::from(anyhow::Error::from(error)))?,
        });
    }

    Ok(Json(resolved_infos))
}

async fn public_music_stem_audio(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((access_key, track_index)): Path<(String, i64)>,
) -> Result<Response, AppError> {
    let record = find_public_music_record(&state, &access_key)
        .await?
        .ok_or_else(|| AppError::not_found("Music not found"))?;

    let stem = find_public_stem(&state.db_ro, &state.db_rw, &record.id, track_index)
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

async fn store_conversion(
    state: &AppState,
    music_id: &str,
    kind: &str,
    outcome: audio::ConversionOutcome,
) -> Result<(Option<String>, String, Option<String>), AppError> {
    match outcome {
        audio::ConversionOutcome::Ready {
            bytes,
            content_type,
            extension,
        } => {
            let object_key = format!("{kind}/{music_id}.{extension}");
            let (stored_bytes, content_encoding) = if kind == "musicxml" && state.storage.is_s3() {
                (gzip_bytes(&bytes)?, Some("gzip"))
            } else {
                (bytes, None)
            };
            state
                .storage
                .upload_bytes_with_encoding(
                    &object_key,
                    stored_bytes,
                    content_type,
                    content_encoding,
                )
                .await?;
            Ok((Some(object_key), "ready".to_owned(), None))
        }
        audio::ConversionOutcome::Unavailable { reason } => {
            Ok((None, "unavailable".to_owned(), Some(reason)))
        }
        audio::ConversionOutcome::Failed { reason } => {
            warn!("{kind} conversion failed for {music_id}: {reason}");
            Ok((None, "failed".to_owned(), Some(reason)))
        }
    }
}

async fn admin_update_music(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(payload): Json<UpdateMusicRequest>,
) -> Result<Json<AdminMusicResponse>, AppError> {
    require_admin(&headers, &state.config)?;

    let public_id = normalize_public_id(payload.public_id.as_deref())?;
    ensure_public_id_available(&state.db_rw, public_id.as_deref(), Some(&id)).await?;

    let update_result = sqlx::query("UPDATE musics SET public_id = $1 WHERE id = $2")
        .bind(&public_id)
        .bind(&id)
        .execute(&state.db_rw)
        .await?;

    if update_result.rows_affected() == 0 {
        return Err(AppError::not_found("Music not found"));
    }

    let record = find_music_by_id(&state.db_rw, &id)
        .await?
        .ok_or_else(|| AppError::not_found("Music not found"))?;

    let stems_total = fetch_stems_total(&state.db_rw, &id).await;
    Ok(Json(record_to_admin_response(
        &state.config,
        &state.storage,
        record,
        stems_total,
    )))
}

async fn admin_move_music(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(payload): Json<MoveMusicRequest>,
) -> Result<Json<AdminMusicResponse>, AppError> {
    require_admin(&headers, &state.config)?;

    let directory_id = payload.directory_id.trim();
    if directory_id.is_empty() {
        return Err(AppError::bad_request("Choose a target directory"));
    }
    ensure_directory_exists(&state.db_rw, directory_id).await?;

    let update_result = sqlx::query("UPDATE musics SET directory_id = $1 WHERE id = $2")
        .bind(directory_id)
        .bind(&id)
        .execute(&state.db_rw)
        .await?;

    if update_result.rows_affected() == 0 {
        return Err(AppError::not_found("Music not found"));
    }

    let record = find_music_by_id(&state.db_rw, &id)
        .await?
        .ok_or_else(|| AppError::not_found("Music not found"))?;
    let stems_total = fetch_stems_total(&state.db_rw, &id).await;
    Ok(Json(record_to_admin_response(
        &state.config,
        &state.storage,
        record,
        stems_total,
    )))
}

async fn admin_delete_music(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    require_admin(&headers, &state.config)?;

    let record = find_music_by_id(&state.db_rw, &id)
        .await?
        .ok_or_else(|| AppError::not_found("Music not found"))?;
    let stems = find_public_stems(&state.db_rw, &state.db_rw, &id).await?;

    sqlx::query("DELETE FROM stems WHERE music_id = $1")
        .bind(&id)
        .execute(&state.db_rw)
        .await?;
    sqlx::query("DELETE FROM musics WHERE id = $1")
        .bind(&id)
        .execute(&state.db_rw)
        .await?;

    let mut keys = vec![record.object_key];
    if let Some(value) = record.audio_object_key {
        keys.push(value);
    }
    if let Some(value) = record.midi_object_key {
        keys.push(value);
    }
    if let Some(value) = record.musicxml_object_key {
        keys.push(value);
    }
    for stem in stems {
        keys.push(stem.storage_key);
    }

    for key in keys {
        if let Err(error) = state.storage.delete_key(&key).await {
            warn!("failed to delete storage object {key}: {error}");
        }
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn admin_export_score_gains(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    require_admin(&headers, &state.config)?;

    let record = find_music_by_id(&state.db_rw, &id)
        .await?
        .ok_or_else(|| AppError::not_found("Music not found"))?;

    export_score_gains_response(&state, &record).await
}

async fn admin_export_public_score_gains(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(access_key): Path<String>,
) -> Result<Response, AppError> {
    require_admin(&headers, &state.config)?;

    let record = find_public_music_record(&state, &access_key)
        .await?
        .ok_or_else(|| AppError::not_found("Music not found"))?;

    export_score_gains_response(&state, &record).await
}

async fn admin_export_public_mixer_gains(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(access_key): Path<String>,
    Json(payload): Json<ExportMixerGainsRequest>,
) -> Result<Response, AppError> {
    require_admin(&headers, &state.config)?;

    let record = find_public_music_record(&state, &access_key)
        .await?
        .ok_or_else(|| AppError::not_found("Music not found"))?;
    let midi_key = record
        .midi_object_key
        .clone()
        .ok_or_else(|| AppError::not_found("MIDI export is not available for this score"))?;
    let (midi_bytes, _, _) = state.storage.get_bytes(&midi_key).await?;
    let track_settings = payload
        .tracks
        .into_iter()
        .map(|track| audio::LiveMixerTrackSetting {
            track_index: track.track_index,
            volume_multiplier: track.volume_multiplier,
            muted: track.muted,
        })
        .collect::<Vec<_>>();
    let gains =
        audio::export_live_mixer_gain_template(&state.config, &midi_bytes, &track_settings).await?;

    Ok(binary_response(
        Bytes::from(
            serde_json::to_vec_pretty(&gains)
                .map_err(|error| AppError::from(anyhow::Error::from(error)))?,
        ),
        "application/json".to_owned(),
        None,
        Some(format!(
            "attachment; filename=\"{}\"",
            gains_filename_for(&record.filename)
        )),
    ))
}

async fn export_score_gains_response(
    state: &AppState,
    record: &MusicRecord,
) -> Result<Response, AppError> {
    let gains = if let Some(path) = state.storage.local_path_for_key(&record.object_key) {
        audio::export_score_gain_template(&state.config, &path).await?
    } else {
        let (bytes, _, _) = state.storage.get_bytes(&record.object_key).await?;
        let temp_dir = tempfile::tempdir()?;
        let temp_score_path = temp_dir.path().join(sanitize_filename(&record.filename));
        fs::write(&temp_score_path, bytes).await?;
        audio::export_score_gain_template(&state.config, &temp_score_path).await?
    };

    let body = serde_json::to_vec_pretty(&gains)
        .map_err(|error| AppError::from(anyhow::Error::from(error)))?;

    Ok(binary_response(
        Bytes::from(body),
        "application/json".to_owned(),
        None,
        Some(format!(
            "attachment; filename=\"{}\"",
            gains_filename_for(&record.filename)
        )),
    ))
}

async fn public_music(
    State(state): State<AppState>,
    Path(access_key): Path<String>,
) -> Result<Json<PublicMusicResponse>, AppError> {
    let record = find_public_music_record(&state, &access_key)
        .await?
        .ok_or_else(|| AppError::not_found("Music not found"))?;

    Ok(Json(record_to_public_response(
        &state.storage,
        record,
        &access_key,
    )))
}

async fn public_music_audio(
    State(state): State<AppState>,
    Path(access_key): Path<String>,
) -> Result<Response, AppError> {
    let record = find_public_music_record(&state, &access_key)
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
    let record = find_public_music_record(&state, &access_key)
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
            midi_filename_for(&record.filename)
        )),
    ))
}

async fn public_music_musicxml(
    State(state): State<AppState>,
    Path(access_key): Path<String>,
) -> Result<Response, AppError> {
    let record = find_public_music_record(&state, &access_key)
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
        // inline so the browser/OSMD can fetch it; filename still set for right-click-save
        Some(format!(
            "inline; filename=\"{}.musicxml\"",
            sanitize_content_disposition(&record.filename.trim_end_matches(".mscz").to_owned())
        )),
    ))
}

async fn public_music_download(
    State(state): State<AppState>,
    Path(access_key): Path<String>,
) -> Result<Response, AppError> {
    let record = find_public_music_record(&state, &access_key)
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

async fn exchange_login_token(
    State(state): State<AppState>,
    Json(payload): Json<ExchangeLoginTokenRequest>,
) -> Result<Json<AuthSessionResponse>, AppError> {
    let token = payload.token.trim();
    if token.is_empty() {
        return Err(AppError::bad_request("Missing login token"));
    }

    let mut transaction = state.db_rw.begin().await?;
    let now = utc_now_string();
    let user_id = sqlx::query_scalar::<_, String>(
        r#"
        UPDATE user_login_links
        SET consumed_at = $1
        WHERE token = $2
          AND consumed_at IS NULL
          AND expires_at > $1
        RETURNING user_id
        "#,
    )
    .bind(&now)
    .bind(token)
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or_else(|| AppError::unauthorized("This connection link is invalid or expired"))?;

    let user = find_user_by_id(&state.db_rw, &user_id)
        .await?
        .ok_or_else(|| AppError::unauthorized("User not found"))?;

    let session_token = generate_auth_token(64);
    let session_expires_at =
        format_timestamp(Utc::now() + Duration::days(USER_SESSION_TTL_DAYS));

    sqlx::query(
        r#"
        INSERT INTO user_sessions (id, user_id, session_token, created_at, expires_at)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&user_id)
    .bind(&session_token)
    .bind(&now)
    .bind(&session_expires_at)
    .execute(&mut *transaction)
    .await?;

    transaction.commit().await?;

    Ok(Json(AuthSessionResponse {
        session_token,
        session_expires_at,
        user: user_record_to_response(user),
    }))
}

async fn current_user(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<CurrentUserResponse>, AppError> {
    let (user, session) = require_user_session(&state, &headers).await?;
    Ok(Json(CurrentUserResponse {
        session_expires_at: session.expires_at,
        user: user_record_to_response(user),
    }))
}

async fn current_user_library(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<UserLibraryResponse>, AppError> {
    let (user, _) = require_user_session(&state, &headers).await?;
    let musics = find_accessible_music_for_user(&state.db_rw, &user.id).await?;

    let mut directories: Vec<UserLibraryDirectoryResponse> = Vec::new();
    for music in musics {
        let public_id_url = music
            .public_id
            .as_ref()
            .map(|public_id| state.config.public_url_for(public_id));
        let score = UserLibraryScoreResponse {
            id: music.id.clone(),
            title: music.title,
            filename: music.filename,
            public_url: state.config.public_url_for(&music.public_token),
            public_id_url,
            created_at: music.created_at,
            directory_id: music.directory_id.clone(),
            directory_name: music.directory_name.clone(),
        };

        if let Some(directory) = directories
            .iter_mut()
            .find(|directory| directory.id == music.directory_id)
        {
            directory.scores.push(score);
        } else {
            directories.push(UserLibraryDirectoryResponse {
                id: music.directory_id,
                name: music.directory_name,
                scores: vec![score],
            });
        }
    }

    Ok(Json(UserLibraryResponse { directories }))
}

async fn create_my_login_link(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<LoginLinkResponse>, AppError> {
    let (user, _) = require_user_session(&state, &headers).await?;
    Ok(Json(create_login_link_response(&state, &user.id).await?))
}

fn require_admin(headers: &HeaderMap, config: &AppConfig) -> Result<(), AppError> {
    let Some(header_value) = headers.get("x-admin-password") else {
        return Err(AppError::unauthorized("Missing x-admin-password header"));
    };

    let password = header_value
        .to_str()
        .map_err(|_| AppError::unauthorized("Invalid x-admin-password header"))?;

    if password != config.admin_password {
        return Err(AppError::unauthorized("Invalid admin password"));
    }

    Ok(())
}

async fn ensure_public_id_available(
    db: &PgPool,
    public_id: Option<&str>,
    current_music_id: Option<&str>,
) -> Result<(), AppError> {
    let Some(public_id) = public_id else {
        return Ok(());
    };

    let existing = sqlx::query_scalar::<_, String>("SELECT id FROM musics WHERE public_id = $1")
        .bind(public_id)
        .fetch_optional(db)
        .await?;

    if let Some(existing_id) = existing {
        if Some(existing_id.as_str()) != current_music_id {
            return Err(AppError::conflict("That public id is already in use"));
        }
    }

    Ok(())
}

async fn find_music_by_id(db: &PgPool, id: &str) -> Result<Option<MusicRecord>> {
    Ok(sqlx::query_as::<_, MusicRecord>(
        r#"
        SELECT m.id, m.title, m.filename, m.content_type, m.object_key, m.audio_object_key, m.audio_status, m.audio_error, m.midi_object_key, m.midi_status, m.midi_error, m.musicxml_object_key, m.musicxml_status, m.musicxml_error, m.stems_status, m.stems_error, m.public_token, m.public_id, m.quality_profile, m.created_at, m.directory_id, COALESCE(d.name, 'General') AS directory_name
        FROM musics m
        LEFT JOIN directories d ON d.id = m.directory_id
        WHERE m.id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(db)
    .await?)
}

async fn find_music_by_access_key(db: &PgPool, access_key: &str) -> Result<Option<MusicRecord>> {
    Ok(sqlx::query_as::<_, MusicRecord>(
        r#"
        SELECT m.id, m.title, m.filename, m.content_type, m.object_key, m.audio_object_key, m.audio_status, m.audio_error, m.midi_object_key, m.midi_status, m.midi_error, m.musicxml_object_key, m.musicxml_status, m.musicxml_error, m.stems_status, m.stems_error, m.public_token, m.public_id, m.quality_profile, m.created_at, m.directory_id, COALESCE(d.name, 'General') AS directory_name
        FROM musics m
        LEFT JOIN directories d ON d.id = m.directory_id
        WHERE m.public_token = $1 OR m.public_id = $2
        LIMIT 1
        "#,
    )
    .bind(access_key)
    .bind(access_key)
    .fetch_optional(db)
    .await?)
}

fn record_to_admin_response(
    config: &AppConfig,
    storage: &Storage,
    record: MusicRecord,
    stems_total_bytes: i64,
) -> AdminMusicResponse {
    let public_id_url = record
        .public_id
        .as_ref()
        .map(|public_id| config.public_url_for(public_id));
    let midi_download_url = record.midi_object_key.as_ref().map(|object_key| {
        storage
            .public_url(object_key)
            .unwrap_or_else(|| format!("/api/public/{}/midi", record.public_token))
    });
    let download_url = storage
        .public_url(&record.object_key)
        .unwrap_or_else(|| format!("/api/public/{}/download", record.public_token));

    AdminMusicResponse {
        id: record.id,
        title: record.title,
        filename: record.filename,
        content_type: record.content_type,
        audio_status: record.audio_status,
        audio_error: record.audio_error,
        midi_status: record.midi_status,
        midi_error: record.midi_error,
        musicxml_status: record.musicxml_status,
        musicxml_error: record.musicxml_error,
        stems_status: record.stems_status,
        stems_error: record.stems_error,
        public_token: record.public_token.clone(),
        public_id: record.public_id,
        public_url: config.public_url_for(&record.public_token),
        public_id_url,
        download_url,
        midi_download_url,
        quality_profile: record.quality_profile,
        created_at: record.created_at,
        stems_total_bytes,
        directory_id: record.directory_id,
        directory_name: record.directory_name,
    }
}

fn record_to_public_response(
    storage: &Storage,
    record: MusicRecord,
    access_key: &str,
) -> PublicMusicResponse {
    let midi_download_url = record.midi_object_key.as_ref().map(|object_key| {
        storage
            .public_url(object_key)
            .unwrap_or_else(|| format!("/api/public/{access_key}/midi"))
    });
    let musicxml_url = record.musicxml_object_key.as_ref().map(|object_key| {
        storage
            .public_url(object_key)
            .unwrap_or_else(|| format!("/api/public/{access_key}/musicxml"))
    });
    let download_url = storage
        .public_url(&record.object_key)
        .unwrap_or_else(|| format!("/api/public/{access_key}/download"));

    PublicMusicResponse {
        title: record.title,
        filename: record.filename,
        audio_status: "disabled".to_owned(),
        audio_error: None,
        can_stream_audio: false,
        audio_stream_url: None,
        midi_status: record.midi_status,
        midi_error: record.midi_error,
        midi_download_url,
        musicxml_url,
        stems_status: record.stems_status,
        stems_error: record.stems_error,
        download_url,
        created_at: record.created_at,
    }
}

fn user_record_to_response(record: UserRecord) -> UserResponse {
    UserResponse {
        id: record.id,
        username: record.username,
        created_at: record.created_at,
    }
}

fn generate_public_token() -> String {
    generate_auth_token(24)
}

fn generate_auth_token(length: usize) -> String {
    rand::rng()
        .sample_iter(Alphanumeric)
        .take(length)
        .map(char::from)
        .collect()
}

fn utc_now_string() -> String {
    format_timestamp(Utc::now())
}

fn format_timestamp(value: chrono::DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn parse_quality_profile(raw: Option<&str>) -> Result<audio::StemQualityProfile, AppError> {
    let value = raw
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(audio::DEFAULT_STEM_QUALITY_PROFILE);

    audio::StemQualityProfile::from_slug(value).ok_or_else(|| {
        AppError::bad_request("Invalid quality profile. Use one of: compact, standard, high.")
    })
}

fn normalize_username(raw: &str) -> Result<String, AppError> {
    let value = raw.trim().to_ascii_lowercase();
    if !(3..=32).contains(&value.len()) {
        return Err(AppError::bad_request(
            "Usernames must be between 3 and 32 characters",
        ));
    }

    if !value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '-' || character == '_')
    {
        return Err(AppError::bad_request(
            "Usernames can only contain letters, numbers, hyphens, and underscores",
        ));
    }

    Ok(value)
}

fn normalize_name(
    raw: &str,
    label: &str,
    min_len: usize,
    max_len: usize,
) -> Result<String, AppError> {
    let value = raw.trim();
    if !(min_len..=max_len).contains(&value.len()) {
        return Err(AppError::bad_request(format!(
            "{label} must be between {min_len} and {max_len} characters",
        )));
    }

    Ok(value.to_owned())
}

fn slugify_name(raw: &str) -> String {
    let mut slug = raw
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    let trimmed = slug.trim_matches('-').to_owned();
    if trimmed.is_empty() {
        Uuid::new_v4().to_string()
    } else {
        trimmed
    }
}

fn normalize_public_id(raw: Option<&str>) -> Result<Option<String>, AppError> {
    let Some(value) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };

    if !(3..=64).contains(&value.len()) {
        return Err(AppError::bad_request(
            "Public ids must be between 3 and 64 characters",
        ));
    }

    if !value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '-' || character == '_')
    {
        return Err(AppError::bad_request(
            "Public ids can only contain letters, numbers, hyphens, and underscores",
        ));
    }

    Ok(Some(value.to_ascii_lowercase()))
}

async fn ensure_directory_exists(db: &PgPool, directory_id: &str) -> Result<(), AppError> {
    if find_directory_by_id(db, directory_id).await?.is_none() {
        return Err(AppError::not_found("Directory not found"));
    }
    Ok(())
}

async fn ensure_membership_entities_exist(
    db: &PgPool,
    ensemble_id: &str,
    user_id: &str,
) -> Result<(), AppError> {
    if find_ensemble_by_id(db, ensemble_id).await?.is_none() {
        return Err(AppError::not_found("Ensemble not found"));
    }
    if find_user_by_id(db, user_id).await?.is_none() {
        return Err(AppError::not_found("User not found"));
    }
    Ok(())
}

async fn ensure_directory_and_ensemble_exist(
    db: &PgPool,
    directory_id: &str,
    ensemble_id: &str,
) -> Result<(), AppError> {
    ensure_directory_exists(db, directory_id).await?;
    if find_ensemble_by_id(db, ensemble_id).await?.is_none() {
        return Err(AppError::not_found("Ensemble not found"));
    }
    Ok(())
}

fn sanitize_filename(filename: &str) -> String {
    let mut sanitized = filename
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric()
                || character == '.'
                || character == '-'
                || character == '_'
            {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();

    if sanitized.is_empty() {
        sanitized = "score.mscz".to_owned();
    }

    sanitized
}

fn sanitize_content_disposition(filename: &str) -> String {
    filename.replace('"', "")
}

fn midi_filename_for(filename: &str) -> String {
    let stem = filename
        .trim_end_matches(".mscz")
        .trim_end_matches(".MSCZ")
        .trim_end_matches(".mscx")
        .trim_end_matches(".MSCX");
    sanitize_content_disposition(&format!("{stem}.mid"))
}

fn gains_filename_for(filename: &str) -> String {
    let stem = filename
        .trim_end_matches(".mscz")
        .trim_end_matches(".MSCZ")
        .trim_end_matches(".mscx")
        .trim_end_matches(".MSCX");
    sanitize_content_disposition(&format!("{stem}.gains.json"))
}

fn stem_full_key(music_id: &str, track_index: usize) -> String {
    format!("stems/{music_id}/{track_index}.ogg")
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

fn gzip_bytes(bytes: &Bytes) -> Result<Bytes, AppError> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes).map_err(AppError::from)?;
    let compressed = encoder.finish().map_err(AppError::from)?;
    Ok(Bytes::from(compressed))
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
