// TV Details (checklist items 270–318, 430–437; tvmode.js `details`,
// `detBody`, `detTabStep`, `savesOf`, `ACT.*`): the left column's lines, the
// header line, the button row per state, the tabs, the Saves wording and the
// confirmations' words. Pure: TvDetails reads the shared `DetailsStore` and
// `CloudStore` and lays these out.
import type { CloudRecord, SaveScope, SaveType } from '../../api';
import type { DetailsTab } from '../../details/tabs';
import { DETAILS_TAB_LABELS } from '../../details/tabs';
import { ago, daysSince } from '../tabs/server/discoverRows';
import { formatDay } from '../../discover/feed';
import { summaryLine, unlockDate } from '../../details/achievements';
import type { AchievementsView } from '../../api';
import type { Hint } from '../hints';
import { noEmulatorHead, noEmulatorTail, type NoEmulator } from '../../details/noEmulator';
import type { MediaItem } from '../../details/media';

/** One run of text in a line; `b` is bold (the prototype's `<b>`). */
export type Seg = { text: string; b?: boolean };
export type Line = Seg[];

/** The live install for the game: queued, or running with its percent. */
export type DetLive = { queued: boolean; pct: number; cancellable: boolean };

export type DetState = {
  title: string;
  /** A Windows or Linux game (Install app, Game settings). */
  native: boolean;
  installed: boolean;
  /** Installed, its files not found (9c). */
  missing: boolean;
  /** An installed row with no server game (Q4). */
  unlinked: boolean;
  /** No server id and nothing installed. */
  noId: boolean;
  live: DetLive | null;
  /** A session runs for the game. */
  playing: boolean;
  /** The Update button's label ("Update", "Update to v1.2"), or null. */
  updateLabel: string | null;
  /** PS4 / Xbox 360 extra content the server has. */
  content: { update: boolean; dlc: boolean };
  /** Shown from a Backup drive: the pack's name. */
  drive: string | null;
  connected: boolean;
};

export type DetButtonKind =
  | 'cancel'
  | 'play'
  | 'stop'
  | 'install'
  | 'install-drive'
  | 'remove-drive'
  | 'link'
  | 'remove-row'
  | 'locate'
  | 'update'
  | 'install-update'
  | 'install-dlc'
  | 'game-settings'
  | 'uninstall'
  | 'backup';

export type DetButton = {
  kind: DetButtonKind;
  label: string;
  primary?: boolean;
  danger?: boolean;
  icon?: 'play' | 'download';
  disabled?: boolean;
  /** What A on it says while disabled. */
  why?: string;
};

/** Why Backup to drive… is disabled offline (item 437). */
export const BACKUP_NEEDS_SERVER = 'Backing up needs a server.';

/** A on Install offline (items 279, 289). */
export function installOfflineWhy(title: string): string {
  return `Couldn't add ${title} to the download queue: the server can't be reached`;
}

/** The desktop's reason Cancel can't stop an install that is extracting. */
export const CANCEL_EXTRACTING = 'Installing — this step cannot be cancelled';

function cancelButton(live: DetLive): DetButton {
  return {
    kind: 'cancel',
    label: live.queued ? 'Cancel · queued' : `Cancel install · ${live.pct}%`,
    primary: true,
    ...(live.cancellable ? {} : { disabled: true, why: CANCEL_EXTRACTING }),
  };
}

/**
 * The one row of buttons, left to right (items 276–283, 431, 435, 437).
 * Uninstall and Remove use the danger style. While a session runs, Play
 * becomes Stop game (the desktop's Stop).
 */
