use crate::models::{
    EnsembleRecord, MusicEnsembleLinkRecord, MusicRecord, NewEnsemble, NewMusic,
    NewMusicEnsembleLink, NewUser, NewUserEnsembleMembership, UpdateAdminUser,
    UpdateMusicDirectory, UpdateMusicMetadata, UpdateMusicProcessing, UserEnsembleMembershipRecord,
    UserRecord,
};
use crate::schema::{
    ensembles, music_ensemble_links, musics, stems, user_ensemble_memberships, users,
};
use crate::schemas::{
    AdminEnsembleResponse, AdminMusicPlaytimeResponse, AdminMusicResponse,
    AdminUpdateMusicMultipartRequest, AdminUpdateUserMultipartRequest,
    AdminUploadMusicMultipartRequest, AdminUserMetadataResponse, CreateEnsembleRequest,
    CreateUserRequest, EnsembleMemberResponse, ErrorResponse, LoginLinkResponse, MoveMusicRequest,
    UpdateEnsembleMemberRequest, UpdateEnsembleMembersRequest, UpdateMusicEnsemblesRequest,
    UserResponse,
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
};
use bytes::Bytes;
use diesel::QueryableByName;
use diesel::prelude::*;
use diesel::sql_types::{BigInt, Text};
use diesel::upsert::excluded;
use diesel_async::{AsyncConnection, RunQueryDsl};
use std::collections::{HashMap, HashSet};
use tokio::fs;
use tracing::Instrument;
use uuid::Uuid;

#[derive(QueryableByName)]
struct MusicTotalBytesRow {
    #[diesel(sql_type = Text)]
    music_id: String,
    #[diesel(sql_type = BigInt)]
    total_bytes: i64,
}

