use crate::config::AppConfig;
use crate::db::DbPool;
use crate::models::{
    BigIntValueRow, EnsembleRecord, MusicEnsembleLinkRecord, MusicRecord, NewScoreAnnotation,
    NewUserMusicTrackPlaytime, ProcessingJobProgress, ProcessingJobRecord, StemRecord,
    UserEnsembleMembershipRecord, UserRecord,
};
use crate::schema::{
    ensembles, music_ensemble_links, musics, processing_jobs, score_annotations, stems,
    user_ensemble_memberships, user_music_track_playtime,
};
use crate::schemas::{
    AdminMusicPlaytimeResponse, AdminMusicProcessingProgressResponse,
    AdminMusicProcessingStepResponse, AdminMusicResponse, AdminUserScorePlaytimeResponse,
    CreateScoreAnnotationRequest, MusicPlaytimeLeaderboardEntryResponse,
    MusicPlaytimeTrackSummaryResponse, PublicMusicResponse, ScoreAnnotationListResponse,
    ScoreAnnotationResponse, StemInfo,
};
use crate::storage::Storage;
use crate::{
    AppError, AppRole, AppState, AuthContext, sanitize_content_disposition, utc_now_string,
};
use chrono::{DateTime, Utc};
use diesel::OptionalExtension;
use diesel::QueryableByName;
use diesel::prelude::*;
use diesel::sql_types::{BigInt, Double, Nullable, Text};
use diesel::upsert::excluded;
use diesel_async::{AsyncConnection, RunQueryDsl};
pub(crate) use fumen_core::music::{find_public_stems, processing_log_key};
use std::collections::HashMap;
use tracing::warn;

#[derive(QueryableByName)]
struct AdminMusicPlaytimeRow {
    #[diesel(sql_type = Text)]
    user_id: String,
    #[diesel(sql_type = Text)]
    username: String,
    #[diesel(sql_type = Nullable<Text>)]
    display_name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    avatar_image_key: Option<String>,
    #[diesel(sql_type = BigInt)]
    track_index: i64,
    #[diesel(sql_type = Text)]
    track_name: String,
    #[diesel(sql_type = Text)]
    instrument_name: String,
    #[diesel(sql_type = Double)]
    total_seconds: f64,
}

#[derive(QueryableByName)]
struct AdminUserScorePlaytimeRow {
    #[diesel(sql_type = Text)]
    music_id: String,
    #[diesel(sql_type = Text)]
    title: String,
    #[diesel(sql_type = Nullable<Text>)]
    subtitle: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    icon: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    icon_image_key: Option<String>,
    #[diesel(sql_type = Text)]
    public_token: String,
    #[diesel(sql_type = Nullable<Text>)]
    public_id: Option<String>,
    #[diesel(sql_type = Double)]
    total_seconds: f64,
}

#[derive(QueryableByName)]
struct ScoreAnnotationRow {
    #[diesel(sql_type = Text)]
    id: String,
    #[diesel(sql_type = Text)]
    music_id: String,
    #[diesel(sql_type = Text)]
    user_id: String,
    #[diesel(sql_type = Text)]
    username: String,
    #[diesel(sql_type = Nullable<Text>)]
    display_name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    avatar_image_key: Option<String>,
    #[diesel(sql_type = Text)]
    comment: String,
    #[diesel(sql_type = BigInt)]
    bar_number: i64,
    #[diesel(sql_type = BigInt)]
    beat_number: i64,
    #[diesel(sql_type = Text)]
    instrument: String,
    #[diesel(sql_type = Nullable<Double>)]
    system_y_ratio: Option<f64>,
    #[diesel(sql_type = Text)]
    created_at: String,
}

fn resolve_user_avatar_url(
    storage: &Storage,
    user_id: &str,
    avatar_image_key: Option<&str>,
) -> Option<String> {
    avatar_image_key
        .and_then(|key| storage.public_url(key))
        .or_else(|| avatar_image_key.map(|_| format!("/api/users/{user_id}/avatar")))
}

fn score_annotation_visibility_scope(
    auth: Option<&AuthContext>,
    record: &MusicRecord,
    music_ensemble_ids: &[String],
) -> &'static str {
    let Some(auth) = auth else {
        return "none";
    };

    if auth.has_global_power() {
        return "all";
    }

    let owns_score = record.owner_user_id.as_deref() == Some(auth.user.id.as_str());
    match auth.role {
        AppRole::Manager => {
            if owns_score
                || music_ensemble_ids
                    .iter()
                    .any(|ensemble_id| auth.managed_ensemble_ids.contains(ensemble_id))
            {
                "all"
            } else {
                "own"
            }
        }
        AppRole::Editor => {
            if owns_score {
                "all"
            } else {
                "own"
            }
        }
        AppRole::User => "own",
        AppRole::Superadmin | AppRole::Admin => "all",
    }
}

fn score_annotation_response_from_row(
    storage: &Storage,
    row: ScoreAnnotationRow,
) -> ScoreAnnotationResponse {
    let avatar_url =
        resolve_user_avatar_url(storage, &row.user_id, row.avatar_image_key.as_deref());

    ScoreAnnotationResponse {
        id: row.id,
        music_id: row.music_id,
        user_id: row.user_id,
        username: row.username,
        display_name: row.display_name,
        avatar_url,
        comment: row.comment,
        instrument: row.instrument,
        bar_number: row.bar_number,
        beat_number: row.beat_number,
        system_y_ratio: row.system_y_ratio,
        created_at: row.created_at,
    }
}

fn score_annotation_response_from_user(
    storage: &Storage,
    music_id: &str,
    user: &UserRecord,
    annotation_id: &str,
    comment: &str,
    instrument: &str,
    bar_number: i64,
    beat_number: i64,
    system_y_ratio: Option<f64>,
    created_at: &str,
) -> ScoreAnnotationResponse {
    let avatar_url = resolve_user_avatar_url(storage, &user.id, user.avatar_image_key.as_deref());

    ScoreAnnotationResponse {
        id: annotation_id.to_owned(),
        music_id: music_id.to_owned(),
        user_id: user.id.clone(),
        username: user.username.clone(),
        display_name: user.display_name.clone(),
        avatar_url,
        comment: comment.to_owned(),
        instrument: instrument.to_owned(),
        bar_number,
        beat_number,
        system_y_ratio,
        created_at: created_at.to_owned(),
    }
}

fn resolve_music_public_url(
    config: &AppConfig,
    public_token: &str,
    public_id: Option<&str>,
) -> String {
    public_id
        .map(|public_id| config.public_url_for(public_id))
        .unwrap_or_else(|| config.public_url_for(public_token))
}

