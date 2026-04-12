use crate::models::{EnsembleRecord, MusicRecord, UserEnsembleMembershipRecord, UserRecord};
use crate::schemas::{
    AdminEnsembleResponse, AdminMusicPlaytimeResponse, AdminMusicResponse,
    AdminUserMetadataResponse, CreateEnsembleRequest, CreateUserRequest, EnsembleMemberResponse,
    LoginLinkResponse, MoveMusicRequest, UpdateEnsembleMemberRequest, UpdateEnsembleMembersRequest,
    UpdateMusicEnsemblesRequest, UserResponse,
};
use crate::services::{auth, music};
use crate::{
    AppError, AppRole, AppState, EnsembleRole, audio, ensure_membership_entities_exist,
    generate_public_token, normalize_music_icon, normalize_name, normalize_public_id,
    normalize_username, parse_quality_profile, sanitize_filename, utc_now_string,
};
use axum::{
    Json, Router,
    extract::{Multipart, Path, State, multipart::Field},
    http::{HeaderMap, StatusCode},
    routing::{delete, get, patch, post},
};
use bytes::Bytes;
use std::collections::{HashMap, HashSet};
use tokio::fs;
use tracing::Instrument;
use uuid::Uuid;

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/admin/users",
            get(admin_list_users).post(admin_create_user),
        )
        .route(
            "/admin/users/{id}",
            patch(admin_update_user).delete(admin_delete_user),
        )
        .route(
            "/admin/users/{id}/login-link",
            post(admin_create_user_login_link),
        )
        .route("/admin/users/{id}/metadata", get(admin_user_metadata))
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
            "/admin/ensembles/{id}/users",
            patch(admin_update_ensemble_members),
        )
        .route(
            "/admin/musics",
            get(admin_list_musics).post(admin_upload_music),
        )
        .route("/admin/musics/{id}", patch(admin_update_music))
        .route("/admin/musics/{id}/playtime", get(admin_music_playtime))
        .route("/admin/musics/{id}/move", post(admin_move_music))
        .route(
            "/admin/musics/{id}/ensembles/{ensemble_id}",
            post(admin_add_music_to_ensemble).delete(admin_remove_music_from_ensemble),
        )
        .route(
            "/admin/musics/{id}/ensembles",
            patch(admin_update_music_ensembles),
        )
        .route("/admin/musics/{id}/delete", post(admin_delete_music))
        .route("/admin/musics/{id}/retry", post(admin_retry_render))
}

