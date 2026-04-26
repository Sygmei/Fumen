/**
 * ScoreViewer — wraps OpenSheetMusicDisplay (OSMD) with a paged display:
 *
 *  - The score renders normally (measures wrap into rows / systems).
 *  - A "system map" is built after render: the Y offset of each system row.
 *  - The container is sized to show exactly ONE system at a time (height =
 *    tallest system + padding).  overflow is hidden so nothing bleeds.
 *  - On every seek() call we detect which system the cursor is on and
 *    instantly snap-scroll (scrollTop) so that system fills the window.
 *    No smooth scroll — the intent is a clean page-flip, not continuous scroll.
 *  - Instrument names are part of OSMD's normal system rendering and are
 *    always visible on the left edge of each system.
 */

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type OSMD = any

interface TimeEntry {
  seconds: number
  step: number
  measureIndex: number
  barNumber: number
  beatNumber: number
  wholeNotes: number
  relativeInMeasureWholeNotes: number
  /** Cursor element left edge relative to SVG left; -1 if not measured. */
  xPx: number
  /** Cursor element top relative to container content area; -1 if not measured. */
  topPx: number
}

/** Y pixel offset (relative to SVG top) for each rendered system. */
interface SystemEntry {
  topPx: number      // scrollTop that puts this system at the top of the viewport
  heightPx: number   // height of this system row
}

interface TempoInfo {
  bpm: number
  beatUnitWholeNotes: number
}

interface MeterInfo {
  beatsPerBar: number
  beatUnitWholeNotes: number
}

interface MeasureMetadata {
  meter: MeterInfo
  implicit: boolean
}

interface FermataAdjustment {
  extraWholeNotes: number
}

export interface CountInInfo {
  bpm: number
  beatsPerBar: number
  beatSeconds: number
  leadInBeats: number
  pickupBeats: number
}

export interface ScoreAnnotationAnchor {
  step: number
  seconds: number
  measureIndex: number
  systemIndex: number
  xPx: number
  topPx: number
  lineTopPx: number | null
}

export interface ScoreAnnotationContext {
  barNumber: number
  beatNumber: number
  positionLabel: string
  instrumentName: string | null
  systemYRatio: number | null
  clientX: number
  clientY: number
}

export interface ScoreVisibleInstrumentRow {
  instrumentName: string
  systemIndex: number
  topRatio: number
  centerRatio: number
  bottomRatio: number
  topPx: number
  centerPx: number
  bottomPx: number
  anchorPx: number
}

export interface ScoreSystemInfo {
  index: number
  topPx: number
  heightPx: number
}

export class ScoreViewer {
  private osmd: OSMD = null
  private timeMap: TimeEntry[] = []
  private systemMap: SystemEntry[] = []
  private currentStep = 0
  private currentSystemIdx = -1
  private container: HTMLElement
  private _clickHandler: ((e: MouseEvent) => void) | null = null
  private _contextMenuHandler: ((e: MouseEvent) => void) | null = null
  private readonly zoom: number
  private countInInfo: CountInInfo = {
    bpm: 120,
    beatsPerBar: 4,
    beatSeconds: 0.5,
    leadInBeats: 4,
    pickupBeats: 0,
  }
  private meterInfo: MeterInfo = { beatsPerBar: 4, beatUnitWholeNotes: 0.25 }
  private measureMetadata: MeasureMetadata[] = []
  private measureTempoMap: Map<number, TempoInfo> = new Map([
    [0, { bpm: 120, beatUnitWholeNotes: 0.25 }],
  ])
  private measureFermataAdjustments: Map<number, FermataAdjustment> = new Map()
  private initialTempo: TempoInfo = { bpm: 120, beatUnitWholeNotes: 0.25 }

  /** Called when the user clicks on the score. Argument is seconds into the piece. */
  onClickSeek: ((seconds: number) => void) | null = null

  /** Called when the user opens the score annotation menu. */
  onAnnotationContextMenu: ((context: ScoreAnnotationContext) => void) | null = null

  /** Called whenever the visible score system changes. */
  onSystemChange: ((system: ScoreSystemInfo) => void) | null = null

  constructor(container: HTMLElement) {
    this.container = container
    const viewportWidth = window.visualViewport?.width ?? window.innerWidth
    this.zoom = viewportWidth <= 840 ? 0.26 : 0.35
  }