fn with_cache_busting_query(url: String, version: &str) -> String {
    let separator = if url.contains('?') { '&' } else { '?' };
    format!("{url}{separator}v={version}")
}

pub(crate) async fn fetch_stems_total(db: &DbPool, music_id: &str) -> i64 {
    let mut conn = match db.get().await {
        Ok(conn) => conn,
        Err(_) => return 0,
    };

    diesel::sql_query(
        "SELECT COALESCE(SUM(size_bytes), 0)::BIGINT AS value FROM stems WHERE music_id = $1",
    )
    .bind::<Text, _>(music_id)
    .get_result::<BigIntValueRow>(&mut conn)
    .await
    .map(|row| row.value)
    .unwrap_or(0)
}

pub(crate) async fn find_ensemble_by_id(
    db: &DbPool,
    id: &str,
) -> Result<Option<EnsembleRecord>, AppError> {
    let mut conn = db.get().await?;
    Ok(ensembles::table
        .find(id)
        .select(EnsembleRecord::as_select())
        .first(&mut conn)
        .await
        .optional()?)
}

pub(crate) async fn find_ensemble_by_name(
    db: &DbPool,
    name: &str,
) -> Result<Option<EnsembleRecord>, AppError> {
    let mut conn = db.get().await?;
    Ok(ensembles::table
        .filter(ensembles::name.eq(name))
        .select(EnsembleRecord::as_select())
        .first(&mut conn)
        .await
        .optional()?)
}

pub(crate) async fn fetch_user_ensemble_memberships(
    db: &DbPool,
) -> Result<Vec<UserEnsembleMembershipRecord>, AppError> {
    let mut conn = db.get().await?;
    Ok(user_ensemble_memberships::table
        .select(UserEnsembleMembershipRecord::as_select())
        .load(&mut conn)
        .await?)
}

pub(crate) async fn fetch_music_ensemble_links(
    db: &DbPool,
) -> Result<Vec<MusicEnsembleLinkRecord>, AppError> {
    let mut conn = db.get().await?;
    Ok(music_ensemble_links::table
        .select(MusicEnsembleLinkRecord::as_select())
        .load(&mut conn)
        .await?)
}

pub(crate) async fn fetch_music_ensemble_ids(
    db: &DbPool,
    music_id: &str,
) -> Result<Vec<String>, AppError> {
    let mut conn = db.get().await?;
    Ok(music_ensemble_links::table
        .filter(music_ensemble_links::music_id.eq(music_id))
        .select(music_ensemble_links::ensemble_id)
        .order(music_ensemble_links::ensemble_id.asc())
        .load(&mut conn)
        .await?)
}

pub(crate) async fn fetch_ensemble_summaries(
    db: &DbPool,
) -> Result<HashMap<String, String>, AppError> {
    let mut conn = db.get().await?;
    Ok(ensembles::table
        .select((ensembles::id, ensembles::name))
        .order(ensembles::name.asc())
        .load::<(String, String)>(&mut conn)
        .await?
        .into_iter()
        .collect())
}

pub(crate) async fn fetch_ensemble_score_counts(
    db: &DbPool,
) -> Result<Vec<(String, i64)>, AppError> {
    let mut conn = db.get().await?;
    let links = music_ensemble_links::table
        .select(MusicEnsembleLinkRecord::as_select())
        .load::<MusicEnsembleLinkRecord>(&mut conn)
        .await?;
    let mut counts: HashMap<String, std::collections::HashSet<String>> = HashMap::new();
    for link in links {
        counts
            .entry(link.ensemble_id)
            .or_default()
            .insert(link.music_id);
    }

    Ok(counts
        .into_iter()
        .map(|(ensemble_id, music_ids)| (ensemble_id, music_ids.len() as i64))
        .collect())
}

pub(crate) fn build_music_ensemble_maps(
    links: Vec<MusicEnsembleLinkRecord>,
    ensemble_names: &HashMap<String, String>,
) -> (HashMap<String, Vec<String>>, HashMap<String, Vec<String>>) {
    let mut id_map: HashMap<String, Vec<String>> = HashMap::new();
    let mut name_map: HashMap<String, Vec<String>> = HashMap::new();

    for link in links {
        id_map
            .entry(link.music_id.clone())
            .or_default()
            .push(link.ensemble_id.clone());
        if let Some(name) = ensemble_names.get(&link.ensemble_id) {
            name_map
                .entry(link.music_id)
                .or_default()
                .push(name.clone());
        }
    }

    for values in id_map.values_mut() {
        values.sort();
    }
    for values in name_map.values_mut() {
        values.sort();
    }

    (id_map, name_map)
}

pub(crate) async fn ensemble_metadata_for_music(
    db: &DbPool,
    music_id: &str,
) -> Result<(Vec<String>, Vec<String>), AppError> {
    let ensemble_ids = fetch_music_ensemble_ids(db, music_id).await?;
    let ensemble_name_map = fetch_ensemble_summaries(db).await?;
    let ensemble_names = ensemble_ids
        .iter()
        .filter_map(|ensemble_id| ensemble_name_map.get(ensemble_id).cloned())
        .collect::<Vec<_>>();

    Ok((ensemble_ids, ensemble_names))
}

pub(crate) async fn can_view_music_in_control_room(
    db: &DbPool,
    auth: &AuthContext,
    music_id: &str,
) -> Result<bool, AppError> {
    if auth.has_global_power() {
        return Ok(true);
    }

    let record = find_music_by_id(db, music_id)
        .await?
        .ok_or_else(|| AppError::not_found("Music not found"))?;
    if record.owner_user_id.as_deref() == Some(auth.user.id.as_str()) {
        return Ok(true);
    }

    let ensemble_ids = fetch_music_ensemble_ids(db, music_id).await?;
    if ensemble_ids.is_empty() {
        return Ok(false);
    }

    Ok(ensemble_ids
        .iter()
        .any(|ensemble_id| auth.editable_ensemble_ids.contains(ensemble_id)))
}

pub(crate) async fn can_manage_owned_music(
    db: &DbPool,
    auth: &AuthContext,
    music_id: &str,
) -> Result<bool, AppError> {
    if auth.has_global_power() {
        return Ok(true);
    }

    let record = find_music_by_id(db, music_id)
        .await?
        .ok_or_else(|| AppError::not_found("Music not found"))?;
    Ok(record.owner_user_id.as_deref() == Some(auth.user.id.as_str()))
}

pub(crate) async fn ensure_can_manage_music(
    db: &DbPool,
    auth: &AuthContext,
    music_id: &str,
) -> Result<(), AppError> {
    if can_manage_owned_music(db, auth, music_id).await? {
        Ok(())
    } else {
        Err(AppError::unauthorized(
            "You can only change score metadata for scores you own",
        ))
    }
}

