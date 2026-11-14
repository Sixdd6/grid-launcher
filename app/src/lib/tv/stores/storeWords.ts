// TV mode's game stores (TVStores README; prototype `tvstores.js`,
// `tvstores_bk.js`): what each store screen says and offers with a
// controller. The words and rules are the GameStores card's
// (`lib/gamestores/*`, imported, never edited); this module holds only what
// the TV prototype lays out its own way (the two-row header, one cover tag,
// X's quick action, the game page's button order and left column, the rows
// pages, the Downloads list). Pure.
import type {
  DownloadEntry,
  DriveStoreGame,
  InstallerInspection,
  StoreCloudState,
  StoreGame,
  StoreStatus,
} from '../../api';
import { failureReason, hasStoreBadge, storeBackupLine, storeEntryLine, storeKindLabel, entrySource } from '../../gamestores/downloads';
import { cloudSwitchLabel, storeHeaderLine } from '../../gamestores/details';
import { inspectionView, fileName, type InspectionView } from '../../gamestores/inspection';
import { isLive, jobAct, jobPercent } from '../../gamestores/jobs';
import { hasStoreCloud, noBackupReason, storeName, storeTool } from '../../gamestores/marks';
import { availBlock, type StorePart } from '../../gamestores/rail';
import { agoText, changedAt, plural, sizeGb } from '../../gamestores/text';
import { storePanel, type PanelAct, type StorePanel } from '../../gamestores/view';

// ── Library: the header ─────────────────────────────────────────────────

/** "GOG" and "· Installed" (the muted half). */
export function tvViewTitle(status: StoreStatus, part: StorePart): { name: string; part: string } {
  return { name: storeName(status.source), part: part === 'installed' ? 'Installed' : 'Available' };
}

/**
 * The count line (prototype `libPaint`): "3 games" (Installed), "5 games you
 * own and haven’t installed · updated 5 min ago" (Available), or the plain
 * line while Available can't list them.
 */
export function tvCountLine(status: StoreStatus, part: StorePart, n: number, nowSeconds: number): string {
  if (part === 'installed') return plural(n, 'game');
  if (availBlock(status) !== '') return 'Games you own and haven’t installed';
  return `${plural(n, 'game')} you own and haven’t installed · updated ${agoText(status.refreshed_at, nowSeconds)}`;
}

/** A header chip: plain (with a bold part) or the focusable Login expired one. */
export type TvChip = { key: string; text: string; bold?: string; warn?: boolean; signIn?: boolean };

/** "Signed in as **sam_plays**", "Login expired · Sign in again" (focusable), "Not connected", then "Cloud saves: **on**". */
export function tvChips(status: StoreStatus): TvChip[] {
  const out: TvChip[] = [];
  if (status.connection === 'connected') out.push({ key: 'account', text: 'Signed in as ', bold: status.username });
  else if (status.connection === 'expired') out.push({ key: 'login', text: 'Login expired · Sign in again', warn: true, signIn: true });
  else out.push({ key: 'off', text: 'Not connected' });
  if (hasStoreCloud(status.source) && status.cloud_saves && status.connection !== 'not_connected') {
    out.push({ key: 'cloud', text: 'Cloud saves: ', bold: 'on' });
  }
  return out;
}

/** The header's buttons: Backup all N… (GOG › Installed), Install from installer… (GOG), Refresh (Available, when it lists games). */
export function tvHeaderActions(status: StoreStatus, part: StorePart, installedCount: number): { backupAll: number; installer: boolean; refresh: boolean; select: boolean } {
  const gog = status.source === 'gog';
  return {
    backupAll: gog && part === 'installed' ? installedCount : 0,
    installer: gog,
    refresh: part === 'available' && availBlock(status) === '',
    select: gog && part === 'installed',
  };
}

/** The panel in place of the grid (the GameStores card's), its first button the main one (prototype `B(…, 1)`). */
export function tvPanel(status: StoreStatus, part: StorePart, games: readonly StoreGame[]): StorePanel | null {
  const p = storePanel(status, part, games);
  if (p === null || p.kind !== 'panel') return p;
  return { ...p, actions: p.actions.map((a, i) => ({ ...a, primary: i === 0 })) };
}
export type { PanelAct, StorePanel };

