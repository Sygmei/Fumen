<script lang="ts">
    import {
        addMusicToEnsemble,
        addUserToEnsemble,
        createAdminUserLoginLink,
        createEnsemble,
        createUser,
        deleteMusic,
        listEnsembles,
        listMusics,
        listUsers,
        moveMusic,
        removeMusicFromEnsemble,
        removeUserFromEnsemble,
        retryRender,
        STEM_QUALITY_PROFILES,
        updatePublicId,
        uploadMusic,
        type AdminMusic,
        type AppUser,
        type Ensemble,
        type LoginLinkResponse,
        type StemQualityProfile,
    } from "../lib/api";
    import { formatBytes, prettyDate, qualityProfileLabel } from "../lib/utils";

    const {
        currentUser,
        userLoading,
        userError,
        preloadedUsername,
        onShowCredential,
        onLogout,
    }: {
        currentUser: AppUser | null;
        userLoading: boolean;
        userError: string;
        preloadedUsername: string;
        onShowCredential: (
            title: string,
            response: LoginLinkResponse,
        ) => Promise<void>;
        onLogout: () => Promise<void>;
    } = $props();

    type AdminSection = "users" | "ensembles" | "scores";

    const adminSectionItems: Array<{
        id: AdminSection;
        label: string;
        eyebrow: string;
    }> = [
        { id: "users", label: "Users", eyebrow: "Accounts" },
        { id: "ensembles", label: "Ensembles", eyebrow: "Groups" },
        { id: "scores", label: "Scores", eyebrow: "Library" },
    ];

    let adminSection = $state<AdminSection>("users");
    let adminLoading = $state(false);
    let adminError = $state("");
    let adminSuccess = $state("");
    let uploadTitle = $state("");
    let uploadPublicId = $state("");
    let uploadQualityProfile = $state<StemQualityProfile>("standard");
    let selectedFile = $state<File | null>(null);
    let uploadBusy = $state(false);
    let musics = $state<AdminMusic[]>([]);
    let adminUsers = $state<AppUser[]>([]);
    let ensembles = $state<Ensemble[]>([]);
    let newUsername = $state("");
    let creatingUser = $state(false);
    let newEnsembleName = $state("");
    let creatingEnsemble = $state(false);
    let uploadEnsembleId = $state("");
    let editPublicIds = $state<Record<string, string>>({});
    let editEnsembleIds = $state<Record<string, string>>({});
    let savingIdFor = $state("");
    let movingMusicFor = $state("");
    let retryingFor = $state("");
    let deletingMusicFor = $state("");

    function canAccessAdmin(user = currentUser) {
        return user?.role === "admin" || user?.role === "superadmin";
    }

    function isSuperadmin(user = currentUser) {
        return user?.role === "superadmin";
    }

    function currentAdminSectionItem() {
        return (
            adminSectionItems.find((section) => section.id === adminSection) ??
            adminSectionItems[0]
        );
    }

    $effect(() => {
        if (currentUser && canAccessAdmin()) {
            void refreshAdminData();
        }
    });

    async function refreshAdminData() {
        adminLoading = true;
        adminError = "";

        try {
            const [musicItems, userItems, ensembleItems] = await Promise.all([
                listMusics(),
                listUsers(),
                listEnsembles(),
            ]);
            musics = musicItems;
            adminUsers = userItems;
            ensembles = ensembleItems;
            editPublicIds = Object.fromEntries(
                musicItems.map((music) => [music.id, music.public_id ?? ""]),
            );
            editEnsembleIds = Object.fromEntries(
                musicItems.map((music) => [
                    music.id,
                    music.ensemble_ids[0] ?? ensembleItems[0]?.id ?? "",
                ]),
            );
            if (
                !uploadEnsembleId ||
                !ensembleItems.some(
                    (ensemble) => ensemble.id === uploadEnsembleId,
                )
            ) {
                uploadEnsembleId = ensembleItems[0]?.id ?? "";
            }
        } catch (error) {
            adminError =
                error instanceof Error
                    ? error.message
                    : "Unable to load admin data";
        } finally {
            adminLoading = false;
        }
    }

    async function handleCreateUser() {
        const trimmed = newUsername.trim();
        if (!trimmed) {
            adminError = "Choose a username first.";
            return;
        }

        creatingUser = true;
        adminError = "";
        adminSuccess = "";

        try {
            const user = await createUser(trimmed);
            adminUsers = [...adminUsers, user].sort((left, right) =>
                left.username.localeCompare(right.username),
            );
            newUsername = "";
            adminSuccess = `User ${user.username} created.`;
        } catch (error) {
            adminError =
                error instanceof Error
                    ? error.message
                    : "Unable to create user";
        } finally {
            creatingUser = false;
        }
    }

    async function handleCreateEnsemble() {
        const trimmed = newEnsembleName.trim();
        if (!trimmed) {
            adminError = "Choose an ensemble name first.";
            return;
        }

        creatingEnsemble = true;
        adminError = "";
        adminSuccess = "";

        try {
            const ensemble = await createEnsemble(trimmed);
            ensembles = [...ensembles, ensemble].sort((left, right) =>
                left.name.localeCompare(right.name),
            );
            newEnsembleName = "";
            adminSuccess = `Ensemble ${ensemble.name} created.`;
        } catch (error) {
            adminError =
                error instanceof Error
                    ? error.message
                    : "Unable to create ensemble";
        } finally {
            creatingEnsemble = false;
        }
    }

    function ensembleMemberRole(
        ensemble: Ensemble,
        userId: string,
    ): "none" | "user" | "admin" {
        return (
            ensemble.members.find((member) => member.user_id === userId)
                ?.role ?? "none"
        );
    }

    async function updateUserEnsembleRole(
        ensembleId: string,
        userId: string,
        role: "none" | "user" | "admin",
    ) {
        adminError = "";
        adminSuccess = "";

        try {
            if (role === "none") {
                await removeUserFromEnsemble(ensembleId, userId);
            } else {
                await addUserToEnsemble(ensembleId, userId, role);
            }
            await refreshAdminData();
            adminSuccess = "Ensemble role updated.";
        } catch (error) {
            adminError =
                error instanceof Error
                    ? error.message
                    : "Unable to update ensemble role";
        }
    }

    function musicHasEnsemble(music: AdminMusic, ensembleId: string) {
        return music.ensemble_ids.includes(ensembleId);
    }

    async function toggleMusicEnsembleAssignment(
        musicId: string,
        ensembleId: string,
        shouldAdd: boolean,
    ) {
        adminError = "";
        adminSuccess = "";

        try {
            if (shouldAdd) {
                await addMusicToEnsemble(musicId, ensembleId);
            } else {
                await removeMusicFromEnsemble(musicId, ensembleId);
            }
            await refreshAdminData();
            adminSuccess = "Score ensembles updated.";
        } catch (error) {
            adminError =
                error instanceof Error
                    ? error.message
                    : "Unable to update score ensembles";
        }
    }

    async function handleUpload() {
        if (!selectedFile) {
            adminError = "Choose an .mscz file first.";
            return;
        }
        if (!uploadEnsembleId) {
            adminError = "Choose an ensemble first.";
            return;
        }

        uploadBusy = true;
        adminError = "";
        adminSuccess = "";

        try {
            await uploadMusic({
                file: selectedFile,
                title: uploadTitle,
                publicId: uploadPublicId,
                qualityProfile: uploadQualityProfile,
                ensembleId: uploadEnsembleId,
            });

            uploadTitle = "";
            uploadPublicId = "";
            uploadQualityProfile = "standard";
            selectedFile = null;
            const input = document.getElementById(
                "mscz-input",
            ) as HTMLInputElement | null;
            if (input) {
                input.value = "";
            }

            await refreshAdminData();
            adminSuccess = "Upload completed.";
        } catch (error) {
            adminError =
                error instanceof Error ? error.message : "Upload failed";
        } finally {
            uploadBusy = false;
        }
    }

    async function handleSavePublicId(musicId: string) {
        savingIdFor = musicId;
        adminError = "";
        adminSuccess = "";

        try {
            const updated = await updatePublicId(
                musicId,
                editPublicIds[musicId] ?? "",
            );
            musics = musics.map((music) =>
                music.id === musicId ? updated : music,
            );
            editPublicIds = {
                ...editPublicIds,
                [musicId]: updated.public_id ?? "",
            };
            adminSuccess = "Public id updated.";
        } catch (error) {
            adminError =
                error instanceof Error
                    ? error.message
                    : "Unable to update public id";
        } finally {
            savingIdFor = "";
        }
    }

    async function handleMoveMusic(musicId: string) {
        const ensembleId = editEnsembleIds[musicId] ?? "";
        if (!ensembleId) {
            adminError = "Choose an ensemble first.";
            return;
        }

        movingMusicFor = musicId;
        adminError = "";
        adminSuccess = "";

        try {
            const updated = await moveMusic(musicId, ensembleId);
            musics = musics.map((music) =>
                music.id === musicId ? updated : music,
            );
            editEnsembleIds = {
                ...editEnsembleIds,
                [musicId]: updated.ensemble_ids[0] ?? "",
            };
            await refreshAdminData();
            adminSuccess = "Score ensemble updated.";
        } catch (error) {
            adminError =
                error instanceof Error
                    ? error.message
                    : "Unable to update score ensemble";
        } finally {
            movingMusicFor = "";
        }
    }

    async function handleDeleteMusic(musicId: string) {
        if (!window.confirm("Delete this score permanently?")) {
            return;
        }

        deletingMusicFor = musicId;
        adminError = "";
        adminSuccess = "";

        try {
            await deleteMusic(musicId);
            musics = musics.filter((music) => music.id !== musicId);
            delete editPublicIds[musicId];
            delete editEnsembleIds[musicId];
            await refreshAdminData();
            adminSuccess = "Score deleted.";
        } catch (error) {
            adminError =
                error instanceof Error
                    ? error.message
                    : "Unable to delete score";
        } finally {
            deletingMusicFor = "";
        }
    }

    async function handleRetryRender(musicId: string) {
        retryingFor = musicId;
        adminError = "";
        adminSuccess = "";

        try {
            const updated = await retryRender(musicId);
            musics = musics.map((music) =>
                music.id === musicId ? updated : music,
            );
            adminSuccess = "Render retried successfully.";
        } catch (error) {
            adminError =
                error instanceof Error ? error.message : "Retry failed";
        } finally {
            retryingFor = "";
        }
    }

    async function copyText(value: string, successMessage: string) {
        await navigator.clipboard.writeText(value);
        adminSuccess = successMessage;
        adminError = "";
    }

    async function handleShowUserQr(user: AppUser) {
        adminError = "";
        adminSuccess = "";

        try {
            const response = await createAdminUserLoginLink(user.id);
            await onShowCredential(`QR code for ${user.username}`, response);
            adminSuccess = `QR code ready for ${user.username}.`;
        } catch (error) {
            adminError =
                error instanceof Error
                    ? error.message
                    : "Unable to create QR code";
        }
    }

    async function handleCopyUserLink(user: AppUser) {
        adminError = "";
        adminSuccess = "";

        try {
            const response = await createAdminUserLoginLink(user.id);
            await copyText(
                response.connection_url,
                `Connection link copied for ${user.username}.`,
            );
        } catch (error) {
            adminError =
                error instanceof Error
                    ? error.message
                    : "Unable to create connection link";
        }
    }

    function handleFileSelection(event: Event) {
        const target = event.currentTarget as HTMLInputElement;
        selectedFile = target.files?.[0] ?? null;
    }