export function detButtons(s: DetState): DetButton[] {
  const out: DetButton[] = [];
  if (s.drive !== null) {
    if (s.live) out.push(cancelButton(s.live));
    else if (s.installed) out.push(s.playing ? { kind: 'stop', label: 'Stop game', danger: true } : { kind: 'play', label: 'Play', primary: true, icon: 'play' });
    else out.push({ kind: 'install-drive', label: 'Install from drive', primary: true, icon: 'download' });
    out.push({ kind: 'remove-drive', label: 'Remove from drive…', danger: true });
  } else if (s.unlinked) {
    out.push({ kind: 'link', label: 'Link to server game', primary: true });
    out.push({ kind: 'remove-row', label: 'Remove from library (keeps files)', danger: true });
  } else if (s.missing) {
    out.push({ kind: 'locate', label: 'Locate…', primary: true });
    out.push({ kind: 'remove-row', label: 'Remove from library (keeps files)', danger: true });
  } else if (s.noId) {
    // Nothing to act on: the left column says why.
  } else if (s.live) out.push(cancelButton(s.live));
  else if (s.playing) out.push({ kind: 'stop', label: 'Stop game', danger: true });
  else if (s.installed) out.push({ kind: 'play', label: 'Play', primary: true, icon: 'play' });
  else
    out.push({
      kind: 'install',
      label: s.native ? 'Install app' : 'Install',
      primary: true,
      icon: 'download',
      ...(s.connected ? {} : { disabled: true, why: installOfflineWhy(s.title) }),
    });
  const ready = s.installed && !s.missing && !s.unlinked;
  if (ready && s.updateLabel !== null) out.push({ kind: 'update', label: s.updateLabel });
  if (ready && s.drive === null && !s.live) {
    if (s.content.update) out.push({ kind: 'install-update', label: 'Install update' });
    if (s.content.dlc) out.push({ kind: 'install-dlc', label: 'Install DLC' });
  }
  if (ready && s.native) out.push({ kind: 'game-settings', label: 'Game settings' });
  if (ready && s.drive === null) out.push({ kind: 'uninstall', label: 'Uninstall', danger: true });
  if (s.drive === null && !s.unlinked && !s.noId)
    out.push({ kind: 'backup', label: 'Backup to drive…', ...(s.connected ? {} : { disabled: true, why: BACKUP_NEEDS_SERVER }) });
  return out;
}

export type LeftInput = {
  installed: boolean;
  missing: boolean;
  unlinked: boolean;
  noId: boolean;
  lastPlayedAt: number;
  /** Now, in epoch seconds (for "Last played <when>"). */
  nowSeconds: number;
  /** The desktop's launch line ("Snes9x", "RetroArch · snes9x", "No
   *  default emulator"); '' for a native game, which states none. */
  launchTarget: string;
  /** The backend's reason the install would be refused; ''. */
  installBlocked: string;
  /** The game's system has no emulator (ruling A, items 273, 288); null otherwise. */
  noEmulator: NoEmulator | null;
  /** "1.2 GB", or ''. */
  size: string;
  /** "On Couch kit" when connected drives hold the game, or ''. */
  onDrives: string;
  drive: string | null;
  /** A drive copy's lines ("A copy of an installed game · v1.1", …). */
  driveCopy: string[];
};

/** "Last played <when or never>" (item 272): relative, as the hero's
 *  "Played <when>" (items 161–162). */
export function lastPlayedWhen(lastPlayedAt: number, nowSeconds: number): string {
  return lastPlayedAt > 0 ? ago(daysSince(lastPlayedAt, nowSeconds)) : 'never';
}

/** "No emulator for <system>." and what Install does about it (ruling A,
 *  items 273, 288; the line is shared with the desktop's Details). */
export function noEmulatorLine(n: NoEmulator): Line {
  return [{ text: noEmulatorHead(n), b: true }, { text: ` ${noEmulatorTail(n)}` }];
}

/** The left column's status lines (items 272–273, 281–282, 432–433, 436). */
export function leftLines(i: LeftInput): Line[] {
  if (i.drive !== null) {
    const lines: Line[] = [];
    if (i.installed) lines.push([{ text: 'Installed on this computer.' }]);
    lines.push([{ text: 'From ' }, { text: i.drive, b: true }, ...(i.size ? [{ text: ` · ${i.size}` }] : [])]);
    lines.push([{ text: 'Installing copies it from the drive, no network needed.' }]);
    for (const l of i.driveCopy) lines.push([{ text: l }]);
    return lines;
  }
  const lines: Line[] = [];
  if (i.missing) lines.push([{ text: 'Files not found.', b: true }, { text: ' Play and Uninstall are unavailable until the files are found.' }]);
  else if (i.unlinked) lines.push([{ text: 'Not linked to a server game.', b: true }, { text: ' Play, cloud saves and updates need the link.' }]);
  else if (i.noId) lines.push([{ text: 'This entry has no server id' }]);
  else if (i.installed) {
    lines.push([{ text: 'Last played ' }, { text: lastPlayedWhen(i.lastPlayedAt, i.nowSeconds), b: true }]);
    if (i.launchTarget !== '') {
      const emu = i.launchTarget === 'No default emulator' ? 'no emulator' : i.launchTarget;
      lines.push([{ text: 'Launches with ' }, { text: emu, b: true }]);
    }
  } else if (i.noEmulator) lines.push(noEmulatorLine(i.noEmulator));
  else if (i.installBlocked !== '') lines.push([{ text: i.installBlocked, b: true }]);
  else lines.push([{ text: i.size ? `Not installed · ${i.size}` : 'Not installed' }]);
  if (i.onDrives !== '') {
    const pack = i.onDrives.replace(/^On /, '');
    lines.push([{ text: 'On ' }, { text: pack, b: true }]);
  }
  return lines;
}

