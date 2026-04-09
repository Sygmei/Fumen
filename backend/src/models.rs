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
    pub owner_user_id: Option<String>,
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
    pub role: String,
    pub created_by_user_id: Option<String>,
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
    pub created_by_user_id: Option<String>,
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
