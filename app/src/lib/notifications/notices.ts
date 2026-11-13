// What each download, install and launch event says as a notification card
// (design: "One notification style"), and the coalescing that keeps a batch
// from flooding the four-card stack. Pure: `notifier.ts` wires it to the
// stores and `pushToast`; everything here is tested by `notices.test.ts`.
//
// The downloads queue is observed by diffing successive `downloads-changed`
// snapshots (every event carries the whole list), so each transition is seen
// exactly once whichever view started it.

import type { BackupNotice, DownloadEntry, DownloadStatus, GameSession } from '../api';
import type { ToastLevel } from '../stores/toasts.svelte';

/**
 * Cards of one group that arrive together are merged into one summary card.
 * Errors and successes never share a group, so a summary keeps one level.
 */
export type NoticeGroup =
  | 'game-queued'
  | 'update-queued'
  | 'content-queued'
  | 'game-installed'
  | 'game-updated'
  | 'content-installed'
  | 'game-failed'
  | 'cancelled'
  | 'tool-queued'
  | 'tool-updating'
  | 'tool-installed'
  | 'tool-updated'
  | 'tool-failed'
  | 'launch'
  | 'launch-failed'
  | 'removed'
  | 'drive-queued'
  | 'drive-installed'
  | 'drive-failed'
  // What a drive could not bring with a game ("Couch kit has no emulator
  // for PlayStation", checklist item 428).
  | 'drive-note'
  // One card per backup batch (backups/notices.ts).
  | 'backup-queued'
  | 'backup-done'
  | 'backup-failed';

/**
 * A card may offer a place to go: `downloads` is "Open Downloads". The
 * desktop's cards show no action; TV mode's View runs it (checklist items
 * 144, 148–149).
 */
export type NoticeAction =
  | 'downloads'
  /** "Set up <Platform>": that system's page (a launch with no emulator). */
  | { system: string };

/**
 * `sub` is a card's second line. Only TV mode's cards show it ("Copying from
 * SAMSUNG T7", "Used for PlayStation"); the desktop's card is the text alone.
 */
export type Notice = { group: NoticeGroup; text: string; level: ToastLevel; action?: NoticeAction; sub?: string };

const TERMINAL: ReadonlySet<DownloadStatus> = new Set(['completed', 'failed', 'cancelled']);

/** One entry's status move between two snapshots; `from` is null for a new entry. */
export type Transition = { entry: DownloadEntry; from: DownloadStatus | null };

export type StatusBook = Map<number, DownloadStatus>;

/**
 * The transitions between the statuses last seen (`prev`, null before the
 * first snapshot) and `entries`. The first snapshot is a baseline and yields
 * nothing: what was already queued when the app came up is not news.
 */
export function diffSnapshot(
  prev: StatusBook | null,
  entries: readonly DownloadEntry[],
): { book: StatusBook; transitions: Transition[] } {
  const book: StatusBook = new Map(entries.map((e) => [e.id, e.status]));
  if (prev === null) return { book, transitions: [] };
  const transitions: Transition[] = [];
  // Oldest first, so cards stack in the order things happened (a snapshot
  // lists its entries newest first).
  for (const entry of [...entries].reverse()) {
    const from = prev.get(entry.id) ?? null;
    if (from === entry.status) continue;
    if (from === null || isEvent(from, entry.status)) transitions.push({ entry, from });
  }
  return { book, transitions };
}

/** Only arrivals in and out of a terminal status are events; live-to-live
 *  moves (queued → downloading → installing) are progress. */
function isEvent(from: DownloadStatus, to: DownloadStatus): boolean {
  return TERMINAL.has(from) !== TERMINAL.has(to);
}

/** Per-entry wording a caller registered before queueing it. */
export type Hint = {
  /** The queue entry re-installs an emulator that is already there. */
  update?: boolean;
  /** Replaces the completion text (a Proton picked as the Windows default). */
  done?: string;
};

export type NoticeContext = {
  /** The first-run setup card is up and reports every emulator itself. */
  setupActive: boolean;
  hint?: Hint;
};

