export type StemTrack = {
  id: string
  name: string
  instrumentName: string
  volume: number
  muted: boolean
}

export type LoadedStems = {
  duration: number
  tracks: StemTrack[]
}

type StemSource = {
  id: string
  name: string
  instrumentName: string
  fullStemUrl: string
  durationSeconds: number
}

type InternalTrack = {
  meta: StemSource
  element: HTMLAudioElement
  source: MediaElementAudioSourceNode
  gain: GainNode
  analyser: AnalyserNode
  analyserData: Uint8Array<ArrayBuffer>
  volume: number
  muted: boolean
}

export class StemMixerPlayer {
  private static readonly SEEK_TOLERANCE_SECONDS = 0.03
  private static readonly STALL_THRESHOLD_SECONDS = 1.0
  private context: AudioContext | null = null
  private tracks = new Map<string, InternalTrack>()
  private _duration = 0
  private _levelMultiplier = 15
  private _isPlaying = false
  private _playbackOffset = 0
  private _isReadyToPlay = false
  // AudioContext time recorded when play() transitions to playing.
  // Used as master clock so getCurrentTime() doesn't depend on any
  // individual HTMLAudioElement.currentTime (which has ~100ms jitter).
  private _contextTimeAtStart = 0

  async loadStems(stems: StemSource[]): Promise<LoadedStems> {
    await this.dispose()

    this.context = new AudioContext()
    this._duration = stems.reduce((max, stem) => Math.max(max, stem.durationSeconds), 0)
    this._isReadyToPlay = false
    const readinessTasks: Array<Promise<void>> = []

    for (const stem of stems) {
      const element = new Audio()
      element.preload = 'auto'
      element.crossOrigin = 'anonymous'
      element.src = stem.fullStemUrl

      const source = this.context.createMediaElementSource(element)
      const gain = this.context.createGain()
      gain.gain.value = 1
      const analyser = this.context.createAnalyser()
      analyser.fftSize = 1024
      analyser.smoothingTimeConstant = 0.6

      source.connect(gain)
      gain.connect(analyser)
      analyser.connect(this.context.destination)

      this.tracks.set(stem.id, {
        meta: stem,
        element,
        source,
        gain,
        analyser,
        analyserData: new Uint8Array(analyser.fftSize) as Uint8Array<ArrayBuffer>,
        volume: 1,
        muted: false,
      })

      readinessTasks.push(this.prepareTrackForPlayback(element, 0, stem.fullStemUrl))
    }

    await Promise.all(readinessTasks)
    this._isReadyToPlay = true

    return {
      duration: this._duration,
      tracks: stems.map((stem) => ({
        id: stem.id,
        name: stem.name,
        instrumentName: stem.instrumentName,
        volume: 1,
        muted: false,
      })),
    }
  }

  async play(): Promise<void> {
    if (!this.context) return
    if (this.context.state === 'suspended') {
      await this.context.resume()
    }

    const seekTime = this._playbackOffset
    console.log(`[StemMixer] play() offset=${seekTime.toFixed(3)} tracks=${this.tracks.size}`)
    await Promise.all(
      [...this.tracks.values()].map((track) =>
        this.prepareTrackForPlayback(track.element, seekTime, track.meta.fullStemUrl),
      ),
    )
    console.log(`[StemMixer] play() all tracks prepared, starting...`)

    const starts = [...this.tracks.values()].map(async (track) => {
      try {
        await track.element.play()
      } catch (error) {
        throw error instanceof Error ? error : new Error('Unable to start stem playback')
      }
    })

    await Promise.all(starts)
    // Latch the AudioContext clock so getCurrentTime() is driven by the
    // high-precision Web Audio clock, not by HTMLAudioElement.currentTime.
    this._contextTimeAtStart = this.context.currentTime
    this._isPlaying = true
  }

  pause(): void {
    if (!this._isPlaying) return
    this._playbackOffset = this.getCurrentTime()
    this._contextTimeAtStart = 0
    this._isPlaying = false
    for (const track of this.tracks.values()) {
      track.element.pause()
    }
  }

