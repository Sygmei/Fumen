use crate::schema::{
    ensembles, music_ensemble_links, musics, processing_jobs, score_annotations, stems,
    user_ensemble_memberships, user_login_links, user_music_track_playtime, user_sessions, users,
};
use diesel::QueryableByName;
use diesel::sql_types::{BigInt, Bool, Double, Nullable, Text};
use diesel::{AsChangeset, Associations, Identifiable, Insertable, Queryable, Selectable};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Queryable, QueryableByName, Selectable, Identifiable)]
#[diesel(table_name = musics)]
pub struct MusicRecord {
    #[diesel(sql_type = Text)]
    pub id: String,
    #[diesel(sql_type = Text)]
    pub title: String,
    #[diesel(sql_type = Nullable<Text>)]
    pub subtitle: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    pub icon: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    pub icon_image_key: Option<String>,
    #[diesel(sql_type = Text)]
    pub filename: String,
    #[diesel(sql_type = Text)]
    pub content_type: String,
    #[diesel(sql_type = Text)]
    pub object_key: String,
    #[diesel(sql_type = Nullable<Text>)]
    pub audio_object_key: Option<String>,
    #[diesel(sql_type = Text)]
    pub audio_status: String,
    #[diesel(sql_type = Nullable<Text>)]
    pub audio_error: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    pub midi_object_key: Option<String>,
    #[diesel(sql_type = Text)]
    pub midi_status: String,
    #[diesel(sql_type = Nullable<Text>)]
    pub midi_error: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    pub musicxml_object_key: Option<String>,
    #[diesel(sql_type = Text)]
    pub musicxml_status: String,
    #[diesel(sql_type = Nullable<Text>)]
    pub musicxml_error: Option<String>,
    #[diesel(sql_type = Text)]
    pub stems_status: String,
    #[diesel(sql_type = Nullable<Text>)]
    pub stems_error: Option<String>,
    #[diesel(sql_type = Text)]
    pub public_token: String,
    #[diesel(sql_type = Nullable<Text>)]
    pub public_id: Option<String>,
    #[diesel(sql_type = Text)]
    pub quality_profile: String,
    #[diesel(sql_type = Text)]
    pub created_at: String,
    #[diesel(sql_type = Text)]
    pub directory_id: String,
    #[diesel(sql_type = Nullable<Text>)]
    pub owner_user_id: Option<String>,
}

