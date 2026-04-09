use crate::models::{EnsembleRecord, MusicRecord, UserRecord};
use crate::schemas::{
    AdminEnsembleResponse, AdminMusicResponse, CreateEnsembleRequest, CreateUserRequest,
    EnsembleMemberResponse, LoginLinkResponse, MoveMusicRequest, UpdateEnsembleMemberRequest,
    UpdateMusicRequest, UserResponse,
};
use crate::services::{auth, music};
use crate::{
    AppError, AppRole, AppState, audio, ensure_membership_entities_exist,
    generate_public_token, normalize_music_icon, normalize_name, normalize_public_id,
    normalize_username, parse_quality_profile, sanitize_filename, utc_now_string,
};
use axum::{
    Json, Router,
    extract::{Multipart, Path, State},
    http::{HeaderMap, StatusCode},
    routing::{delete, get, patch, post},
};
use bytes::Bytes;
use std::collections::HashMap;
use tokio::fs;
use uuid::Uuid;

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/admin/users",
            get(admin_list_users).post(admin_create_user),
        )
        .route("/admin/users/{id}", delete(admin_delete_user))
        .route(
            "/admin/users/{id}/login-link",
            post(admin_create_user_login_link),
        )
        .route(
            "/admin/ensembles",
            get(admin_list_ensembles).post(admin_create_ensemble),
        )
        .route("/admin/ensembles/{id}", delete(admin_delete_ensemble))
        .route(
            "/admin/ensembles/{id}/users/{user_id}",
            post(admin_add_user_to_ensemble).delete(admin_remove_user_from_ensemble),
        )
        .route(
            "/admin/musics",
            get(admin_list_musics).post(admin_upload_music),
        )
        .route("/admin/musics/{id}", patch(admin_update_music))
        .route("/admin/musics/{id}/move", post(admin_move_music))
        .route(
            "/admin/musics/{id}/ensembles/{ensemble_id}",
            post(admin_add_music_to_ensemble).delete(admin_remove_music_from_ensemble),
        )
        .route("/admin/musics/{id}/delete", post(admin_delete_music))
        .route("/admin/musics/{id}/retry", post(admin_retry_render))
}