/** A search with no match (TV's own empty state; "(Y)" is the Y glyph). */
export function tvNoMatch(search: string): { title: string; body: string } {
  return { title: `No games match “${search.trim()}”`, body: 'Press (Y) to change the search.' };
}

// ── Covers ──────────────────────────────────────────────────────────────

/** One top-left tag, in the prototype's order: NOT SUPPORTED, SAVE CONFLICT, UPDATE (none while a job runs). */
export function tvCoverTag(game: StoreGame, job: DownloadEntry | null): { text: string; cls: 'sns' | 'scf' | 'upd' } | null {
  if (game.not_supported !== '') return { text: 'NOT SUPPORTED', cls: 'sns' };
  if (game.installed && game.cloud === 'conflict') return { text: 'SAVE CONFLICT', cls: 'scf' };
  if (game.installed && game.update_version !== '' && job === null) return { text: 'UPDATE', cls: 'upd' };
  return null;
}

/** The job's tag ("38%" or "Queued") and its bar's fill. */
export function tvJobTag(job: DownloadEntry | null): { text: string; pct: number } | null {
  if (job === null) return null;
  if (job.status === 'queued') return { text: 'Queued', pct: 0 };
  const pct = jobPercent(job);
  return { text: `${pct}%`, pct };
}

/** X on a store cover (prototype `sQuick`): Play, Install…, or nothing while a job runs, for a game GRID can't install, or signed out. */
export function tvQuick(game: StoreGame, status: StoreStatus | null, job: DownloadEntry | null): 'Play' | 'Install…' | null {
  if (job !== null || game.not_supported !== '') return null;
  if (game.installed) return 'Play';
  return status?.connection === 'connected' ? 'Install…' : null;
}

/** X on a drive's GOG cover: Play when installed here, else Install from drive; none while a job runs. */
export function tvDriveQuick(installedHere: boolean, job: DownloadEntry | null): 'Play' | 'Install from drive' | null {
  if (job !== null) return null;
  return installedHere ? 'Play' : 'Install from drive';
}

// ── The game page ───────────────────────────────────────────────────────

export type TvStoreAct =
  | 'play'
  | 'install'
  | 'cancel'
  | 'restore'
  | 'update'
  | 'settings'
  | 'verify'
  | 'uninstall'
  | 'remove_drive'
  | 'backup';

export type TvStoreButton = { act: TvStoreAct; label: string; primary: boolean; danger?: boolean; disabled?: boolean; why?: string; icon?: 'play' | 'download' };

/**
 * The buttons in the prototype's order (`sdetails`): the main one (Cancel
 * install · 45% / Play / Install from drive / Not supported / Install…), then
 * Update to 2.2, Game settings, Verify and repair, Uninstall, and Remove from
 * drive… or Backup to drive….
 */
export function tvStoreButtons(o: {
  game: StoreGame;
  status: StoreStatus | null;
  job: DownloadEntry | null;
  /** Opened from a drive's grid, which holds this game. */
  fromDrive: boolean;
}): TvStoreButton[] {
  const { game, status, job } = o;
  const name = storeName(game.source);
  const out: TvStoreButton[] = [];
  if (job !== null) {
    const a = jobAct(job);
    const what = a === 'update' ? 'update' : a === 'repair' ? 'verify' : 'install';
    out.push({ act: 'cancel', label: `Cancel ${what}${job.status === 'queued' ? ' · queued' : ` · ${jobPercent(job)}%`}`, primary: true });
  } else if (game.installed) out.push({ act: 'play', label: 'Play', primary: true, icon: 'play' });
  else if (o.fromDrive) out.push({ act: 'restore', label: 'Install from drive', primary: true, icon: 'download' });
  else if (game.not_supported !== '') {
    out.push({ act: 'install', label: 'Not supported', primary: true, disabled: true, why: `Needs ${game.not_supported}. GRID can’t install games that start through another company’s launcher.` });
  } else {
    const conn = status?.connection ?? 'not_connected';
    const why =
      conn === 'connected'
        ? undefined
        : conn === 'not_connected'
          ? `Connect ${name} in Settings › Stores to install.`
          : `Your ${name} login expired. Sign in again in Settings › Stores.`;
    out.push({ act: 'install', label: 'Install…', primary: true, icon: 'download', disabled: why !== undefined, why });
  }
  if (game.installed && game.update_version !== '' && job === null) out.push({ act: 'update', label: `Update to ${game.update_version}`, primary: false });
  if (game.installed) out.push({ act: 'settings', label: 'Game settings', primary: false });
  if (game.installed && job === null) {
    out.push({ act: 'verify', label: 'Verify and repair', primary: false });
    out.push({ act: 'uninstall', label: 'Uninstall', primary: false, danger: true });
  }
  if (o.fromDrive) out.push({ act: 'remove_drive', label: 'Remove from drive…', primary: false });
  else if (game.source === 'gog' && game.installed) out.push({ act: 'backup', label: 'Backup to drive…', primary: false });
  return out;
}

