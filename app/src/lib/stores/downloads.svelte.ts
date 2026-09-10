import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { api, type DownloadEntry, type DownloadsSnapshot } from '../api';
import type { Sample } from '../downloads/ring';
import { createSampler, graphsOf, observe, SAMPLE_INTERVAL_MS, tick } from '../downloads/sampler';

const LIVE_STATUSES = new Set(['queued', 'downloading', 'installing', 'cancelling']);

const state = $state<{ entries: DownloadEntry[] }>({ entries: [] });

// Design §8: the store keeps a ring buffer per entry fed from the byte
// deltas of every `downloads-changed` snapshot, sampled once per second.
// The sampler is plain (non-reactive) state; `graphs` is its once-per-second
// reactive mirror, so rows re-render on the tick and not on every 100ms
// progress event. It is `$state.raw`: the sample arrays are replaced, never
// mutated, and a raw mirror keeps the identity `graphsOf` memoises, so an
// unchanged track's sparkline does not recompute.
let graphs = $state.raw<Record<number, Sample[]>>({});

// `performance.now()`, not the wall clock: a backwards system-clock step
// would make `tick` see a non-positive elapsed time and stall sampling.
let sampler = createSampler(performance.now());

const NO_SAMPLES: Sample[] = [];

export const downloads = {
  get entries() {
    return state.entries;
  },
  get hasLive() {
    return state.entries.some((e) => LIVE_STATUSES.has(e.status));
  },
  /** The entry's transfer-rate samples, oldest first; empty for an unknown id. */
  samplesFor(id: number): Sample[] {
    return graphs[id] ?? NO_SAMPLES;
  },
};

function apply(snapshot: DownloadsSnapshot): void {
  state.entries = snapshot.entries;
  observe(sampler, snapshot.entries);
}

export async function init(): Promise<UnlistenFn> {
  sampler = createSampler(performance.now());
  graphs = {};
  apply(await api.listDownloads());
  const timer = setInterval(() => {
    tick(sampler, performance.now());
    // `graphsOf` returns the same record while nothing changed; skipping the
    // assignment then keeps every row, live or frozen, out of the re-render.
    const next = graphsOf(sampler);
    if (next !== graphs) graphs = next;
  }, SAMPLE_INTERVAL_MS);
  const unlisten = await listen<DownloadsSnapshot>('downloads-changed', (e) => {
    apply(e.payload);
  });
  return () => {
    clearInterval(timer);
    unlisten();
  };
}
