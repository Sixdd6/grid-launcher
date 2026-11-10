// Everything a Details screen knows about one game, and every action it
// offers, apart from how it lays them out. One `DetailsStore` per open
// Details (desktop popup or TV page), created with a getter for its subject
// so a parent that swaps the subject is followed.
//
// It holds the ROM detail overlay, the launch defaults, the Related title
// filter, the install-block reason, PS4 / Xbox 360 content availability, the
// cloud panel facts, the media failure map, the install state derived from
// the shared stores, and the actions' pending and error state. Two-click
// confirms, focus and which tab or picture is shown stay with the view.
//
// The loaders (`loadDetail`, `refreshContentAvailability`, …) read their
// inputs synchronously before their first await, so a view that calls each
// one from its own `$effect` re-runs it exactly when those inputs change.
import {
  api,
  type CloudPanelInfo,
  type ContentAvailability,
  type ContentKind,
  type DownloadStatus,
  type GameSummary,
  type InstalledGame,
  type LaunchDefaults,
  type RomDetail,
} from '../api';
import { subjectFromDetails } from '../background';
import { backupRequest } from '../backups/dialog';
import { openBackup } from '../backups/dialogState.svelte';
import { removeFromDrive as removeDriveGames } from '../backups/install';
import * as run from '../games/run';
import { detailsPrimary } from '../games/quick';
import { missingActions } from '../library/missing';
import { downloads } from '../stores/downloads.svelte';
import { drives } from '../stores/drives.svelte';
import { installed, isInstalled, matchesInstalled } from '../stores/installed.svelte';
import { noteViewed } from '../stores/lastViewed.svelte';
import { session } from '../stores/session.svelte';
import { sessions } from '../stores/sessions.svelte';
import { updates } from '../stores/updates.svelte';
import { backupAction, contentButtons, isContentPlatform, isNativePlatform, onDrivesLine } from './actions';
import { contentBlockReason } from './blocked';
import { syntheticCloudGame } from './cloud';
import { epochDate, flagList, headerLine, launchTargetLine, ratingValue } from './header';
import { galleryItems } from './media';
import { detailApplies, driveCopyLines, fromDriveText, mergeDetail, summaryOf, versionSource, type DetailsSubject } from './subject';
import { visibleTabs } from './tabs';
import { showsUnlinkedActions } from './unlinked';
import { fileVersionLabel, romFileNamesFor, showsFilesVersionLine, versionLabel } from './version';

export const LIVE_INSTALL_STATUSES: DownloadStatus[] = ['queued', 'downloading', 'installing', 'cancelling'];

export type DetailsPending = 'install' | 'uninstall' | 'play' | 'stop' | null;