/** A line segment of the left column: plain, bold, or the warning colour. */
export type Seg = { text: string; b?: boolean; warn?: boolean };
export type Line = { key: string; segs: Seg[] };

/**
 * The left column (prototype `sdetails` `lp`): why it can't install, Last
 * played, how it runs, the cloud state, "Installed from an offline
 * installer", and the drive line ("From Couch kit · …" or "On Couch kit").
 */
export function tvLeftLines(o: {
  game: StoreGame;
  nowSeconds: number;
  lastPlayedAt: number;
  compat: string;
  /** The drive the page was opened from, with its record of the game. */
  drive: { name: string; game: DriveStoreGame } | null;
  /** The ready drives that hold this game. */
  onDrives: readonly string[];
}): Line[] {
  const { game } = o;
  const out: Line[] = [];
  if (game.not_supported !== '') {
    out.push({ key: 'ns', segs: [{ text: `Needs ${game.not_supported}.`, b: true }, { text: ' GRID can’t install games that start through another company’s launcher.' }] });
  }
  if (game.installed) out.push({ key: 'played', segs: [{ text: 'Last played ' }, { text: o.lastPlayedAt > 0 ? agoText(o.lastPlayedAt, o.nowSeconds) : 'never', b: true }] });
  out.push({ key: 'runs', segs: runsSegs(game, o.compat) });
  if (hasStoreCloud(game.source) && game.installed) {
    let segs: Seg[];
    if (game.cloud === 'conflict') segs = [{ text: 'Cloud save conflict.', b: true, warn: true }, { text: ' See Cloud saves.' }];
    else if (game.cloud === 'off') segs = [{ text: 'Cloud saves off for this game' }];
    else if (game.cloud === 'none') segs = [{ text: 'No cloud saves for this game' }];
    else segs = game.last_sync > 0 ? [{ text: 'Cloud saves synced ' }, { text: agoText(game.last_sync, o.nowSeconds), b: true }] : [{ text: 'Cloud saves on' }];
    out.push({ key: 'cloud', segs });
  }
  if (game.origin === 'offline') out.push({ key: 'offline', segs: [{ text: 'Installed from an offline installer' }] });
  if (o.drive) {
    const how =
      o.drive.game.layout === 'installer'
        ? 'its installer files and saves. Installing runs the installer again.'
        : 'a copy of an installed game, with its saves. Installing copies it back.';
    out.push({ key: 'drive', segs: [{ text: 'From ' }, { text: o.drive.name, b: true }, { text: ` · ${how}` }] });
  } else if (game.source === 'gog') {
    if (o.onDrives.length > 0) out.push({ key: 'on', segs: [{ text: 'On ' }, { text: o.onDrives.join(', '), b: true }] });
  } else {
    out.push({ key: 'nobk', segs: [{ text: noBackupReason(game.source) }] });
  }
  return out;
}

function runsSegs(game: StoreGame, compat: string): Seg[] {
  if (game.source === 'steam') return [{ text: 'Launches through ' }, { text: 'Steam', b: true }];
  if (!game.installed) {
    return game.platforms.includes('linux')
      ? [{ text: 'Has a ' }, { text: 'Linux version', b: true }]
      : [{ text: 'Windows only · runs through ' }, { text: compat || 'Proton', b: true }];
  }
  if (game.installed_platform === 'linux') return [{ text: 'Runs natively · ' }, { text: 'Linux version', b: true }];
  return [{ text: 'Runs through ' }, { text: game.compat_tool || compat || 'Proton', b: true }, { text: ' · Windows version' }];
}

/** "From Couch kit · GOG · 2022 · Lantern Works · Adventure". */
export function tvHeaderLine(game: StoreGame, driveName: string | null): string {
  const line = storeHeaderLine(game);
  return driveName ? `From ${driveName} · ${line}` : line;
}