</script>

{#if userLoading || adminLoading}
    <div class="home-loading-overlay">
        <div class="loading-eq" aria-label="Loading">
            <span></span>
            <span></span>
            <span></span>
            <span></span>
            <span></span>
        </div>
        <p class="loading-eq-label">{preloadedUsername ? `Hello, ${preloadedUsername}` : 'Fumen'}</p>
    </div>
{:else if !currentUser}
    <section class="admin-login-shell">
        <div class="music-card auth-card admin-auth-card">
            <div>
                <p class="eyebrow">Fumen • Admin</p>
                <h1>Control room</h1>
                <p class="lede">
                    Sign in as the seeded superadmin or another admin-enabled
                    user to open the full-screen control room.
                </p>
            </div>
            <div class="actions">
                <a class="button ghost" href="/">User homepage</a>
            </div>
            <p class="hint">
                Use the backend CLI to generate a temporary connection link for
                the superadmin account, then open it here.
            </p>
            {#if userError}<p class="status error">{userError}</p>{/if}
        </div>
    </section>
{:else if !canAccessAdmin()}
    <section class="admin-login-shell">
        <div class="music-card auth-card admin-auth-card">
            <div>
                <p class="eyebrow">Fumen • Admin</p>
                <h1>Control room</h1>
                <p class="lede">
                    {currentUser.username} is signed in, but this account does not
                    have admin access.
                </p>
            </div>
            <div class="actions">
                <a class="button ghost" href="/">User homepage</a>
            </div>
            {#if adminError}<p class="status error">{adminError}</p>{/if}
        </div>
    </section>
{:else}
    <section class="admin-app-shell">
        <header class="admin-topbar">
            <div class="admin-topbar-title">
                <div class="admin-breadcrumb" aria-label="Breadcrumb">
                    <a class="admin-brand" href="/">Fumen</a>
                    <span
                        class="admin-breadcrumb-separator admin-breadcrumb-dash"
                        >—</span
                    >
                    <span>Admin</span>
                    <span class="admin-breadcrumb-separator">/</span>
                    <strong>{currentAdminSectionItem().label}</strong>
                </div>
            </div>
            <div class="admin-topbar-actions">
                <span class="status-pill">{currentUser.role}</span>
                <a class="button ghost" href="/">User homepage</a>
                <button class="button ghost" onclick={() => void onLogout()}
                    >Sign out</button
                >
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
                            <span class="admin-nav-eyebrow"
                                >{section.eyebrow}</span
                            >
                            <strong>{section.label}</strong>
                            {#if section.id === "users"}
                                <small>{adminUsers.length} total</small>
                            {:else if section.id === "ensembles"}
                                <small>{ensembles.length} groups</small>
                            {:else if section.id === "scores"}
                                <small>{musics.length} scores</small>
                            {:else}
                                <small>Everything at a glance</small>
                            {/if}
                        </button>
                    {/each}
                </nav>

                {#if adminError}<p class="status error">{adminError}</p>{/if}
                {#if adminSuccess}<p class="status success">
                        {adminSuccess}
                    </p>{/if}
            </aside>

            <div class="admin-main">
                {#if adminSection === "users"}
                    <section class="list-section admin-stage">
                        <p class="section-blurb admin-stage-intro">
                            Create username-only accounts and hand out one-tap
                            temporary login access.
                        </p>

                        <div class="admin-panel-stack">
                            <div class="music-card admin-utility-card">
                                <div class="card-header">
                                    <div>
                                        <p class="meta-label">Create</p>
                                        <h3>New account</h3>
                                    </div>
                                </div>
                                <label class="field"
                                    ><span>Username</span><input
                                        bind:value={newUsername}
                                        placeholder="example: lucas"
                                    /></label
                                >
                                <button
                                    class="button"
                                    disabled={creatingUser}
                                    onclick={() => void handleCreateUser()}
                                    >{creatingUser
                                        ? "Creating..."
                                        : "Create user"}</button
                                >
                            </div>

                            {#if adminUsers.length === 0}
                                <div class="music-card">
                                    <p class="hint">No users yet.</p>
                                </div>
                            {:else}
                                <div class="music-list">
                                    {#each adminUsers as user}
                                        <article class="music-card">
                                            <div class="music-topline">
                                                <div>
                                                    <h3>{user.username}</h3>
                                                    <p class="subtle">
                                                        Created {prettyDate(
                                                            user.created_at,
                                                        )}
                                                    </p>
                                                </div>
                                                <p class="status-pill">
                                                    {user.role}
                                                </p>
                                            </div>
                                            <div class="actions">
                                                <button
                                                    class="button secondary"
                                                    onclick={() =>
                                                        void handleShowUserQr(
                                                            user,
                                                        )}>Show QR code</button
                                                >
                                                <button
                                                    class="button ghost"
                                                    onclick={() =>
                                                        void handleCopyUserLink(
                                                            user,
                                                        )}
                                                    >Copy connection link</button
                                                >
                                            </div>
                                        </article>
                                    {/each}
                                </div>
                            {/if}
                        </div>
                    </section>
                {:else if adminSection === "ensembles"}
                    <section class="list-section admin-stage">
                        <p class="section-blurb admin-stage-intro">
                            Use ensembles as the core unit for membership, admin
                            scope, and score access.
                        </p>

                        <div class="admin-panel-stack">
                            {#if isSuperadmin()}
                                <div class="music-card admin-utility-card">
                                    <div class="card-header">
                                        <div>
                                            <p class="meta-label">Create</p>
                                            <h3>New ensemble</h3>
                                        </div>
                                    </div>
                                    <label class="field"
                                        ><span>Ensemble name</span><input
                                            bind:value={newEnsembleName}
                                            placeholder="example: strings"
                                        /></label
                                    >
                                    <button
                                        class="button"
                                        disabled={creatingEnsemble}
                                        onclick={() =>
                                            void handleCreateEnsemble()}
                                        >{creatingEnsemble
                                            ? "Creating..."
                                            : "Create ensemble"}</button
                                    >
                                </div>
                            {/if}

                            {#if ensembles.length === 0}
                                <div class="music-card">
                                    <p class="hint">No ensembles yet.</p>
                                </div>
                            {:else}
                                <div class="music-list">
                                    {#each ensembles as ensemble}
                                        <article class="music-card">
                                            <div class="music-topline">
                                                <div>
                                                    <h3>{ensemble.name}</h3>
                                                    <p class="subtle">
                                                        Created {prettyDate(
                                                            ensemble.created_at,
                                                        )}
                                                    </p>
                                                </div>
                                                <p class="status-pill">
                                                    {ensemble.score_count} scores
                                                </p>
                                            </div>
                                            <div class="toggle-grid">
                                                {#each adminUsers as user}
                                                    <label class="toggle-row">
                                                        <span
                                                            >{user.username}</span
                                                        >
                                                        <select
                                                            value={ensembleMemberRole(
                                                                ensemble,
                                                                user.id,
                                                            )}
                                                            onchange={(event) =>
                                                                void updateUserEnsembleRole(
                                                                    ensemble.id,
                                                                    user.id,
                                                                    (
                                                                        event.currentTarget as HTMLSelectElement
                                                                    ).value as
                                                                        | "none"
                                                                        | "user"
                                                                        | "admin",
                                                                )}
                                                        >
                                                            <option value="none"
                                                                >No access</option
                                                            >
                                                            <option value="user"
                                                                >User</option
                                                            >
                                                            <option
                                                                value="admin"
                                                                >Admin</option
                                                            >
                                                        </select>
                                                    </label>
                                                {/each}
                                            </div>
                                        </article>
                                    {/each}
                                </div>
                            {/if}
                        </div>
                    </section>
                {:else if adminSection === "scores"}
                    <section class="list-section admin-stage">
                        <p class="section-blurb admin-stage-intro">
                            Upload new pieces into ensembles, tidy public ids,
                            and retire old scores.
                        </p>

                        <div class="admin-panel-stack">
                            <div
                                class="music-card upload-card admin-utility-card"
                            >
                                <div class="card-header">
                                    <div>
                                        <p class="meta-label">Upload</p>
                                        <h3>Add a MuseScore score</h3>
                                    </div>
                                </div>
                                <div class="upload-grid">
                                    <label class="field"
                                        ><span>Title</span><input
                                            bind:value={uploadTitle}
                                            placeholder="Optional display title"
                                        /></label
                                    >
                                    <label class="field"
                                        ><span>Public id</span><input
                                            bind:value={uploadPublicId}
                                            placeholder="Optional friendly id"
                                        /></label
                                    >
                                    <label class="field"
                                        ><span>Ensemble</span><select
                                            bind:value={uploadEnsembleId}
                                            >{#each ensembles as ensemble}<option
                                                    value={ensemble.id}
                                                    >{ensemble.name}</option
                                                >{/each}</select
                                        ></label
                                    >
                                    <label class="field"
                                        ><span>Stem quality</span><select
                                            bind:value={uploadQualityProfile}
                                            >{#each STEM_QUALITY_PROFILES as option}<option
                                                    value={option.value}
                                                    >{option.label} ({option.value ===
                                                    "standard"
                                                        ? "32k"
                                                        : option.value ===
                                                            "compact"
                                                          ? "24k"
                                                          : "48k"})</option
                                                >{/each}</select
                                        ><small class="subtle"
                                            >{STEM_QUALITY_PROFILES.find(
                                                (option) =>
                                                    option.value ===
                                                    uploadQualityProfile,
                                            )?.description}</small
                                        ></label
                                    >
                                    <label class="field file-field"
                                        ><span>MSCZ file</span><input
                                            id="mscz-input"
                                            type="file"
                                            accept=".mscz"
                                            onchange={handleFileSelection}
                                        /></label
                                    >
                                </div>
                                <button
                                    class="button"
                                    disabled={uploadBusy}
                                    onclick={() => void handleUpload()}
                                    >{uploadBusy
                                        ? "Uploading..."
                                        : "Upload score"}</button
                                >
                            </div>

                            {#if musics.length === 0}
                                <div class="music-card">
                                    <p class="hint">No uploads yet.</p>
                                </div>
                            {:else}
                                <div class="music-list">
                                    {#each musics as music}
                                        <article class="music-card">
                                            <div class="music-topline">
                                                <div>
                                                    <h3>{music.title}</h3>
                                                    <p class="subtle">
                                                        {music.filename}
                                                    </p>
                                                </div>
                                                <p class="status-pill">
                                                    {music.midi_status} midi
                                                </p>
                                            </div>
                                            <div class="meta-grid">
                                                <div>
                                                    <p class="meta-label">
                                                        Ensembles
                                                    </p>
                                                    <p>
                                                        {music.ensemble_names.join(
                                                            ", ",
                                                        ) || "None"}
                                                    </p>
                                                </div>
                                                <div>
                                                    <p class="meta-label">
                                                        Random link
                                                    </p>
                                                    <a
                                                        href={music.public_url}
                                                        target="_blank"
                                                        rel="noreferrer"
                                                        >{music.public_url}</a
                                                    >
                                                </div>
                                                <div>
                                                    <p class="meta-label">
                                                        Uploaded
                                                    </p>
                                                    <p>
                                                        {prettyDate(
                                                            music.created_at,
                                                        )}
                                                    </p>
                                                </div>
                                                <div>
                                                    <p class="meta-label">
                                                        Audio export
                                                    </p>
                                                    <p>{music.audio_status}</p>
                                                </div>
                                                <div>
                                                    <p class="meta-label">
                                                        Quality
                                                    </p>
                                                    <p>
                                                        {qualityProfileLabel(
                                                            music.quality_profile,
                                                        )}
                                                    </p>
                                                </div>
                                                <div>
                                                    <p class="meta-label">
                                                        Stems
                                                    </p>
                                                    <p>{music.stems_status}</p>
                                                </div>
                                                <div>
                                                    <p class="meta-label">
                                                        Stems size
                                                    </p>
                                                    <p>
                                                        {formatBytes(
                                                            music.stems_total_bytes,
                                                        )}
                                                    </p>
                                                </div>
                                            </div>
                                            {#if music.audio_error}<p
                                                    class="hint"
                                                >
                                                    {music.audio_error}
                                                </p>{/if}
                                            {#if music.stems_error}<p
                                                    class="hint"
                                                >
                                                    {music.stems_error}
                                                </p>{/if}
                                            <div class="id-row">
                                                <label class="field"
                                                    ><span
                                                        >Friendly public id</span
                                                    ><input
                                                        bind:value={
                                                            editPublicIds[
                                                                music.id
                                                            ]
                                                        }
                                                        placeholder="example: moonlight-sonata"
                                                    /></label
                                                >
                                                <button
                                                    class="button secondary"
                                                    disabled={savingIdFor ===
                                                        music.id}
                                                    onclick={() =>
                                                        void handleSavePublicId(
                                                            music.id,
                                                        )}
                                                    >{savingIdFor === music.id
                                                        ? "Saving..."
                                                        : "Save id"}</button
                                                >
                                            </div>
                                            <div class="id-row">
                                                <label class="field"
                                                    ><span
                                                        >Primary ensemble</span
                                                    ><select
                                                        bind:value={
                                                            editEnsembleIds[
                                                                music.id
                                                            ]
                                                        }
                                                        >{#each ensembles as ensemble}<option
                                                                value={ensemble.id}
                                                                >{ensemble.name}</option
                                                            >{/each}</select
                                                    ></label
                                                >
                                                <button
                                                    class="button secondary"
                                                    disabled={movingMusicFor ===
                                                        music.id}
                                                    onclick={() =>
                                                        void handleMoveMusic(
                                                            music.id,
                                                        )}
                                                    >{movingMusicFor ===
                                                    music.id
                                                        ? "Saving..."
                                                        : "Set primary ensemble"}</button
                                                >
                                            </div>
                                            <div class="toggle-grid">
                                                {#each ensembles as ensemble}
                                                    <label class="toggle-row">
                                                        <span
                                                            >{ensemble.name}</span
                                                        >
                                                        <input
                                                            type="checkbox"
                                                            checked={musicHasEnsemble(
                                                                music,
                                                                ensemble.id,
                                                            )}
                                                            onchange={(event) =>
                                                                void toggleMusicEnsembleAssignment(
                                                                    music.id,
                                                                    ensemble.id,
                                                                    (
                                                                        event.currentTarget as HTMLInputElement
                                                                    ).checked,
                                                                )}
                                                        />
                                                    </label>
                                                {/each}
                                            </div>
                                            <div class="actions">
                                                <button
                                                    class="button ghost"
                                                    onclick={() =>
                                                        void copyText(
                                                            music.public_url,
                                                            "Random link copied.",
                                                        )}
                                                    >Copy random link</button
                                                >
                                                {#if music.public_id_url}<button
                                                        class="button ghost"
                                                        onclick={() =>
                                                            void copyText(
                                                                music.public_id_url!,
                                                                "Id link copied.",
                                                            )}
                                                        >Copy id link</button
                                                    >{/if}
                                                {#if music.stems_status !== "ready"}<button
                                                        class="button secondary"
                                                        disabled={retryingFor ===
                                                            music.id}
                                                        onclick={() =>
                                                            void handleRetryRender(
                                                                music.id,
                                                            )}
                                                        >{retryingFor ===
                                                        music.id
                                                            ? "Retrying..."
                                                            : "Retry render"}</button
                                                    >{/if}
                                                <button
                                                    class="button ghost danger"
                                                    disabled={deletingMusicFor ===
                                                        music.id}
                                                    onclick={() =>
                                                        void handleDeleteMusic(
                                                            music.id,
                                                        )}
                                                    >{deletingMusicFor ===
                                                    music.id
                                                        ? "Deleting..."
                                                        : "Delete score"}</button
                                                >
                                                <a
                                                    class="button secondary"
                                                    href={music.download_url}
                                                    target="_blank"
                                                    rel="noreferrer"
                                                    >Original file</a
                                                >
                                                {#if music.midi_download_url}<a
                                                        class="button secondary"
                                                        href={music.midi_download_url}
                                                        target="_blank"
                                                        rel="noreferrer"
                                                        >MIDI export</a
                                                    >{/if}
                                                {#if music.public_id_url}<a
                                                        class="button secondary"
                                                        href={music.public_id_url}
                                                        target="_blank"
                                                        rel="noreferrer"
                                                        >Open id link</a
                                                    >{/if}
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
