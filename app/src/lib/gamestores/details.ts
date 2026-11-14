// Details for a store game (GameStores README, "Details for a store game",
// "Cloud saves tab", "Game Settings", "GOG games on backup drives › Details
// from a drive"; prototype `stRenderDetails`, `stCloudHTML`, `stConflictHTML`,
// `stSettingsHTML`). Pure: which buttons, lines, chips, tabs and facts show.
import type { DownloadEntry, DriveStoreGame, StoreCloudState, StoreGame, StoreStatus } from '../api';
import { isManagedRow, jobAct, jobPercent } from './jobs';
import { canBackup, hasCloud, hasDlc, installedByTool, managedByClient, noBackupReason, shownVersion, storeLauncher, storeName, updateLabel } from './marks';
import {
  INSTALL_IN_STEAM,
  installingInSteam,
  MANAGED_BY_STEAM,
  MANAGED_NOTE,
  OUTSIDE_GRID,
  RUNNING_IN_STEAM,
  UNINSTALL_IN_STEAM,
  VERIFY_IN_STEAM,
} from './steam';
import { agoText, changedAt, sizeGb } from './text';

export type StoreButtonAct =
  | 'play'
  | 'stop'
  | 'install'
  | 'cancel'
  | 'update'
  | 'settings'
  | 'backup'
  | 'restore'
  | 'remove_drive'
  | 'remove_drive_confirm'
  | 'verify'
  | 'uninstall'
  | 'uninstall_confirm';

export type StoreButton = {
  label: string;
  act: StoreButtonAct;
  /** The filled main button; the rest are secondary. */
  primary: boolean;
  destructive?: boolean;
  /** The confirming click of a two-click button (filled danger). */
  confirm?: boolean;
  disabled?: boolean;
  title?: string;
};

/** Which two-click button is waiting for its second click. */
export type PendingConfirm = 'uninstall' | 'remove_drive' | null;

export type DetailsContext = {
  game: StoreGame;
  status: StoreStatus | null;
  job: DownloadEntry | null;
  playing: boolean;
  confirm: PendingConfirm;
  /** Opened from a backup drive's grid: that drive's pack name and its entry. */
  drive?: { name: string; game: DriveStoreGame } | null;
};

/**
 * The left column's buttons, top to bottom: Play / Stop / Cancel install ·
 * 45% / Install… / Not supported / Install from drive, then Update to 2.2
 * ("Update" for Amazon, whose versions are opaque),
 * Game Settings, Backup to drive… (or Remove from drive…), Verify and repair,
 * Uninstall (two clicks).
 */
export function storeButtons(ctx: DetailsContext): StoreButton[] {
  const { game, status, job, playing, confirm } = ctx;
  if (managedByClient(game.source)) return clientButtons(ctx);
  const out: StoreButton[] = [];
  const signedIn = status !== null && status.connection === 'connected';
  if (job !== null) {
    const act = jobAct(job);
    const what = act === 'update' ? 'update' : act === 'repair' ? 'verify' : 'install';
    out.push({
      label: `Cancel ${what} · ${job.status === 'queued' ? 'queued' : `${jobPercent(job)}%`}`,
      act: 'cancel',
      primary: true,
    });
  } else if (game.installed) {
    out.push(playing ? { label: 'Stop', act: 'stop', primary: true } : { label: 'Play', act: 'play', primary: true });
  } else if (game.not_supported !== '') {
    out.push({ label: 'Not supported', act: 'install', primary: true, disabled: true });
  } else if (ctx.drive) {
    out.push({ label: 'Install from drive', act: 'restore', primary: true });
  } else {
    out.push({ label: 'Install…', act: 'install', primary: true, disabled: !signedIn });
  }
  if (game.installed && game.update_version !== '' && job === null) {
    out.push({ label: updateLabel(game), act: 'update', primary: false, disabled: !signedIn });
  }
  if (game.installed) out.push({ label: 'Game Settings', act: 'settings', primary: false });
  if (ctx.drive) {
    out.push(
      confirm === 'remove_drive'
        ? { label: `Remove from ${ctx.drive.name}`, act: 'remove_drive_confirm', primary: false, destructive: true, confirm: true }
        : { label: 'Remove from drive…', act: 'remove_drive', primary: false, destructive: true },
    );
  } else if (canBackup(game.source) && game.installed) {
    out.push({ label: 'Backup to drive…', act: 'backup', primary: false });
  }
  if (game.installed && job === null) {
    // Only the store's tool can verify: an offline install or a drive copy can't be.
    out.push({ label: 'Verify and repair', act: 'verify', primary: false, disabled: !signedIn || !installedByTool(game) });
    out.push(
      confirm === 'uninstall'
        ? { label: 'Confirm uninstall', act: 'uninstall_confirm', primary: false, destructive: true, confirm: true }
        : { label: 'Uninstall', act: 'uninstall', primary: false, destructive: true },
    );
  }
  return out;
}

