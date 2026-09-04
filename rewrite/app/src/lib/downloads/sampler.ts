// Design §8: "the downloads store keeps a ring buffer per entry fed from
// `downloaded_bytes` and `install_processed_bytes` deltas on each progress
// event, sampled once per second. No new IPC."
//
// `observe` runs on every `downloads-changed` snapshot and folds each
// entry's byte-counter movement into a pending delta. `tick` runs once per
// second, turns the pending deltas into one bytes-per-second sample per live
// entry, and resets them. Pure: the store owns the timer and the clock.
import type { DownloadEntry, DownloadStatus } from '../api';
import { createRing, pushSample, samplesOf, type Ring, type Sample } from './ring';

/** How often the store calls `tick`. */
export const SAMPLE_INTERVAL_MS = 1000;

const LIVE_STATUSES = new Set<DownloadStatus>(['queued', 'downloading', 'installing', 'cancelling']);

type Track = {
  ring: Ring;
  lastDownloaded: number;
  lastInstalled: number;
  pendingNet: number;
  pendingDisk: number;
  /** False once the entry is terminal: the ring freezes. */
  live: boolean;
};

export type Sampler = {
  tracks: Map<number, Track>;
  lastTickAt: number;
};

export function createSampler(nowMs: number): Sampler {
  return { tracks: new Map(), lastTickAt: nowMs };
}

/**
 * Folds one snapshot into the pending deltas. A new entry starts a track at
 * its current counters (an app that comes up mid-transfer must not book the
 * whole downloaded-so-far figure as one second's rate); a counter that moves
 * backwards contributes nothing; an entry missing from the snapshot loses
 * its track.
 */
export function observe(sampler: Sampler, entries: DownloadEntry[]): void {
  const seen = new Set<number>();
  for (const e of entries) {
    seen.add(e.id);
    const track = sampler.tracks.get(e.id);
    if (track === undefined) {
      sampler.tracks.set(e.id, {
        ring: createRing(),
        lastDownloaded: e.downloaded_bytes,
        lastInstalled: e.install_processed_bytes,
        pendingNet: 0,
        pendingDisk: 0,
        live: LIVE_STATUSES.has(e.status),
      });
      continue;
    }
    track.pendingNet += Math.max(0, e.downloaded_bytes - track.lastDownloaded);
    track.pendingDisk += Math.max(0, e.install_processed_bytes - track.lastInstalled);
    track.lastDownloaded = e.downloaded_bytes;
    track.lastInstalled = e.install_processed_bytes;
    track.live = LIVE_STATUSES.has(e.status);
  }
  for (const id of Array.from(sampler.tracks.keys())) {
    if (!seen.has(id)) sampler.tracks.delete(id);
  }
}

/**
 * Emits one sample per live track, normalised to bytes per second over the
 * time since the previous tick, and clears the pending deltas. A tick with
 * no elapsed time is ignored rather than dividing by zero.
 */
export function tick(sampler: Sampler, nowMs: number): void {
  const elapsed = nowMs - sampler.lastTickAt;
  if (elapsed <= 0) return;
  sampler.lastTickAt = nowMs;
  const perSecond = 1000 / elapsed;
  for (const track of sampler.tracks.values()) {
    if (!track.live) continue;
    pushSample(track.ring, {
      net: track.pendingNet * perSecond,
      disk: track.pendingDisk * perSecond,
    });
    track.pendingNet = 0;
    track.pendingDisk = 0;
  }
}

/** Every track's samples, oldest first, keyed by entry id. */
export function graphsOf(sampler: Sampler): Record<number, Sample[]> {
  const out: Record<number, Sample[]> = {};
  for (const [id, track] of sampler.tracks) {
    out[id] = samplesOf(track.ring);
  }
  return out;
}