async fn read_field_bytes_with_progress(
    field: &mut Field<'_>,
    field_name: &str,
    file_name: Option<&str>,
) -> Result<Bytes, AppError> {
    const LOG_STEP_BYTES: usize = 1024 * 1024;

    let mut data = Vec::new();
    let mut total_bytes = 0usize;
    let mut next_log_at = LOG_STEP_BYTES;

    while let Some(chunk) = field.chunk().await? {
        total_bytes += chunk.len();
        data.extend_from_slice(&chunk);

        if total_bytes >= next_log_at {
            tracing::info!(
                field = field_name,
                file_name = file_name.unwrap_or(""),
                bytes_read = total_bytes,
                mib_read = total_bytes / LOG_STEP_BYTES,
                "score upload progress"
            );
            next_log_at += LOG_STEP_BYTES;
        }
    }

    tracing::info!(
        field = field_name,
        file_name = file_name.unwrap_or(""),
        bytes_read = total_bytes,
        "score upload field fully read"
    );

    Ok(Bytes::from(data))
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
        "SELECT id, username, display_name, avatar_image_key, created_at, is_superadmin, role, created_by_user_id FROM users ORDER BY username ASC",
    )
    .fetch_all(&state.db_rw)
    .await?;

    let mut users = Vec::with_capacity(rows.len());
    for row in rows {
        users.push(auth::user_record_to_response(&state.db_rw, &state.storage, row).await?);
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
        display_name: None,
        avatar_image_key: None,
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

    Ok(Json(
        auth::user_record_to_response(&state.db_rw, &state.storage, record).await?,
    ))
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

async fn admin_user_metadata(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<AdminUserMetadataResponse>, AppError> {
    let auth_context = auth::require_admin_context(&state, &headers).await?;
    if !auth_context.has_global_power() {
        return Err(AppError::unauthorized("Only admins can view user metadata"));
    }

    let user = auth::find_user_by_id(&state.db_rw, &id)
        .await?
        .ok_or_else(|| AppError::not_found("User not found"))?;
    let last_login_at = auth::find_user_last_login_at(&state.db_rw, &user.id).await?;
    let (total_playtime_seconds, score_playtimes) =
        music::build_admin_user_metadata_playtime_response(
            &state.config,
            &state.storage,
            &state.db_rw,
            &user.id,
        )
        .await?;

    Ok(Json(AdminUserMetadataResponse {
        last_login_at,
        total_playtime_seconds,
        score_playtimes,
    }))
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

async fn admin_update_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    mut multipart: Multipart,
) -> Result<Json<UserResponse>, AppError> {
    let auth_context = auth::require_admin_context(&state, &headers).await?;

    let existing = auth::find_user_by_id(&state.db_rw, &id)
        .await?
        .ok_or_else(|| AppError::not_found("User not found"))?;

    // Only admin/superadmin can edit users; managers have no edit power
    if !auth_context.has_global_power() {
        return Err(AppError::unauthorized(
            "Only admins and superadmins can edit user profiles",
        ));
    }

    // Cannot edit the superadmin account (unless you are the superadmin editing themselves)
    let existing_role = auth::parse_global_role(&existing.role)?;
    if existing_role == AppRole::Superadmin && existing.id != auth_context.user.id {
        return Err(AppError::bad_request(
            "The superadmin account cannot be modified via this endpoint",
        ));
    }

    let mut form_display_name: Option<String> = None;
    let mut form_role: Option<String> = None;
    let mut avatar_file: Option<(String, bytes::Bytes)> = None;
    let mut clear_avatar = false;

    while let Some(field) = multipart.next_field().await? {
        match field.name() {
            Some("display_name") => {
                let v = field.text().await?.trim().to_owned();
                form_display_name = Some(v);
            }
            Some("role") => {
                form_role = Some(field.text().await?.trim().to_owned());
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

    // Resolve new role
    let new_role = if let Some(ref role_str) = form_role {
        let requested = auth::parse_global_role(role_str)?;
        // Only superadmin can promote to admin; admin can manage manager/editor/user
        match auth_context.role {
            AppRole::Superadmin => {
                if requested == AppRole::Superadmin {
                    return Err(AppError::bad_request(
                        "Cannot assign the superadmin role via this endpoint",
                    ));
                }
                requested
            }
            AppRole::Admin => match requested {
                AppRole::Admin | AppRole::Manager | AppRole::Editor | AppRole::User => requested,
                AppRole::Superadmin => {
                    return Err(AppError::bad_request(
                        "Admins cannot promote users to admin or superadmin",
                    ));
                }
            },
            _ => {
                return Err(AppError::unauthorized(
                    "Only admins and superadmins can change roles",
                ));
            }
        }
    } else {
        existing_role
    };

    // Resolve display name
    let new_display_name: Option<String> = if let Some(ref v) = form_display_name {
        if v.is_empty() { None } else { Some(v.clone()) }
    } else {
        existing.display_name.clone()
    };

    // Resolve avatar
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
            let key = format!("avatars/{}.{}", id, ext);
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

    sqlx::query(
        "UPDATE users SET display_name = $1, role = $2, is_superadmin = $3, avatar_image_key = $4 WHERE id = $5",
    )
    .bind(&new_display_name)
    .bind(new_role.as_str())
    .bind(new_role == AppRole::Superadmin)
    .bind(&new_avatar_key)
    .bind(&id)
    .execute(&state.db_rw)
    .await?;

    let updated = auth::find_user_by_id(&state.db_rw, &id)
        .await?
        .ok_or_else(|| AppError::not_found("User not found"))?;

    Ok(Json(
        auth::user_record_to_response(&state.db_rw, &state.storage, updated).await?,
    ))
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
                auth_context.has_global_power()
                    || auth_context.can_edit_ensemble_scores(&ensemble.id)
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
    if music::find_ensemble_by_name(&state.db_rw, &name)
        .await?
        .is_some()
    {
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

    let role =
        auth::validate_target_membership_role(&auth_context, &target_user, payload.role.trim())?;

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

async fn admin_update_ensemble_members(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(payload): Json<UpdateEnsembleMembersRequest>,
) -> Result<StatusCode, AppError> {
    let auth_context = auth::require_admin_context(&state, &headers).await?;
    auth::ensure_can_manage_ensemble(&auth_context, &id)?;
    music::find_ensemble_by_id(&state.db_rw, &id)
        .await?
        .ok_or_else(|| AppError::not_found("Ensemble not found"))?;

    let current_members = sqlx::query_as::<_, UserEnsembleMembershipRecord>(
        "SELECT user_id, ensemble_id, role FROM user_ensemble_memberships WHERE ensemble_id = $1",
    )
    .bind(&id)
    .fetch_all(&state.db_rw)
    .await?;

    let mut desired_members: HashMap<String, EnsembleRole> = HashMap::new();
    for member in payload.members {
        let target_user = auth::find_user_by_id(&state.db_rw, &member.user_id)
            .await?
            .ok_or_else(|| AppError::not_found("User not found"))?;
        let role =
            auth::validate_target_membership_role(&auth_context, &target_user, member.role.trim())?;
        desired_members.insert(target_user.id, role);
    }

    let current_member_map: HashMap<String, String> = current_members
        .into_iter()
        .map(|membership| (membership.user_id, membership.role))
        .collect();

    let mut to_remove: Vec<String> = current_member_map
        .keys()
        .filter(|user_id| !desired_members.contains_key(*user_id))
        .cloned()
        .collect();
    to_remove.sort();

    for user_id in &to_remove {
        let target_user = auth::find_user_by_id(&state.db_rw, user_id)
            .await?
            .ok_or_else(|| AppError::not_found("User not found"))?;
        auth::ensure_can_remove_member_from_ensemble(&auth_context, &target_user)?;
    }

    let mut to_upsert: Vec<(String, EnsembleRole)> = desired_members.into_iter().collect();
    to_upsert.sort_by(|left, right| left.0.cmp(&right.0));

    let mut tx = state.db_rw.begin().await?;
    for (user_id, role) in to_upsert {
        sqlx::query(
            "INSERT INTO user_ensemble_memberships (user_id, ensemble_id, role) VALUES ($1, $2, $3) ON CONFLICT (user_id, ensemble_id) DO UPDATE SET role = EXCLUDED.role",
        )
        .bind(&user_id)
        .bind(&id)
        .bind(role.as_str())
        .execute(&mut *tx)
        .await?;
    }

    for user_id in to_remove {
        sqlx::query(
            "DELETE FROM user_ensemble_memberships WHERE user_id = $1 AND ensemble_id = $2",
        )
        .bind(&user_id)
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok(StatusCode::NO_CONTENT)
}

async fn admin_add_music_to_ensemble(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((id, ensemble_id)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    let auth_context = auth::require_admin_context(&state, &headers).await?;
    music::ensure_music_and_ensemble_exist(&state.db_rw, &id, &ensemble_id).await?;
    music::ensure_can_manage_music_and_target_ensemble(
        &state.db_rw,
        &auth_context,
        &id,
        &ensemble_id,
    )
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
    music::ensure_can_manage_music_and_target_ensemble(
        &state.db_rw,
        &auth_context,
        &id,
        &ensemble_id,
    )
    .await?;

    let linked_ensemble_ids = music::fetch_music_ensemble_ids(&state.db_rw, &id).await?;
    if linked_ensemble_ids.len() <= 1
        && linked_ensemble_ids
            .iter()
            .any(|value| value == &ensemble_id)
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

async fn admin_update_music_ensembles(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(payload): Json<UpdateMusicEnsemblesRequest>,
) -> Result<StatusCode, AppError> {
    let auth_context = auth::require_admin_context(&state, &headers).await?;

    music::find_music_by_id(&state.db_rw, &id)
        .await?
        .ok_or_else(|| AppError::not_found("Music not found"))?;

    let mut desired_ensemble_ids = Vec::new();
    let mut desired_seen = HashSet::new();
    for ensemble_id in payload.ensemble_ids {
        let ensemble_id = ensemble_id.trim().to_owned();
        if ensemble_id.is_empty() || !desired_seen.insert(ensemble_id.clone()) {
            continue;
        }
        music::find_ensemble_by_id(&state.db_rw, &ensemble_id)
            .await?
            .ok_or_else(|| AppError::not_found("Ensemble not found"))?;
        music::ensure_can_manage_music_and_target_ensemble(
            &state.db_rw,
            &auth_context,
            &id,
            &ensemble_id,
        )
        .await?;
        desired_ensemble_ids.push(ensemble_id);
    }

    if desired_ensemble_ids.is_empty() {
        return Err(AppError::bad_request(
            "A score must belong to at least one ensemble",
        ));
    }

    let current_ensemble_ids = music::fetch_music_ensemble_ids(&state.db_rw, &id).await?;
    let current_seen: HashSet<String> = current_ensemble_ids.iter().cloned().collect();
    let desired_seen: HashSet<String> = desired_ensemble_ids.iter().cloned().collect();

    let mut to_remove: Vec<String> = current_ensemble_ids
        .into_iter()
        .filter(|ensemble_id| !desired_seen.contains(ensemble_id))
        .collect();
    to_remove.sort();

    for ensemble_id in &to_remove {
        music::ensure_can_manage_music_and_target_ensemble(
            &state.db_rw,
            &auth_context,
            &id,
            ensemble_id,
        )
        .await?;
    }

    let mut to_add: Vec<String> = desired_ensemble_ids
        .into_iter()
        .filter(|ensemble_id| !current_seen.contains(ensemble_id))
        .collect();
    to_add.sort();

    let mut tx = state.db_rw.begin().await?;
    for ensemble_id in to_add {
        sqlx::query(
            "INSERT INTO music_ensemble_links (music_id, ensemble_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        )
        .bind(&id)
        .bind(&ensemble_id)
        .execute(&mut *tx)
        .await?;
    }

    for ensemble_id in to_remove {
        sqlx::query("DELETE FROM music_ensemble_links WHERE music_id = $1 AND ensemble_id = $2")
            .bind(&id)
            .bind(&ensemble_id)
            .execute(&mut *tx)
            .await?;
    }

    tx.commit().await?;

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

#[tracing::instrument(skip(state, headers, multipart))]
async fn admin_upload_music(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Json<AdminMusicResponse>, AppError> {
    tracing::info!("score upload request received");
    let auth_context = auth::require_admin_context(&state, &headers).await?;
    tracing::info!(user_id = %auth_context.user.id, username = %auth_context.user.username, "score upload authorized");

    let mut title: Option<String> = None;
    let mut icon: Option<String> = None;
    let mut requested_public_id: Option<String> = None;
    let mut requested_quality_profile: Option<String> = None;
    let mut requested_ensemble_ids: Vec<String> = Vec::new();
    let mut icon_file: Option<(String, Bytes)> = None;
    let mut upload: Option<(String, String, Bytes)> = None;

    while let Some(mut field) = multipart.next_field().await? {
        let field_name = field.name().map(str::to_owned);
        tracing::info!(
            field = field_name.as_deref().unwrap_or("<unnamed>"),
            "score upload field received"
        );

        match field_name.as_deref() {
            Some("title") => title = Some(field.text().await?.trim().to_owned()),
            Some("icon") => icon = Some(field.text().await?.trim().to_owned()),
            Some("icon_file") => {
                let content_type = field
                    .content_type()
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| "image/jpeg".to_owned());
                let icon_file_name = field.file_name().map(ToOwned::to_owned);
                let icon_bytes = read_field_bytes_with_progress(
                    &mut field,
                    "icon_file",
                    icon_file_name.as_deref(),
                )
                .await?;
                tracing::info!(bytes = icon_bytes.len(), content_type = %content_type, "score upload icon parsed");
                icon_file = Some((content_type, icon_bytes));
            }
            Some("public_id") => requested_public_id = Some(field.text().await?.trim().to_owned()),
            Some("quality_profile") => {
                requested_quality_profile = Some(field.text().await?.trim().to_owned())
            }
            Some("ensemble_id") => {
                let ensemble_id = field.text().await?.trim().to_owned();
                if !ensemble_id.is_empty() {
                    requested_ensemble_ids.push(ensemble_id);
                }
            }
            Some("file") => {
                let filename = field.file_name().map(ToOwned::to_owned).ok_or_else(|| {
                    AppError::bad_request("The uploaded file is missing a filename")
                })?;
                let content_type = field
                    .content_type()
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| "application/octet-stream".to_owned());
                let file_bytes =
                    read_field_bytes_with_progress(&mut field, "file", Some(&filename)).await?;
                tracing::info!(filename = %filename, bytes = file_bytes.len(), content_type = %content_type, "score upload file parsed");
                upload = Some((filename, content_type, file_bytes));
            }
            _ => {}
        }
    }

    tracing::info!("score upload multipart parsing completed");

    let (filename, content_type, bytes) =
        upload.ok_or_else(|| AppError::bad_request("Please attach an .mscz file"))?;
    if !filename.to_lowercase().ends_with(".mscz") {
        return Err(AppError::bad_request("Only .mscz uploads are supported"));
    }

    let public_id = normalize_public_id(requested_public_id.as_deref())?;
    let icon = normalize_music_icon(icon.as_deref())?;
    music::ensure_public_id_available(&state.db_rw, public_id.as_deref(), None).await?;
    let quality_profile = parse_quality_profile(requested_quality_profile.as_deref())?;
    if requested_ensemble_ids.is_empty() {
        return Err(AppError::bad_request("Choose an ensemble for this score"));
    }

    let mut ensemble_ids = Vec::new();
    for ensemble_id in requested_ensemble_ids {
        if !ensemble_ids.contains(&ensemble_id) {
            if music::find_ensemble_by_id(&state.db_rw, &ensemble_id)
                .await?
                .is_none()
            {
                return Err(AppError::not_found("Ensemble not found"));
            }
            auth::ensure_can_upload_to_ensemble(&auth_context, &ensemble_id)?;
            ensemble_ids.push(ensemble_id);
        }
    }

    let primary_ensemble_id = ensemble_ids
        .first()
        .cloned()
        .ok_or_else(|| AppError::bad_request("Choose an ensemble for this score"))?;

    tracing::info!(ensemble_count = ensemble_ids.len(), quality_profile = %quality_profile.as_str(), filename = %filename, bytes = bytes.len(), "score upload validated");

    let music_id = Uuid::new_v4().to_string();
    let public_token = generate_public_token();
    let resolved_title = title
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| filename.trim_end_matches(".mscz").to_owned());
    let safe_filename = sanitize_filename(&filename);
    let object_key = format!("scores/{music_id}/{safe_filename}");

    tracing::info!(music_id = %music_id, storage_key = %object_key, "score upload storing source file");
    state
        .storage
        .upload_bytes(&object_key, bytes.clone(), &content_type)
        .await?;
    tracing::info!(music_id = %music_id, storage_key = %object_key, "score upload stored source file");

    let icon_image_key: Option<String> = if let Some((icon_content_type, icon_bytes)) = icon_file {
        let ext = match icon_content_type.as_str() {
            "image/png" => "png",
            "image/gif" => "gif",
            "image/webp" => "webp",
            _ => "jpg",
        };
        let icon_key = format!("scores/{music_id}/icon.{ext}");
        tracing::info!(music_id = %music_id, storage_key = %icon_key, bytes = icon_bytes.len(), content_type = %icon_content_type, "score upload storing icon file");
        state
            .storage
            .upload_bytes(&icon_key, icon_bytes, &icon_content_type)
            .await?;
        tracing::info!(music_id = %music_id, storage_key = %icon_key, "score upload stored icon file");
        Some(icon_key)
    } else {
        None
    };

    let created_at = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO musics (
            id, title, icon, icon_image_key, filename, content_type, object_key,
            audio_object_key, audio_status, audio_error,
            midi_object_key, midi_status, midi_error,
            musicxml_object_key, musicxml_status, musicxml_error,
            stems_status, stems_error,
            public_token, public_id, quality_profile, created_at, directory_id, owner_user_id
        )
        VALUES (
            $1, $2, $3, $4, $5, $6, $7,
            NULL, 'processing', NULL,
            NULL, 'processing', NULL,
            NULL, 'processing', NULL,
            'processing', NULL,
            $8, $9, $10, $11, $12, $13
        )
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
    .bind(&primary_ensemble_id)
    .bind(&auth_context.user.id)
    .execute(&state.db_rw)
    .await?;
    for ensemble_id in &ensemble_ids {
        sqlx::query(
            "INSERT INTO music_ensemble_links (music_id, ensemble_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        )
        .bind(&music_id)
        .bind(ensemble_id)
        .execute(&state.db_rw)
        .await?;
    }

    let ensemble_name_map = music::fetch_ensemble_summaries(&state.db_rw).await?;
    let ensemble_names = ensemble_ids
        .iter()
        .filter_map(|ensemble_id| ensemble_name_map.get(ensemble_id).cloned())
        .collect::<Vec<_>>();

    let record = MusicRecord {
        id: music_id.clone(),
        title: resolved_title.clone(),
        icon: icon.clone(),
        icon_image_key: icon_image_key.clone(),
        filename: filename.clone(),
        content_type: content_type.clone(),
        object_key: object_key.clone(),
        audio_object_key: None,
        audio_status: "processing".to_owned(),
        audio_error: None,
        midi_object_key: None,
        midi_status: "processing".to_owned(),
        midi_error: None,
        musicxml_object_key: None,
        musicxml_status: "processing".to_owned(),
        musicxml_error: None,
        stems_status: "processing".to_owned(),
        stems_error: None,
        public_token: public_token.clone(),
        public_id: public_id.clone(),
        quality_profile: quality_profile.as_str().to_owned(),
        created_at: created_at.clone(),
        owner_user_id: Some(auth_context.user.id.clone()),
    };

    let render_state = state.clone();
    let failure_state = state.clone();
    let render_music_id = music_id.clone();
    let render_quality_profile = quality_profile;
    let render_bytes = bytes;
    let render_filename = safe_filename.clone();
    tokio::spawn(
        async move {
            if let Err(error) = process_uploaded_music(
                render_state,
                render_music_id.clone(),
                render_quality_profile,
                render_bytes,
                render_filename,
            )
            .await
            {
                tracing::error!(music_id = %render_music_id, error = ?error, "score upload processing failed");
                if let Err(mark_error) =
                    mark_music_processing_failed(&failure_state, &render_music_id, error.message.clone()).await
                {
                    tracing::error!(
                        music_id = %render_music_id,
                        error = ?mark_error,
                        "failed to mark score upload as failed"
                    );
                }
            }
        }
        .in_current_span(),
    );

    Ok(Json(music::record_to_admin_response(
        &state.config,
        &state.storage,
        record,
        0,
        ensemble_ids,
        ensemble_names,
    )))
}

#[tracing::instrument(
    skip(state, bytes),
    fields(music_id = %music_id, quality_profile = %quality_profile.as_str(), filename = %safe_filename)
)]
async fn process_uploaded_music(
    state: AppState,
    music_id: String,
    quality_profile: audio::StemQualityProfile,
    bytes: Bytes,
    safe_filename: String,
) -> Result<(), AppError> {
    let temp_dir = tempfile::tempdir()?;
    let temp_input_path = temp_dir.path().join(&safe_filename);
    tracing::info!(music_id = %music_id, path = %temp_input_path.display(), "score upload writing temporary input file");
    fs::write(&temp_input_path, &bytes).await?;
    tracing::info!(music_id = %music_id, path = %temp_input_path.display(), "score upload wrote temporary input file");

    tracing::info!(music_id = %music_id, "score upload starting conversion pipeline");
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

    tracing::info!(music_id = %music_id, "score upload processing completed");
    Ok(())
}

#[tracing::instrument(skip(state, error), fields(music_id = %music_id))]
async fn mark_music_processing_failed(
    state: &AppState,
    music_id: &str,
    error: String,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        UPDATE musics SET
            audio_status = 'failed', audio_error = $1,
            midi_status = 'failed', midi_error = $1,
            musicxml_status = 'failed', musicxml_error = $1,
            stems_status = 'failed', stems_error = $1
        WHERE id = $2
        "#,
    )
    .bind(&error)
    .bind(music_id)
    .execute(&state.db_rw)
    .await?;

    Ok(())
}

#[tracing::instrument(skip(state, headers), fields(music_id = %id))]
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
    let quality_profile =
        audio::StemQualityProfile::from_stored_or_default(&record.quality_profile);

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
    let (ensemble_ids, ensemble_names) =
        music::ensemble_metadata_for_music(&state.db_rw, &id).await?;
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
    mut multipart: Multipart,
) -> Result<Json<AdminMusicResponse>, AppError> {
    let auth_context = auth::require_admin_context(&state, &headers).await?;

    let existing = music::find_music_by_id(&state.db_rw, &id)
        .await?
        .ok_or_else(|| AppError::not_found("Music not found"))?;
    music::ensure_can_manage_music(&state.db_rw, &auth_context, &id).await?;

    let mut form_title: Option<String> = None;
    let mut form_icon: Option<String> = None;
    let mut form_public_id: Option<String> = None;
    let mut icon_file: Option<(String, Bytes)> = None;

    while let Some(field) = multipart.next_field().await? {
        match field.name() {
            Some("title") => form_title = Some(field.text().await?.trim().to_owned()),
            Some("icon") => form_icon = Some(field.text().await?.trim().to_owned()),
            Some("public_id") => form_public_id = Some(field.text().await?.trim().to_owned()),
            Some("icon_file") => {
                icon_file = Some((
                    field
                        .content_type()
                        .map(ToOwned::to_owned)
                        .unwrap_or_else(|| "image/jpeg".to_owned()),
                    field.bytes().await?,
                ));
            }
            _ => {}
        }
    }

    let title = match form_title {
        Some(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return Err(AppError::bad_request("Score title cannot be empty"));
            }
            trimmed.to_owned()
        }
        None => existing.title,
    };
    let public_id = normalize_public_id(form_public_id.as_deref())?;
    let icon = normalize_music_icon(form_icon.as_deref())?;
    music::ensure_public_id_available(&state.db_rw, public_id.as_deref(), Some(&id)).await?;

    let icon_image_key: Option<String> = if let Some((icon_content_type, icon_bytes)) = icon_file {
        if !icon_bytes.is_empty() {
            let ext = match icon_content_type.as_str() {
                "image/png" => "png",
                "image/gif" => "gif",
                "image/webp" => "webp",
                _ => "jpg",
            };
            let icon_key = format!("scores/{id}/icon.{ext}");
            state
                .storage
                .upload_bytes(&icon_key, icon_bytes, &icon_content_type)
                .await?;
            Some(icon_key)
        } else {
            existing.icon_image_key.clone()
        }
    } else {
        existing.icon_image_key.clone()
    };

    let update_result = sqlx::query(
        "UPDATE musics SET title = $1, public_id = $2, icon = $3, icon_image_key = $4 WHERE id = $5",
    )
    .bind(&title)
    .bind(&public_id)
    .bind(&icon)
    .bind(&icon_image_key)
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
    let (ensemble_ids, ensemble_names) =
        music::ensemble_metadata_for_music(&state.db_rw, &id).await?;
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
    let (ensemble_ids, ensemble_names) =
        music::ensemble_metadata_for_music(&state.db_rw, &id).await?;
    Ok(Json(music::record_to_admin_response(
        &state.config,
        &state.storage,
        record,
        stems_total,
        ensemble_ids,
        ensemble_names,
    )))
}

async fn admin_music_playtime(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<AdminMusicPlaytimeResponse>, AppError> {
    let auth_context = auth::require_admin_context(&state, &headers).await?;
    if !music::can_view_music_in_control_room(&state.db_rw, &auth_context, &id).await? {
        return Err(AppError::unauthorized(
            "You are not allowed to view playtime for this score",
        ));
    }

    music::find_music_by_id(&state.db_rw, &id)
        .await?
        .ok_or_else(|| AppError::not_found("Music not found"))?;

    Ok(Json(
        music::build_admin_music_playtime_response(&state.db_rw, &state.storage, &id).await?,
    ))
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
