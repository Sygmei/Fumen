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
  directory_id: string
  directory_name: string
}

export type StemQualityProfile = 'compact' | 'standard' | 'high'

export type AppUser = {
  id: string
  username: string
  created_at: string
}

export type Ensemble = {
  id: string
  name: string
  created_at: string
  member_user_ids: string[]
}

export type Directory = {
  id: string
  name: string
  created_at: string
  permitted_ensemble_ids: string[]
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
  directory_id: string
  directory_name: string
}

export type UserLibraryDirectory = {
  id: string
  name: string
  scores: UserLibraryScore[]
}

export type UserLibraryResponse = {
  directories: UserLibraryDirectory[]
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

type JsonOptions = RequestInit & {
  password?: string
  authToken?: string
}

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

function apiUrl(path: string): string {
  return `${API_BASE_URL}${path}`
}

async function requestJson<T>(path: string, options: JsonOptions = {}): Promise<T> {
  const headers = new Headers(options.headers)

  if (options.password) {
    headers.set('x-admin-password', options.password)
  }

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

  if (options.password) {
    headers.set('x-admin-password', options.password)
  }

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

export async function login(password: string): Promise<void> {
  await requestJson('/admin/login', {
    method: 'POST',
    body: JSON.stringify({ password }),
  })
}

export async function listMusics(password: string): Promise<AdminMusic[]> {
  return requestJson<AdminMusic[]>('/admin/musics', {
    password,
  })
}

export async function listUsers(password: string): Promise<AppUser[]> {
  return requestJson<AppUser[]>('/admin/users', {
    password,
  })
}

export async function createUser(password: string, username: string): Promise<AppUser> {
  return requestJson<AppUser>('/admin/users', {
    method: 'POST',
    password,
    body: JSON.stringify({ username }),
  })
}

export async function createAdminUserLoginLink(
  password: string,
  userId: string,
): Promise<LoginLinkResponse> {
  return requestJson<LoginLinkResponse>(`/admin/users/${userId}/login-link`, {
    method: 'POST',
    password,
  })
}

export async function listEnsembles(password: string): Promise<Ensemble[]> {
  return requestJson<Ensemble[]>('/admin/ensembles', {
    password,
  })
}

export async function createEnsemble(password: string, name: string): Promise<Ensemble> {
  return requestJson<Ensemble>('/admin/ensembles', {
    method: 'POST',
    password,
    body: JSON.stringify({ name }),
  })
}

export async function addUserToEnsemble(
  password: string,
  ensembleId: string,
  userId: string,
): Promise<void> {
  await requestJson(`/admin/ensembles/${ensembleId}/users/${userId}`, {
    method: 'POST',
    password,
  })
}

export async function removeUserFromEnsemble(
  password: string,
  ensembleId: string,
  userId: string,
): Promise<void> {
  await requestJson(`/admin/ensembles/${ensembleId}/users/${userId}`, {
    method: 'DELETE',
    password,
  })
}

export async function listDirectories(password: string): Promise<Directory[]> {
  return requestJson<Directory[]>('/admin/directories', {
    password,
  })
}

export async function createDirectory(password: string, name: string): Promise<Directory> {
  return requestJson<Directory>('/admin/directories', {
    method: 'POST',
    password,
    body: JSON.stringify({ name }),
  })
}

export async function grantDirectoryToEnsemble(
  password: string,
  directoryId: string,
  ensembleId: string,
): Promise<void> {
  await requestJson(`/admin/directories/${directoryId}/ensembles/${ensembleId}`, {
    method: 'POST',
    password,
  })
}

export async function revokeDirectoryFromEnsemble(
  password: string,
  directoryId: string,
  ensembleId: string,
): Promise<void> {
  await requestJson(`/admin/directories/${directoryId}/ensembles/${ensembleId}`, {
    method: 'DELETE',
    password,
  })
}

export async function uploadMusic(
  password: string,
  payload: {
    file: File
    title: string
    publicId: string
    qualityProfile: StemQualityProfile
    directoryId: string
  },
): Promise<AdminMusic> {
  const body = new FormData()
  body.append('file', payload.file)
  body.append('title', payload.title)
  body.append('public_id', payload.publicId)
  body.append('quality_profile', payload.qualityProfile)
  body.append('directory_id', payload.directoryId)

  return requestJson<AdminMusic>('/admin/musics', {
    method: 'POST',
    password,
    body,
  })
}

export async function retryRender(password: string, id: string): Promise<AdminMusic> {
  return requestJson<AdminMusic>(`/admin/musics/${id}/retry`, {
    method: 'POST',
    password,
  })
}

export async function downloadScoreGains(password: string, id: string): Promise<Blob> {
  return requestBlob(`/admin/musics/${id}/gains`, {
    password,
  })
}

export async function downloadPublicScoreGains(password: string, accessKey: string): Promise<Blob> {
  return requestBlob(`/admin/public/${encodeURIComponent(accessKey)}/gains`, {
    password,
  })
}

export async function exportPublicMixerGains(
  password: string,
  accessKey: string,
  tracks: Array<{ track_index: number; volume_multiplier: number; muted: boolean }>,
): Promise<Blob> {
  return requestBlob(`/admin/public/${encodeURIComponent(accessKey)}/gains`, {
    method: 'POST',
    password,
    body: JSON.stringify({ tracks }),
  })
}

export async function updatePublicId(
  password: string,
  id: string,
  publicId: string,
): Promise<AdminMusic> {
  return requestJson<AdminMusic>(`/admin/musics/${id}`, {
    method: 'PATCH',
    password,
    body: JSON.stringify({
      public_id: publicId.trim() ? publicId.trim() : null,
    }),
  })
}

export async function fetchPublicMusic(accessKey: string): Promise<PublicMusic> {
  return requestJson<PublicMusic>(`/public/${encodeURIComponent(accessKey)}`)
}

export async function fetchStems(accessKey: string): Promise<Stem[]> {
  return requestJson<Stem[]>(`/public/${encodeURIComponent(accessKey)}/stems`)
}

export async function exchangeLoginToken(token: string): Promise<AuthSessionResponse> {
  return requestJson<AuthSessionResponse>('/auth/exchange', {
    method: 'POST',
    body: JSON.stringify({ token }),
  })
}

export async function moveMusic(
  password: string,
  id: string,
  directoryId: string,
): Promise<AdminMusic> {
  return requestJson<AdminMusic>(`/admin/musics/${id}/move`, {
    method: 'POST',
    password,
    body: JSON.stringify({ directory_id: directoryId }),
  })
}

export async function deleteMusic(password: string, id: string): Promise<void> {
  await requestJson(`/admin/musics/${id}/delete`, {
    method: 'POST',
    password,
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