/** "From <pack> · <Platform> · <year> · <developer> · <genres> · <rating>"
 *  (items 274, 430): the desktop's header line with the drive first and the
 *  rating last. */
export function tvHeaderLine(header: string, drive: string | null, rating: string): string {
  return [drive !== null ? `From ${drive}` : '', header, rating.trim()].filter((p) => p !== '').join(' · ');
}

/** The tabs between the LB and RB glyphs (item 291). */
export function tabLabel(tab: DetailsTab): string {
  return DETAILS_TAB_LABELS[tab];
}

/** LB / RB: the next tab, wrapping at the ends (item 292). */
export function tabStep(tabs: readonly DetailsTab[], current: DetailsTab, d: -1 | 1): DetailsTab {
  const i = Math.max(0, tabs.indexOf(current));
  return tabs[(i + d + tabs.length) % tabs.length];
}

// ---- Overview

export type Fact = { key: string; label: string; value: string };

/** The facts panel (item 295): the desktop's Overview facts that have a
 *  value, in its order. */
export function factsOf(rows: readonly (readonly [string, string, string])[]): Fact[] {
  return rows.filter(([, , v]) => v.trim() !== '').map(([key, label, value]) => ({ key, label, value }));
}

/** The Release fact (item 295): "d Mon yyyy" in UTC, as the prototype's
 *  relDate; '' for 0, never or a value that isn't a number. */
export function releaseFact(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) return '';
  return formatDay(seconds * 1000);
}

/** Related shows up to six games (item 295). */
export const RELATED_MAX = 6;

// ---- Media

/** A tile's A hint (item 302). */
export function mediaAction(item: MediaItem): string {
  return item.kind === 'screenshot' ? 'View' : 'Play video';
}

/** The viewer's caption between LB and RB (item 303). */
export function viewerCaption(items: readonly MediaItem[], index: number, title: string): string {
  const item = items[index];
  if (!item) return '';
  if (item.kind !== 'screenshot') return `Trailer · ${title}`;
  const shots = items.filter((m) => m.kind === 'screenshot');
  return `Screenshot ${shots.indexOf(item) + 1} of ${shots.length} · ${title}`;
}

/** What the viewer shows, for its controls: a screenshot, a video that
 *  plays (a server video, or the YouTube player until it falls back), or a
 *  trailer's poster (the YouTube fallback). */
export type ViewerShow = 'screenshot' | 'video' | 'poster' | null;

export function viewerShow(kind: MediaItem['kind'] | null, embedShowing: boolean): ViewerShow {
  if (kind === null) return null;
  if (kind === 'screenshot') return 'screenshot';
  if (kind === 'youtube') return embedShowing ? 'video' : 'poster';
  return 'video';
}

/** What a viewer button does (user decision 2026-11-10, replacing ruling
 *  E6): while a video shows, LB / RB seek 10 s, LT / RT step its volume and
 *  Left / Right page; otherwise LB / RB and Left / Right page. X opens a
 *  trailer on YouTube, playing or not. */
export type ViewerAct = 'prev' | 'next' | 'back' | 'forward' | 'volDown' | 'volUp' | 'watch' | null;

export const VIEWER_BUTTONS = ['LB', 'RB', 'LEFT', 'RIGHT', 'LT', 'RT', 'X'] as const;
export type ViewerButton = (typeof VIEWER_BUTTONS)[number];

