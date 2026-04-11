use serde::{Deserialize, Serialize};

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
    pub role: Option<String>,
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
pub struct MoveMusicRequest {
    pub ensemble_id: String,
}

#[derive(Debug, Deserialize)]
pub struct TrackPlaytimeIncrementRequest {
    pub track_index: i64,
    pub seconds: f64,
}

#[derive(Debug, Deserialize)]
pub struct ReportPlaytimeRequest {
    pub tracks: Vec<TrackPlaytimeIncrementRequest>,
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
    pub owner_user_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MusicPlaytimeTrackSummaryResponse {
    pub track_index: i64,
    pub track_name: String,
    pub instrument_name: String,
    pub total_seconds: f64,
}

#[derive(Debug, Serialize)]
pub struct MusicPlaytimeLeaderboardEntryResponse {
    pub user_id: String,
    pub username: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub best_track_seconds: f64,
    pub track_totals: Vec<MusicPlaytimeTrackSummaryResponse>,
}

#[derive(Debug, Serialize)]
pub struct AdminMusicPlaytimeResponse {
    pub total_seconds: f64,
    pub listener_count: i64,
    pub track_totals: Vec<MusicPlaytimeTrackSummaryResponse>,
    pub leaderboard: Vec<MusicPlaytimeLeaderboardEntryResponse>,
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
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub created_at: String,
    pub role: String,
    pub managed_ensemble_ids: Vec<String>,
    pub editable_ensemble_ids: Vec<String>,
    pub created_by_user_id: Option<String>,
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
    pub created_by_user_id: Option<String>,
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