#[derive(Clone, Debug, Queryable, QueryableByName, Selectable, Identifiable, Associations)]
#[diesel(table_name = processing_jobs)]
#[diesel(primary_key(music_id))]
#[diesel(belongs_to(MusicRecord, foreign_key = music_id))]
pub struct ProcessingJobRecord {
    #[diesel(sql_type = Text)]
    pub music_id: String,
    #[diesel(sql_type = Text)]
    pub source_object_key: String,
    #[diesel(sql_type = Text)]
    pub source_filename: String,
    #[diesel(sql_type = Text)]
    pub quality_profile: String,
    #[diesel(sql_type = Text)]
    pub status: String,
    #[diesel(sql_type = Text)]
    pub current_step: String,
    #[diesel(sql_type = BigInt)]
    pub attempt: i64,
    #[diesel(sql_type = BigInt)]
    pub max_attempts: i64,
    #[diesel(sql_type = Nullable<Text>)]
    pub worker_id: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    pub lease_expires_at: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    pub heartbeat_at: Option<String>,
    #[diesel(sql_type = Text)]
    pub queued_at: String,
    #[diesel(sql_type = Nullable<Text>)]
    pub started_at: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    pub finished_at: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    pub progress_json: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    pub error_message: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ProcessingJobProgress {
    pub steps: std::collections::HashMap<String, ProcessingJobProgressStep>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ProcessingJobProgressStep {
    pub status: Option<String>,
    pub detail: Option<String>,
    pub last_updated_at: Option<String>,
    pub tooltip: Option<String>,
}

#[derive(Clone, Debug, Queryable, QueryableByName, Selectable, Identifiable, Associations)]
#[diesel(table_name = stems)]
#[diesel(belongs_to(MusicRecord, foreign_key = music_id))]
pub struct StemRecord {
    #[diesel(sql_type = BigInt)]
    pub id: i64,
    #[diesel(sql_type = Text)]
    pub music_id: String,
    #[diesel(sql_type = BigInt)]
    pub track_index: i64,
    #[diesel(sql_type = Text)]
    pub track_name: String,
    #[diesel(sql_type = Text)]
    pub instrument_name: String,
    #[diesel(sql_type = Text)]
    pub storage_key: String,
    #[diesel(sql_type = BigInt)]
    pub size_bytes: i64,
    #[diesel(sql_type = Nullable<Text>)]
    pub drum_map_json: Option<String>,
}

#[derive(Clone, Debug, Queryable, QueryableByName, Selectable, Identifiable)]
#[diesel(table_name = users)]
pub struct UserRecord {
    #[diesel(sql_type = Text)]
    pub id: String,
    #[diesel(sql_type = Text)]
    pub username: String,
    #[diesel(sql_type = Text)]
    pub created_at: String,
    #[diesel(sql_type = Bool)]
    pub is_superadmin: bool,
    #[diesel(sql_type = Text)]
    pub role: String,
    #[diesel(sql_type = Nullable<Text>)]
    pub display_name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    pub avatar_image_key: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    pub created_by_user_id: Option<String>,
}

#[derive(Clone, Debug, Queryable, QueryableByName, Selectable, Identifiable, Associations)]
#[diesel(table_name = user_sessions)]
#[diesel(belongs_to(UserRecord, foreign_key = user_id))]
pub struct UserSessionRecord {
    #[diesel(sql_type = Text)]
    pub id: String,
    #[diesel(sql_type = Text)]
    pub user_id: String,
    #[diesel(sql_type = Text)]
    pub session_token: String,
    #[diesel(sql_type = Text)]
    pub created_at: String,
    #[diesel(sql_type = Nullable<Text>)]
    pub expires_at: Option<String>,
}

#[derive(Clone, Debug, Queryable, QueryableByName, Selectable, Identifiable)]
#[diesel(table_name = ensembles)]
pub struct EnsembleRecord {
    #[diesel(sql_type = Text)]
    pub id: String,
    #[diesel(sql_type = Text)]
    pub name: String,
    #[diesel(sql_type = Text)]
    pub created_at: String,
    #[diesel(sql_type = Nullable<Text>)]
    pub created_by_user_id: Option<String>,
}

#[derive(Clone, Debug, Queryable, QueryableByName, Selectable, Associations)]
#[diesel(table_name = user_ensemble_memberships)]
#[diesel(primary_key(user_id, ensemble_id))]
#[diesel(belongs_to(UserRecord, foreign_key = user_id))]
#[diesel(belongs_to(EnsembleRecord, foreign_key = ensemble_id))]
pub struct UserEnsembleMembershipRecord {
    #[diesel(sql_type = Text)]
    pub user_id: String,
    #[diesel(sql_type = Text)]
    pub ensemble_id: String,
    #[diesel(sql_type = Text)]
    pub role: String,
}

#[derive(Clone, Debug, Queryable, QueryableByName, Selectable, Associations)]
#[diesel(table_name = music_ensemble_links)]
#[diesel(primary_key(music_id, ensemble_id))]
#[diesel(belongs_to(MusicRecord, foreign_key = music_id))]
#[diesel(belongs_to(EnsembleRecord, foreign_key = ensemble_id))]
pub struct MusicEnsembleLinkRecord {
    #[diesel(sql_type = Text)]
    pub music_id: String,
    #[diesel(sql_type = Text)]
    pub ensemble_id: String,
}

#[derive(Clone, Debug, Queryable, QueryableByName, Selectable, Identifiable, Associations)]
#[diesel(table_name = score_annotations)]
#[diesel(belongs_to(MusicRecord, foreign_key = music_id))]
#[diesel(belongs_to(UserRecord, foreign_key = user_id))]
pub struct ScoreAnnotationRecord {
    #[diesel(sql_type = Text)]
    pub id: String,
    #[diesel(sql_type = Text)]
    pub music_id: String,
    #[diesel(sql_type = Text)]
    pub user_id: String,
    #[diesel(sql_type = BigInt)]
    pub bar_number: i64,
    #[diesel(sql_type = BigInt)]
    pub beat_number: i64,
    #[diesel(sql_type = Text)]
    pub instrument: String,
    #[diesel(sql_type = Nullable<Double>)]
    pub system_y_ratio: Option<f64>,
    #[diesel(sql_type = Text)]
    pub comment: String,
    #[diesel(sql_type = Text)]
    pub created_at: String,
}

#[derive(Clone, Debug, Queryable, Selectable, Identifiable, Associations)]
#[diesel(table_name = user_login_links)]
#[diesel(belongs_to(UserRecord, foreign_key = user_id))]
pub struct UserLoginLinkRecord {
    pub id: String,
    pub user_id: String,
    pub token: String,
    pub created_at: String,
    pub expires_at: String,
    pub consumed_at: Option<String>,
}

#[derive(Clone, Debug, QueryableByName)]
pub struct BigIntValueRow {
    #[diesel(sql_type = BigInt)]
    pub value: i64,
}

#[derive(Insertable)]
#[diesel(table_name = users)]
pub struct NewUser<'a> {
    pub id: &'a str,
    pub username: &'a str,
    pub created_at: &'a str,
    pub is_superadmin: bool,
    pub role: &'a str,
    pub display_name: Option<&'a str>,
    pub avatar_image_key: Option<&'a str>,
    pub created_by_user_id: Option<&'a str>,
}

#[derive(Insertable)]
#[diesel(table_name = ensembles)]
pub struct NewEnsemble<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub created_at: &'a str,
    pub created_by_user_id: Option<&'a str>,
}

