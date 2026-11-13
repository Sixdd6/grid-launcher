<script lang="ts">
  import { type ContentKind, type GameSummary, type SaveType } from './api';
  import { refresh as refreshInstalled } from './stores/installed.svelte';
  import { session } from './stores/session.svelte';
  import Icon from './Icon.svelte';
  import Image from './Image.svelte';
  import NativeSettings from './details/NativeSettings.svelte';
  import UmuInstallOffer from './UmuInstallOffer.svelte';
  import OverviewTab from './details/OverviewTab.svelte';
  import MediaTab from './details/MediaTab.svelte';
  import MediaViewer from './details/MediaViewer.svelte';
  import SavesTab from './details/SavesTab.svelte';
  import AchievementsTab from './details/AchievementsTab.svelte';
  import LinkPicker from './details/LinkPicker.svelte';
  import RemoveConfirm from './details/RemoveConfirm.svelte';
  import { keptFolder } from './details/unlinked';
  import { pickFolder } from './pickers';
  import { fullIndex, viewableIndex, viewableItems } from './details/media';
  import { type DetailsSubject } from './details/subject';
  import { type CloudMode } from './details/cloud';
  import { installLabel } from './details/actions';
  import { DetailsStore } from './details/detailsStore.svelte';
  import { NoEmulatorCheck } from './details/noEmulatorCheck.svelte';
  import { noEmulatorText } from './details/noEmulator';
  import { SystemsStore } from './systems/systemsStore.svelte';
  import { lastPlayedText, verificationLabel } from './details/header';
  import {
    DETAILS_TAB_LABELS,
    rememberTab,
    rememberedTab,
    shownTab,
    tabTestId,
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

  // The game's data, install state and actions (details/detailsStore.svelte.ts).
  // This file holds only the popup's own state: confirms, focus, the tab and
  // the media viewer. The aliases below keep the markup reading as before.
  const d = new DetailsStore(() => subject);
  // A game whose system has no emulator: Install brings the system's
  // recommended emulator too, as in TV mode (user decision 2026-11-10). The
  // popup reads its own Systems store; nothing here needs its PS3 poll.
  const noEmu = new NoEmulatorCheck(new SystemsStore(() => false), d, { queuedSub: false });
  let noEmulator = $derived(noEmu.info);

  let detail = $derived(d.detail);
  let coverSmall = $derived(d.coverSmall);
  let coverLarge = $derived(d.coverLarge);
  let mediaItems = $derived(d.mediaItems);
  let failedMedia = $derived(d.failedMedia);
  let description = $derived(d.description);
  let serverTitles = $derived(d.serverTitles);
  let pendingAction = $derived(d.pendingAction);
  let pending = $derived(d.pending);
  let error = $derived(d.error);
  let umuOfferFor = $derived(d.umuOfferFor);
  let cancelPending = $derived(d.cancelPending);
  let contentActionKind = $derived(d.contentActionKind);
  let updatePending = $derived(d.updatePending);
  let liveEntry = $derived(d.liveEntry);
  let installedNow = $derived(d.installedNow);
  let liveSession = $derived(d.liveSession);
  let platformSlug = $derived(d.platformSlug);
  let isNative = $derived(d.isNative);
  let buttons = $derived(d.buttons);
  let installBlocked = $derived(d.installBlocked);
  let updateBlocked = $derived(d.updateBlocked);
  let dlcBlocked = $derived(d.dlcBlocked);
  let installedRow = $derived(d.installedRow);
  let cloudGame = $derived(d.cloudGame);
  let unlinkedRow = $derived(d.unlinkedRow);
  let heldRomIds = $derived(d.heldRomIds);
  let missing = $derived(d.missing);
  let locateError = $derived(d.locateError);
  let locatePending = $derived(d.locatePending);
  let removeTarget = $derived(d.removeTarget);
  let launchTarget = $derived(d.launchTarget);
  let updateLabel = $derived(d.updateLabel);
  let version = $derived(d.version);
  let serverVersion = $derived(d.serverVersion);
  let installedVersion = $derived(d.installedVersion);
  let savePanelInfo = $derived(d.savePanelInfo);
  let statePanelInfo = $derived(d.statePanelInfo);
  let cloudPanelInfoError = $derived(d.cloudPanelInfoError);
  let backup = $derived(d.backup);
  let onDrives = $derived(d.onDrives);
  let fromDrive = $derived(d.fromDrive);
  let driveCopy = $derived(d.driveCopy);
  let header = $derived(d.header);
  let ratingNumber = $derived(d.ratingNumber);
  let flags = $derived(d.flags);

  let confirmingUninstall = $state(false);
  let panelEl = $state<HTMLElement | null>(null);
  let showNativeSettings = $state(false);

  // Design §7: last tab remembered per session, not per game.
  let tab = $state<DetailsTab>(initialCloudMode === 'overview' ? rememberedTab() : 'saves');
  function selectTab(next: DetailsTab) {
    tab = next;
    rememberTab(next);
  }

  // Achievements shows only for a ROM whose RomM detail lists achievements;
  // a remembered Achievements choice falls back to Overview elsewhere.
  let tabs = $derived(d.tabs);
  let activeTab = $derived(shownTab(tab, tabs));

  // The grid already reported what IT knew when the popup opened; once the
  // detail lands, the merged subject is strictly richer (the only place
  // fanart is ever known). Reporting again is idempotent.
  $effect(() => {
    d.reportViewed();
  });

  // Which picture the viewer is on, held as an index into `mediaItems` — the
  // FULL list, not the filtered one. A screenshot that fails while the viewer
  // is open shortens `viewerItems` under it, and a position in that shrinking
  // list would silently address a different picture; a full-list index cannot,
  // because `mediaItems` only changes when the game does. `null` means the
  // viewer is closed.
  let viewerAnchor = $state<number | null>(null);
  function markMediaFailed(url: string) {
    d.markMediaFailed(url);
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

  $effect(() => {
    d.loadDetail();
  });

  $effect(() => {
    d.loadLaunchDefaults();
  });

  $effect(() => d.loadServerTitles());

  $effect(() => {
    d.refreshInstallBlocked();
  });

  $effect(() => {
    d.refreshContentAvailability();
  });

  let showLinkPicker = $state(false);
  let showRemoveConfirm = $state(false);

  async function linkTo(rom: GameSummary) {
    // The opener sees the row gain its rom id in the refreshed list and
    // re-opens Details on it (Library.svelte). A refusal throws to the picker.
    if (unlinkedRow === null) return;
    await d.linkTo(rom);
    showLinkPicker = false;
  }

  let locatePath = $state('');

  async function locate(folder: string) {
    if (await d.locate(folder)) locatePath = '';
  }

  async function locateWithPicker() {
    const folder = await pickFolder('Locate the game folder');
    if (folder !== null) await locate(folder);
  }

  async function removeFromLibrary() {
    if (!(await d.removeFromLibrary())) return;
    showRemoveConfirm = false;
    onClose();
  }

  // The update's queued and "<title> updated" cards come from the downloads
  // queue (notifications/notifier.ts), so they show with Details closed too.
  let confirmingUpdate = $state(false);

  /** The kind the Saves tab shows; a card's "Cloud sync" opens on saves. */
  let cloudKind = $state<SaveType>(initialCloudMode === 'state' ? 'state' : 'save');

  $effect(() => {
    panelEl?.focus();
  });

  $effect(() => {
    d.loadCloudPanelInfo();
  });

  async function handleInstall() {
    // The queued card and a refusal's error card are games/run.ts's.
    const outcome = await noEmu.install();
    if (outcome?.kind === 'needs-library-folder') onLibraryPathUnset();
  }

  async function handleUninstallClick() {
    if (subject.romId === null) return;
    if (!confirmingUninstall) {
      confirmingUninstall = true;
      return;
    }
    // The uninstalled card, the kept-folders card and the refresh are games/run.ts's.
    if (await d.uninstall()) onClose();
    else confirmingUninstall = false;
  }

  // Two-click confirm for native installs only (doc 10).
  async function handleUpdateClick() {
    if (subject.romId === null) return;
    if (isNative && !confirmingUpdate) {
      confirmingUpdate = true;
      return;
    }
    await d.update();
    confirmingUpdate = false;
  }

  function handleCancel() {
    void d.cancel();
  }

  function handleInstallContent(kind: ContentKind) {
    void d.installContent(kind);
  }

  function handlePlay() {
    void d.play();
  }

  function handleStop() {
    void d.stop();
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

  // GameBackups README: "Backup to drive…" under the install actions.
  // "Set up a drive…" leaves for Settings: Details closes behind it.
  function backUp() {
    d.backUp(onClose);
  }

  // Details from a drive (GameBackups README): Install from drive copies the
  // game from the drive, and Remove from drive… (two clicks) takes it off.
  let confirmingDriveRemove = $state(false);

  function handleDriveInstall() {
    void d.installFromDrive();
  }

  async function handleDriveRemove() {
    if (subject.drive === undefined || subject.romId === null) return;
    if (!confirmingDriveRemove) {
      confirmingDriveRemove = true;
      return;
    }
    if (await d.removeFromDrive()) onClose();
    else confirmingDriveRemove = false;
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
            {:else if subject.drive !== undefined}
              <button data-testid="details-install" disabled={pending} onclick={handleDriveInstall}>
                {pendingAction === 'install' ? 'Installing…' : 'Install from drive'}
              </button>
            {:else}
              <button
                data-testid="details-install"
                title={noEmulator ? undefined : installBlocked || undefined}
                aria-describedby={noEmulator ? 'details-no-emulator' : undefined}
                disabled={pending}
                onclick={handleInstall}
              >
                {pendingAction === 'install' ? 'Installing…' : installLabel(platformSlug, subject.platformName)}
              </button>
              {#if noEmulator}
                <!-- In place of the hover-only reason: what Install brings. -->
                <p id="details-no-emulator" class="meta-line offline-note" data-testid="details-no-emulator">{noEmulatorText(noEmulator)}</p>
              {/if}
            {/if}
            {#if subject.drive !== undefined}
              <!-- Two clicks; the second names the pack. -->
              <button
                data-testid="details-drive-remove"
                class="secondary"
                class:destructive={!confirmingDriveRemove}
                class:confirm={confirmingDriveRemove}
                onclick={handleDriveRemove}
              >
                {confirmingDriveRemove ? `Remove from ${subject.drive.name}` : 'Remove from drive…'}
              </button>
            {:else if !missing.showMissing}
              <button
                data-testid="details-backup"
                class="secondary"
                disabled={backup.disabled}
                aria-describedby={backup.disabled ? 'details-backup-offline' : undefined}
                title={backup.reason || undefined}
                onclick={backUp}
              >
                Backup to drive…
              </button>
              {#if backup.disabled}
                <!-- The button's `title` is hover-only; this line is the visible reason. -->
                <p id="details-backup-offline" class="meta-line offline-note">{backup.reason}.</p>
              {/if}
            {/if}
          </div>
          {#if installedNow && installedRow?.installed_by_installer}
            <p class="meta-line" data-testid="details-gog-installer">Installed by running its GOG installer</p>
          {/if}
          {#if onDrives !== '' && subject.drive === undefined}
            <p class="meta-line on-drives" data-testid="details-on-drives"><Icon name="drive" size={12} />{onDrives}</p>
          {/if}
          {#each driveCopy as line (line)}
            <p class="meta-line" data-testid="details-drive-copy">{line}</p>
          {/each}
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
          <p class="header-line" data-testid="details-header-line">{header}{#if fromDrive !== ''}{#if header !== ''}{' · '}{/if}<span class="from-drive" data-testid="details-from-drive">{fromDrive}</span>{/if}{#if ratingNumber !== ''}{#if header !== ''}{' · '}{/if}<span class="rating"><span class="star"><Icon name="star" size={14} /></span>{ratingNumber}</span>{/if}</p>
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

  /* "From Couch kit" in the header line (GameBackups prototype `.bk-src`). */
  .from-drive {
    color: var(--primary);
  }

  .on-drives {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 5px;
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
