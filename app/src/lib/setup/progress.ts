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

/**
 * Where setup's installs stand, from the Downloads rows of the emulators it
 * queued (`job: 'emulator'`, matched by `source_id`). An emulator with no row
 * yet counts as running: it was admitted and its row is on its way. When an
 * emulator has several rows (a retry), the newest one (highest `id`) decides.
 */
export function summarize(queued: readonly string[], entries: readonly DownloadEntry[]): SetupProgress {
  const latest = new Map<string, DownloadEntry>();
  for (const entry of entries) {
    if (entry.job !== 'emulator' || !queued.includes(entry.source_id)) continue;
    const seen = latest.get(entry.source_id);
    if (seen === undefined || entry.id > seen.id) latest.set(entry.source_id, entry);
  }
  let done = 0;
  let running = 0;
  const failed: DownloadEntry[] = [];
  for (const id of queued) {
    const entry = latest.get(id);
    if (entry === undefined || RUNNING.has(entry.status)) running += 1;
    else if (entry.status === 'completed') done += 1;
    else failed.push(entry);
  }
  return { total: queued.length, done, running, failed };
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