function errorMessage(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

/** Runs a game action whose refusal the caller shows from a thrown error
 *  (the Link picker and the Remove confirm show it inside themselves). */
async function orThrow(outcome: Promise<run.RunOutcome>): Promise<void> {
  const done = await outcome;
  if (done.kind !== 'ok') throw new Error(done.reason);
}

export class DetailsStore {
  readonly #subject: () => DetailsSubject;

  constructor(subject: () => DetailsSubject) {
    this.#subject = subject;
  }

  get subject(): DetailsSubject {
    return this.#subject();
  }

  // --- Fetched data -------------------------------------------------------

  // Metadata overlay (task-10-brief.md): the subject carries whatever the
  // grid it opened from already had on hand; the full `RomDetail` fills in
  // the gaps (files, related, the IGDB block, media fields). It stays a
  // display overlay: the subject is the caller's data and is never mutated.
  detail = $state<RomDetail | null>(null);
  launchDefaults = $state<LaunchDefaults | null>(null);
  /** The platform's server titles, to filter Related against. */
  serverTitles = $state<string[]>([]);
  /** The primary button's reason it cannot install; `''` when none. */
  installBlocked = $state('');
  contentAvailability = $state<ContentAvailability | null>(null);
  /** Whether a live install existed on the previous availability check. */
  wasLive = $state(false);
  savePanelInfo = $state<CloudPanelInfo | null>(null);
  statePanelInfo = $state<CloudPanelInfo | null>(null);
  cloudPanelInfoError = $state<string | null>(null);
  /** One failure map for the media gallery and the viewer, keyed by URL. */
  failedMedia = $state<Record<string, true>>({});

  // --- Action state --------------------------------------------------------

  pendingAction = $state<DetailsPending>(null);
  cancelPending = $state(false);
  contentActionKind = $state<ContentKind | null>(null);
  updatePending = $state(false);
  error = $state<string | null>(null);
  // The error that offers the umu-launcher install (a Proton launch with no
  // umu-run); `null` for any other. Compared by value, so a later action's
  // own error never shows the offer.
  umuOfferFor = $state<string | null>(null);
  locateError = $state<string | null>(null);
  locatePending = $state(false);

  // --- Derived --------------------------------------------------------------

  merged = $derived(this.detail === null ? this.subject : mergeDetail(this.subject, this.detail));
  /** Achievements shows only for a ROM whose detail lists achievements. */
  tabs = $derived(visibleTabs(this.detail?.achievement_count));
  coverSmall = $derived(this.merged.coverSmall);
  coverLarge = $derived(this.merged.coverLarge);
  screenshotUrls = $derived(this.merged.screenshotUrls);
  description = $derived(this.merged.description);
  rating = $derived(this.merged.rating);
  genres = $derived(this.merged.genres);
  /** The gallery the Media tab and the viewer share. */
  mediaItems = $derived(
    galleryItems({
      title: this.subject.name,
      screenshotUrls: this.screenshotUrls,
      youtubeVideoId: this.detail?.youtube_video_id ?? '',
      videoPath: this.detail?.video_path ?? '',
    }),
  );

  pending = $derived(this.pendingAction !== null);
  summary = $derived(summaryOf(this.subject));
  liveEntry = $derived(
    this.subject.romId !== null
      ? downloads.entries.find((e) => e.rom_id === this.subject.romId && LIVE_INSTALL_STATUSES.includes(e.status))
      : undefined,
  );
  installedNow = $derived(isInstalled(this.summary, this.subject.platformName));
  liveSession = $derived(this.subject.romId !== null ? sessions.sessionFor(this.subject.romId) : undefined);

  isContent = $derived(isContentPlatform(this.subject.platformName));
  // One native predicate for the whole screen (user ruling 2026-09-08): the
  // install label, the native Game Settings button and the Saves tab's PC
  // save-folder mode all read it. The server slug decides first (a custom
  // display name on slug `win` is still native); a registry row the backfill
  // has not reached gets it from the ROM detail (`mergeDetail`).
  platformSlug = $derived(this.merged.platformSlug);
  isNative = $derived(isNativePlatform(this.platformSlug, this.subject.platformName));
  buttons = $derived(contentButtons(this.contentAvailability, this.installedNow, this.liveEntry !== undefined));
  updateBlocked = $derived(
    contentBlockReason('update', this.subject.platformName, this.installedNow, this.subject.romId, this.buttons.update),
  );
  dlcBlocked = $derived(
    contentBlockReason('dlc', this.subject.platformName, this.installedNow, this.subject.romId, this.buttons.dlc),
  );

  /** The registry row for the game, when it is installed. */
  installedRow = $derived<InstalledGame | null>(
    installed.list.find((row) => matchesInstalled(row, this.summary, this.subject.platformName)) ?? null,
  );
  // A row the slug backfill has not reached borrows the slug the ROM detail
  // carries, so the cloud commands see the same nativeness as the screen.
  cloudGame = $derived<InstalledGame>(
    this.installedRow
      ? { ...this.installedRow, platform_slug: this.installedRow.platform_slug || this.platformSlug }
      : syntheticCloudGame(this.summary, this.subject.platformName, this.platformSlug),
  );

  // Q4: an installed row with no server rom id offers Link to server game
  // and Remove from library (keeps files) instead of the server actions.
  unlinkedRow = $derived(showsUnlinkedActions(this.subject.romId, this.installedRow) ? this.installedRow : null);
  /** Rom ids other installed rows hold; the link list leaves them out. */
  heldRomIds = $derived(new Set(installed.list.flatMap((row) => (row.rom_id === null ? [] : [row.rom_id]))));
  // 9c: the row's files are not on disk. Locate… and Remove from library are
  // offered; Play and Uninstall are disabled with the reason.
  missing = $derived(missingActions(this.installedRow));
  /** The row Remove from library acts on: an unlinked row or a missing one. */
  removeTarget = $derived(this.unlinkedRow ?? (this.missing.showMissing ? this.installedRow : null));

  /** Which main action group the screen shows (`games/quick.ts`). */
  primary = $derived(
    detailsPrimary({
      romId: this.subject.romId,
      unlinked: this.unlinkedRow !== null,
      live: this.liveEntry !== undefined,
      playing: this.liveSession !== undefined,
      installed: this.installedNow,
      fromDrive: this.subject.drive !== undefined,
    }),
  );

  /** `''` means no launch target to state (a native platform). */
  launchTarget = $derived(launchTargetLine(this.launchDefaults, this.subject.platformName, this.platformSlug));
  /** The update button's label; `null` hides it. */
  updateLabel = $derived(this.installedNow ? updates.labelFor(this.subject.romId) : null);
  version = $derived(
    versionLabel(
      this.subject.platformName,
      romFileNamesFor(versionSource(this.subject), this.installedRow?.rom_file_name ?? '', this.detail?.fs_name ?? ''),
      this.detail?.revision || this.installedRow?.revision || '',
    ),
  );
  // D-UI-10, per side, PC platforms only (`showsFilesVersionLine`).
  versionLineShown = $derived(showsFilesVersionLine(this.subject.platformName));
  topLevelFile = $derived(this.detail?.files.find((f) => f.is_top_level) ?? this.detail?.files[0] ?? null);
  serverVersion = $derived(
    this.versionLineShown && this.detail
      ? fileVersionLabel(this.detail.fs_name, this.topLevelFile?.last_modified ?? '')
      : '',
  );
  installedVersion = $derived(
    this.versionLineShown && this.installedRow
      ? fileVersionLabel(this.installedRow.rom_file_name, '') || epochDate(this.installedRow.installed_at)
      : '',
  );

  // GameBackups README: "Backup to drive…" and the drives holding the game.
  backup = $derived(backupAction(session.connected));
  onDrives = $derived(
    this.subject.romId === null
      ? ''
      : onDrivesLine(
          drives.holdingRom(session.serverUrl, this.subject.romId).map((d) => d.pack?.name ?? d.label),
        ),
  );
  fromDrive = $derived(fromDriveText(this.subject));
  driveCopy = $derived(driveCopyLines(this.subject));

  header = $derived(
    headerLine({
      platformName: this.subject.platformName,
      firstReleaseDate: this.detail?.first_release_date || this.subject.releaseDate || '',
      companies: this.detail?.companies || this.subject.companies || '',
      genres: this.genres,
    }),
  );
  ratingNumber = $derived(ratingValue(this.rating));
  // `||`, not `??`: a loaded detail always carries strings, so `''` from the
  // server would win over the registry's stored value under `??`.
  flags = $derived([
    ...flagList(this.detail?.regions || this.installedRow?.regions || ''),
    ...flagList(this.detail?.languages || this.installedRow?.languages || ''),
  ]);

  // --- Loaders ----------------------------------------------------------------

  /** Reports the merged subject to the background layer. A drive's art is
   *  not on a server the background can build from. */
  reportViewed(): void {
    if (this.subject.source === 'drive') return;
    noteViewed(subjectFromDetails(this.merged));
  }

  /** Fetches the ROM detail overlay, when the subject's id is on this server. */
  loadDetail(): void {
    const subject = this.subject;
    if (subject.romId === null) return; // no server id: nothing to overlay
    if (!session.connected) return;
    // A drive game's id belongs to the server it was backed up from.
    if (!detailApplies(subject, session.serverUrl)) return;
    api
      .getRomDetail(subject.romId)
      .then((fetched) => {
        this.detail = fetched;
      })
      .catch(() => {}); // offline/removed rom: the subject's own data stands
  }

  loadLaunchDefaults(): void {
    api
      .getLaunchDefaults()
      .then((d) => (this.launchDefaults = d))
      .catch(() => {}); // unreadable config: the emulator row says "No default emulator"
  }

  /**
   * Design §7 Overview: Related is filtered against the platform's own game
   * list, fetched once the detail names the platform. A failure leaves the
   * list empty, which shows no Related row rather than a wrong one. Answers
   * the function that drops a pending answer, or nothing when no fetch ran.
   */
  loadServerTitles(): (() => void) | undefined {
    const platformId = this.detail?.platform_id ?? null;
    if (platformId === null || !session.connected) return;
    if (this.detail !== null && this.detail.related.length === 0) return; // nothing to filter
    let cancelled = false;
    api
      .listGames(platformId)
      .then((games) => {
        if (!cancelled) this.serverTitles = games.map((g) => g.name);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }

  /** The primary button's reason needs the configured emulator list, so it
   *  comes from the backend; a failure leaves it blank rather than guessing. */
  refreshInstallBlocked(): void {
    // A drive install brings the system's emulator from the drive when it
    // has one, so the server-side reason would not hold.
    if (this.installedNow || this.subject.drive !== undefined) {
      this.installBlocked = '';
      return;
    }
    api
      .installBlockReason(this.subject.platformName, this.platformSlug)
      .then((reason) => (this.installBlocked = reason))
      .catch(() => (this.installBlocked = ''));
  }

  /** Fetched once the game is installed on a content platform, and again
   *  right after a live install for it finishes. */
  refreshContentAvailability(): void {
    const romId = this.subject.romId;
    if (romId === null || !this.installedNow || !this.isContent) {
      this.contentAvailability = null;
      this.wasLive = this.liveEntry !== undefined;
      return;
    }
    const live = this.liveEntry !== undefined;
    const justFinished = this.wasLive && !live;
    this.wasLive = live;
    if (live || (this.contentAvailability !== null && !justFinished)) return;
    api
      .contentAvailability(romId)
      .then((avail) => (this.contentAvailability = avail))
      .catch(() => (this.contentAvailability = null));
  }

  /** The cloud panel facts for saves and states (task-19-brief.md). */
  loadCloudPanelInfo(): void {
    if (this.subject.romId === null) return; // no server id: no cloud saves to manage
    const game = this.cloudGame;
    api
      .cloudPanelInfo(game, 'save')
      .then((info) => (this.savePanelInfo = info))
      .catch((err) => (this.cloudPanelInfoError = errorMessage(err)));
    api
      .cloudPanelInfo(game, 'state')
      .then((info) => (this.statePanelInfo = info))
      .catch((err) => (this.cloudPanelInfoError = errorMessage(err)));
  }

  markMediaFailed(url: string): void {
    this.failedMedia = { ...this.failedMedia, [url]: true };
  }

  // --- Actions ------------------------------------------------------------------
  // Each sends its own cards (`games/run.ts`); a refusal the screen shows
  // inline lands in `error`.

  /** Queues the server install. Answers the outcome, so the screen can offer
   *  the library-folder prompt for `needs-library-folder`; `null` when the
   *  game has no server id. */
  async install(): Promise<run.RunOutcome | null> {
    const { romId, name } = this.subject;
    if (romId === null) return null;
    this.error = null;
    this.pendingAction = 'install';
    try {
      return await run.installServerGame(romId, name);
    } finally {
      this.pendingAction = null;
    }
  }

  /** Uninstalls the game. Answers whether it went (the screen then closes). */
  async uninstall(): Promise<boolean> {
    const { romId, name } = this.subject;
    if (romId === null) return false;
    this.error = null;
    this.pendingAction = 'uninstall';
    const outcome = await run.uninstallGame(romId, name);
    if (outcome.kind !== 'ok') this.error = outcome.reason;
    this.pendingAction = null;
    return outcome.kind === 'ok';
  }

  /** Queues the server's update for the installed game. */
  async update(): Promise<void> {
    const romId = this.subject.romId;
    if (romId === null) return;
    this.error = null;
    this.updatePending = true;
    const outcome = await run.updateGame(romId);
    if (outcome.kind !== 'ok') this.error = outcome.reason;
    this.updatePending = false;
  }

  /** Cancels the live install. */
  async cancel(): Promise<void> {
    const romId = this.subject.romId;
    if (romId === null) return;
    this.error = null;
    this.cancelPending = true;
    const outcome = await run.cancelInstall(romId);
    if (outcome.kind !== 'ok') this.error = outcome.reason;
    this.cancelPending = false;
  }

  /** Queues a PS4 / Xbox 360 update or DLC. */
  async installContent(kind: ContentKind): Promise<void> {
    const romId = this.subject.romId;
    if (romId === null) return;
    this.error = null;
    this.contentActionKind = kind;
    const outcome = await run.installContent(romId, kind);
    if (outcome.kind !== 'ok') this.error = outcome.reason;
    this.contentActionKind = null;
  }

  /** Launches the game. "Launching <title>…" and a refusal are cards; the
   *  reason also shows inline only beside the umu-launcher install it needs. */
  async play(): Promise<void> {
    const { romId, name } = this.subject;
    if (romId === null) return;
    this.error = null;
    this.pendingAction = 'play';
    const outcome = await run.playInstalled(romId, name);
    if (outcome.kind !== 'ok') {
      const umu = outcome.kind === 'needs-umu';
      this.error = umu ? outcome.reason : null;
      this.umuOfferFor = umu ? this.error : null;
    }
    this.pendingAction = null;
  }

  /** Stops the running session. */
  async stop(): Promise<void> {
    const live = this.liveSession;
    if (!live) return;
    this.error = null;
    this.pendingAction = 'stop';
    const outcome = await run.stopGame(live.id);
    if (outcome.kind !== 'ok') this.error = outcome.reason;
    this.pendingAction = null;
  }

  /** Links the unlinked row to `rom`. Throws the refusal, for the link list
   *  to show. The opener sees the row gain its id in the refreshed registry. */
  async linkTo(rom: GameSummary): Promise<void> {
    const row = this.unlinkedRow;
    if (row === null) return;
    await orThrow(run.linkGame(row.title, row.platform, rom.id));
  }

  /** Points the missing row at `folder`. Answers whether it was accepted;
   *  a refusal lands in `locateError`. */
  async locate(folder: string): Promise<boolean> {
    const row = this.installedRow;
    if (row === null || folder.trim() === '') return false;
    this.locateError = null;
    this.locatePending = true;
    const outcome = await run.locateGame(row.title, row.platform, folder);
    if (outcome.kind !== 'ok') this.locateError = outcome.reason;
    this.locatePending = false;
    return outcome.kind === 'ok';
  }

  /** Drops the unlinked or missing row, keeping its files. Throws the
   *  refusal, for the confirm to show. Answers whether a row was removed. */
  async removeFromLibrary(): Promise<boolean> {
    const target = this.removeTarget;
    if (target === null) return false;
    await orThrow(run.removeFromLibrary(target.title, target.platform));
    return true;
  }

  /** Copies the game from the backup drive it is shown from. */
  async installFromDrive(): Promise<void> {
    const { drive: from, romId, name } = this.subject;
    if (from === undefined || romId === null) return;
    this.error = null;
    this.pendingAction = 'install';
    try {
      await run.installFromDrive(from.key, { rom_id: romId, name, server: from.server });
    } finally {
      this.pendingAction = null;
    }
  }

  /** Takes the game off the drive it is shown from. Answers whether it went;
   *  a refusal lands in `error`. */
  async removeFromDrive(): Promise<boolean> {
    const { drive: from, romId, name } = this.subject;
    if (from === undefined || romId === null) return false;
    this.error = null;
    try {
      await removeDriveGames(from.key, from.name, [{ rom_id: romId, name, server: from.server }]);
      return true;
    } catch (err) {
      this.error = errorMessage(err);
      return false;
    }
  }

  /** Opens the Backup dialog for this game. `onLeave` runs when "Set up a
   *  drive…" leaves for Settings (the screen closes behind it). */
  backUp(onLeave: () => void): void {
    const { romId, name } = this.subject;
    if (romId === null || this.backup.disabled) return;
    openBackup({ request: backupRequest({ kind: 'game', name }, [romId]), onLeave });
  }
}