  async load(musicXmlUrl: string): Promise<void> {
    const response = await fetch(musicXmlUrl)
    if (!response.ok) {
      throw new Error(`Failed to fetch MusicXML: HTTP ${response.status}`)
    }
    let xmlText = await response.text()
    const trimmed = xmlText.trimStart().toLowerCase()
    if (trimmed.startsWith('<!doctype html') || trimmed.startsWith('<html')) {
      throw new Error(
        `MusicXML URL returned HTML instead of XML: ${response.url}. Check that frontend asset URLs point to the backend API origin.`,
      )
    }
    xmlText = stripUnsupportedElements(xmlText)
    this.measureMetadata = this.extractMeasureMetadata(xmlText)
    this.meterInfo = this.measureMetadata[0]?.meter ?? this.extractMeterInfo(xmlText)
    this.measureTempoMap = this.buildMeasureTempoMap(xmlText)
    this.measureFermataAdjustments = this.buildMeasureFermataAdjustmentMap(xmlText)
    this.countInInfo = {
      bpm: this.initialTempo.bpm,
      beatsPerBar: this.meterInfo.beatsPerBar,
      beatSeconds: this.wholeNotesToSeconds(this.meterInfo.beatUnitWholeNotes, this.initialTempo) || 0.5,
      leadInBeats: Math.max(1, Math.round(this.meterInfo.beatsPerBar)),
      pickupBeats: 0,
    }

    const { OpenSheetMusicDisplay } = await import('opensheetmusicdisplay')

    this.osmd = new OpenSheetMusicDisplay(
      this.container,
      {
        autoResize: false,
        backend: 'svg',
        // Keep explicit rest measures in the rendered model so the cursor/seek
        // map can advance through sections where upper parts rest while lower
        // parts continue playing.
        autoGenerateMultipleRestMeasuresFromRestMeasures: false,
        drawTitle: false,
        drawSubtitle: false,
        drawComposer: false,
        drawCursors: true,
        followCursor: false,
        zoom: this.zoom,
      } as Record<string, unknown>,
    )

    await this.osmd.load(xmlText)

    // Ensure abbreviation labels = full names both in the XML (already done
    // in stripUnsupportedElements) AND at the OSMD model level.
    //
    // OSMD caches abbreviation text from the XML during load() and uses those
    // cached strings to compute the label-column width before render() runs.
    // Overriding the private abbreviationStr field here forces the render to
    // use the full name width for the label column on every system.
    const parts: unknown[] = (this.osmd as any).Sheet?.Parts ?? []
    for (const part of parts) {
      try {
        const fullName: string = (part as any).nameStr ?? (part as any).Name ?? ''
        if (fullName) (part as any).abbreviationStr = fullName
      } catch { /* ignore */ }
    }
    this.osmd.zoom = this.zoom
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    if (typeof (this.osmd as any).setOptions === 'function') {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      ;(this.osmd as any).setOptions({ zoom: this.zoom })
    }
    console.log('[ScoreViewer] zoom set to', this.osmd.zoom)

    // Ensure part names AND abbreviations are both rendered.
    // We rewrote the abbreviations in the XML to equal the full names, so all
    // systems will show the same label text.  Set EngravingRules explicitly so
    // OSMD doesn't fall back to its defaults and suppress either.
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const rules = (this.osmd as any).EngravingRules
    if (rules) {
      rules.RenderPartNames = true
      rules.RenderPartAbbreviations = true
    }

    // The CSS base rule has height:0; overflow:hidden on this container.
    // We must override BOTH before calling osmd.render() for two reasons:
    //  1. OSMD reads container.offsetWidth to determine page width — that
    //     works fine because width is never constrained.
    //  2. After render, we call getBoundingClientRect() on SVG child elements
    //     to detect system row positions.  With height:0 + overflow:hidden
    //     the browser clips everything to 0px, making all measurements 0.
    //     Setting height:auto allows the container to expand to the SVG's
    //     intrinsic height so measurements return real pixel values.
    this.container.style.height = 'auto'
    this.container.style.overflow = 'visible'

    await this.osmd.render()
    // One rAF so the browser has committed layout before we measure.
    await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()))

    try {
      const cursor = this.osmd?.cursor
      if (!cursor) {
        return
      }

      // OSMD may otherwise skip hidden/rest-derived positions while walking the
      // cursor, which truncates our time map on scores whose upper staves go
      // silent before lower staves do.
      cursor.SkipInvisibleNotes = false

      // buildTimeMap also builds systemMap from cursor element pixel positions.
      // height:auto + overflow:visible (set above) must be in effect here so
      // cursor.cursorElement.getBoundingClientRect() returns real values.
      this.buildTimeMap()

      // Constrain the container to one system row.  height and scrollTop are
      // managed per-flip inside snapToSystem(), which sizes the container to
      // the current system's exact page height.  We just need overflow:hidden
      // here so the full SVG doesn't show before the first snap.
      this.container.style.overflow = 'hidden'

      this.osmd.cursor.show()
      this.snapToSystem(0)
      this.attachClickHandler()
    } catch (cursorErr) {
      console.warn('[ScoreViewer] cursor init failed (non-fatal):', cursorErr)
      this.container.style.overflow = 'hidden'
    }
  }

  /**
   * Derive system row positions from the cursor element's pixel coordinates.
   *
   * We walk the cursor without calling cursor.hide() because hide() sets
   * display:none, which makes getBoundingClientRect() return all zeros.
   * The container still has CSS visibility:hidden (class .score-container,
   * not yet .loaded) so there is no visible flicker during the walk.
   *
   * The OSMD cursor element spans the full height of the system it sits in
   * (top staff to bottom staff, across all instruments).  Clustering the
   * per-step top-Y values therefore gives us exact system top + height.
   */
  private buildTimeMap(): void {
    const cursor = this.osmd?.cursor
    if (!cursor) return

    const svgEl = this.container.querySelector('svg')
    if (!svgEl) return
    const svgRect = svgEl.getBoundingClientRect()
    const svgH     = svgRect.height
    console.log(`[ScoreViewer] buildTimeMap: svgH=${svgH}, containerH=${this.container.getBoundingClientRect().height}`)

    const measureTempo = this.measureTempoMap
    const map: TimeEntry[] = []
    const geoms: Array<{ top: number; height: number }> = []
    this.resetCursorIterator(cursor)
    const iterator = cursor.iterator
    if (!iterator) return

    const containerRect = this.container.getBoundingClientRect()
    let step = 0
    // Accumulate seconds via deltas rather than using RealValue directly.
    // RealValue is the musical timestamp from the start of the score; on a
    // repeat jump it resets to the earlier value, making the timeline
    // non-monotonic.  Accumulating deltas keeps the timeline strictly
    // increasing across repeats (one note = one forward step in time).
    let cumulativeSeconds = 0

    while (!iterator.EndReached) {
      cursor.update()

      const enrolledValue = this.getIteratorTimestamp(iterator)
      const measureIdx: number = iterator.CurrentMeasureIndex ?? 0
      const relativeInMeasureWholeNotes = this.getIteratorRelativeInMeasureTimestamp(iterator, enrolledValue)
      const tempo = measureTempo.get(measureIdx) ?? measureTempo.get(0) ?? {
        bpm: 120,
        beatUnitWholeNotes: 0.25,
      }
      const stepWholeNotes = this.getBeatDuration(cursor)

      map.push({
        seconds: cumulativeSeconds,
        step,
        measureIndex: measureIdx,
        barNumber: measureIdx + 1,
        beatNumber: this.getBeatNumber(relativeInMeasureWholeNotes),
        wholeNotes: enrolledValue,
        relativeInMeasureWholeNotes,
        xPx: -1,
        topPx: -1,
      })

      try {
        const el = cursor.cursorElement as HTMLElement | null
        if (el) {
          const r = el.getBoundingClientRect()
          const top = r.top - containerRect.top + this.container.scrollTop
          const left = r.left - svgRect.left
          geoms.push({ top, height: r.height })
          map[map.length - 1].xPx = left
          map[map.length - 1].topPx = top
        } else {
          geoms.push({ top: -1, height: 0 })
        }
      } catch {
        geoms.push({ top: -1, height: 0 })
      }

      iterator.moveToNext()
      step++

      if (!iterator.EndReached) {
        const nextEnrolledValue = this.getIteratorTimestamp(iterator)
        const nextMeasureIdx: number = iterator.CurrentMeasureIndex ?? measureIdx
        const delta = nextEnrolledValue - enrolledValue
        if (delta > 0) {
          // Normal forward step: use the actual inter-step delta for accuracy.
          cumulativeSeconds += this.wholeNotesToSeconds(delta, tempo)
        } else {
          // Repeat jump (or D.C./D.S.): nextRealValue went backward or stayed
          // the same.  Use the note's own duration so the timeline advances
          // by exactly one beat.
          cumulativeSeconds += this.wholeNotesToSeconds(stepWholeNotes, tempo)
        }
        cumulativeSeconds += this.getFermataAdjustmentSecondsForTransition(
          measureIdx,
          nextMeasureIdx,
          tempo,
        )
      }
    }

    this.timeMap = map
    this.currentStep = 0
    this.updateCountInInfoFromPickup()
    this.resetCursorIterator(cursor)

    console.log(`[ScoreViewer] walked ${geoms.length} steps (incl. repeats); svgH=${svgH}px`)
    this.buildSystemMapFromGeoms(geoms, svgH)
  }

  private getIteratorTimestamp(iterator: any): number {
    return (
      iterator?.CurrentEnrolledTimestamp?.RealValue ??
      iterator?.CurrentSourceTimestamp?.RealValue ??
      iterator?.currentTimeStamp?.RealValue ??
      0
    )
  }

  private getIteratorRelativeInMeasureTimestamp(iterator: any, enrolledValue: number): number {
    return (
      iterator?.CurrentRelativeInMeasureTimestamp?.RealValue ??
      iterator?.currentRelativeInMeasureTimestamp?.RealValue ??
      enrolledValue
    )
  }

  private resetCursorIterator(cursor: OSMD): void {
    // Try OSMD's public reset() first — it creates a fresh iterator internally
    // and repositions the cursor visually at step 0.
    let resetViaPublicApi = false
    try {
      if (typeof cursor.reset === 'function') {
        cursor.reset()
        resetViaPublicApi = true
      }
    } catch { /* ignore */ }

    if (!resetViaPublicApi) {
      // Fallback: replace the cursor's iterator with a freshly created one.
      const manager = this.osmd?.Sheet?.MusicPartManager
      const iterator = manager?.getIterator?.()
      if (iterator) {
        iterator.SkipInvisibleNotes = false
        cursor.iterator = iterator
      }
      // cursor.update() may hide the cursor if the iterator is still at
      // EndReached (OSMD auto-hides on end), so we call show() after.
      cursor.update()
    }

    // Always ensure the cursor is visible regardless of which path was taken.
    cursor.show()
    this.currentStep = 0
    this.currentSystemIdx = -1
  }

  /**
   * Return the duration in whole notes of the beat currently under the cursor,
   * derived from the shortest note in any voice entry at this position.
   * Falls back to a quarter note (0.25) if nothing else is available.
   */
  private getBeatDuration(cursor: OSMD): number {
    try {
      const entries: unknown[] = cursor.VoicesUnderCursor?.() ?? []
      let min = Infinity
      for (const entry of entries as Array<{ Notes?: Array<{ Length?: { RealValue?: number } }> }>) {
        for (const note of entry.Notes ?? []) {
          const len = note.Length?.RealValue ?? 0
          if (len > 0 && len < min) min = len
        }
      }
      return Number.isFinite(min) ? min : 0.25
    } catch {
      return 0.25
    }
  }

  private buildSystemMapFromGeoms(
    geoms: Array<{ top: number; height: number }>,
    svgH: number
  ): void {
    this.systemMap = []
    const valid = geoms.filter((g) => g.top >= 0 && g.height > 0)

    if (valid.length === 0) {
      if (svgH > 0) this.systemMap = [{ topPx: 0, heightPx: svgH }]
      console.warn('[ScoreViewer] no cursor geometry — single system fallback')
      return
    }

    // Cluster steps by cursor top-Y.
    type Cluster = { top: number; bottom: number }
    const clusters: Cluster[] = []
    for (const g of valid) {
      const last = clusters[clusters.length - 1]
      if (!last || g.top - last.top > 30) {
        clusters.push({ top: g.top, bottom: g.top + g.height })
      } else {
        last.bottom = Math.max(last.bottom, g.top + g.height)
      }
    }

    // MARGIN: px above the cursor top to include when scrolling to a system.
    // Needed because clef symbols, time signatures, and top staff lines render
    // slightly above where the cursor element starts.
    const MARGIN = 16

    this.systemMap = clusters.map((c, i) => {
      // System 0: always show from y=0 so OSMD's top margin and first stave
      // lines are never clipped.
      // System N>0: scroll to MARGIN px above the cursor.
      const topPx = i === 0 ? 0 : Math.max(0, c.top - MARGIN)

      // Page height = distance from THIS system's topPx to the NEXT system's
      // topPx.  This guarantees the viewport stops exactly where the next page
      // starts — no bleed, regardless of system height variation.
      const nextTopPx = i < clusters.length - 1
        ? Math.max(0, clusters[i + 1].top - MARGIN)
        : svgH
      const heightPx = nextTopPx - topPx

      return { topPx, heightPx }
    })
    console.log('[ScoreViewer] systemMap:', this.systemMap.map((s, i) =>
      `[${i}] top=${s.topPx} h=${s.heightPx}`))
  }

  private snapToSystem(idx: number): void {
    if (this.systemMap.length === 0) return
    const i = Math.max(0, Math.min(idx, this.systemMap.length - 1))
    if (i === this.currentSystemIdx) return
    this.currentSystemIdx = i

    // Resize the container to this system's exact page height.
    // No padding offset needed — .score-container.loaded has no padding,
    // so border-box height == visible viewport (minus 2px for 1px top+bottom
    // border, which is imperceptible).
    const h = this.systemMap[i].heightPx
    if (h > 0) this.container.style.height = `${h}px`

    // Instant snap — no smooth-scroll so it feels like a page flip.
    this.container.scrollTop = this.systemMap[i].topPx

    this.onSystemChange?.({
      index: i,
      topPx: this.systemMap[i].topPx,
      heightPx: this.systemMap[i].heightPx,
    })
  }



  private toAnnotationAnchor(entry: TimeEntry): ScoreAnnotationAnchor {
    return {
      step: entry.step,
      seconds: entry.seconds,
      measureIndex: entry.measureIndex,
      systemIndex: this.findSystemIndexForTop(entry.topPx),
      xPx: entry.xPx,
      topPx: entry.topPx,
      lineTopPx: null,
    }
  }

  private findSystemIndexForTop(topPx: number): number {
    if (this.systemMap.length === 0) return 0

    let systemIndex = 0
    for (let index = 0; index < this.systemMap.length; index++) {
      if (this.systemMap[index].topPx <= topPx + 4) {
        systemIndex = index
      }
    }

    return systemIndex
  }

  getAnnotationPositionLabel(step: number, seconds?: number): string | null {
    const entry = this.timeMap[step] ?? (typeof seconds === 'number' ? this.getEntryForSeconds(seconds) : undefined)
    if (!entry) return null

    return `Bar ${entry.barNumber} · Beat ${entry.beatNumber}`
  }

  private getBeatNumber(relativeInMeasureWholeNotes: number): number {
    const beatsPerBar = Math.max(1, Math.round(this.meterInfo.beatsPerBar))
    const beatUnitWholeNotes = this.meterInfo.beatUnitWholeNotes > 0 ? this.meterInfo.beatUnitWholeNotes : 0.25
    const offsetWholeNotes = Math.max(0, relativeInMeasureWholeNotes)
    return Math.max(1, Math.min(beatsPerBar, Math.floor(offsetWholeNotes / beatUnitWholeNotes + 1e-6) + 1))
  }

  private getAnnotationInstrumentName(entry: TimeEntry, clientY: number): string | null {
    const clickedSystem = this.findSystemIndexForTop(entry.topPx)
    const system = this.systemMap[clickedSystem]
    const containerRect = this.container.getBoundingClientRect()
    const clickYInContainer = clientY - containerRect.top + this.container.scrollTop
    const svgEl = this.container.querySelector('svg')
    const rows = this.getVisibleInstrumentRows(clickedSystem)
    if (rows.length > 0) {
      const nearestRow = rows.reduce((bestRow, row) => {
        if (!bestRow) return row

        const rowCenter = Number.isFinite(row.centerPx)
          ? row.centerPx
          : system
            ? system.topPx + row.centerRatio * system.heightPx
            : row.centerRatio
        const bestCenter = Number.isFinite(bestRow.centerPx)
          ? bestRow.centerPx
          : system
            ? system.topPx + bestRow.centerRatio * system.heightPx
            : bestRow.centerRatio

        return Math.abs(rowCenter - clickYInContainer) < Math.abs(bestCenter - clickYInContainer)
          ? row
          : bestRow
      }, null as ScoreVisibleInstrumentRow | null)
      if (nearestRow) {
        return nearestRow.instrumentName
      }
    }

    if (svgEl) {
      const svgRect = svgEl.getBoundingClientRect()
      const clickY = clientY - svgRect.top
      const noteX = entry.xPx
      const candidates: Array<{ text: string; score: number }> = []

      for (const textNode of Array.from(svgEl.querySelectorAll('text'))) {
        const text = (textNode.textContent ?? '').replace(/\s+/g, ' ').trim()
        if (!text || !/[A-Za-z]/.test(text)) continue

        const rect = textNode.getBoundingClientRect()
        if (rect.width <= 0 || rect.height <= 0) continue

        const x = rect.left - svgRect.left
        if (x > Math.max(24, noteX - 8) || x > 220) continue

        const y = rect.top - svgRect.top + rect.height / 2
        const score = Math.abs(y - clickY) + x * 0.02
        if (score > 120) continue

        candidates.push({ text, score })
      }

      candidates.sort((leftCandidate, rightCandidate) => leftCandidate.score - rightCandidate.score)
      if (candidates[0]) {
        return candidates[0].text
      }
    }

    const voices = this.osmd?.cursor?.VoicesUnderCursor?.() ?? []
    if (Array.isArray(voices)) {
      for (const voice of voices) {
        const label = this.findLabelValue(voice)
        if (label) {
          return label
        }
      }
    }

    return this.getFallbackInstrumentName()
  }

  private getAnnotationSystemYRatio(entry: TimeEntry, clientY: number): number | null {
    const clickedSystem = this.findSystemIndexForTop(entry.topPx)
    const system = this.systemMap[clickedSystem]
    if (!system || !(system.heightPx > 0)) {
      return null
    }

    const containerRect = this.container.getBoundingClientRect()
    const clickYInContainer = clientY - containerRect.top + this.container.scrollTop
    const ratio = (clickYInContainer - system.topPx) / system.heightPx
    if (!Number.isFinite(ratio)) {
      return null
    }

    return Math.max(0, Math.min(1, ratio))
  }

  private getFallbackInstrumentName(): string | null {
    const parts: unknown[] = this.osmd?.Sheet?.Parts ?? []
    for (const part of parts) {
      const label = this.findLabelValue(part)
      if (label) {
        return label
      }
    }

    return null
  }

  private normalizeLabel(value: string): string {
    return value.replace(/\s+/g, ' ').trim().toLowerCase()
  }

  private getSvgViewportMetrics(): { topPx: number; scaleY: number; viewBoxY: number } | null {
    const svgEl = this.container.querySelector('svg') as SVGSVGElement | null
    if (!svgEl) {
      return null
    }

    const svgRect = svgEl.getBoundingClientRect()
    if (!(svgRect.height > 0)) {
      return null
    }

    const containerRect = this.container.getBoundingClientRect()
    const viewBox = svgEl.viewBox?.baseVal
    const viewBoxHeight = Number(viewBox?.height)

    return {
      topPx: svgRect.top - containerRect.top + this.container.scrollTop,
      scaleY: Number.isFinite(viewBoxHeight) && viewBoxHeight > 0 ? svgRect.height / viewBoxHeight : 1,
      viewBoxY: Number(viewBox?.y) || 0,
    }
  }

  private mapOsmdYToContainerPx(
    y: number,
    metrics: { topPx: number; scaleY: number; viewBoxY: number } | null,
  ): number {
    if (!metrics || !Number.isFinite(y)) {
      return Number.NaN
    }

    return metrics.topPx + (y - metrics.viewBoxY) * metrics.scaleY
  }

  private getMusicSystems(): unknown[] {
    const pages: unknown[] = this.osmd?.GraphicSheet?.MusicPages ?? []
    return pages.flatMap((page) => ((page as { MusicSystems?: unknown[] }).MusicSystems ?? []))
  }

  private getMusicSystemByIndex(systemIndex: number): Record<string, unknown> | null {
    if (!Number.isInteger(systemIndex) || systemIndex < 0) {
      return null
    }

    const systems = this.getMusicSystems()
    const system = systems[systemIndex]
    return system && typeof system === 'object' ? (system as Record<string, unknown>) : null
  }

  private getKnownInstrumentNames(): Array<{ displayName: string; normalizedName: string }> {
    const parts: unknown[] = this.osmd?.Sheet?.Parts ?? []
    const known = new Map<string, string>()

    for (const part of parts) {
      const label = this.findLabelValue(part)
      if (!label) {
        continue
      }

      const normalizedLabel = this.normalizeLabel(label)
      if (!normalizedLabel || known.has(normalizedLabel)) {
        continue
      }

      known.set(normalizedLabel, label.trim())
    }

    return [...known.entries()].map(([normalizedName, displayName]) => ({
      displayName,
      normalizedName,
    }))
  }

  private resolveKnownInstrumentLabel(label: string): string | null {
    const normalizedLabel = this.normalizeLabel(label)
    if (!normalizedLabel) {
      return null
    }

    for (const instrument of this.getKnownInstrumentNames()) {
      if (
        normalizedLabel === instrument.normalizedName
        || normalizedLabel.includes(instrument.normalizedName)
        || instrument.normalizedName.includes(normalizedLabel)
      ) {
        return instrument.displayName
      }
    }

    return null
  }

  private getVisibleInstrumentRowsFromSvgText(systemIndex: number): ScoreVisibleInstrumentRow[] {
    const svgEl = this.container.querySelector('svg') as SVGSVGElement | null
    const system = this.systemMap[systemIndex]
    if (!svgEl || !system) {
      return []
    }

    const svgRect = svgEl.getBoundingClientRect()
    const containerRect = this.container.getBoundingClientRect()
    const systemTopPx = system.topPx
    const systemBottomPx = system.topPx + system.heightPx

    const systemEntries = this.timeMap.filter((entry) => (
      entry.xPx >= 0
      && entry.topPx >= 0
      && this.findSystemIndexForTop(entry.topPx) === systemIndex
    ))
    const noteStartX = systemEntries.length > 0
      ? Math.min(...systemEntries.map((entry) => entry.xPx))
      : Number.POSITIVE_INFINITY
    const maxLabelRightX = Number.isFinite(noteStartX)
      ? Math.max(240, noteStartX + 8)
      : 280

    const candidates = new Map<string, {
      instrumentName: string
      labelCenterPx: number
      leftPx: number
      topPx: number
      bottomPx: number
    }>()

    for (const textNode of Array.from(svgEl.querySelectorAll('text'))) {
      const rawText = (textNode.textContent ?? '').replace(/\s+/g, ' ').trim()
      if (!rawText) {
        continue
      }

      const instrumentName = this.resolveKnownInstrumentLabel(rawText)
      if (!instrumentName) {
        continue
      }

      const rect = textNode.getBoundingClientRect()
      if (rect.width <= 0 || rect.height <= 0) {
        continue
      }

      const leftPx = rect.left - svgRect.left
      const rightPx = rect.right - svgRect.left
      if (leftPx < 0 || rightPx > maxLabelRightX) {
        continue
      }

      const topPx = rect.top - containerRect.top + this.container.scrollTop
      const bottomPx = rect.bottom - containerRect.top + this.container.scrollTop
      const labelCenterPx = (topPx + bottomPx) / 2
      if (labelCenterPx < systemTopPx - 24 || labelCenterPx > systemBottomPx + 24) {
        continue
      }

      const key = this.normalizeLabel(instrumentName)
      const current = candidates.get(key)
      if (!current || leftPx < current.leftPx) {
        candidates.set(key, {
          instrumentName,
          labelCenterPx,
          leftPx,
          topPx,
          bottomPx,
        })
      }
    }

    const sorted = [...candidates.values()].sort((leftRow, rightRow) => (
      leftRow.labelCenterPx - rightRow.labelCenterPx || leftRow.leftPx - rightRow.leftPx
    ))
    if (sorted.length === 0) {
      return []
    }

    return sorted.map((row, index) => {
      const previous = sorted[index - 1]
      const next = sorted[index + 1]
      const topPx = index === 0
        ? systemTopPx
        : (previous.labelCenterPx + row.labelCenterPx) / 2
      const bottomPx = index === sorted.length - 1
        ? systemBottomPx
        : (row.labelCenterPx + next.labelCenterPx) / 2
      const hitCenterPx = (topPx + bottomPx) / 2
      const systemHeight = Math.max(1, system.heightPx)

      return {
        instrumentName: row.instrumentName,
        systemIndex,
        topRatio: Math.min(1, Math.max(0, (topPx - systemTopPx) / systemHeight)),
        centerRatio: Math.min(1, Math.max(0, (hitCenterPx - systemTopPx) / systemHeight)),
        bottomRatio: Math.min(1, Math.max(0, (bottomPx - systemTopPx) / systemHeight)),
        topPx,
        centerPx: hitCenterPx,
        bottomPx,
        anchorPx: row.labelCenterPx,
      }
    })
  }

  getVisibleInstrumentRows(systemIndex = this.currentSystemIdx >= 0 ? this.currentSystemIdx : 0): ScoreVisibleInstrumentRow[] {
    const renderedRows = this.getVisibleInstrumentRowsFromSvgText(systemIndex)
    if (renderedRows.length > 0) {
      return renderedRows
    }

    const system = this.getMusicSystemByIndex(systemIndex)
    const staffLines = Array.isArray(system?.StaffLines) ? system.StaffLines : []
    if (staffLines.length === 0) {
      return []
    }
    const svgMetrics = this.getSvgViewportMetrics()

    const instrumentBounds = new Map<string, {
      instrumentName: string
      order: number
      top: number
      bottom: number
    }>()

    for (let index = 0; index < staffLines.length; index += 1) {
      const staffLine = staffLines[index] as {
        PositionAndShape?: {
          AbsolutePosition?: { y?: number }
          BorderTop?: number
          BorderBottom?: number
        }
        ParentStaff?: {
          ParentInstrument?: { Name?: string }
        }
        StaffHeight?: number
      }

      const instrumentName = staffLine.ParentStaff?.ParentInstrument?.Name?.trim()
      if (!instrumentName) {
        continue
      }

      const absoluteY = Number(staffLine.PositionAndShape?.AbsolutePosition?.y)
      if (!Number.isFinite(absoluteY)) {
        continue
      }

      const rawStaffHeight = Number(staffLine.StaffHeight)
      const staffHeight = Number.isFinite(rawStaffHeight) && rawStaffHeight > 0
        ? rawStaffHeight
        : 4
      const borderTop = Number.isFinite(staffLine.PositionAndShape?.BorderTop)
        ? Number(staffLine.PositionAndShape?.BorderTop)
        : 0
      const borderBottom = Number.isFinite(staffLine.PositionAndShape?.BorderBottom)
        ? Number(staffLine.PositionAndShape?.BorderBottom)
        : staffHeight
      const top = Number(absoluteY) + Math.min(borderTop, borderBottom)
      const bottom = Number(absoluteY) + Math.max(borderTop, borderBottom, staffHeight)
      const key = this.normalizeLabel(instrumentName)
      const current = instrumentBounds.get(key)

      if (current) {
        current.top = Math.min(current.top, top)
        current.bottom = Math.max(current.bottom, bottom)
      } else {
        instrumentBounds.set(key, {
          instrumentName,
          order: index,
          top,
          bottom,
        })
      }
    }

    const bounds = [...instrumentBounds.values()].sort((leftRow, rightRow) => leftRow.order - rightRow.order)
    if (bounds.length === 0) {
      return []
    }

    const systemTop = Math.min(...bounds.map((row) => row.top))
    const systemBottom = Math.max(...bounds.map((row) => row.bottom))
    const systemHeight = Math.max(1, systemBottom - systemTop)

    return bounds.map((row) => {
      const center = (row.top + row.bottom) / 2
      const topPx = this.mapOsmdYToContainerPx(row.top, svgMetrics)
      const centerPx = this.mapOsmdYToContainerPx(center, svgMetrics)
      const bottomPx = this.mapOsmdYToContainerPx(row.bottom, svgMetrics)
      return {
        instrumentName: row.instrumentName,
        systemIndex,
        topRatio: Math.min(1, Math.max(0, (row.top - systemTop) / systemHeight)),
        centerRatio: Math.min(1, Math.max(0, (center - systemTop) / systemHeight)),
        bottomRatio: Math.min(1, Math.max(0, (row.bottom - systemTop) / systemHeight)),
        topPx,
        centerPx,
        bottomPx,
        anchorPx: centerPx,
      }
    })
  }

  private findVisibleInstrumentRow(systemIndex: number, instrumentName: string | null | undefined): ScoreVisibleInstrumentRow | null {
    if (!instrumentName) {
      return null
    }

    const wanted = this.normalizeLabel(instrumentName)
    if (!wanted) {
      return null
    }

    const rows = this.getVisibleInstrumentRows(systemIndex)
    return rows.find((row) => {
      const name = this.normalizeLabel(row.instrumentName)
      return name === wanted || name.includes(wanted) || wanted.includes(name)
    }) ?? null
  }

  private findLabelValue(
    value: unknown,
    depth = 0,
    seen = new WeakSet<object>(),
  ): string | null {
    if (typeof value === 'string') {
      const trimmed = value.trim()
      return trimmed.length > 0 ? trimmed : null
    }

    if (!value || typeof value !== 'object' || depth > 3) {
      return null
    }

    if (seen.has(value as object)) {
      return null
    }
    seen.add(value as object)

    if (Array.isArray(value)) {
      for (const item of value) {
        const label = this.findLabelValue(item, depth + 1, seen)
        if (label) {
          return label
        }
      }
      return null
    }

    const record = value as Record<string, unknown>
    const preferredKeys = [
      'instrumentName',
      'InstrumentName',
      'partName',
      'PartName',
      'staffName',
      'StaffName',
      'nameStr',
      'Name',
    ]
    for (const key of preferredKeys) {
      const rawValue = record[key]
      if (typeof rawValue === 'string') {
        const trimmed = rawValue.trim()
        if (trimmed.length > 0) {
          return trimmed
        }
      }
    }

    for (const key of Object.keys(record)) {
      if (!/(instrument|part|staff)/i.test(key)) continue
      const label = this.findLabelValue(record[key], depth + 1, seen)
      if (label) {
        return label
      }
    }

    for (const key of Object.keys(record)) {
      if (!/^name(?:Str)?$/i.test(key)) continue
      const label = this.findLabelValue(record[key], depth + 1, seen)
      if (label) {
        return label
      }
    }

    return null
  }

  private getEntryForSeconds(seconds: number): TimeEntry | undefined {
    if (!Number.isFinite(seconds) || this.timeMap.length === 0) {
      return undefined
    }

    let lo = 0
    let hi = this.timeMap.length - 1
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1
      if (this.timeMap[mid].seconds <= seconds) lo = mid
      else hi = mid - 1
    }

    return this.timeMap[lo]
  }

  private resolveClosestEntry(clientX: number, clientY: number): TimeEntry | null {
    if (this.timeMap.length === 0 || !this.osmd?.cursor) return null

    const svgEl = this.container.querySelector('svg')
    if (!svgEl) return null

    const svgRect = svgEl.getBoundingClientRect()
    const clickYInSvg = clientY - svgRect.top
    const clickXInSvg = clientX - svgRect.left

    let clickedSystem = 0
    for (let i = 0; i < this.systemMap.length; i++) {
      if (this.systemMap[i].topPx <= clickYInSvg + 4) clickedSystem = i
    }

    const inSystem = this.timeMap.filter((entry) => {
      if (entry.topPx < 0) return false

      if (this.systemMap.length === 0) {
        return true
      }

      let sysIdx = 0
      for (let i = 0; i < this.systemMap.length; i++) {
        if (this.systemMap[i].topPx <= entry.topPx + 4) sysIdx = i
      }
      return sysIdx === clickedSystem
    })

    const pool = inSystem.filter((entry) => entry.xPx >= 0)
    if (pool.length === 0) return null

    const sorted = [...pool].sort((a, b) => a.xPx - b.xPx || a.step - b.step)
    let lo = 0
    let hi = sorted.length - 1
    while (lo < hi) {
      const mid = (lo + hi) >> 1
      if (sorted[mid].xPx < clickXInSvg) lo = mid + 1
      else hi = mid
    }

    const right = sorted[lo]
    const left = lo > 0 ? sorted[lo - 1] : null
    if (!left) {
      return right
    }

    return Math.abs(right.xPx - clickXInSvg) < Math.abs(clickXInSvg - left.xPx)
      ? right
      : left
  }

  getAnnotationAnchor(step: number, seconds?: number): ScoreAnnotationAnchor | null {
    const entry = this.timeMap[step]
    if (entry && entry.xPx >= 0 && entry.topPx >= 0) {
      return this.toAnnotationAnchor(entry)
    }

    if (typeof seconds === 'number' && Number.isFinite(seconds)) {
      return this.getAnnotationAnchorFromSeconds(seconds)
    }

    return entry ? this.toAnnotationAnchor(entry) : null
  }

  getAnnotationAnchorFromSeconds(seconds: number): ScoreAnnotationAnchor | null {
    if (!Number.isFinite(seconds) || this.timeMap.length === 0) {
      return null
    }

    let lo = 0
    let hi = this.timeMap.length - 1
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1
      if (this.timeMap[mid].seconds <= seconds) lo = mid
      else hi = mid - 1
    }

    const entry = this.timeMap[lo]
    return entry ? this.toAnnotationAnchor(entry) : null
  }

  getAnnotationAnchorForPosition(
    barNumber: number,
    beatNumber: number,
    instrumentName?: string | null,
    systemYRatio?: number | null,
  ): ScoreAnnotationAnchor | null {
    if (!Number.isFinite(barNumber) || !Number.isFinite(beatNumber)) {
      return null
    }

    const matches = this.timeMap.filter((entry) => (
      entry.barNumber === barNumber
      && entry.beatNumber === beatNumber
      && entry.xPx >= 0
      && entry.topPx >= 0
    ))

    if (matches.length === 0) {
      return null
    }

    const entry = [...matches].sort((leftEntry, rightEntry) => {
      if (leftEntry.measureIndex !== rightEntry.measureIndex) {
        return leftEntry.measureIndex - rightEntry.measureIndex
      }
      if (leftEntry.wholeNotes !== rightEntry.wholeNotes) {
        return leftEntry.wholeNotes - rightEntry.wholeNotes
      }
      if (leftEntry.xPx !== rightEntry.xPx) {
        return leftEntry.xPx - rightEntry.xPx
      }
      return leftEntry.step - rightEntry.step
    })[0]

    const anchor = this.toAnnotationAnchor(entry)
    const system = this.systemMap[anchor.systemIndex]
    if (
      Number.isFinite(systemYRatio)
      && system
      && system.heightPx > 0
    ) {
      const clampedRatio = Math.max(0, Math.min(1, Number(systemYRatio)))
      anchor.lineTopPx = system.topPx + clampedRatio * system.heightPx
    } else {
      const row = this.findVisibleInstrumentRow(anchor.systemIndex, instrumentName)
      if (row) {
        anchor.lineTopPx = Number.isFinite(row.anchorPx)
          ? row.anchorPx
          : system
            ? system.topPx + row.centerRatio * system.heightPx
            : null
      }
    }

    return anchor
  }

  private buildMeasureTempoMap(xmlText: string): Map<number, TempoInfo> {
    const result = new Map<number, TempoInfo>()
    const defaultTempo = { bpm: 120, beatUnitWholeNotes: 0.25 }
    result.set(0, defaultTempo)
    this.initialTempo = defaultTempo
    try {
      const doc = new DOMParser().parseFromString(xmlText, 'application/xml')
      if (doc.querySelector('parsererror')) {
        return result
      }

      const part = doc.getElementsByTagName('part')[0]
      if (!part) return result

      let lastTempo = result.get(0) ?? { bpm: 120, beatUnitWholeNotes: 0.25 }
      const measures = Array.from(part.children).filter((node) => node.tagName === 'measure')
      for (let i = 0; i < measures.length; i++) {
        const tempo = this.extractTempoFromMeasure(measures[i] as Element, lastTempo)
        if (tempo) {
          lastTempo = tempo
          if (this.initialTempo === defaultTempo) {
            this.initialTempo = tempo
          }
        }
        result.set(i, lastTempo)
      }
    } catch {
      /* ignore */
    }
    return result
  }

  private buildMeasureFermataAdjustmentMap(xmlText: string): Map<number, FermataAdjustment> {
    const result = new Map<number, FermataAdjustment>()

    try {
      const doc = new DOMParser().parseFromString(xmlText, 'application/xml')
      if (doc.querySelector('parsererror')) {
        return result
      }

      for (const part of Array.from(doc.getElementsByTagName('part'))) {
        let divisions = 1
        const measures = Array.from(part.children).filter((node) => node.tagName === 'measure')

        for (let measureIndex = 0; measureIndex < measures.length; measureIndex += 1) {
          const measure = measures[measureIndex] as Element
          const parsedDivisions = Number(measure.querySelector('attributes > divisions')?.textContent?.trim())
          if (Number.isFinite(parsedDivisions) && parsedDivisions > 0) {
            divisions = parsedDivisions
          }

          let maxFermataNoteWholeNotes = 0
          for (const note of Array.from(measure.children).filter((node) => node.tagName === 'note') as Element[]) {
            if (!note.querySelector('notations fermata')) {
              continue
            }

            const durationDivisions = Number(note.querySelector('duration')?.textContent?.trim())
            if (!Number.isFinite(durationDivisions) || durationDivisions <= 0) {
              continue
            }

            maxFermataNoteWholeNotes = Math.max(
              maxFermataNoteWholeNotes,
              durationDivisions / (divisions * 4),
            )
          }

          if (maxFermataNoteWholeNotes > 0) {
            const existing = result.get(measureIndex)?.extraWholeNotes ?? 0
            result.set(measureIndex, {
              // MuseScore's normal fermata playback approximately doubles the
              // held note/rest. Add only the extra hold time to the OSMD map.
              extraWholeNotes: Math.max(existing, maxFermataNoteWholeNotes),
            })
          }
        }
      }
    } catch {
      /* ignore */
    }

    return result
  }

  getCountInInfo(): CountInInfo {
    return this.countInInfo
  }

  getScorePartNames(): string[] {
    const parts: unknown[] = this.osmd?.Sheet?.Parts ?? []
    const labels: string[] = []

    for (const part of parts) {
      const label = this.findLabelValue(part)
      if (label) {
        labels.push(label)
      }
    }

    return labels
  }

  private extractMeasureMetadata(xmlText: string): MeasureMetadata[] {
    const result: MeasureMetadata[] = []
    let currentMeter: MeterInfo = { beatsPerBar: 4, beatUnitWholeNotes: 0.25 }

    try {
      const doc = new DOMParser().parseFromString(xmlText, 'application/xml')
      if (doc.querySelector('parsererror')) {
        return result
      }

      const part = doc.getElementsByTagName('part')[0]
      if (!part) {
        return result
      }

      const measures = Array.from(part.children).filter((node) => node.tagName === 'measure')
      for (const measureNode of measures) {
        const measure = measureNode as Element
        const time = measure.querySelector('attributes > time')
        const beats = time?.querySelector('beats')?.textContent?.trim()
        const beatType = time?.querySelector('beat-type')?.textContent?.trim()
        const parsedBeats = beats ? Number(beats) : NaN
        const parsedBeatType = beatType ? Number(beatType) : NaN

        if (Number.isFinite(parsedBeats) && parsedBeats > 0) {
          currentMeter = {
            ...currentMeter,
            beatsPerBar: parsedBeats,
          }
        }
        if (Number.isFinite(parsedBeatType) && parsedBeatType > 0) {
          currentMeter = {
            ...currentMeter,
            beatUnitWholeNotes: 1 / parsedBeatType,
          }
        }

        result.push({
          meter: { ...currentMeter },
          implicit: measure.getAttribute('implicit') === 'yes' || measure.getAttribute('number') === '0',
        })
      }
    } catch {
      /* ignore */
    }

    return result
  }

  private extractMeterInfo(xmlText: string): MeterInfo {
    const result: MeterInfo = { beatsPerBar: 4, beatUnitWholeNotes: 0.25 }
    try {
      const doc = new DOMParser().parseFromString(xmlText, 'application/xml')
      if (doc.querySelector('parsererror')) {
        return result
      }

      const time = doc.querySelector('part measure time') ?? doc.querySelector('measure time')
      const beats = time?.querySelector('beats')?.textContent?.trim()
      const beatType = time?.querySelector('beat-type')?.textContent?.trim()

      const parsedBeats = beats ? Number(beats) : NaN
      const parsedBeatType = beatType ? Number(beatType) : NaN

      if (Number.isFinite(parsedBeats) && parsedBeats > 0) {
        result.beatsPerBar = parsedBeats
      }
      if (Number.isFinite(parsedBeatType) && parsedBeatType > 0) {
        result.beatUnitWholeNotes = 1 / parsedBeatType
      }
    } catch {
      // Keep the default 4/4 fallback when the XML structure is unusual.
    }
    return result
  }

  private extractTempoFromMeasure(measure: Element, fallback: TempoInfo): TempoInfo | null {
    let current = fallback
    let changed = false

    for (const child of Array.from(measure.children)) {
      if (child.tagName !== 'direction') {
        continue
      }

      const metronome = child.querySelector('direction-type metronome')
      if (metronome) {
        const perMinute = Number(metronome.querySelector('per-minute')?.textContent?.trim())
        const beatUnit = metronome.querySelector('beat-unit')?.textContent?.trim()
        if (Number.isFinite(perMinute) && perMinute > 0) {
          current = {
            bpm: perMinute,
            beatUnitWholeNotes: this.beatUnitToWholeNotes(
              beatUnit,
              metronome.querySelectorAll('beat-unit-dot').length,
            ) ?? fallback.beatUnitWholeNotes,
          }
          changed = true
          continue
        }
      }

      const soundTempo = Number(child.querySelector('sound')?.getAttribute('tempo'))
      if (Number.isFinite(soundTempo) && soundTempo > 0) {
        current = {
          bpm: soundTempo,
          beatUnitWholeNotes: current.beatUnitWholeNotes,
        }
        changed = true
      }
    }

    return changed ? current : null
  }

  private beatUnitToWholeNotes(
    beatUnit: string | null | undefined,
    dotCount: number,
  ): number | null {
    const base = (() => {
      switch ((beatUnit ?? '').trim().toLowerCase()) {
        case 'whole':
          return 1
        case 'half':
          return 0.5
        case 'quarter':
          return 0.25
        case 'eighth':
          return 0.125
        case '16th':
          return 0.0625
        case '32nd':
          return 0.03125
        case '64th':
          return 0.015625
        case '128th':
          return 0.0078125
        default:
          return null
      }
    })()

    if (base === null) {
      return null
    }

    return base * this.dotMultiplier(dotCount)
  }

  private dotMultiplier(dotCount: number): number {
    let multiplier = 1
    let addition = 0.5
    for (let i = 0; i < dotCount; i++) {
      multiplier += addition
      addition /= 2
    }
    return multiplier
  }

  private wholeNotesToSeconds(wholeNotes: number, tempo: TempoInfo): number {
    if (wholeNotes <= 0 || tempo.bpm <= 0 || tempo.beatUnitWholeNotes <= 0) {
      return 0
    }

    return (wholeNotes * 60) / (tempo.bpm * tempo.beatUnitWholeNotes)
  }

  private getFermataAdjustmentSecondsForTransition(
    measureIndex: number,
    nextMeasureIndex: number,
    tempo: TempoInfo,
  ): number {
    if (this.measureFermataAdjustments.size === 0 || nextMeasureIndex === measureIndex) {
      return 0
    }

    let extraWholeNotes = 0
    if (nextMeasureIndex > measureIndex) {
      for (let index = measureIndex; index < nextMeasureIndex; index += 1) {
        extraWholeNotes += this.measureFermataAdjustments.get(index)?.extraWholeNotes ?? 0
      }
    } else {
      extraWholeNotes = this.measureFermataAdjustments.get(measureIndex)?.extraWholeNotes ?? 0
    }

    return this.wholeNotesToSeconds(extraWholeNotes, tempo)
  }

  private getMeasureDurationWholeNotes(measureIndex: number): number | null {
    const firstEntry = this.timeMap.find((entry) => entry.measureIndex === measureIndex)
    if (!firstEntry) {
      return null
    }

    const nextMeasureEntry = this.timeMap.find((entry) => entry.measureIndex === measureIndex + 1)
    if (nextMeasureEntry) {
      const duration = nextMeasureEntry.wholeNotes - firstEntry.wholeNotes
      return duration > 0 ? duration : null
    }

    return null
  }

  private updateCountInInfoFromPickup(): void {
    const tolerance = 1e-4
    const firstMeasureMeta = this.measureMetadata[0]
    const secondMeasureMeta = this.measureMetadata[1]
    const defaultMeter = firstMeasureMeta?.meter ?? this.meterInfo
    const fullBarMeter = firstMeasureMeta?.implicit && secondMeasureMeta
      ? secondMeasureMeta.meter
      : defaultMeter
    const fullBarWholeNotes = fullBarMeter.beatsPerBar * fullBarMeter.beatUnitWholeNotes
    const firstMeasureWholeNotes = this.getMeasureDurationWholeNotes(0)
    const isPickup = (
      firstMeasureMeta?.implicit === true
      || (
        firstMeasureWholeNotes !== null
        && firstMeasureWholeNotes > 0
        && firstMeasureWholeNotes + tolerance < fullBarWholeNotes
      )
    )
    const leadInWholeNotes = isPickup && firstMeasureWholeNotes !== null
      ? Math.max(fullBarMeter.beatUnitWholeNotes, fullBarWholeNotes - firstMeasureWholeNotes)
      : fullBarWholeNotes
    const beatSeconds = this.wholeNotesToSeconds(fullBarMeter.beatUnitWholeNotes, this.initialTempo)
    const pickupBeats = isPickup && firstMeasureWholeNotes !== null
      ? Math.max(0, firstMeasureWholeNotes / Math.max(fullBarMeter.beatUnitWholeNotes, tolerance))
      : 0

    this.countInInfo = {
      bpm: this.initialTempo.bpm,
      beatsPerBar: Math.max(1, Math.round(fullBarMeter.beatsPerBar)),
      beatSeconds: beatSeconds > 0 ? beatSeconds : 0.5,
      leadInBeats: Math.max(
        1,
        Math.ceil(leadInWholeNotes / Math.max(fullBarMeter.beatUnitWholeNotes, tolerance) - tolerance),
      ),
      pickupBeats,
    }
  }

  seek(seconds: number): void {
    const cursor = this.osmd?.cursor
    if (!cursor || this.timeMap.length === 0) return

    if (seconds < 0) {
      this.resetCursorIterator(cursor)
      try {
        cursor.hide()
      } catch {
        /* ignore */
      }
      if (this.systemMap.length > 0) {
        this.snapToSystem(0)
      }
      return
    }

    // Binary search: highest entry with .seconds <= seconds
    let lo = 0, hi = this.timeMap.length - 1
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1
      if (this.timeMap[mid].seconds <= seconds) lo = mid; else hi = mid - 1
    }

    const targetStep = this.timeMap[lo].step
    if (targetStep === this.currentStep) {
      // Position unchanged — cursor may have been hidden (e.g. OSMD auto-hides
      // when EndReached), so re-show it without moving.
      cursor.show()
      return
    }

    if (targetStep < this.currentStep) {
      this.resetCursorIterator(cursor)
    }

    const iterator = cursor.iterator
    if (!iterator) {
      cursor.show()
      return
    }

    while (this.currentStep < targetStep && !iterator.EndReached) {
      iterator.moveToNext()
      this.currentStep++
      cursor.update()
    }

    // Ensure the cursor is visible after advancing — cursor.update() can hide
    // it when the iterator lands on an invisible/rest position in some OSMD builds.
    cursor.show()
    this.flipPageIfNeeded(cursor)
  }

  /**
   * Determine which system row the cursor is currently on by comparing the
   * cursor element's Y position against the system map, then snap to it.
   */
  private flipPageIfNeeded(cursor: OSMD): void {
    try {
      const el: HTMLElement | null = cursor.cursorElement
      if (!el || this.systemMap.length === 0) return

      const svgEl = this.container.querySelector('svg')
      if (!svgEl) return

      const svgRect = svgEl.getBoundingClientRect()
      const cursorRect = el.getBoundingClientRect()
      // Y of the cursor relative to the SVG top (same coordinate space as systemMap).
      const cursorY = cursorRect.top - svgRect.top

      // Find the system whose top is closest to (and not below) the cursor.
      let bestIdx = 0
      for (let i = 0; i < this.systemMap.length; i++) {
        if (this.systemMap[i].topPx <= cursorY + 4) bestIdx = i
      }

      this.snapToSystem(bestIdx)
    } catch { /* ignore */ }
  }

  /**
   * Attach a click listener on the score SVG.  A click is mapped to the
   * nearest time-map entry by comparing the click's X coordinate (as a
   * fraction of SVG width) to each step's cursor X position.
   *
   * Because the cursor element is a <div> overlay on top of the SVG, we
   * listen on the container for both; the SVG and the cursor <div> both
   * bubble up to it.  We convert the viewport-Y of the click back into
   * SVG coordinates to find which system was clicked, then pick the
   * time-map entry whose cursor Y is closest to that system's top, and
   * whose cursor X is closest to the click X.
   */
  private attachClickHandler(): void {
    if (this._clickHandler) {
      this.container.removeEventListener('click', this._clickHandler)
    }

    if (this._contextMenuHandler) {
      this.container.removeEventListener('contextmenu', this._contextMenuHandler)
    }

    this._clickHandler = (e: MouseEvent) => {
      const best = this.resolveClosestEntry(e.clientX, e.clientY)
      if (!best) return

      this.onClickSeek?.(best.seconds)
    }

    this._contextMenuHandler = (e: MouseEvent) => {
      e.preventDefault()
      const best = this.resolveClosestEntry(e.clientX, e.clientY)
      if (!best) return
      const positionLabel = this.getAnnotationPositionLabel(best.step, best.seconds) ?? `Bar ${best.step + 1}`
      const instrumentName = this.getAnnotationInstrumentName(best, e.clientY)
      const systemYRatio = this.getAnnotationSystemYRatio(best, e.clientY)

      this.onAnnotationContextMenu?.({
        barNumber: best.barNumber,
        beatNumber: best.beatNumber,
        positionLabel,
        instrumentName,
        systemYRatio,
        clientX: e.clientX,
        clientY: e.clientY,
      })
    }

    this.container.addEventListener('click', this._clickHandler)
    this.container.addEventListener('contextmenu', this._contextMenuHandler)
  }

  reset(): void {
    if (this.osmd?.cursor) {
      this.resetCursorIterator(this.osmd.cursor)
    }
    if (this.systemMap.length > 0) {
      this.container.scrollTop = 0
    }
  }

  dispose(): void {
    if (this._clickHandler) {
      this.container.removeEventListener('click', this._clickHandler)
      this._clickHandler = null
    }
    if (this._contextMenuHandler) {
      this.container.removeEventListener('contextmenu', this._contextMenuHandler)
      this._contextMenuHandler = null
    }
    try { this.osmd?.cursor?.hide() } catch { /* ignore */ }
    this.osmd = null
    this.timeMap = []
    this.systemMap = []
    this.currentStep = 0
    this.container.innerHTML = ''
  }
}