  stop(): void {
    this._isPlaying = false
    this._playbackOffset = 0
    this._contextTimeAtStart = 0
    for (const track of this.tracks.values()) {
      track.element.pause()
      track.element.currentTime = 0
    }
  }

  seek(seconds: number): void {
    const clamped = Math.max(0, Math.min(seconds, this._duration))
    this._playbackOffset = clamped
    this._contextTimeAtStart = 0  // reset; play() will latch a new value
    for (const track of this.tracks.values()) {
      const cappedTime = Math.min(clamped, this.maxSeekTimeForTrack(track))
      if (Math.abs(track.element.currentTime - cappedTime) > StemMixerPlayer.SEEK_TOLERANCE_SECONDS) {
        track.element.currentTime = cappedTime
      }
    }
  }

  getCurrentTime(): number {
    if (!this._isPlaying || !this.context || this._contextTimeAtStart === 0) {
      return this._playbackOffset
    }
    const elapsed = this.context.currentTime - this._contextTimeAtStart
    return Math.min(this._playbackOffset + elapsed, this._duration)
  }

  getDuration(): number {
    return this._duration
  }

  isPlaying(): boolean {
    return this._isPlaying
  }

  isReadyToPlay(): boolean {
    return this._isReadyToPlay
  }

  synchronizePlayback(): void {
    if (!this._isPlaying || !this.context) return

    // Use the AudioContext clock as the authoritative position.
    // Only nudge elements that have drifted by more than 1 second —
    // i.e. a track that genuinely stalled (network hiccup, browser throttle).
    // Sub-second jitter is normal scheduling variance and must NOT be
    // corrected: writing currentTime on a playing element causes an audible
    // click every time.
    const expectedTime = this.getCurrentTime()

    for (const track of this.tracks.values()) {
      if (track.element.paused || track.element.ended || track.element.seeking) continue
      if (track.element.readyState < HTMLMediaElement.HAVE_CURRENT_DATA) continue

      const cappedExpected = Math.min(expectedTime, this.maxSeekTimeForTrack(track))
      const drift = Math.abs(track.element.currentTime - cappedExpected)
      if (drift > StemMixerPlayer.STALL_THRESHOLD_SECONDS) {
        track.element.currentTime = cappedExpected
      }
    }
  }

  getLevel(trackId: string): number {
    const track = this.tracks.get(trackId)
    if (!track) return 0
    track.analyser.getByteTimeDomainData(track.analyserData)

    let sumSquares = 0
    const data = track.analyserData
    for (let index = 0; index < data.length; index += 1) {
      const centered = (data[index] - 128) / 128
      sumSquares += centered * centered
    }

    const rms = Math.sqrt(sumSquares / data.length)
    return Math.min(1, rms * this._levelMultiplier)
  }

  setTrackVolume(trackId: string, volume: number): void {
    const track = this.tracks.get(trackId)
    if (!track) return
    track.volume = Math.max(0, Math.min(volume, 2))
    this._applyTrackGain(track)
  }

  setTrackMuted(trackId: string, muted: boolean): void {
    const track = this.tracks.get(trackId)
    if (!track) return
    track.muted = muted
    this._applyTrackGain(track)
  }

  setLevelMultiplier(multiplier: number): void {
    this._levelMultiplier = Math.max(1, multiplier)
  }

  async dispose(): Promise<void> {
    this._isPlaying = false
    this._playbackOffset = 0
    this._isReadyToPlay = false
    this._contextTimeAtStart = 0

    for (const track of this.tracks.values()) {
      track.element.pause()
      track.element.removeAttribute('src')
      track.element.load()
      track.source.disconnect()
      track.gain.disconnect()
      track.analyser.disconnect()
    }

    this.tracks.clear()
    this._duration = 0

    if (this.context) {
      await this.context.close()
      this.context = null
    }
  }

  private _applyTrackGain(track: InternalTrack): void {
    track.gain.gain.value = track.muted ? 0 : track.volume
  }

