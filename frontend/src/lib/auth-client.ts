import { ApiClient as GeneratedApiClient } from '../adapters/fumen-backend/src'
import type { ErrorResponse } from '../adapters/fumen-backend/src/models'

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
    return new URL(
      normalizeApiBaseUrl(configuredValue),
      globalThis.location?.origin ?? 'http://localhost',
    ).toString().replace(/\/+$/, '')
  }

  if (import.meta.env.DEV) {
    return 'http://127.0.0.1:3000/api'
  }

  throw new Error(
    'Missing API base URL. Set VITE_API_BASE_URL for local development or API_BASE_URL in the frontend runtime.',
  )
}

function resolveApiClientBaseUrl(apiBaseUrl: string): string {
  return apiBaseUrl.replace(/\/api\/?$/, '')
}

const API_BASE_URL = resolveApiBaseUrl()
const API_CLIENT_BASE_URL = resolveApiClientBaseUrl(API_BASE_URL)

let refreshToken: string | null = null
let accessToken: string | null = null
let accessTokenExp = 0
let refreshPromise: Promise<string> | null = null
let onSessionExpired: (() => void) | null = null
let onTokenRefreshed: ((nextAccessToken: string) => void) | null = null
let pageUnloading = false

if (typeof window !== 'undefined') {
  window.addEventListener('beforeunload', () => {
    pageUnloading = true
  })
}

export function isPageUnloading(): boolean {
  return pageUnloading
}

export function setOnSessionExpired(callback: () => void): void {
  onSessionExpired = callback
}

export function setOnTokenRefreshed(callback: (nextAccessToken: string) => void): void {
  onTokenRefreshed = callback
}

export function initAuth(nextRefreshToken: string, nextAccessToken: string): void {
  refreshToken = nextRefreshToken
  accessToken = nextAccessToken
  accessTokenExp = parseJwtExp(nextAccessToken)
}

export function clearAuth(): void {
  refreshToken = null
  accessToken = null
  accessTokenExp = 0
  refreshPromise = null
}

export function hasAuth(): boolean {
  return refreshToken !== null && refreshToken !== ''
}

function parseJwtExp(token: string): number {
  try {
    const payload = JSON.parse(atob(token.split('.')[1])) as { exp?: number }
    return payload.exp ?? 0
  } catch {
    return 0
  }
}

async function handleApiError(response: Response): Promise<never> {
  let message = `Request failed with status ${response.status}`

  try {
    const payload = (await response.json()) as ErrorResponse
    if (payload.error) {
      message = payload.error
    }
  } catch {
    // Keep the fallback message if parsing fails.
  }

  throw new ApiError(message, response.status)
}

const baseFetch: typeof globalThis.fetch = (...args) => globalThis.fetch(...args)

async function getAccessToken(): Promise<string> {
  if (!refreshToken) throw new Error('Not authenticated')

  if (accessToken && Date.now() / 1000 < accessTokenExp - 60) {
    return accessToken
  }

  if (refreshPromise) return refreshPromise

  refreshPromise = (async (): Promise<string> => {
    try {
      const response = await publicApiClient.refreshAccessToken({ refresh_token: refreshToken! })
      accessToken = response.access_token
      accessTokenExp = parseJwtExp(response.access_token)
      onTokenRefreshed?.(accessToken)
      return accessToken
    } catch (error) {
      if (pageUnloading || (error instanceof Error && error.name === 'AbortError')) throw error
      if (error instanceof ApiError && error.status === 401) {
        clearAuth()
        onSessionExpired?.()
        throw new Error('Session expired. Please sign in again.')
      }

      throw error
    } finally {
      refreshPromise = null
    }
  })()

  return refreshPromise
}

async function authenticatedFetch(
  input: Parameters<typeof fetch>[0],
  init?: Parameters<typeof fetch>[1],
): Promise<Response> {
  const token = await getAccessToken()
  const request = new Request(input, init)
  const headers = new Headers(request.headers)
  headers.set('authorization', `Bearer ${token}`)
  return baseFetch(new Request(request, { headers }))
}

export const publicApiClient = new GeneratedApiClient({
  baseUrl: API_CLIENT_BASE_URL,
  fetch: baseFetch,
  onError: handleApiError,
})

export const authenticatedApiClient = new GeneratedApiClient({
  baseUrl: API_CLIENT_BASE_URL,
  fetch: authenticatedFetch,
  onError: handleApiError,
})
