<script lang="ts">
  import { onDestroy, onMount, tick } from 'svelte'
  import QRCode from 'qrcode'
  import QrScanner from 'qr-scanner'
  import qrWorkerUrl from 'qr-scanner/qr-scanner-worker.min?url'
  import {
    addUserToEnsemble,
    createDirectory,
    createEnsemble,
    createAdminUserLoginLink,
    createMyLoginLink,
    createUser,
    deleteMusic,
    exchangeLoginToken,
    fetchCurrentUser,
    fetchPublicMusic,
    fetchStems,
    fetchUserLibrary,
    grantDirectoryToEnsemble,
    listDirectories,
    listEnsembles,
    listMusics,
    listUsers,
    login,
    moveMusic,
    removeUserFromEnsemble,
    revokeDirectoryFromEnsemble,
    retryRender,
    STEM_QUALITY_PROFILES,
    updatePublicId,
    uploadMusic,
    type AdminMusic,
    type AppUser,
    type Directory,
    type Ensemble,
    type LoginLinkResponse,
    type PublicMusic,
    type StemQualityProfile,
    type UserLibraryDirectory,
  } from './lib/api'
  import { MidiMixerPlayer, type MixerTrack } from './lib/midi-player'
  import { StemMixerPlayer, type StemTrack } from './lib/stem-mixer'
  import { ScoreViewer } from './lib/score-viewer'
  import Mixer from './components/Mixer.svelte'

  ;(QrScanner as unknown as { WORKER_PATH: string }).WORKER_PATH = qrWorkerUrl

  type AppRoute =
    | { kind: 'user' }
    | { kind: 'admin' }
    | { kind: 'public'; accessKey: string }
    | { kind: 'connect'; token: string }

  type AdminSection = 'users' | 'ensembles' | 'directories' | 'scores'

  const adminSectionItems: Array<{
    id: AdminSection
    label: string
    eyebrow: string
  }> = [
    { id: 'users', label: 'Users', eyebrow: 'Accounts' },
    { id: 'ensembles', label: 'Ensembles', eyebrow: 'Groups' },
    { id: 'directories', label: 'Directories', eyebrow: 'Permissions' },
    { id: 'scores', label: 'Scores', eyebrow: 'Library' },
  ]

  const storedAdminPassword =
    typeof window !== 'undefined' ? window.localStorage.getItem('admin-password') ?? '' : ''
  const storedUserSessionToken =
    typeof window !== 'undefined' ? window.localStorage.getItem('user-session-token') ?? '' : ''

  let route = $state(resolveRoute(typeof window !== 'undefined' ? window.location.pathname : '/'))

  let adminPassword = $state(storedAdminPassword)
  let adminLoggedIn = $state(false)
  let adminSection = $state<AdminSection>('users')
  let adminLoading = $state(false)
  let adminError = $state('')
  let adminSuccess = $state('')
  let uploadTitle = $state('')
  let uploadPublicId = $state('')
  let uploadQualityProfile = $state<StemQualityProfile>('standard')
  let selectedFile = $state<File | null>(null)
  let uploadBusy = $state(false)
  let musics = $state<AdminMusic[]>([])
  let adminUsers = $state<AppUser[]>([])
  let ensembles = $state<Ensemble[]>([])
  let directories = $state<Directory[]>([])
  let newUsername = $state('')
  let creatingUser = $state(false)
  let newEnsembleName = $state('')
  let creatingEnsemble = $state(false)
  let newDirectoryName = $state('')
  let creatingDirectory = $state(false)
  let uploadDirectoryId = $state('')
  let editPublicIds = $state<Record<string, string>>({})
  let editDirectoryIds = $state<Record<string, string>>({})
  let savingIdFor = $state('')
  let movingMusicFor = $state('')
  let retryingFor = $state('')
  let deletingMusicFor = $state('')

  let userSessionToken = $state(storedUserSessionToken)
  let currentUser = $state<AppUser | null>(null)
  let userSessionExpiresAt = $state('')
  let userLoading = $state(false)
  let userError = $state('')
  let userSuccess = $state('')
  let userLibrary = $state<UserLibraryDirectory[]>([])
  let manualConnectionLink = $state('')
  let connectionBusy = $state(false)

  let credentialModalOpen = $state(false)
  let credentialModalTitle = $state('')
  let credentialLink = $state('')
  let credentialExpiresAt = $state('')
  let credentialQrDataUrl = $state('')

  let scannerOpen = $state(false)
  let scannerError = $state('')
  let scannerVideo = $state<HTMLVideoElement | null>(null)
  let qrScanner: QrScanner | null = null

  let publicMusic = $state<PublicMusic | null>(null)
  let publicLoading = $state(false)
  let publicError = $state('')
  let downloadMenuOpen = $state(false)
  let mixerRequested = $state(false)

  let scoreViewer = $state<ScoreViewer | null>(null)
  let scoreContainer = $state<HTMLElement | null>(null)
  let scoreLoading = $state(false)
  let scoreLoaded = $state(false)
  let scoreError = $state('')

  let midiPlayer = $state<MidiMixerPlayer | null>(null)
  let stemPlayer = $state<StemMixerPlayer | null>(null)
  let mixerTracks = $state<(MixerTrack | StemTrack)[]>([])
  let playerMode = $state<'stems' | 'midi' | null>(null)
  let midiLoading = $state(false)
  let midiPlayerError = $state('')
  let playbackState = $state<'stopped' | 'playing' | 'paused'>('stopped')
  let playbackPosition = $state(0)
  let playbackDuration = $state(0)
  let pct = $derived(playbackDuration > 0 ? (playbackPosition / playbackDuration) * 100 : 0)
  let playbackFrame = $state<number | null>(null)
  let globalVolume = $state(1.0)
  let trackLevels = $state<Record<string, number>>({})

  function currentAdminSectionItem() {
    return adminSectionItems.find((section) => section.id === adminSection) ?? adminSectionItems[0]
  }

  onMount(() => {
    const handlePopState = () => {
      route = resolveRoute(window.location.pathname)
      void syncRoute()
    }

    window.addEventListener('popstate', handlePopState)
    void syncRoute()

    return () => {
      window.removeEventListener('popstate', handlePopState)
    }
  })

  onDestroy(() => {
    closeScanner()
    stopPlaybackLoop()
    if (stemPlayer) {
      void stemPlayer.dispose()
      stemPlayer = null
    }
    if (midiPlayer) {
      void midiPlayer.dispose()
      midiPlayer = null
    }
    if (scoreViewer) {
      scoreViewer.dispose()
      scoreViewer = null
    }
  })

  function resolveRoute(pathname: string): AppRoute {
    const publicMatch = pathname.match(/^\/listen\/([^/]+)$/)
    if (publicMatch) {
      return { kind: 'public', accessKey: decodeURIComponent(publicMatch[1]) }
    }

    const connectMatch = pathname.match(/^\/connect\/([^/]+)$/)
    if (connectMatch) {
      return { kind: 'connect', token: decodeURIComponent(connectMatch[1]) }
    }

    if (pathname === '/admin') {
      return { kind: 'admin' }
    }

    return { kind: 'user' }
  }

  async function syncRoute() {
    downloadMenuOpen = false

    if (route.kind === 'public') {
      await loadPublicMusic(route.accessKey)
      return
    }

    if (route.kind === 'connect') {
      await completeConnectionFromToken(route.token, true)
      return
    }

    if (route.kind === 'admin') {
      if (adminPassword) {
        await tryAdminSession(adminPassword)
      } else {
        adminLoggedIn = false
      }
      return
    }

    if (userSessionToken) {
      await loadCurrentUser(userSessionToken)
    }
  }

  async function tryAdminSession(password: string) {
    adminLoading = true
    adminError = ''

    try {
      await login(password)
      adminLoggedIn = true
      window.localStorage.setItem('admin-password', password)
      await refreshAdminData(password)
    } catch (error) {
      adminLoggedIn = false
      adminError = error instanceof Error ? error.message : 'Unable to log in'
      window.localStorage.removeItem('admin-password')
    } finally {
      adminLoading = false
    }
  }

  async function handleLogin() {
    adminSuccess = ''
    await tryAdminSession(adminPassword)
  }

  async function refreshAdminData(password = adminPassword) {
    const [musicItems, userItems, ensembleItems, directoryItems] = await Promise.all([
      listMusics(password),
      listUsers(password),
      listEnsembles(password),
      listDirectories(password),
    ])
    musics = musicItems
    adminUsers = userItems
    ensembles = ensembleItems
    directories = directoryItems
    editPublicIds = Object.fromEntries(musicItems.map((music) => [music.id, music.public_id ?? '']))
    editDirectoryIds = Object.fromEntries(
      musicItems.map((music) => [music.id, music.directory_id ?? directoryItems[0]?.id ?? '']),
    )
    if (!uploadDirectoryId || !directoryItems.some((directory) => directory.id === uploadDirectoryId)) {
      uploadDirectoryId = directoryItems[0]?.id ?? ''
    }
  }

  async function handleCreateUser() {
    const trimmed = newUsername.trim()
    if (!trimmed) {
      adminError = 'Choose a username first.'
      return
    }

    creatingUser = true
    adminError = ''
    adminSuccess = ''

    try {
      const user = await createUser(adminPassword, trimmed)
      adminUsers = [...adminUsers, user].sort((left, right) =>
        left.username.localeCompare(right.username),
      )
      newUsername = ''
      adminSuccess = `User ${user.username} created.`
    } catch (error) {
      adminError = error instanceof Error ? error.message : 'Unable to create user'
    } finally {
      creatingUser = false
    }
  }

  async function handleCreateEnsemble() {
    const trimmed = newEnsembleName.trim()
    if (!trimmed) {
      adminError = 'Choose an ensemble name first.'
      return
    }

    creatingEnsemble = true
    adminError = ''
    adminSuccess = ''

    try {
      const ensemble = await createEnsemble(adminPassword, trimmed)
      ensembles = [...ensembles, ensemble].sort((left, right) => left.name.localeCompare(right.name))
      newEnsembleName = ''
      adminSuccess = `Ensemble ${ensemble.name} created.`
    } catch (error) {
      adminError = error instanceof Error ? error.message : 'Unable to create ensemble'
    } finally {
      creatingEnsemble = false
    }
  }

  async function handleCreateDirectory() {
    const trimmed = newDirectoryName.trim()
    if (!trimmed) {
      adminError = 'Choose a directory name first.'
      return
    }

    creatingDirectory = true
    adminError = ''
    adminSuccess = ''

    try {
      const directory = await createDirectory(adminPassword, trimmed)
      directories = [...directories, directory].sort((left, right) => left.name.localeCompare(right.name))
      newDirectoryName = ''
      if (!uploadDirectoryId) {
        uploadDirectoryId = directory.id
      }
      adminSuccess = `Directory ${directory.name} created.`
    } catch (error) {
      adminError = error instanceof Error ? error.message : 'Unable to create directory'
    } finally {
      creatingDirectory = false
    }
  }

  async function toggleUserEnsembleMembership(ensembleId: string, userId: string, shouldAdd: boolean) {
    adminError = ''
    adminSuccess = ''

    try {
      if (shouldAdd) {
        await addUserToEnsemble(adminPassword, ensembleId, userId)
      } else {
        await removeUserFromEnsemble(adminPassword, ensembleId, userId)
      }
      await refreshAdminData()
      adminSuccess = 'Membership updated.'
    } catch (error) {
      adminError = error instanceof Error ? error.message : 'Unable to update membership'
    }
  }

  async function toggleDirectoryGrant(directoryId: string, ensembleId: string, shouldGrant: boolean) {
    adminError = ''
    adminSuccess = ''

    try {
      if (shouldGrant) {
        await grantDirectoryToEnsemble(adminPassword, directoryId, ensembleId)
      } else {
        await revokeDirectoryFromEnsemble(adminPassword, directoryId, ensembleId)
      }
      await refreshAdminData()
      adminSuccess = 'Directory permissions updated.'
    } catch (error) {
      adminError = error instanceof Error ? error.message : 'Unable to update directory permission'
    }
  }

  async function handleUpload() {
    if (!selectedFile) {
      adminError = 'Choose an .mscz file first.'
      return
    }
    if (!uploadDirectoryId) {
      adminError = 'Choose a directory first.'
      return
    }

    uploadBusy = true
    adminError = ''
    adminSuccess = ''

    try {
      await uploadMusic(adminPassword, {
        file: selectedFile,
        title: uploadTitle,
        publicId: uploadPublicId,
        qualityProfile: uploadQualityProfile,
        directoryId: uploadDirectoryId,
      })

      uploadTitle = ''
      uploadPublicId = ''
      uploadQualityProfile = 'standard'
      selectedFile = null
      const input = document.getElementById('mscz-input') as HTMLInputElement | null
      if (input) {
        input.value = ''
      }

      await refreshAdminData()
      adminSuccess = 'Upload completed.'
    } catch (error) {
      adminError = error instanceof Error ? error.message : 'Upload failed'
    } finally {
      uploadBusy = false
    }
  }
  async function handleSavePublicId(musicId: string) {
    savingIdFor = musicId
    adminError = ''
    adminSuccess = ''

    try {
      const updated = await updatePublicId(adminPassword, musicId, editPublicIds[musicId] ?? '')
      musics = musics.map((music) => (music.id === musicId ? updated : music))
      editPublicIds = { ...editPublicIds, [musicId]: updated.public_id ?? '' }
      adminSuccess = 'Public id updated.'
    } catch (error) {
      adminError = error instanceof Error ? error.message : 'Unable to update public id'
    } finally {
      savingIdFor = ''
    }
  }

  async function handleMoveMusic(musicId: string) {
    const directoryId = editDirectoryIds[musicId] ?? ''
    if (!directoryId) {
      adminError = 'Choose a directory first.'
      return
    }

    movingMusicFor = musicId
    adminError = ''
    adminSuccess = ''

    try {
      const updated = await moveMusic(adminPassword, musicId, directoryId)
      musics = musics.map((music) => (music.id === musicId ? updated : music))
      editDirectoryIds = { ...editDirectoryIds, [musicId]: updated.directory_id }
      await refreshAdminData()
      adminSuccess = 'Score moved.'
    } catch (error) {
      adminError = error instanceof Error ? error.message : 'Unable to move score'
    } finally {
      movingMusicFor = ''
    }
  }

  async function handleDeleteMusic(musicId: string) {
    if (!window.confirm('Delete this score permanently?')) {
      return
    }

    deletingMusicFor = musicId
    adminError = ''
    adminSuccess = ''

    try {
      await deleteMusic(adminPassword, musicId)
      musics = musics.filter((music) => music.id !== musicId)
      delete editPublicIds[musicId]
      delete editDirectoryIds[musicId]
      await refreshAdminData()
      adminSuccess = 'Score deleted.'
    } catch (error) {
      adminError = error instanceof Error ? error.message : 'Unable to delete score'
    } finally {
      deletingMusicFor = ''
    }
  }

  async function handleRetryRender(musicId: string) {
    retryingFor = musicId
    adminError = ''
    adminSuccess = ''

    try {
      const updated = await retryRender(adminPassword, musicId)
      musics = musics.map((music) => (music.id === musicId ? updated : music))
      adminSuccess = 'Render retried successfully.'
    } catch (error) {
      adminError = error instanceof Error ? error.message : 'Retry failed'
    } finally {
      retryingFor = ''
    }
  }

  async function copyText(value: string, scope: 'admin' | 'user', successMessage: string) {
    await navigator.clipboard.writeText(value)
    if (scope === 'admin') {
      adminSuccess = successMessage
      adminError = ''
    } else {
      userSuccess = successMessage
      userError = ''
    }
  }

  async function showCredentialModal(title: string, linkResponse: LoginLinkResponse) {
    credentialModalTitle = title
    credentialLink = linkResponse.connection_url
    credentialExpiresAt = linkResponse.expires_at
    credentialQrDataUrl = await QRCode.toDataURL(linkResponse.connection_url, {
      width: 280,
      margin: 1,
      color: {
        dark: '#111111',
        light: '#0000',
      },
    })
    credentialModalOpen = true
  }

  async function handleAdminShowUserQr(user: AppUser) {
    adminError = ''
    adminSuccess = ''

    try {
      const response = await createAdminUserLoginLink(adminPassword, user.id)
      await showCredentialModal(`QR code for ${user.username}`, response)
      adminSuccess = `QR code ready for ${user.username}.`
    } catch (error) {
      adminError = error instanceof Error ? error.message : 'Unable to create QR code'
    }
  }

  async function handleAdminCopyUserLink(user: AppUser) {
    adminError = ''
    adminSuccess = ''

    try {
      const response = await createAdminUserLoginLink(adminPassword, user.id)
      await copyText(response.connection_url, 'admin', `Connection link copied for ${user.username}.`)
    } catch (error) {
      adminError = error instanceof Error ? error.message : 'Unable to create connection link'
    }
  }

  async function loadCurrentUser(authToken = userSessionToken) {
    if (!authToken) {
      clearUserSession()
      return
    }

    userLoading = true

    try {
      const response = await fetchCurrentUser(authToken)
      const library = await fetchUserLibrary(authToken)
      userSessionToken = authToken
      currentUser = response.user
      userSessionExpiresAt = response.session_expires_at
      userLibrary = library.directories
      userError = ''
    } catch (error) {
      clearUserSession()
      userError = error instanceof Error ? error.message : 'Unable to restore your session'
    } finally {
      userLoading = false
    }
  }

  function clearUserSession() {
    userSessionToken = ''
    currentUser = null
    userSessionExpiresAt = ''
    userLibrary = []
    window.localStorage.removeItem('user-session-token')
  }

  function persistUserSession(sessionToken: string, user: AppUser, expiresAt: string) {
    userSessionToken = sessionToken
    currentUser = user
    userSessionExpiresAt = expiresAt
    userLoading = false
    window.localStorage.setItem('user-session-token', sessionToken)
  }

  async function completeConnectionFromToken(token: string, fromRoute = false) {
    userLoading = true
    connectionBusy = true
    userError = ''
    userSuccess = ''

    try {
      const response = await exchangeLoginToken(token)
      persistUserSession(response.session_token, response.user, response.session_expires_at)
      manualConnectionLink = ''
      closeScanner()
      userSuccess = `Connected as ${response.user.username}.`
      if (fromRoute || route.kind !== 'user') {
        navigate('/', true)
      }
    } catch (error) {
      clearUserSession()
      userError = error instanceof Error ? error.message : 'Unable to use this connection link'
    } finally {
      connectionBusy = false
      userLoading = false
    }
  }

  async function handleManualConnect() {
    const token = extractConnectionToken(manualConnectionLink)
    if (!token) {
      userError = 'Paste a valid connection link from Fumen.'
      userSuccess = ''
      return
    }

    await completeConnectionFromToken(token)
  }

  function extractConnectionToken(value: string): string | null {
    const trimmed = value.trim()
    if (!trimmed) {
      return null
    }

    try {
      const parsed = new URL(trimmed, window.location.origin)
      const match = parsed.pathname.match(/^\/connect\/([^/]+)$/)
      if (match) {
        return decodeURIComponent(match[1])
      }
    } catch {
      // Ignore malformed URLs and try local patterns below.
    }

    const pathMatch = trimmed.match(/^\/connect\/([^/]+)$/)
    if (pathMatch) {
      return decodeURIComponent(pathMatch[1])
    }

    if (/^[a-zA-Z0-9]+$/.test(trimmed)) {
      return trimmed
    }

    return null
  }

  async function handleShowMyQr() {
    if (!userSessionToken || !currentUser) {
      userError = 'Sign in first.'
      return
    }

    userError = ''
    userSuccess = ''

    try {
      const response = await createMyLoginLink(userSessionToken)
      await showCredentialModal(`QR code for ${currentUser.username}`, response)
      userSuccess = 'QR code ready.'
    } catch (error) {
      userError = error instanceof Error ? error.message : 'Unable to create QR code'
    }
  }

  async function handleCopyMyLink() {
    if (!userSessionToken || !currentUser) {
      userError = 'Sign in first.'
      return
    }

    userError = ''
    userSuccess = ''

    try {
      const response = await createMyLoginLink(userSessionToken)
      await copyText(response.connection_url, 'user', 'Connection link copied to clipboard.')
    } catch (error) {
      userError = error instanceof Error ? error.message : 'Unable to create connection link'
    }
  }

  function logoutUser() {
    clearUserSession()
    userSuccess = ''
    userError = ''
  }

  function logoutAdmin() {
      adminLoggedIn = false
      adminSection = 'users'
      adminPassword = ''
    musics = []
    adminUsers = []
    ensembles = []
    directories = []
    editPublicIds = {}
    editDirectoryIds = {}
    adminSuccess = ''
    adminError = ''
    window.localStorage.removeItem('admin-password')
  }

  function navigate(pathname: string, replace = false) {
    if (typeof window === 'undefined') {
      return
    }

    const currentPath = window.location.pathname
    if (currentPath !== pathname) {
      const method = replace ? 'replaceState' : 'pushState'
      window.history[method]({}, '', pathname)
    }

    route = resolveRoute(pathname)
    void syncRoute()
  }

  async function openScanner() {
    scannerError = ''
    scannerOpen = true
    await tick()

    if (!scannerVideo) {
      scannerError = 'Camera preview is unavailable on this device.'
      return
    }

    try {
      qrScanner?.destroy()
      qrScanner = new QrScanner(
        scannerVideo,
        (result) => {
          const value = typeof result === 'string' ? result : result.data
          void handleScannedValue(value)
        },
        {
          highlightScanRegion: true,
          highlightCodeOutline: true,
        },
      )
      await qrScanner.start()
    } catch (error) {
      scannerError = error instanceof Error ? error.message : 'Unable to start the camera'
    }
  }

  async function handleScannedValue(value: string) {
    const token = extractConnectionToken(value)
    if (!token) {
      scannerError = 'That QR code is not a valid Fumen connection link.'
      return
    }

    closeScanner()
    await completeConnectionFromToken(token)
  }

  function closeScanner() {
    qrScanner?.stop()
    qrScanner?.destroy()
    qrScanner = null
    scannerOpen = false
  }

  function handleBackdropClick(event: MouseEvent, onClose: () => void) {
    if (event.target === event.currentTarget) {
      onClose()
    }
  }
  async function loadPublicMusic(accessKey: string) {
    publicLoading = true
    publicError = ''
    downloadMenuOpen = false

    try {
      const music = await fetchPublicMusic(accessKey)
      publicMusic = music
      publicLoading = false
      await tick()
      await resetMixers()
      mixerRequested = false

      let scoreTask: Promise<void> = Promise.resolve()
      if (music.musicxml_url && scoreContainer) {
        scoreLoading = true
        const sv = new ScoreViewer(scoreContainer)
        sv.onClickSeek = (seconds: number) => handleScoreSeek(seconds)
        scoreViewer = sv
        scoreTask = sv
          .load(music.musicxml_url)
          .then(() => {
            scoreLoaded = true
          })
          .catch((err: unknown) => {
            console.error('[ScoreViewer] load failed:', err)
            scoreError = err instanceof Error ? `${err.message}\n${err.stack ?? ''}` : String(err)
          })
          .finally(() => {
            scoreLoading = false
          })
      }

      await scoreTask

      mixerRequested = true
      if (music.stems_status === 'ready') {
        await loadStemMixer(accessKey)
      }
    } catch (error) {
      publicError = error instanceof Error ? error.message : 'Unable to load this score'
    } finally {
      publicLoading = false
    }
  }

  async function resetMixers() {
    stopPlaybackLoop()
    playbackState = 'stopped'
    playbackPosition = 0
    playbackDuration = 0
    globalVolume = 1.0
    mixerTracks = []
    playerMode = null
    midiLoading = false
    midiPlayerError = ''

    if (stemPlayer) {
      await stemPlayer.dispose()
      stemPlayer = null
    }
    if (midiPlayer) {
      await midiPlayer.dispose()
      midiPlayer = null
    }
    if (scoreViewer) {
      scoreViewer.dispose()
      scoreViewer = null
    }
    scoreLoading = false
    scoreLoaded = false
    scoreError = ''
    mixerRequested = false
  }

  async function loadStemMixer(accessKey: string) {
    midiLoading = true
    midiPlayerError = ''

    try {
      const stems = await fetchStems(accessKey)
      if (stems.length === 0) {
        midiPlayerError = 'No stems available for this score'
        return
      }

      stemPlayer = new StemMixerPlayer()
      const loaded = await stemPlayer.loadStems(
        stems.map((stem) => ({
          id: String(stem.track_index),
          name: stem.track_name,
          instrumentName: stem.instrument_name,
          fullStemUrl: stem.full_stem_url,
          durationSeconds: stem.duration_seconds,
        })),
      )
      stemPlayer.setLevelMultiplier(15)
      mixerTracks = loaded.tracks
      playbackDuration = loaded.duration
      playbackPosition = 0
      playbackState = 'stopped'
      playerMode = 'stems'
    } catch (error) {
      midiPlayerError = error instanceof Error ? error.message : 'Unable to prepare stem playback'
    } finally {
      midiLoading = false
    }
  }

  async function togglePlayback() {
    const player = stemPlayer ?? midiPlayer
    if (!player || playbackDuration <= 0) {
      return
    }

    try {
      if (playbackState === 'playing') {
        player.pause()
        playbackState = 'paused'
        playbackPosition = player.getCurrentTime()
        stopPlaybackLoop()
        return
      }

      if (playbackPosition >= playbackDuration - 0.01) {
        player.seek(0)
        playbackPosition = 0
      }

      await player.play()
      playbackState = 'playing'
      startPlaybackLoop()
    } catch (error) {
      midiPlayerError = error instanceof Error ? error.message : 'Unable to start playback'
    }
  }

  function stopPlayback() {
    const player = stemPlayer ?? midiPlayer
    if (!player) {
      return
    }

    player.stop()
    playbackState = 'stopped'
    playbackPosition = 0
    stopPlaybackLoop()
  }

  function handleSeek(event: Event) {
    const player = stemPlayer ?? midiPlayer
    if (!player) {
      return
    }

    const target = event.currentTarget as HTMLInputElement
    const seconds = Number(target.value)
    void handleScoreSeek(seconds)
  }

  async function handleScoreSeek(seconds: number) {
    scoreViewer?.seek(seconds)
    playbackPosition = seconds
    const player = stemPlayer ?? midiPlayer
    if (!player) return
    const wasPlaying = playbackState === 'playing'
    if (wasPlaying) {
      player.pause()
      stopPlaybackLoop()
    }
    player.seek(seconds)
    if (wasPlaying) {
      await player.play()
      startPlaybackLoop()
    }
  }

  function updateTrackVolume(trackId: string, volume: number) {
    mixerTracks = mixerTracks.map((track) => (track.id === trackId ? { ...track, volume } : track))
    if (stemPlayer && playerMode === 'stems') {
      stemPlayer.setTrackVolume(trackId, volume)
    } else if (midiPlayer && playerMode === 'midi') {
      midiPlayer.setTrackVolume(trackId, volume)
    }
  }

  function updateGlobalVolume(volume: number) {
    globalVolume = volume
    mixerTracks = mixerTracks.map((track) => ({ ...track, volume: globalVolume }))
    if (stemPlayer && playerMode === 'stems') {
      for (const track of mixerTracks) {
        stemPlayer.setTrackVolume(track.id, globalVolume)
      }
    } else if (midiPlayer && playerMode === 'midi') {
      for (const track of mixerTracks) {
        midiPlayer.setTrackVolume(track.id, globalVolume)
      }
    }
  }

  function toggleTrackMute(trackId: string) {
    mixerTracks = mixerTracks.map((track) => {
      if (track.id !== trackId) {
        return track
      }

      const muted = !track.muted
      if (stemPlayer && playerMode === 'stems') {
        stemPlayer.setTrackMuted(trackId, muted)
      } else if (midiPlayer && playerMode === 'midi') {
        midiPlayer.setTrackMuted(trackId, muted)
      }
      return { ...track, muted }
    })
  }

  function startPlaybackLoop() {
    stopPlaybackLoop()
    const tickPlayback = () => {
      const player = stemPlayer ?? midiPlayer
      if (!player) return
      playbackPosition = player.getCurrentTime()
      scoreViewer?.seek(playbackPosition)
      if (stemPlayer && playerMode === 'stems') {
        const levels: Record<string, number> = {}
        for (const track of mixerTracks) levels[track.id] = stemPlayer.getLevel(track.id)
        trackLevels = levels
      }
      if (playbackState === 'playing') {
        if (playbackDuration > 0 && playbackPosition >= playbackDuration - 0.03) {
          player.pause()
          player.seek(playbackDuration)
          playbackState = 'paused'
          playbackPosition = playbackDuration
          stopPlaybackLoop()
          return
        }
        playbackFrame = requestAnimationFrame(tickPlayback)
      }
    }
    playbackFrame = requestAnimationFrame(tickPlayback)
  }

  function stopPlaybackLoop() {
    if (playbackFrame !== null) {
      cancelAnimationFrame(playbackFrame)
      playbackFrame = null
    }
    trackLevels = {}
  }

  function handleAdminPasswordKeydown(event: KeyboardEvent) {
    if (event.key === 'Enter') void handleLogin()
  }

  function handleFileSelection(event: Event) {
    const target = event.currentTarget as HTMLInputElement
    selectedFile = target.files?.[0] ?? null
  }

  function prettyDate(value: string) {
    return new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'short' }).format(
      new Date(value),
    )
  }

  function formatBytes(bytes: number) {
    if (bytes === 0) return '—'
    if (bytes < 1024) return `${bytes} B`
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
  }

  function formatTime(seconds: number) {
    const safeSeconds = Math.max(0, Math.floor(seconds))
    const minutes = Math.floor(safeSeconds / 60)
    const remainingSeconds = safeSeconds % 60
    return `${minutes}:${remainingSeconds.toString().padStart(2, '0')}`
  }

  function qualityProfileLabel(profile: string) {
    return STEM_QUALITY_PROFILES.find((option) => option.value === profile)?.label ?? profile
  }
