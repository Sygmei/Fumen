use crate::schemas::{
    CurrentUserResponse, LoginLinkResponse, UserLibraryEnsembleResponse, UserLibraryResponse,
    UserLibraryScoreResponse,
};
use crate::services::{auth, music};
use crate::{AppError, AppState};
use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/me", get(current_user))
        .route("/me/library", get(current_user_library))
        .route("/me/login-link", post(create_my_login_link))
        .route("/me/logout", post(me_logout))
}

async fn current_user(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<CurrentUserResponse>, AppError> {
    let auth_context = auth::build_auth_context(&state, &headers).await?;
    Ok(Json(CurrentUserResponse {
        session_expires_at: Some(auth::exp_to_timestamp(auth_context.access_token_exp)),
        user: auth::auth_context_to_user_response(&auth_context),
    }))
}

async fn current_user_library(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<UserLibraryResponse>, AppError> {
    let auth_context = auth::build_auth_context(&state, &headers).await?;
    let music_entries = if auth_context.has_global_power() {
        music::find_all_accessible_music(&state.db_rw).await?
    } else {
        music::find_accessible_music_for_user(&state.db_rw, &auth_context.user.id).await?
    };

    let mut ensembles: Vec<UserLibraryEnsembleResponse> = Vec::new();
    for (music_record, ensemble_id, ensemble_name) in music_entries {
        let public_id_url = music_record
            .public_id
            .as_ref()
            .map(|public_id| state.config.public_url_for(public_id));
        let icon_image_url = music_record.icon_image_key.as_ref().map(|key| {
            state
                .storage
                .public_url(key)
                .unwrap_or_else(|| format!("/api/public/{}/icon", music_record.public_token))
        });
        let score = UserLibraryScoreResponse {
            id: music_record.id.clone(),
            title: music_record.title,
            icon: music_record.icon,
            icon_image_url,
            filename: music_record.filename,
            public_url: state.config.public_url_for(&music_record.public_token),
            public_id_url,
            created_at: music_record.created_at,
        };

        if let Some(ensemble) = ensembles.iter_mut().find(|ensemble| ensemble.id == ensemble_id) {
            ensemble.scores.push(score);
        } else {
            ensembles.push(UserLibraryEnsembleResponse {
                id: ensemble_id,
                name: ensemble_name,
                scores: vec![score],
            });
        }
    }

    Ok(Json(UserLibraryResponse { ensembles }))
}

async fn create_my_login_link(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<LoginLinkResponse>, AppError> {
    let (user, _, _) = auth::require_user_session(&state, &headers).await?;
    Ok(Json(
        auth::create_login_link(&state.db_rw, &state.config, &user.id).await?,
    ))
}

async fn me_logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<StatusCode, AppError> {
    let auth_context = auth::build_auth_context(&state, &headers).await?;
    sqlx::query("DELETE FROM user_sessions WHERE session_token = $1")
        .bind(&auth_context.session.session_token)
        .execute(&state.db_rw)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}