async fn admin_list_users(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<UserResponse>>, AppError> {
    let auth = auth::require_admin_context(&state, &headers).await?;
    if !auth.can_list_users() {
        return Err(AppError::unauthorized(
            "You are not allowed to view all users",
        ));
    }

    let rows = sqlx::query_as::<_, UserRecord>(
        "SELECT id, username, created_at, is_superadmin, role, created_by_user_id FROM users ORDER BY username ASC",
    )
    .fetch_all(&state.db_rw)
    .await?;

    let mut users = Vec::with_capacity(rows.len());
    for row in rows {
        users.push(auth::user_record_to_response(&state.db_rw, row).await?);
    }

    Ok(Json(users))
}

async fn admin_create_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateUserRequest>,
) -> Result<Json<UserResponse>, AppError> {
    let auth_context = auth::require_admin_context(&state, &headers).await?;
    if !auth_context.can_create_users() {
        return Err(AppError::unauthorized(
            "You are not allowed to create users",
        ));
    }

    let username = normalize_username(&payload.username)?;
    let requested_role = auth::resolve_creatable_user_role(&auth_context, payload.role.as_deref())?;
    if auth::find_user_by_username(&state.db_rw, &username)
        .await?
        .is_some()
    {
        return Err(AppError::conflict("That username already exists"));
    }

    let record = UserRecord {
        id: Uuid::new_v4().to_string(),
        username,
        created_at: utc_now_string(),
        is_superadmin: requested_role == AppRole::Superadmin,
        role: requested_role.as_str().to_owned(),
        created_by_user_id: Some(auth_context.user.id.clone()),
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
    .execute(&state.db_rw)
    .await?;

    Ok(Json(auth::user_record_to_response(&state.db_rw, record).await?))
}

async fn admin_create_user_login_link(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<LoginLinkResponse>, AppError> {
    let auth_context = auth::require_admin_context(&state, &headers).await?;

    let user = auth::find_user_by_id(&state.db_rw, &id)
        .await?
        .ok_or_else(|| AppError::not_found("User not found"))?;
    auth::ensure_can_generate_login_link_for_user(&auth_context, &user)?;

    Ok(Json(
        auth::create_login_link(&state.db_rw, &state.config, &user.id).await?,
    ))
}

async fn admin_delete_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let auth_context = auth::require_admin_context(&state, &headers).await?;
    let user = auth::find_user_by_id(&state.db_rw, &id)
        .await?
        .ok_or_else(|| AppError::not_found("User not found"))?;
    auth::ensure_can_delete_user(&auth_context, &user)?;

    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(&id)
        .execute(&state.db_rw)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

async fn admin_list_ensembles(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<AdminEnsembleResponse>>, AppError> {
    let auth_context = auth::require_admin_context(&state, &headers).await?;

    let ensembles = sqlx::query_as::<_, EnsembleRecord>(
        "SELECT id, name, created_at, created_by_user_id FROM ensembles ORDER BY name ASC",
    )
    .fetch_all(&state.db_rw)
    .await?;
    let memberships = music::fetch_user_ensemble_memberships(&state.db_rw).await?;
    let score_counts = music::fetch_ensemble_score_counts(&state.db_rw).await?;
    let mut member_map: HashMap<String, Vec<EnsembleMemberResponse>> = HashMap::new();
    for membership in memberships {
        if !auth_context.can_manage_ensemble(&membership.ensemble_id) {
            continue;
        }
        member_map
            .entry(membership.ensemble_id.clone())
            .or_default()
            .push(EnsembleMemberResponse {
                user_id: membership.user_id,
                role: membership.role,
            });
    }
    let mut score_count_map: HashMap<String, i64> = score_counts.into_iter().collect();

    Ok(Json(
        ensembles
            .into_iter()
            .filter(|ensemble| {
                auth_context.has_global_power() || auth_context.can_edit_ensemble_scores(&ensemble.id)
            })
            .map(|ensemble| AdminEnsembleResponse {
                id: ensemble.id.clone(),
                name: ensemble.name,
                created_at: ensemble.created_at,
                members: member_map.remove(&ensemble.id).unwrap_or_default(),
                score_count: score_count_map.remove(&ensemble.id).unwrap_or(0),
                created_by_user_id: ensemble.created_by_user_id,
            })
            .collect(),
    ))
}

async fn admin_create_ensemble(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateEnsembleRequest>,
) -> Result<Json<AdminEnsembleResponse>, AppError> {
    let auth_context = auth::require_admin_context(&state, &headers).await?;
    if !auth_context.can_create_ensembles() {
        return Err(AppError::unauthorized(
            "You are not allowed to create ensembles",
        ));
    }

    let name = normalize_name(&payload.name, "Ensemble names", 2, 64)?;
    if music::find_ensemble_by_name(&state.db_rw, &name).await?.is_some() {
        return Err(AppError::conflict("That ensemble already exists"));
    }

    let record = EnsembleRecord {
        id: Uuid::new_v4().to_string(),
        name,
        created_at: utc_now_string(),
        created_by_user_id: Some(auth_context.user.id.clone()),
    };

    sqlx::query(
        "INSERT INTO ensembles (id, name, created_at, created_by_user_id) VALUES ($1, $2, $3, $4)",
    )
    .bind(&record.id)
    .bind(&record.name)
    .bind(&record.created_at)
    .bind(&record.created_by_user_id)
    .execute(&state.db_rw)
    .await?;

    if auth_context.role == AppRole::Manager {
        sqlx::query(
            "INSERT INTO user_ensemble_memberships (user_id, ensemble_id, role) VALUES ($1, $2, $3) ON CONFLICT (user_id, ensemble_id) DO UPDATE SET role = EXCLUDED.role",
        )
        .bind(&auth_context.user.id)
        .bind(&record.id)
        .bind(crate::EnsembleRole::Manager.as_str())
        .execute(&state.db_rw)
        .await?;
    }

    Ok(Json(AdminEnsembleResponse {
        id: record.id,
        name: record.name,
        created_at: record.created_at,
        members: Vec::new(),
        score_count: 0,
        created_by_user_id: record.created_by_user_id,
    }))
}

async fn admin_delete_ensemble(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let auth_context = auth::require_admin_context(&state, &headers).await?;

    let ensemble = music::find_ensemble_by_id(&state.db_rw, &id)
        .await?
        .ok_or_else(|| AppError::not_found("Ensemble not found"))?;
    auth::ensure_can_delete_ensemble(&auth_context, &ensemble)?;

    let orphan_music_ids = sqlx::query_scalar::<_, String>(
        r#"
        SELECT mel.music_id
        FROM music_ensemble_links mel
        WHERE mel.ensemble_id = $1
        GROUP BY mel.music_id
        HAVING COUNT(*) FILTER (WHERE mel.ensemble_id <> $1) = 0
        "#,
    )
    .bind(&id)
    .fetch_all(&state.db_rw)
    .await?;

    sqlx::query("DELETE FROM ensembles WHERE id = $1")
        .bind(&id)
        .execute(&state.db_rw)
        .await?;

    for music_id in orphan_music_ids {
        music::delete_music_record_and_assets(&state, &music_id).await?;
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn admin_add_user_to_ensemble(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((id, user_id)): Path<(String, String)>,
    Json(payload): Json<UpdateEnsembleMemberRequest>,
) -> Result<StatusCode, AppError> {
    let auth_context = auth::require_admin_context(&state, &headers).await?;
    let target_user = ensure_membership_entities_exist(&state.db_rw, &id, &user_id).await?;
    auth::ensure_can_manage_ensemble(&auth_context, &id)?;

    let role = auth::validate_target_membership_role(&auth_context, &target_user, payload.role.trim())?;

    sqlx::query(
        "INSERT INTO user_ensemble_memberships (user_id, ensemble_id, role) VALUES ($1, $2, $3) ON CONFLICT (user_id, ensemble_id) DO UPDATE SET role = EXCLUDED.role",
    )
    .bind(&user_id)
    .bind(&id)
    .bind(role.as_str())
    .execute(&state.db_rw)
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

async fn admin_remove_user_from_ensemble(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((id, user_id)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    let auth_context = auth::require_admin_context(&state, &headers).await?;
    auth::ensure_can_manage_ensemble(&auth_context, &id)?;
    let target_user = auth::find_user_by_id(&state.db_rw, &user_id)
        .await?
        .ok_or_else(|| AppError::not_found("User not found"))?;
    auth::ensure_can_remove_member_from_ensemble(&auth_context, &target_user)?;

    sqlx::query("DELETE FROM user_ensemble_memberships WHERE user_id = $1 AND ensemble_id = $2")
        .bind(&user_id)
        .bind(&id)
        .execute(&state.db_rw)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

async fn admin_add_music_to_ensemble(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((id, ensemble_id)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    let auth_context = auth::require_admin_context(&state, &headers).await?;
    music::ensure_music_and_ensemble_exist(&state.db_rw, &id, &ensemble_id).await?;
    music::ensure_can_manage_music_and_target_ensemble(&state.db_rw, &auth_context, &id, &ensemble_id)
        .await?;

    sqlx::query(
        "INSERT INTO music_ensemble_links (music_id, ensemble_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(&id)
    .bind(&ensemble_id)
    .execute(&state.db_rw)
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

async fn admin_remove_music_from_ensemble(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((id, ensemble_id)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    let auth_context = auth::require_admin_context(&state, &headers).await?;
    music::ensure_can_manage_music_and_target_ensemble(&state.db_rw, &auth_context, &id, &ensemble_id)
        .await?;

    let linked_ensemble_ids = music::fetch_music_ensemble_ids(&state.db_rw, &id).await?;
    if linked_ensemble_ids.len() <= 1 && linked_ensemble_ids.iter().any(|value| value == &ensemble_id)
    {
        return Err(AppError::bad_request(
            "A score must belong to at least one ensemble",
        ));
    }

    sqlx::query("DELETE FROM music_ensemble_links WHERE music_id = $1 AND ensemble_id = $2")
        .bind(&id)
        .bind(&ensemble_id)
        .execute(&state.db_rw)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

async fn admin_list_musics(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<AdminMusicResponse>>, AppError> {
    let auth_context = auth::require_admin_context(&state, &headers).await?;

    let rows = sqlx::query_as::<_, MusicRecord>(
        r#"
        SELECT id, title, icon, icon_image_key, filename, content_type, object_key, audio_object_key, audio_status, audio_error, midi_object_key, midi_status, midi_error, musicxml_object_key, musicxml_status, musicxml_error, stems_status, stems_error, public_token, public_id, quality_profile, created_at, owner_user_id
        FROM musics
        ORDER BY created_at DESC
        "#,
    )
    .fetch_all(&state.db_rw)
    .await?;

    let total_rows = sqlx::query_as::<_, (String, i64)>(
        "SELECT music_id, COALESCE(SUM(size_bytes), 0)::BIGINT AS total_bytes FROM stems GROUP BY music_id",
    )
    .fetch_all(&state.db_rw)
    .await?;
    let totals: HashMap<String, i64> = total_rows.into_iter().collect();
    let ensemble_names = music::fetch_ensemble_summaries(&state.db_rw).await?;
    let links = music::fetch_music_ensemble_links(&state.db_rw).await?;
    let (mut music_ensemble_ids, mut music_ensemble_names) =
        music::build_music_ensemble_maps(links, &ensemble_names);

    let mut visible_items = Vec::new();
    for record in rows {
        if music::can_view_music_in_control_room(&state.db_rw, &auth_context, &record.id).await? {
            let total = totals.get(&record.id).copied().unwrap_or(0);
            let ensemble_ids = music_ensemble_ids.remove(&record.id).unwrap_or_default();
            let ensemble_names = music_ensemble_names.remove(&record.id).unwrap_or_default();
            visible_items.push(music::record_to_admin_response(
                &state.config,
                &state.storage,
                record,
                total,
                ensemble_ids,
                ensemble_names,
            ));
        }
    }

    Ok(Json(visible_items))
}

async fn admin_upload_music(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Json<AdminMusicResponse>, AppError> {
    let auth_context = auth::require_admin_context(&state, &headers).await?;

    let mut title: Option<String> = None;
    let mut icon: Option<String> = None;
    let mut requested_public_id: Option<String> = None;
    let mut requested_quality_profile: Option<String> = None;
    let mut requested_ensemble_id: Option<String> = None;
    let mut icon_file: Option<(String, Bytes)> = None;
    let mut upload: Option<(String, String, Bytes)> = None;

    while let Some(field) = multipart.next_field().await? {
        match field.name() {
            Some("title") => title = Some(field.text().await?.trim().to_owned()),
            Some("icon") => icon = Some(field.text().await?.trim().to_owned()),
            Some("icon_file") => {
                icon_file = Some((
                    field
                        .content_type()
                        .map(ToOwned::to_owned)
                        .unwrap_or_else(|| "image/jpeg".to_owned()),
                    field.bytes().await?,
                ));
            }
            Some("public_id") => requested_public_id = Some(field.text().await?.trim().to_owned()),
            Some("quality_profile") => {
                requested_quality_profile = Some(field.text().await?.trim().to_owned())
            }
            Some("ensemble_id") => requested_ensemble_id = Some(field.text().await?.trim().to_owned()),
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
    let icon = normalize_music_icon(icon.as_deref())?;
    music::ensure_public_id_available(&state.db_rw, public_id.as_deref(), None).await?;
    let quality_profile = parse_quality_profile(requested_quality_profile.as_deref())?;
    let ensemble_id = requested_ensemble_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::bad_request("Choose an ensemble for this score"))?
        .to_owned();
    if music::find_ensemble_by_id(&state.db_rw, &ensemble_id)
        .await?
        .is_none()
    {
        return Err(AppError::not_found("Ensemble not found"));
    }
    auth::ensure_can_upload_to_ensemble(&auth_context, &ensemble_id)?;

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

    let icon_image_key: Option<String> = if let Some((icon_content_type, icon_bytes)) = icon_file {
        let ext = match icon_content_type.as_str() {
            "image/png" => "png",
            "image/gif" => "gif",
            "image/webp" => "webp",
            _ => "jpg",
        };
        let icon_key = format!("scores/{music_id}/icon.{ext}");
        state
            .storage
            .upload_bytes(&icon_key, icon_bytes, &icon_content_type)
            .await?;
        Some(icon_key)
    } else {
        None
    };

    let temp_dir = tempfile::tempdir()?;
    let temp_input_path = temp_dir.path().join(&safe_filename);
    fs::write(&temp_input_path, &bytes).await?;

    let (audio_outcome, midi_outcome, musicxml_outcome) = tokio::try_join!(
        async {
            audio::generate_audio(&state.config, &temp_input_path, temp_dir.path())
                .await
                .map_err(AppError::from)
        },
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

    let created_at = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO musics (id, title, icon, icon_image_key, filename, content_type, object_key, public_token, public_id, quality_profile, created_at, directory_id, owner_user_id)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
        "#,
    )
    .bind(&music_id)
    .bind(&resolved_title)
    .bind(&icon)
    .bind(&icon_image_key)
    .bind(&filename)
    .bind(&content_type)
    .bind(&object_key)
    .bind(&public_token)
    .bind(&public_id)
    .bind(quality_profile.as_str())
    .bind(&created_at)
    .bind(&ensemble_id)
    .bind(&auth_context.user.id)
    .execute(&state.db_rw)
    .await?;
    sqlx::query("INSERT INTO music_ensemble_links (music_id, ensemble_id) VALUES ($1, $2)")
        .bind(&music_id)
        .bind(&ensemble_id)
        .execute(&state.db_rw)
        .await?;

    let (
        (audio_object_key, audio_status, audio_error),
        (midi_object_key, midi_status, midi_error),
        (musicxml_object_key, musicxml_status, musicxml_error),
        (stems_status, stems_error),
    ) = tokio::try_join!(
        music::store_conversion(&state, &music_id, "audio", audio_outcome),
        music::store_conversion(&state, &music_id, "midi", midi_outcome),
        music::store_conversion(&state, &music_id, "musicxml", musicxml_outcome),
        music::store_stems(&state, &music_id, stem_results, stems_status, stems_error),
    )?;

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
        icon,
        icon_image_key: icon_image_key.clone(),
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
        owner_user_id: Some(auth_context.user.id.clone()),
    };

    let stems_total = music::fetch_stems_total(&state.db_rw, &record.id).await;
    let ensemble_name = music::find_ensemble_by_id(&state.db_rw, &ensemble_id)
        .await?
        .map(|ensemble| ensemble.name)
        .unwrap_or_else(|| ensemble_id.clone());
    Ok(Json(music::record_to_admin_response(
        &state.config,
        &state.storage,
        record,
        stems_total,
        vec![ensemble_id],
        vec![ensemble_name],
    )))
}

async fn admin_retry_render(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<AdminMusicResponse>, AppError> {
    let auth_context = auth::require_admin_context(&state, &headers).await?;

    let record = music::find_music_by_id(&state.db_rw, &id)
        .await?
        .ok_or_else(|| AppError::not_found("Music not found"))?;
    music::ensure_can_manage_music(&state.db_rw, &auth_context, &id).await?;
    let quality_profile = audio::StemQualityProfile::from_stored_or_default(&record.quality_profile);

    let (score_bytes, _, _) = state.storage.get_bytes(&record.object_key).await?;

    let safe_filename = sanitize_filename(&record.filename);
    let temp_dir = tempfile::tempdir()?;
    let temp_input_path = temp_dir.path().join(&safe_filename);
    fs::write(&temp_input_path, &score_bytes).await?;

    let (audio_outcome, midi_outcome, musicxml_outcome) = tokio::try_join!(
        async {
            audio::generate_audio(&state.config, &temp_input_path, temp_dir.path())
                .await
                .map_err(AppError::from)
        },
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
    let (audio_object_key, audio_status, audio_error) =
        music::store_conversion(&state, &id, "audio", audio_outcome).await?;
    let (midi_object_key, midi_status, midi_error) =
        music::store_conversion(&state, &id, "midi", midi_outcome).await?;
    let (musicxml_object_key, musicxml_status, musicxml_error) =
        music::store_conversion(&state, &id, "musicxml", musicxml_outcome).await?;

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
        music::store_stems(&state, &id, stem_results, stems_status, stems_error).await?;

    sqlx::query(
        "UPDATE musics SET \
         audio_object_key = $1, audio_status = $2, audio_error = $3, \
         midi_object_key = $4, midi_status = $5, midi_error = $6, \
         musicxml_object_key = $7, musicxml_status = $8, musicxml_error = $9, \
         stems_status = $10, stems_error = $11 WHERE id = $12",
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
    .bind(&id)
    .execute(&state.db_rw)
    .await?;

    let updated = music::find_music_by_id(&state.db_rw, &id)
        .await?
        .ok_or_else(|| AppError::not_found("Music not found"))?;

    let stems_total = music::fetch_stems_total(&state.db_rw, &id).await;
    let (ensemble_ids, ensemble_names) = music::ensemble_metadata_for_music(&state.db_rw, &id).await?;
    Ok(Json(music::record_to_admin_response(
        &state.config,
        &state.storage,
        updated,
        stems_total,
        ensemble_ids,
        ensemble_names,
    )))
}

async fn admin_update_music(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(payload): Json<UpdateMusicRequest>,
) -> Result<Json<AdminMusicResponse>, AppError> {
    let auth_context = auth::require_admin_context(&state, &headers).await?;

    let existing = music::find_music_by_id(&state.db_rw, &id)
        .await?
        .ok_or_else(|| AppError::not_found("Music not found"))?;
    music::ensure_can_manage_music(&state.db_rw, &auth_context, &id).await?;

    let title = match payload.title {
        Some(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return Err(AppError::bad_request("Score title cannot be empty"));
            }
            trimmed.to_owned()
        }
        None => existing.title,
    };
    let public_id = normalize_public_id(payload.public_id.as_deref())?;
    let icon = normalize_music_icon(payload.icon.as_deref())?;
    music::ensure_public_id_available(&state.db_rw, public_id.as_deref(), Some(&id)).await?;

    let update_result =
        sqlx::query("UPDATE musics SET title = $1, public_id = $2, icon = $3 WHERE id = $4")
            .bind(&title)
            .bind(&public_id)
            .bind(&icon)
            .bind(&id)
            .execute(&state.db_rw)
            .await?;

    if update_result.rows_affected() == 0 {
        return Err(AppError::not_found("Music not found"));
    }

    let record = music::find_music_by_id(&state.db_rw, &id)
        .await?
        .ok_or_else(|| AppError::not_found("Music not found"))?;

    let stems_total = music::fetch_stems_total(&state.db_rw, &id).await;
    let (ensemble_ids, ensemble_names) = music::ensemble_metadata_for_music(&state.db_rw, &id).await?;
    Ok(Json(music::record_to_admin_response(
        &state.config,
        &state.storage,
        record,
        stems_total,
        ensemble_ids,
        ensemble_names,
    )))
}

async fn admin_move_music(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(payload): Json<MoveMusicRequest>,
) -> Result<Json<AdminMusicResponse>, AppError> {
    let auth_context = auth::require_admin_context(&state, &headers).await?;

    let _existing = music::find_music_by_id(&state.db_rw, &id)
        .await?
        .ok_or_else(|| AppError::not_found("Music not found"))?;
    music::ensure_can_manage_music(&state.db_rw, &auth_context, &id).await?;

    let ensemble_id = payload.ensemble_id.trim();
    if ensemble_id.is_empty() {
        return Err(AppError::bad_request("Choose a target ensemble"));
    }
    auth::ensure_can_upload_to_ensemble(&auth_context, ensemble_id)?;
    if music::find_ensemble_by_id(&state.db_rw, ensemble_id)
        .await?
        .is_none()
    {
        return Err(AppError::not_found("Ensemble not found"));
    }
    sqlx::query("DELETE FROM music_ensemble_links WHERE music_id = $1")
        .bind(&id)
        .execute(&state.db_rw)
        .await?;
    sqlx::query("INSERT INTO music_ensemble_links (music_id, ensemble_id) VALUES ($1, $2)")
        .bind(&id)
        .bind(ensemble_id)
        .execute(&state.db_rw)
        .await?;
    sqlx::query("UPDATE musics SET directory_id = $1 WHERE id = $2")
        .bind(ensemble_id)
        .bind(&id)
        .execute(&state.db_rw)
        .await?;

    let record = music::find_music_by_id(&state.db_rw, &id)
        .await?
        .ok_or_else(|| AppError::not_found("Music not found"))?;
    let stems_total = music::fetch_stems_total(&state.db_rw, &id).await;
    let (ensemble_ids, ensemble_names) = music::ensemble_metadata_for_music(&state.db_rw, &id).await?;
    Ok(Json(music::record_to_admin_response(
        &state.config,
        &state.storage,
        record,
        stems_total,
        ensemble_ids,
        ensemble_names,
    )))
}

async fn admin_delete_music(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let auth_context = auth::require_admin_context(&state, &headers).await?;

    let record = music::find_music_by_id(&state.db_rw, &id)
        .await?
        .ok_or_else(|| AppError::not_found("Music not found"))?;
    auth::ensure_can_delete_music(&auth_context, &record)?;
    music::delete_music_record_and_assets(&state, &id).await?;

    Ok(StatusCode::NO_CONTENT)
}