  private async prepareTrackForPlayback(
    element: HTMLAudioElement,
    seekTime: number,
    stemUrl: string,
  ): Promise<void> {
    const label = stemUrl.split('/').pop() ?? stemUrl

    console.log(`[prep:${label}] start seekTime=${seekTime.toFixed(3)} readyState=${element.readyState} seeking=${element.seeking} currentTime=${element.currentTime.toFixed(3)}`)
    await waitForLoadedMetadata(element, stemUrl)
    const maxTime = Number.isFinite(element.duration) ? element.duration : Infinity
    console.log(`[prep:${label}] metadata ok duration=${element.duration?.toFixed(3)} readyState=${element.readyState}`)

    // Track ends before the seek target — no content to buffer here, skip entirely.
    if (seekTime >= maxTime) {
      console.log(`[prep:${label}] past track end, skipping`)
      return
    }

    const cappedTime = Math.max(0, Math.min(seekTime, maxTime))
    const needsSeek = element.seeking || Math.abs(element.currentTime - cappedTime) > StemMixerPlayer.SEEK_TOLERANCE_SECONDS
    console.log(`[prep:${label}] needsSeek=${needsSeek} capped=${cappedTime.toFixed(3)}`)
    if (needsSeek) {
      console.log(`[prep:${label}] awaiting seeked...`)
      await seekMediaElement(element, cappedTime, stemUrl)
      console.log(`[prep:${label}] seeked done, readyState=${element.readyState} currentTime=${element.currentTime.toFixed(3)}`)
      return
    }

    // No seek needed. Only wait for canPlay on initial load when there's no data yet.
    // Tracks already positioned (readyState >= HAVE_CURRENT_DATA) are ready to play.
    if (element.readyState < HTMLMediaElement.HAVE_CURRENT_DATA) {
      console.log(`[prep:${label}] awaiting canPlay...`)
      await waitForCanPlay(element, stemUrl)
      console.log(`[prep:${label}] canPlay done, readyState=${element.readyState}`)
    }
  }

  private maxSeekTimeForTrack(track: InternalTrack): number {
    const measured = Number.isFinite(track.element.duration) ? track.element.duration : 0
    const fallback = track.meta.durationSeconds
    const duration = Math.max(measured, fallback, 0)
    return duration
  }


}

async function waitForLoadedMetadata(
  element: HTMLAudioElement,
  stemUrl: string,
): Promise<void> {
  if (element.readyState >= HTMLMediaElement.HAVE_METADATA && Number.isFinite(element.duration)) {
    return
  }

  await waitForMediaEvent(element, 'loadedmetadata', stemUrl)
}

async function waitForCanPlay(element: HTMLAudioElement, stemUrl: string): Promise<void> {
  if (element.readyState >= HTMLMediaElement.HAVE_FUTURE_DATA && !element.seeking) {
    return
  }

  await new Promise<void>((resolve, reject) => {
    const cleanup = () => {
      element.removeEventListener('canplay', handleReady)
      element.removeEventListener('canplaythrough', handleReady)
      element.removeEventListener('error', handleError)
    }
    const handleReady = () => { cleanup(); resolve() }
    const handleError = () => { cleanup(); reject(new Error(`Unable to stream stem: ${stemUrl}`)) }
    element.addEventListener('canplay', handleReady, { once: true })
    element.addEventListener('canplaythrough', handleReady, { once: true })
    element.addEventListener('error', handleError, { once: true })
  })
}

async function seekMediaElement(
  element: HTMLAudioElement,
  time: number,
  stemUrl: string,
): Promise<void> {
  if (!Number.isFinite(time)) {
    return
  }

  const seeked = waitForMediaEvent(element, 'seeked', stemUrl)
  element.currentTime = time
  await seeked
}

function waitForMediaEvent(
  element: HTMLAudioElement,
  eventName: keyof HTMLMediaElementEventMap,
  stemUrl: string,
): Promise<void> {
  return new Promise<void>((resolve, reject) => {
    const cleanup = () => {
      element.removeEventListener(eventName, handleReady)
      element.removeEventListener('error', handleError)
    }

    const handleReady = () => {
      cleanup()
      resolve()
    }

    const handleError = () => {
      cleanup()
      reject(new Error(`Unable to stream stem: ${stemUrl}`))
    }

    element.addEventListener(eventName, handleReady, { once: true })
    element.addEventListener('error', handleError, { once: true })
  })
}
