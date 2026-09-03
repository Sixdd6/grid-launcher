<script lang="ts">
  import { api, type CloudPanelInfo, type ContentAvailability, type ContentKind, type DownloadStatus, type RomDetail } from './api';
  import { downloads } from './stores/downloads.svelte';
  import { isInstalled, installed, matchesInstalled, refresh as refreshInstalled } from './stores/installed.svelte';
  import { session } from './stores/session.svelte';
  import { sessions } from './stores/sessions.svelte';
  import Image from './Image.svelte';
  import CloudPanel from './details/CloudPanel.svelte';
  import NativeSettings from './details/NativeSettings.svelte';
  import { mergeDetail, summaryOf, type DetailsSubject } from './details/subject';
  import { cloudButtonLabel, isNativeExecutablePlatform, syntheticCloudGame, toggleCloudMode, type CloudMode } from './details/cloud';
  import { contentButtons, installLabel, isContentPlatform, isNativePlatform } from './details/actions';

  let {
    subject,
    onClose,
    onLibraryPathUnset,
  }: {
    subject: DetailsSubject;
    onClose: () => void;
    onLibraryPathUnset: () => void;
  } = $props();

  const LIVE_INSTALL_STATUSES: DownloadStatus[] = ['queued', 'downloading', 'installing', 'cancelling'];

  type PendingAction = 'install' | 'uninstall' | 'play' | 'stop' | null;

  let confirmingUninstall = $state(false);
  let pendingAction = $state<PendingAction>(null);
  let error = $state<string | null>(null);
  let panelEl = $state<HTMLElement | null>(null);

  // Install specials (task-16-brief.md): Cancel for a live install,
  // Install Update/DLC for installed PS4/Xbox 360 games, and the native
  // Game Settings dialog.
  let cancelPending = $state(false);
  let contentAvailability = $state<ContentAvailability | null>(null);
  let wasLive = $state(false);
  let contentActionKind = $state<ContentKind | null>(null);
  let showNativeSettings = $state(false);

  // Metadata overlay (task-10-brief.md): the subject carries whatever the
  // grid it opened from already had on hand; when that's thin (a server
  // subject only ever has a cover, or an installed row with no stored
  // screenshots) fetch the full RomDetail once and let it fill in the
  // gaps. Kept as separate local state rather than mutating `subject` —
  // the prop is the caller's data, this is purely a display overlay. The
  // fold itself lives in `mergeDetail` (details/subject.ts), which treats a
  // detail's empty strings/lists as "the server has nothing here" and keeps
  // the subject's own value rather than blanking the field.
  let detail = $state<RomDetail | null>(null);

  let merged = $derived(detail === null ? subject : mergeDetail(subject, detail));
  let coverSmall = $derived(merged.coverSmall);
  let coverLarge = $derived(merged.coverLarge);
  let screenshotUrls = $derived(merged.screenshotUrls);
  let description = $derived(merged.description);
  let rating = $derived(merged.rating);
  let genres = $derived(merged.genres);

  let failedScreenshots = $state<Record<string, true>>({});
  function markScreenshotFailed(url: string) {
    failedScreenshots = { ...failedScreenshots, [url]: true };
  }

  $effect(() => {
    if (subject.romId === null) return; // no server id: nothing to overlay
    if (!session.connected) return;
    if (subject.source !== 'server' && subject.screenshotUrls.length > 0) return;
    api
      .getRomDetail(subject.romId)
      .then((fetched) => {
        detail = fetched;
      })
      .catch(() => {}); // offline/removed rom: the subject's own data stands
  });

  let pending = $derived(pendingAction !== null);
  let summary = $derived(summaryOf(subject));
  let liveEntry = $derived(
    subject.romId !== null
      ? downloads.entries.find((e) => e.rom_id === subject.romId && LIVE_INSTALL_STATUSES.includes(e.status))
      : undefined
  );
  let installedNow = $derived(isInstalled(summary, subject.platformName));
  let liveSession = $derived(subject.romId !== null ? sessions.sessionFor(subject.romId) : undefined);

  let isContent = $derived(isContentPlatform(subject.platformName));
  let isNativeInstall = $derived(isNativePlatform(subject.platformName));
  let buttons = $derived(contentButtons(contentAvailability, installedNow, liveEntry !== undefined));

  // Fetched once the subject is installed-and-a-content-platform, and
  // re-fetched right after a live install for it finishes (`wasLive` tracks
  // the previous liveEntry-defined-ness across effect runs) — the server's
  // file list only changes once an update/DLC job completes.
  $effect(() => {
    if (subject.romId === null || !installedNow || !isContent) {
      contentAvailability = null;
      wasLive = liveEntry !== undefined;
      return;
    }
    const live = liveEntry !== undefined;
    const justFinished = wasLive && !live;
    wasLive = live;
    if (live || (contentAvailability !== null && !justFinished)) return;
    api
      .contentAvailability(subject.romId)
      .then((avail) => (contentAvailability = avail))
      .catch(() => (contentAvailability = null));
  });

  // Cloud saves/states (task-19-brief.md). `cloudGame` is the InstalledGame
  // registry row when one exists, else a synthetic stand-in built from the
  // subject — the cloud commands resolve "installed" themselves by identity
  // match (cloud_service.rs's `panel_info`), so a non-installed shared-scope
  // game (e.g. an entry on the synthetic `Emulators` platform) can still
  // open its panel.
  let installedRow = $derived(installed.list.find((row) => matchesInstalled(row, summary, subject.platformName)) ?? null);
  let cloudGame = $derived(installedRow ?? syntheticCloudGame(summary, subject.platformName));
  let isNative = $derived(isNativeExecutablePlatform(subject.platformName));

  let cloudMode = $state<CloudMode>('overview');
  let savePanelInfo = $state<CloudPanelInfo | null>(null);
  let statePanelInfo = $state<CloudPanelInfo | null>(null);
  let cloudPanelInfoError = $state<string | null>(null);

  let activeCloudPanelInfo = $derived(cloudMode === 'save' ? savePanelInfo : cloudMode === 'state' ? statePanelInfo : null);

  $effect(() => {
    panelEl?.focus();
  });

  $effect(() => {
    if (subject.romId === null) return; // no server id: nothing to manage cloud saves for
    api
      .cloudPanelInfo(cloudGame, 'save')
      .then((info) => (savePanelInfo = info))
      .catch((err) => (cloudPanelInfoError = errorMessage(err)));
    api
      .cloudPanelInfo(cloudGame, 'state')
      .then((info) => (statePanelInfo = info))
      .catch((err) => (cloudPanelInfoError = errorMessage(err)));
  });

  function handleCloudToggle(saveType: 'save' | 'state') {
    cloudMode = toggleCloudMode(cloudMode, saveType);
  }

  function errorMessage(err: unknown): string {
    return err instanceof Error ? err.message : String(err);
  }

  async function handleInstall() {
    if (subject.romId === null) return;
    error = null;
    pendingAction = 'install';
    try {
      await api.installGame(subject.romId);
    } catch (err) {
      const message = errorMessage(err);
      error = message;
      if (message.includes('library folder')) onLibraryPathUnset();
    } finally {
      pendingAction = null;
    }
  }

  async function handleUninstallClick() {
    if (subject.romId === null) return;
    if (!confirmingUninstall) {
      confirmingUninstall = true;
      return;
    }
    error = null;
    pendingAction = 'uninstall';
    try {
      await api.uninstallGame(subject.romId);
      await refreshInstalled();
      onClose();
    } catch (err) {
      error = errorMessage(err);
      confirmingUninstall = false;
    } finally {
      pendingAction = null;
    }
  }

  async function handleCancel() {
    if (subject.romId === null) return;
    error = null;
    cancelPending = true;
    try {
      await api.cancelDownloadForRom(subject.romId);
    } catch (err) {
      error = errorMessage(err);
    } finally {
      cancelPending = false;
    }
  }

  async function handleInstallContent(kind: ContentKind) {
    if (subject.romId === null) return;
    error = null;
    contentActionKind = kind;
    try {
      await api.installContent(subject.romId, kind);
    } catch (err) {
      error = errorMessage(err);
    } finally {
      contentActionKind = null;
    }
  }

  async function handlePlay() {
    if (subject.romId === null) return;
    error = null;
    pendingAction = 'play';
    try {
      await api.launchGame(subject.romId);
    } catch (err) {
      error = errorMessage(err);
    } finally {
      pendingAction = null;
    }
  }

  async function handleStop() {
    if (!liveSession) return;
    error = null;
    pendingAction = 'stop';
    try {
      await api.stopGame(liveSession.id);
    } catch (err) {
      error = errorMessage(err);
    } finally {
      pendingAction = null;
    }
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      onClose();
    }
  }

  function onBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) onClose();
  }
