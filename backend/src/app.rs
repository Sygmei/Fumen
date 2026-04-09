use crate::audio;
use crate::config::AppConfig;
use crate::models::{UserRecord, UserSessionRecord};
use crate::storage::Storage;
use axum::{
    Json,
    extract::multipart::MultipartError,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::{SecondsFormat, Utc};
use rand::{Rng, distr::Alphanumeric};
use sqlx::PgPool;
use std::collections::HashSet;

pub(crate) const LOGIN_LINK_TTL_MINUTES: i64 = 5;
pub(crate) const ACCESS_TOKEN_TTL_SECONDS: i64 = 86400;
pub(crate) const DEFAULT_ENSEMBLE_ID: &str = "general";
pub(crate) const DEFAULT_ENSEMBLE_NAME: &str = "General";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AppRole {
    Superadmin,
    Admin,
    Manager,
    Editor,
    User,
}

impl AppRole {
    pub(crate) fn from_str(value: &str) -> Option<Self> {
        match value {
            "superadmin" => Some(Self::Superadmin),
            "admin" => Some(Self::Admin),
            "manager" => Some(Self::Manager),
            "editor" => Some(Self::Editor),
            "user" => Some(Self::User),
            _ => None,
        }
    }

    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Superadmin => "superadmin",
            Self::Admin => "admin",
            Self::Manager => "manager",
            Self::Editor => "editor",
            Self::User => "user",
        }
    }

    pub(crate) fn has_global_power(&self) -> bool {
        matches!(self, Self::Superadmin | Self::Admin)
    }

    pub(crate) fn can_access_control_room(&self) -> bool {
        !matches!(self, Self::User)
    }

    pub(crate) fn can_list_users(&self) -> bool {
        matches!(self, Self::Superadmin | Self::Admin | Self::Manager)
    }

    pub(crate) fn can_create_users(&self) -> bool {
        self.can_list_users()
    }

    pub(crate) fn can_create_ensembles(&self) -> bool {
        matches!(self, Self::Superadmin | Self::Admin | Self::Manager)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EnsembleRole {
    Manager,
    Editor,
    User,
}

impl EnsembleRole {
    pub(crate) fn from_str(value: &str) -> Option<Self> {
        match value {
            "manager" => Some(Self::Manager),
            "editor" => Some(Self::Editor),
            "user" => Some(Self::User),
            _ => None,
        }
    }

    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Manager => "manager",
            Self::Editor => "editor",
            Self::User => "user",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct AuthContext {
    pub(crate) user: UserRecord,
    pub(crate) session: UserSessionRecord,
    pub(crate) role: AppRole,
    pub(crate) managed_ensemble_ids: HashSet<String>,
    pub(crate) editable_ensemble_ids: HashSet<String>,
    pub(crate) access_token_exp: i64,
}

impl AuthContext {
    pub(crate) fn has_global_power(&self) -> bool {
        self.role.has_global_power()
    }

    pub(crate) fn can_access_control_room(&self) -> bool {
        self.role.can_access_control_room()
    }

    pub(crate) fn can_list_users(&self) -> bool {
        self.role.can_list_users()
    }

    pub(crate) fn can_create_users(&self) -> bool {
        self.role.can_create_users()
    }

    pub(crate) fn can_create_ensembles(&self) -> bool {
        self.role.can_create_ensembles()
    }

    pub(crate) fn can_manage_ensemble(&self, ensemble_id: &str) -> bool {
        self.has_global_power() || self.managed_ensemble_ids.contains(ensemble_id)
    }

    pub(crate) fn can_edit_ensemble_scores(&self, ensemble_id: &str) -> bool {
        self.has_global_power() || self.editable_ensemble_ids.contains(ensemble_id)
    }
}

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) config: AppConfig,
    pub(crate) db_rw: PgPool,
    pub(crate) db_ro: PgPool,
    pub(crate) storage: Storage,
}

#[derive(Debug)]
pub(crate) struct AppError {
    pub(crate) status: StatusCode,
    pub(crate) message: String,
}

impl AppError {
    pub(crate) fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    pub(crate) fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, message)
    }

    pub(crate) fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, message)
    }

    pub(crate) fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, message)
    }

    pub(crate) fn conflict(message: impl Into<String>) -> Self {
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

pub(crate) fn generate_public_token() -> String {
    generate_auth_token(24)
}

pub(crate) fn generate_auth_token(length: usize) -> String {
    rand::rng()
        .sample_iter(Alphanumeric)
        .take(length)
        .map(char::from)
        .collect()
}

pub(crate) fn normalize_music_icon(value: Option<&str>) -> Result<Option<String>, AppError> {
    let Some(raw) = value else {
        return Ok(None);
    };

    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    if trimmed.chars().count() > 2 {
        return Err(AppError::bad_request(
            "Score icon must be 1 or 2 characters",
        ));
    }

    Ok(Some(trimmed.chars().take(2).collect::<String>()))
}

pub(crate) fn utc_now_string() -> String {
    format_timestamp(Utc::now())
}

pub(crate) fn format_timestamp(value: chrono::DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Secs, true)
}

pub(crate) fn parse_quality_profile(
    raw: Option<&str>,
) -> Result<audio::StemQualityProfile, AppError> {
    let value = raw
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(audio::DEFAULT_STEM_QUALITY_PROFILE);

    audio::StemQualityProfile::from_slug(value).ok_or_else(|| {
        AppError::bad_request(
            "Invalid quality profile. Use one of: standard, small, very-small, tiny.",
        )
    })
}

pub(crate) fn normalize_username(raw: &str) -> Result<String, AppError> {
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

pub(crate) fn normalize_name(
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

pub(crate) fn normalize_public_id(raw: Option<&str>) -> Result<Option<String>, AppError> {
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

pub(crate) async fn ensure_membership_entities_exist(
    db: &PgPool,
    ensemble_id: &str,
    user_id: &str,
) -> Result<UserRecord, AppError> {
    if crate::services::music::find_ensemble_by_id(db, ensemble_id)
        .await?
        .is_none()
    {
        return Err(AppError::not_found("Ensemble not found"));
    }

    crate::services::auth::find_user_by_id(db, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("User not found"))
}

pub(crate) fn sanitize_filename(filename: &str) -> String {
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

pub(crate) fn sanitize_content_disposition(filename: &str) -> String {
    filename.replace('"', "")
}