/** The chips after the store's: Installed, v1.0.4, Update available, Offline installer, Not supported. */
export function tvDetailChips(game: StoreGame): { text: string; bold?: boolean; warn?: boolean }[] {
  const out: { text: string; bold?: boolean; warn?: boolean }[] = [];
  if (game.installed) out.push({ text: 'Installed', bold: true });
  if (game.installed && game.version.trim() !== '') out.push({ text: `v${game.version.trim()}` });
  if (game.installed && game.update_version !== '') out.push({ text: 'Update available', warn: true });
  if (game.origin === 'offline') out.push({ text: 'Offline installer' });
  if (game.not_supported !== '') out.push({ text: 'Not supported', warn: true });
  return out;
}

export type TvTab = 'Overview' | 'Media' | 'Cloud saves';

/** Overview, Media, and Cloud saves for an installed GOG (or Epic) game. */
export function tvTabs(game: StoreGame): TvTab[] {
  return hasStoreCloud(game.source) && game.installed ? ['Overview', 'Media', 'Cloud saves'] : ['Overview', 'Media'];
}

/** LB / RB: the next tab, wrapping. */
export function tvTabStep(tabs: readonly TvTab[], cur: TvTab, d: -1 | 1): TvTab {
  const i = Math.max(0, tabs.indexOf(cur));
  return tabs[(i + d + tabs.length) % tabs.length];
}

/** Overview's facts: Store, Developer, Release, Size, Version ("v2.1 · GOG v2.2"). */
export function tvFacts(game: StoreGame): { term: string; value: string }[] {
  const name = storeName(game.source);
  const out = [{ term: 'Store', value: name }];
  if (game.developer.trim()) out.push({ term: 'Developer', value: game.developer });
  if (game.year.trim()) out.push({ term: 'Release', value: game.year });
  out.push({ term: 'Size', value: sizeGb(game.installed && game.disk_bytes > 0 ? game.disk_bytes : game.download_bytes) });
  if (game.installed && game.version.trim()) {
    out.push({ term: 'Version', value: `v${game.version}${game.update_version ? ` · ${name} v${game.update_version}` : ''}` });
  }
  return out;
}

/** A DLC row's A hint: Remove / Install on an installed game, else Select. */
export function dlcAction(gameInstalled: boolean, dlcInstalled: boolean): string {
  if (!gameInstalled) return 'Select';
  return dlcInstalled ? 'Remove' : 'Install';
}

// ── Cloud saves ─────────────────────────────────────────────────────────

export type CloudTab =
  | { kind: 'synced'; title: string; sub: string }
  | { kind: 'conflict'; note: string; local: { title: string; sub: string }; cloud: { title: string; sub: string } }
  | { kind: 'off'; title: string; sub: string }
  | { kind: 'none'; note: string };

/** The Cloud saves tab (prototype `sBody`). */
export function tvCloudTab(game: StoreGame, cloud: StoreCloudState | null, nowSeconds: number, syncing: boolean): CloudTab {
  const name = storeName(game.source);
  const state = syncing ? 'syncing' : (cloud?.state ?? (game.cloud === 'unknown' ? 'synced' : game.cloud));
  if (state === 'conflict' && cloud?.conflict) {
    const c = cloud.conflict;
    return {
      kind: 'conflict',
      note: `Both this computer’s save and the one in your ${name} account changed since the last sync. Pick the one to keep; the other is replaced.`,
      local: { title: 'Keep this computer’s save', sub: `Changed ${changedAt(c.local_at, nowSeconds)} · ${mb(c.local_bytes)} · uploads it to ${name}` },
      cloud: {
        title: 'Use the cloud save',
        sub: `Changed ${changedAt(c.cloud_at, nowSeconds)} on another computer${c.cloud_bytes === null ? '' : ` · ${mb(c.cloud_bytes)}`} · replaces the save here`,
      },
    };
  }
  if (state === 'off') return { kind: 'off', title: 'Cloud saves are off for this game', sub: 'Turn them on in Game settings' };
  if (state === 'none') return { kind: 'none', note: `${name} doesn’t keep saves for this game.` };
  const at = cloud?.synced_at || game.last_sync;
  return {
    kind: 'synced',
    title: state === 'syncing' ? 'Syncing…' : at > 0 ? `Synced ${agoText(at, nowSeconds)}` : 'Not synced yet',
    sub: `GRID syncs with ${name} before the game starts and after it closes`,
  };
}

