<script lang="ts">
  type InstrumentStripProps = {
    name: string
    volume: number
    muted: boolean
    soloed?: boolean
    anySoloed?: boolean
    level?: number
    opacity?: number
    disabled?: boolean
    skeleton?: boolean
    skeletonIndex?: number
    showGauge?: boolean
    showMuteButton?: boolean
    highlight?: boolean
    onVolumeChange?: (volume: number) => void
    onMuteToggle?: () => void
    onSoloToggle?: () => void
  }

  const noopVolumeChange = (_volume: number) => {}
  const noopMuteToggle = () => {}
  const noopSoloToggle = () => {}

  let {
    name,
    volume,
    muted,
    soloed = false,
    anySoloed = false,
    level = 0,
    opacity = 1,
    disabled = false,
    skeleton = false,
    skeletonIndex = 0,
    showGauge = true,
    showMuteButton = true,
    highlight = false,
    onVolumeChange = noopVolumeChange,
    onMuteToggle = noopMuteToggle,
    onSoloToggle = noopSoloToggle,
  }: InstrumentStripProps = $props()

  let soloInactive = $derived(anySoloed && !soloed)

  let displayVolume = $derived(Math.round(volume * 100))
  let displayLevel = $derived(Math.round(level * 100))
  let sliderPercent = $derived(displayVolume / 2)

  function handleVolumeInput(event: Event) {
    const target = event.currentTarget as HTMLInputElement
    onVolumeChange(Number(target.value) / 100)
  }
</script>

<div
  class="channel-strip"
  class:muted={!skeleton && muted}
  class:solo-inactive={!skeleton && soloInactive}
  class:global-strip={highlight}
  class:skel-strip={skeleton}
  style:--skel-i={skeletonIndex}
  style:opacity
>
  <div class="channel-fader">
    {#if showGauge}
      <div class="channel-gauge" class:skel={skeleton} class:skel-gauge={skeleton}>
        {#if !skeleton}
          <div class="channel-gauge-fill" style="--l: {displayLevel}%"></div>
        {/if}
      </div>
    {/if}
    <div class="channel-slider-wrap" style:--slider-pct={sliderPercent}>
      {#if skeleton}
        <input
          class="channel-slider-input"
          type="range"
          min="0"
          max="200"
          value={displayVolume}
          disabled
          tabindex="-1"
          aria-hidden="true"
        />
      {:else}
        <input
          class="channel-slider-input"
          type="range"
          min="0"
          max="200"
          value={displayVolume}
          disabled={disabled || soloInactive}
          oninput={handleVolumeInput}
        />
      {/if}
      <span
        class="channel-slider-handle"
        class:is-disabled={disabled || skeleton}
        aria-hidden="true"
      >
        {skeleton ? '100%' : `${displayVolume}%`}
      </span>
    </div>
  </div>
  {#if showMuteButton}
    <div class="ms-btn-stack">
    {#if skeleton}
      <button
        class="mute-btn mute-btn-skeleton"
        type="button"
        disabled
        tabindex="-1"
        aria-hidden="true"
      >M</button>
      <button
        class="solo-btn solo-btn-skeleton"
        type="button"
        disabled
        tabindex="-1"
        aria-hidden="true"
      >S</button>
    {:else}
      <button
        class="mute-btn"
        type="button"
        disabled={disabled}
        class:active={muted}
        onclick={onMuteToggle}
      >M</button>
      <button
        class="solo-btn"
        type="button"
        disabled={disabled}
        class:active={soloed}
        onclick={onSoloToggle}
      >S</button>
    {/if}
    </div>
  {/if}
  <p class="channel-name">
    {#if skeleton}
      <span class="channel-name-skeleton" aria-hidden="true">
        {#each [0, 1, 2, 3, 4, 5] as dotIndex}
          <span class="channel-name-dot" style:--dot-i={dotIndex}>.</span>
        {/each}
      </span>
    {:else}
      {name}
    {/if}
  </p>
</div>