pub(crate) async fn ensure_music_and_ensemble_exist(
    db: &DbPool,
    music_id: &str,
    ensemble_id: &str,
) -> Result<(), AppError> {
    if find_music_by_id(db, music_id).await?.is_none() {
        return Err(AppError::not_found("Music not found"));
    }
    if find_ensemble_by_id(db, ensemble_id).await?.is_none() {
        return Err(AppError::not_found("Ensemble not found"));
    }
    Ok(())
}

pub(crate) async fn ensure_can_manage_music_and_target_ensemble(
    db: &DbPool,
    auth: &AuthContext,
    music_id: &str,
    ensemble_id: &str,
) -> Result<(), AppError> {
    if auth.has_global_power() {
        return Ok(());
    }

    match auth.role {
        AppRole::Manager => {
            crate::services::auth::ensure_can_manage_ensemble(auth, ensemble_id)?;
            if can_view_music_in_control_room(db, auth, music_id).await? {
                Ok(())
            } else {
                Err(AppError::unauthorized(
                    "You can only manage scores that belong to ensembles you manage",
                ))
            }
        }
        AppRole::Editor => {
            if !auth.can_edit_ensemble_scores(ensemble_id) {
                return Err(AppError::unauthorized(
                    "You can only manage scores for ensembles where you are an editor",
                ));
            }
            if can_manage_owned_music(db, auth, music_id).await? {
                Ok(())
            } else {
                Err(AppError::unauthorized(
                    "Editors can only change scores they added themselves",
                ))
            }
        }
        AppRole::User => Err(AppError::unauthorized(
            "You do not have access to manage scores",
        )),
        AppRole::Superadmin | AppRole::Admin => Ok(()),
    }
}

pub(crate) async fn find_public_music_record(
    state: &AppState,
    access_key: &str,
) -> Result<Option<MusicRecord>, AppError> {
    if let Some(record) = find_music_by_access_key(&state.db_ro, access_key).await? {
        return Ok(Some(record));
    }

    Ok(find_music_by_access_key(&state.db_rw, access_key).await?)
}

pub(crate) async fn find_all_accessible_music(
    db: &DbPool,
) -> Result<Vec<(MusicRecord, String, String)>, AppError> {
    let mut conn = db.get().await?;
    Ok(music_ensemble_links::table
        .inner_join(musics::table)
        .inner_join(ensembles::table)
        .select((
            MusicRecord::as_select(),
            music_ensemble_links::ensemble_id,
            ensembles::name,
        ))
        .order((ensembles::name.asc(), musics::title.asc()))
        .load::<(MusicRecord, String, String)>(&mut conn)
        .await?)
}

pub(crate) async fn find_accessible_music_for_user(
    db: &DbPool,
    user_id: &str,
) -> Result<Vec<(MusicRecord, String, String)>, AppError> {
    let mut conn = db.get().await?;
    Ok(music_ensemble_links::table
        .inner_join(musics::table)
        .inner_join(ensembles::table)
        .inner_join(
            user_ensemble_memberships::table
                .on(user_ensemble_memberships::ensemble_id.eq(music_ensemble_links::ensemble_id)),
        )
        .filter(user_ensemble_memberships::user_id.eq(user_id))
        .select((
            MusicRecord::as_select(),
            music_ensemble_links::ensemble_id,
            ensembles::name,
        ))
        .distinct()
        .order((ensembles::name.asc(), musics::title.asc()))
        .load::<(MusicRecord, String, String)>(&mut conn)
        .await?)
}

pub(crate) async fn find_public_stem(
    db_primary: &DbPool,
    db_fallback: &DbPool,
    music_id: &str,
    track_index: i64,
) -> Result<Option<StemRecord>, AppError> {
    let mut primary = db_primary.get().await?;
    if let Some(stem) = stems::table
        .filter(stems::music_id.eq(music_id))
        .filter(stems::track_index.eq(track_index))
        .select(StemRecord::as_select())
        .first(&mut primary)
        .await
        .optional()?
    {
        return Ok(Some(stem));
    }

    let mut fallback = db_fallback.get().await?;
    Ok(stems::table
        .filter(stems::music_id.eq(music_id))
        .filter(stems::track_index.eq(track_index))
        .select(StemRecord::as_select())
        .first(&mut fallback)
        .await
        .optional()?)
}

pub(crate) async fn add_user_track_playtime(
    db: &DbPool,
    user_id: &str,
    music_id: &str,
    track_totals: &[(i64, f64)],
) -> Result<(), AppError> {
    if track_totals.is_empty() {
        return Ok(());
    }

    let updated_at = utc_now_string();
    let mut track_totals = track_totals.to_vec();
    track_totals.sort_unstable_by_key(|(track_index, _)| *track_index);
    let mut conn = db.get().await?;
    conn.transaction::<_, AppError, _>(|tx| {
        Box::pin(async move {
            for (track_index, total_seconds) in track_totals {
                diesel::insert_into(user_music_track_playtime::table)
                    .values(NewUserMusicTrackPlaytime {
                        user_id,
                        music_id,
                        track_index,
                        total_seconds,
                        updated_at: &updated_at,
                    })
                    .on_conflict((
                        user_music_track_playtime::user_id,
                        user_music_track_playtime::music_id,
                        user_music_track_playtime::track_index,
                    ))
                    .do_update()
                    .set((
                        user_music_track_playtime::total_seconds
                            .eq(user_music_track_playtime::total_seconds
                                + excluded(user_music_track_playtime::total_seconds)),
                        user_music_track_playtime::updated_at
                            .eq(excluded(user_music_track_playtime::updated_at)),
                    ))
                    .execute(tx)
                    .await?;
            }
            Ok(())
        })
    })
    .await?;

    Ok(())
}