</script>

{#if route.kind === 'public'}
  <main class="page public-shell">
    <section class="content-panel">
      {#if publicLoading}
        <p class="status">Loading score...</p>
      {:else if publicError}
        <p class="status error">{publicError}</p>
      {:else if publicMusic}
        <div class="public-card">
          <div class="public-score-pane">
            <div class="score-scroll-area">
              <div class="score-title-row">
                <h2>{publicMusic.title}</h2>
                <div class="download-menu" class:open={downloadMenuOpen}>
                  <button class="download-menu-btn" onclick={() => (downloadMenuOpen = !downloadMenuOpen)} aria-haspopup="true" aria-expanded={downloadMenuOpen}>
                    <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3v12M7 11l5 5 5-5" /><path d="M4 20h16" /></svg>
                    Download
                    <svg class="chevron" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="6 9 12 15 18 9" /></svg>
                  </button>
                  {#if downloadMenuOpen}
                    <div class="download-dropdown">
                      {#if publicMusic.midi_download_url}
                        <a class="download-item" href={publicMusic.midi_download_url} download onclick={() => (downloadMenuOpen = false)}>Download MIDI</a>
                      {/if}
                      <a class="download-item" href={publicMusic.download_url} download onclick={() => (downloadMenuOpen = false)}>Download MuseScore</a>
                    </div>
                  {/if}
                </div>
              </div>
              <div class="meta-grid">
                <div><p class="meta-label">Filename</p><p>{publicMusic.filename}</p></div>
                <div><p class="meta-label">Uploaded</p><p>{prettyDate(publicMusic.created_at)}</p></div>
                <div><p class="meta-label">Instruments</p><p>{mixerTracks.length || 0}</p></div>
              </div>
              <div class="score-container" class:loaded={scoreLoaded} bind:this={scoreContainer}></div>
              {#if scoreLoading}
                <p class="status">Loading score...</p>
              {:else if scoreError}
                <p class="status error">Score: {scoreError}</p>
              {/if}
            </div>
            <div class="playbar" class:is-playing={playbackState === 'playing'}>
              <button class="playbar-btn playbar-play" onclick={() => void togglePlayback()} disabled={mixerTracks.length === 0} aria-label={playbackState === 'playing' ? 'Pause' : 'Play'}>
                {#if playbackState === 'playing'}
                  <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor"><rect x="5" y="4" width="4" height="16" rx="1.5" /><rect x="15" y="4" width="4" height="16" rx="1.5" /></svg>
                {:else}
                  <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor"><path d="M7 4.5 L7 19.5 L20 12 Z" /></svg>
                {/if}
              </button>
              <button class="playbar-btn playbar-stop" onclick={stopPlayback} disabled={mixerTracks.length === 0} aria-label="Stop"><svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><rect x="4" y="4" width="16" height="16" rx="2" /></svg></button>
              <div class="playbar-progress">
                <input class="playbar-track" type="range" min="0" max={playbackDuration || 0} step="0.01" value={playbackPosition} oninput={handleSeek} disabled={mixerTracks.length === 0} style="--pct: {pct}%" aria-label="Playback position" />
              </div>
              <span class="playbar-time">{formatTime(playbackPosition)}<span class="playbar-sep"> / </span>{formatTime(playbackDuration)}</span>
            </div>
          </div>
          <div class="public-mixer-pane">
            <Mixer {midiLoading} {mixerTracks} {mixerRequested} {globalVolume} {trackLevels} {midiPlayerError} stemsError={publicMusic.stems_error} onGlobalVolumeChange={updateGlobalVolume} onTrackVolumeChange={updateTrackVolume} onTrackMuteToggle={toggleTrackMute} />
          </div>
        </div>
      {/if}
    </section>
  </main>
{:else if route.kind === 'admin'}
  <main class="page admin-shell">
    {#if !adminLoggedIn}
      <section class="admin-login-shell">
        <div class="music-card auth-card admin-auth-card">
          <div>
            <p class="eyebrow">Fumen • Admin</p>
            <h1>Control room</h1>
            <p class="lede">Open the full-screen admin shell to manage users, ensembles, directories, and score permissions.</p>
          </div>
          <label class="field"><span>Admin password</span><input bind:value={adminPassword} type="password" placeholder="Hard-coded backend password" onkeydown={handleAdminPasswordKeydown} /></label>
          <div class="actions">
            <button class="button" disabled={adminLoading} onclick={() => void handleLogin()}>{adminLoading ? 'Checking...' : 'Open admin shell'}</button>
            <a class="button ghost" href="/">User homepage</a>
          </div>
          {#if adminError}<p class="status error">{adminError}</p>{/if}
        </div>
      </section>
    {:else}
      <section class="admin-app-shell">
        <header class="admin-topbar">
          <div class="admin-topbar-title">
            <div>
              <p class="eyebrow">Fumen</p>
              <div class="admin-breadcrumb" aria-label="Breadcrumb">
                <span>Admin</span>
                <span class="admin-breadcrumb-separator">/</span>
                <strong>{currentAdminSectionItem().label}</strong>
              </div>
              <p class="lede">
                {#if adminSection === 'users'}
                  Create accounts and issue temporary device access.
                {:else if adminSection === 'ensembles'}
                  Assign users to rehearsal groups.
                {:else if adminSection === 'directories'}
                  Grant directory access to ensembles.
                {:else}
                  Upload, organize, and retire scores.
                {/if}
              </p>
            </div>
          </div>
          <div class="admin-topbar-actions">
            <div class="admin-topbar-stats">
              <span>{adminUsers.length} users</span>
              <span>{ensembles.length} ensembles</span>
              <span>{directories.length} directories</span>
              <span>{musics.length} scores</span>
            </div>
            <a class="button ghost" href="/">User homepage</a>
            <button class="button ghost" onclick={logoutAdmin}>Log out</button>
          </div>
        </header>

        <div class="admin-shell-body">
          <aside class="admin-sidebar">
            <nav class="admin-nav-list" aria-label="Admin sections">
              {#each adminSectionItems as section}
                <button
                  class="admin-nav-button"
                  class:is-active={adminSection === section.id}
                  onclick={() => (adminSection = section.id)}
                >
                  <span class="admin-nav-eyebrow">{section.eyebrow}</span>
                  <strong>{section.label}</strong>
                  {#if section.id === 'users'}
                    <small>{adminUsers.length} total</small>
                  {:else if section.id === 'ensembles'}
                    <small>{ensembles.length} groups</small>
                  {:else if section.id === 'directories'}
                    <small>{directories.length} folders</small>
                  {:else if section.id === 'scores'}
                    <small>{musics.length} scores</small>
                  {:else}
                    <small>Everything at a glance</small>
                  {/if}
                </button>
              {/each}
            </nav>

            {#if adminError}<p class="status error">{adminError}</p>{/if}
            {#if adminSuccess}<p class="status success">{adminSuccess}</p>{/if}
          </aside>

          <div class="admin-main">
            {#if adminSection === 'users'}
              <section class="list-section admin-stage">
                <p class="section-blurb admin-stage-intro">Create username-only accounts and hand out one-tap temporary login access.</p>

                <div class="admin-panel-stack">
                  <div class="music-card admin-utility-card">
                    <div class="card-header"><div><p class="meta-label">Create</p><h3>New account</h3></div></div>
                    <label class="field"><span>Username</span><input bind:value={newUsername} placeholder="example: lucas" /></label>
                    <button class="button" disabled={creatingUser} onclick={() => void handleCreateUser()}>{creatingUser ? 'Creating...' : 'Create user'}</button>
                  </div>

                  {#if adminUsers.length === 0}
                    <div class="music-card"><p class="hint">No users yet.</p></div>
                  {:else}
                    <div class="music-list">
                      {#each adminUsers as user}
                        <article class="music-card">
                          <div class="music-topline"><div><h3>{user.username}</h3><p class="subtle">Created {prettyDate(user.created_at)}</p></div><p class="status-pill">user</p></div>
                          <div class="actions">
                            <button class="button secondary" onclick={() => void handleAdminShowUserQr(user)}>Show QR code</button>
                            <button class="button ghost" onclick={() => void handleAdminCopyUserLink(user)}>Copy connection link</button>
                          </div>
                        </article>
                      {/each}
                    </div>
                  {/if}
                </div>
              </section>
            {:else if adminSection === 'ensembles'}
              <section class="list-section admin-stage">
                <p class="section-blurb admin-stage-intro">Use ensembles as the membership layer that unlocks directories for groups of users.</p>

                <div class="admin-panel-stack">
                  <div class="music-card admin-utility-card">
                    <div class="card-header"><div><p class="meta-label">Create</p><h3>New ensemble</h3></div></div>
                    <label class="field"><span>Ensemble name</span><input bind:value={newEnsembleName} placeholder="example: strings" /></label>
                    <button class="button" disabled={creatingEnsemble} onclick={() => void handleCreateEnsemble()}>{creatingEnsemble ? 'Creating...' : 'Create ensemble'}</button>
                  </div>

                  {#if ensembles.length === 0}
                    <div class="music-card"><p class="hint">No ensembles yet.</p></div>
                  {:else}
                    <div class="music-list">
                      {#each ensembles as ensemble}
                        <article class="music-card">
                          <div class="music-topline"><div><h3>{ensemble.name}</h3><p class="subtle">Created {prettyDate(ensemble.created_at)}</p></div><p class="status-pill">{ensemble.member_user_ids.length} members</p></div>
                          <div class="toggle-grid">
                            {#each adminUsers as user}
                              <label class="toggle-row">
                                <span>{user.username}</span>
                                <input
                                  type="checkbox"
                                  checked={ensemble.member_user_ids.includes(user.id)}
                                  onchange={(event) =>
                                    void toggleUserEnsembleMembership(
                                      ensemble.id,
                                      user.id,
                                      (event.currentTarget as HTMLInputElement).checked,
                                    )}
                                />
                              </label>
                            {/each}
                          </div>
                        </article>
                      {/each}
                    </div>
                  {/if}
                </div>
              </section>
            {:else if adminSection === 'directories'}
              <section class="list-section admin-stage">
                <p class="section-blurb admin-stage-intro">Directories are the shelves scores live on. Grant each shelf to the right ensembles.</p>

                <div class="admin-panel-stack">
                  <div class="music-card admin-utility-card">
                    <div class="card-header"><div><p class="meta-label">Create</p><h3>New directory</h3></div></div>
                    <label class="field"><span>Directory name</span><input bind:value={newDirectoryName} placeholder="example: rehearsals" /></label>
                    <button class="button" disabled={creatingDirectory} onclick={() => void handleCreateDirectory()}>{creatingDirectory ? 'Creating...' : 'Create directory'}</button>
                  </div>

                  {#if directories.length === 0}
                    <div class="music-card"><p class="hint">No directories yet.</p></div>
                  {:else}
                    <div class="music-list">
                      {#each directories as directory}
                        <article class="music-card">
                          <div class="music-topline"><div><h3>{directory.name}</h3><p class="subtle">Created {prettyDate(directory.created_at)}</p></div><p class="status-pill">{directory.score_count} scores</p></div>
                          <div class="toggle-grid">
                            {#each ensembles as ensemble}
                              <label class="toggle-row">
                                <span>{ensemble.name}</span>
                                <input
                                  type="checkbox"
                                  checked={directory.permitted_ensemble_ids.includes(ensemble.id)}
                                  onchange={(event) =>
                                    void toggleDirectoryGrant(
                                      directory.id,
                                      ensemble.id,
                                      (event.currentTarget as HTMLInputElement).checked,
                                    )}
                                />
                              </label>
                            {/each}
                          </div>
                        </article>
                      {/each}
                    </div>
                  {/if}
                </div>
              </section>
            {:else if adminSection === 'scores'}
              <section class="list-section admin-stage">
                <p class="section-blurb admin-stage-intro">Upload new pieces, assign them to directories, tidy public ids, and remove old scores.</p>

                <div class="admin-panel-stack">
                  <div class="music-card upload-card admin-utility-card">
                    <div class="card-header"><div><p class="meta-label">Upload</p><h3>Add a MuseScore score</h3></div></div>
                    <div class="upload-grid">
                      <label class="field"><span>Title</span><input bind:value={uploadTitle} placeholder="Optional display title" /></label>
                      <label class="field"><span>Public id</span><input bind:value={uploadPublicId} placeholder="Optional friendly id" /></label>
                      <label class="field"><span>Directory</span><select bind:value={uploadDirectoryId}>{#each directories as directory}<option value={directory.id}>{directory.name}</option>{/each}</select></label>
                      <label class="field"><span>Stem quality</span><select bind:value={uploadQualityProfile}>{#each STEM_QUALITY_PROFILES as option}<option value={option.value}>{option.label} ({option.value === 'standard' ? '32k' : option.value === 'compact' ? '24k' : '48k'})</option>{/each}</select><small class="subtle">{STEM_QUALITY_PROFILES.find((option) => option.value === uploadQualityProfile)?.description}</small></label>
                      <label class="field file-field"><span>MSCZ file</span><input id="mscz-input" type="file" accept=".mscz" onchange={handleFileSelection} /></label>
                    </div>
                    <button class="button" disabled={uploadBusy} onclick={() => void handleUpload()}>{uploadBusy ? 'Uploading...' : 'Upload score'}</button>
                  </div>

                  {#if musics.length === 0}
                    <div class="music-card"><p class="hint">No uploads yet.</p></div>
                  {:else}
                    <div class="music-list">
                      {#each musics as music}
                        <article class="music-card">
                          <div class="music-topline"><div><h3>{music.title}</h3><p class="subtle">{music.filename}</p></div><p class="status-pill">{music.midi_status} midi</p></div>
                          <div class="meta-grid">
                            <div><p class="meta-label">Directory</p><p>{music.directory_name}</p></div>
                            <div><p class="meta-label">Random link</p><a href={music.public_url} target="_blank" rel="noreferrer">{music.public_url}</a></div>
                            <div><p class="meta-label">Uploaded</p><p>{prettyDate(music.created_at)}</p></div>
                            <div><p class="meta-label">Audio export</p><p>{music.audio_status}</p></div>
                            <div><p class="meta-label">Quality</p><p>{qualityProfileLabel(music.quality_profile)}</p></div>
                            <div><p class="meta-label">Stems</p><p>{music.stems_status}</p></div>
                            <div><p class="meta-label">Stems size</p><p>{formatBytes(music.stems_total_bytes)}</p></div>
                          </div>
                          {#if music.audio_error}<p class="hint">{music.audio_error}</p>{/if}
                          {#if music.stems_error}<p class="hint">{music.stems_error}</p>{/if}
                          <div class="id-row">
                            <label class="field"><span>Friendly public id</span><input bind:value={editPublicIds[music.id]} placeholder="example: moonlight-sonata" /></label>
                            <button class="button secondary" disabled={savingIdFor === music.id} onclick={() => void handleSavePublicId(music.id)}>{savingIdFor === music.id ? 'Saving...' : 'Save id'}</button>
                          </div>
                          <div class="id-row">
                            <label class="field"><span>Directory</span><select bind:value={editDirectoryIds[music.id]}>{#each directories as directory}<option value={directory.id}>{directory.name}</option>{/each}</select></label>
                            <button class="button secondary" disabled={movingMusicFor === music.id} onclick={() => void handleMoveMusic(music.id)}>{movingMusicFor === music.id ? 'Moving...' : 'Move score'}</button>
                          </div>
                          <div class="actions">
                            <button class="button ghost" onclick={() => void copyText(music.public_url, 'admin', 'Random link copied.')}>Copy random link</button>
                            {#if music.public_id_url}<button class="button ghost" onclick={() => void copyText(music.public_id_url!, 'admin', 'Id link copied.')}>Copy id link</button>{/if}
                            {#if music.stems_status !== 'ready'}<button class="button secondary" disabled={retryingFor === music.id} onclick={() => void handleRetryRender(music.id)}>{retryingFor === music.id ? 'Retrying...' : 'Retry render'}</button>{/if}
                            <button class="button ghost danger" disabled={deletingMusicFor === music.id} onclick={() => void handleDeleteMusic(music.id)}>{deletingMusicFor === music.id ? 'Deleting...' : 'Delete score'}</button>
                            <a class="button secondary" href={music.download_url} target="_blank" rel="noreferrer">Original file</a>
                            {#if music.midi_download_url}<a class="button secondary" href={music.midi_download_url} target="_blank" rel="noreferrer">MIDI export</a>{/if}
                            {#if music.public_id_url}<a class="button secondary" href={music.public_id_url} target="_blank" rel="noreferrer">Open id link</a>{/if}
                          </div>
                        </article>
                      {/each}
                    </div>
                  {/if}
                </div>
              </section>
            {/if}
          </div>
        </div>
      </section>
    {/if}
  </main>
{:else}
  <main class="page home-shell">
    <section class="hero-panel">
      <div class="hero-actions">
        <div>
          <p class="eyebrow">Fumen • Users</p>
          <h1>{currentUser ? `Welcome, ${currentUser.username}` : 'Connect a device'}</h1>
          <p class="lede">{#if currentUser}You are signed in on this device. Generate a QR code or a short-lived connection link to open your session somewhere else.{:else}Scan a QR code from the admin panel or another signed-in device, or paste a 5-minute connection link to log in.{/if}</p>
        </div>
        <div class="hero-actions-stack">
          {#if currentUser}
            <button class="button secondary" onclick={() => void handleShowMyQr()}>Show QR code</button>
            <button class="button ghost" onclick={() => void handleCopyMyLink()}>Copy connection link</button>
            <button class="button ghost" onclick={logoutUser}>Log out</button>
          {:else}
            <button class="button" onclick={() => void openScanner()}>Scan QR code</button>
            <a class="button ghost" href="/admin">Admin panel</a>
          {/if}
        </div>
      </div>
    </section>
    <section class="content-panel home-grid">
      {#if route.kind === 'connect'}
        <div class="music-card connect-card"><p class="meta-label">Connection</p><h2>Finishing sign-in</h2><p class="lede">We are validating your temporary connection link.</p>{#if connectionBusy || userLoading}<p class="status">Connecting...</p>{/if}{#if userError}<p class="status error">{userError}</p>{/if}</div>
      {:else if currentUser}
        <div class="music-card">
          <div class="card-header"><div><p class="meta-label">Session</p><h2>{currentUser.username}</h2></div><p class="status-pill">signed in</p></div>
          <div class="meta-grid">
            <div><p class="meta-label">Username</p><p>{currentUser.username}</p></div>
            <div><p class="meta-label">Session until</p><p>{prettyDate(userSessionExpiresAt)}</p></div>
            <div><p class="meta-label">Accessible directories</p><p>{userLibrary.length}</p></div>
          </div>
          <p class="hint">Every QR code and connection link is single-use and valid for 5 minutes.</p>
          {#if userError}<p class="status error">{userError}</p>{/if}
          {#if userSuccess}<p class="status success">{userSuccess}</p>{/if}
        </div>
        <div class="music-card">
          <div class="card-header"><div><p class="meta-label">Library</p><h2>Your directories</h2></div></div>
          {#if userLibrary.length === 0}
            <p class="hint">No directories are available for your ensembles yet.</p>
          {:else}
            <div class="directory-stack">
              {#each userLibrary as directory}
                <section class="directory-panel">
                  <div class="music-topline"><div><h3>{directory.name}</h3><p class="subtle">{directory.scores.length} scores</p></div></div>
                  <div class="score-link-list">
                    {#each directory.scores as score}
                      <a class="score-link-row" href={score.public_url}>
                        <span>{score.title}</span>
                        <small>{score.filename}</small>
                      </a>
                    {/each}
                  </div>
                </section>
              {/each}
            </div>
          {/if}
        </div>
      {:else}
        <div class="music-card">
          <div class="card-header"><div><p class="meta-label">Connect</p><h2>Use a connection link</h2></div><button class="button secondary" onclick={() => void openScanner()}>Scan QR code</button></div>
          <label class="field"><span>Connection link</span><input bind:value={manualConnectionLink} placeholder="Paste a link like https://.../connect/..." /></label>
          <div class="actions"><button class="button" onclick={() => void handleManualConnect()} disabled={connectionBusy}>{connectionBusy ? 'Connecting...' : 'Connect this device'}</button><a class="button ghost" href="/admin">Open admin panel</a></div>
          {#if userError}<p class="status error">{userError}</p>{/if}
          {#if userSuccess}<p class="status success">{userSuccess}</p>{/if}
        </div>
        <div class="music-card"><p class="meta-label">How it works</p><h2>Username-only access</h2><p class="lede">An admin creates your username once. After that, any signed-in device can generate a temporary QR code or link for another device.</p><p class="hint">If your browser blocks camera access, paste the connection link here instead.</p></div>
      {/if}
    </section>
  </main>
{/if}
{#if credentialModalOpen}
  <!-- svelte-ignore a11y_no_noninteractive_tabindex a11y_click_events_have_key_events -->
  <div
    class="modal-backdrop"
    role="presentation"
    tabindex="0"
    onclick={(event) => handleBackdropClick(event, () => (credentialModalOpen = false))}
    onkeydown={(event) => event.key === 'Escape' && (credentialModalOpen = false)}
  >
    <div class="modal-card" role="dialog" aria-modal="true" tabindex="-1">
      <div class="card-header">
        <div><p class="meta-label">Temporary access</p><h2>{credentialModalTitle}</h2></div>
        <button class="button ghost" onclick={() => (credentialModalOpen = false)}>Close</button>
      </div>
      {#if credentialQrDataUrl}<img class="qr-preview" src={credentialQrDataUrl} alt={credentialModalTitle} />{/if}
      <p class="hint">Valid until {prettyDate(credentialExpiresAt)}</p>
      <div class="field"><span>Connection link</span><input value={credentialLink} readonly /></div>
      <button class="button" onclick={() => void copyText(credentialLink, route.kind === 'admin' ? 'admin' : 'user', 'Connection link copied to clipboard.')}>Copy link</button>
    </div>
  </div>
{/if}

{#if scannerOpen}
  <!-- svelte-ignore a11y_no_noninteractive_tabindex a11y_click_events_have_key_events -->
  <div
    class="modal-backdrop"
    role="presentation"
    tabindex="0"
    onclick={(event) => handleBackdropClick(event, closeScanner)}
    onkeydown={(event) => event.key === 'Escape' && closeScanner()}
  >
    <div class="modal-card" role="dialog" aria-modal="true" tabindex="-1">
      <div class="card-header">
        <div><p class="meta-label">Scan</p><h2>Scan a QR code</h2></div>
        <button class="button ghost" onclick={closeScanner}>Close</button>
      </div>
      <div class="scanner-frame"><video class="scanner-video" bind:this={scannerVideo} muted playsinline></video></div>
      <p class="hint">Point the camera at a Fumen QR code to connect this device.</p>
      {#if scannerError}<p class="status error">{scannerError}</p>{/if}
    </div>
  </div>
{/if}