function withReason(text: string, error: string): string {
  const reason = error.trim();
  return reason === '' ? text : `${text}: ${reason}`;
}

/**
 * The cards one transition earns. A new entry is announced as queued even if
 * it is already downloading (an idle queue starts it at once); one that is
 * also already terminal gets its outcome card too.
 */
export function noticesFor(t: Transition, ctx: NoticeContext): Notice[] {
  const { entry } = t;
  // A backup row's cards are its batch's (backups/notices.ts), never its own.
  if (entry.job === 'backup') return [];
  const out: Notice[] = [];
  const arrived = t.from === null || TERMINAL.has(t.from);
  if (arrived) {
    const q = queuedNotice(entry, ctx);
    if (q) out.push(q);
  }
  if (TERMINAL.has(entry.status)) {
    const o = outcomeNotice(entry, ctx);
    if (o) out.push(o);
  }
  return out;
}

type GameShape = 'base' | 'update' | 'content';

function gameShape(entry: DownloadEntry): GameShape {
  if (entry.kind === 'update' || entry.kind === 'native_update') return 'update';
  if (entry.kind === 'ps4_content' || entry.kind === 'xbox360_content') return 'content';
  return 'base';
}

/** A game row's title without the kind suffix the queue gives update and
 *  content rows ("Chrono Trigger (update)", "Bloodborne (dlc)"). */
function gameTitle(entry: DownloadEntry): string {
  return gameShape(entry) === 'base' ? entry.title : entry.title.replace(/\s\((update|dlc)\)$/i, '');
}

function isTool(entry: DownloadEntry): boolean {
  return entry.job === 'emulator' && (entry.kind === 'emulator' || entry.kind === 'compat_tool');
}

function queuedNotice(entry: DownloadEntry, ctx: NoticeContext): Notice | null {
  const t = entry.job === 'game' ? gameTitle(entry) : entry.title;
  if (entry.job === 'game') {
    switch (gameShape(entry)) {
      case 'update':
        return { group: 'update-queued', text: `Update for ${t} added to download queue`, level: 'success', action: 'downloads' };
      case 'content':
        return { group: 'content-queued', text: `Extra content for ${t} added to download queue`, level: 'success', action: 'downloads' };
      default: {
        const n: Notice = { group: 'game-queued', text: `${t} added to download queue`, level: 'success', action: 'downloads' };
        // An install from a Backup drive says where it copies from (TV
        // checklist 426; the Game backups card's "Copying from <label>").
        const label = entry.drive_label.trim();
        if (label !== '') n.sub = `Copying from ${label}`;
        return n;
      }
    }
  }
  // An emulator or Proton an install from a drive brings is announced by
  // that command's own notice ("DuckStation from Couch kit added to download
  // queue"), so its queue row stays quiet.
  if (isTool(entry) && !ctx.setupActive && entry.drive_label.trim() === '') {
    return ctx.hint?.update
      ? { group: 'tool-updating', text: `Updating ${t}…`, level: 'success', action: 'downloads' }
      : { group: 'tool-queued', text: `Installing ${t}…`, level: 'success', action: 'downloads' };
  }
  return null;
}

