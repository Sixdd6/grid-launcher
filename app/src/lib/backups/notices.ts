// The notification cards for backups to a drive (GameBackups design:
// "Notifications are one card per backup, not one per game"). Pure:
// `notifications/notifier.ts` feeds it every `downloads-changed` snapshot and
// shows what it returns; `notices.test.ts` beside it tests it.
//
// A backup is one batch (`DownloadEntry.batch`): every row `start_backup`
// (or `set_drive_os`) queued together. Its cards are keyed by the batch, not
// by the notifier's 0.4 s coalescing window:
//   1. queued, once: "3 games added to the backup queue for Couch kit";
//   2. when every row of the batch is terminal, once: either
//      "3 games and 1 emulator backed up to Couch kit", or, when a row failed
//      or the drive was removed, one error card
//      ("SAMSUNG T7 was removed. 4 of 17 items were written to Couch kit.").
// Per-row cards for backup rows never show (`notices.ts` skips them).
//
// The queue admits a batch one row per snapshot, so the queued card waits
// until the batch has stopped growing for `BATCH_SETTLE_MS` (or ends).

import type { DownloadEntry, DownloadKind, DownloadStatus, OsChangeOutcome, PackOs } from '../api';
import type { Notice } from '../notifications/notices';

/** How long a batch must stop growing before its queued card shows. */
export const BATCH_SETTLE_MS = 500;

const TERMINAL: ReadonlySet<DownloadStatus> = new Set(['completed', 'failed', 'cancelled']);

/** The platform column of a compat tool row (Proton, umu-launcher, the Steam Linux Runtime). */
const COMPAT_TOOL_PLATFORM = 'Compatibility Tool';

/** Wording for a batch `set_drive_os` queued: "Windows builds of 2 emulators …". */
export type OsChangeHint = { os: PackOs[]; queued: number };

type Row = {
  kind: DownloadKind;
  title: string;
  platform: string;
  status: DownloadStatus;
  error: string;
  /** Left the snapshot (folded into a summary row, dismissed, or pruned). */
  gone: boolean;
  /** It left while still live: the drive was removed while it waited. */
  goneLive: boolean;
};

type Batch = {
  id: number;
  name: string;
  label: string;
  rows: Map<number, Row>;
  /** `backup_cancelled` rows: their `error` is the removal reason. */
  summaries: Row[];
  announced: boolean;
  grewAt: number;
};

function plural(n: number, one: string, many = `${one}s`): string {
  return `${n} ${n === 1 ? one : many}`;
}

/** "A", "A and B", "A, B and C". */
function joinParts(parts: string[]): string {
  if (parts.length <= 1) return parts.join('');
  return `${parts.slice(0, -1).join(', ')} and ${parts[parts.length - 1]}`;
}

const OS_NAMES: Record<PackOs, string> = { linux: 'Linux', windows: 'Windows' };

/** "Windows", "Linux and Windows". */
export function osNames(os: readonly PackOs[]): string {
  return joinParts(os.map((o) => OS_NAMES[o]));
}

/** An emulator row's title is "DuckStation for Linux": one row per OS, one emulator. */
function emulatorName(title: string): string {
  return title.replace(/ for (Linux|Windows)$/, '');
}

type Counts = { games: string[]; emulators: Set<string>; tools: Set<string>; firmware: string[] };

function countRows(rows: Iterable<Row>): Counts {
  const c: Counts = { games: [], emulators: new Set(), tools: new Set(), firmware: [] };
  for (const r of rows) {
    if (r.kind === 'backup_game') c.games.push(r.title);
    else if (r.kind === 'backup_firmware') c.firmware.push(r.title);
    else if (r.kind === 'backup_emulator') {
      (r.platform === COMPAT_TOOL_PLATFORM ? c.tools : c.emulators).add(emulatorName(r.title));
    }
  }
  return c;
}

/**
 * "Velvet Static" or "3 games", "1 emulator", "2 compatibility tools",
 * "PlayStation firmware" or "firmware for 2 systems". A lone emulator or
 * tool is named ("DuckStation backed up to Couch kit").
 */