#[derive(Insertable)]
#[diesel(table_name = user_sessions)]
pub struct NewUserSession<'a> {
    pub id: &'a str,
    pub user_id: &'a str,
    pub session_token: &'a str,
    pub created_at: &'a str,
    pub expires_at: Option<&'a str>,
}

#[derive(Insertable)]
#[diesel(table_name = user_login_links)]
pub struct NewUserLoginLink<'a> {
    pub id: &'a str,
    pub user_id: &'a str,
    pub token: &'a str,
    pub created_at: &'a str,
    pub expires_at: &'a str,
    pub consumed_at: Option<&'a str>,
}

#[derive(Insertable)]
#[diesel(table_name = user_ensemble_memberships)]
pub struct NewUserEnsembleMembership<'a> {
    pub user_id: &'a str,
    pub ensemble_id: &'a str,
    pub role: &'a str,
}

#[derive(Insertable)]
#[diesel(table_name = music_ensemble_links)]
pub struct NewMusicEnsembleLink<'a> {
    pub music_id: &'a str,
    pub ensemble_id: &'a str,
}

#[derive(Insertable)]
#[diesel(table_name = stems)]
pub struct NewStem<'a> {
    pub music_id: &'a str,
    pub track_index: i64,
    pub track_name: &'a str,
    pub instrument_name: &'a str,
    pub storage_key: &'a str,
    pub size_bytes: i64,
    pub drum_map_json: Option<&'a str>,
}

#[derive(Insertable)]
#[diesel(table_name = musics)]
pub struct NewMusic<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub subtitle: Option<&'a str>,
    pub icon: Option<&'a str>,
    pub icon_image_key: Option<&'a str>,
    pub filename: &'a str,
    pub content_type: &'a str,
    pub object_key: &'a str,
    pub audio_object_key: Option<&'a str>,
    pub audio_status: &'a str,
    pub audio_error: Option<&'a str>,
    pub midi_object_key: Option<&'a str>,
    pub midi_status: &'a str,
    pub midi_error: Option<&'a str>,
    pub musicxml_object_key: Option<&'a str>,
    pub musicxml_status: &'a str,
    pub musicxml_error: Option<&'a str>,
    pub stems_status: &'a str,
    pub stems_error: Option<&'a str>,
    pub public_token: &'a str,
    pub public_id: Option<&'a str>,
    pub quality_profile: &'a str,
    pub created_at: &'a str,
    pub directory_id: &'a str,
    pub owner_user_id: Option<&'a str>,
}

