use crate::models::NewUserSession;
use crate::schema::{user_login_links, user_sessions};
use crate::schemas::{
    AccessTokenRefreshResponse, AuthTokenResponse, ErrorResponse, ExchangeLoginTokenRequest,
    RefreshTokenRequest,
};
use crate::services::auth;
use crate::{AppError, AppState, utc_now_string};
use axum::{Json, Router, extract::State};
use diesel::OptionalExtension;
use diesel::prelude::*;
use diesel_async::{AsyncConnection, RunQueryDsl};
use uuid::Uuid;

pub(super) fn routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route(
            "/auth/exchange",
            crate::op_post!(state, "/auth/exchange", exchange_login_token),
        )
        .route(
            "/auth/refresh",
            crate::op_post!(state, "/auth/refresh", refresh_access_token),
        )
}

#[utoipa::path(
    post,
    path = "/api/auth/exchange",
    tag = "auth",
    request_body = ExchangeLoginTokenRequest,
    responses(
        (status = 200, description = "Exchange login token for session tokens", body = AuthTokenResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 401, description = "Invalid or expired login token", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse)
    )
)]
pub(crate) async fn exchange_login_token(
    State(state): State<AppState>,
    Json(payload): Json<ExchangeLoginTokenRequest>,
) -> Result<Json<AuthTokenResponse>, AppError> {
    let token = payload.token.trim();
    if token.is_empty() {
        return Err(AppError::bad_request("Missing login token"));
    }

    let now = utc_now_string();
    let session_id = crate::generate_auth_token(64);
    let session_id_for_tx = session_id.clone();
    let mut conn = state.db_rw.get().await?;
    let user_id = conn
        .transaction::<_, AppError, _>(|tx| {
            Box::pin(async move {
                let user_id = diesel::update(
                    user_login_links::table
                        .filter(user_login_links::token.eq(token))
                        .filter(user_login_links::consumed_at.is_null())
                        .filter(user_login_links::expires_at.gt(&now)),
                )
                .set(user_login_links::consumed_at.eq(Some(now.clone())))
                .returning(user_login_links::user_id)
                .get_result::<String>(tx)
                .await
                .optional()?
                .ok_or_else(|| {
                    AppError::unauthorized("This connection link is invalid or expired")
                })?;

                let new_session_id = Uuid::new_v4().to_string();
                diesel::insert_into(user_sessions::table)
                    .values(&NewUserSession {
                        id: &new_session_id,
                        user_id: &user_id,
                        session_token: &session_id_for_tx,
                        created_at: &now,
                        expires_at: None,
                    })
                    .execute(tx)
                    .await?;

                Ok(user_id)
            })
        })
        .await?;

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

#[utoipa::path(
    post,
    path = "/api/auth/refresh",
    tag = "auth",
    request_body = RefreshTokenRequest,
    responses(
        (status = 200, description = "Refresh access token", body = AccessTokenRefreshResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 401, description = "Invalid or revoked refresh token", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse)
    )
)]
pub(crate) async fn refresh_access_token(
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