function phraseParts(c: Counts): string[] {
  const only = c.games.length + c.emulators.size + c.tools.size + c.firmware.length === 1;
  if (only && c.emulators.size === 1) return [...c.emulators];
  if (only && c.tools.size === 1) return [...c.tools];
  const parts: string[] = [];
  if (c.games.length > 0) parts.push(c.games.length === 1 ? c.games[0] : plural(c.games.length, 'game'));
  if (c.emulators.size > 0) parts.push(plural(c.emulators.size, 'emulator'));
  if (c.tools.size > 0) parts.push(plural(c.tools.size, 'compatibility tool'));
  if (c.firmware.length > 0) {
    parts.push(c.firmware.length === 1 ? c.firmware[0] : `firmware for ${plural(c.firmware.length, 'system')}`);
  }
  return parts;
}

function isTerminal(r: Row): boolean {
  return r.gone || TERMINAL.has(r.status);
}

/** A backup row the batch book does not own: batch 0, or one appearing after its batch ended. */
function loneOutcome(e: DownloadEntry): Notice | null {
  const name = e.drive_name.trim() || e.drive_label.trim();
  if (e.kind === 'backup_cancelled') {
    const reason = e.error.trim();
    return { group: 'drive-failed', text: reason === '' ? `Cancelled ${e.title}` : `Cancelled ${e.title}: ${reason}`, level: 'error' };
  }
  switch (e.status) {
    case 'completed':
      return { group: 'backup-done', text: `${e.title} backed up to ${name}`, level: 'success' };
    case 'failed': {
      const reason = e.error.trim();
      const head = `Couldn't backup ${e.title}`;
      return { group: 'backup-failed', text: reason === '' ? head : `${head}: ${reason}`, level: 'error' };
    }
    case 'cancelled':
      return { group: 'cancelled', text: `Cancelled ${e.title}`, level: 'success' };
    default:
      return null;
  }
}

/**
 * Tracks every backup batch across snapshots and says which cards each
 * snapshot earns. One per notifier; `now` is injected for tests.
 */
export class BackupBatches {
  private batches = new Map<number, Batch>();
  private ended = new Set<number>();
  /** Every backup row id seen, so a row of an ended batch is not news again. */
  private known = new Set<number>();
  /** Last status of rows outside any open batch. */
  private lone = new Map<number, DownloadStatus>();
  private hints = new Map<number, OsChangeHint>();

  /**
   * Folds one snapshot in. `first` marks the baseline snapshot: what was
   * already queued when the app came up is not news, so it earns no card.
   */
  observe(entries: readonly DownloadEntry[], first: boolean, now: number): Notice[] {
    const out: Notice[] = [];
    const present = new Set<number>();
    // Oldest first, as the snapshot lists its entries newest first.
    for (const e of [...entries].reverse()) {
      if (e.job !== 'backup') continue;
      present.add(e.id);
      const isNew = !this.known.has(e.id);
      this.known.add(e.id);
      const open = e.batch !== 0 && !this.ended.has(e.batch);
      // A summary row naming a batch this book never saw (it predates the
      // baseline) has nothing to fold into: it is a card of its own.
      if (!open || (e.kind === 'backup_cancelled' && !this.batches.has(e.batch))) {
        if (!isNew && !this.lone.has(e.id)) continue; // a row of a batch that already ended
        const before = this.lone.get(e.id) ?? null;
        this.lone.set(e.id, e.status);
        if (!first && before !== e.status && TERMINAL.has(e.status)) {
          const n = loneOutcome(e);
          if (n) out.push(n);
        }
        continue;
      }
      let batch = this.batches.get(e.batch);
      if (!batch) {
        batch = {
          id: e.batch,
          name: e.drive_name.trim() || e.drive_label.trim(),
          label: e.drive_label.trim() || e.drive_name.trim(),
          rows: new Map(),
          summaries: [],
          announced: first,
          grewAt: now,
        };
        this.batches.set(e.batch, batch);
      }
      const row: Row = {
        kind: e.kind,
        title: e.title,
        platform: e.platform,
        status: e.status,
        error: e.error,
        gone: false,
        goneLive: false,
      };
      if (e.kind === 'backup_cancelled') {
        if (isNew) batch.summaries.push(row);
        continue;
      }
      if (!batch.rows.has(e.id)) batch.grewAt = now;
      batch.rows.set(e.id, row);
    }
    for (const id of Array.from(this.lone.keys())) if (!present.has(id)) this.lone.delete(id);

    for (const batch of Array.from(this.batches.values())) {
      for (const [id, row] of batch.rows) {
        if (!present.has(id) && !row.gone) {
          row.gone = true;
          row.goneLive = !TERMINAL.has(row.status);
        }
      }
      if (batch.rows.size === 0 || ![...batch.rows.values()].every(isTerminal)) continue;
      this.batches.delete(batch.id);
      this.ended.add(batch.id);
      const hint = this.hints.get(batch.id);
      this.hints.delete(batch.id);
      if (first) continue;
      if (!batch.announced) out.push(this.queuedCard(batch, hint));
      const end = this.endCard(batch, hint);
      if (end) out.push(end);
    }
    return out;
  }

