export type AdminMusic = {
  id: string
  title: string
  filename: string
  content_type: string
  audio_status: string
  audio_error: string | null
  midi_status: string
  midi_error: string | null
  stems_status: string
  stems_error: string | null
  public_token: string
  public_id: string | null
  public_url: string
  public_id_url: string | null
  download_url: string
  midi_download_url: string | null
  quality_profile: StemQualityProfile
  created_at: string
  stems_total_bytes: number
  ensemble_ids: string[]
  ensemble_names: string[]
}

export type StemQualityProfile = 'compact' | 'standard' | 'high'

export type AppUser = {
  id: string
  username: string
  created_at: string
  role: 'superadmin' | 'admin' | 'user'
  managed_ensemble_ids: string[]
}

export type EnsembleMember = {
  user_id: string
  role: 'admin' | 'user'
}

export type Ensemble = {
  id: string
  name: string
  created_at: string
  members: EnsembleMember[]
  score_count: number
}

export type LoginLinkResponse = {
  connection_url: string
  expires_at: string
}

export type AuthSessionResponse = {
  session_token: string
  session_expires_at: string
  user: AppUser
}

export type CurrentUserResponse = {
  session_expires_at: string
  user: AppUser
}

export type UserLibraryScore = {
  id: string
  title: string
  filename: string
  public_url: string
  public_id_url: string | null
  created_at: string
}

export type UserLibraryEnsemble = {
  id: string
  name: string
  scores: UserLibraryScore[]
}

export type UserLibraryResponse = {
  ensembles: UserLibraryEnsemble[]
}

export const STEM_QUALITY_PROFILES: Array<{
  value: StemQualityProfile
  label: string
  description: string
}> = [
  {
    value: 'compact',
    label: 'Compact',
    description: 'Smaller stem files with more aggressive Opus compression at 24k.',
  },
  {
    value: 'standard',
    label: 'Standard',
    description: 'Balanced stem quality and size at 32k.',
  },
  {
    value: 'high',
    label: 'High',
    description: 'Higher stem quality with larger files at 48k.',
  },
]

export type PublicMusic = {
  title: string
  filename: string
  audio_status: string
  audio_error: string | null
  can_stream_audio: boolean
  audio_stream_url: string | null
  midi_status: string
  midi_error: string | null
  midi_download_url: string | null
  stems_status: string
  stems_error: string | null
  musicxml_url: string | null
  download_url: string
  created_at: string
}

export type Stem = {
  track_index: number
  track_name: string
  instrument_name: string
  full_stem_url: string
  duration_seconds: number
  drum_map?: Array<{
    pitch: number
    name: string
    head?: string | null
    line?: number | null
    voice?: number | null
    stem?: number | null
    shortcut?: string | null
  }> | null
}

type JsonOptions = RequestInit & { authToken?: string }

type RuntimeConfig = {
  apiBaseUrl?: string
}

const runtimeConfig = (
  globalThis as typeof globalThis & {
    __FUMEN_CONFIG__?: RuntimeConfig
  }
).__FUMEN_CONFIG__

function normalizeApiBaseUrl(value: string): string {
  return value.replace(/\/+$/, '')
}

function resolveApiBaseUrl(): string {
  const configuredValue =
    runtimeConfig?.apiBaseUrl?.trim() || import.meta.env.VITE_API_BASE_URL?.trim()

  if (configuredValue) {
    return normalizeApiBaseUrl(configuredValue)
  }

  if (import.meta.env.DEV) {
    return 'http://127.0.0.1:3000/api'
  }

  throw new Error(
    'Missing API base URL. Set VITE_API_BASE_URL for local development or API_BASE_URL in the frontend runtime.',
  )
}

const API_BASE_URL = resolveApiBaseUrl()
const API_BASE_ORIGIN = new URL(
  API_BASE_URL,
  globalThis.location?.origin ?? 'http://localhost',
).origin

function apiUrl(path: string): string {
  return `${API_BASE_URL}${path}`
}

function resolveBackendAssetUrl(url: string | null | undefined): string | null {
  if (!url) {
    return null
  }

  return new URL(url, API_BASE_ORIGIN).toString()
}

function normalizeAdminMusic(music: AdminMusic): AdminMusic {
  return {
    ...music,
    download_url: resolveBackendAssetUrl(music.download_url) ?? music.download_url,
    midi_download_url: resolveBackendAssetUrl(music.midi_download_url),
  }
}