#[tracing::instrument(skip(db, storage), fields(music_id = %music_id))]
pub(crate) async fn build_admin_music_playtime_response(
    db: &DbPool,
    storage: &Storage,
    music_id: &str,
) -> Result<AdminMusicPlaytimeResponse, AppError> {
    let stems = find_public_stems(db, db, music_id).await?;
    let mut conn = db.get().await?;
    let rows = diesel::sql_query(
        r#"
        SELECT
            p.user_id,
            u.username,
            u.display_name,
            u.avatar_image_key,
            s.track_index,
            s.track_name,
            s.instrument_name,
            p.total_seconds
        FROM user_music_track_playtime p
        JOIN users u
            ON u.id = p.user_id
        JOIN stems s
            ON s.music_id = p.music_id
           AND s.track_index = p.track_index
        WHERE p.music_id = $1
          AND p.total_seconds > 0
        ORDER BY p.total_seconds DESC, u.username ASC, s.track_index ASC
        "#,
    )
    .bind::<Text, _>(music_id)
    .load::<AdminMusicPlaytimeRow>(&mut conn)
    .await?;

    let mut overall_tracks = stems
        .iter()
        .map(|stem| {
            (
                stem.track_index,
                MusicPlaytimeTrackSummaryResponse {
                    track_index: stem.track_index,
                    track_name: stem.track_name.clone(),
                    instrument_name: stem.instrument_name.clone(),
                    total_seconds: 0.0,
                },
            )
        })
        .collect::<HashMap<_, _>>();
    let mut leaderboard = HashMap::<String, MusicPlaytimeLeaderboardEntryResponse>::new();
    let mut total_seconds = 0.0;

    for row in rows {
        total_seconds += row.total_seconds;

        if let Some(track) = overall_tracks.get_mut(&row.track_index) {
            track.total_seconds += row.total_seconds;
        }

        let entry = leaderboard.entry(row.user_id.clone()).or_insert_with(|| {
            MusicPlaytimeLeaderboardEntryResponse {
                user_id: row.user_id.clone(),
                username: row.username.clone(),
                display_name: row.display_name.clone(),
                avatar_url: resolve_user_avatar_url(
                    storage,
                    &row.user_id,
                    row.avatar_image_key.as_deref(),
                ),
                best_track_seconds: 0.0,
                track_totals: Vec::new(),
            }
        });
        entry.best_track_seconds = entry.best_track_seconds.max(row.total_seconds);
        entry.track_totals.push(MusicPlaytimeTrackSummaryResponse {
            track_index: row.track_index,
            track_name: row.track_name,
            instrument_name: row.instrument_name,
            total_seconds: row.total_seconds,
        });
    }

    let mut track_totals = overall_tracks.into_values().collect::<Vec<_>>();
    track_totals.sort_by(|left, right| {
        right
            .total_seconds
            .partial_cmp(&left.total_seconds)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(left.track_index.cmp(&right.track_index))
    });

    let mut leaderboard = leaderboard.into_values().collect::<Vec<_>>();
    for entry in &mut leaderboard {
        entry.track_totals.sort_by(|left, right| {
            right
                .total_seconds
                .partial_cmp(&left.total_seconds)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(left.track_index.cmp(&right.track_index))
        });
    }
    leaderboard.sort_by(|left, right| {
        right
            .best_track_seconds
            .partial_cmp(&left.best_track_seconds)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(left.username.cmp(&right.username))
    });

    Ok(AdminMusicPlaytimeResponse {
        total_seconds,
        listener_count: leaderboard.len() as i64,
        track_totals,
        leaderboard,
    })
}

#[tracing::instrument(skip(config, storage, db), fields(user_id = %user_id))]
pub(crate) async fn build_admin_user_metadata_playtime_response(
    config: &AppConfig,
    storage: &Storage,
    db: &DbPool,
    user_id: &str,
) -> Result<(f64, Vec<AdminUserScorePlaytimeResponse>), AppError> {
    let mut conn = db.get().await?;
    let rows = diesel::sql_query(
        r#"
        SELECT
            m.id AS music_id,
            m.title,
            m.subtitle,
            m.icon,
            m.icon_image_key,
            m.public_token,
            m.public_id,
            SUM(p.total_seconds)::DOUBLE PRECISION AS total_seconds
        FROM user_music_track_playtime p
        JOIN musics m
            ON m.id = p.music_id
        WHERE p.user_id = $1
          AND p.total_seconds > 0
        GROUP BY
            m.id,
            m.title,
            m.subtitle,
            m.icon,
            m.icon_image_key,
            m.public_token,
            m.public_id
        ORDER BY total_seconds DESC, m.title ASC
        "#,
    )
    .bind::<Text, _>(user_id)
    .load::<AdminUserScorePlaytimeRow>(&mut conn)
    .await?;

    let mut total_seconds = 0.0;
    let mut score_playtimes = Vec::with_capacity(rows.len());

    for row in rows {
        total_seconds += row.total_seconds;
        let icon_image_url = row.icon_image_key.as_ref().map(|key| {
            storage
                .public_url(key)
                .unwrap_or_else(|| format!("/api/public/{}/icon", row.public_token))
        });
        score_playtimes.push(AdminUserScorePlaytimeResponse {
            music_id: row.music_id,
            title: row.title,
            subtitle: row.subtitle,
            icon: row.icon,
            icon_image_url,
            public_url: resolve_music_public_url(
                config,
                &row.public_token,
                row.public_id.as_deref(),
            ),
            total_seconds: row.total_seconds,
        });
    }

    Ok((total_seconds, score_playtimes))
}

pub(crate) async fn build_public_stem_infos(
    state: &AppState,
    access_key: &str,
    music_id: &str,
) -> Result<Vec<StemInfo>, AppError> {
    let stems = find_public_stems(&state.db_ro, &state.db_rw, music_id).await?;
    let mut resolved_infos = Vec::new();
    for stem in stems {
        let stem_version = format!("{}-{}", stem.id, stem.size_bytes);
        let full_stem_url = with_cache_busting_query(
            state
                .storage
                .public_url(&stem.storage_key)
                .unwrap_or_else(|| format!("/api/public/{access_key}/stems/{}", stem.track_index)),
            &stem_version,
        );

        resolved_infos.push(StemInfo {
            track_index: stem.track_index,
            track_name: stem.track_name,
            instrument_name: stem.instrument_name,
            full_stem_url,
            duration_seconds: 0.0,
            drum_map: stem
                .drum_map_json
                .as_deref()
                .map(serde_json::from_str)
                .transpose()
                .map_err(|error| AppError::from(anyhow::Error::from(error)))?,
        });
    }

    Ok(resolved_infos)
}