function outcomeNotice(entry: DownloadEntry, ctx: NoticeContext): Notice | null {
  const t = entry.job === 'game' ? gameTitle(entry) : entry.title;
  if (entry.job === 'game') {
    const shape = gameShape(entry);
    switch (entry.status) {
      case 'completed':
        if (shape === 'update') return { group: 'game-updated', text: `${t} updated`, level: 'success' };
        if (shape === 'content')
          return { group: 'content-installed', text: `Extra content for ${t} installed`, level: 'success' };
        return { group: 'game-installed', text: `${t} installed`, level: 'success' };
      case 'failed': {
        // A failure in the archive's GOG installer has its own card, worded
        // from `failure` (gamestores/notices.ts `installerFailedNotice`).
        if (entry.failure) return null;
        const what = shape === 'update' ? `Couldn't update ${t}` : shape === 'content' ? `Couldn't install extra content for ${t}` : `Couldn't install ${t}`;
        return { group: 'game-failed', text: withReason(what, entry.error), level: 'error', action: 'downloads' };
      }
      case 'cancelled':
        return {
          group: 'cancelled',
          text: shape === 'update' ? `Cancelled the update for ${t}` : `Cancelled ${t}`,
          level: 'success',
        };
      default:
        return null;
    }
  }
  if (!isTool(entry) || ctx.setupActive) return null;
  switch (entry.status) {
    case 'completed':
      // An emulator's completion is the `emulator-installed` event's card
      // (worded install or update by the backend); a compat tool has none.
      if (entry.kind !== 'compat_tool') return null;
      return { group: 'tool-installed', text: ctx.hint?.done ?? `${t} installed`, level: 'success' };
    case 'failed':
      return { group: 'tool-failed', text: withReason(`Couldn't install ${t}`, entry.error), level: 'error', action: 'downloads' };
    case 'cancelled':
      return { group: 'cancelled', text: `Cancelled ${t}`, level: 'success' };
    default:
      return null;
  }
}

/** "A", "A and B", "A, B and C". */
function joinNames(names: readonly string[]): string {
  if (names.length <= 1) return names.join('');
  return `${names.slice(0, -1).join(', ')} and ${names[names.length - 1]}`;
}

/**
 * The `emulator-installed` event's card: a fresh install or an update. A
 * fresh install that systems switched to says so on its second line
 * ("Used for PlayStation and Saturn"; TV only, the desktop card is the
 * title alone).
 */
export function emulatorInstalledNotice(name: string, fresh: boolean, usedFor: readonly string[] = []): Notice {
  if (!fresh) return { group: 'tool-updated', text: `${name} updated`, level: 'success' };
  const n: Notice = { group: 'tool-installed', text: `${name} installed`, level: 'success' };
  const systems = usedFor.map((s) => s.trim()).filter((s) => s !== '');
  if (systems.length > 0) n.sub = `Used for ${joinNames(systems)}`;
  return n;
}

export function launchingNotice(title: string): Notice {
  return { group: 'launch', text: `Launching ${title}…`, level: 'success' };
}

export function launchFailedNotice(title: string, reason: string, setUp?: string): Notice {
  const n: Notice = { group: 'launch-failed', text: withReason(`Couldn't launch ${title}`, reason), level: 'error' };
  // TV mode's card offers the system's page ("Set up <Platform>",
  // checklist item 318); the desktop's card shows no action.
  if (setUp) n.action = { system: setUp };
  return n;
}

/** A refused Install: nothing was queued. */
export function queueRefusedNotice(title: string, reason: string): Notice {
  return { group: 'game-failed', text: withReason(`Couldn't add ${title} to the download queue`, reason), level: 'error' };
}

export function uninstalledNotice(title: string): Notice {
  return { group: 'removed', text: `${title} uninstalled`, level: 'success' };
}

export function removedFromLibraryNotice(title: string): Notice {
  return { group: 'removed', text: `${title} removed from the library`, level: 'success' };
}

/**
 * A line `install_from_drive` answers with: something it queued from the
 * drive beside the game ("DuckStation from Couch kit added to download
 * queue"). Its own group, so a firmware "installed from" card arriving in the
 * same moment is not summed with it as "2 items installed".
 */
export function driveQueuedNotice(message: string): Notice {
  return { group: 'drive-queued', text: message, level: 'success', action: 'downloads' };
}

/**
 * A line of `install_from_drive`'s answer as its card: what it queued
 * (`driveQueuedNotice`), or what the drive could not bring ("<pack> has no
 * emulator for <system>", item 428), which queued nothing and opens nothing.
 */
export function driveOutcomeNotice(line: string): Notice {
  return / has no emulator for /.test(line) ? { group: 'drive-note', text: line, level: 'success' } : driveQueuedNotice(line);
}