function mb(bytes: number): string {
  return bytes >= 1024 ** 3 ? `${(bytes / 1024 ** 3).toFixed(1)} GB` : `${(bytes / 1024 ** 2).toFixed(1)} MB`;
}

export const ROMM_CLOUD_OFF = 'RomM cloud saves are off for store games.';

/** The confirmations of the two conflict rows. */
export function keepConfirm(which: 'local' | 'cloud', source: StoreGame['source']): { title: string; body: string; ok: string } {
  const name = storeName(source);
  return which === 'local'
    ? { title: 'Keep this computer’s save?', body: `It is uploaded to ${name} and replaces the cloud save.`, ok: 'Keep and upload' }
    : { title: 'Use the cloud save?', body: 'It replaces the save on this computer.', ok: 'Use the cloud save' };
}

/** The list before a launch with a conflict (prototype `sPlay`). */
export function conflictPrompt(game: StoreGame, c: NonNullable<StoreCloudState['conflict']> | null, nowSeconds: number): {
  title: string;
  sub: string;
  local: string;
  cloud: string;
} {
  const name = storeName(game.source);
  return {
    title: `Which save should ${game.title} use?`,
    sub: 'Both changed since the last sync. The game starts after you pick.',
    local: c ? `Changed ${changedAt(c.local_at, nowSeconds)} · uploads it to ${name}` : `Uploads it to ${name}`,
    cloud: c ? `Changed ${changedAt(c.cloud_at, nowSeconds)} on another computer` : 'Replaces the save here',
  };
}

/** The cards after a conflict choice. */
export function keptCard(which: 'local' | 'cloud', game: StoreGame): { text: string; sub?: string } {
  return which === 'local'
    ? { text: `Kept this computer’s save for ${game.title}`, sub: `Uploaded to ${storeName(game.source)}` }
    : { text: `Restored the cloud save for ${game.title}` };
}

/** "Launching Ashen Tide…" with "Synced cloud saves with GOG" when its saves sync. */
export function launchingCard(game: StoreGame): { text: string; sub?: string } {
  const sync = hasStoreCloud(game.source) && game.cloud !== 'off' && game.cloud !== 'none';
  return sync ? { text: `Launching ${game.title}…`, sub: `Synced cloud saves with ${storeName(game.source)}` } : { text: `Launching ${game.title}…` };
}

/** Playing's line: "GOG · Native · 0:12" or "GOG · GE-Proton9-20 · 0:12". */
export function tvPlayingLine(game: StoreGame, compat: string, time: string): string {
  const how = game.installed_platform === 'windows' ? game.compat_tool || compat || 'Proton' : 'Native';
  return `${storeName(game.source)} · ${how} · ${time}`;
}

/** Playing's note while the game's saves sync. */
export function tvPlayingNote(game: StoreGame): string | null {
  if (!hasStoreCloud(game.source) || game.cloud === 'off' || game.cloud === 'none') return null;
  return `Your saves sync with ${storeName(game.source)} when the game closes.`;
}

export function uninstallWords(game: StoreGame): { title: string; body: string; ok: string } {
  const cloud = hasStoreCloud(game.source) ? ` Its cloud saves stay in your ${storeName(game.source)} account.` : '';
  return { title: `Uninstall ${game.title}?`, body: `Its files are deleted from this computer.${cloud}`, ok: 'Uninstall' };
}

export function updateWords(game: StoreGame): { title: string; body: string; ok: string } {
  return { title: `Update ${game.title} to ${game.update_version}?`, body: 'Saves and settings stay.', ok: 'Update' };
}

export function cancelJobWords(title: string, job: DownloadEntry): { title: string; body: string; ok: string } {
  const a = jobAct(job);
  const fresh = a === 'install' || a === 'restore';
  return {
    title: `Cancel ${title}?`,
    body: fresh ? 'The download stops and its files are removed. You can install it again later.' : 'It stops; the game stays as it was.',
    ok: `Cancel ${a === 'repair' ? 'verify' : a === 'restore' ? 'install' : a}`,
  };
}