pub(super) fn routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route(
            "/admin/users",
            crate::op_get!(state, "/admin/users", admin_list_users),
        )
        .route(
            "/admin/users",
            crate::op_post!(state, "/admin/users", admin_create_user),
        )
        .route(
            "/admin/users/{id}",
            crate::op_patch!(state, "/admin/users/{id}", admin_update_user),
        )
        .route(
            "/admin/users/{id}",
            crate::op_delete!(state, "/admin/users/{id}", admin_delete_user),
        )
        .route(
            "/admin/users/{id}/login-link",
            crate::op_post!(
                state,
                "/admin/users/{id}/login-link",
                admin_create_user_login_link
            ),
        )
        .route(
            "/admin/users/{id}/metadata",
            crate::op_get!(state, "/admin/users/{id}/metadata", admin_user_metadata),
        )
        .route(
            "/admin/ensembles",
            crate::op_get!(state, "/admin/ensembles", admin_list_ensembles),
        )
        .route(
            "/admin/ensembles",
            crate::op_post!(state, "/admin/ensembles", admin_create_ensemble),
        )
        .route(
            "/admin/ensembles/{id}",
            crate::op_delete!(state, "/admin/ensembles/{id}", admin_delete_ensemble),
        )
        .route(
            "/admin/ensembles/{id}/users/{user_id}",
            crate::op_post!(
                state,
                "/admin/ensembles/{id}/users/{user_id}",
                admin_add_user_to_ensemble
            ),
        )
        .route(
            "/admin/ensembles/{id}/users/{user_id}",
            crate::op_delete!(
                state,
                "/admin/ensembles/{id}/users/{user_id}",
                admin_remove_user_from_ensemble
            ),
        )
        .route(
            "/admin/ensembles/{id}/users",
            crate::op_patch!(
                state,
                "/admin/ensembles/{id}/users",
                admin_update_ensemble_members
            ),
        )
        .route(
            "/admin/musics",
            crate::op_get!(state, "/admin/musics", admin_list_musics),
        )
        .route(
            "/admin/musics",
            crate::op_post!(state, "/admin/musics", admin_upload_music),
        )
        .route(
            "/admin/musics/{id}",
            crate::op_patch!(state, "/admin/musics/{id}", admin_update_music),
        )
        .route(
            "/admin/musics/{id}/playtime",
            crate::op_get!(state, "/admin/musics/{id}/playtime", admin_music_playtime),
        )
        .route(
            "/admin/musics/{id}/move",
            crate::op_post!(state, "/admin/musics/{id}/move", admin_move_music),
        )
        .route(
            "/admin/musics/{id}/ensembles/{ensemble_id}",
            crate::op_post!(
                state,
                "/admin/musics/{id}/ensembles/{ensemble_id}",
                admin_add_music_to_ensemble
            ),
        )
        .route(
            "/admin/musics/{id}/ensembles/{ensemble_id}",
            crate::op_delete!(
                state,
                "/admin/musics/{id}/ensembles/{ensemble_id}",
                admin_remove_music_from_ensemble
            ),
        )
        .route(
            "/admin/musics/{id}/ensembles",
            crate::op_patch!(
                state,
                "/admin/musics/{id}/ensembles",
                admin_update_music_ensembles
            ),
        )
        .route(
            "/admin/musics/{id}/delete",
            crate::op_post!(state, "/admin/musics/{id}/delete", admin_delete_music),
        )
        .route(
            "/admin/musics/{id}/retry",
            crate::op_post!(state, "/admin/musics/{id}/retry", admin_retry_render),
        )
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

    tracing::info!(
        field = field_name,
        file_name = file_name.unwrap_or(""),
        "score upload starting field read"
    );

    while let Some(chunk) = field.chunk().await.map_err(|error| {
        tracing::warn!(
            field = field_name,
            file_name = file_name.unwrap_or(""),
            bytes_read = total_bytes,
            error = %error,
            "score upload field read failed"
        );
        AppError::from(error)
    })? {
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

#[utoipa::path(
    get,
    path = "/api/admin/users",
    tag = "admin",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "List users", body = [UserResponse]),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse)
    )
)]
pub(crate) async fn admin_list_users(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<UserResponse>>, AppError> {
    let auth = auth::require_admin_context(&state, &headers).await?;
    if !auth.can_list_users() {
        return Err(AppError::unauthorized(
            "You are not allowed to view all users",
        ));
    }

    let mut conn = state.db_rw.get().await?;
    let rows = users::table
        .order(users::username.asc())
        .select(UserRecord::as_select())
        .load(&mut conn)
        .await?;

    let mut users = Vec::with_capacity(rows.len());
    for row in rows {
        users.push(auth::user_record_to_response(&state.db_rw, &state.storage, row).await?);
    }

    Ok(Json(users))
}

#[utoipa::path(
    post,
    path = "/api/admin/users",
    tag = "admin",
    security(("bearer_auth" = [])),
    request_body = CreateUserRequest,
    responses(
        (status = 200, description = "Created user", body = UserResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 409, description = "Username already exists", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse)
    )
)]
pub(crate) async fn admin_create_user(
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

    let mut conn = state.db_rw.get().await?;
    diesel::insert_into(users::table)
        .values(NewUser {
            id: &record.id,
            username: &record.username,
            created_at: &record.created_at,
            is_superadmin: record.is_superadmin,
            role: &record.role,
            display_name: None,
            avatar_image_key: None,
            created_by_user_id: record.created_by_user_id.as_deref(),
        })
        .execute(&mut conn)
        .await?;

    Ok(Json(
        auth::user_record_to_response(&state.db_rw, &state.storage, record).await?,
    ))
}