/** A `backup-notice` event's card ("PlayStation firmware installed from Couch kit"); the backend words it. */
export function backupNoticeCard(notice: BackupNotice): Notice {
  return notice.error
    ? { group: 'drive-failed', text: notice.message, level: 'error' }
    : { group: 'drive-installed', text: notice.message, level: 'success' };
}

/**
 * The early-exit warning a `sessions-changed` snapshot carries, prefixed
 * with the game it was about when exactly one session went away with it.
 */
export function earlyExitNotice(
  before: readonly GameSession[],
  after: readonly GameSession[],
  warning: string,
): Notice {
  const still = new Set(after.map((s) => s.id));
  const gone = before.filter((s) => !still.has(s.id));
  const text = gone.length === 1 ? `${gone[0].title} — ${warning}` : warning;
  return { group: 'launch-failed', text, level: 'error' };
}

function plural(n: number, one: string, many: string): string {
  return `${n} ${n === 1 ? one : many}`;
}

/** The one card that stands for `n` cards of `group` arriving together. */
export function summaryText(group: NoticeGroup, n: number): string {
  const games = plural(n, 'game', 'games');
  switch (group) {
    case 'game-queued':
      return `${games} added to download queue`;
    case 'update-queued':
      return `${plural(n, 'update', 'updates')} added to download queue`;
    case 'content-queued':
      return `Extra content for ${games} added to download queue`;
    case 'game-installed':
      return `${games} installed`;
    case 'game-updated':
      return `${games} updated`;
    case 'content-installed':
      return `Extra content for ${games} installed`;
    case 'game-failed':
      return `${plural(n, 'download', 'downloads')} failed — see Downloads`;
    case 'cancelled':
      return `Cancelled ${plural(n, 'download', 'downloads')}`;
    case 'tool-queued':
      return `Installing ${plural(n, 'emulator', 'emulators')}…`;
    case 'tool-updating':
      return `Updating ${plural(n, 'emulator', 'emulators')}…`;
    case 'tool-installed':
      return `${plural(n, 'emulator', 'emulators')} installed`;
    case 'tool-updated':
      return `${plural(n, 'emulator', 'emulators')} updated`;
    case 'tool-failed':
      return `${plural(n, 'emulator install', 'emulator installs')} failed — see Downloads`;
    case 'launch':
      return `Launching ${games}…`;
    case 'launch-failed':
      return `${plural(n, 'launch', 'launches')} failed`;
    case 'removed':
      return `${games} removed`;
    case 'drive-queued':
      return `${plural(n, 'item', 'items')} from a backup drive added to download queue`;
    case 'drive-installed':
      return `${plural(n, 'item', 'items')} installed from a backup drive`;
    case 'drive-failed':
      return `${plural(n, 'item', 'items')} from a backup drive could not be installed`;
    case 'drive-note':
      return `A backup drive has no emulator for ${plural(n, 'system', 'systems')}`;
    case 'backup-queued':
      return `${plural(n, 'backup', 'backups')} added to the backup queue`;
    case 'backup-done':
      return `${plural(n, 'backup', 'backups')} finished`;
    case 'backup-failed':
      return `${plural(n, 'backup', 'backups')} stopped — see Downloads`;
  }
}

/**
 * Notices that arrive within `windowMs` of each other are held and then shown
 * per group: one notice as itself, several as one summary card. The window
 * restarts with each arrival but never holds a card past `maxMs`.
 */
export const COALESCE_WINDOW_MS = 400;
export const COALESCE_MAX_MS = 1_500;

export type Timers = {
  set: (fn: () => void, ms: number) => unknown;
  clear: (handle: unknown) => void;
};

const realTimers: Timers = {
  set: (fn, ms) => setTimeout(fn, ms),
  clear: (handle) => clearTimeout(handle as ReturnType<typeof setTimeout>),
};

export class Coalescer {
  private held: Notice[] = [];
  private timer: unknown = null;
  private firstAt = 0;