pub(crate) async fn build_public_score_annotations_response(
    state: &AppState,
    access_key: &str,
    auth: Option<&AuthContext>,
) -> Result<ScoreAnnotationListResponse, AppError> {
    let record = find_public_music_record(state, access_key)
        .await?
        .ok_or_else(|| AppError::not_found("Music not found"))?;

    let music_ensemble_ids = match auth {
        Some(auth) if matches!(auth.role, AppRole::Manager) => {
            fetch_music_ensemble_ids(&state.db_rw, &record.id).await?
        }
        _ => Vec::new(),
    };
    let visibility_scope =
        score_annotation_visibility_scope(auth, &record, &music_ensemble_ids).to_owned();

    if visibility_scope == "none" {
        return Ok(ScoreAnnotationListResponse {
            visibility_scope,
            annotations: Vec::new(),
        });
    }

    let mut conn = state.db_rw.get().await?;
    let rows = if visibility_scope == "all" {
        diesel::sql_query(
            r#"
            SELECT
                a.id,
                a.music_id,
                a.user_id,
                u.username,
                u.display_name,
                u.avatar_image_key,
                a.comment,
                a.bar_number,
                a.beat_number,
                a.instrument,
                a.system_y_ratio,
                a.created_at
            FROM score_annotations a
            JOIN users u
                ON u.id = a.user_id
            WHERE a.music_id = $1
            ORDER BY a.bar_number ASC, a.beat_number ASC, a.created_at ASC, a.id ASC
            "#,
        )
        .bind::<Text, _>(&record.id)
        .load::<ScoreAnnotationRow>(&mut conn)
        .await?
    } else {
        let auth = auth.ok_or_else(|| AppError::unauthorized("Missing Authorization header"))?;
        diesel::sql_query(
            r#"
            SELECT
                a.id,
                a.music_id,
                a.user_id,
                u.username,
                u.display_name,
                u.avatar_image_key,
                a.comment,
                a.bar_number,
                a.beat_number,
                a.instrument,
                a.system_y_ratio,
                a.created_at
            FROM score_annotations a
            JOIN users u
                ON u.id = a.user_id
            WHERE a.music_id = $1
              AND a.user_id = $2
            ORDER BY a.bar_number ASC, a.beat_number ASC, a.created_at ASC, a.id ASC
            "#,
        )
        .bind::<Text, _>(&record.id)
        .bind::<Text, _>(&auth.user.id)
        .load::<ScoreAnnotationRow>(&mut conn)
        .await?
    };

    Ok(ScoreAnnotationListResponse {
        visibility_scope,
        annotations: rows
            .into_iter()
            .map(|row| score_annotation_response_from_row(&state.storage, row))
            .collect(),
    })
}

pub(crate) async fn create_public_score_annotation(
    state: &AppState,
    access_key: &str,
    auth: &AuthContext,
    payload: CreateScoreAnnotationRequest,
) -> Result<ScoreAnnotationResponse, AppError> {
    let record = find_public_music_record(state, access_key)
        .await?
        .ok_or_else(|| AppError::not_found("Music not found"))?;

    let comment = payload.comment.trim();
    if comment.is_empty() {
        return Err(AppError::bad_request("Annotation comment cannot be empty"));
    }
    if comment.chars().count() > 1000 {
        return Err(AppError::bad_request("Annotation comment is too long"));
    }
    if payload.bar_number <= 0 || payload.beat_number <= 0 {
        return Err(AppError::bad_request("Annotation timestamp is invalid"));
    }

    let instrument = payload.instrument.trim();
    if instrument.is_empty() {
        return Err(AppError::bad_request("Annotation instrument is required"));
    }
    if instrument.chars().count() > 200 {
        return Err(AppError::bad_request("Annotation instrument is too long"));
    }
    let system_y_ratio = match payload.system_y_ratio {
        Some(value) if value.is_finite() && (0.0..=1.0).contains(&value) => Some(value),
        Some(_) => {
            return Err(AppError::bad_request(
                "Annotation vertical position must be between 0 and 1",
            ));
        }
        None => None,
    };

    let created_at = utc_now_string();
    let annotation_id = uuid::Uuid::new_v4().to_string();
    let mut conn = state.db_rw.get().await?;
    diesel::insert_into(score_annotations::table)
        .values(NewScoreAnnotation {
            id: &annotation_id,
            music_id: &record.id,
            user_id: &auth.user.id,
            bar_number: payload.bar_number,
            beat_number: payload.beat_number,
            instrument,
            system_y_ratio,
            comment,
            created_at: &created_at,
        })
        .execute(&mut conn)
        .await?;

    let author = crate::services::auth::find_user_by_id(&state.db_rw, &auth.user.id)
        .await?
        .ok_or_else(|| AppError::not_found("User not found"))?;

    Ok(score_annotation_response_from_user(
        &state.storage,
        &record.id,
        &author,
        &annotation_id,
        comment,
        instrument,
        payload.bar_number,
        payload.beat_number,
        system_y_ratio,
        &created_at,
    ))
}

pub(crate) async fn ensure_public_id_available(
    db: &DbPool,
    public_id: Option<&str>,
    current_music_id: Option<&str>,
) -> Result<(), AppError> {
    let Some(public_id) = public_id else {
        return Ok(());
    };

    let mut conn = db.get().await?;
    let existing = musics::table
        .filter(musics::public_id.eq(Some(public_id)))
        .select(musics::id)
        .first::<String>(&mut conn)
        .await
        .optional()?
        .map(|value| value);

    if let Some(existing_id) = existing {
        if Some(existing_id.as_str()) != current_music_id {
            return Err(AppError::conflict("That public id is already in use"));
        }
    }

    Ok(())
}

pub(crate) async fn find_music_by_id(
    db: &DbPool,
    id: &str,
) -> Result<Option<MusicRecord>, AppError> {
    let mut conn = db.get().await?;
    Ok(musics::table
        .find(id)
        .select(MusicRecord::as_select())
        .first(&mut conn)
        .await
        .optional()?)
}

pub(crate) async fn find_music_by_access_key(
    db: &DbPool,
    access_key: &str,
) -> Result<Option<MusicRecord>, AppError> {
    let mut conn = db.get().await?;
    Ok(musics::table
        .filter(
            musics::public_token
                .eq(access_key)
                .or(musics::public_id.eq(Some(access_key))),
        )
        .select(MusicRecord::as_select())
        .first(&mut conn)
        .await
        .optional()?)
}

pub(crate) async fn find_processing_job_by_music_id(
    db: &DbPool,
    music_id: &str,
) -> Result<Option<ProcessingJobRecord>, AppError> {
    let mut conn = db.get().await?;
    Ok(processing_jobs::table
        .find(music_id)
        .select(ProcessingJobRecord::as_select())
        .first(&mut conn)
        .await
        .optional()?)
}

pub(crate) async fn fetch_processing_job_map(
    db: &DbPool,
) -> Result<HashMap<String, ProcessingJobRecord>, AppError> {
    let mut conn = db.get().await?;
    Ok(processing_jobs::table
        .select(ProcessingJobRecord::as_select())
        .load::<ProcessingJobRecord>(&mut conn)
        .await?
        .into_iter()
        .map(|job| (job.music_id.clone(), job))
        .collect())
}

