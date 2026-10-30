<script lang="ts">
  import {
    api,
    type CloudPanelInfo,
    type ContentAvailability,
    type ContentKind,
    type DownloadStatus,
    type GameSummary,
    type LaunchDefaults,
    type RomDetail,
    type SaveType,
  } from './api';
  import { subjectFromDetails } from './background';
  import { noteViewed } from './stores/lastViewed.svelte';
  import { downloads } from './stores/downloads.svelte';
  import { updates } from './stores/updates.svelte';
  import { isInstalled, installed, matchesInstalled, refresh as refreshInstalled } from './stores/installed.svelte';
  import { session } from './stores/session.svelte';
  import { sessions } from './stores/sessions.svelte';
  import Icon from './Icon.svelte';
  import Image from './Image.svelte';
  import NativeSettings from './details/NativeSettings.svelte';
  import UmuInstallOffer from './UmuInstallOffer.svelte';
  import { isUmuMissing } from './launchError';
  import OverviewTab from './details/OverviewTab.svelte';
  import MediaTab from './details/MediaTab.svelte';
  import MediaViewer from './details/MediaViewer.svelte';
  import SavesTab from './details/SavesTab.svelte';
  import AchievementsTab from './details/AchievementsTab.svelte';
  import LinkPicker from './details/LinkPicker.svelte';
  import RemoveConfirm from './details/RemoveConfirm.svelte';
  import { keptFolder, showsUnlinkedActions } from './details/unlinked';
  import { missingActions } from './library/missing';
  import { pickFolder } from './pickers';
  import { pushToast } from './stores/toasts.svelte';
  import { SKIPPED_TOAST_MS, skippedRemovalText } from './uninstallSkips';
  import { fullIndex, galleryItems, viewableIndex, viewableItems } from './details/media';
  import { mergeDetail, summaryOf, type DetailsSubject } from './details/subject';
  import { syntheticCloudGame, type CloudMode } from './details/cloud';
  import { contentButtons, installLabel, isContentPlatform, isNativePlatform } from './details/actions';
  import { contentBlockReason } from './details/blocked';
  import {
    fileVersionLabel,
    romFileNamesFor,
    showsFilesVersionLine,
    versionLabel,
  } from './details/version';
  import {
    epochDate,
    flagList,
    headerLine,
    lastPlayedText,
    launchTargetLine,
    ratingValue,
    verificationLabel,
  } from './details/header';
  import {
    DETAILS_TAB_LABELS,
    rememberTab,
    rememberedTab,
    shownTab,
    tabTestId,
    visibleTabs,
    type DetailsTab,
  } from './details/tabs';

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
  // The error that offers the umu-launcher install (a Proton launch with no
  // umu-run); `null` for any other. Compared by value, so a later action's
  // own error never shows the offer.
  let umuOfferFor = $state<string | null>(null);
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

  // Achievements shows only for a ROM whose RomM detail lists achievements;
  // a remembered Achievements choice falls back to Overview elsewhere.
  let tabs = $derived(visibleTabs(detail?.achievement_count));
  let activeTab = $derived(shownTab(tab, tabs));

  // The grid already reported what IT knew when the popup opened (a summary
  // has a cover and, since round 4, the list payload's screenshots); once the
  // detail lands, the merged subject is strictly richer — it is the only
  // place fanart is ever known. Reporting again is idempotent when nothing
  // changed, because `noteViewed` just replaces the subject.
  $effect(() => {
    noteViewed(subjectFromDetails(merged));
  });
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
  // Which picture the viewer is on, held as an index into `mediaItems` — the
  // FULL list, not the filtered one. A screenshot that fails while the viewer
  // is open shortens `viewerItems` under it, and a position in that shrinking
  // list would silently address a different picture; a full-list index cannot,
  // because `mediaItems` only changes when the game does. `null` means the
  // viewer is closed.
  let viewerAnchor = $state<number | null>(null);
  // One failure map for the Media tab AND the fullscreen viewer: the viewer
  // is rendered outside the tab (above the whole dialog), so a map owned by
  // either one would let the two disagree about which screenshot is dead.
  // Keyed by URL, exactly like OverviewTab's own `failedScreenshots`.
  let failedMedia = $state<Record<string, true>>({});
  function markMediaFailed(url: string) {
    failedMedia = { ...failedMedia, [url]: true };
  }
  // The viewer walks the same list the gallery shows, so a screenshot whose
  // image 404s is skipped instead of paged onto. `viewerIndex` is the anchor's
  // position in THAT list: `viewableIndex` moves to the next viewable item
  // (wrapping) when the anchor itself has died, and answers `null` when
  // nothing is left, which unmounts the viewer.
  let viewerItems = $derived(viewableItems(mediaItems, failedMedia));
  let viewerIndex = $derived(
    viewerAnchor === null ? null : viewableIndex(mediaItems, failedMedia, viewerAnchor)
  );
  $effect(() => {
    // The viewer closed itself because the last viewable item died. Forget the
    // anchor and hand focus back, exactly as the close button does.
    if (viewerAnchor !== null && viewerIndex === null) {
      viewerAnchor = null;
      panelEl?.focus();
    }
  });
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
  // One native predicate for the whole popup (user ruling 2026-09-08): the
  // install label, the native Game Settings button, and the Saves tab's PC
  // save-folder mode all read the same answer.
  // The server platform slug decides first (user bug 2026-10-10: a custom
  // display name on slug `win` is still native). A registry row the
  // backfill has not reached gets it from the ROM detail (`mergeDetail`).
  let platformSlug = $derived(merged.platformSlug);
  let isNative = $derived(isNativePlatform(platformSlug, subject.platformName));
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
    const slug = platformSlug;
    api
      .installBlockReason(platform, slug)
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
  // A row the slug backfill has not reached borrows the slug the ROM detail
  // carries, so the cloud commands see the same nativeness as the popup.
  let cloudGame = $derived(
    installedRow
      ? { ...installedRow, platform_slug: installedRow.platform_slug || platformSlug }
      : syntheticCloudGame(summary, subject.platformName, platformSlug)
  );

  // Q4: an installed row with no server rom id. Details offers "Link to
  // server game" and "Remove from library (keeps files)" for it instead of
  // the server-backed actions.
  let unlinkedRow = $derived(showsUnlinkedActions(subject.romId, installedRow) ? installedRow : null);
  let heldRomIds = $derived(
    new Set(installed.list.flatMap((row) => (row.rom_id === null ? [] : [row.rom_id]))),
  );
  let showLinkPicker = $state(false);
  let showRemoveConfirm = $state(false);

  async function linkTo(rom: GameSummary) {
    if (unlinkedRow === null) return;
    await api.linkInstalledRow(unlinkedRow.title, unlinkedRow.platform, rom.id);
    showLinkPicker = false;
    // The opener sees the row gain its rom id in the refreshed list and
    // re-opens Details on it (Library.svelte).
    await refreshInstalled();
  }

  // 9c: the row's files are not on disk (the backend's reconcile pass sets
  // the flag on every listing). Details offers Locate… and Remove from
  // library (keeps files); Play and Uninstall are disabled with the reason.
  let missing = $derived(missingActions(installedRow));
  let locatePath = $state('');
  let locateError = $state<string | null>(null);
  let locatePending = $state(false);

  // The row the Remove confirm acts on: an unlinked row (Q4) or a missing one.
  let removeTarget = $derived(unlinkedRow ?? (missing.showMissing ? installedRow : null));

  async function locate(folder: string) {
    if (installedRow === null || folder.trim() === '') return;
    locateError = null;
    locatePending = true;
    try {
      await api.locateInstalledRow(installedRow.title, installedRow.platform, folder.trim());
      locatePath = '';
      await refreshInstalled();
    } catch (err) {
      locateError = errorMessage(err);
    } finally {
      locatePending = false;
    }
  }

  async function locateWithPicker() {
    const folder = await pickFolder('Locate the game folder');
    if (folder !== null) await locate(folder);
  }

  async function removeFromLibrary() {
    if (removeTarget === null) return;
    await api.removeFromLibrary(removeTarget.title, removeTarget.platform);
    showRemoveConfirm = false;
    await refreshInstalled();
    onClose();
  }
  // `''` from `launchTargetLine` means "no launch target to state" — today
  // only a native platform, whose game runs its own executable.
  let launchTarget = $derived(launchTargetLine(launchDefaults, subject.platformName, platformSlug));

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
  // (`showsFilesVersionLine`); off PC both sides are '' and the Overview
  // drops the Version fact.
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

  /** The kind the Saves tab shows; a card's "Cloud sync" opens on saves. */
  let cloudKind = $state<SaveType>(initialCloudMode === 'state' ? 'state' : 'save');
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
      const report = await api.uninstallGame(subject.romId);
      pushToast(skippedRemovalText(report?.skipped), 'error', SKIPPED_TOAST_MS);
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
    if (isNative && !confirmingUpdate) {
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
      umuOfferFor = isUmuMissing(err) ? error : null;
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

        {#if missing.showMissing}
          <!-- A notice, not a plain action list: danger edge, Locate… as the one
               primary action, the typed path as a secondary group, and the
               destructive choice last. DOM order is the Tab order. -->
          <div class="action notice" data-testid="details-missing" role="group" aria-labelledby="details-missing-title">
            <span id="details-missing-title" class="chip unlinked missing">Files not found</span>
            <p class="meta-line">{missing.playBlocked}</p>
            <button data-testid="details-locate" disabled={locatePending} onclick={locateWithPicker}>
              {locatePending ? 'Locating…' : 'Locate…'}
            </button>
            <!-- The typed path is the same action as the picker: the picker
                 cannot open in every environment (pickers.ts). -->
            <div class="locate-path">
              <input
                data-testid="details-locate-path"
                type="text"
                placeholder="Or type the folder that holds the game"
                aria-label="Folder that holds the game"
                aria-invalid={locateError ? 'true' : undefined}
                aria-describedby={locateError ? 'details-locate-error' : undefined}
                bind:value={locatePath}
              />
              <button
                data-testid="details-locate-submit"
                class="secondary"
                disabled={locatePending || locatePath.trim() === ''}
                onclick={() => locate(locatePath)}
              >
                Use this folder
              </button>
            </div>
            {#if locateError}
              <p id="details-locate-error" data-testid="locate-error" class="error" role="alert">{locateError}</p>
            {/if}
            <button
              data-testid="details-missing-remove"
              class="secondary destructive"
              onclick={() => (showRemoveConfirm = true)}
            >
              Remove from library (keeps files)
            </button>
          </div>
        {/if}

        {#if subject.romId === null}
          {#if unlinkedRow}
            <div class="action" data-testid="details-unlinked">
              <span class="chip unlinked" data-testid="details-unlinked-note">Not linked to a server game</span>
              <button
                data-testid="details-link-server"
                disabled={!session.connected}
                aria-describedby={session.connected ? undefined : 'details-link-offline'}
                title={session.connected ? undefined : 'Connect to a server to link this game'}
                onclick={() => (showLinkPicker = true)}
              >
                Link to server game
              </button>
              {#if !session.connected}
                <!-- The button's `title` is hover-only; this line is the visible reason. -->
                <p id="details-link-offline" class="meta-line offline-note">Connect to a server to link this game.</p>
              {/if}
              <!-- A row that is also missing already offers Remove in the notice
                   above: one Remove button, not two. -->
              {#if !missing.showMissing}
                <button
                  data-testid="details-remove-from-library"
                  class="secondary destructive"
                  onclick={() => (showRemoveConfirm = true)}
                >
                  Remove from library (keeps files)
                </button>
              {/if}
            </div>
          {:else}
            <p data-testid="details-no-id">This entry has no server id</p>
          {/if}
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
              <button
                data-testid="details-play"
                disabled={pending || missing.playBlocked !== ''}
                aria-describedby={missing.playBlocked !== '' ? 'details-blocked-note' : undefined}
                title={missing.playBlocked || undefined}
                onclick={handlePlay}
              >
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
              {#if isNative}
                <button data-testid="details-game-settings" class="secondary" onclick={() => (showNativeSettings = true)}>
                  Game Settings
                </button>
              {/if}
              <button
                data-testid="details-uninstall"
                class="secondary"
                class:confirm={confirmingUninstall}
                disabled={pending || missing.uninstallBlocked !== ''}
                aria-describedby={missing.uninstallBlocked !== '' ? 'details-blocked-note' : undefined}
                title={missing.uninstallBlocked || undefined}
                onclick={handleUninstallClick}
              >
                {confirmingUninstall ? 'Confirm uninstall' : 'Uninstall'}
              </button>
              {#if missing.playBlocked !== '' || missing.uninstallBlocked !== ''}
                <!-- The buttons' `title` is hover-only; this line is the visible reason. -->
                <p id="details-blocked-note" class="meta-line offline-note">Play and Uninstall are unavailable until the files are found.</p>
              {/if}
            {:else}
              <button data-testid="details-install" title={installBlocked} disabled={pending} onclick={handleInstall}>
                {pendingAction === 'install' ? 'Installing…' : installLabel(platformSlug, subject.platformName)}
              </button>
            {/if}
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

        <!-- A click does not move focus to the tab (mouse-down is cancelled):
             WebKitGTK drew the keyboard focus ring on a clicked tab. The
             Tab key still focuses it. -->
        <div class="tabs" role="tablist">
          {#each tabs as name (name)}
            <button
              role="tab"
              data-testid={tabTestId(name)}
              class:active={activeTab === name}
              aria-selected={activeTab === name}
              onclick={() => selectTab(name)}
              onmousedown={(e) => e.preventDefault()}
            >
              {DETAILS_TAB_LABELS[name]}
            </button>
          {/each}
        </div>

        <div class="tabpanel" role="tabpanel">
          {#if activeTab === 'overview'}
            <OverviewTab {description} {detail} {serverTitles} {installedNow} {installedVersion} {serverVersion} />
          {:else if activeTab === 'media'}
            <MediaTab
              items={mediaItems}
              onOpen={(i) => (viewerAnchor = i)}
              failed={failedMedia}
              onScreenshotError={markMediaFailed}
              coverUrl={coverLarge ?? coverSmall}
            />
          {:else if activeTab === 'saves'}
            <SavesTab
              gameTitle={subject.name}
              {cloudGame}
              {isNative}
              {savePanelInfo}
              {statePanelInfo}
              kind={cloudKind}
              infoError={cloudPanelInfoError}
              onKind={(kind) => (cloudKind = kind)}
            />
          {:else if activeTab === 'achievements' && subject.romId !== null}
            <AchievementsTab romId={subject.romId} />
          {/if}
        </div>

        {#if error}
          <p data-testid="details-error" class="error" role="alert">{error}</p>
          {#if error === umuOfferFor}<UmuInstallOffer />{/if}
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
    platform={subject.platformName}
    {platformSlug}
    onClose={() => (showNativeSettings = false)}
    onSaved={refreshInstalled}
  />
{/if}

{#if showLinkPicker && unlinkedRow}
  <LinkPicker
    title={unlinkedRow.title}
    platform={unlinkedRow.platform}
    {heldRomIds}
    onPick={linkTo}
    onClose={() => {
      showLinkPicker = false;
      panelEl?.focus();
    }}
  />
{/if}

{#if showRemoveConfirm && removeTarget}
  <RemoveConfirm
    title={removeTarget.title}
    folder={keptFolder(removeTarget)}
    onConfirm={removeFromLibrary}
    onClose={() => {
      showRemoveConfirm = false;
      panelEl?.focus();
    }}
  />
{/if}

{#if viewerIndex !== null}
  <MediaViewer
    items={viewerItems}
    index={viewerIndex}
    onIndex={(i) => (viewerAnchor = fullIndex(mediaItems, failedMedia, i))}
    onScreenshotError={markMediaFailed}
    coverUrl={coverLarge ?? coverSmall}
    onClose={() => {
      viewerAnchor = null;
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
    background: var(--scrim);
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
    border-radius: var(--r-dialog);
    /* The overlay level (look-and-feel plan 2026-10-30): opaque, with a
       shadow over the scrim, and no coloured edge. */
    background: var(--overlay);
    border: 1px solid var(--line);
    box-shadow: var(--shadow-overlay);
  }

  /* The dialog takes focus when it opens so the keyboard lands inside it;
     the dialog itself is not a control, so it draws no ring. Its controls
     draw theirs. */
  .panel:focus-visible {
    outline: none;
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
     placement and the colour are this dialog's own. Hover/focus repeats
     app.css's `--primary` because this scoped `color` would otherwise win
     over the global hover rule at equal specificity. */
  .close {
    position: absolute;
    top: 8px;
    right: 8px;
    color: var(--text);
  }

  .close:hover,
  .close:focus-visible {
    color: var(--primary);
  }

  /* The whole cover at its own shape, never cropped: the box shrinks to the
     image, capped at the column's width and a portrait cover's height, so
     a square or wide cover leaves no gap above the actions. */
  .cover {
    display: flex;
    justify-content: center;
    flex: none;
  }

  .cover :global(img) {
    display: block;
    max-width: 100%;
    max-height: 320px;
    border-radius: var(--r-card);
    box-shadow: var(--shadow-lift);
  }

  .cover :global(.skeleton),
  .cover :global(.placeholder) {
    width: 100%;
    aspect-ratio: 3 / 4;
    height: auto;
    border-radius: var(--r-card);
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
    color: var(--on-primary);
    text-transform: uppercase;
    letter-spacing: 0.03em;
  }

  /* Status chip for an installed row with no server id. Same pill as the
     header chips; the amber dot (decorative, the text carries the meaning)
     marks it as a state to resolve rather than a fact about the game. In the
     left column it sits centred above the two actions. */
  .chip.unlinked {
    align-self: center;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: var(--text-h);
    text-align: center;
  }

  .chip.unlinked::before {
    content: '';
    width: 7px;
    height: 7px;
    flex: none;
    border-radius: 50%;
    background: var(--warning);
  }

  .chip.unlinked.missing::before {
    background: var(--danger);
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

  /* "Files not found": the same column as the other action groups, framed
     as a notice with a danger edge. Text stays on --text/--text-muted so
     contrast holds in both themes; only the edge and the dot carry colour. */
  .action.notice {
    box-sizing: border-box;
    padding: 12px;
    border: 1px solid var(--danger);
    border-left-width: 4px;
    border-radius: var(--r-row);
    background: var(--surface);
  }

  .locate-path {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .locate-path input {
    font: inherit;
    width: 100%;
    box-sizing: border-box;
    padding: 8px 10px;
    border-radius: var(--r-control);
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text-h);
  }

  .locate-path input[aria-invalid='true'] {
    border-color: var(--danger);
  }

  .locate-path input:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: 1px;
  }

  .action button {
    width: 100%;
    font: inherit;
    padding: 10px 16px;
    border-radius: var(--r-control);
    border: none;
    background: var(--primary);
    color: var(--on-primary);
    cursor: pointer;
    transition: background var(--m-fast) ease;
  }

  .action button:hover:not(:disabled) {
    background: var(--primary-hover);
  }

  .action button.confirm {
    background: var(--danger);
    color: var(--on-danger);
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

  /* "Remove from library (keeps files)": secondary shape, danger-coloured
     label. Text-only danger keeps AA contrast in both themes. */
  .action button.secondary.destructive {
    color: var(--danger);
  }

  .action button:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .offline-note {
    text-align: center;
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
