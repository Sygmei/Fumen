use crate::schemas::{
    AccessTokenRefreshResponse, AuthTokenResponse, ExchangeLoginTokenRequest, RefreshTokenRequest,
};
use crate::services::auth;
use crate::{AppError, AppState, utc_now_string};
use axum::{Json, Router, extract::State, routing::post};
use uuid::Uuid;

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/auth/exchange", post(exchange_login_token))
        .route("/auth/refresh", post(refresh_access_token))
}

async fn exchange_login_token(
    State(state): State<AppState>,
    Json(payload): Json<ExchangeLoginTokenRequest>,
) -> Result<Json<AuthTokenResponse>, AppError> {
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

    let session_id = crate::generate_auth_token(64);

    sqlx::query(
        r#"
        INSERT INTO user_sessions (id, user_id, session_token, created_at, expires_at)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&user_id)
    .bind(&session_id)
    .bind(&now)
    .bind(None::<String>)
    .execute(&mut *transaction)
    .await?;

    transaction.commit().await?;

    let user = auth::find_user_by_id(&state.db_rw, &user_id)
        .await?
        .ok_or_else(|| AppError::unauthorized("User not found"))?;

    let refresh_token = auth::sign_refresh_token(&session_id, &state.config.jwt_secret)?;
    let (access_token, access_token_exp) =
        auth::sign_access_token(&user.username, &session_id, &state.config.jwt_secret)?;

    Ok(Json(AuthTokenResponse {
        refresh_token,
        access_token,
        access_token_expires_at: auth::exp_to_timestamp(access_token_exp),
        user: auth::user_record_to_response(&state.db_rw, &state.storage, user).await?,
    }))
}

async fn refresh_access_token(
    State(state): State<AppState>,
    Json(payload): Json<RefreshTokenRequest>,
) -> Result<Json<AccessTokenRefreshResponse>, AppError> {
    let refresh_token = payload.refresh_token.trim();
    if refresh_token.is_empty() {
        return Err(AppError::bad_request("Missing refresh_token"));
    }

    let session_token = auth::verify_refresh_token(refresh_token, &state.config.jwt_secret)?;

    let session = auth::find_session_by_token(&state.db_rw, &session_token)
        .await?
        .ok_or_else(|| AppError::unauthorized("Session has been revoked"))?;

    let user = auth::find_user_by_id(&state.db_rw, &session.user_id)
        .await?
        .ok_or_else(|| AppError::unauthorized("User not found"))?;

    let (access_token, access_token_exp) =
        auth::sign_access_token(&user.username, &session_token, &state.config.jwt_secret)?;

    Ok(Json(AccessTokenRefreshResponse {
        access_token,
        access_token_expires_at: auth::exp_to_timestamp(access_token_exp),
    }))
}