pub(crate) fn record_to_admin_response(
    config: &AppConfig,
    storage: &Storage,
    record: MusicRecord,
    stems_total_bytes: i64,
    ensemble_ids: Vec<String>,
    ensemble_names: Vec<String>,
    processing_job: Option<&ProcessingJobRecord>,
) -> AdminMusicResponse {
    let public_id_url = record
        .public_id
        .as_ref()
        .map(|public_id| config.public_url_for(public_id));
    let public_url = public_id_url
        .clone()
        .unwrap_or_else(|| config.public_url_for(&record.public_token));
    let midi_download_url = record.midi_object_key.as_ref().map(|object_key| {
        storage
            .public_url(object_key)
            .unwrap_or_else(|| format!("/api/public/{}/midi", record.public_token))
    });
    let download_url = storage
        .public_url(&record.object_key)
        .unwrap_or_else(|| format!("/api/public/{}/download", record.public_token));

    let icon_image_url = record.icon_image_key.as_ref().map(|key| {
        storage
            .public_url(key)
            .unwrap_or_else(|| format!("/api/public/{}/icon", record.public_token))
    });

    AdminMusicResponse {
        id: record.id,
        title: record.title,
        subtitle: record.subtitle,
        icon: record.icon.clone(),
        icon_image_url,
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
        public_url,
        public_id_url,
        download_url,
        midi_download_url,
        quality_profile: record.quality_profile,
        created_at: record.created_at,
        stems_total_bytes,
        ensemble_ids,
        ensemble_names,
        owner_user_id: record.owner_user_id,
        processing_job_status: processing_job.map(|job| job.status.clone()),
        processing_job_step: processing_job.map(|job| job.current_step.clone()),
        processing_job_attempt: processing_job.map(|job| job.attempt),
        processing_job_lease_expires_at: processing_job
            .and_then(|job| job.lease_expires_at.clone()),
        processing_job_heartbeat_at: processing_job.and_then(|job| job.heartbeat_at.clone()),
        processing_job_error: processing_job.and_then(|job| job.error_message.clone()),
    }
}