/**
 * A store whose client runs everything (Steam; prototype `stRenderDetails`'s
 * Steam branches): a disabled Cancel beside the managed box while Steam
 * installs, Play / Running in Steam, Install in Steam (no dialog), then
 * Verify in Steam and Uninstall in Steam… (Steam asks, so no confirm of
 * GRID's own). No Game Settings, Update or drive buttons.
 */
function clientButtons(ctx: DetailsContext): StoreButton[] {
  const { game, job, playing } = ctx;
  const out: StoreButton[] = [];
  if (job !== null && isManagedRow(job)) {
    out.push({ label: 'Cancel', act: 'cancel', primary: false, disabled: true, title: MANAGED_BY_STEAM });
  } else if (game.installed) {
    out.push(playing ? { label: RUNNING_IN_STEAM, act: 'stop', primary: true } : { label: 'Play', act: 'play', primary: true });
  } else {
    out.push({ label: INSTALL_IN_STEAM, act: 'install', primary: true });
  }
  if (game.installed && job === null) {
    out.push({ label: VERIFY_IN_STEAM, act: 'verify', primary: false });
    out.push({ label: UNINSTALL_IN_STEAM, act: 'uninstall', primary: false });
  }
  return out;
}

/** The box above the disabled Cancel while the store's client installs: "Installing in Steam · 52%", its bar and note. */
export type ManagedBox = { title: string; pct: number; note: string };

export function managedBox(job: DownloadEntry | null): ManagedBox | null {
  if (job === null || !isManagedRow(job)) return null;
  const pct = jobPercent(job);
  return { title: installingInSteam(pct), pct, note: MANAGED_NOTE };
}

export type LineKind = 'why' | 'meta' | 'cloud' | 'cloud-err' | 'drive';
/** A line under the buttons. `link` is a trailing link button ("Sign in again", "Resolve"). */
export type StoreLine = { kind: LineKind; text: string; link?: { label: string; act: 'sign_in' | 'cloud_tab' }; testId: string };

/** `compat` is the game's compatibility tool (or the Settings › Systems default for a game not installed). */
export function storeLines(
  ctx: DetailsContext,
  opts: { nowSeconds: number; lastPlayedAt: number; compat: string; onDrives: string[]; syncing: boolean },
): StoreLine[] {
  const { game, status } = ctx;
  const name = storeName(game.source);
  const out: StoreLine[] = [];
  if (!game.installed && status !== null && status.connection !== 'connected' && !ctx.drive && !managedByClient(game.source)) {
    out.push(
      status.connection === 'not_connected'
        ? { kind: 'why', text: `Connect ${name} in Settings › Stores to install.`, testId: 'store-details-why' }
        : { kind: 'why', text: `Your ${name} login expired.`, link: { label: 'Sign in again', act: 'sign_in' }, testId: 'store-details-why' },
    );
  }
  if (game.not_supported !== '') {
    out.push({
      kind: 'why',
      text: `Needs ${game.not_supported}. GRID can’t install games that start through another company’s launcher; install it from the ${storeLauncher(game.source)} instead.`,
      testId: 'store-details-unsupported',
    });
  }
  if (game.installed) {
    out.push({
      kind: 'meta',
      text: opts.lastPlayedAt > 0 ? `Last played ${agoText(opts.lastPlayedAt, opts.nowSeconds)}` : 'Never played',
      testId: 'store-details-last-played',
    });
  }
  out.push({ kind: 'meta', text: runsLine(game, opts.compat), testId: 'store-details-runs' });
  if (game.installed && hasCloud(game.source)) {
    out.push(cloudLine(game, opts.syncing, opts.nowSeconds));
  }
  if (game.origin === 'offline') out.push({ kind: 'meta', text: 'Installed from an offline installer', testId: 'store-details-offline' });
  if (game.installed && game.origin === 'steam_outside') out.push({ kind: 'meta', text: OUTSIDE_GRID, testId: 'store-details-outside' });
  if (ctx.drive) {
    out.push({ kind: 'meta', text: driveRestoreLine(ctx.drive.game), testId: 'store-details-drive-copy' });
  } else if (canBackup(game.source)) {
    if (opts.onDrives.length > 0) out.push({ kind: 'drive', text: `On ${opts.onDrives.join(', ')}`, testId: 'store-details-on-drives' });
  } else {
    out.push({ kind: 'meta', text: noBackupReason(game.source), testId: 'store-details-no-backup' });
  }
  return out;
}

