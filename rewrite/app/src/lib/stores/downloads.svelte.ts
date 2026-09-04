import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { api, type DownloadEntry, type DownloadsSnapshot } from '../api';
import type { Sample } from '../downloads/ring';
import { createSampler, graphsOf, observe, SAMPLE_INTERVAL_MS, tick } from '../downloads/sampler';

const LIVE_STATUSES = new Set(['queued', 'downloading', 'installing', 'cancelling']);

const state = $state<{ entries: DownloadEntry[]; graphs: Record<number, Sample[]> }>({
  entries: [],
  graphs: {},
});

// Design §8: the store keeps a ring buffer per entry fed from the byte
// deltas of every `downloads-changed` snapshot, sampled once per second.
// The sampler is plain (non-reactive) state; `state.graphs` is its
// once-per-second reactive mirror, so rows re-render on the tick and not on
// every 100ms progress event.
let sampler = createSampler(Date.now());

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
    return state.graphs[id] ?? NO_SAMPLES;
  },
};

function apply(snapshot: DownloadsSnapshot): void {
  state.entries = snapshot.entries;
  observe(sampler, snapshot.entries);
}

export async function init(): Promise<UnlistenFn> {
  sampler = createSampler(Date.now());
  state.graphs = {};
  apply(await api.listDownloads());
  const timer = setInterval(() => {
    tick(sampler, Date.now());
    state.graphs = graphsOf(sampler);
  }, SAMPLE_INTERVAL_MS);
  const unlisten = await listen<DownloadsSnapshot>('downloads-changed', (e) => {
    apply(e.payload);
  });
  return () => {
    clearInterval(timer);
    unlisten();
  };
}