export function viewerAction(b: ViewerButton, show: ViewerShow, youtube: boolean): ViewerAct {
  if (b === 'LEFT') return 'prev';
  if (b === 'RIGHT') return 'next';
  if (b === 'X') return youtube ? 'watch' : null;
  if (show === 'video') {
    if (b === 'LB') return 'back';
    if (b === 'RB') return 'forward';
    if (b === 'LT') return 'volDown';
    return 'volUp';
  }
  if (b === 'LB') return 'prev';
  if (b === 'RB') return 'next';
  return null;
}

/** Held LT / RT repeat like a direction while a video shows. */
export function viewerRepeats(b: string, show: ViewerShow): boolean {
  return show === 'video' && (b === 'LT' || b === 'RT');
}

/** The viewer's hints (item 305). A video: A Pause / Play, LB / RB seek,
 *  LT / RT volume (user decision 2026-11-10); a trailer's poster: A Watch on
 *  YouTube. */
export function viewerHints(show: ViewerShow, paused: boolean): Hint[] {
  if (show === 'video') {
    return [
      { button: 'A', label: paused ? 'Play' : 'Pause' },
      { button: 'LB', label: 'Back 10 s' },
      { button: 'RB', label: 'Forward 10 s' },
      { button: 'LT', label: 'Volume −' },
      { button: 'RT', label: 'Volume +' },
      { button: 'B', label: 'Close' },
    ];
  }
  const base: Hint[] = [
    { button: 'LB', label: 'Previous' },
    { button: 'RB', label: 'Next' },
    { button: 'B', label: 'Close' },
  ];
  return show === 'poster' ? [{ button: 'A', label: 'Watch on YouTube' }, ...base] : base;
}

/** Where a 10 s seek lands: inside the video. */
export function seekTarget(current: number, duration: number, delta: number): number {
  const end = Number.isFinite(duration) && duration > 0 ? duration : Infinity;
  return Math.max(0, Math.min(end, current + delta));
}

/** LB / RB in the viewer: one step through screenshots and videos as one
 *  sequence, stopping at the ends (item 304). */
export function viewerStep(index: number, count: number, d: -1 | 1): number {
  return Math.max(0, Math.min(count - 1, index + d));
}

// ---- Saves

/** What Saves says instead of a list (item 314), or null for the list. */
export function savesNote(o: { unlinked: boolean; noId: boolean; installed: boolean; lastPlayedAt: number }): string | null {
  if (o.unlinked || o.noId) return 'Cloud saves need the link to a server game.';
  if (!o.installed && o.lastPlayedAt <= 0) return 'No cloud saves yet. They appear after you play.';
  return null;
}

function word(kind: SaveType): 'save' | 'state' {
  return kind === 'save' ? 'save' : 'state';
}

function Word(kind: SaveType): 'Save' | 'State' {
  return kind === 'save' ? 'Save' : 'State';
}

/** The Saves | States switch's labels (item 306). */
export const SAVE_KINDS: readonly (readonly [SaveType, string])[] = [
  ['save', 'Saves'],
  ['state', 'States'],
];

/** "Cloud Saves" / "Cloud States" ("Emulator Saves" for shared media, the
 *  desktop's title) (item 306). */
export function savesHeading(kind: SaveType, scope: SaveScope): string {
  if (kind === 'state') return 'Cloud States';
  return scope === 'per_game' ? 'Cloud Saves' : 'Emulator Saves';
}

/** "Upload latest save" / "Upload latest state" (item 306). */
export function uploadLabel(kind: SaveType, scope: SaveScope): string {
  if (kind === 'save' && scope !== 'per_game') return 'Upload emulator saves';
  return `Upload latest ${word(kind)}`;
}

/** A save row's second line: "<when> · <size>", plus " · slot <n>" (item 307). */
export function saveLine(r: Pick<CloudRecord, 'relative_time' | 'size_text' | 'slot'>): string {
  const slot = (r.slot ?? '').trim();
  return `${r.relative_time} · ${r.size_text}${slot ? ` · slot ${slot}` : ''}`;
}

/** A save row's name: its emulator. */
export function saveName(r: Pick<CloudRecord, 'emulator'>): string {
  return r.emulator.trim() || 'Unknown emulator';
}