#[derive(Insertable)]
#[diesel(table_name = processing_jobs)]
pub struct NewProcessingJob<'a> {
    pub music_id: &'a str,
    pub source_object_key: &'a str,
    pub source_filename: &'a str,
    pub quality_profile: &'a str,
    pub status: &'a str,
    pub current_step: &'a str,
    pub attempt: i64,
    pub max_attempts: i64,
    pub worker_id: Option<&'a str>,
    pub lease_expires_at: Option<&'a str>,
    pub heartbeat_at: Option<&'a str>,
    pub queued_at: &'a str,
    pub started_at: Option<&'a str>,
    pub finished_at: Option<&'a str>,
    pub progress_json: Option<&'a str>,
    pub error_message: Option<&'a str>,
}

#[derive(Insertable)]
#[diesel(table_name = score_annotations)]
pub struct NewScoreAnnotation<'a> {
    pub id: &'a str,
    pub music_id: &'a str,
    pub user_id: &'a str,
    pub bar_number: i64,
    pub beat_number: i64,
    pub instrument: &'a str,
    pub system_y_ratio: Option<f64>,
    pub comment: &'a str,
    pub created_at: &'a str,
}

#[derive(Insertable)]
#[diesel(table_name = user_music_track_playtime)]
pub struct NewUserMusicTrackPlaytime<'a> {
    pub user_id: &'a str,
    pub music_id: &'a str,
    pub track_index: i64,
    pub total_seconds: f64,
    pub updated_at: &'a str,
}

#[derive(AsChangeset)]
#[diesel(table_name = users)]
pub struct UpdateUserProfile<'a> {
    pub display_name: Option<&'a str>,
    pub avatar_image_key: Option<&'a str>,
}

#[derive(AsChangeset)]
#[diesel(table_name = users)]
pub struct UpdateAdminUser<'a> {
    pub display_name: Option<&'a str>,
    pub role: &'a str,
    pub is_superadmin: bool,
    pub avatar_image_key: Option<&'a str>,
}

#[derive(AsChangeset)]
#[diesel(table_name = musics)]
pub struct UpdateMusicProcessing<'a> {
    pub audio_object_key: Option<&'a str>,
    pub audio_status: &'a str,
    pub audio_error: Option<&'a str>,
    pub midi_object_key: Option<&'a str>,
    pub midi_status: &'a str,
    pub midi_error: Option<&'a str>,
    pub musicxml_object_key: Option<&'a str>,
    pub musicxml_status: &'a str,
    pub musicxml_error: Option<&'a str>,
    pub stems_status: &'a str,
    pub stems_error: Option<&'a str>,
}

#[derive(AsChangeset)]
#[diesel(table_name = musics)]
pub struct MarkMusicProcessingFailed<'a> {
    pub audio_status: &'a str,
    pub audio_error: Option<&'a str>,
    pub midi_status: &'a str,
    pub midi_error: Option<&'a str>,
    pub musicxml_status: &'a str,
    pub musicxml_error: Option<&'a str>,
    pub stems_status: &'a str,
    pub stems_error: Option<&'a str>,
}

#[derive(AsChangeset)]
#[diesel(table_name = musics)]
pub struct UpdateMusicMetadata<'a> {
    pub title: &'a str,
    pub subtitle: Option<&'a str>,
    pub public_id: Option<&'a str>,
    pub icon: Option<&'a str>,
    pub icon_image_key: Option<&'a str>,
}

#[derive(AsChangeset)]
#[diesel(table_name = musics)]
pub struct UpdateMusicDirectory<'a> {
    pub directory_id: &'a str,
}

#[derive(AsChangeset)]
#[diesel(table_name = processing_jobs)]
pub struct UpdateProcessingJobState<'a> {
    pub status: &'a str,
    pub current_step: &'a str,
    pub worker_id: Option<&'a str>,
    pub lease_expires_at: Option<&'a str>,
    pub heartbeat_at: Option<&'a str>,
    pub started_at: Option<&'a str>,
    pub finished_at: Option<&'a str>,
    pub progress_json: Option<&'a str>,
    pub error_message: Option<&'a str>,
}