pub(crate) fn build_admin_music_processing_progress_response(
    record: &MusicRecord,
    processing_job: Option<&ProcessingJobRecord>,
    _processing_log: &str,
) -> AdminMusicProcessingProgressResponse {
    let job_status = processing_job.map(|job| job.status.clone());
    let job_step = processing_job.map(|job| job.current_step.clone());
    let job_started_at = processing_job.and_then(|job| job.started_at.clone());
    let job_finished_at = processing_job.and_then(|job| job.finished_at.clone());
    let job_queued_at = processing_job.map(|job| job.queued_at.clone());
    let lease_expires_at = processing_job.and_then(|job| job.lease_expires_at.clone());
    let heartbeat_at = processing_job.and_then(|job| job.heartbeat_at.clone());
    let stalled = job_status.as_deref() == Some("running")
        && lease_expires_at
            .as_deref()
            .and_then(parse_rfc3339_utc)
            .is_some_and(|lease| lease <= Utc::now());
    let complete = crate::processing::processing_statuses(record)
        .iter()
        .all(|status| *status == "ready")
        || job_status.as_deref() == Some("completed");
    let progress = processing_job
        .and_then(|job| job.progress_json.as_deref())
        .and_then(parse_processing_job_progress);

    let queue_status = if job_status.as_deref() == Some("queued") {
        step_status_with_stall(
            progress_step_status(progress.as_ref(), "queue").unwrap_or("active"),
            false,
        )
    } else if processing_job.is_some() || job_started_at.is_some() {
        "done"
    } else {
        "pending"
    };
    let queue_detail = progress_step_detail(progress.as_ref(), "queue");

    let musicxml_status = structured_step_status(
        progress.as_ref(),
        "musicxml",
        record.musicxml_status.as_str(),
        false,
        stalled && job_step.as_deref() == Some("generating_core"),
    );
    let midi_status = structured_step_status(
        progress.as_ref(),
        "midi",
        record.midi_status.as_str(),
        false,
        stalled && job_step.as_deref() == Some("generating_core"),
    );
    let preview_status = structured_step_status(
        progress.as_ref(),
        "preview_mp3",
        record.audio_status.as_str(),
        false,
        stalled && job_step.as_deref() == Some("generating_core"),
    );
    let stems_status = structured_step_status(
        progress.as_ref(),
        "stems",
        record.stems_status.as_str(),
        false,
        stalled && job_step.as_deref() == Some("generating_stems"),
    );
    let upload_done = upload_status_would_be_done(record, complete);
    let compression_enabled = stem_profile_uses_compression(&record.quality_profile);
    let compression_status = if !compression_enabled {
        if stems_status == "done" || upload_done {
            "done"
        } else {
            "pending"
        }
    } else {
        structured_step_status(
            progress.as_ref(),
            "compress_stems",
            if record.stems_status == "ready" || complete {
                "ready"
            } else {
                record.stems_status.as_str()
            },
            false,
            stalled && job_step.as_deref() == Some("generating_stems"),
        )
    };
    let upload_status = if upload_done {
        "done"
    } else {
        structured_step_status(
            progress.as_ref(),
            "upload_assets",
            if complete { "ready" } else { "processing" },
            false,
            stalled && job_step.as_deref() == Some("uploading_assets"),
        )
    };
    let upload_detail = if upload_done {
        complete_progress_ratio(progress_step_detail(progress.as_ref(), "upload_assets"))
    } else {
        progress_step_detail(progress.as_ref(), "upload_assets")
    };
    let upload_last_updated_at = progress_step_last_updated_at(progress.as_ref(), "upload_assets");
    let done_status = structured_step_status(
        progress.as_ref(),
        "done",
        if complete {
            "ready"
        } else if job_status.as_deref() == Some("failed") {
            "failed"
        } else {
            "processing"
        },
        false,
        stalled && job_step.as_deref() == Some("finalizing"),
    );

    let mut steps = vec![
        build_processing_step(
            "upload",
            "Upload",
            "done",
            None,
            Some(record.created_at.clone()),
            Some(format!("Score uploaded on {}", record.created_at)),
            None,
        ),
        build_processing_step(
            "queue",
            "Queue",
            queue_status,
            queue_detail.clone(),
            progress_step_last_updated_at(progress.as_ref(), "queue")
                .or(job_started_at.clone())
                .or(job_queued_at.clone()),
            progress_step_tooltip(progress.as_ref(), "queue").or_else(|| {
                build_processing_tooltip(
                    queue_detail,
                    progress_step_last_updated_at(progress.as_ref(), "queue")
                        .or(job_started_at.clone())
                        .or(job_queued_at.clone()),
                    Some(format!("Score ID: {}", record.id)),
                )
            }),
            None,
        ),
        build_processing_step(
            "musicxml",
            "MusicXML",
            musicxml_status,
            progress_step_detail(progress.as_ref(), "musicxml")
                .or_else(|| failure_detail(record.musicxml_error.as_deref(), musicxml_status)),
            progress_step_last_updated_at(progress.as_ref(), "musicxml"),
            progress_step_tooltip(progress.as_ref(), "musicxml").or_else(|| {
                build_processing_tooltip(
                    failure_detail(record.musicxml_error.as_deref(), musicxml_status),
                    progress_step_last_updated_at(progress.as_ref(), "musicxml"),
                    Some("MusicXML export".to_owned()),
                )
            }),
            Some("core_exports"),
        ),
        build_processing_step(
            "midi",
            "MIDI",
            midi_status,
            progress_step_detail(progress.as_ref(), "midi")
                .or_else(|| failure_detail(record.midi_error.as_deref(), midi_status)),
            progress_step_last_updated_at(progress.as_ref(), "midi"),
            progress_step_tooltip(progress.as_ref(), "midi").or_else(|| {
                build_processing_tooltip(
                    failure_detail(record.midi_error.as_deref(), midi_status),
                    progress_step_last_updated_at(progress.as_ref(), "midi"),
                    Some("MIDI export".to_owned()),
                )
            }),
            Some("core_exports"),
        ),
        build_processing_step(
            "preview_mp3",
            "Audio",
            preview_status,
            progress_step_detail(progress.as_ref(), "preview_mp3")
                .or_else(|| failure_detail(record.audio_error.as_deref(), preview_status)),
            progress_step_last_updated_at(progress.as_ref(), "preview_mp3"),
            progress_step_tooltip(progress.as_ref(), "preview_mp3").or_else(|| {
                build_processing_tooltip(
                    failure_detail(record.audio_error.as_deref(), preview_status),
                    progress_step_last_updated_at(progress.as_ref(), "preview_mp3"),
                    Some("Audio rendering".to_owned()),
                )
            }),
            Some("core_exports"),
        ),
        build_processing_step(
            "stems",
            "Stems",
            stems_status,
            progress_step_detail(progress.as_ref(), "stems")
                .or_else(|| failure_detail(record.stems_error.as_deref(), stems_status)),
            progress_step_last_updated_at(progress.as_ref(), "stems"),
            progress_step_tooltip(progress.as_ref(), "stems").or_else(|| {
                build_processing_tooltip(
                    failure_detail(record.stems_error.as_deref(), stems_status),
                    progress_step_last_updated_at(progress.as_ref(), "stems"),
                    None,
                )
            }),
            None,
        ),
        build_processing_step(
            "compress_stems",
            "Compress",
            compression_status,
            if compression_enabled {
                progress_step_detail(progress.as_ref(), "compress_stems")
            } else {
                None
            },
            progress_step_last_updated_at(progress.as_ref(), "compress_stems"),
            if compression_enabled {
                progress_step_tooltip(progress.as_ref(), "compress_stems").or_else(|| {
                    build_processing_tooltip(
                        failure_detail(record.stems_error.as_deref(), compression_status),
                        progress_step_last_updated_at(progress.as_ref(), "compress_stems"),
                        Some(format!(
                            "Stem compression profile: {}",
                            record.quality_profile
                        )),
                    )
                })
            } else {
                Some("No extra stem compression for this profile.".to_owned())
            },
            None,
        ),
        build_processing_step(
            "upload_assets",
            "Upload",
            upload_status,
            upload_detail.clone(),
            upload_last_updated_at.clone(),
            if upload_done {
                build_processing_tooltip(
                    upload_detail.clone(),
                    upload_last_updated_at.clone(),
                    Some("All derived assets uploaded.".to_owned()),
                )
            } else {
                progress_step_tooltip(progress.as_ref(), "upload_assets").or_else(|| {
                    build_processing_tooltip(
                        upload_detail.clone(),
                        upload_last_updated_at.clone(),
                        None,
                    )
                })
            },
            None,
        ),
        build_processing_step(
            "done",
            "Done",
            done_status,
            progress_step_detail(progress.as_ref(), "done"),
            job_finished_at
                .clone()
                .or(progress_step_last_updated_at(progress.as_ref(), "done")),
            progress_step_tooltip(progress.as_ref(), "done").or_else(|| {
                build_processing_tooltip(
                    progress_step_detail(progress.as_ref(), "done"),
                    job_finished_at
                        .clone()
                        .or(progress_step_last_updated_at(progress.as_ref(), "done")),
                    if complete {
                        Some("Processing completed successfully.".to_owned())
                    } else {
                        None
                    },
                )
            }),
            None,
        ),
    ];

    if job_status.as_deref() == Some("failed") {
        match job_step.as_deref() {
            Some("fetching_input") => steps[1].status = "failed".to_owned(),
            Some("generating_core") => {
                if steps[2].status == "failed" || record.musicxml_status == "failed" {
                    steps[2].status = "failed".to_owned();
                } else if steps[3].status == "failed" || record.midi_status == "failed" {
                    steps[3].status = "failed".to_owned();
                } else if steps[4].status == "failed" || record.audio_status == "failed" {
                    steps[4].status = "failed".to_owned();
                } else {
                    steps[2].status = "failed".to_owned();
                    steps[3].status = "failed".to_owned();
                    steps[4].status = "failed".to_owned();
                }
            }
            Some("generating_stems") => {
                if compression_enabled && steps[6].last_updated_at.is_some() {
                    steps[6].status = "failed".to_owned();
                } else {
                    steps[5].status = "failed".to_owned();
                }
            }
            Some("uploading_assets") => steps[7].status = "failed".to_owned(),
            Some("finalizing") => steps[8].status = "failed".to_owned(),
            _ => {}
        }
    }

    let state_message = if stalled {
        heartbeat_at.as_ref().map_or_else(
            || Some("Processor worker lease expired. Waiting for another worker to reclaim the job.".to_owned()),
            |heartbeat| {
                Some(format!(
                    "Processor worker heartbeat stopped after {}. Waiting for another worker to reclaim the job.",
                    heartbeat
                ))
            },
        )
    } else if let Some(error) = processing_job.and_then(|job| job.error_message.clone()) {
        Some(error)
    } else if complete {
        Some("Processing completed successfully.".to_owned())
    } else if job_status.as_deref() == Some("queued") {
        Some("Processing is queued and waiting for a worker.".to_owned())
    } else {
        None
    };

    AdminMusicProcessingProgressResponse {
        music_id: record.id.clone(),
        processing_job_status: job_status,
        processing_job_step: job_step,
        processing_job_attempt: processing_job.map(|job| job.attempt),
        processing_job_error: processing_job.and_then(|job| job.error_message.clone()),
        processing_job_lease_expires_at: lease_expires_at,
        processing_job_heartbeat_at: heartbeat_at,
        stalled,
        state_message,
        steps,
    }
}