  /** The queued cards of batches that have stopped growing for `BATCH_SETTLE_MS`. */
  due(now: number): Notice[] {
    const out: Notice[] = [];
    for (const batch of this.batches.values()) {
      if (batch.announced || now - batch.grewAt < BATCH_SETTLE_MS) continue;
      batch.announced = true;
      out.push(this.queuedCard(batch, this.hints.get(batch.id)));
    }
    return out;
  }

  /** Milliseconds until the next queued card is due, or `null` when none waits. */
  nextDueIn(now: number): number | null {
    let next: number | null = null;
    for (const batch of this.batches.values()) {
      if (batch.announced) continue;
      const wait = Math.max(0, batch.grewAt + BATCH_SETTLE_MS - now);
      next = next === null ? wait : Math.min(next, wait);
    }
    return next;
  }

  /** Words batch `batch` as an OS change ("Windows builds of 2 emulators …"). */
  hint(batch: number, hint: OsChangeHint): void {
    if (batch === 0 || this.ended.has(batch)) return;
    this.hints.set(batch, hint);
  }

  private queuedCard(batch: Batch, hint: OsChangeHint | undefined): Notice {
    batch.announced = true;
    const c = countRows(batch.rows.values());
    let what: string;
    if (hint) {
      const n = c.emulators.size + c.tools.size || hint.queued;
      what = `${osNames(hint.os)} builds of ${plural(n, 'emulator')}`;
    } else if (c.games.length > 0) {
      what = c.games.length === 1 ? c.games[0] : plural(c.games.length, 'game');
    } else {
      what = joinParts(phraseParts(c)) || plural(batch.rows.size, 'item');
    }
    return { group: 'backup-queued', text: `${what} added to the backup queue for ${batch.name}`, level: 'success' };
  }

  private endCard(batch: Batch, hint: OsChangeHint | undefined): Notice | null {
    const rows = [...batch.rows.values()];
    const written = rows.filter((r) => r.status === 'completed');
    const failed = rows.filter((r) => r.status === 'failed');
    const goneLive = rows.filter((r) => r.goneLive);
    if (failed.length === 0 && goneLive.length === 0 && batch.summaries.length === 0) {
      if (written.length === 0) return null; // every row cancelled by hand
      const c = countRows(written);
      const what = hint
        ? `${osNames(hint.os)} builds of ${plural(c.emulators.size + c.tools.size, 'emulator')}`
        : joinParts(phraseParts(c));
      return { group: 'backup-done', text: `${what} backed up to ${batch.name}`, level: 'success' };
    }
    const removal = `${batch.label} was removed`;
    const removed =
      batch.summaries.map((s) => s.error.trim()).find((r) => r !== '') ??
      failed.map((r) => r.error.trim()).find((r) => r.endsWith(' was removed')) ??
      (goneLive.length > 0 ? removal : undefined);
    let lead: string;
    if (removed !== undefined) lead = removed;
    else if (failed.length === 1) {
      const reason = failed[0].error.trim();
      lead = reason === '' ? `Couldn't backup ${failed[0].title}` : `Couldn't backup ${failed[0].title}: ${reason}`;
    } else lead = `${plural(failed.length, 'item')} couldn't be backed up`;
    lead = lead.replace(/\.$/, '');
    const total = rows.length;
    const items = total === 1 ? 'item was' : 'items were';
    return {
      group: 'backup-failed',
      text: `${lead}. ${written.length} of ${total} ${items} written to ${batch.name}.`,
      level: 'error',
    };
  }
}

/** The hint for an `OsChangeOutcome` with `added` OSes, or `null` when nothing was queued. */
export function osChangeHint(outcome: OsChangeOutcome, added: readonly PackOs[]): OsChangeHint | null {
  if (outcome.batch === 0 || outcome.queued === 0 || added.length === 0) return null;
  return { os: [...added], queued: outcome.queued };
}
