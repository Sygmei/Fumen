use crate::config::AppConfig;
use crate::models::{EnsembleRecord, MusicRecord, UserRecord, UserSessionRecord};
use crate::schemas::{LoginLinkResponse, UserResponse};
use crate::{
    ACCESS_TOKEN_TTL_SECONDS, AppError, AppRole, AppState, AuthContext, EnsembleRole,
    LOGIN_LINK_TTL_MINUTES, format_timestamp, generate_auth_token,
};
use axum::http::{HeaderMap, header};
use chrono::{Duration, SecondsFormat, Utc};
use sqlx::PgPool;
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct RefreshTokenClaims {
    sub: String,
    iat: i64,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct AccessTokenClaims {
    sub: String,
    sid: String,
    exp: i64,
    iat: i64,
}

pub(crate) async fn find_user_by_id(db: &PgPool, id: &str) -> Result<Option<UserRecord>, AppError> {
    Ok(sqlx::query_as::<_, UserRecord>(
        "SELECT id, username, created_at, is_superadmin, role, created_by_user_id FROM users WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(db)
    .await?)
}

pub(crate) async fn find_user_by_username(
    db: &PgPool,
    username: &str,
) -> Result<Option<UserRecord>, AppError> {
    Ok(sqlx::query_as::<_, UserRecord>(
        "SELECT id, username, created_at, is_superadmin, role, created_by_user_id FROM users WHERE username = $1",
    )
    .bind(username)
    .fetch_optional(db)
    .await?)
}

pub(crate) async fn find_session_by_token(
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

pub(crate) async fn require_user_session(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<(UserRecord, UserSessionRecord, i64), AppError> {
    let Some(header_value) = headers.get(header::AUTHORIZATION) else {
        return Err(AppError::unauthorized("Missing Authorization header"));
    };

    let authorization = header_value
        .to_str()
        .map_err(|_| AppError::unauthorized("Invalid Authorization header"))?;
    let token = authorization
        .strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::unauthorized("Expected a Bearer token"))?;

    let claims = verify_access_token(token, &state.config.jwt_secret)?;

    let session = find_session_by_token(&state.db_rw, &claims.sid)
        .await?
        .ok_or_else(|| AppError::unauthorized("Session has been revoked"))?;

    let user = find_user_by_username(&state.db_rw, &claims.sub)
        .await?
        .ok_or_else(|| AppError::unauthorized("User not found"))?;

    Ok((user, session, claims.exp))
}

pub(crate) async fn build_auth_context(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<AuthContext, AppError> {
    let (user, session, access_token_exp) = require_user_session(state, headers).await?;
    let managed_ensemble_ids = fetch_managed_ensemble_ids(&state.db_rw, &user.id)
        .await?
        .into_iter()
        .collect::<HashSet<_>>();
    let editable_ensemble_ids = fetch_editable_ensemble_ids(&state.db_rw, &user.id)
        .await?
        .into_iter()
        .collect::<HashSet<_>>();
    let role = parse_global_role(&user.role)?;

    Ok(AuthContext {
        user,
        session,
        role,
        managed_ensemble_ids,
        editable_ensemble_ids,
        access_token_exp,
    })
}

pub(crate) async fn require_admin_context(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<AuthContext, AppError> {
    let auth = build_auth_context(state, headers).await?;
    if !auth.can_access_control_room() {
        return Err(AppError::unauthorized("Privileged access is required"));
    }
    Ok(auth)
}

pub(crate) fn parse_global_role(value: &str) -> Result<AppRole, AppError> {
    AppRole::from_str(value)
        .ok_or_else(|| AppError::bad_request("Unknown global role configured for user"))
}

pub(crate) fn resolve_creatable_user_role(
    auth: &AuthContext,
    requested_role: Option<&str>,
) -> Result<AppRole, AppError> {
    let requested = requested_role
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("user");
    let role = parse_global_role(requested)?;

    match auth.role {
        AppRole::Superadmin | AppRole::Admin => match role {
            AppRole::Admin | AppRole::Manager | AppRole::Editor | AppRole::User => Ok(role),
            AppRole::Superadmin => Err(AppError::bad_request(
                "The seeded superadmin account cannot be created from the UI",
            )),
        },
        AppRole::Manager => {
            if role == AppRole::User {
                Ok(role)
            } else {
                Err(AppError::unauthorized(
                    "Managers can only create standard users",
                ))
            }
        }
        AppRole::Editor | AppRole::User => Err(AppError::unauthorized(
            "You are not allowed to create users",
        )),
    }
}

pub(crate) fn ensure_can_generate_login_link_for_user(
    auth: &AuthContext,
    target_user: &UserRecord,
) -> Result<(), AppError> {
    match auth.role {
        AppRole::Superadmin | AppRole::Admin => Ok(()),
        AppRole::Manager => {
            if target_user.created_by_user_id.as_deref() == Some(auth.user.id.as_str()) {
                Ok(())
            } else {
                Err(AppError::unauthorized(
                    "Managers can only create login links for users they created",
                ))
            }
        }
        AppRole::Editor | AppRole::User => Err(AppError::unauthorized(
            "You are not allowed to create login links for other users",
        )),
    }
}

pub(crate) fn validate_target_membership_role(
    auth: &AuthContext,
    target_user: &UserRecord,
    requested_role: &str,
) -> Result<EnsembleRole, AppError> {
    let target_role = parse_global_role(&target_user.role)?;
    let membership_role = EnsembleRole::from_str(requested_role).ok_or_else(|| {
        AppError::bad_request("Membership role must be 'manager', 'editor', or 'user'")
    })?;

    match target_role {
        AppRole::Superadmin | AppRole::Admin => Err(AppError::bad_request(
            "Admins and superadmins cannot be assigned per-ensemble memberships",
        )),
        AppRole::Manager => {
            if auth.role == AppRole::Manager {
                return Err(AppError::unauthorized(
                    "Managers cannot add or update other managers on ensembles",
                ));
            }
            Ok(membership_role)
        }
        AppRole::Editor => match membership_role {
            EnsembleRole::Editor | EnsembleRole::User => Ok(membership_role),
            EnsembleRole::Manager => Err(AppError::bad_request(
                "Editors can only be assigned as editors or users on ensembles",
            )),
        },
        AppRole::User => match membership_role {
            EnsembleRole::User => Ok(membership_role),
            EnsembleRole::Editor | EnsembleRole::Manager => Err(AppError::bad_request(
                "Standard users can only be assigned as users on ensembles",
            )),
        },
    }
}

pub(crate) fn ensure_can_remove_member_from_ensemble(
    auth: &AuthContext,
    target_user: &UserRecord,
) -> Result<(), AppError> {
    let target_role = parse_global_role(&target_user.role)?;
    if auth.role == AppRole::Manager && target_role == AppRole::Manager {
        return Err(AppError::unauthorized(
            "Managers cannot remove other managers from ensembles",
        ));
    }
    if matches!(target_role, AppRole::Superadmin | AppRole::Admin) {
        return Err(AppError::bad_request(
            "Admins and superadmins do not use per-ensemble memberships",
        ));
    }
    Ok(())
}

pub(crate) fn ensure_can_upload_to_ensemble(
    auth: &AuthContext,
    ensemble_id: &str,
) -> Result<(), AppError> {
    if auth.can_edit_ensemble_scores(ensemble_id) {
        Ok(())
    } else {
        Err(AppError::unauthorized(
            "You can only add scores to ensembles where you have editor access",
        ))
    }
}

pub(crate) fn ensure_can_delete_ensemble(
    auth: &AuthContext,
    ensemble: &EnsembleRecord,
) -> Result<(), AppError> {
    if auth.has_global_power()
        || ensemble.created_by_user_id.as_deref() == Some(auth.user.id.as_str())
        || auth.can_manage_ensemble(&ensemble.id)
    {
        Ok(())
    } else {
        Err(AppError::unauthorized(
            "You can only delete ensembles that you created or manage",
        ))
    }
}

pub(crate) fn ensure_can_delete_music(
    auth: &AuthContext,
    record: &MusicRecord,
) -> Result<(), AppError> {
    if auth.has_global_power() || record.owner_user_id.as_deref() == Some(auth.user.id.as_str()) {
        Ok(())
    } else {
        Err(AppError::unauthorized(
            "You can only delete scores that you own",
        ))
    }
}

pub(crate) fn ensure_can_delete_user(
    auth: &AuthContext,
    target_user: &UserRecord,
) -> Result<(), AppError> {
    let target_role = parse_global_role(&target_user.role)?;
    if target_user.id == auth.user.id {
        return Err(AppError::bad_request("You cannot delete your own account"));
    }

    match auth.role {
        AppRole::Superadmin => {
            if target_role == AppRole::Superadmin {
                Err(AppError::bad_request(
                    "The seeded superadmin account cannot be removed",
                ))
            } else {
                Ok(())
            }
        }
        AppRole::Admin => {
            if matches!(target_role, AppRole::Admin | AppRole::Superadmin) {
                Err(AppError::unauthorized(
                    "Admins cannot remove other admins or the superadmin",
                ))
            } else {
                Ok(())
            }
        }
        AppRole::Manager => {
            if target_user.created_by_user_id.as_deref() == Some(auth.user.id.as_str())
                && target_role == AppRole::User
            {
                Ok(())
            } else {
                Err(AppError::unauthorized(
                    "Managers can only remove standard users they created",
                ))
            }
        }
        AppRole::Editor | AppRole::User => Err(AppError::unauthorized(
            "You are not allowed to delete users",
        )),
    }
}

pub(crate) fn ensure_can_manage_ensemble(
    auth: &AuthContext,
    ensemble_id: &str,
) -> Result<(), AppError> {
    if auth.can_manage_ensemble(ensemble_id) {
        Ok(())
    } else {
        Err(AppError::unauthorized(
            "You can only manage ensembles where you are a manager",
        ))
    }
}

pub(crate) async fn fetch_managed_ensemble_ids(
    db: &PgPool,
    user_id: &str,
) -> Result<Vec<String>, AppError> {
    Ok(sqlx::query_scalar::<_, String>(
        "SELECT ensemble_id FROM user_ensemble_memberships WHERE user_id = $1 AND role = 'manager' ORDER BY ensemble_id ASC",
    )
    .bind(user_id)
    .fetch_all(db)
    .await?)
}

pub(crate) async fn fetch_editable_ensemble_ids(
    db: &PgPool,
    user_id: &str,
) -> Result<Vec<String>, AppError> {
    Ok(sqlx::query_scalar::<_, String>(
        "SELECT ensemble_id FROM user_ensemble_memberships WHERE user_id = $1 AND role IN ('manager', 'editor') ORDER BY ensemble_id ASC",
    )
    .bind(user_id)
    .fetch_all(db)
    .await?)
}

pub(crate) async fn user_record_to_response(
    db: &PgPool,
    record: UserRecord,
) -> Result<UserResponse, AppError> {
    let managed_ensemble_ids = fetch_managed_ensemble_ids(db, &record.id).await?;
    let editable_ensemble_ids = fetch_editable_ensemble_ids(db, &record.id).await?;
    let role = parse_global_role(&record.role)?;

    Ok(UserResponse {
        id: record.id,
        username: record.username,
        created_at: record.created_at,
        role: role.as_str().to_owned(),
        managed_ensemble_ids,
        editable_ensemble_ids,
        created_by_user_id: record.created_by_user_id,
    })
}

pub(crate) fn auth_context_to_user_response(auth: &AuthContext) -> UserResponse {
    let mut managed_ensemble_ids = auth
        .managed_ensemble_ids
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    managed_ensemble_ids.sort();
    let mut editable_ensemble_ids = auth
        .editable_ensemble_ids
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    editable_ensemble_ids.sort();

    UserResponse {
        id: auth.user.id.clone(),
        username: auth.user.username.clone(),
        created_at: auth.user.created_at.clone(),
        role: auth.role.as_str().to_owned(),
        managed_ensemble_ids,
        editable_ensemble_ids,
        created_by_user_id: auth.user.created_by_user_id.clone(),
    }
}

pub(crate) async fn create_login_link(
    db: &PgPool,
    config: &AppConfig,
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
    .execute(db)
    .await?;

    Ok(LoginLinkResponse {
        connection_url: config.connection_url_for(&token),
        expires_at,
    })
}

pub(crate) fn sign_refresh_token(session_id: &str, secret: &str) -> Result<String, AppError> {
    let claims = RefreshTokenClaims {
        sub: session_id.to_owned(),
        iat: Utc::now().timestamp(),
    };
    jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|error| {
        AppError::new(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to sign refresh token: {error}"),
        )
    })
}

pub(crate) fn sign_access_token(
    username: &str,
    session_id: &str,
    secret: &str,
) -> Result<(String, i64), AppError> {
    let now = Utc::now().timestamp();
    let exp = now + ACCESS_TOKEN_TTL_SECONDS;
    let claims = AccessTokenClaims {
        sub: username.to_owned(),
        sid: session_id.to_owned(),
        exp,
        iat: now,
    };
    let token = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|error| {
        AppError::new(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to sign access token: {error}"),
        )
    })?;
    Ok((token, exp))
}

pub(crate) fn verify_refresh_token(
    token: &str,
    secret: &str,
) -> Result<String, AppError> {
    let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
    validation.validate_exp = false;
    validation.required_spec_claims = HashSet::new();
    jsonwebtoken::decode::<RefreshTokenClaims>(
        token,
        &jsonwebtoken::DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map(|data| data.claims.sub)
    .map_err(|error| AppError::unauthorized(format!("Invalid refresh token: {error}")))
}

fn verify_access_token(token: &str, secret: &str) -> Result<AccessTokenClaims, AppError> {
    jsonwebtoken::decode::<AccessTokenClaims>(
        token,
        &jsonwebtoken::DecodingKey::from_secret(secret.as_bytes()),
        &jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256),
    )
    .map(|data| data.claims)
    .map_err(|error| AppError::unauthorized(format!("Invalid or expired access token: {error}")))
}

pub(crate) fn exp_to_timestamp(exp: i64) -> String {
    chrono::DateTime::<Utc>::from_timestamp(exp, 0)
        .unwrap_or_else(Utc::now)
        .to_rfc3339_opts(SecondsFormat::Secs, true)
}