/** "Runs natively · Linux version", "Runs through GE-Proton10-17 · Windows version", or for a game not installed "Has a Linux version" / "Windows only · runs through …". */
export function runsLine(game: StoreGame, compat: string): string {
  if (managedByClient(game.source)) return 'Launches through Steam';
  if (!game.installed) {
    return game.platforms.includes('linux') ? 'Has a Linux version' : `Windows only · runs through ${compat}`;
  }
  return game.installed_platform === 'linux' ? 'Runs natively · Linux version' : `Runs through ${game.compat_tool || compat} · Windows version`;
}

/** The cloud line under the buttons. */
export function cloudLine(game: StoreGame, syncing: boolean, nowSeconds: number): StoreLine {
  switch (game.cloud) {
    case 'conflict':
      return { kind: 'cloud-err', text: 'Cloud save conflict · ', link: { label: 'Resolve', act: 'cloud_tab' }, testId: 'store-details-cloud' };
    case 'off':
      return { kind: 'cloud', text: 'Cloud saves off for this game', testId: 'store-details-cloud' };
    case 'none':
      return { kind: 'cloud', text: 'This game has no cloud saves', testId: 'store-details-cloud' };
    default:
      return {
        kind: 'cloud',
        text: syncing
          ? 'Syncing cloud saves…'
          : game.last_sync > 0
            ? `Cloud saves synced ${agoText(game.last_sync, nowSeconds)}`
            : 'Cloud saves on',
        testId: 'store-details-cloud',
      };
  }
}

/** Details from a drive: how the pack holds the game and what installing does. */
export function driveRestoreLine(g: DriveStoreGame): string {
  const v = g.version.trim() ? ` · v${g.version.trim()}` : '';
  return g.layout === 'installer'
    ? `On the drive as its installer files${v} and saves. Installing runs the installer again.`
    : `On the drive as a copy of an installed game${v}, with its saves. Installing copies it back.`;
}

/** "GOG · 2023 · Lantern Works · RPG"; a drive leads with "From Couch kit". */
export function storeHeaderLine(game: StoreGame): string {
  const genres = managedByClient(game.source) ? '' : game.genres.join(', ');
  return [storeName(game.source), game.year, game.developer, genres].filter((p) => p.trim() !== '').join(' · ');
}

export type StoreChipKind = 'store' | 'playing' | 'plain';
export type StoreDetailChip = { kind: StoreChipKind; text: string; testId: string };

/** The header chips: store (mark + name), Playing, v2.1, Linux/Windows version, Offline installer, Not supported. */
export function storeDetailChips(game: StoreGame, playing: boolean): StoreDetailChip[] {
  const out: StoreDetailChip[] = [{ kind: 'store', text: storeName(game.source), testId: 'store-details-chip-store' }];
  if (playing) out.push({ kind: 'playing', text: 'Playing', testId: 'store-details-chip-playing' });
  // Amazon's version is an opaque id: no chip (user decision 2026-11-13).
  const version = shownVersion(game);
  if (game.installed && version !== '') out.push({ kind: 'plain', text: `v${version}`, testId: 'store-details-chip-version' });
  // Steam's chips are the store's and Playing only: Steam picks the build.
  if (game.installed && game.installed_platform !== null && !managedByClient(game.source)) {
    out.push({
      kind: 'plain',
      text: game.installed_platform === 'linux' ? 'Linux version' : 'Windows version',
      testId: 'store-details-chip-platform',
    });
  }
  if (game.origin === 'offline') out.push({ kind: 'plain', text: 'Offline installer', testId: 'store-details-chip-offline' });
  if (game.not_supported !== '') out.push({ kind: 'plain', text: 'Not supported', testId: 'store-details-chip-unsupported' });
  return out;
}

export type StoreTab = 'overview' | 'media' | 'cloud';

/** Overview, Media, and Cloud saves for an installed GOG (or Epic) game. */
export function storeTabs(game: StoreGame): StoreTab[] {
  return game.installed && hasCloud(game.source) ? ['overview', 'media', 'cloud'] : ['overview', 'media'];
}