function normalizePublicMusic(music: PublicMusic): PublicMusic {
  return {
    ...music,
    audio_stream_url: resolveBackendAssetUrl(music.audio_stream_url),
    midi_download_url: resolveBackendAssetUrl(music.midi_download_url),
    musicxml_url: resolveBackendAssetUrl(music.musicxml_url),
    download_url: resolveBackendAssetUrl(music.download_url) ?? music.download_url,
  }
}

function normalizeStem(stem: Stem): Stem {
  return {
    ...stem,
    full_stem_url: resolveBackendAssetUrl(stem.full_stem_url) ?? stem.full_stem_url,
  }
}

async function requestJson<T>(path: string, options: JsonOptions = {}): Promise<T> {
  const headers = new Headers(options.headers)

  if (options.authToken) {
    headers.set('authorization', `Bearer ${options.authToken}`)
  }

  if (!(options.body instanceof FormData) && !headers.has('content-type')) {
    headers.set('content-type', 'application/json')
  }

  const response = await fetch(apiUrl(path), {
    ...options,
    headers,
  })

  if (!response.ok) {
    let message = `Request failed with status ${response.status}`

    try {
      const payload = (await response.json()) as { error?: string }
      if (payload.error) {
        message = payload.error
      }
    } catch {
      // Ignore JSON parsing errors and keep the fallback message.
    }

    throw new Error(message)
  }

  if (response.status === 204) {
    return undefined as T
  }

  return (await response.json()) as T
}

async function requestBlob(path: string, options: JsonOptions = {}): Promise<Blob> {
  const headers = new Headers(options.headers)

  if (options.authToken) {
    headers.set('authorization', `Bearer ${options.authToken}`)
  }

  if (!(options.body instanceof FormData) && options.body && !headers.has('content-type')) {
    headers.set('content-type', 'application/json')
  }

  const response = await fetch(apiUrl(path), {
    ...options,
    headers,
  })

  if (!response.ok) {
    let message = `Request failed with status ${response.status}`

    try {
      const payload = (await response.json()) as { error?: string }
      if (payload.error) {
        message = payload.error
      }
    } catch {
      // Ignore JSON parsing errors and keep the fallback message.
    }

    throw new Error(message)
  }

  return response.blob()
}

export async function listMusics(authToken: string): Promise<AdminMusic[]> {
  const musics = await requestJson<AdminMusic[]>('/admin/musics', {
    authToken,
  })

  return musics.map(normalizeAdminMusic)
}

export async function listUsers(authToken: string): Promise<AppUser[]> {
  return requestJson<AppUser[]>('/admin/users', {
    authToken,
  })
}

export async function createUser(authToken: string, username: string): Promise<AppUser> {
  return requestJson<AppUser>('/admin/users', {
    method: 'POST',
    authToken,
    body: JSON.stringify({ username }),
  })
}

export async function createAdminUserLoginLink(
  authToken: string,
  userId: string,
): Promise<LoginLinkResponse> {
  return requestJson<LoginLinkResponse>(`/admin/users/${userId}/login-link`, {
    method: 'POST',
    authToken,
  })
}

export async function listEnsembles(authToken: string): Promise<Ensemble[]> {
  return requestJson<Ensemble[]>('/admin/ensembles', {
    authToken,
  })
}

export async function createEnsemble(authToken: string, name: string): Promise<Ensemble> {
  return requestJson<Ensemble>('/admin/ensembles', {
    method: 'POST',
    authToken,
    body: JSON.stringify({ name }),
  })
}

export async function addUserToEnsemble(
  authToken: string,
  ensembleId: string,
  userId: string,
  role: 'admin' | 'user',
): Promise<void> {
  await requestJson(`/admin/ensembles/${ensembleId}/users/${userId}`, {
    method: 'POST',
    authToken,
    body: JSON.stringify({ role }),
  })
}

export async function removeUserFromEnsemble(
  authToken: string,
  ensembleId: string,
  userId: string,
): Promise<void> {
  await requestJson(`/admin/ensembles/${ensembleId}/users/${userId}`, {
    method: 'DELETE',
    authToken,
  })
}

