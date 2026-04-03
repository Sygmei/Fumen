/**
 * StemMixerPlayer — plays per-instrument OGG stems rendered by the backend
 * using the Web Audio API.
 *
 * Stems are fully fetched and decoded into AudioBuffers, then played via
 * AudioBufferSourceNode.  All sources are scheduled to start at the same
 * future AudioContext clock time (+100 ms), giving sample-accurate
 * synchronisation that is impossible to achieve with HTMLAudioElement.
 *
 * The 100 ms scheduling headroom also eliminates the jitter introduced by
 * calling HTMLAudioElement.play() in sequence: every source fires at
 * exactly the same audio-thread sample regardless of JS execution time.
 */

export type StemTrack = {
  id: string
  trackIndex: number
  name: string
  instrumentName: string
  volume: number
  muted: boolean
}

export type LoadedStems = {
  tracks: StemTrack[]
  duration: number
}

type InternalTrack = {
  buffer: AudioBuffer
  gain: GainNode
  analyser: AnalyserNode
  analyserData: Uint8Array<ArrayBuffer>
  volume: number
  muted: boolean
  /** Active source node; recreated on every play/seek. */
  source: AudioBufferSourceNode | null
}

export class StemMixerPlayer {
  private context: AudioContext | null = null
  private tracks = new Map<string, InternalTrack>()
  private _duration = 0
  private _levelMultiplier = 6

  // Playback position tracking via the AudioContext clock.
  private _isPlaying = false
  /** Offset (seconds) into the audio where the last play() call started. */
  private _playbackStartOffset = 0
  /** context.currentTime at which the last play() scheduled the sources. */
  private _playbackStartCtxTime = 0

  async loadStems(
    stems: Array<{ id: string; name: string; instrumentName: string; streamUrl: string }>,
  ): Promise<LoadedStems> {
    await this.dispose()

    this.context = new AudioContext()
    await Promise.all(stems.map((stem) => this.loadOneStem(stem)))

    const result: StemTrack[] = stems
      .filter((s) => this.tracks.has(s.id))
      .map((s) => ({
        id: s.id,
        trackIndex: Number(s.id),
        name: s.name,
        instrumentName: s.instrumentName,
        volume: 0.5,
        muted: false,
      }))

    return { tracks: result, duration: this._duration }
  }

  private async loadOneStem(stem: {
    id: string
    name: string
    streamUrl: string
  }): Promise<void> {
    const response = await fetch(stem.streamUrl)
    if (!response.ok) {
      throw new Error(`Failed to fetch stem "${stem.name}": HTTP ${response.status}`)
    }
    const arrayBuffer = await response.arrayBuffer()
    // decodeAudioData handles Opus pre-skip trimming automatically, so the
    // resulting AudioBuffer starts at the true t=0 of the audio signal.
    const audioBuffer = await this.context!.decodeAudioData(arrayBuffer)

    if (audioBuffer.duration > this._duration) {
      this._duration = audioBuffer.duration
    }

    const gain = this.context!.createGain()
    gain.gain.value = 0.5
    const analyser = this.context!.createAnalyser()
    analyser.fftSize = 1024
    analyser.smoothingTimeConstant = 0.6
    gain.connect(analyser)
    analyser.connect(this.context!.destination)

    this.tracks.set(stem.id, {
      buffer: audioBuffer,
      gain,
      analyser,
      analyserData: new Uint8Array(new ArrayBuffer(analyser.fftSize)),
      volume: 0.5,
      muted: false,
      source: null,
    })
  }

  async play(): Promise<void> {
    if (!this.context) return
    if (this.context.state === 'suspended') {
      await this.context.resume()
    }

    // Schedule all sources to fire at the same audio-clock instant (+100 ms
    // of headroom so the audio thread has time to prepare all buffers before
    // the first sample is due).
    const startAt = this.context.currentTime + 0.1
    const offset = this._playbackStartOffset

    for (const t of this.tracks.values()) {
      if (t.source) {
        try { t.source.stop() } catch { /* already stopped */ }
        t.source.disconnect()
        t.source = null
      }
      const src = this.context.createBufferSource()
      src.buffer = t.buffer
      src.connect(t.gain)
      src.start(startAt, offset)
      t.source = src
    }

    this._playbackStartCtxTime = startAt
    this._playbackStartOffset = offset
    this._isPlaying = true
  }

  pause(): void {
    if (!this._isPlaying) return
    // Snapshot position before stopping so getCurrentTime() stays accurate.
    this._playbackStartOffset = this.getCurrentTime()
    this._isPlaying = false
    this._stopAllSources()
  }

  stop(): void {
    this._isPlaying = false
    this._playbackStartOffset = 0
    this._stopAllSources()
  }

  seek(seconds: number): void {
    const clamped = Math.max(0, Math.min(seconds, this._duration))
    const wasPlaying = this._isPlaying
    if (wasPlaying) {
      this._isPlaying = false
      this._stopAllSources()
    }
    this._playbackStartOffset = clamped
    if (wasPlaying) {
      // play() is synchronous when the context is already running.
      void this.play()
    }
  }

  getCurrentTime(): number {
    if (!this.context) return this._playbackStartOffset
    if (!this._isPlaying) return this._playbackStartOffset
    const elapsed = this.context.currentTime - this._playbackStartCtxTime
    return Math.min(this._playbackStartOffset + Math.max(0, elapsed), this._duration)
  }

  getDuration(): number {
    return this._duration
  }

  isPlaying(): boolean {
    return this._isPlaying
  }

  /** Returns a 0–1 RMS level for the given track, suitable for a VU meter. */
  getLevel(trackId: string): number {
    const t = this.tracks.get(trackId)
    if (!t) return 0
    t.analyser.getByteTimeDomainData(t.analyserData)
    let sum = 0
    for (let i = 0; i < t.analyserData.length; i++) {
      const v = (t.analyserData[i] - 128) / 128
      sum += v * v
    }
    return Math.min(1, Math.sqrt(sum / t.analyserData.length) * this._levelMultiplier)
  }

  setLevelMultiplier(value: number): void {
    this._levelMultiplier = Math.max(1, value)
  }

  setTrackVolume(trackId: string, volume: number): void {
    const t = this.tracks.get(trackId)
    if (!t || !this.context) return
    t.volume = Math.max(0, Math.min(1, volume))
    if (!t.muted) {
      t.gain.gain.setTargetAtTime(t.volume, this.context.currentTime, 0.01)
    }
  }

  setTrackMuted(trackId: string, muted: boolean): void {
    const t = this.tracks.get(trackId)
    if (!t || !this.context) return
    t.muted = muted
    t.gain.gain.setTargetAtTime(
      muted ? 0 : t.volume,
      this.context.currentTime,
      0.01,
    )
  }

  async dispose(): Promise<void> {
    this._isPlaying = false
    this._playbackStartOffset = 0
    this._stopAllSources()
    for (const t of this.tracks.values()) {
      t.gain.disconnect()
      t.analyser.disconnect()
    }
    this.tracks.clear()
    this._duration = 0
    if (this.context) {
      await this.context.close()
      this.context = null
    }
  }

  private _stopAllSources(): void {
    for (const t of this.tracks.values()) {
      if (t.source) {
        try { t.source.stop() } catch { /* already stopped */ }
        t.source.disconnect()
        t.source = null
      }
    }
  }
}
