use crate::config::AppConfig;
use crate::models::{
    EnsembleRecord, EnsembleSummaryRecord, MusicEnsembleLinkRecord, MusicRecord, StemRecord,
    UserEnsembleMembershipRecord,
};
use crate::schemas::{AdminMusicResponse, PublicMusicResponse, StemInfo};
use crate::storage::Storage;
use crate::{
    AppError, AppRole, AppState, AuthContext, sanitize_content_disposition,
};
use anyhow::anyhow;
use bytes::Bytes;
use flate2::{Compression, write::GzEncoder};
use sqlx::PgPool;
use std::collections::HashMap;
use std::io::Write;
use tokio::fs;
use tokio::process::Command;
use tracing::warn;

#[derive(sqlx::FromRow)]
struct EnsembleScoreCountRow {
    ensemble_id: String,
    score_count: i64,
}

#[derive(sqlx::FromRow)]
struct UserAccessibleMusicRow {
    id: String,
    title: String,
    icon: Option<String>,
    icon_image_key: Option<String>,
    filename: String,
    content_type: String,
    object_key: String,
    audio_object_key: Option<String>,
    audio_status: String,
    audio_error: Option<String>,
    midi_object_key: Option<String>,
    midi_status: String,
    midi_error: Option<String>,
    musicxml_object_key: Option<String>,
    musicxml_status: String,
    musicxml_error: Option<String>,
    stems_status: String,
    stems_error: Option<String>,
    public_token: String,
    public_id: Option<String>,
    quality_profile: String,
    created_at: String,
    owner_user_id: Option<String>,
    ensemble_id: String,
    ensemble_name: String,
}

fn accessible_music_row_to_tuple(row: UserAccessibleMusicRow) -> (MusicRecord, String, String) {
    (
        MusicRecord {
            id: row.id,
            title: row.title,
            icon: row.icon,
            icon_image_key: row.icon_image_key,
            filename: row.filename,
            content_type: row.content_type,
            object_key: row.object_key,
            audio_object_key: row.audio_object_key,
            audio_status: row.audio_status,
            audio_error: row.audio_error,
            midi_object_key: row.midi_object_key,
            midi_status: row.midi_status,
            midi_error: row.midi_error,
            musicxml_object_key: row.musicxml_object_key,
            musicxml_status: row.musicxml_status,
            musicxml_error: row.musicxml_error,
            stems_status: row.stems_status,
            stems_error: row.stems_error,
            public_token: row.public_token,
            public_id: row.public_id,
            quality_profile: row.quality_profile,
            created_at: row.created_at,
            owner_user_id: row.owner_user_id,
        },
        row.ensemble_id,
        row.ensemble_name,
    )
}

pub(crate) async fn fetch_stems_total(db: &PgPool, music_id: &str) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE(SUM(size_bytes), 0)::BIGINT FROM stems WHERE music_id = $1",
    )
    .bind(music_id)
    .fetch_one(db)
    .await
    .unwrap_or(0)
}

