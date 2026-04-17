use crate::config::AppConfig;
use crate::db::DbPool;
use crate::models::{
    BigIntValueRow, EnsembleRecord, MusicEnsembleLinkRecord, MusicRecord, NewScoreAnnotation,
    NewStem, NewUserMusicTrackPlaytime, StemRecord, UserEnsembleMembershipRecord, UserRecord,
};
use crate::schema::{
    ensembles, music_ensemble_links, musics, score_annotations, stems, user_ensemble_memberships,
    user_music_track_playtime,
};
use crate::schemas::{
    AdminMusicPlaytimeResponse, AdminMusicResponse, AdminUserScorePlaytimeResponse,
    CreateScoreAnnotationRequest, MusicPlaytimeLeaderboardEntryResponse,
    MusicPlaytimeTrackSummaryResponse, PublicMusicResponse, ScoreAnnotationListResponse,
    ScoreAnnotationResponse, StemInfo,
};
use crate::storage::Storage;
use crate::{
    AppError, AppRole, AppState, AuthContext, sanitize_content_disposition, utc_now_string,
};
use anyhow::anyhow;
use bytes::Bytes;
use diesel::OptionalExtension;
use diesel::QueryableByName;
use diesel::prelude::*;
use diesel::sql_types::{BigInt, Double, Nullable, Text};
use diesel::upsert::excluded;
use diesel_async::{AsyncConnection, RunQueryDsl};
use flate2::{Compression, write::GzEncoder};
use std::collections::HashMap;
use std::io::Write;
use tokio::fs;
use tokio::process::Command;
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
            if owns_score { "all" } else { "own" }
        }
        AppRole::User => "own",
        AppRole::Superadmin | AppRole::Admin => "all",
    }
}

fn score_annotation_response_from_row(
    storage: &Storage,
    row: ScoreAnnotationRow,
) -> ScoreAnnotationResponse {
    let avatar_url = resolve_user_avatar_url(storage, &row.user_id, row.avatar_image_key.as_deref());

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

pub(crate) async fn find_public_stems(
    db_primary: &DbPool,
    db_fallback: &DbPool,
    music_id: &str,
) -> Result<Vec<StemRecord>, AppError> {
    let mut primary = db_primary.get().await?;
    let stems = stems::table
        .filter(stems::music_id.eq(music_id))
        .order(stems::track_index.asc())
        .select(StemRecord::as_select())
        .load(&mut primary)
        .await?;

    if !stems.is_empty() {
        return Ok(stems);
    }

    let mut fallback = db_fallback.get().await?;
    Ok(stems::table
        .filter(stems::music_id.eq(music_id))
        .order(stems::track_index.asc())
        .select(StemRecord::as_select())
        .load(&mut fallback)
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

#[tracing::instrument(
    skip(state, stems, error),
    fields(music_id = %music_id, stem_count = stems.len(), stems_status = status)
)]
pub(crate) async fn store_stems(
    state: &AppState,
    music_id: &str,
    stems: Vec<crate::audio::StemResult>,
    status: String,
    error: Option<String>,
) -> Result<(String, Option<String>), AppError> {
    let mut conn = state.db_rw.get().await?;

    for stem in stems {
        let size_bytes = stem.bytes.len() as i64;
        let storage_key = format!("stems/{music_id}/{}.ogg", stem.track_index);
        let drum_map_json = stem
            .drum_map
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| AppError::from(anyhow::Error::from(error)))?;
        state
            .storage
            .upload_bytes(&storage_key, stem.bytes.clone(), "audio/ogg")
            .await?;

        diesel::insert_into(stems::table)
            .values(NewStem {
                music_id,
                track_index: stem.track_index as i64,
                track_name: &stem.track_name,
                instrument_name: &stem.instrument_name,
                storage_key: &storage_key,
                size_bytes,
                drum_map_json: drum_map_json.as_deref(),
            })
            .execute(&mut conn)
            .await?;
    }
    Ok((status, error))
}