/**
 * Remove MusicXML elements that OSMD 1.9.x doesn't support, using plain
 * string replacement on the original text.
 *
 * IMPORTANT: do NOT use DOMParser + XMLSerializer here — XMLSerializer
 * injects xmlns namespace attributes into every element, which corrupts
 * OSMD's internal XML lookups and causes note pitches to parse as undefined,
 * producing "NoteEnum[FundamentalNote] is undefined" crashes.
 *
 * Elements removed:
 *   <for-part>  — MusicXML 4.0 part-linking blocks written by MuseScore 4
 *                 for transposing instruments (Bb clarinet, F horn, etc.).
 *   <transpose> — older per-measure transposition hints.
 */
function stripUnsupportedElements(xmlText: string): string {
  // <for-part> — MusicXML 4.0 part-linking blocks (MuseScore 4 transposing instruments).
  // <transpose> — older per-measure transposition hints.
  // Both cause OSMD 1.9.x to crash with "NoteEnum[FundamentalNote] is undefined".
  let result = xmlText.replace(/<for-part[^>]*>[\s\S]*?<\/for-part>/g, '')
  result = result.replace(/<transpose[^>]*>[\s\S]*?<\/transpose>/g, '')

  // <display-step></display-step> — MuseScore 4 writes empty display-step inside
  // <unpitched> percussion notes (alongside bogus octave values like 1434).
  // OSMD tries NoteEnum[""] → undefined → .toLowerCase() → crash.
  // Replace with a valid placeholder note name so OSMD can at least parse them.
  result = result.replace(/<display-step><\/display-step>/g, '<display-step>B</display-step>')
  // Also clamp the absurd octave values MuseScore 4 writes for these same notes.
  result = result.replace(/<display-octave>(\d{3,})<\/display-octave>/g, '<display-octave>4</display-octave>')

  // Replace each <part-abbreviation> with the corresponding <part-name> so
  // OSMD renders the same full instrument name on every system row, not just
  // the first.  OSMD uses the abbreviation text for systems 2+; by making
  // abbreviations equal to full names, all pages look identical.
  // Also INSERT a <part-abbreviation> for parts that don't have one at all,
  // otherwise OSMD stores an empty string and computes a zero-width label
  // column for those parts on systems 2+.
  result = result.replace(
    /<score-part[^>]*>[\s\S]*?<\/score-part>/g,
    (scorePart) => {
      const nameMatch = scorePart.match(/<part-name[^>]*>([\s\S]*?)<\/part-name>/)
      if (!nameMatch) return scorePart
      const fullName = nameMatch[1]
      if (/<part-abbreviation/.test(scorePart)) {
        // Replace existing abbreviation with full name.
        return scorePart.replace(
          /<part-abbreviation[^>]*>[\s\S]*?<\/part-abbreviation>/g,
          `<part-abbreviation>${fullName}</part-abbreviation>`
        )
      } else {
        // No abbreviation element at all — insert one after </part-name>.
        return scorePart.replace(
          /(<\/part-name>)/,
          `$1<part-abbreviation>${fullName}</part-abbreviation>`
        )
      }
    }
  )

  const forPartCount = (xmlText.match(/<for-part[^>]*>/g) ?? []).length
  const transposeCount = (xmlText.match(/<transpose[^>]*>/g) ?? []).length
  const emptyStepCount = (xmlText.match(/<display-step><\/display-step>/g) ?? []).length
  console.log(`[ScoreViewer] stripped ${forPartCount} <for-part>, ${transposeCount} <transpose>, ${emptyStepCount} empty <display-step>`)

  return result
}