export async function addMusicToEnsemble(
  authToken: string,
  musicId: string,
  ensembleId: string,
): Promise<void> {
  await requestJson(`/admin/musics/${musicId}/ensembles/${ensembleId}`, {
    method: 'POST',
    authToken,
  })
}

export async function removeMusicFromEnsemble(
  authToken: string,
  musicId: string,
  ensembleId: string,
): Promise<void> {
  await requestJson(`/admin/musics/${musicId}/ensembles/${ensembleId}`, {
    method: 'DELETE',
    authToken,
  })
}

export async function uploadMusic(
  authToken: string,
  payload: {
    file: File
    title: string
    publicId: string
    qualityProfile: StemQualityProfile
    ensembleId: string
  },
): Promise<AdminMusic> {
  const body = new FormData()
  body.append('file', payload.file)
  body.append('title', payload.title)
  body.append('public_id', payload.publicId)
  body.append('quality_profile', payload.qualityProfile)
  body.append('ensemble_id', payload.ensembleId)

  const music = await requestJson<AdminMusic>('/admin/musics', {
    method: 'POST',
    authToken,
    body,
  })

  return normalizeAdminMusic(music)
}

export async function retryRender(authToken: string, id: string): Promise<AdminMusic> {
  const music = await requestJson<AdminMusic>(`/admin/musics/${id}/retry`, {
    method: 'POST',
    authToken,
  })

  return normalizeAdminMusic(music)
}

export async function downloadScoreGains(authToken: string, id: string): Promise<Blob> {
  return requestBlob(`/admin/musics/${id}/gains`, {
    authToken,
  })
}

export async function downloadPublicScoreGains(authToken: string, accessKey: string): Promise<Blob> {
  return requestBlob(`/admin/public/${encodeURIComponent(accessKey)}/gains`, {
    authToken,
  })
}

export async function exportPublicMixerGains(
  authToken: string,
  accessKey: string,
  tracks: Array<{ track_index: number; volume_multiplier: number; muted: boolean }>,
): Promise<Blob> {
  return requestBlob(`/admin/public/${encodeURIComponent(accessKey)}/gains`, {
    method: 'POST',
    authToken,
    body: JSON.stringify({ tracks }),
  })
}

export async function updatePublicId(
  authToken: string,
  id: string,
  publicId: string,
): Promise<AdminMusic> {
  const music = await requestJson<AdminMusic>(`/admin/musics/${id}`, {
    method: 'PATCH',
    authToken,
    body: JSON.stringify({
      public_id: publicId.trim() ? publicId.trim() : null,
    }),
  })

  return normalizeAdminMusic(music)
}

export async function fetchPublicMusic(accessKey: string): Promise<PublicMusic> {
  const music = await requestJson<PublicMusic>(`/public/${encodeURIComponent(accessKey)}`)
  return normalizePublicMusic(music)
}

export async function fetchStems(accessKey: string): Promise<Stem[]> {
  const stems = await requestJson<Stem[]>(`/public/${encodeURIComponent(accessKey)}/stems`)
  return stems.map(normalizeStem)
}

export async function exchangeLoginToken(token: string): Promise<AuthSessionResponse> {
  return requestJson<AuthSessionResponse>('/auth/exchange', {
    method: 'POST',
    body: JSON.stringify({ token }),
  })
}

export async function moveMusic(
  authToken: string,
  id: string,
  ensembleId: string,
): Promise<AdminMusic> {
  const music = await requestJson<AdminMusic>(`/admin/musics/${id}/move`, {
    method: 'POST',
    authToken,
    body: JSON.stringify({ ensemble_id: ensembleId }),
  })

  return normalizeAdminMusic(music)
}

export async function deleteMusic(authToken: string, id: string): Promise<void> {
  await requestJson(`/admin/musics/${id}/delete`, {
    method: 'POST',
    authToken,
  })
}

export async function fetchCurrentUser(authToken: string): Promise<CurrentUserResponse> {
  return requestJson<CurrentUserResponse>('/me', {
    authToken,
  })
}

export async function fetchUserLibrary(authToken: string): Promise<UserLibraryResponse> {
  return requestJson<UserLibraryResponse>('/me/library', {
    authToken,
  })
}

export async function createMyLoginLink(authToken: string): Promise<LoginLinkResponse> {
  return requestJson<LoginLinkResponse>('/me/login-link', {
    method: 'POST',
    authToken,
  })
}