/** An empty list (item 313). */
export function savesEmpty(kind: SaveType): string {
  return `No cloud ${word(kind)}s for this game. Upload the latest one, or play and GRID uploads it when the game closes.`;
}

/** The list A on a save row opens (item 308). */
export function saveListTitle(kind: SaveType, r: Pick<CloudRecord, 'relative_time'>): string {
  return `${Word(kind)} from ${r.relative_time}`;
}

export type ConfirmWords = { title: string; body: string; ok: string };

function withWarning(body: string, warning: string): string {
  return warning.trim() ? `${body} ${warning.trim()}` : body;
}

/** Restore to this computer's confirmation (item 309). `warning` is the
 *  shared-media notice, when there is one. */
export function restoreConfirm(kind: SaveType, title: string, warning = ''): ConfirmWords {
  return {
    title: `Restore this ${word(kind)}?`,
    body: withWarning(`It replaces the ${word(kind)} for ${title} on this computer. The one there now is overwritten.`, warning),
    ok: 'Restore',
  };
}

/** Delete from the server's confirmation (item 310). */
export function deleteConfirm(kind: SaveType, warning = ''): ConfirmWords {
  return { title: `Delete this ${word(kind)}?`, body: withWarning("It is removed from your server. This can't be undone.", warning), ok: 'Delete' };
}

export function restoredNotice(kind: SaveType, title: string): string {
  return `${Word(kind)} restored for ${title}`;
}

export function deletedNotice(kind: SaveType): string {
  return `${Word(kind)} deleted from the server`;
}

/** Upload's first notice (item 312); the backend's upload-finished card
 *  follows with the desktop's wording. */
export function uploadingNotice(kind: SaveType): string {
  return `Uploading the latest ${word(kind)}…`;
}

// ---- The action confirmations (items 284–287, 434)

export function cancelConfirm(title: string): ConfirmWords {
  return { title: `Cancel ${title}?`, body: 'The download stops and its files are removed. You can install it again later.', ok: 'Cancel install' };
}

export function updateConfirm(title: string): ConfirmWords {
  return { title: `Update ${title}?`, body: 'Saves and configuration are kept.', ok: 'Update' };
}

export function uninstallConfirm(title: string): ConfirmWords {
  return { title: `Uninstall ${title}?`, body: 'Its files are deleted from this computer. Cloud saves stay on your server.', ok: 'Uninstall' };
}

export function removeRowConfirm(title: string, folder: string): ConfirmWords {
  return {
    title: `Remove ${title} from the library?`,
    body: folder ? `Only the library entry goes. Its folder stays on disk: ${folder}.` : 'Only the library entry goes. No files are deleted.',
    ok: 'Remove from library',
  };
}

export function removeDriveConfirm(title: string, pack: string, installed: boolean): ConfirmWords {
  return {
    title: `Remove ${title} from ${pack}?`,
    body: `Its files leave the drive. ${installed ? 'The copy installed on this computer stays.' : 'It can be backed up again later.'}`,
    ok: 'Remove from drive',
  };
}

export function stopConfirm(title: string): ConfirmWords {
  return { title: `Stop ${title}?`, body: 'Unsaved progress in the game is lost.', ok: 'Stop game' };
}

/** Locate… (item 290). */
export function locateTitle(title: string): string {
  return `Where is ${title}?`;
}

export function locatedNotice(title: string, path: string): { text: string; sub: string } {
  return { text: `${title} found`, sub: `Plays from ${path}` };
}

export function locateFailedNotice(title: string, path: string): { text: string; sub: string } {
  return { text: `Couldn't find ${title} in ${path}`, sub: 'Pick the folder that holds the game.' };
}

// ---- Playing (items 321–325)

/** "m:ss" of a session that started `startedMs` ago. */
export function sessionTime(elapsedMs: number): string {
  const s = Math.max(0, Math.floor(elapsedMs / 1000));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
}

/** "<Platform> · <emulator> · <m:ss>" (item 321); a native game names no
 *  emulator. */
export function playingLine(platform: string, emulator: string, time: string): string {
  return [platform, emulator, time].filter((p) => p.trim() !== '').join(' · ');
}