#[utoipa::path(
    post,
    path = "/api/admin/users/{id}/login-link",
    tag = "admin",
    security(("bearer_auth" = [])),
    params(
        ("id" = String, Path, description = "User identifier")
    ),
    responses(
        (status = 200, description = "Login link for target user", body = LoginLinkResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "User not found", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse)
    )
)]
pub(crate) async fn admin_create_user_login_link(
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

#[utoipa::path(
    get,
    path = "/api/admin/users/{id}/metadata",
    tag = "admin",
    security(("bearer_auth" = [])),
    params(
        ("id" = String, Path, description = "User identifier")
    ),
    responses(
        (status = 200, description = "Administrative metadata for a user", body = AdminUserMetadataResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "User not found", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse)
    )
)]
pub(crate) async fn admin_user_metadata(
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

#[utoipa::path(
    delete,
    path = "/api/admin/users/{id}",
    tag = "admin",
    security(("bearer_auth" = [])),
    params(
        ("id" = String, Path, description = "User identifier")
    ),
    responses(
        (status = 204, description = "User deleted"),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "User not found", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse)
    )
)]
pub(crate) async fn admin_delete_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let auth_context = auth::require_admin_context(&state, &headers).await?;
    let user = auth::find_user_by_id(&state.db_rw, &id)
        .await?
        .ok_or_else(|| AppError::not_found("User not found"))?;
    auth::ensure_can_delete_user(&auth_context, &user)?;

    let mut conn = state.db_rw.get().await?;
    diesel::delete(users::table.find(&id))
        .execute(&mut conn)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    patch,
    path = "/api/admin/users/{id}",
    tag = "admin",
    security(("bearer_auth" = [])),
    params(
        ("id" = String, Path, description = "User identifier")
    ),
    request_body(content = AdminUpdateUserMultipartRequest, content_type = "multipart/form-data"),
    responses(
        (status = 200, description = "Updated user", body = UserResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "User not found", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse)
    )
)]
pub(crate) async fn admin_update_user(
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

    let mut conn = state.db_rw.get().await?;
    diesel::update(users::table.find(&id))
        .set(UpdateAdminUser {
            display_name: new_display_name.as_deref(),
            role: new_role.as_str(),
            is_superadmin: new_role == AppRole::Superadmin,
            avatar_image_key: new_avatar_key.as_deref(),
        })
        .execute(&mut conn)
        .await?;

    let updated = auth::find_user_by_id(&state.db_rw, &id)
        .await?
        .ok_or_else(|| AppError::not_found("User not found"))?;

    Ok(Json(
        auth::user_record_to_response(&state.db_rw, &state.storage, updated).await?,
    ))
}

