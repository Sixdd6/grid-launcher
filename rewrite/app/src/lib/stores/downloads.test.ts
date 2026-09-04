import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { DownloadEntry, DownloadsSnapshot } from '../api';

// `downloads.svelte.ts` is module-scoped state, so each test takes a fresh
// module instance: `vi.resetModules()` plus a dynamic `import()` after the
// fakes are wired with `vi.doMock`. Fake timers also fake `Date.now`, which
// is the clock the store hands the sampler.

function entry(overrides: Partial<DownloadEntry>): DownloadEntry {
  return {
    id: 1,
    job: 'game',
    kind: 'base',
    rom_id: 1,
    source_id: '',
    title: 'Game',
    platform: 'Platform',
    status: 'downloading',
    downloaded_bytes: 0,
    total_bytes: 0,
    speed_bps: 0,
    install_processed_bytes: 0,
    install_total_bytes: 0,
    error: '',
    ...overrides,
  };
}

type SnapshotHandler = (event: { payload: DownloadsSnapshot }) => void;

function wire(initial: DownloadEntry[]) {
  const captured: { handler?: SnapshotHandler } = {};
  const unlisten = vi.fn();
  vi.doMock('../api', () => ({
    api: { listDownloads: async () => ({ entries: initial }) },
  }));
  vi.doMock('@tauri-apps/api/event', () => ({
    listen: async (_name: string, handler: SnapshotHandler) => {
      captured.handler = handler;
      return unlisten;
    },
  }));
  return {
    unlisten,
    emit(entries: DownloadEntry[]) {
      captured.handler!({ payload: { entries } });
    },
  };
}

describe('downloads store sampling', () => {
  beforeEach(() => {
    vi.resetModules();
    vi.useFakeTimers();
    vi.setSystemTime(new Date('2026-09-04T12:00:00Z'));
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.doUnmock('../api');
    vi.doUnmock('@tauri-apps/api/event');
  });

  it('exposes one sample per second built from the byte deltas of the snapshots', async () => {
    const mock = wire([entry({ id: 1, downloaded_bytes: 1_000, total_bytes: 10_000 })]);
    const { downloads, init } = await import('./downloads.svelte');
    const stop = await init();

    expect(downloads.entries).toHaveLength(1);
    expect(downloads.samplesFor(1)).toEqual([]);

    mock.emit([entry({ id: 1, downloaded_bytes: 3_000, total_bytes: 10_000 })]);
    mock.emit([entry({ id: 1, status: 'installing', downloaded_bytes: 3_000, install_processed_bytes: 500 })]);
    await vi.advanceTimersByTimeAsync(1_000);

    expect(downloads.samplesFor(1)).toEqual([{ net: 2_000, disk: 500 }]);
    expect(downloads.entries[0].status).toBe('installing');

    stop();
    expect(mock.unlisten).toHaveBeenCalledTimes(1);
  });

  it('stops sampling once stopped and returns an empty list for an unknown id', async () => {
    const mock = wire([entry({ id: 1 })]);
    const { downloads, init } = await import('./downloads.svelte');
    const stop = await init();
    stop();

    mock.emit([entry({ id: 1, downloaded_bytes: 999 })]);
    await vi.advanceTimersByTimeAsync(3_000);

    expect(downloads.samplesFor(1)).toEqual([]);
    expect(downloads.samplesFor(42)).toEqual([]);
  });

  it('keeps hasLive on the live statuses only', async () => {
    const mock = wire([entry({ id: 1, status: 'completed' })]);
    const { downloads, init } = await import('./downloads.svelte');
    await init();
    expect(downloads.hasLive).toBe(false);
    mock.emit([entry({ id: 2, status: 'queued' })]);
    expect(downloads.hasLive).toBe(true);
  });
});