/** The note under it (item 322). The controller drives Playing whenever
 *  GRID's window is focused (user decision 2026-11-10), so it says so. */
export function playingNote(uploadOnExit: boolean): string {
  return `The game has the controller while it's in front. GRID listens again when you switch back or the game closes${uploadOnExit ? ', and uploads your save when it closes' : ''}.`;
}

/** After the game closes with Upload after exit on (item 325). */
export function uploadingSaveNotice(title: string): string {
  return `Uploading the save for ${title}…`;
}

// ---- Link to server game (items 346–348)

export function linkSub(platform: string): string {
  return `Pick the ${platform} game on your server that this is.`;
}

export function linkedNotice(title: string, server: string): string {
  return `${title} linked to ${server}`;
}

/** A server game's line: "<Platform> · <year>". */
export function linkLine(platform: string, firstRelease: number | null): string {
  if (firstRelease === null || !Number.isFinite(firstRelease) || firstRelease <= 0) return platform;
  const secs = firstRelease > 1e11 ? firstRelease / 1000 : firstRelease;
  return `${platform} · ${new Date(secs * 1000).getUTCFullYear()}`;
}

export const LINK_NO_MATCH = 'No games match.';

// ---- Game settings (items 342–345)

/** The launch file's line: "Set by game.json" while game.json chose it,
 *  else "Your choice". */
export function launchFileSub(launchSource: string, selected: string, saved: string): string {
  return launchSource === 'sidecar' && selected === saved ? 'Set by game.json' : 'Your choice';
}

export const GAME_SETTINGS_SAVED = 'Game settings saved';
export const LAUNCH_INFO_RELOADED = 'Launch info reloaded';

// ---- The refusal prompts (README § Consultant decisions; checklist 529–530)

/** Install or Play refused for want of umu-run: the confirm's words. */
export function umuConfirm(title: string): ConfirmWords {
  return {
    title: 'Install umu-launcher?',
    body: `${title} runs through Proton, which needs umu-run. umu-launcher downloads as a Downloads row; launch ${title} again when it finishes.`,
    ok: 'Install umu-launcher',
  };
}

/** Install refused for want of a library folder: the confirm's words. */
export function libraryFolderConfirm(): ConfirmWords {
  return {
    title: 'Set a library folder',
    body: 'GRID installs games into a library folder, and none is set. Choose one, then press Install again.',
    ok: 'Choose a folder…',
  };
}

export const LIBRARY_FOLDER_BROWSER_TITLE = 'Library folder';
/** The card once the folder is saved; the path is its second line. */
export const LIBRARY_FOLDER_SET = 'Library folder set';

// ---- Details › Saves › Save locations on a native game (checklist 531)

export const SAVE_LOCATIONS_HEADING = 'Save locations';
export const ADD_SAVE_FOLDER = 'Add a save folder…';
export const SAVE_FOLDER_ADDED = 'Save folder added';

/** The folder browser's title for a new save folder. */
export function saveFolderBrowserTitle(title: string): string {
  return `Save folder for ${title}`;
}

/** A on a save location: asks first, naming the row's path. */
export function removeLocationConfirm(raw: string): ConfirmWords {
  return { title: 'Remove this save location?', body: raw, ok: 'Remove' };
}

/** The Achievements summary (item 315): the desktop's line, but with
 *  "· <h> hardcore" always shown once progress is known, 0 included. */
export function achSummary(view: AchievementsView): string {
  const line = summaryLine(view);
  if (!view.progress_known || view.summary.hardcore > 0) return line;
  return `${line} · 0 hardcore`;
}

/** "<when>" in "Unlocked <when>" (item 315): relative, as the prototype's
 *  ago(). RomM's stamp (`2024-03-02 10:00:00`, UTC) falls back to its date
 *  when it can't be read. */
export function unlockedWhen(raw: string | null, nowSeconds: number): string {
  if (!raw) return '';
  const t = raw.trim();
  const iso = t.replace(' ', 'T');
  const ms = Date.parse(/[zZ]|[+-]\d{2}:?\d{2}$/.test(iso) ? iso : `${iso}Z`);
  return Number.isNaN(ms) ? unlockDate(t) : ago(daysSince(ms / 1000, nowSeconds));
}
