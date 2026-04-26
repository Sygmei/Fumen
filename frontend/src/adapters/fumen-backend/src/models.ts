export type JsonPrimitive = string | number | boolean | null;
export type JsonValue = JsonPrimitive | JsonValue[] | { [key: string]: JsonValue };

export interface AccessTokenRefreshResponse {
  access_token: string;
  access_token_expires_at: string;
}


export interface AdminEnsembleResponse {
  created_at: string;
  created_by_user_id?: string | null;
  id: string;
  members: EnsembleMemberResponse[];
  name: string;
  score_count: number;
}


export interface AdminMusicPlaytimeResponse {
  leaderboard: MusicPlaytimeLeaderboardEntryResponse[];
  listener_count: number;
  total_seconds: number;
  track_totals: MusicPlaytimeTrackSummaryResponse[];
}


export interface AdminMusicProcessingLogResponse {
  content: string;
}

export interface AdminRetryMusicProcessingRequest {
  quality_profile?: string | null;
}

export interface AdminMusicProcessingProgressResponse {
  music_id: string;
  processing_job_attempt?: number | null;
  processing_job_error?: string | null;
  processing_job_heartbeat_at?: string | null;
  processing_job_lease_expires_at?: string | null;
  processing_job_status?: string | null;
  processing_job_step?: string | null;
  stalled: boolean;
  state_message?: string | null;
  steps: AdminMusicProcessingStepResponse[];
}

export interface AdminMusicProcessingStepResponse {
  detail?: string | null;
  group?: string | null;
  key: string;
  label: string;
  last_updated_at?: string | null;
  status: string;
  tooltip?: string | null;
}


export interface AdminMusicResponse {
  audio_error?: string | null;
  audio_status: string;
  content_type: string;
  created_at: string;
  download_url: string;
  ensemble_ids: string[];
  ensemble_names: string[];
  filename: string;
  icon?: string | null;
  icon_image_url?: string | null;
  id: string;
  midi_download_url?: string | null;
  midi_error?: string | null;
  midi_status: string;
  musicxml_error?: string | null;
  musicxml_status: string;
    owner_user_id?: string | null;
    processing_job_attempt?: number | null;
    processing_job_error?: string | null;
    processing_job_heartbeat_at?: string | null;
    processing_job_lease_expires_at?: string | null;
    processing_job_status?: string | null;
    processing_job_step?: string | null;
  public_id?: string | null;
  public_id_url?: string | null;
  public_token: string;
  public_url: string;
  quality_profile: string;
  subtitle?: string | null;
  stems_error?: string | null;
  stems_status: string;
  stems_total_bytes: number;
  title: string;
}


export interface AdminUpdateMusicMultipartRequest {
  icon?: string | null;
  icon_file: Blob;
  public_id?: string | null;
  subtitle?: string | null;
  title?: string | null;
}


export interface AdminUpdateUserMultipartRequest {
  avatar_file: Blob;
  clear_avatar?: boolean | null;
  display_name?: string | null;
  role?: string | null;
}


export interface AdminUploadMusicMultipartRequest {
  ensemble_id: string[];
  file: Blob;
  icon?: string | null;
  icon_file: Blob;
  public_id?: string | null;
  quality_profile?: string | null;
  subtitle?: string | null;
  title?: string | null;
}


export interface AdminUserMetadataResponse {
  last_login_at?: string | null;
  score_playtimes: AdminUserScorePlaytimeResponse[];
  total_playtime_seconds: number;
}


export interface AdminUserScorePlaytimeResponse {
  icon?: string | null;
  icon_image_url?: string | null;
  music_id: string;
  public_url: string;
  subtitle?: string | null;
  title: string;
  total_seconds: number;
}


export interface AuthTokenResponse {
  access_token: string;
  access_token_expires_at: string;
  refresh_token: string;
  user: UserResponse;
}


export interface CreateEnsembleRequest {
  name: string;
}


export interface CreateScoreAnnotationRequest {
  bar_number: number;
  beat_number: number;
  comment: string;
  instrument: string;
  system_y_ratio?: number | null;
}


