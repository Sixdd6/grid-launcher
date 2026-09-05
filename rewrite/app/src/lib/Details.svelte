<script lang="ts">
  import { listen } from '@tauri-apps/api/event';
  import {
    api,
    FIRMWARE_PASS_FINISHED_EVENT,
    type CloudPanelInfo,
    type ContentAvailability,
    type ContentKind,
    type DownloadStatus,
    type FirmwarePassFinished,
    type LaunchDefaults,
    type RomDetail,
  } from './api';
  import { downloads } from './stores/downloads.svelte';
  import { updates } from './stores/updates.svelte';
  import { isInstalled, installed, matchesInstalled, refresh as refreshInstalled } from './stores/installed.svelte';
  import { session } from './stores/session.svelte';
  import { sessions } from './stores/sessions.svelte';
  import Icon from './Icon.svelte';
  import Image from './Image.svelte';
  import NativeSettings from './details/NativeSettings.svelte';
  import OverviewTab from './details/OverviewTab.svelte';
  import MediaTab from './details/MediaTab.svelte';
  import MediaViewer from './details/MediaViewer.svelte';
  import SavesTab from './details/SavesTab.svelte';
  import FilesTab from './details/FilesTab.svelte';
  import { galleryItems } from './details/media';
  import { mergeDetail, summaryOf, type DetailsSubject } from './details/subject';
  import { isNativeExecutablePlatform, syntheticCloudGame, toggleCloudMode, type CloudMode } from './details/cloud';
  import { contentButtons, installLabel, isContentPlatform, isNativePlatform } from './details/actions';
  import { contentBlockReason } from './details/blocked';
  import {
    fileVersionLabel,
    romFileNamesFor,
    showsFilesVersionLine,
    versionLabel,
  } from './details/version';
  import {
    cloudStatusLabel,
    epochDate,
    flagList,
    headerLine,
    lastPlayedText,
    launchTargetLine,
    ratingValue,
    verificationLabel,
  } from './details/header';
  import {
    DETAILS_TABS,
    DETAILS_TAB_LABELS,
    rememberTab,
    rememberedTab,
    tabTestId,
    type DetailsTab,
  } from './details/tabs';
  import type { FirmwareChipState } from './server/header';

  let {
    subject,
    onClose,
    onLibraryPathUnset,
    initialCloudMode = 'overview',
  }: {
    subject: DetailsSubject;
    onClose: () => void;
    onLibraryPathUnset: () => void;
    /** Which cloud panel the popup opens with. A card's "Cloud sync" action
     *  passes `'save'`, which also selects the Saves tab. */
    initialCloudMode?: CloudMode;
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

  // Design §7: last tab remembered per session, not per game.
  let tab = $state<DetailsTab>(initialCloudMode === 'overview' ? rememberedTab() : 'saves');
  function selectTab(next: DetailsTab) {
    tab = next;
    rememberTab(next);
  }

  // Metadata overlay (task-10-brief.md): the subject carries whatever the
  // grid it opened from already had on hand; the full `RomDetail` fills in
  // the gaps. The redesigned tabs read `files`, `related`, the IGDB block
  // and the media fields off it, none of which the registry stores, so the
  // fetch now runs for every rom with a server id rather than only for a
  // subject with no screenshots. It stays a display overlay: `subject` is
  // the caller's data and is never mutated.
  let detail = $state<RomDetail | null>(null);
  let launchDefaults = $state<LaunchDefaults | null>(null);

  let merged = $derived(detail === null ? subject : mergeDetail(subject, detail));
  let coverSmall = $derived(merged.coverSmall);
  let coverLarge = $derived(merged.coverLarge);
  let screenshotUrls = $derived(merged.screenshotUrls);

  // The gallery lives here, not in MediaTab: the viewer is rendered above
  // the whole dialog, so both need the same list and the same indices.
  let mediaItems = $derived(
    galleryItems({
      title: subject.name,
      screenshotUrls,
      youtubeVideoId: detail?.youtube_video_id ?? '',
      videoPath: detail?.video_path ?? '',
    })
  );
  let viewerIndex = $state<number | null>(null);
  // One failure map for the Media tab AND the fullscreen viewer: the viewer
  // is rendered outside the tab (above the whole dialog), so a map owned by
  // either one would let the two disagree about which screenshot is dead.
  // Keyed by URL, exactly like OverviewTab's own `failedScreenshots`.
  let failedMedia = $state<Record<string, true>>({});
  function markMediaFailed(url: string) {
    failedMedia = { ...failedMedia, [url]: true };
  }
  let description = $derived(merged.description);
  let rating = $derived(merged.rating);
  let genres = $derived(merged.genres);

  $effect(() => {
    if (subject.romId === null) return; // no server id: nothing to overlay
    if (!session.connected) return;
    api
      .getRomDetail(subject.romId)
      .then((fetched) => {
        detail = fetched;
      })
      .catch(() => {}); // offline/removed rom: the subject's own data stands
  });

  $effect(() => {
    api
      .getLaunchDefaults()
      .then((d) => (launchDefaults = d))
      .catch(() => {}); // unreadable config: the emulator row says "No default emulator"
  });

  // Design §7 Overview: Related is filtered against the platform's own game
  // list. Fetched once the detail names the platform id; a failure leaves
  // the list empty, which renders no Related row at all rather than a row
  // of titles the user may not have.
  let serverTitles = $state<string[]>([]);
  let firmware = $state<FirmwareChipState>(null);
  let firmwarePending = $state(false);

  $effect(() => {
    const platformId = detail?.platform_id ?? null;
    if (platformId === null || !session.connected) return;
    if (detail !== null && detail.related.length === 0) return; // nothing to filter
    let cancelled = false;
    api
      .listGames(platformId)
      .then((games) => {
        if (!cancelled) serverTitles = games.map((g) => g.name);
      })
      .catch(() => {}); // offline/refused: no Related row rather than a wrong one
    return () => {
      cancelled = true;
    };
  });

  // One firmware status read. The sequence guard drops an answer that a
  // newer read has already superseded, so the chip never steps backwards
  // when the pass-finished refetch overtakes the first fetch.
  let firmwareSeq = 0;
  function refreshFirmware(platformId: number) {
    const seq = ++firmwareSeq;
    api
      .platformFirmwareStatus(platformId, subject.platformName)
      .then((status) => {
        if (seq === firmwareSeq) firmware = status;
      })
      .catch(() => {
        // Refused or unreachable. "unavailable" is the honest chip: sitting
        // at "checking…" forever would claim a call is still in flight.
        if (seq === firmwareSeq) firmware = 'unavailable';
      });
  }

  $effect(() => {
    const platformId = detail?.platform_id ?? null;
    if (platformId === null || !session.connected) return;
    refreshFirmware(platformId);
  });

  // The pass runs in the background and answers with one event; the button
  // stays disabled until it lands, whether or not anything was fetched. The
  // status is then read again, since a pass can change what the server side
  // reports.
  $effect(() => {
    const unlisten = listen<FirmwarePassFinished>(FIRMWARE_PASS_FINISHED_EVENT, (e) => {
      const platformId = detail?.platform_id ?? null;
      if (platformId === null || e.payload.platform_id !== platformId) return;
      firmwarePending = false;
      refreshFirmware(platformId);
    });
    return () => {
      void unlisten.then((off) => off());
    };
  });

  async function installFirmware() {
    const platformId = detail?.platform_id ?? null;
    if (platformId === null) return;
    error = null;
    firmwarePending = true;
    try {
      await api.installFirmwareForPlatform(platformId, subject.platformName);
    } catch (err) {
      error = errorMessage(err);
      firmwarePending = false;
    }
  }

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

  // The primary button's reason needs the configured emulator list, so it
  // comes from the backend; a failure leaves it blank rather than guessing.
  let installBlocked = $state('');
  $effect(() => {
    if (installedNow) {
      installBlocked = '';
      return;
    }
    const platform = subject.platformName;
    api
      .installBlockReason(platform)
      .then((reason) => (installBlocked = reason))
      .catch(() => (installBlocked = ''));
  });

  let updateBlocked = $derived(
    contentBlockReason('update', subject.platformName, installedNow, subject.romId, buttons.update)
  );
  let dlcBlocked = $derived(
    contentBlockReason('dlc', subject.platformName, installedNow, subject.romId, buttons.dlc)
  );

  // Fetched once the subject is installed-and-a-content-platform, and
  // re-fetched right after a live install for it finishes (`wasLive` tracks
  // the previous liveEntry-defined-ness across effect runs).
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
  // subject.
  let installedRow = $derived(installed.list.find((row) => matchesInstalled(row, summary, subject.platformName)) ?? null);
  let cloudGame = $derived(installedRow ?? syntheticCloudGame(summary, subject.platformName));
  let isNative = $derived(isNativeExecutablePlatform(subject.platformName));
  // `''` from `launchTargetLine` means "no launch target to state" — today
  // only a native platform, whose game runs its own executable.
  let launchTarget = $derived(launchTargetLine(launchDefaults, subject.platformName));

  // Server-side game updates (doc 10). `updateLabel` is null when the rom
  // has no update, which also hides the button. `version` is the header row:
  // the version tag parsed out of the file name for Windows/PC, else the raw
  // revision.
  let updateLabel = $derived(installedNow ? updates.labelFor(subject.romId) : null);
  let version = $derived(
    versionLabel(
      subject.platformName,
      romFileNamesFor(subject.source, installedRow?.rom_file_name ?? '', detail?.fs_name ?? ''),
      detail?.revision || installedRow?.revision || ''
    )
  );

  // D-UI-10, per side. The server side reads the top-level file's own
  // timestamp; the installed side has no server timestamp to fall back on,
  // so it falls back to when the install landed. Those two fallbacks are
  // different quantities, so the whole comparison is gated to PC platforms
  // (`showsFilesVersionLine`); off PC both sides are '' and `FilesTab`
  // drops the line.
  let versionLineShown = $derived(showsFilesVersionLine(subject.platformName));
  let topLevelFile = $derived(detail?.files.find((f) => f.is_top_level) ?? detail?.files[0] ?? null);
  let serverVersion = $derived(
    versionLineShown && detail
      ? fileVersionLabel(detail.fs_name, topLevelFile?.last_modified ?? '')
      : ''
  );
  let installedVersion = $derived(
    versionLineShown && installedRow
      ? fileVersionLabel(installedRow.rom_file_name, '') || epochDate(installedRow.installed_at)
      : ''
  );

  let confirmingUpdate = $state(false);
  let updateToast = $state<string | null>(null);
  let updatePending = $state(false);

  // Last seen status per update entry for this rom. Deliberately NOT `$state`.
  const seenUpdateStatus = new Map<number, DownloadStatus>();

  // Toast on completion, not on click.
  $effect(() => {
    for (const entry of downloads.entries) {
      if (entry.rom_id !== subject.romId) continue;
      if (entry.kind !== 'update' && entry.kind !== 'native_update') continue;
      const previous = seenUpdateStatus.get(entry.id);
      seenUpdateStatus.set(entry.id, entry.status);
      if (previous !== undefined && previous !== 'completed' && entry.status === 'completed') {
        updateToast = `Updated '${subject.name}' successfully.`;
      }
    }
  });

  let cloudMode = $state<CloudMode>(initialCloudMode);
  let savePanelInfo = $state<CloudPanelInfo | null>(null);
  let statePanelInfo = $state<CloudPanelInfo | null>(null);
  let cloudPanelInfoError = $state<string | null>(null);

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

  /** The left column's cloud button: go to the Saves tab and open a panel. */
  function openCloud() {
    selectTab('saves');
    if (cloudMode === 'overview') {
      cloudMode = savePanelInfo?.supported ? 'save' : statePanelInfo?.supported ? 'state' : 'overview';
    }
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

  // Two-click confirm for native installs only (doc 10).
  async function handleUpdateClick() {
    if (subject.romId === null) return;
    if (isNativeInstall && !confirmingUpdate) {
      confirmingUpdate = true;
      return;
    }
    error = null;
    updatePending = true;
    try {
      await api.updateGame(subject.romId);
    } catch (err) {
      error = errorMessage(err);
    } finally {
      updatePending = false;
      confirmingUpdate = false;
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

  let header = $derived(
    headerLine({
      platformName: subject.platformName,
      firstReleaseDate: detail?.first_release_date ?? '',
      companies: detail?.companies ?? '',
      genres,
    })
  );
  let ratingNumber = $derived(ratingValue(rating));
  // `||`, not `??`: a loaded detail always carries strings, so `''` from the
  // server would win over the registry's stored value under `??`.
  let flags = $derived([
    ...flagList(detail?.regions || installedRow?.regions || ''),
    ...flagList(detail?.languages || installedRow?.languages || ''),
  ]);
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
    <button data-testid="details-close" class="close icon-btn" onclick={onClose} aria-label="Close">
      <Icon name="close" size={20} />
    </button>

    <div class="layout">
      <aside class="left">
        <div class="cover">
          <Image url={coverLarge ?? coverSmall} alt={subject.name} placeholder="No cover" data-testid="details-cover" />
        </div>

        {#if subject.romId === null}
          <p data-testid="details-no-id">This entry has no server id</p>
        {:else}
          <div class="action">
            {#if liveEntry}
              <button disabled>Installing…</button>
              <!-- `cancel_for_rom` leaves a finalizing entry alone —
                   extraction is not cancellable — so the button is disabled
                   rather than silently doing nothing while it runs. -->
              <button
                data-testid="details-cancel"
                class="secondary"
                disabled={cancelPending || liveEntry.status === 'installing'}
                title={liveEntry.status === 'installing' ? 'Installing — this step cannot be cancelled' : undefined}
                onclick={handleCancel}
              >
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
              {#if updateLabel !== null}
                <button
                  data-testid="details-update"
                  class="update"
                  class:confirm={confirmingUpdate}
                  disabled={pending || updatePending || liveEntry !== undefined}
                  onclick={handleUpdateClick}
                >
                  {updatePending
                    ? 'Updating…'
                    : confirmingUpdate
                      ? 'Saves and configuration will be preserved — confirm update'
                      : updateLabel}
                </button>
              {/if}
              {#if buttons.update}
                <button
                  data-testid="details-install-update"
                  class="secondary"
                  title={updateBlocked}
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
                  title={dlcBlocked}
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
              <button
                data-testid="details-uninstall"
                class="secondary"
                class:confirm={confirmingUninstall}
                disabled={pending}
                onclick={handleUninstallClick}
              >
                {confirmingUninstall ? 'Confirm uninstall' : 'Uninstall'}
              </button>
            {:else}
              <button data-testid="details-install" title={installBlocked} disabled={pending} onclick={handleInstall}>
                {pendingAction === 'install' ? 'Installing…' : installLabel(subject.platformName)}
              </button>
            {/if}
            <button data-testid="details-cloud-status" class="secondary" onclick={openCloud}>
              {cloudStatusLabel(savePanelInfo?.supported === true, statePanelInfo?.supported === true)}
            </button>
          </div>

          {#if updateToast}
            <p data-testid="details-update-toast" class="hint" role="status">{updateToast}</p>
          {/if}
        {/if}

        <p class="meta-line" data-testid="details-last-played">
          {lastPlayedText(installedRow?.last_played_at ?? 0)}
        </p>
        {#if launchTarget !== ''}
          <p class="meta-line" data-testid="details-emulator">{launchTarget}</p>
        {/if}
      </aside>

      <section class="right">
        <header class="head">
          <h2>{subject.name}</h2>
          <p class="header-line" data-testid="details-header-line">{header}{#if ratingNumber !== ''}{#if header !== ''}{' · '}{/if}<span class="rating"><span class="star"><Icon name="star" size={14} /></span>{ratingNumber}</span>{/if}</p>
          <div class="chips">
            {#if liveSession}
              <span data-testid="details-playing-chip" class="chip playing">Playing</span>
            {/if}
            <!-- Only once the server's answer is in hand: before that (no
                 server id, offline, or the first frames of any open) the
                 chip would assert "Unidentified" the app cannot know. -->
            {#if detail}
              <span class="chip" data-testid="details-verification">
                {verificationLabel(detail.is_identified)}
              </span>
            {/if}
            {#if flags.length}
              <span class="chip" data-testid="details-flags">{flags.join(' · ')}</span>
            {/if}
            {#if version}
              <span class="chip" data-testid="details-version">{version}</span>
            {/if}
          </div>
        </header>

        <div class="tabs" role="tablist">
          {#each DETAILS_TABS as name (name)}
            <button
              role="tab"
              data-testid={tabTestId(name)}
              class:active={tab === name}
              aria-selected={tab === name}
              onclick={() => selectTab(name)}
            >
              {DETAILS_TAB_LABELS[name]}
            </button>
          {/each}
        </div>

        <div class="tabpanel" role="tabpanel">
          {#if tab === 'overview'}
            <OverviewTab name={subject.name} {description} {screenshotUrls} {detail} {serverTitles} />
          {:else if tab === 'media'}
            <MediaTab
              items={mediaItems}
              onOpen={(i) => (viewerIndex = i)}
              failed={failedMedia}
              onScreenshotError={markMediaFailed}
            />
          {:else if tab === 'saves'}
            <SavesTab
              gameTitle={subject.name}
              {cloudGame}
              {isNative}
              {savePanelInfo}
              {statePanelInfo}
              {cloudMode}
              infoError={cloudPanelInfoError}
              onToggle={handleCloudToggle}
              onBack={() => (cloudMode = 'overview')}
            />
          {:else}
            <FilesTab
              files={detail?.files ?? []}
              {installedVersion}
              {serverVersion}
              {installedNow}
              {firmware}
              onInstallFirmware={firmwarePending ? null : installFirmware}
            />
          {/if}
        </div>

        {#if error}
          <p data-testid="details-error" class="error" role="alert">{error}</p>
        {/if}
        {#if sessions.lastWarning}
          <p data-testid="details-warning" class="error warning" role="alert">
            {sessions.lastWarning}
            <button data-testid="details-warning-dismiss" class="dismiss icon-btn" onclick={() => sessions.dismissWarning()} aria-label="Dismiss warning">
              <Icon name="close" size={14} />
            </button>
          </p>
        {/if}
      </section>
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

{#if viewerIndex !== null}
  <MediaViewer
    items={mediaItems}
    index={viewerIndex}
    onIndex={(i) => (viewerIndex = i)}
    failed={failedMedia}
    onScreenshotError={markMediaFailed}
    onClose={() => {
      viewerIndex = null;
      // The viewer took focus off the panel; without handing it back, the
      // popup's own Escape handler would no longer hear the next press.
      panelEl?.focus();
    }}
  />
{/if}

<style>
  /* Design §7: dimmed AND blurred shell behind the dialog. */
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    backdrop-filter: blur(6px);
    display: grid;
    place-items: center;
    z-index: 20;
  }

  .panel {
    position: relative;
    width: min(1040px, calc(100vw - 48px));
    height: min(680px, calc(100vh - 48px));
    box-sizing: border-box;
    padding: 24px;
    border-radius: 12px;
    background: var(--bg);
    border: 1px solid var(--border);
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.35);
  }

  .panel:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: 2px;
  }

  /* The left column is fixed at design §7's 240px; only the right column's
     tab panel scrolls, so the cover and the actions never leave the view. */
  .layout {
    display: grid;
    grid-template-columns: 240px 1fr;
    gap: 24px;
    height: 100%;
    min-height: 0;
  }

  .left {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-height: 0;
    overflow-y: auto;
  }

  .right {
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-height: 0;
    min-width: 0;
  }

  /* Box, radius and reset come from `.icon-btn` in app.css. Only the
     placement and the colour are this dialog's own. */
  .close {
    position: absolute;
    top: 8px;
    right: 8px;
    color: var(--text);
  }

  .cover {
    width: 100%;
    aspect-ratio: 3 / 4;
    border-radius: var(--r-row);
    overflow: hidden;
    flex: none;
  }

  .cover :global(img) {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  h2 {
    margin: 0;
    color: var(--text-h);
    font-size: 20px;
  }

  .header-line {
    margin: 0;
    color: var(--text-muted);
    font-size: 13px;
    overflow-wrap: anywhere;
  }

  /* The rating is the one part of the header line that is not muted: the old
     app drew it in the accent colour so it stood out from the metadata
     around it. The star takes `--primary` (the accent the old app used for
     the rating); the NUMBER takes `--text-h`, because `--warning` is the same
     amber in both themes and would fall below a readable contrast on the
     light background. */
  .rating {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    color: var(--text-h);
  }

  .star {
    display: flex;
    color: var(--primary);
  }

  .head {
    display: flex;
    flex-direction: column;
    gap: 6px;
    flex: none;
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .chip {
    font-size: 11px;
    font-weight: 600;
    padding: 2px 10px;
    border-radius: var(--r-pill);
    background: var(--surface);
    color: var(--text);
    border: 1px solid var(--border);
  }

  .chip.playing {
    background: var(--primary);
    border-color: var(--primary);
    color: #fff;
    text-transform: uppercase;
    letter-spacing: 0.03em;
  }

  .tabs {
    display: flex;
    gap: 4px;
    border-bottom: 1px solid var(--border);
    flex: none;
  }

  .tabs button {
    font: inherit;
    padding: 8px 14px;
    border: none;
    border-bottom: 2px solid transparent;
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
    transition:
      color var(--m-fast) ease,
      border-color var(--m-fast) ease;
  }

  .tabs button.active {
    color: var(--text-h);
    border-bottom-color: var(--primary);
  }

  .tabpanel {
    flex: 1;
    min-height: 0;
    min-width: 0;
    overflow-y: auto;
    padding-right: 4px;
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
    border-radius: var(--r-control);
    border: none;
    background: var(--primary);
    color: #fff;
    cursor: pointer;
    transition: background var(--m-fast) ease;
  }

  .action button:hover:not(:disabled) {
    background: var(--primary-hover);
  }

  .action button.confirm {
    background: var(--danger);
  }

  .action button.secondary {
    padding: 8px 16px;
    background: transparent;
    color: var(--text);
    border: 1px solid var(--border);
  }

  .action button.secondary:hover:not(:disabled) {
    background: var(--surface);
  }

  .action button.secondary.confirm {
    background: transparent;
    color: var(--danger);
    border-color: var(--danger);
  }

  /* The update confirm is a caution, not a destruction: it keeps the
     two-click shape but takes the warning amber instead of `.confirm`'s
     red, which would contradict the label's "will be preserved". */
  .action button.update.confirm {
    background: var(--warning);
    color: #16171d;
  }

  .action button:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .meta-line {
    margin: 0;
    color: var(--text-muted);
    font-size: 12px;
  }

  .hint {
    margin: 0;
    color: var(--text);
    opacity: 0.75;
    font-size: 13px;
  }

  .error {
    margin: 0;
    color: var(--danger);
    font-size: 13px;
    flex: none;
  }

  .error.warning {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  /* Was 18×18 — below the minimum pointer target. `.icon-btn` makes it
     28×28; the 14px icon inside keeps it visually small next to the 13px
     warning text. */
  .dismiss {
    color: var(--danger);
  }
</style>