pub(crate) async fn find_ensemble_by_id(
    db: &PgPool,
    id: &str,
) -> Result<Option<EnsembleRecord>, AppError> {
    Ok(sqlx::query_as::<_, EnsembleRecord>(
        "SELECT id, name, created_at, created_by_user_id FROM ensembles WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(db)
    .await?)
}

pub(crate) async fn find_ensemble_by_name(
    db: &PgPool,
    name: &str,
) -> Result<Option<EnsembleRecord>, AppError> {
    Ok(sqlx::query_as::<_, EnsembleRecord>(
        "SELECT id, name, created_at, created_by_user_id FROM ensembles WHERE name = $1",
    )
    .bind(name)
    .fetch_optional(db)
    .await?)
}

pub(crate) async fn fetch_user_ensemble_memberships(
    db: &PgPool,
) -> Result<Vec<UserEnsembleMembershipRecord>, AppError> {
    Ok(sqlx::query_as::<_, UserEnsembleMembershipRecord>(
        "SELECT user_id, ensemble_id, role FROM user_ensemble_memberships",
    )
    .fetch_all(db)
    .await?)
}

pub(crate) async fn fetch_music_ensemble_links(
    db: &PgPool,
) -> Result<Vec<MusicEnsembleLinkRecord>, AppError> {
    Ok(sqlx::query_as::<_, MusicEnsembleLinkRecord>(
        "SELECT music_id, ensemble_id FROM music_ensemble_links",
    )
    .fetch_all(db)
    .await?)
}

pub(crate) async fn fetch_music_ensemble_ids(
    db: &PgPool,
    music_id: &str,
) -> Result<Vec<String>, AppError> {
    Ok(sqlx::query_scalar::<_, String>(
        "SELECT ensemble_id FROM music_ensemble_links WHERE music_id = $1 ORDER BY ensemble_id ASC",
    )
    .bind(music_id)
    .fetch_all(db)
    .await?)
}

pub(crate) async fn fetch_ensemble_summaries(
    db: &PgPool,
) -> Result<HashMap<String, String>, AppError> {
    Ok(sqlx::query_as::<_, EnsembleSummaryRecord>(
        "SELECT id, name FROM ensembles ORDER BY name ASC",
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|ensemble| (ensemble.id, ensemble.name))
    .collect())
}

pub(crate) async fn fetch_ensemble_score_counts(
    db: &PgPool,
) -> Result<Vec<(String, i64)>, AppError> {
    Ok(sqlx::query_as::<_, EnsembleScoreCountRow>(
        "SELECT ensemble_id, COUNT(DISTINCT music_id)::BIGINT AS score_count FROM music_ensemble_links GROUP BY ensemble_id",
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|row| (row.ensemble_id, row.score_count))
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
    db: &PgPool,
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
    db: &PgPool,
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
    db: &PgPool,
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
    db: &PgPool,
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
    db: &PgPool,
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
    db: &PgPool,
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
    db: &PgPool,
) -> Result<Vec<(MusicRecord, String, String)>, AppError> {
    Ok(sqlx::query_as::<_, UserAccessibleMusicRow>(
        r#"
            SELECT m.id, m.title, m.icon, m.icon_image_key, m.filename, m.content_type, m.object_key, m.audio_object_key, m.audio_status, m.audio_error, m.midi_object_key, m.midi_status, m.midi_error, m.musicxml_object_key, m.musicxml_status, m.musicxml_error, m.stems_status, m.stems_error, m.public_token, m.public_id, m.quality_profile, m.created_at, m.owner_user_id, mel.ensemble_id, e.name AS ensemble_name
        FROM musics m
        JOIN music_ensemble_links mel ON mel.music_id = m.id
        JOIN ensembles e ON e.id = mel.ensemble_id
        ORDER BY e.name ASC, m.title ASC
        "#,
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(accessible_music_row_to_tuple)
    .collect())
}

pub(crate) async fn find_accessible_music_for_user(
    db: &PgPool,
    user_id: &str,
) -> Result<Vec<(MusicRecord, String, String)>, AppError> {
    Ok(sqlx::query_as::<_, UserAccessibleMusicRow>(
        r#"
            SELECT DISTINCT m.id, m.title, m.icon, m.icon_image_key, m.filename, m.content_type, m.object_key, m.audio_object_key, m.audio_status, m.audio_error, m.midi_object_key, m.midi_status, m.midi_error, m.musicxml_object_key, m.musicxml_status, m.musicxml_error, m.stems_status, m.stems_error, m.public_token, m.public_id, m.quality_profile, m.created_at, m.owner_user_id, mel.ensemble_id, e.name AS ensemble_name
        FROM musics m
        JOIN music_ensemble_links mel ON mel.music_id = m.id
        JOIN user_ensemble_memberships uem ON uem.ensemble_id = mel.ensemble_id
        JOIN ensembles e ON e.id = mel.ensemble_id
        WHERE uem.user_id = $1
        ORDER BY e.name ASC, m.title ASC
        "#,
    )
    .bind(user_id)
    .fetch_all(db)
    .await?
    .into_iter()
    .map(accessible_music_row_to_tuple)
    .collect())
}

pub(crate) async fn find_public_stems(
    db_primary: &PgPool,
    db_fallback: &PgPool,
    music_id: &str,
) -> Result<Vec<StemRecord>, AppError> {
    let query = "SELECT id, music_id, track_index, track_name, instrument_name, storage_key, drum_map_json \
         FROM stems WHERE music_id = $1 ORDER BY track_index";

    let stems = sqlx::query_as::<_, StemRecord>(query)
        .bind(music_id)
        .fetch_all(db_primary)
        .await?;

    if !stems.is_empty() {
        return Ok(stems);
    }

    Ok(sqlx::query_as::<_, StemRecord>(query)
        .bind(music_id)
        .fetch_all(db_fallback)
        .await?)
}

pub(crate) async fn find_public_stem(
    db_primary: &PgPool,
    db_fallback: &PgPool,
    music_id: &str,
    track_index: i64,
) -> Result<Option<StemRecord>, AppError> {
    let query = "SELECT id, music_id, track_index, track_name, instrument_name, storage_key, drum_map_json \
         FROM stems WHERE music_id = $1 AND track_index = $2";

    if let Some(stem) = sqlx::query_as::<_, StemRecord>(query)
        .bind(music_id)
        .bind(track_index)
        .fetch_optional(db_primary)
        .await?
    {
        return Ok(Some(stem));
    }

    Ok(sqlx::query_as::<_, StemRecord>(query)
        .bind(music_id)
        .bind(track_index)
        .fetch_optional(db_fallback)
        .await?)
}

pub(crate) async fn store_stems(
    state: &AppState,
    music_id: &str,
    stems: Vec<crate::audio::StemResult>,
    status: String,
    error: Option<String>,
) -> Result<(String, Option<String>), AppError> {
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

        sqlx::query(
            "INSERT INTO stems (music_id, track_index, track_name, instrument_name, storage_key, size_bytes, drum_map_json) \
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(music_id)
        .bind(stem.track_index as i64)
        .bind(&stem.track_name)
        .bind(&stem.instrument_name)
        .bind(&storage_key)
        .bind(size_bytes)
        .bind(&drum_map_json)
        .execute(&state.db_rw)
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
    db: &PgPool,
    public_id: Option<&str>,
    current_music_id: Option<&str>,
) -> Result<(), AppError> {
    let Some(public_id) = public_id else {
        return Ok(());
    };

    let existing = sqlx::query_scalar::<_, String>("SELECT id FROM musics WHERE public_id = $1")
        .bind(public_id)
        .fetch_optional(db)
        .await?;

    if let Some(existing_id) = existing {
        if Some(existing_id.as_str()) != current_music_id {
            return Err(AppError::conflict("That public id is already in use"));
        }
    }

    Ok(())
}

pub(crate) async fn find_music_by_id(
    db: &PgPool,
    id: &str,
) -> Result<Option<MusicRecord>, AppError> {
    Ok(sqlx::query_as::<_, MusicRecord>(
        r#"
        SELECT id, title, icon, icon_image_key, filename, content_type, object_key, audio_object_key, audio_status, audio_error, midi_object_key, midi_status, midi_error, musicxml_object_key, musicxml_status, musicxml_error, stems_status, stems_error, public_token, public_id, quality_profile, created_at, owner_user_id
        FROM musics
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(db)
    .await?)
}

pub(crate) async fn find_music_by_access_key(
    db: &PgPool,
    access_key: &str,
) -> Result<Option<MusicRecord>, AppError> {
    Ok(sqlx::query_as::<_, MusicRecord>(
        r#"
        SELECT id, title, icon, icon_image_key, filename, content_type, object_key, audio_object_key, audio_status, audio_error, midi_object_key, midi_status, midi_error, musicxml_object_key, musicxml_status, musicxml_error, stems_status, stems_error, public_token, public_id, quality_profile, created_at, owner_user_id
        FROM musics
        WHERE public_token = $1 OR public_id = $2
        LIMIT 1
        "#,
    )
    .bind(access_key)
    .bind(access_key)
    .fetch_optional(db)
    .await?)
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
        public_url: config.public_url_for(&record.public_token),
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

pub(crate) async fn delete_music_record_and_assets(
    state: &AppState,
    music_id: &str,
) -> Result<(), AppError> {
    let record = find_music_by_id(&state.db_rw, music_id)
        .await?
        .ok_or_else(|| AppError::not_found("Music not found"))?;
    let stems = find_public_stems(&state.db_rw, &state.db_rw, music_id).await?;

    sqlx::query("DELETE FROM stems WHERE music_id = $1")
        .bind(music_id)
        .execute(&state.db_rw)
        .await?;
    sqlx::query("DELETE FROM musics WHERE id = $1")
        .bind(music_id)
        .execute(&state.db_rw)
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