export const STORE_TAB_LABELS: Record<StoreTab, string> = { overview: 'Overview', media: 'Media', cloud: 'Cloud saves' };

export type Fact = { term: string; value: string; code?: boolean };

/**
 * Overview's facts: Store, Developer, Released, Genres, Size, Version ("v2.1 ·
 * GOG v2.2"; none for Amazon), Folder. Steam has no Genres (Choices 6), no
 * Size for a game not installed (Steam's API gives none; never guessed) and
 * no Folder (Steam owns it).
 */
export function storeFacts(game: StoreGame): Fact[] {
  const name = storeName(game.source);
  const managed = managedByClient(game.source);
  const out: Fact[] = [{ term: 'Store', value: name }];
  if (game.developer.trim()) out.push({ term: 'Developer', value: game.developer });
  if (game.year.trim()) out.push({ term: 'Released', value: game.year });
  if (game.genres.length && !managed) out.push({ term: 'Genres', value: game.genres.join(', ') });
  if (game.installed) out.push({ term: 'Size', value: `${sizeGb(game.disk_bytes)} on disk` });
  else if (!managed) out.push({ term: 'Size', value: `${sizeGb(game.download_bytes)} download` });
  if (game.installed && shownVersion(game) !== '') {
    out.push({
      term: 'Version',
      value: game.update_version ? `v${game.version} · ${name} v${game.update_version}` : `v${game.version}`,
    });
  }
  if (game.installed && !managed && game.install_dir.trim()) out.push({ term: 'Folder', value: game.install_dir, code: true });
  return out;
}

/** The store's description split into paragraphs. */
export function descriptionParagraphs(text: string): string[] {
  return text
    .split(/\n\s*\n|\r\n\s*\r\n/)
    .map((p) => p.trim())
    .filter((p) => p !== '');
}

export type DlcRow = { id: string; title: string; size: string; installed: boolean; busy: boolean; canAct: boolean };

/** "DLC you own": each with its size, Installed, and Install / Remove when the game is installed. */
export function dlcRows(game: StoreGame, entries: readonly DownloadEntry[]): DlcRow[] {
  if (!hasDlc(game.source)) return [];
  return game.dlc.map((d) => ({
    id: d.id,
    title: d.title.startsWith(`${game.title} — `) ? d.title.slice(game.title.length + 3) : d.title,
    size: sizeGb(d.download_bytes),
    installed: d.installed,
    busy: entries.some(
      (e) => e.job === 'store' && e.kind === 'store_dlc' && e.source_id === game.key && ['queued', 'downloading', 'installing'].includes(e.status) && e.title.includes(d.title),
    ),
    canAct: game.installed,
  }));
}

// ── Cloud saves tab ─────────────────────────────────────────────────────

export type CloudPanel =
  | { kind: 'synced'; title: string; note: string; button: string; busy: boolean }
  | { kind: 'conflict'; title: string; note: string; sides: ConflictSide[] }
  | { kind: 'off'; title: string; note: string }
  | { kind: 'none'; title: string; note: string };

export type ConflictSide = {
  which: 'local' | 'cloud';
  label: string;
  note: string;
  button: string;
  after: string;
  newer: boolean;
};

const MB = 1024 * 1024;

function bytesShort(b: number): string {
  return b >= 1024 ** 3 ? `${(b / 1024 ** 3).toFixed(1)} GB` : `${(b / MB).toFixed(1)} MB`;
}

function filesText(n: number): string {
  return n === 1 ? '1 file' : `${n} files`;
}

/** The two sides of a conflict: This computer (newer when its change is later) and the store's cloud. */
export function conflictSides(
  source: StoreGame['source'],
  c: NonNullable<StoreCloudState['conflict']>,
  nowSeconds: number,
  prompt = false,
): ConflictSide[] {
  const name = storeName(source);
  const files = filesText(c.local_files);
  // The cloud side as the prototype words it: "· 3 files · 2.0 MB" on the
  // Cloud saves tab, "· 2.0 MB" in the prompt. A file count or size the
  // store's listing didn't give is left out, never guessed.
  const cloudParts = [
    ...(prompt || c.cloud_files === null ? [] : [filesText(c.cloud_files)]),
    ...(c.cloud_bytes === null ? [] : [bytesShort(c.cloud_bytes)]),
  ];
  const cloudTail = cloudParts.length > 0 ? ` · ${cloudParts.join(' · ')}` : '';
  return [
    {
      which: 'local',
      label: 'This computer',
      note: prompt
        ? `Changed ${changedAt(c.local_at, nowSeconds)} · ${bytesShort(c.local_bytes)}`
        : `Changed ${changedAt(c.local_at, nowSeconds)} · ${files} · ${bytesShort(c.local_bytes)}`,
      button: 'Keep this computer’s save',
      after: prompt ? '' : `Uploads it to ${name}.`,
      newer: c.local_at >= c.cloud_at,
    },
    {
      which: 'cloud',
      label: `${name} cloud`,
      note: `Changed ${changedAt(c.cloud_at, nowSeconds)} on another computer${cloudTail}`,
      button: 'Use the cloud save',
      after: prompt ? '' : 'Replaces the save on this computer.',
      newer: c.cloud_at > c.local_at,
    },
  ];
}