export interface CreateUserRequest {
  role?: string | null;
  username: string;
}


export interface CurrentUserResponse {
  session_expires_at?: string | null;
  user: UserResponse;
}


export interface DrumMapEntry {
  head?: string | null;
  line?: number | null;
  name: string;
  pitch: number;
  shortcut?: string | null;
  stem?: number | null;
  voice?: number | null;
}


export interface EnsembleMemberResponse {
  role: string;
  user_id: string;
}


export interface ErrorResponse {
  error: string;
}


export interface ExchangeLoginTokenRequest {
  token: string;
}


export interface HealthResponse {
  ok: boolean;
}


export interface LoginLinkResponse {
  connection_url: string;
  expires_at: string;
}


export interface MoveMusicRequest {
  ensemble_id: string;
}


export interface MusicPlaytimeLeaderboardEntryResponse {
  avatar_url?: string | null;
  best_track_seconds: number;
  display_name?: string | null;
  track_totals: MusicPlaytimeTrackSummaryResponse[];
  user_id: string;
  username: string;
}


export interface MusicPlaytimeTrackSummaryResponse {
  instrument_name: string;
  total_seconds: number;
  track_index: number;
  track_name: string;
}


export interface PublicMusicResponse {
  audio_error?: string | null;
  audio_status: string;
  audio_stream_url?: string | null;
  can_stream_audio: boolean;
  created_at: string;
  download_url: string;
  filename: string;
  icon?: string | null;
  icon_image_url?: string | null;
  midi_download_url?: string | null;
  midi_error?: string | null;
  midi_status: string;
  musicxml_url?: string | null;
  subtitle?: string | null;
  stems_error?: string | null;
  stems_status: string;
  title: string;
}


export interface RefreshTokenRequest {
  refresh_token: string;
}


export interface ReportPlaytimeRequest {
  tracks: TrackPlaytimeIncrementRequest[];
}


export interface ScoreAnnotationListResponse {
  annotations: ScoreAnnotationResponse[];
  visibility_scope: string;
}


export interface ScoreAnnotationResponse {
  avatar_url?: string | null;
  bar_number: number;
  beat_number: number;
  comment: string;
  created_at: string;
  display_name?: string | null;
  id: string;
  instrument: string;
  music_id: string;
  system_y_ratio?: number | null;
  user_id: string;
  username: string;
}


export interface StemInfo {
  drum_map?: DrumMapEntry[];
  duration_seconds: number;
  full_stem_url: string;
  instrument_name: string;
  track_index: number;
  track_name: string;
}


export interface TrackPlaytimeIncrementRequest {
  seconds: number;
  track_index: number;
}


export interface UpdateEnsembleMemberItemRequest {
  role: string;
  user_id: string;
}


export interface UpdateEnsembleMemberRequest {
  role: string;
}


export interface UpdateEnsembleMembersRequest {
  members: UpdateEnsembleMemberItemRequest[];
}

export interface UpdateEnsembleScoresRequest {
  music_ids: string[];
}


export interface UpdateMusicEnsemblesRequest {
  ensemble_ids: string[];
}


export interface UpdateMyProfileMultipartRequest {
  avatar_file: Blob;
  clear_avatar?: boolean | null;
  display_name?: string | null;
}


export interface UserLibraryEnsembleResponse {
  id: string;
  name: string;
  scores: UserLibraryScoreResponse[];
}


export interface UserLibraryResponse {
  ensembles: UserLibraryEnsembleResponse[];
}


export interface UserLibraryScoreResponse {
  created_at: string;
  filename: string;
  icon?: string | null;
  icon_image_url?: string | null;
  id: string;
  public_id_url?: string | null;
  public_url: string;
  subtitle?: string | null;
  title: string;
}


export interface UserResponse {
  avatar_url?: string | null;
  created_at: string;
  created_by_user_id?: string | null;
  display_name?: string | null;
  editable_ensemble_ids: string[];
  id: string;
  managed_ensemble_ids: string[];
  role: string;
  username: string;
}
