// The in-shell setup progress summary (`SetupProgress.svelte`), kept pure.
import type { DownloadEntry, SetupOutcome } from '../api';

export type SetupProgress = {
  /** Emulators setup queued. */
  total: number;
  /** Installed (their Downloads row completed). */
  done: number;
  /** Still queued, downloading or installing. */
  running: number;
  /** Failed or cancelled rows, newest state per emulator: Retry lives on them. */
  failed: DownloadEntry[];
};

const RUNNING = new Set<DownloadEntry['status']>(['queued', 'downloading', 'installing', 'cancelling']);

/** The key one queued install's Downloads row is found by: a catalog
 *  emulator by its `source_id`, a server package (a game job) by its ROM. */
function rowKey(entry: DownloadEntry): string | null {
  if (entry.job === 'emulator') return entry.source_id;
  if (entry.job === 'game') return `romm:${entry.rom_id}`;
  return null;
}

/**
 * Where setup's installs stand, from the Downloads rows of the emulators it
 * queued: catalog emulators (`job: 'emulator'`, by `source_id`) and server
 * packages (`job: 'game'`, by ROM id). An emulator with no row yet counts as
 * running: it was admitted and its row is on its way. When an emulator has
 * several rows (a retry), the newest one (highest `id`) decides.
 */
export function summarize(
  queued: readonly string[],
  entries: readonly DownloadEntry[],
  queuedServer: readonly number[] = [],
): SetupProgress {
  const wanted = [...queued, ...queuedServer.map((id) => `romm:${id}`)];
  const latest = new Map<string, DownloadEntry>();
  for (const entry of entries) {
    const key = rowKey(entry);
    if (key === null || !wanted.includes(key)) continue;
    const seen = latest.get(key);
    if (seen === undefined || entry.id > seen.id) latest.set(key, entry);
  }
  let done = 0;
  let running = 0;
  const failed: DownloadEntry[] = [];
  for (const id of wanted) {
    const entry = latest.get(id);
    if (entry === undefined || RUNNING.has(entry.status)) running += 1;
    else if (entry.status === 'completed') done += 1;
    else failed.push(entry);
  }
  return { total: wanted.length, done, running, failed };
}

/** The card's headline: "Setting up 3 emulators · 1 done", then the end state. */
export function headline(p: SetupProgress): string {
  const noun = `emulator${p.total === 1 ? '' : 's'}`;
  if (p.running > 0) return `Setting up ${p.total} ${noun} · ${p.done} done`;
  if (p.failed.length > 0) return `${p.done} of ${p.total} ${noun} installed`;
  return `${p.total} ${noun} installed`;
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
