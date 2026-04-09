export type AdminMusic = {
  id: string
  title: string
  icon: string | null
  icon_image_url: string | null
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
  owner_user_id: string | null
}

export type StemQualityProfile = 'standard' | 'small' | 'very-small' | 'tiny'

export type GlobalRole = 'superadmin' | 'admin' | 'manager' | 'editor' | 'user'

export type EnsembleRole = 'manager' | 'editor' | 'user'

export type AppUser = {
  id: string
  username: string
  created_at: string
  role: GlobalRole
  managed_ensemble_ids: string[]
  editable_ensemble_ids: string[]
  created_by_user_id: string | null
}

export type EnsembleMember = {
  user_id: string
  role: EnsembleRole
}

export type Ensemble = {
  id: string
  name: string
  created_at: string
  members: EnsembleMember[]
  score_count: number
  created_by_user_id: string | null
}

export type LoginLinkResponse = {
  connection_url: string
  expires_at: string
}

export type AuthTokenResponse = {
  refresh_token: string
  access_token: string
  access_token_expires_at: string
  user: AppUser
}

export type AccessTokenRefreshResponse = {
  access_token: string
  access_token_expires_at: string
}

export type CurrentUserResponse = {
  session_expires_at: string | null
  user: AppUser
}

export type UserLibraryScore = {
  id: string
  title: string
  icon: string | null
  icon_image_url: string | null
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
      value: 'standard',
      label: 'Standard',
      description: 'Keep the MuseScore-rendered OGG stems as-is with no extra compression.',
    },
    {
      value: 'small',
      label: 'Small',
      description: 'Apply light Opus recompression for smaller stem files.',
    },
    {
      value: 'very-small',
      label: 'Very small',
      description: 'Apply stronger Opus recompression to reduce stem size further.',
    },
    {
      value: 'tiny',
      label: 'Tiny',
      description: 'Apply the strongest Opus recompression for the smallest stored stems.',
    },
  ]

export type PublicMusic = {
  title: string
  icon: string | null
  icon_image_url: string | null
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

type JsonOptions = RequestInit & { authenticated?: boolean }

type RuntimeConfig = {
  apiBaseUrl?: string
}

export class ApiError extends Error {
  status: number

  constructor(message: string, status: number) {
    super(message)
    this.name = 'ApiError'
    this.status = status
  }
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

// ── Module-level JWT auth state ───────────────────────────────────────────

let _refreshToken: string | null = null
let _accessToken: string | null = null
let _accessTokenExp: number = 0
let _refreshPromise: Promise<string> | null = null
let _onSessionExpired: (() => void) | null = null
let _onTokenRefreshed: ((accessToken: string) => void) | null = null
let _isPageUnloading = false

if (typeof window !== 'undefined') {
  window.addEventListener('beforeunload', () => {
    _isPageUnloading = true
  })
}

export function isPageUnloading(): boolean {
  return _isPageUnloading
}

export function setOnSessionExpired(cb: () => void): void {
  _onSessionExpired = cb
}

export function setOnTokenRefreshed(cb: (accessToken: string) => void): void {
  _onTokenRefreshed = cb
}

export function initAuth(refreshToken: string, accessToken: string): void {
  _refreshToken = refreshToken
  _accessToken = accessToken
  _accessTokenExp = parseJwtExp(accessToken)
}

export function clearAuth(): void {
  _refreshToken = null
  _accessToken = null
  _accessTokenExp = 0
  _refreshPromise = null
}

export function hasAuth(): boolean {
  return _refreshToken !== null && _refreshToken !== ''
}

function parseJwtExp(token: string): number {
  try {
    const payload = JSON.parse(atob(token.split('.')[1])) as { exp?: number }
    return payload.exp ?? 0
  } catch {
    return 0
  }
}

async function getAccessToken(): Promise<string> {
  if (!_refreshToken) throw new Error('Not authenticated')

  // Token valid for more than 60 more seconds — return immediately
  if (_accessToken && Date.now() / 1000 < _accessTokenExp - 60) {
    return _accessToken
  }

  // Deduplicate concurrent refresh calls
  if (_refreshPromise) return _refreshPromise

  _refreshPromise = (async (): Promise<string> => {
    try {
      const resp = await _callRefreshEndpoint(_refreshToken!)
      _accessToken = resp.access_token
      _accessTokenExp = parseJwtExp(resp.access_token)
      _onTokenRefreshed?.(_accessToken)
      return _accessToken
    } catch (err) {
      // Don't clear auth if the request was aborted (e.g. page unload).
      if (_isPageUnloading || (err instanceof Error && err.name === 'AbortError')) throw err
      if (err instanceof ApiError && err.status === 401) {
        clearAuth()
        _onSessionExpired?.()
        throw new Error('Session expired. Please sign in again.')
      }

      throw err
    } finally {
      _refreshPromise = null
    }
  })()

  return _refreshPromise
}

async function _callRefreshEndpoint(refreshToken: string): Promise<AccessTokenRefreshResponse> {
  return requestJson<AccessTokenRefreshResponse>('/auth/refresh', {
    method: 'POST',
    body: JSON.stringify({ refresh_token: refreshToken }),
  })
}

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
    icon_image_url: resolveBackendAssetUrl(music.icon_image_url),
    download_url: resolveBackendAssetUrl(music.download_url) ?? music.download_url,
    midi_download_url: resolveBackendAssetUrl(music.midi_download_url),
  }
}