export function removeDlcWords(dlc: string, game: string): { title: string; body: string; ok: string } {
  return { title: `Remove ${dlc}?`, body: `Its files leave ${game}’s folder. You can install it again later.`, ok: 'Remove' };
}

export function removeFromDriveWords(game: string, pack: string, installedHere: boolean): { title: string; body: string; ok: string } {
  return {
    title: `Remove ${game} from ${pack}?`,
    body: `Its files leave the drive. ${installedHere ? 'The copy installed on this computer stays.' : 'You can back it up again later.'}`,
    ok: 'Remove from drive',
  };
}

export function stopWords(title: string): { title: string; body: string; ok: string } {
  return { title: `Stop ${title}?`, body: 'Unsaved progress in the game is lost.', ok: 'Stop game' };
}

// ── The install page ────────────────────────────────────────────────────

export const COMPAT_SUB = 'The default from Settings › Systems › Windows';

export function installFolderSub(source: StoreGame['source']): string {
  return `The default for ${storeName(source)} games is in Settings › Stores`;
}

/** A folder name from a title, as the prototype makes it ("MarrowindKeep"). */
export function folderName(title: string): string {
  return title.replace(/[^A-Za-z0-9]+/g, '');
}

/** `name` inside `dir`. */
export function joinPath(dir: string, name: string): string {
  if (name === '') return dir;
  const sep = dir.includes('\\') && !dir.includes('/') ? '\\' : '/';
  return dir.endsWith(sep) ? `${dir}${name}` : `${dir}${sep}${name}`;
}

// ── Game settings ───────────────────────────────────────────────────────

export function launchFileSub(source: StoreGame['source'], userPinned: boolean): string {
  return userPinned ? 'Your choice' : `Set by ${storeName(source)}`;
}
export const LINUX_RUNS_DIRECTLY = 'Linux games run directly.';
export function cloudToggleSub(source: StoreGame['source']): string {
  return cloudSwitchLabel(source).replace('Sync this game’s cloud saves with', 'Sync this game’s saves with');
}

// ── Settings › Stores ───────────────────────────────────────────────────

export const STORES_PANE_NOTE = 'Each connected store gets a group in the Library with its Installed and Available games.';

/** The pane's row: "Signed in as sam_plays" (with "· gogdl not installed"), or "Not signed in". */
export function storeRowSub(status: StoreStatus): string {
  if (status.connection === 'not_connected') return 'Not signed in';
  const tool = storeTool(status.source);
  const missing = tool !== null && status.tool.state !== 'installed' ? ` · ${tool.name} not installed` : '';
  return `Signed in as ${status.username}${missing}`;
}

/** The row's status: Connected, Login expired, Not connected. */
export function storeRowPill(status: StoreStatus): { text: string; tone: 'ok' | 'warn' | '' } {
  if (status.connection === 'connected') return { text: 'Connected', tone: 'ok' };
  if (status.connection === 'expired') return { text: 'Login expired', tone: 'warn' };
  return { text: 'Not connected', tone: '' };
}

/** A not-connected store page's note (prototype `sStorePage`). */
export function notConnectedNote(status: StoreStatus): string {
  const name = storeName(status.source);
  const tool = storeTool(status.source) ?? { name: 'its tool', pkg: '' };
  return `Sign in to ${name} to see the games you own in the Library and install them. GRID installs with ${tool.name} (${tool.pkg}, ${
    status.tool.license || 'GPL-3.0'
  }, ${sizeGb(status.tool.size_bytes)}), which it downloads when you connect.${status.source === 'gog' ? ' You can install GOG offline installers without signing in.' : ''}`;
}

/** "8 · 3 installed · updated 5 min ago" ("Loading…" while it loads). */
export function ownedValue(status: StoreStatus, nowSeconds: number): string {
  if (status.owned === 'loading' || status.owned === 'idle') return 'Loading…';
  if (status.owned === 'error') return status.owned_error.trim() || 'Couldn’t load your games';
  return `${status.owned_count} · ${status.installed_count} installed · updated ${agoText(status.refreshed_at, nowSeconds)}`;
}

