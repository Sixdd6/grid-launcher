// The first-run setup card in TV mode (checklist items 151–156; README
// § Startup screens › Choose your systems; tvmode.js `setupCard`). Pure: what
// the card shows from setup's queued installs and their Downloads rows.
// TvSetupCard keeps it on show as a held, pinned notice (notices.svelte.ts).
//
// One segment per install, each emulator once (setup queues each source
// once). A segment's first half fills while its emulator downloads and its
// second half while it installs; it is full when done and red if it failed.
// A line under the bar names the step. The card holds still, with no
// countdown, until nothing is still installing; then the line says how many
// systems are ready and the card closes after 10 seconds.
import type { DownloadEntry, SetupOutcome } from '../../api';
import { headline, latestRows, readyCount, readyText, segments, summarize, type CompatRun } from '../../setup/progress';
import type { NoticeSeg } from '../notices';

/** How long the card stays once every install is done (item 156). */
export const SETUP_CARD_DONE_MS = 10_000;

const pctOf = (done: number, total: number) => (total > 0 ? Math.max(0, Math.min(1, done / total)) : 0);

/** A segment's fill, 0–100: the download fills the first half, the install
 *  the second (item 153). Waiting is empty; done and failed are full. */
export function segFill(entry: DownloadEntry | undefined): NoticeSeg {
  if (entry === undefined || entry.status === 'queued') return { pct: 0, state: 'wait' };
  if (entry.status === 'completed') return { pct: 100, state: 'done' };
  if (entry.status === 'failed' || entry.status === 'cancelled') return { pct: 100, state: 'fail' };
  if (entry.status === 'installing') {
    return { pct: Math.round(50 + pctOf(entry.install_processed_bytes, entry.install_total_bytes) * 50), state: 'run' };
  }
  // downloading, cancelling
  return { pct: Math.round(pctOf(entry.downloaded_bytes, entry.total_bytes) * 50), state: 'run' };
}

/** The step line for the install in progress (item 155): "DuckStation ·
 *  downloading 62%", then "DuckStation · installing…". */
export function stepLine(name: string, entry: DownloadEntry): string {
  if (entry.status === 'installing') return `${name} · installing…`;
  return `${name} · downloading ${Math.round(pctOf(entry.downloaded_bytes, entry.total_bytes) * 100)}%`;
}

export type SetupCardView = {
  /** "Setting up 3 installs · 1 done", then "3 installs done" (item 154). */
  title: string;
  segs: NoticeSeg[];
  /** The step in progress, then "3 systems are ready to play." (items 155–156). */
  step: string | null;
  /** Installs that didn't finish, in the desktop card's words. */
  sub: string | null;
  /** Nothing is still installing: the countdown may start (item 156). */
  done: boolean;
};

export type SetupRunView = {
  queued: readonly string[];
  errors: readonly string[];
  outcomes: readonly SetupOutcome[];
  names: Readonly<Record<string, string>>;
  compat: CompatRun;
};

export function setupCardView(run: SetupRunView, entries: readonly DownloadEntry[]): SetupCardView {
  // Each emulator once (item 152).
  const queued = [...new Set(run.queued)];
  const progress = summarize(queued, entries);
  const latest = latestRows(queued, entries);
  const segs = segments(queued, entries, run.names);
  const done = progress.running === 0;
  let step: string | null = null;
  if (!done) {
    const live = segs.find((s) => s.state === 'running');
    const entry = live ? latest.get(live.sourceId) : undefined;
    if (live && entry) step = stepLine(live.name, entry);
  } else {
    const ready = readyCount(run.outcomes, run.compat, segs);
    if (ready > 0) step = readyText(ready);
  }
  const failed = [
    ...segs.filter((s) => s.state === 'failed').map((s) => `${s.name} didn't install.`),
    ...run.errors,
  ];
  return {
    title: headline(progress),
    segs: queued.map((id) => segFill(latest.get(id))),
    step,
    sub: failed.length ? failed.join(' ') : null,
    done,
  };
}