  constructor(
    private readonly show: (text: string, level: ToastLevel, card: Notice) => void,
    private readonly now: () => number = () => Date.now(),
    private readonly timers: Timers = realTimers,
    private readonly windowMs: number = COALESCE_WINDOW_MS,
    private readonly maxMs: number = COALESCE_MAX_MS,
  ) {}

  add(notice: Notice): void {
    if (notice.text.trim() === '') return;
    const now = this.now();
    if (this.held.length === 0) this.firstAt = now;
    this.held.push(notice);
    if (this.timer !== null) this.timers.clear(this.timer);
    const wait = Math.max(0, Math.min(this.windowMs, this.firstAt + this.maxMs - now));
    this.timer = this.timers.set(() => this.flush(), wait);
  }

  /** Shows everything held now. */
  flush(): void {
    if (this.timer !== null) this.timers.clear(this.timer);
    this.timer = null;
    const held = this.held;
    this.held = [];
    for (const card of coalesce(held)) this.show(card.text, card.level, card);
  }

  /** Drops what is held without showing it. */
  dispose(): void {
    if (this.timer !== null) this.timers.clear(this.timer);
    this.timer = null;
    this.held = [];
  }
}

/** Collapses `held` into the cards to show, in first-arrival order per group. */
export function coalesce(held: readonly Notice[]): Notice[] {
  const groups = new Map<NoticeGroup, Notice[]>();
  for (const n of held) {
    const list = groups.get(n.group) ?? [];
    // A drive's note for each game of one system is one card.
    if (n.group !== 'drive-note' || !list.some((m) => m.text === n.text)) list.push(n);
    groups.set(n.group, list);
  }
  return [...groups.entries()].map(([group, list]) => {
    if (list.length === 1) return list[0];
    const card: Notice = { group, text: summaryText(group, list.length), level: list[0].level };
    // The summary keeps the action its cards share ("2 games added to
    // download queue" still opens Downloads).
    const action = list[0].action;
    if (action && list.every((n) => n.action === action)) card.action = action;
    // And the second line they share ("Copying from SAMSUNG T7", item 450).
    const sub = list[0].sub;
    if (sub && list.every((n) => n.sub === sub)) card.sub = sub;
    return card;
  });
}

/**
 * Game installs whose own "<title> added to download queue" card a batch
 * stands in for (item 450: installing several games from a drive shows one
 * card, however slowly each call returns). Each id is taken once; ids not
 * taken within `ttlMs` lapse.
 */
export class QuietQueued {
  private ids = new Map<number, number>();

  constructor(private readonly ttlMs: number = 60_000) {}

  add(romIds: readonly number[], now: number): void {
    for (const id of romIds) this.ids.set(id, now);
  }

  drop(romId: number): void {
    this.ids.delete(romId);
  }

  /** Whether `notice`, about `romId`'s queue entry, is one the batch stands in for. */
  takes(notice: Notice, romId: number, now: number): boolean {
    if (notice.group !== 'game-queued') return false;
    const at = this.ids.get(romId);
    if (at === undefined) return false;
    this.ids.delete(romId);
    return now - at <= this.ttlMs;
  }
}

/**
 * A second line for a game's next "<title> added to download queue" card
 * ("PCSX2 installs with it": TV Details installs the system's recommended
 * emulator with the game, ruling A). Each id is used once; ids not used
 * within `ttlMs` lapse.
 */
export class QueuedSubs {
  private subs = new Map<number, { sub: string; at: number }>();

  constructor(private readonly ttlMs: number = 60_000) {}

  add(romId: number, sub: string, now: number): void {
    this.subs.set(romId, { sub, at: now });
  }

  /** `notice` with its second line, when it is `romId`'s queued card and one waits. */
  apply(notice: Notice, romId: number, now: number): Notice {
    if (notice.group !== 'game-queued') return notice;
    const held = this.subs.get(romId);
    if (!held) return notice;
    this.subs.delete(romId);
    return now - held.at <= this.ttlMs ? { ...notice, sub: held.sub } : notice;
  }
}
