use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::{Modify, OpenApi};

use crate::schemas::{
    AccessTokenRefreshResponse, AdminEnsembleResponse, AdminMusicPlaytimeResponse,
    AdminMusicProcessingLogResponse, AdminMusicProcessingProgressResponse,
    AdminMusicProcessingStepResponse, AdminMusicResponse, AdminUpdateMusicMultipartRequest,
    AdminUpdateUserMultipartRequest, AdminUploadMusicMultipartRequest, AdminUserMetadataResponse,
    AdminUserScorePlaytimeResponse, AuthTokenResponse, CreateEnsembleRequest,
    CreateScoreAnnotationRequest, CreateUserRequest, CurrentUserResponse, DrumMapEntry,
    EnsembleMemberResponse, ErrorResponse, ExchangeLoginTokenRequest, HealthResponse,
    LoginLinkResponse, MoveMusicRequest, MusicPlaytimeLeaderboardEntryResponse,
    MusicPlaytimeTrackSummaryResponse, PublicMusicResponse, RefreshTokenRequest,
    ReportPlaytimeRequest, ScoreAnnotationListResponse, ScoreAnnotationResponse, StemInfo,
    TrackPlaytimeIncrementRequest, UpdateEnsembleMemberItemRequest, UpdateEnsembleMemberRequest,
    UpdateEnsembleMembersRequest, UpdateEnsembleScoresRequest, UpdateMusicEnsemblesRequest,
    UpdateMyProfileMultipartRequest, UserLibraryEnsembleResponse, UserLibraryResponse,
    UserLibraryScoreResponse, UserResponse,
};

pub(crate) struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .build(),
                ),
            );
        }
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::routes::health,
        crate::routes::auth::exchange_login_token,
        crate::routes::auth::refresh_access_token,
        crate::routes::me::current_user,
        crate::routes::me::update_my_profile,
        crate::routes::me::user_avatar,
        crate::routes::me::current_user_library,
        crate::routes::me::create_my_login_link,
        crate::routes::me::me_logout,
        crate::routes::public::public_music,
        crate::routes::public::public_music_audio,
        crate::routes::public::public_music_midi,
        crate::routes::public::public_music_musicxml,
        crate::routes::public::public_music_download,
        crate::routes::public::public_music_icon,
        crate::routes::public::public_music_annotations,
        crate::routes::public::create_public_music_annotation,
        crate::routes::public::public_music_stems,
        crate::routes::public::public_music_stem_audio,
        crate::routes::public::report_public_music_playtime,
        crate::routes::admin::admin_list_users,
        crate::routes::admin::admin_create_user,
        crate::routes::admin::admin_create_user_login_link,
        crate::routes::admin::admin_user_metadata,
        crate::routes::admin::admin_delete_user,
        crate::routes::admin::admin_update_user,
        crate::routes::admin::admin_list_ensembles,
        crate::routes::admin::admin_create_ensemble,
        crate::routes::admin::admin_delete_ensemble,
        crate::routes::admin::admin_add_user_to_ensemble,
        crate::routes::admin::admin_remove_user_from_ensemble,
        crate::routes::admin::admin_update_ensemble_members,
        crate::routes::admin::admin_update_ensemble_scores,
        crate::routes::admin::admin_add_music_to_ensemble,
        crate::routes::admin::admin_remove_music_from_ensemble,
        crate::routes::admin::admin_update_music_ensembles,
        crate::routes::admin::admin_list_musics,
        crate::routes::admin::admin_upload_music,
        crate::routes::admin::admin_music_processing_log,
        crate::routes::admin::admin_music_processing_progress,
        crate::routes::admin::admin_retry_render,
        crate::routes::admin::admin_update_music,
        crate::routes::admin::admin_move_music,
        crate::routes::admin::admin_music_playtime,
        crate::routes::admin::admin_delete_music
    ),
    components(
        schemas(
            AccessTokenRefreshResponse,
            AdminEnsembleResponse,
            AdminMusicPlaytimeResponse,
            AdminMusicProcessingLogResponse,
            AdminMusicProcessingProgressResponse,
            AdminMusicProcessingStepResponse,
            AdminMusicResponse,
            AdminUpdateMusicMultipartRequest,
            AdminUpdateUserMultipartRequest,
            AdminUploadMusicMultipartRequest,
            AdminUserMetadataResponse,
            AdminUserScorePlaytimeResponse,
            AuthTokenResponse,
            CreateEnsembleRequest,
            CreateUserRequest,
            CreateScoreAnnotationRequest,
            CurrentUserResponse,
            DrumMapEntry,
            EnsembleMemberResponse,
            ErrorResponse,
            ExchangeLoginTokenRequest,
            HealthResponse,
            LoginLinkResponse,
            MoveMusicRequest,
            MusicPlaytimeLeaderboardEntryResponse,
            MusicPlaytimeTrackSummaryResponse,
            PublicMusicResponse,
            RefreshTokenRequest,
            ReportPlaytimeRequest,
            ScoreAnnotationListResponse,
            ScoreAnnotationResponse,
            StemInfo,
            TrackPlaytimeIncrementRequest,
            UpdateEnsembleMemberItemRequest,
            UpdateEnsembleMemberRequest,
            UpdateEnsembleMembersRequest,
            UpdateEnsembleScoresRequest,
            UpdateMusicEnsemblesRequest,
            UpdateMyProfileMultipartRequest,
            UserLibraryEnsembleResponse,
            UserLibraryResponse,
            UserLibraryScoreResponse,
            UserResponse
        )
    ),
    modifiers(&SecurityAddon),
    tags(
        (name = "system", description = "Service health and metadata"),
        (name = "auth", description = "Authentication and token exchange"),
        (name = "me", description = "Authenticated user endpoints"),
        (name = "public", description = "Public score access endpoints"),
        (name = "admin", description = "Administrative score and user management endpoints")
    ),
    info(
        title = "Fumen Backend API",
        description = "Generated OpenAPI document for the Axum backend."
    )
)]
pub(crate) struct ApiDoc;
