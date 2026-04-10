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
  buffer: AudioBuffer
  gain: GainNode
  analyser: AnalyserNode
  analyserData: Uint8Array<ArrayBuffer>
  // Created fresh on every play(); null when paused/stopped.
  // AudioBufferSourceNode cannot be reused after stop().
  sourceNode: AudioBufferSourceNode | null
  volume: number
  muted: boolean
}

export class StemMixerPlayer {
  // Small lookahead so all source nodes are scheduled before the start time
  // arrives.  50 ms is imperceptible but gives the browser enough runway.
  private static readonly START_LOOKAHEAD_SECONDS = 0.05

  private context: AudioContext | null = null
  private tracks = new Map<string, InternalTrack>()
  private _duration = 0
  private _levelMultiplier = 15
  private _isPlaying = false
  private _playbackOffset = 0
  private _isReadyToPlay = false
  // The AudioContext.currentTime value at which playback was scheduled to
  // begin (i.e. START_LOOKAHEAD_SECONDS in the future from the moment play()
  // ran).  Used as the single authoritative clock; no per-track clocks exist.
  private _contextTimeAtStart = 0

  async loadStems(stems: StemSource[], onProgress?: (progress: number) => void): Promise<LoadedStems> {
    await this.dispose()

    this.context = new AudioContext()
    this._duration = stems.reduce((max, s) => Math.max(max, s.durationSeconds), 0)
    this._isReadyToPlay = false

    // Per-stem byte counters for progress reporting.
    type StemBytes = { loaded: number; total: number }
    const stemBytes = new Map<string, StemBytes>(stems.map((s) => [s.id, { loaded: 0, total: 0 }]))

    const reportProgress = () => {
      if (!onProgress) return
      const all = [...stemBytes.values()]
      const totalBytes = all.reduce((s, p) => s + p.total, 0)
      const loadedBytes = all.reduce((s, p) => s + p.loaded, 0)
      if (totalBytes > 0) {
        onProgress(Math.min(1, loadedBytes / totalBytes))
      } else {
        // No Content-Length headers — report by count of fully-loaded stems.
        const done = all.filter((p) => p.loaded > 0 && p.loaded === p.total).length
        onProgress(done / stems.length)
      }
    }

    // Fetch and decode all stems in parallel.  AudioBufferSourceNode requires
    // the full decoded PCM in memory; in exchange we get sample-accurate start
    // and zero per-element clock skew.
    await Promise.all(stems.map(async (stem) => {
      const response = await fetch(stem.fullStemUrl)
      if (!response.ok) {
        throw new Error(`Failed to fetch stem "${stem.name}": HTTP ${response.status}`)
      }

      const contentLength = Number(response.headers.get('Content-Length') ?? '0')
      stemBytes.set(stem.id, { loaded: 0, total: contentLength })
      reportProgress()

      // Stream the response body so we can count bytes as they arrive.
      let arrayBuffer: ArrayBuffer
      if (response.body && contentLength > 0) {
        const reader = response.body.getReader()
        const chunks: Uint8Array[] = []
        let received = 0
        while (true) {
          const { done, value } = await reader.read()
          if (done) break
          chunks.push(value)
          received += value.byteLength
          stemBytes.set(stem.id, { loaded: received, total: contentLength })
          reportProgress()
        }
        arrayBuffer = await new Blob(chunks).arrayBuffer()
      } else {
        // Fallback: no streaming (no Content-Length or no body stream).
        arrayBuffer = await response.arrayBuffer()
        const size = arrayBuffer.byteLength
        stemBytes.set(stem.id, { loaded: size, total: size })
        reportProgress()
      }

      // decodeAudioData runs in a worker thread; multiple parallel calls are fine.
      const buffer = await this.context!.decodeAudioData(arrayBuffer)

      const gain = this.context!.createGain()
      gain.gain.value = 1
      const analyser = this.context!.createAnalyser()
      analyser.fftSize = 1024
      analyser.smoothingTimeConstant = 0.6

      // gain -> analyser -> destination stays wired for the lifetime of the
      // AudioContext.  The transient sourceNode plugs into gain on each play().
      gain.connect(analyser)
      analyser.connect(this.context!.destination)

      this.tracks.set(stem.id, {
        meta: stem,
        buffer,
        gain,
        analyser,
        analyserData: new Uint8Array(analyser.fftSize) as Uint8Array<ArrayBuffer>,
        sourceNode: null,
        volume: 1,
        muted: false,
      })
    }))

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

    // Schedule every track to start at exactly the same AudioContext time.
    // AudioBufferSourceNode.start(when, offset) is sample-accurate: all tracks
    // begin on the same audio-engine clock tick regardless of JS event-loop
    // timing, eliminating any inter-track drift at the source.
    const startAt = this.context.currentTime + StemMixerPlayer.START_LOOKAHEAD_SECONDS
    const offset = Math.max(0, Math.min(this._playbackOffset, this._duration))

    for (const track of this.tracks.values()) {
      this._releaseSourceNode(track)

      if (offset >= track.buffer.duration) {
        // Track is shorter than the seek position -- nothing to schedule.
        // Silence flows naturally through the persistent gain/analyser chain.
        continue
      }

      const sourceNode = this.context.createBufferSource()
      sourceNode.buffer = track.buffer
      sourceNode.connect(track.gain)
      sourceNode.start(startAt, offset)
      track.sourceNode = sourceNode
    }

    this._contextTimeAtStart = startAt
    this._isPlaying = true
  }

  pause(): void {
    if (!this._isPlaying) return
    this._playbackOffset = this.getCurrentTime()
    this._isPlaying = false
    this._contextTimeAtStart = 0
    for (const track of this.tracks.values()) {
      this._releaseSourceNode(track)
    }
  }

  stop(): void {
    this._isPlaying = false
    this._playbackOffset = 0
    this._contextTimeAtStart = 0
    for (const track of this.tracks.values()) {
      this._releaseSourceNode(track)
    }
  }

  seek(seconds: number): void {
    // Seeking is synchronous and instant -- no buffering delay, no async events.
    // The new offset is picked up by the next play() call.
    this._playbackOffset = Math.max(0, Math.min(seconds, this._duration))
    this._contextTimeAtStart = 0
  }

  getCurrentTime(): number {
    if (!this._isPlaying || !this.context || this._contextTimeAtStart === 0) {
      return this._playbackOffset
    }
    // All source nodes share the same AudioContext clock so this formula is
    // exact -- no re-anchoring, no polling of individual element positions.
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
    // AudioBufferSourceNode is sample-accurate -- all tracks share one clock.
    // No drift correction is necessary or possible.  Kept for API compatibility.
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
      this._releaseSourceNode(track)
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

  private _releaseSourceNode(track: InternalTrack): void {
    if (!track.sourceNode) return
    try { track.sourceNode.stop() } catch { /* already stopped or never started */ }
    track.sourceNode.disconnect()
    track.sourceNode = null
  }

  private _applyTrackGain(track: InternalTrack): void {
    track.gain.gain.value = track.muted ? 0 : track.volume
  }
}