export function conflictNote(source: StoreGame['source']): string {
  return `Both this computer’s save and the one in your ${storeName(source)} account changed since the last sync. Pick the one to keep; the other is replaced.`;
}

export function cloudPanel(game: StoreGame, state: StoreCloudState | null, nowSeconds: number): CloudPanel {
  const name = storeName(game.source);
  const kind = state?.state ?? (game.cloud === 'unknown' ? 'synced' : game.cloud);
  if (kind === 'conflict' && state?.conflict) {
    return { kind: 'conflict', title: 'These saves changed in two places', note: conflictNote(game.source), sides: conflictSides(game.source, state.conflict, nowSeconds) };
  }
  if (kind === 'off') {
    return { kind: 'off', title: 'Cloud saves are off for this game', note: `Turn them on in Game Settings, or for every ${name} game in Settings › Stores.` };
  }
  if (kind === 'none') return { kind: 'none', title: 'This game has no cloud saves', note: `${name} doesn’t keep saves for it.` };
  const busy = kind === 'syncing';
  const at = state?.synced_at || game.last_sync;
  return {
    kind: 'synced',
    title: busy ? 'Syncing…' : at > 0 ? `Synced ${agoText(at, nowSeconds)}` : 'Not synced yet',
    note: `GRID syncs with ${name} before the game starts and after it closes.`,
    button: busy ? 'Syncing…' : 'Sync now',
    busy,
  };
}

export function saveLocationsNote(source: StoreGame['source']): string {
  return `${storeName(source)} says where the game keeps its saves. RomM cloud saves are off for store games.`;
}

/** The conflict prompt before launch: "Which save should Lanternfall use?". */
export function conflictPromptTitle(title: string): string {
  return `Which save should ${title} use?`;
}

// ── Game Settings ───────────────────────────────────────────────────────

export type ExecutableChoice = { value: string; label: string };

/**
 * Game Settings › Executable's list (prototype `stSettingsHTML`): the game's
 * executables, labelled from its folder (`Launcher.exe`, `game/run`), with
 * the current one first when the list doesn't hold it (a file the user
 * pinned elsewhere, or one the folder no longer has).
 */
export function executableChoices(current: string, candidates: readonly string[], installDir: string): ExecutableChoice[] {
  const label = (path: string) => relativeTo(path, installDir);
  const cur = current.trim();
  const out: ExecutableChoice[] = candidates.map((value) => ({ value, label: label(value) }));
  if (cur !== '' && !candidates.some((c) => samePath(c, cur))) out.unshift({ value: cur, label: label(cur) });
  return out;
}

/** The choice in `choices` that is `current`, so a list keeps its selection across separators. */
export function chosenExecutable(current: string, choices: readonly ExecutableChoice[]): string {
  return choices.find((c) => samePath(c.value, current))?.value ?? current;
}

function norm(path: string): string {
  return path.trim().replace(/\\/g, '/').replace(/\/+$/, '');
}

function samePath(a: string, b: string): boolean {
  return norm(a) === norm(b);
}

/** `path` below `dir` with `/` separators, else `path` itself. */
function relativeTo(path: string, dir: string): string {
  const p = norm(path);
  const d = norm(dir);
  return d !== '' && p.startsWith(`${d}/`) ? p.slice(d.length + 1) : path.trim();
}

export function executableHelp(source: StoreGame['source']): string {
  const name = storeName(source);
  return `Set by ${name}. Picking another one makes it your choice; ${name} updates won’t change it.`;
}

export function cloudSwitchLabel(source: StoreGame['source']): string {
  return `Sync this game’s cloud saves with ${storeName(source)}`;
}

export const LINUX_RUNS_DIRECTLY = 'Linux games run directly.';
