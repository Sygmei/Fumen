use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Clone, Debug, FromRow)]
pub struct MusicRecord {
    pub id: String,
    pub title: String,
    pub icon: Option<String>,
    pub icon_image_key: Option<String>,
    pub filename: String,
    pub content_type: String,
    pub object_key: String,
    pub audio_object_key: Option<String>,
    pub audio_status: String,
    pub audio_error: Option<String>,
    pub midi_object_key: Option<String>,
    pub midi_status: String,
    pub midi_error: Option<String>,
    pub musicxml_object_key: Option<String>,
    pub musicxml_status: String,
    pub musicxml_error: Option<String>,
    pub stems_status: String,
    pub stems_error: Option<String>,
    pub public_token: String,
    pub public_id: Option<String>,
    pub quality_profile: String,
    pub created_at: String,
}

#[derive(Clone, Debug, FromRow)]
#[allow(dead_code)]
pub struct StemRecord {
    pub id: i64,
    pub music_id: String,
    pub track_index: i64,
    pub track_name: String,
    pub instrument_name: String,
    pub storage_key: String,
    pub drum_map_json: Option<String>,
}

#[derive(Clone, Debug, FromRow)]
pub struct UserRecord {
    pub id: String,
    pub username: String,
    pub created_at: String,
    pub is_superadmin: bool,
}

#[derive(Clone, Debug, FromRow)]
#[allow(dead_code)]
pub struct UserSessionRecord {
    pub id: String,
    pub user_id: String,
    pub session_token: String,
    pub created_at: String,
    pub expires_at: Option<String>,
}

#[derive(Clone, Debug, FromRow)]
pub struct EnsembleRecord {
    pub id: String,
    pub name: String,
    pub created_at: String,
}

#[derive(Clone, Debug, FromRow)]
pub struct UserEnsembleMembershipRecord {
    pub user_id: String,
    pub ensemble_id: String,
    pub role: String,
}

#[derive(Clone, Debug, FromRow)]
pub struct MusicEnsembleLinkRecord {
    pub music_id: String,
    pub ensemble_id: String,
}

#[derive(Clone, Debug, FromRow)]
pub struct EnsembleSummaryRecord {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DrumMapEntry {
    pub pitch: u8,
    pub name: String,
    pub head: Option<String>,
    pub line: Option<i8>,
    pub voice: Option<u8>,
    pub stem: Option<i8>,
    pub shortcut: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateEnsembleRequest {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateEnsembleMemberRequest {
    pub role: String,
}

#[derive(Debug, Deserialize)]
pub struct ExchangeLoginTokenRequest {
    pub token: String,
}

#[derive(Debug, Deserialize)]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateMusicRequest {
    pub public_id: Option<String>,
    pub icon: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MoveMusicRequest {
    pub ensemble_id: String,
}

#[derive(Debug, Deserialize)]
pub struct ExportMixerGainsRequest {
    pub tracks: Vec<ExportMixerTrackRequest>,
}

#[derive(Debug, Deserialize)]
pub struct ExportMixerTrackRequest {
    pub track_index: usize,
    pub volume_multiplier: f64,
    #[serde(default)]
    pub muted: bool,
}

#[derive(Debug, Serialize)]
pub struct StemInfo {
    pub track_index: i64,
    pub track_name: String,
    pub instrument_name: String,
    pub full_stem_url: String,
    pub duration_seconds: f64,
    pub drum_map: Option<Vec<DrumMapEntry>>,
}

#[derive(Debug, Serialize)]
pub struct AdminMusicResponse {
    pub id: String,
    pub title: String,
    pub icon: Option<String>,
    pub icon_image_url: Option<String>,
    pub filename: String,
    pub content_type: String,
    pub audio_status: String,
    pub audio_error: Option<String>,
    pub midi_status: String,
    pub midi_error: Option<String>,
    pub musicxml_status: String,
    pub musicxml_error: Option<String>,
    pub stems_status: String,
    pub stems_error: Option<String>,
    pub public_token: String,
    pub public_id: Option<String>,
    pub public_url: String,
    pub public_id_url: Option<String>,
    pub download_url: String,
    pub midi_download_url: Option<String>,
    pub quality_profile: String,
    pub created_at: String,
    pub stems_total_bytes: i64,
    pub ensemble_ids: Vec<String>,
    pub ensemble_names: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct PublicMusicResponse {
    pub title: String,
    pub icon: Option<String>,
    pub icon_image_url: Option<String>,
    pub filename: String,
    pub audio_status: String,
    pub audio_error: Option<String>,
    pub can_stream_audio: bool,
    pub audio_stream_url: Option<String>,
    pub midi_status: String,
    pub midi_error: Option<String>,
    pub midi_download_url: Option<String>,
    pub musicxml_url: Option<String>,
    pub stems_status: String,
    pub stems_error: Option<String>,
    pub download_url: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: String,
    pub username: String,
    pub created_at: String,
    pub role: String,
    pub managed_ensemble_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct EnsembleMemberResponse {
    pub user_id: String,
    pub role: String,
}

#[derive(Debug, Serialize)]
pub struct AdminEnsembleResponse {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub members: Vec<EnsembleMemberResponse>,
    pub score_count: i64,
}

#[derive(Debug, Serialize)]
pub struct LoginLinkResponse {
    pub connection_url: String,
    pub expires_at: String,
}

#[derive(Debug, Serialize)]
pub struct AuthTokenResponse {
    pub refresh_token: String,
    pub access_token: String,
    pub access_token_expires_at: String,
    pub user: UserResponse,
}

#[derive(Debug, Serialize)]
pub struct AccessTokenRefreshResponse {
    pub access_token: String,
    pub access_token_expires_at: String,
}

#[derive(Debug, Serialize)]
pub struct CurrentUserResponse {
    pub session_expires_at: Option<String>,
    pub user: UserResponse,
}

#[derive(Debug, Serialize)]
pub struct UserLibraryScoreResponse {
    pub id: String,
    pub title: String,
    pub icon: Option<String>,
    pub icon_image_url: Option<String>,
    pub filename: String,
    pub public_url: String,
    pub public_id_url: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct UserLibraryEnsembleResponse {
    pub id: String,
    pub name: String,
    pub scores: Vec<UserLibraryScoreResponse>,
}

#[derive(Debug, Serialize)]
pub struct UserLibraryResponse {
    pub ensembles: Vec<UserLibraryEnsembleResponse>,
}
