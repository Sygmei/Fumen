use crate::schemas::{
    CurrentUserResponse, LoginLinkResponse, UserLibraryEnsembleResponse, UserLibraryResponse,
    UserLibraryScoreResponse,
};
use crate::services::{auth, music};
use crate::{AppError, AppState};
use axum::{
    Json, Router,
    body::Body,
    extract::{Multipart, Path, State},
    http::{HeaderMap, StatusCode, header},
    response::Response,
    routing::{get, patch, post},
};

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/me", get(current_user))
        .route("/me/profile", patch(update_my_profile))
        .route("/me/library", get(current_user_library))
        .route("/me/login-link", post(create_my_login_link))
        .route("/me/logout", post(me_logout))
        .route("/users/{user_id}/avatar", get(user_avatar))
}

async fn current_user(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<CurrentUserResponse>, AppError> {
    let auth_context = auth::build_auth_context(&state, &headers).await?;
    Ok(Json(CurrentUserResponse {
        session_expires_at: Some(auth::exp_to_timestamp(auth_context.access_token_exp)),
        user: auth::auth_context_to_user_response(&auth_context, &state.storage),
    }))
}

async fn update_my_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Json<CurrentUserResponse>, AppError> {
    let auth_context = auth::build_auth_context(&state, &headers).await?;
    let user_id = auth_context.user.id.clone();

    let mut form_display_name: Option<String> = None;
    let mut avatar_file: Option<(String, bytes::Bytes)> = None;
    let mut clear_avatar = false;

    while let Some(field) = multipart.next_field().await? {
        match field.name() {
            Some("display_name") => {
                let v = field.text().await?.trim().to_owned();
                form_display_name = Some(v);
            }
            Some("clear_avatar") => {
                let v = field.text().await?;
                clear_avatar = v.trim() == "1" || v.trim().eq_ignore_ascii_case("true");
            }
            Some("avatar_file") => {
                let content_type = field
                    .content_type()
                    .map(|s| s.to_owned())
                    .unwrap_or_else(|| "image/jpeg".to_owned());
                avatar_file = Some((content_type, field.bytes().await?));
            }
            _ => {}
        }
    }

    let existing = auth::find_user_by_id(&state.db_rw, &user_id)
        .await?
        .ok_or_else(|| AppError::not_found("User not found"))?;

    let new_display_name: Option<String> = if let Some(ref v) = form_display_name {
        if v.is_empty() { None } else { Some(v.clone()) }
    } else {
        existing.display_name.clone()
    };

    let new_avatar_key = if clear_avatar {
        None
    } else if let Some((content_type, avatar_bytes)) = avatar_file {
        if !avatar_bytes.is_empty() {
            let ext = match content_type.as_str() {
                "image/png" => "png",
                "image/gif" => "gif",
                "image/webp" => "webp",
                _ => "jpg",
            };
            let key = format!("avatars/{}.{}", user_id, ext);
            state
                .storage
                .upload_bytes(&key, avatar_bytes, &content_type)
                .await?;
            Some(key)
        } else {
            existing.avatar_image_key.clone()
        }
    } else {
        existing.avatar_image_key.clone()
    };

    sqlx::query("UPDATE users SET display_name = $1, avatar_image_key = $2 WHERE id = $3")
        .bind(&new_display_name)
        .bind(&new_avatar_key)
        .bind(&user_id)
        .execute(&state.db_rw)
        .await?;

    // Re-build a fresh auth context so the response has updated fields
    let updated = auth::find_user_by_id(&state.db_rw, &user_id)
        .await?
        .ok_or_else(|| AppError::not_found("User not found"))?;
    let user_response =
        auth::user_record_to_response(&state.db_rw, &state.storage, updated).await?;

    Ok(Json(CurrentUserResponse {
        session_expires_at: Some(auth::exp_to_timestamp(auth_context.access_token_exp)),
        user: user_response,
    }))
}

async fn user_avatar(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
) -> Result<Response, AppError> {
    let key = auth::find_user_by_id(&state.db_rw, &user_id)
        .await?
        .and_then(|u| u.avatar_image_key)
        .ok_or_else(|| AppError::not_found("Avatar not found"))?;

    let local_path = state
        .storage
        .local_path_for_key(&key)
        .ok_or_else(|| AppError::not_found("Avatar not found (S3 only exposes public URLs)"))?;

    let bytes = tokio::fs::read(&local_path)
        .await
        .map_err(|_| AppError::not_found("Avatar not found"))?;

    let ext = local_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("jpg");
    let content_type = match ext {
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => "image/jpeg",
    };

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "public, max-age=86400")
        .body(Body::from(bytes))
        .unwrap())
}

async fn current_user_library(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<UserLibraryResponse>, AppError> {
    let auth_context = auth::build_auth_context(&state, &headers).await?;
    let ensemble_names = music::fetch_ensemble_summaries(&state.db_rw).await?;
    let music_entries = if auth_context.has_global_power() {
        music::find_all_accessible_music(&state.db_rw).await?
    } else {
        music::find_accessible_music_for_user(&state.db_rw, &auth_context.user.id).await?
    };

    let mut ensembles: Vec<UserLibraryEnsembleResponse> = if auth_context.has_global_power() {
        let mut all_ensembles = ensemble_names
            .iter()
            .map(|(id, name)| UserLibraryEnsembleResponse {
                id: id.clone(),
                name: name.clone(),
                scores: Vec::new(),
            })
            .collect::<Vec<_>>();
        all_ensembles.sort_by(|left, right| left.name.cmp(&right.name));
        all_ensembles
    } else {
        let mut user_ensembles = music::fetch_user_ensemble_memberships(&state.db_rw)
            .await?
            .into_iter()
            .filter(|membership| membership.user_id == auth_context.user.id)
            .filter_map(|membership| {
                ensemble_names
                    .get(&membership.ensemble_id)
                    .cloned()
                    .map(|name| UserLibraryEnsembleResponse {
                        id: membership.ensemble_id,
                        name,
                        scores: Vec::new(),
                    })
            })
            .collect::<Vec<_>>();
        user_ensembles.sort_by(|left, right| left.name.cmp(&right.name));
        user_ensembles
    };

    for (music_record, ensemble_id, ensemble_name) in music_entries {
        let public_id_url = music_record
            .public_id
            .as_ref()
            .map(|public_id| state.config.public_url_for(public_id));
        let public_url = public_id_url
            .clone()
            .unwrap_or_else(|| state.config.public_url_for(&music_record.public_token));
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
            public_url,
            public_id_url,
            created_at: music_record.created_at,
        };

        if let Some(ensemble) = ensembles
            .iter_mut()
            .find(|ensemble| ensemble.id == ensemble_id)
        {
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