/** The Store tool row: installed (info), or a warning row with Install / Try again. */
export function toolRow(status: StoreStatus): { info: true; value: string } | { info: false; label: string; sub: string; value: string; busy: boolean } {
  const tool = storeTool(status.source) ?? { name: 'tool', pkg: '' };
  const t = status.tool;
  if (t.state === 'installed') return { info: true, value: `${tool.name} ${t.version} · installed` };
  if (t.state === 'failed') {
    const sub = t.error.trim() === '' || /checksum|sha-?256/i.test(t.error) ? 'The download didn’t match its checksum, so GRID deleted it' : t.error.trim();
    return { info: false, label: `Couldn’t install ${tool.name}`, sub, value: 'Try again', busy: false };
  }
  if (t.state === 'installing') return { info: false, label: 'Store tool', sub: 'Installing…', value: 'Install', busy: true };
  return { info: false, label: 'Store tool', sub: `Not installed · ${sizeGb(t.size_bytes)}`, value: 'Install', busy: false };
}

export function disconnectWords(status: StoreStatus): { title: string; body: string; ok: string } {
  const name = storeName(status.source);
  const tool = storeTool(status.source)?.name ?? 'its tool';
  const n = status.installed_count;
  const stay = n > 0 ? ` The ${n} installed games stay and still play; they can’t install, update${hasStoreCloud(status.source) ? ' or sync cloud saves' : ''} until you connect again.` : '';
  return { title: `Disconnect ${name}?`, body: `GRID deletes your ${name} login and ${tool} from this computer.${stay}`, ok: 'Disconnect' };
}

// ── Signing in ──────────────────────────────────────────────────────────

/** The store's sign-in address, shown read-only in the window's frame. */
export function loginHost(source: StoreGame['source']): string {
  switch (source) {
    case 'gog':
      return 'login.gog.com/auth';
    case 'epic':
      return 'www.epicgames.com/id/login';
    case 'amazon':
      return 'www.amazon.com/ap/signin';
    default:
      return '';
  }
}

export const LOGIN_FOOT = 'GRID never sees your password. This window closes on its own when you’ve signed in.';

// ── The offline installer ──────────────────────────────────────────────

export const PICK_INSTALLER_TITLE = 'Choose a GOG installer';
export const READING_INSTALLER = 'Reading the installer…';

/** TV mode runs a Windows installer silently only (user decision 2026-11-13). */
export function silentOnlyHow(compat: string): string {
  return `GRID runs the installer silently under ${compat || 'its compatibility tool'}. TV mode never opens an installer’s own setup window: if it can’t run silently, GRID stops, keeps the installer and asks you to install it in desktop mode.`;
}

/** The GameStores card's view of an inspected installer, with TV mode's silent-only words for a Windows installer. */
export function tvInspectionView(insp: InstallerInspection, compat: string, gogConnected: boolean): InspectionView {
  const v = inspectionView(insp, { compat, gogConnected });
  if (v.kind === 'found' && insp.kind === 'windows_exe' && insp.verdict.kind !== 'dlc') return { ...v, how: silentOnlyHow(compat) };
  return v;
}

/** A refusal's first line, ending with a full stop as in the prototype's dialog. */
export function refusalTitle(title: string): string {
  const t = title.trim();
  return /[.!?]$/.test(t) ? t : `${t}.`;
}

export { fileName };

// ── Downloads ───────────────────────────────────────────────────────────

/** Whether a Downloads row is a store row (a store badge and the store words). */
export function isStoreRow(e: DownloadEntry): boolean {
  return hasStoreBadge(e);
}

/** A store row's line: the GameStores card's words (backup rows their own). */
export function tvStoreLine(e: DownloadEntry, compat: string, version = ''): string {
  if (e.kind === 'backup_store_game') return storeBackupLine(e);
  return storeEntryLine(e, { compat, version });
}

/** The kind badge after the store's ("Update", "Store tool", "Installer"…), or ''. */
export function tvStoreKind(e: DownloadEntry): string {
  return storeKindLabel(e.kind) ?? '';
}

export type StoreRowAct = 'cancel' | 'retry' | 'dismiss' | 'open';

/** A on a store row (prototype `ACT.dlrow`): Cancel, Retry, Dismiss, Open <game>. */
export function tvStoreActions(e: DownloadEntry, gameTitle: string | null): { act: StoreRowAct; label: string; danger?: boolean }[] {
  const out: { act: StoreRowAct; label: string; danger?: boolean }[] = [];
  if (isLive(e) && e.status !== 'cancelling') out.push({ act: 'cancel', label: 'Cancel', danger: true });
  if (e.status === 'failed' || e.status === 'cancelled') out.push({ act: 'retry', label: 'Retry' });
  if (e.status === 'completed' || e.status === 'failed' || e.status === 'cancelled') out.push({ act: 'dismiss', label: 'Dismiss' });
  if (gameTitle) out.push({ act: 'open', label: `Open ${gameTitle}` });
  return out;
}

