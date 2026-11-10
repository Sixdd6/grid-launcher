// The in-shell setup card (`SetupProgress.svelte`), kept pure. It counts
// installs, Proton included: "Setting up 3 installs · 1 done", then
// "3 installs done" with how many systems are ready to play. Its bar has one
// segment per install.
import type { DownloadEntry, SetupOutcome } from '../api';

export type SetupProgress = {
  /** Installs setup queued (emulators and a Proton version). */
  total: number;
  /** Installed (their Downloads row completed). */
  done: number;
  /** Still waiting, downloading or installing. */
  running: number;
  /** Failed or cancelled rows, newest state per install: Retry lives on them. */
  failed: DownloadEntry[];
};

const WAITING = new Set<DownloadEntry['status']>(['queued']);
const RUNNING = new Set<DownloadEntry['status']>(['downloading', 'installing', 'cancelling']);

/**
 * The newest Downloads row of each queued install, by `source_id`: catalog
 * emulators (`owner/repo`), server builds (`romm:<folder>/<emulator>`) and
 * compat tools alike (all `emulator`-job rows). After a retry the newest row
 * (highest `id`) decides.
 */
export function latestRows(queued: readonly string[], entries: readonly DownloadEntry[]): Map<string, DownloadEntry> {
  const latest = new Map<string, DownloadEntry>();
  for (const entry of entries) {
    if (entry.job !== 'emulator') continue;
    const key = entry.source_id;
    if (!queued.includes(key)) continue;
    const seen = latest.get(key);
    if (seen === undefined || entry.id > seen.id) latest.set(key, entry);
  }
  return latest;
}

export type SegmentState = 'waiting' | 'running' | 'done' | 'failed';

/** One install's state. With no row yet it is waiting: it was admitted and
 *  its row is on its way. */
function stateOf(entry: DownloadEntry | undefined): SegmentState {
  if (entry === undefined || WAITING.has(entry.status)) return 'waiting';
  if (RUNNING.has(entry.status)) return 'running';
  if (entry.status === 'completed') return 'done';
  return 'failed';
}

/** Where setup's installs stand. */
export function summarize(queued: readonly string[], entries: readonly DownloadEntry[]): SetupProgress {
  const latest = latestRows(queued, entries);
  let done = 0;
  let running = 0;
  const failed: DownloadEntry[] = [];
  for (const id of queued) {
    const entry = latest.get(id);
    const state = stateOf(entry);
    if (state === 'done') done += 1;
    else if (state === 'failed') failed.push(entry!);
    else running += 1;
  }
  return { total: queued.length, done, running, failed };
}

export type Segment = {
  sourceId: string;
  name: string;
  state: SegmentState;
  /** What pointing at it says: "PCSX2 · Installing". */
  title: string;
};

export const SEGMENT_LABELS: Record<SegmentState, string> = {
  waiting: 'Waiting',
  running: 'Installing',
  done: 'Installed',
  failed: 'Didn’t install',
};

/** One bar segment per queued install, in queue order. Named from `names`
 *  (what the chooser showed), else its Downloads row, else its source id. */
export function segments(
  queued: readonly string[],
  entries: readonly DownloadEntry[],
  names: Readonly<Record<string, string>>,
): Segment[] {
  const latest = latestRows(queued, entries);
  return queued.map((sourceId) => {
    const entry = latest.get(sourceId);
    const state = stateOf(entry);
    const name = names[sourceId] || entry?.title || sourceId;
    return { sourceId, name, state, title: `${name} · ${SEGMENT_LABELS[state]}` };
  });
}

const installs = (n: number) => `${n} install${n === 1 ? '' : 's'}`;

/** The card's headline: "Setting up 3 installs · 1 done", then "3 installs
 *  done" (or "2 of 3 installs done" after a failure). */
export function headline(p: SetupProgress): string {
  if (p.total === 0) return 'Setup done';
  if (p.running > 0) return `Setting up ${installs(p.total)} · ${p.done} done`;
  if (p.failed.length > 0) return `${p.done} of ${installs(p.total)} done`;
  return `${installs(p.total)} done`;
}

/**
 * Whether the setup card's auto-dismiss countdown runs: only once nothing is
 * still installing, whatever the outcome. Until then it stays put however
 * long the installs take.
 */
export function cardCountsDown(p: SetupProgress): boolean {
  return p.running === 0;
}

/** One line per platform the setup could not finish, for the summary. */
export function outcomeNotes(outcomes: readonly SetupOutcome[]): string[] {
  return outcomes.flatMap((o) =>
    o.outcome === 'core_missing'
      ? [`${o.platform}: RetroArch has no core for it, so it was left without an emulator.`]
      : [],
  );
}

/** Platforms that got their chosen emulator. */
export function assignedCount(outcomes: readonly SetupOutcome[]): number {
  return outcomes.filter((o) => o.outcome === 'assigned').length;
}

export type CompatRun = {
  /** The Proton install setup queued, if any. */
  queued: string | null;
  /** The Proton pick was the default at once. */
  set: boolean;
};

/** Systems ready to play: those that got their emulator, plus Windows once
 *  its Proton version is the default (at once, or when its install is done). */
export function readyCount(
  outcomes: readonly SetupOutcome[],
  compat: CompatRun,
  segs: readonly Segment[],
): number {
  const windows =
    compat.set || (compat.queued !== null && segs.some((s) => s.sourceId === compat.queued && s.state === 'done'));
  return assignedCount(outcomes) + (windows ? 1 : 0);
}

/** "3 systems are ready to play." */
export function readyText(count: number): string {
  return `${count} ${count === 1 ? 'system is' : 'systems are'} ready to play.`;
}