fn build_processing_step(
    key: &str,
    label: &str,
    status: &str,
    detail: Option<String>,
    last_updated_at: Option<String>,
    tooltip: Option<String>,
    group: Option<&str>,
) -> AdminMusicProcessingStepResponse {
    AdminMusicProcessingStepResponse {
        key: key.to_owned(),
        label: label.to_owned(),
        detail,
        status: status.to_owned(),
        last_updated_at,
        tooltip,
        group: group.map(ToOwned::to_owned),
    }
}

fn build_processing_tooltip(
    detail: Option<String>,
    last_updated_at: Option<String>,
    fallback: Option<String>,
) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(detail) = detail {
        if !detail.trim().is_empty() {
            parts.push(detail);
        }
    } else if let Some(fallback) = fallback {
        parts.push(fallback);
    }
    if let Some(timestamp) = last_updated_at {
        parts.push(format!("Last update: {timestamp}"));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n"))
    }
}

fn parse_processing_job_progress(value: &str) -> Option<ProcessingJobProgress> {
    serde_json::from_str(value).ok()
}

fn progress_step<'a>(
    progress: Option<&'a ProcessingJobProgress>,
    key: &str,
) -> Option<&'a fumen_core::models::ProcessingJobProgressStep> {
    progress.and_then(|progress| progress.steps.get(key))
}

fn progress_step_status<'a>(
    progress: Option<&'a ProcessingJobProgress>,
    key: &str,
) -> Option<&'a str> {
    progress_step(progress, key).and_then(|step| step.status.as_deref())
}

fn progress_step_detail(progress: Option<&ProcessingJobProgress>, key: &str) -> Option<String> {
    progress_step(progress, key).and_then(|step| step.detail.clone())
}

fn progress_step_last_updated_at(
    progress: Option<&ProcessingJobProgress>,
    key: &str,
) -> Option<String> {
    progress_step(progress, key).and_then(|step| step.last_updated_at.clone())
}

fn progress_step_tooltip(progress: Option<&ProcessingJobProgress>, key: &str) -> Option<String> {
    progress_step(progress, key).and_then(|step| step.tooltip.clone())
}

fn normalize_progress_step_status(status: &str) -> &'static str {
    match status {
        "done" | "ready" | "unavailable" => "done",
        "failed" => "failed",
        "stalled" => "stalled",
        "active" | "processing" => "active",
        _ => "pending",
    }
}

fn step_status_with_stall(status: &str, stalled: bool) -> &'static str {
    if stalled && status == "active" {
        "stalled"
    } else {
        match status {
            "done" => "done",
            "failed" => "failed",
            "stalled" => "stalled",
            "active" => "active",
            _ => "pending",
        }
    }
}

fn structured_step_status(
    progress: Option<&ProcessingJobProgress>,
    key: &str,
    record_status: &str,
    fallback_done: bool,
    stalled: bool,
) -> &'static str {
    if let Some(status) = progress_step_status(progress, key) {
        return step_status_with_stall(normalize_progress_step_status(status), stalled);
    }

    if fallback_done {
        return "done";
    }

    match record_status {
        "ready" | "unavailable" => "done",
        "failed" => "failed",
        _ => "pending",
    }
}

fn failure_detail(error: Option<&str>, status: &str) -> Option<String> {
    if status == "failed" {
        error.map(ToOwned::to_owned)
    } else {
        None
    }
}

fn stem_profile_uses_compression(profile: &str) -> bool {
    !matches!(
        profile.trim().to_ascii_lowercase().as_str(),
        "" | "standard"
    )
}

fn complete_progress_ratio(detail: Option<String>) -> Option<String> {
    detail.map(|detail| {
        if let Some((_, total)) = detail.split_once('/') {
            let total = total.trim();
            format!("{total} / {total}")
        } else {
            detail
        }
    })
}

fn upload_status_would_be_done(record: &MusicRecord, complete: bool) -> bool {
    complete
        || (record.audio_status != "processing"
            && record.midi_status != "processing"
            && record.musicxml_status != "processing"
            && record.stems_status != "processing")
}

fn parse_rfc3339_utc(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|datetime| datetime.with_timezone(&Utc))
}

pub(crate) fn record_to_public_response(
    storage: &Storage,
    record: MusicRecord,
    access_key: &str,
) -> PublicMusicResponse {
    let audio_stream_url = record.audio_object_key.as_ref().map(|object_key| {
        storage
            .public_url(object_key)
            .unwrap_or_else(|| format!("/api/public/{access_key}/audio"))
    });
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
    let icon_image_url = record.icon_image_key.as_ref().map(|key| {
        storage
            .public_url(key)
            .unwrap_or_else(|| format!("/api/public/{access_key}/icon"))
    });

    PublicMusicResponse {
        title: record.title,
        subtitle: record.subtitle,
        icon: record.icon,
        icon_image_url,
        filename: record.filename,
        audio_status: record.audio_status,
        audio_error: record.audio_error,
        can_stream_audio: audio_stream_url.is_some(),
        audio_stream_url,
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

#[tracing::instrument(skip(state), fields(music_id = %music_id))]
pub(crate) async fn delete_music_record_and_assets(
    state: &AppState,
    music_id: &str,
) -> Result<(), AppError> {
    let record = find_music_by_id(&state.db_rw, music_id)
        .await?
        .ok_or_else(|| AppError::not_found("Music not found"))?;
    let stems = find_public_stems(&state.db_rw, &state.db_rw, music_id).await?;

    let mut conn = state.db_rw.get().await?;
    diesel::delete(stems::table.filter(stems::music_id.eq(music_id)))
        .execute(&mut conn)
        .await?;
    diesel::delete(musics::table.find(music_id))
        .execute(&mut conn)
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

    keys.push(processing_log_key(music_id));

    for key in keys {
        if let Err(error) = state.storage.delete_key(&key).await {
            warn!("failed to delete storage object {key}: {error}");
        }
    }

    Ok(())
}

pub(crate) fn midi_filename_for(filename: &str) -> String {
    let stem = filename
        .trim_end_matches(".mscz")
        .trim_end_matches(".MSCZ")
        .trim_end_matches(".mscx")
        .trim_end_matches(".MSCX");
    sanitize_content_disposition(&format!("{stem}.mid"))
}