function normalizePublicMusic(music: PublicMusic): PublicMusic {
  return {
    ...music,
    icon_image_url: resolveBackendAssetUrl(music.icon_image_url),
    audio_stream_url: resolveBackendAssetUrl(music.audio_stream_url),
    midi_download_url: resolveBackendAssetUrl(music.midi_download_url),
    musicxml_url: resolveBackendAssetUrl(music.musicxml_url),
    download_url: resolveBackendAssetUrl(music.download_url) ?? music.download_url,
  }
}

function normalizeUserLibraryResponse(library: UserLibraryResponse): UserLibraryResponse {
  return {
    ensembles: library.ensembles.map((ensemble) => ({
      ...ensemble,
      scores: ensemble.scores.map((score) => ({
        ...score,
        icon_image_url: resolveBackendAssetUrl(score.icon_image_url),
      })),
    })),
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

  if (options.authenticated) {
    const token = await getAccessToken()
    headers.set('authorization', `Bearer ${token}`)
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

    throw new ApiError(message, response.status)
  }

  if (response.status === 204) {
    return undefined as T
  }

  return (await response.json()) as T
}

async function requestBlob(path: string, options: JsonOptions = {}): Promise<Blob> {
  const headers = new Headers(options.headers)

  if (options.authenticated) {
    const token = await getAccessToken()
    headers.set('authorization', `Bearer ${token}`)
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

    throw new ApiError(message, response.status)
  }

  return response.blob()
}

export async function listMusics(): Promise<AdminMusic[]> {
  const musics = await requestJson<AdminMusic[]>('/admin/musics', {
    authenticated: true,
  })

  return musics.map(normalizeAdminMusic)
}

export async function listUsers(): Promise<AppUser[]> {
  return requestJson<AppUser[]>('/admin/users', {
    authenticated: true,
  })
}

export async function createUser(
  username: string,
  role?: Exclude<GlobalRole, 'superadmin'>,
): Promise<AppUser> {
  return requestJson<AppUser>('/admin/users', {
    method: 'POST',
    authenticated: true,
    body: JSON.stringify({ username, role: role ?? 'user' }),
  })
}

export async function deleteUser(id: string): Promise<void> {
  await requestJson(`/admin/users/${id}`, {
    method: 'DELETE',
    authenticated: true,
  })
}

export async function createAdminUserLoginLink(
  userId: string,
): Promise<LoginLinkResponse> {
  return requestJson<LoginLinkResponse>(`/admin/users/${userId}/login-link`, {
    method: 'POST',
    authenticated: true,
  })
}

export async function listEnsembles(): Promise<Ensemble[]> {
  return requestJson<Ensemble[]>('/admin/ensembles', {
    authenticated: true,
  })
}

export async function createEnsemble(name: string): Promise<Ensemble> {
  return requestJson<Ensemble>('/admin/ensembles', {
    method: 'POST',
    authenticated: true,
    body: JSON.stringify({ name }),
  })
}

export async function deleteEnsemble(id: string): Promise<void> {
  await requestJson(`/admin/ensembles/${id}`, {
    method: 'DELETE',
    authenticated: true,
  })
}

export async function addUserToEnsemble(
  ensembleId: string,
  userId: string,
  role: EnsembleRole,
): Promise<void> {
  await requestJson(`/admin/ensembles/${ensembleId}/users/${userId}`, {
    method: 'POST',
    authenticated: true,
    body: JSON.stringify({ role }),
  })
}

export async function removeUserFromEnsemble(
  ensembleId: string,
  userId: string,
): Promise<void> {
  await requestJson(`/admin/ensembles/${ensembleId}/users/${userId}`, {
    method: 'DELETE',
    authenticated: true,
  })
}

export async function addMusicToEnsemble(
  musicId: string,
  ensembleId: string,
): Promise<void> {
  await requestJson(`/admin/musics/${musicId}/ensembles/${ensembleId}`, {
    method: 'POST',
    authenticated: true,
  })
}

export async function removeMusicFromEnsemble(
  musicId: string,
  ensembleId: string,
): Promise<void> {
  await requestJson(`/admin/musics/${musicId}/ensembles/${ensembleId}`, {
    method: 'DELETE',
    authenticated: true,
  })
}

export async function uploadMusic(
  payload: {
    file: File
    title: string
    icon: string
    iconFile?: File | null
    publicId: string
    qualityProfile: StemQualityProfile
    ensembleId: string
  },
): Promise<AdminMusic> {
  const body = new FormData()
  body.append('file', payload.file)
  body.append('title', payload.title)
  body.append('icon', payload.icon)
  if (payload.iconFile) body.append('icon_file', payload.iconFile)
  body.append('public_id', payload.publicId)
  body.append('quality_profile', payload.qualityProfile)
  body.append('ensemble_id', payload.ensembleId)

  const music = await requestJson<AdminMusic>('/admin/musics', {
    method: 'POST',
    authenticated: true,
    body,
  })

  return normalizeAdminMusic(music)
}

export async function retryRender(id: string): Promise<AdminMusic> {
  const music = await requestJson<AdminMusic>(`/admin/musics/${id}/retry`, {
    method: 'POST',
    authenticated: true,
  })

  return normalizeAdminMusic(music)
}

export async function updateMusicMetadata(
  id: string,
  payload: {
    title: string
    publicId: string
    icon?: string
  },
): Promise<AdminMusic> {
  const music = await requestJson<AdminMusic>(`/admin/musics/${id}`, {
    method: 'PATCH',
    authenticated: true,
    body: JSON.stringify({
      title: payload.title.trim(),
      public_id: payload.publicId.trim() ? payload.publicId.trim() : null,
      icon:
        payload.icon !== undefined
          ? payload.icon.trim()
            ? payload.icon.trim()
            : null
          : null,
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

export async function exchangeLoginToken(token: string): Promise<AuthTokenResponse> {
  return requestJson<AuthTokenResponse>('/auth/exchange', {
    method: 'POST',
    body: JSON.stringify({ token }),
  })
}

export async function moveMusic(
  id: string,
  ensembleId: string,
): Promise<AdminMusic> {
  const music = await requestJson<AdminMusic>(`/admin/musics/${id}/move`, {
    method: 'POST',
    authenticated: true,
    body: JSON.stringify({ ensemble_id: ensembleId }),
  })

  return normalizeAdminMusic(music)
}

export async function deleteMusic(id: string): Promise<void> {
  await requestJson(`/admin/musics/${id}/delete`, {
    method: 'POST',
    authenticated: true,
  })
}

export async function fetchCurrentUser(): Promise<CurrentUserResponse> {
  return requestJson<CurrentUserResponse>('/me', {
    authenticated: true,
  })
}

export async function fetchUserLibrary(): Promise<UserLibraryResponse> {
  const library = await requestJson<UserLibraryResponse>('/me/library', {
    authenticated: true,
  })

  return normalizeUserLibraryResponse(library)
}

export async function createMyLoginLink(): Promise<LoginLinkResponse> {
  return requestJson<LoginLinkResponse>('/me/login-link', {
    method: 'POST',
    authenticated: true,
  })
}

export async function logout(): Promise<void> {
  await requestJson('/me/logout', {
    method: 'POST',
    authenticated: true,
  })
}