/** The row's A hint: Cancel, Retry or dismiss, Dismiss. */
export function tvStoreRowA(e: DownloadEntry): string {
  if (isLive(e)) return 'Cancel';
  if (e.status === 'failed') return 'Retry or dismiss';
  return 'Dismiss';
}

/** The list's line: "GOG · Update". */
export function tvStoreListSub(e: DownloadEntry): string {
  const src = entrySource(e);
  return [src ? storeName(src) : 'GOG', tvStoreKind(e)].filter((s) => s !== '').join(' · ');
}

/** Cancel on a store row asks first, in the prototype's words. */
export function tvStoreCancelWords(e: DownloadEntry): { title: string; body: string; ok: string } {
  if (e.kind === 'backup_store_game') {
    const label = e.drive_label.trim() || e.drive_name.trim() || 'the drive';
    return { title: `Cancel ${e.title}?`, body: `The backup stops and what it wrote to ${label} is removed. It can be backed up again later.`, ok: 'Cancel backup' };
  }
  if (e.kind === 'store_tool') return { title: `Cancel ${e.title}?`, body: 'The download stops. GRID can install it again later.', ok: 'Cancel install' };
  return cancelJobWords(e.title, e);
}

// ── Installer failures (RomM archives, offline installers) ─────────────

export type FailureCard = {
  text: string;
  sub: string;
  action: { kind: 'retry' } | { kind: 'open_downloads' } | { kind: 'install_base'; title: string; key?: string } | null;
};

/**
 * The card a failed installer row raises in TV mode: "Couldn’t install
 * Foxglove silently" with Retry (the installer needs its setup window, which
 * TV mode never opens), or "Couldn’t install Iron Lullaby" with Open
 * Downloads / Install Pale Summit. `null` for a row with no installer failure.
 */
export function tvFailureCard(e: DownloadEntry): FailureCard | null {
  if (e.status !== 'failed' || e.failure === null) return null;
  const f = e.failure;
  switch (f.code) {
    case 'installer_needs_wizard':
      return { text: `Couldn’t install ${e.title} silently`, sub: 'This installer needs its setup window. Install it in desktop mode.', action: { kind: 'retry' } };
    case 'installer_unrecognised':
      return { text: `Couldn’t install ${e.title}`, sub: 'Its archive holds an installer GRID doesn’t recognise.', action: { kind: 'open_downloads' } };
    case 'installer_dlc_base_missing': {
      const base = f.base_title.trim();
      return {
        text: `Couldn’t install ${e.title}`,
        sub: `It’s DLC for ${base || 'a game'}, which isn’t installed.`,
        action: base ? { kind: 'install_base', title: base, ...(f.base_gog_id.trim() ? { key: `gog:${f.base_gog_id.trim()}` } : {}) } : { kind: 'open_downloads' },
      };
    }
    case 'installer_exit':
      return { text: `Couldn’t install ${e.title}`, sub: failureReason(e), action: { kind: 'open_downloads' } };
    default:
      return null;
  }
}

// ── Backups ─────────────────────────────────────────────────────────────

/** The bar under GOG › Installed in select mode: "2 selected · 21.4 GB to write". */
export function gogSelSummary(n: number, bytes: number): string {
  return `${n} selected · ${sizeGb(bytes)} to write`;
}

/** The Backup page's drive row line: "SAMSUNG T7 · exFAT · 412 GB free". */
export function driveRadioSub(label: string, fs: string, freeBytes: number): string {
  return [label, fs, `${sizeGb(freeBytes)} free`].filter((s) => s.trim() !== '').join(' · ');
}

export const NO_BACKUP_DRIVE = 'No backup drive connected. Plug in a drive you set up for GRID, or set one up in Settings › Backup drives.';

/** "Brightwater is already on Couch kit and will be skipped." */
export function skippedNote(name: string, pack: string): string {
  return `${name} is already on ${pack} and will be skipped.`;
}