</script>

<div class="backdrop" onclick={onBackdropClick} role="presentation">
  <div
    data-testid="details-panel"
    class="panel"
    bind:this={panelEl}
    role="dialog"
    aria-modal="true"
    aria-label={subject.name}
    tabindex="-1"
    onkeydown={onKey}
  >
    <button data-testid="details-close" class="close" onclick={onClose} aria-label="Close">×</button>

    <div class="layout">
      <div class="cover">
        <Image url={coverLarge ?? coverSmall} alt={subject.name} placeholder="No cover" data-testid="details-cover" />
      </div>

      <div class="center-top">
        <h2>{subject.name}</h2>
        <p class="platform">{subject.platformName}</p>
        {#if liveSession}
          <span data-testid="details-playing-chip" class="chip">Playing</span>
        {/if}
        {#if rating}
          <p data-testid="details-rating" class="rating">{rating}</p>
        {/if}
        <p data-testid="details-genres" class="genres">{genres}</p>
        <p data-testid="details-description" class="description">{description}</p>
      </div>

      {#if screenshotUrls.length}
        <div class="shots" data-testid="details-screenshots">
          {#each screenshotUrls as url, i (url)}
            {#if !failedScreenshots[url]}
              <Image
                url={url}
                alt={`${subject.name} screenshot ${i + 1}`}
                data-testid={`details-screenshot-${i}`}
                onerror={() => markScreenshotFailed(url)}
              />
            {/if}
          {/each}
        </div>
      {:else}
        <p class="shots-empty" data-testid="details-no-screenshots">No screenshots available</p>
      {/if}

      <div class="center-bottom">
        {#if subject.romId === null}
          <p data-testid="details-no-id">This entry has no server id</p>
        {:else}
          <div class="action">
            {#if liveEntry}
              <button disabled>Installing…</button>
              <button data-testid="details-cancel" class="secondary" disabled={cancelPending} onclick={handleCancel}>
                {cancelPending ? 'Cancelling…' : 'Cancel'}
              </button>
            {:else if liveSession}
              <button data-testid="details-stop" disabled={pending} onclick={handleStop}>
                {pendingAction === 'stop' ? 'Stopping…' : 'Stop'}
              </button>
            {:else if installedNow}
              <button data-testid="details-play" disabled={pending} onclick={handlePlay}>
                {pendingAction === 'play' ? 'Launching…' : 'Play'}
              </button>
              <button
                data-testid="details-uninstall"
                class="secondary"
                class:confirm={confirmingUninstall}
                disabled={pending}
                onclick={handleUninstallClick}
              >
                {confirmingUninstall ? 'Confirm uninstall' : 'Uninstall'}
              </button>
              {#if buttons.update}
                <button
                  data-testid="details-install-update"
                  class="secondary"
                  disabled={contentActionKind !== null}
                  onclick={() => handleInstallContent('update')}
                >
                  {contentActionKind === 'update' ? 'Installing…' : 'Install Update'}
                </button>
              {/if}
              {#if buttons.dlc}
                <button
                  data-testid="details-install-dlc"
                  class="secondary"
                  disabled={contentActionKind !== null}
                  onclick={() => handleInstallContent('dlc')}
                >
                  {contentActionKind === 'dlc' ? 'Installing…' : 'Install DLC'}
                </button>
              {/if}
              {#if isNativeInstall}
                <button data-testid="details-game-settings" class="secondary" onclick={() => (showNativeSettings = true)}>
                  Game Settings
                </button>
              {/if}
            {:else}
              <button data-testid="details-install" disabled={pending} onclick={handleInstall}>
                {pendingAction === 'install' ? 'Installing…' : installLabel(subject.platformName)}
              </button>
            {/if}
          </div>

          {#if savePanelInfo?.supported || statePanelInfo?.supported}
            <div class="cloud-toggle">
              {#if savePanelInfo?.supported}
                <button
                  data-testid="details-cloud-save-toggle"
                  class:active={cloudMode === 'save'}
                  onclick={() => handleCloudToggle('save')}
                >
                  {cloudButtonLabel('save', savePanelInfo.scope)}
                </button>
              {/if}
              {#if statePanelInfo?.supported}
                <button
                  data-testid="details-cloud-state-toggle"
                  class:active={cloudMode === 'state'}
                  onclick={() => handleCloudToggle('state')}
                >
                  {cloudButtonLabel('state', statePanelInfo.scope)}
                </button>
              {/if}
            </div>
          {/if}

          {#if cloudPanelInfoError}
            <p data-testid="cloud-panel-info-error" class="error" role="alert">{cloudPanelInfoError}</p>
          {/if}

          {#if cloudMode !== 'overview' && activeCloudPanelInfo}
            <CloudPanel
              game={cloudGame}
              gameTitle={subject.name}
              saveType={cloudMode}
              panelInfo={activeCloudPanelInfo}
              {isNative}
              onBack={() => (cloudMode = 'overview')}
            />
          {/if}
        {/if}

        {#if error}
          <p data-testid="details-error" class="error" role="alert">{error}</p>
        {/if}
        {#if sessions.lastWarning}
          <p data-testid="details-warning" class="error warning" role="alert">
            {sessions.lastWarning}
            <button data-testid="details-warning-dismiss" class="dismiss" onclick={() => sessions.dismissWarning()} aria-label="Dismiss warning">×</button>
          </p>
        {/if}
      </div>
    </div>
  </div>
</div>

{#if showNativeSettings && subject.romId !== null}
  <NativeSettings
    romId={subject.romId}
    title={subject.name}
    onClose={() => (showNativeSettings = false)}
    onSaved={refreshInstalled}
  />
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: grid;
    place-items: center;
    z-index: 20;
  }

  /* The container for `.layout`'s breakpoint below. It has to be declared
     on the ANCESTOR, not on `.layout` itself: a container query never
     matches the element that declares the container, so styling `.layout`
     from a container declared on `.layout` would silently never apply. */
  .panel {
    container-type: inline-size;
    position: relative;
    width: min(1100px, calc(100vw - 48px));
    max-height: calc(100vh - 48px);
    overflow-y: auto;
    box-sizing: border-box;
    padding: 24px;
    border-radius: 12px;
    background: var(--bg);
    border: 1px solid var(--border);
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.35);
  }

  .panel:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  /* The panel's own content-box width (not the viewport) drives the
     breakpoint below, since the panel already scales down on its own via
     min(1100px, calc(100vw - 48px)) on narrow viewports. At full width that
     content box is 1100 - 2*24px padding = 1052px, comfortably over the
     900px threshold. */
  .layout {
    display: grid;
    grid-template-columns: 1fr;
    gap: 20px;
  }

  .close {
    position: absolute;
    top: 8px;
    right: 8px;
    width: 28px;
    height: 28px;
    line-height: 1;
    font-size: 20px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: var(--text);
    cursor: pointer;
  }

  .close:hover,
  .close:focus-visible {
    background: var(--border);
  }

  .cover {
    width: 100%;
    max-width: 240px;
    margin: 0 auto;
    aspect-ratio: 3 / 4;
    border-radius: 8px;
    overflow: hidden;
  }

  .cover :global(img) {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .center-top,
  .center-bottom {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    text-align: center;
  }

  h2 {
    margin: 0;
    color: var(--text-h);
    font-size: 20px;
  }

  .chip {
    align-self: center;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.03em;
    padding: 2px 10px;
    border-radius: 999px;
    background: var(--accent);
    color: #fff;
  }

  .platform {
    margin: 0;
    color: var(--text);
    font-size: 13px;
  }

  .rating {
    margin: 0;
    color: var(--text-h);
    font-weight: 600;
    font-size: 14px;
  }

  .genres {
    margin: 0;
    color: var(--text);
    font-size: 13px;
  }

  .description {
    margin: 0;
    color: var(--text);
    font-size: 14px;
    line-height: 1.5;
  }

  .shots {
    display: flex;
    overflow-x: auto;
    gap: 8px;
    padding-bottom: 4px;
  }

  .shots :global(img) {
    height: 120px;
    width: auto;
    flex: none;
    border-radius: 6px;
    object-fit: cover;
  }

  .shots-empty {
    margin: 0;
    color: var(--text);
    font-size: 13px;
    text-align: center;
  }

  .action {
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .action button {
    width: 100%;
    font: inherit;
    padding: 10px 16px;
    border-radius: 6px;
    border: none;
    background: var(--accent);
    color: #fff;
    cursor: pointer;
  }

  .action button.confirm {
    background: #e5484d;
  }

  .action button.secondary {
    padding: 8px 16px;
    background: transparent;
    color: var(--text);
    border: 1px solid var(--border);
  }

  .action button.secondary.confirm {
    background: transparent;
    color: #e5484d;
    border-color: #e5484d;
  }

  .cloud-toggle {
    margin-top: 4px;
    width: 100%;
    display: flex;
    gap: 8px;
  }

  .cloud-toggle button {
    flex: 1;
    font: inherit;
    padding: 8px 12px;
    border-radius: 6px;
    background: transparent;
    color: var(--text);
    border: 1px solid var(--border);
    cursor: pointer;
  }

  .cloud-toggle button.active {
    background: var(--accent);
    color: #fff;
    border-color: var(--accent);
  }

  .action button:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .error {
    margin: 0;
    color: #e5484d;
    font-size: 13px;
    text-align: center;
  }

  .error.warning {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
  }

  .dismiss {
    flex: none;
    width: 18px;
    height: 18px;
    line-height: 1;
    padding: 0;
    font-size: 14px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: #e5484d;
    cursor: pointer;
  }

  .dismiss:hover,
  .dismiss:focus-visible {
    background: var(--border);
  }

  /* Container-query overrides must come after every base rule they
     override (same specificity; source order decides the winner). */
  @container (min-width: 900px) {
    .layout {
      grid-template-columns: 240px 1fr 220px;
      grid-template-rows: auto auto;
      align-items: start;
    }

    .cover {
      grid-column: 1;
      grid-row: 1 / 3;
      max-width: none;
      margin: 0;
    }

    .center-top {
      grid-column: 2;
      grid-row: 1;
      text-align: left;
      align-items: flex-start;
    }

    .center-bottom {
      grid-column: 2;
      grid-row: 2;
      text-align: left;
      align-items: flex-start;
    }

    .chip {
      align-self: flex-start;
    }

    .shots,
    .shots-empty {
      grid-column: 3;
      grid-row: 1 / 3;
    }

    .shots {
      flex-direction: column;
      overflow-x: hidden;
      overflow-y: auto;
      max-height: min(70vh, 640px);
      padding-bottom: 0;
      padding-right: 4px;
    }

    .shots :global(img) {
      width: 100%;
      height: auto;
      flex: none;
    }
  }
</style>