#[utoipa::path(
    get,
    path = "/api/admin/ensembles",
    tag = "admin",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "List ensembles", body = [AdminEnsembleResponse]),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse)
    )
)]
pub(crate) async fn admin_list_ensembles(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<AdminEnsembleResponse>>, AppError> {
    let auth_context = auth::require_admin_context(&state, &headers).await?;

    let mut conn = state.db_rw.get().await?;
    let ensembles = ensembles::table
        .order(ensembles::name.asc())
        .select(EnsembleRecord::as_select())
        .load(&mut conn)
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

#[utoipa::path(
    post,
    path = "/api/admin/ensembles",
    tag = "admin",
    security(("bearer_auth" = [])),
    request_body = CreateEnsembleRequest,
    responses(
        (status = 200, description = "Created ensemble", body = AdminEnsembleResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 409, description = "Ensemble already exists", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse)
    )
)]
pub(crate) async fn admin_create_ensemble(
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

    let mut conn = state.db_rw.get().await?;
    diesel::insert_into(ensembles::table)
        .values(NewEnsemble {
            id: &record.id,
            name: &record.name,
            created_at: &record.created_at,
            created_by_user_id: record.created_by_user_id.as_deref(),
        })
        .execute(&mut conn)
        .await?;

    if auth_context.role == AppRole::Manager {
        diesel::insert_into(user_ensemble_memberships::table)
            .values(NewUserEnsembleMembership {
                user_id: &auth_context.user.id,
                ensemble_id: &record.id,
                role: crate::EnsembleRole::Manager.as_str(),
            })
            .on_conflict((
                user_ensemble_memberships::user_id,
                user_ensemble_memberships::ensemble_id,
            ))
            .do_update()
            .set(user_ensemble_memberships::role.eq(excluded(user_ensemble_memberships::role)))
            .execute(&mut conn)
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

#[utoipa::path(
    delete,
    path = "/api/admin/ensembles/{id}",
    tag = "admin",
    security(("bearer_auth" = [])),
    params(
        ("id" = String, Path, description = "Ensemble identifier")
    ),
    responses(
        (status = 204, description = "Ensemble deleted"),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "Ensemble not found", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse)
    )
)]
pub(crate) async fn admin_delete_ensemble(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let auth_context = auth::require_admin_context(&state, &headers).await?;

    let ensemble = music::find_ensemble_by_id(&state.db_rw, &id)
        .await?
        .ok_or_else(|| AppError::not_found("Ensemble not found"))?;
    auth::ensure_can_delete_ensemble(&auth_context, &ensemble)?;

    let mut conn = state.db_rw.get().await?;
    let linked_music_ids = music_ensemble_links::table
        .filter(music_ensemble_links::ensemble_id.eq(&id))
        .select(music_ensemble_links::music_id)
        .distinct()
        .load::<String>(&mut conn)
        .await?;
    let related_links = music_ensemble_links::table
        .filter(music_ensemble_links::music_id.eq_any(&linked_music_ids))
        .select(MusicEnsembleLinkRecord::as_select())
        .load::<MusicEnsembleLinkRecord>(&mut conn)
        .await?
        .into_iter()
        .fold(HashMap::<String, Vec<String>>::new(), |mut acc, link| {
            acc.entry(link.music_id).or_default().push(link.ensemble_id);
            acc
        });
    let orphan_music_ids = related_links
        .into_iter()
        .filter_map(|(music_id, ensemble_ids)| {
            ensemble_ids
                .iter()
                .all(|ensemble_id| ensemble_id == &id)
                .then_some(music_id)
        })
        .collect::<Vec<_>>();

    diesel::delete(ensembles::table.find(&id))
        .execute(&mut conn)
        .await?;

    for music_id in orphan_music_ids {
        music::delete_music_record_and_assets(&state, &music_id).await?;
    }

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/api/admin/ensembles/{id}/users/{user_id}",
    tag = "admin",
    security(("bearer_auth" = [])),
    params(
        ("id" = String, Path, description = "Ensemble identifier"),
        ("user_id" = String, Path, description = "User identifier")
    ),
    request_body = UpdateEnsembleMemberRequest,
    responses(
        (status = 204, description = "User added to ensemble"),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "User or ensemble not found", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse)
    )
)]
pub(crate) async fn admin_add_user_to_ensemble(
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

    let mut conn = state.db_rw.get().await?;
    diesel::insert_into(user_ensemble_memberships::table)
        .values(NewUserEnsembleMembership {
            user_id: &user_id,
            ensemble_id: &id,
            role: role.as_str(),
        })
        .on_conflict((
            user_ensemble_memberships::user_id,
            user_ensemble_memberships::ensemble_id,
        ))
        .do_update()
        .set(user_ensemble_memberships::role.eq(excluded(user_ensemble_memberships::role)))
        .execute(&mut conn)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete,
    path = "/api/admin/ensembles/{id}/users/{user_id}",
    tag = "admin",
    security(("bearer_auth" = [])),
    params(
        ("id" = String, Path, description = "Ensemble identifier"),
        ("user_id" = String, Path, description = "User identifier")
    ),
    responses(
        (status = 204, description = "User removed from ensemble"),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "User or ensemble not found", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse)
    )
)]
pub(crate) async fn admin_remove_user_from_ensemble(
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

    let mut conn = state.db_rw.get().await?;
    diesel::delete(
        user_ensemble_memberships::table
            .filter(user_ensemble_memberships::user_id.eq(&user_id))
            .filter(user_ensemble_memberships::ensemble_id.eq(&id)),
    )
    .execute(&mut conn)
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    patch,
    path = "/api/admin/ensembles/{id}/users",
    tag = "admin",
    security(("bearer_auth" = [])),
    params(
        ("id" = String, Path, description = "Ensemble identifier")
    ),
    request_body = UpdateEnsembleMembersRequest,
    responses(
        (status = 204, description = "Ensemble memberships updated"),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "User or ensemble not found", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse)
    )
)]
pub(crate) async fn admin_update_ensemble_members(
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

    let mut conn = state.db_rw.get().await?;
    let current_members = user_ensemble_memberships::table
        .filter(user_ensemble_memberships::ensemble_id.eq(&id))
        .select(UserEnsembleMembershipRecord::as_select())
        .load(&mut conn)
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

    let mut conn = state.db_rw.get().await?;
    conn.transaction::<_, AppError, _>(|tx| {
        Box::pin(async move {
            for (user_id, role) in to_upsert {
                diesel::insert_into(user_ensemble_memberships::table)
                    .values(NewUserEnsembleMembership {
                        user_id: &user_id,
                        ensemble_id: &id,
                        role: role.as_str(),
                    })
                    .on_conflict((
                        user_ensemble_memberships::user_id,
                        user_ensemble_memberships::ensemble_id,
                    ))
                    .do_update()
                    .set(
                        user_ensemble_memberships::role
                            .eq(excluded(user_ensemble_memberships::role)),
                    )
                    .execute(tx)
                    .await?;
            }

            for user_id in to_remove {
                diesel::delete(
                    user_ensemble_memberships::table
                        .filter(user_ensemble_memberships::user_id.eq(&user_id))
                        .filter(user_ensemble_memberships::ensemble_id.eq(&id)),
                )
                .execute(tx)
                .await?;
            }

            Ok(())
        })
    })
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/api/admin/musics/{id}/ensembles/{ensemble_id}",
    tag = "admin",
    security(("bearer_auth" = [])),
    params(
        ("id" = String, Path, description = "Music identifier"),
        ("ensemble_id" = String, Path, description = "Ensemble identifier")
    ),
    responses(
        (status = 204, description = "Score linked to ensemble"),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "Music or ensemble not found", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse)
    )
)]
pub(crate) async fn admin_add_music_to_ensemble(
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

    let mut conn = state.db_rw.get().await?;
    diesel::insert_into(music_ensemble_links::table)
        .values(NewMusicEnsembleLink {
            music_id: &id,
            ensemble_id: &ensemble_id,
        })
        .on_conflict_do_nothing()
        .execute(&mut conn)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete,
    path = "/api/admin/musics/{id}/ensembles/{ensemble_id}",
    tag = "admin",
    security(("bearer_auth" = [])),
    params(
        ("id" = String, Path, description = "Music identifier"),
        ("ensemble_id" = String, Path, description = "Ensemble identifier")
    ),
    responses(
        (status = 204, description = "Score unlinked from ensemble"),
        (status = 400, description = "The score must remain in at least one ensemble", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "Music or ensemble not found", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse)
    )
)]
pub(crate) async fn admin_remove_music_from_ensemble(
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

    let mut conn = state.db_rw.get().await?;
    diesel::delete(
        music_ensemble_links::table
            .filter(music_ensemble_links::music_id.eq(&id))
            .filter(music_ensemble_links::ensemble_id.eq(&ensemble_id)),
    )
    .execute(&mut conn)
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    patch,
    path = "/api/admin/musics/{id}/ensembles",
    tag = "admin",
    security(("bearer_auth" = [])),
    params(
        ("id" = String, Path, description = "Music identifier")
    ),
    request_body = UpdateMusicEnsemblesRequest,
    responses(
        (status = 204, description = "Score ensemble links updated"),
        (status = 400, description = "The score must remain in at least one ensemble", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "Music or ensemble not found", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse)
    )
)]
pub(crate) async fn admin_update_music_ensembles(
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

    let mut conn = state.db_rw.get().await?;
    conn.transaction::<_, AppError, _>(|tx| {
        Box::pin(async move {
            for ensemble_id in to_add {
                diesel::insert_into(music_ensemble_links::table)
                    .values(NewMusicEnsembleLink {
                        music_id: &id,
                        ensemble_id: &ensemble_id,
                    })
                    .on_conflict_do_nothing()
                    .execute(tx)
                    .await?;
            }

            for ensemble_id in to_remove {
                diesel::delete(
                    music_ensemble_links::table
                        .filter(music_ensemble_links::music_id.eq(&id))
                        .filter(music_ensemble_links::ensemble_id.eq(&ensemble_id)),
                )
                .execute(tx)
                .await?;
            }

            Ok(())
        })
    })
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/api/admin/musics",
    tag = "admin",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "List scores", body = [AdminMusicResponse]),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse)
    )
)]
pub(crate) async fn admin_list_musics(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<AdminMusicResponse>>, AppError> {
    let auth_context = auth::require_admin_context(&state, &headers).await?;

    let mut conn = state.db_rw.get().await?;
    let rows = musics::table
        .order(musics::created_at.desc())
        .select(MusicRecord::as_select())
        .load(&mut conn)
        .await?;

    let total_rows = diesel::sql_query(
        "SELECT music_id, COALESCE(SUM(size_bytes), 0)::BIGINT AS total_bytes FROM stems GROUP BY music_id",
    )
    .load::<MusicTotalBytesRow>(&mut conn)
        .await?;
    let totals: HashMap<String, i64> = total_rows
        .into_iter()
        .map(|row| (row.music_id, row.total_bytes))
        .collect();
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
#[utoipa::path(
    post,
    path = "/api/admin/musics",
    tag = "admin",
    security(("bearer_auth" = [])),
    request_body(content = AdminUploadMusicMultipartRequest, content_type = "multipart/form-data"),
    responses(
        (status = 200, description = "Uploaded score", body = AdminMusicResponse),
        (status = 400, description = "Invalid upload request", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "Ensemble not found", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse)
    )
)]
pub(crate) async fn admin_upload_music(
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
        .as_slice()
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
    let mut conn = state.db_rw.get().await?;
    diesel::insert_into(musics::table)
        .values(NewMusic {
            id: &music_id,
            title: &resolved_title,
            icon: icon.as_deref(),
            icon_image_key: icon_image_key.as_deref(),
            filename: &filename,
            content_type: &content_type,
            object_key: &object_key,
            audio_object_key: None,
            audio_status: "processing",
            audio_error: None,
            midi_object_key: None,
            midi_status: "processing",
            midi_error: None,
            musicxml_object_key: None,
            musicxml_status: "processing",
            musicxml_error: None,
            stems_status: "processing",
            stems_error: None,
            public_token: &public_token,
            public_id: public_id.as_deref(),
            quality_profile: quality_profile.as_str(),
            created_at: &created_at,
            directory_id: &primary_ensemble_id,
            owner_user_id: Some(auth_context.user.id.as_str()),
        })
        .execute(&mut conn)
        .await?;
    for ensemble_id in &ensemble_ids {
        diesel::insert_into(music_ensemble_links::table)
            .values(NewMusicEnsembleLink {
                music_id: &music_id,
                ensemble_id,
            })
            .on_conflict_do_nothing()
            .execute(&mut conn)
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
        directory_id: primary_ensemble_id.clone(),
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

    let mut conn = state.db_rw.get().await?;
    diesel::update(musics::table.find(&music_id))
        .set(UpdateMusicProcessing {
            audio_object_key: audio_object_key.as_deref(),
            audio_status: &audio_status,
            audio_error: audio_error.as_deref(),
            midi_object_key: midi_object_key.as_deref(),
            midi_status: &midi_status,
            midi_error: midi_error.as_deref(),
            musicxml_object_key: musicxml_object_key.as_deref(),
            musicxml_status: &musicxml_status,
            musicxml_error: musicxml_error.as_deref(),
            stems_status: &stems_status,
            stems_error: stems_error.as_deref(),
        })
        .execute(&mut conn)
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
    let mut conn = state.db_rw.get().await?;
    diesel::update(musics::table.find(music_id))
        .set(crate::models::MarkMusicProcessingFailed {
            audio_status: "failed",
            audio_error: Some(error.as_str()),
            midi_status: "failed",
            midi_error: Some(error.as_str()),
            musicxml_status: "failed",
            musicxml_error: Some(error.as_str()),
            stems_status: "failed",
            stems_error: Some(error.as_str()),
        })
        .execute(&mut conn)
        .await?;

    Ok(())
}

#[tracing::instrument(skip(state, headers), fields(music_id = %id))]
#[utoipa::path(
    post,
    path = "/api/admin/musics/{id}/retry",
    tag = "admin",
    security(("bearer_auth" = [])),
    params(
        ("id" = String, Path, description = "Music identifier")
    ),
    responses(
        (status = 200, description = "Re-rendered score assets", body = AdminMusicResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "Music not found", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse)
    )
)]
pub(crate) async fn admin_retry_render(
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

    let mut conn = state.db_rw.get().await?;
    diesel::delete(stems::table.filter(stems::music_id.eq(&id)))
        .execute(&mut conn)
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

    diesel::update(musics::table.find(&id))
        .set(UpdateMusicProcessing {
            audio_object_key: audio_object_key.as_deref(),
            audio_status: &audio_status,
            audio_error: audio_error.as_deref(),
            midi_object_key: midi_object_key.as_deref(),
            midi_status: &midi_status,
            midi_error: midi_error.as_deref(),
            musicxml_object_key: musicxml_object_key.as_deref(),
            musicxml_status: &musicxml_status,
            musicxml_error: musicxml_error.as_deref(),
            stems_status: &stems_status,
            stems_error: stems_error.as_deref(),
        })
        .execute(&mut conn)
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

#[utoipa::path(
    patch,
    path = "/api/admin/musics/{id}",
    tag = "admin",
    security(("bearer_auth" = [])),
    params(
        ("id" = String, Path, description = "Music identifier")
    ),
    request_body(content = AdminUpdateMusicMultipartRequest, content_type = "multipart/form-data"),
    responses(
        (status = 200, description = "Updated score metadata", body = AdminMusicResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "Music not found", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse)
    )
)]
pub(crate) async fn admin_update_music(
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

    let mut conn = state.db_rw.get().await?;
    let update_result = diesel::update(musics::table.find(&id))
        .set(UpdateMusicMetadata {
            title: &title,
            public_id: public_id.as_deref(),
            icon: icon.as_deref(),
            icon_image_key: icon_image_key.as_deref(),
        })
        .execute(&mut conn)
        .await?;

    if update_result == 0 {
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

#[utoipa::path(
    post,
    path = "/api/admin/musics/{id}/move",
    tag = "admin",
    security(("bearer_auth" = [])),
    params(
        ("id" = String, Path, description = "Music identifier")
    ),
    request_body = MoveMusicRequest,
    responses(
        (status = 200, description = "Moved score to a new ensemble", body = AdminMusicResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "Music or ensemble not found", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse)
    )
)]
pub(crate) async fn admin_move_music(
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
    let mut conn = state.db_rw.get().await?;
    diesel::delete(music_ensemble_links::table.filter(music_ensemble_links::music_id.eq(&id)))
        .execute(&mut conn)
        .await?;
    diesel::insert_into(music_ensemble_links::table)
        .values(NewMusicEnsembleLink {
            music_id: &id,
            ensemble_id,
        })
        .execute(&mut conn)
        .await?;
    diesel::update(musics::table.find(&id))
        .set(UpdateMusicDirectory {
            directory_id: ensemble_id,
        })
        .execute(&mut conn)
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

#[utoipa::path(
    get,
    path = "/api/admin/musics/{id}/playtime",
    tag = "admin",
    security(("bearer_auth" = [])),
    params(
        ("id" = String, Path, description = "Music identifier")
    ),
    responses(
        (status = 200, description = "Score playtime summary", body = AdminMusicPlaytimeResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "Music not found", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse)
    )
)]
pub(crate) async fn admin_music_playtime(
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

#[utoipa::path(
    post,
    path = "/api/admin/musics/{id}/delete",
    tag = "admin",
    security(("bearer_auth" = [])),
    params(
        ("id" = String, Path, description = "Music identifier")
    ),
    responses(
        (status = 204, description = "Score deleted"),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "Music not found", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse)
    )
)]
pub(crate) async fn admin_delete_music(
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