pub(crate) async fn probe_audio_duration_seconds(path: &std::path::Path) -> Result<f64, AppError> {
    let output = Command::new("ffprobe")
        .arg("-v")
        .arg("error")
        .arg("-show_entries")
        .arg("format=duration")
        .arg("-of")
        .arg("default=noprint_wrappers=1:nokey=1")
        .arg(path)
        .output()
        .await
        .map_err(AppError::from)?;

    if !output.status.success() {
        return Err(AppError::from(anyhow!(
            "ffprobe failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    let duration = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<f64>()
        .map_err(|error| AppError::from(anyhow!("invalid ffprobe duration: {error}")))?;
    Ok(duration)
}

pub(crate) async fn build_public_stem_infos(
    state: &AppState,
    access_key: &str,
    music_id: &str,
) -> Result<Vec<StemInfo>, AppError> {
    let stems = find_public_stems(&state.db_ro, &state.db_rw, music_id).await?;
    let mut resolved_infos = Vec::new();
    for stem in stems {
        let full_stem_url = state
            .storage
            .public_url(&stem.storage_key)
            .unwrap_or_else(|| format!("/api/public/{access_key}/stems/{}", stem.track_index));
        let duration_seconds =
            if let Some(path) = state.storage.local_path_for_key(&stem.storage_key) {
                probe_audio_duration_seconds(&path).await?
            } else {
                let (stem_bytes, _, _) = state.storage.get_bytes(&stem.storage_key).await?;
                let temp_dir = tempfile::tempdir()?;
                let full_stem_path = temp_dir.path().join("stem.ogg");
                fs::write(&full_stem_path, stem_bytes).await?;
                probe_audio_duration_seconds(&full_stem_path).await?
            };

        resolved_infos.push(StemInfo {
            track_index: stem.track_index,
            track_name: stem.track_name,
            instrument_name: stem.instrument_name,
            full_stem_url,
            duration_seconds,
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
    let visibility_scope = score_annotation_visibility_scope(auth, &record, &music_ensemble_ids)
        .to_owned();

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
        &created_at,
    ))
}

#[tracing::instrument(skip(state, outcome), fields(music_id = %music_id, kind = kind))]
pub(crate) async fn store_conversion(
    state: &AppState,
    music_id: &str,
    kind: &str,
    outcome: crate::audio::ConversionOutcome,
) -> Result<(Option<String>, String, Option<String>), AppError> {
    match outcome {
        crate::audio::ConversionOutcome::Ready {
            bytes,
            content_type,
            extension,
        } => {
            let object_key = format!("{kind}/{music_id}.{extension}");
            let (stored_bytes, content_encoding) = if kind == "musicxml" && state.storage.is_s3() {
                (gzip_bytes(&bytes)?, Some("gzip"))
            } else {
                (bytes, None)
            };
            state
                .storage
                .upload_bytes_with_encoding(
                    &object_key,
                    stored_bytes,
                    content_type,
                    content_encoding,
                )
                .await?;
            Ok((Some(object_key), "ready".to_owned(), None))
        }
        crate::audio::ConversionOutcome::Unavailable { reason } => {
            Ok((None, "unavailable".to_owned(), Some(reason)))
        }
        crate::audio::ConversionOutcome::Failed { reason } => {
            warn!("{kind} conversion failed for {music_id}: {reason}");
            Ok((None, "failed".to_owned(), Some(reason)))
        }
    }
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

pub(crate) fn record_to_admin_response(
    config: &AppConfig,
    storage: &Storage,
    record: MusicRecord,
    stems_total_bytes: i64,
    ensemble_ids: Vec<String>,
    ensemble_names: Vec<String>,
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
    }
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

fn gzip_bytes(bytes: &Bytes) -> Result<Bytes, AppError> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes).map_err(AppError::from)?;
    let compressed = encoder.finish().map_err(AppError::from)?;
    Ok(Bytes::from(compressed))
}
