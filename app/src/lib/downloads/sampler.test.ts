import { describe, expect, it } from 'vitest';
import type { DownloadEntry } from '../api';
import { SAMPLE_COUNT } from './ring';
import { createSampler, graphsOf, observe, SAMPLE_INTERVAL_MS, tick } from './sampler';

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

describe('sampler', () => {
  it('ticks once per second', () => {
    expect(SAMPLE_INTERVAL_MS).toBe(1000);
  });

  it('starts a track at the entry\'s current counters so a mid-transfer start adds no delta', () => {
    const s = createSampler(0);
    observe(s, [entry({ downloaded_bytes: 5_000 })]);
    tick(s, 1000);
    expect(graphsOf(s)).toEqual({ 1: [{ net: 0, disk: 0 }] });
  });

  it('accumulates the byte deltas of several events into one sample per tick', () => {
    const s = createSampler(0);
    observe(s, [entry({ downloaded_bytes: 0 })]);
    observe(s, [entry({ downloaded_bytes: 1_000 })]);
    observe(s, [entry({ downloaded_bytes: 2_500, install_processed_bytes: 400 })]);
    tick(s, 1000);
    expect(graphsOf(s)[1]).toEqual([{ net: 2_500, disk: 400 }]);
    // The pending deltas were consumed: the next second starts from zero.
    tick(s, 2000);
    expect(graphsOf(s)[1]).toEqual([
      { net: 2_500, disk: 400 },
      { net: 0, disk: 0 },
    ]);
  });

  it('normalises a late tick to bytes per second', () => {
    const s = createSampler(0);
    observe(s, [entry({ downloaded_bytes: 0 })]);
    observe(s, [entry({ downloaded_bytes: 3_000 })]);
    tick(s, 2000);
    expect(graphsOf(s)[1]).toEqual([{ net: 1_500, disk: 0 }]);
  });

  it('ignores a tick with no elapsed time', () => {
    const s = createSampler(1000);
    observe(s, [entry({ downloaded_bytes: 0 })]);
    observe(s, [entry({ downloaded_bytes: 10 })]);
    tick(s, 1000);
    expect(graphsOf(s)[1]).toEqual([]);
    tick(s, 2000);
    expect(graphsOf(s)[1]).toEqual([{ net: 10, disk: 0 }]);
  });

  it('clamps a counter that moves backwards to a zero delta', () => {
    const s = createSampler(0);
    observe(s, [entry({ downloaded_bytes: 500 })]);
    observe(s, [entry({ downloaded_bytes: 100 })]);
    observe(s, [entry({ downloaded_bytes: 200 })]);
    tick(s, 1000);
    expect(graphsOf(s)[1]).toEqual([{ net: 100, disk: 0 }]);
  });

  it('freezes a terminal entry\'s ring instead of appending zeros', () => {
    const s = createSampler(0);
    observe(s, [entry({ downloaded_bytes: 0 })]);
    observe(s, [entry({ downloaded_bytes: 800 })]);
    tick(s, 1000);
    observe(s, [entry({ status: 'completed', downloaded_bytes: 800 })]);
    tick(s, 2000);
    tick(s, 3000);
    expect(graphsOf(s)[1]).toEqual([{ net: 800, disk: 0 }]);
  });

  it('samples queued, installing and cancelling entries too (they are live)', () => {
    const s = createSampler(0);
    observe(s, [
      entry({ id: 1, status: 'queued' }),
      entry({ id: 2, status: 'installing', install_processed_bytes: 0 }),
      entry({ id: 3, status: 'cancelling' }),
    ]);
    observe(s, [
      entry({ id: 1, status: 'queued' }),
      entry({ id: 2, status: 'installing', install_processed_bytes: 640 }),
      entry({ id: 3, status: 'cancelling' }),
    ]);
    tick(s, 1000);
    expect(graphsOf(s)).toEqual({
      1: [{ net: 0, disk: 0 }],
      2: [{ net: 0, disk: 640 }],
      3: [{ net: 0, disk: 0 }],
    });
  });

  it('drops the track when the entry leaves the snapshot', () => {
    const s = createSampler(0);
    observe(s, [entry({ id: 1 }), entry({ id: 2 })]);
    tick(s, 1000);
    observe(s, [entry({ id: 2 })]);
    expect(Object.keys(graphsOf(s))).toEqual(['2']);
  });

  it('returns the same array for a track whose ring did not change', () => {
    const s = createSampler(0);
    observe(s, [entry({ id: 1 }), entry({ id: 2 })]);
    observe(s, [entry({ id: 1, downloaded_bytes: 100 }), entry({ id: 2, downloaded_bytes: 100 })]);
    tick(s, 1000);
    const first = graphsOf(s);
    // Nothing moved: the record itself comes back unchanged.
    expect(graphsOf(s)).toBe(first);

    // Track 2 is frozen, so only track 1 gets a new sample; track 2 keeps
    // the array it already handed out.
    observe(s, [
      entry({ id: 1, downloaded_bytes: 300 }),
      entry({ id: 2, status: 'completed', downloaded_bytes: 100 }),
    ]);
    tick(s, 2000);
    const second = graphsOf(s);
    expect(second).not.toBe(first);
    expect(second[1]).not.toBe(first[1]);
    expect(second[2]).toBe(first[2]);
  });

  it('keeps only the newest SAMPLE_COUNT samples', () => {
    const s = createSampler(0);
    observe(s, [entry({ downloaded_bytes: 0 })]);
    for (let i = 1; i <= SAMPLE_COUNT + 5; i += 1) {
      observe(s, [entry({ downloaded_bytes: i * 10 })]);
      tick(s, i * 1000);
    }
    const samples = graphsOf(s)[1];
    expect(samples).toHaveLength(SAMPLE_COUNT);
    expect(samples[0]).toEqual({ net: 10, disk: 0 });
    expect(samples[SAMPLE_COUNT - 1]).toEqual({ net: 10, disk: 0 });
  });
});
