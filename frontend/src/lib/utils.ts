import { STEM_QUALITY_PROFILES } from './api'

export function prettyDate(value: string): string {
    return new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'short' }).format(
        new Date(value),
    )
}

export function formatBytes(bytes: number): string {
    if (bytes === 0) return '—'
    if (bytes < 1024) return `${bytes} B`
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
}

export function formatTime(seconds: number): string {
    const safeSeconds = Math.max(0, Math.floor(seconds))
    const minutes = Math.floor(safeSeconds / 60)
    const remainingSeconds = safeSeconds % 60
    return `${minutes}:${remainingSeconds.toString().padStart(2, '0')}`
}

export function formatPlaytimeDuration(seconds: number): string {
    const safeSeconds = Math.max(0, Math.round(seconds))
    const hours = Math.floor(safeSeconds / 3600)
    const minutes = Math.floor((safeSeconds % 3600) / 60)
    const remainingSeconds = safeSeconds % 60
    const parts: string[] = []

    if (hours > 0) parts.push(`${hours}h`)
    if (hours > 0 || minutes > 0) parts.push(`${minutes}m`)
    parts.push(`${remainingSeconds}s`)

    return parts.join(' ')
}

export function qualityProfileLabel(profile: string): string {
    if (profile === 'compact') return 'Small'
    if (profile === 'high') return 'Very small'
    return STEM_QUALITY_PROFILES.find((option) => option.value === profile)?.label ?? profile
}

/** Decode the `sub` claim from a JWT access token without verifying the signature. */
export function parseJwtSub(token: string): string {
    try {
        const payload = JSON.parse(atob(token.split('.')[1].replace(/-/g, '+').replace(/_/g, '/')))
        return typeof payload.sub === 'string' ? payload.sub : ''
    } catch {
        return ''
    }
}
