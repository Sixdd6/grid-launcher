import { describe, expect, it } from 'vitest';
import type { DownloadEntry, DownloadStatus } from '../api';
import {
  groupBySegment,
  LEGEND_TEXT,
  segmentEmptyText,
  segmentLabel,
  segmentOf,
  SEGMENTS,
} from './segments';

function entry(overrides: Partial<DownloadEntry>): DownloadEntry {
  return {
    id: 1,
    job: 'game',
    kind: 'base',
    rom_id: 1,
    source_id: '',
    title: 'Game',
    platform: 'Platform',
    status: 'queued',
    downloaded_bytes: 0,
    total_bytes: 0,
    speed_bps: 0,
    install_processed_bytes: 0,
    install_total_bytes: 0,
    error: '',
    ...overrides,
  };
}

describe('segments', () => {
  it('lists the three segments in design §8 order', () => {
    expect(SEGMENTS).toEqual(['active', 'queued', 'completed']);
  });

  it('carries the legend line verbatim', () => {
    expect(LEGEND_TEXT).toBe(
      'Active: downloading or installing · Queued: waiting for a slot · Completed: finished, failed, or cancelled',
    );
  });

  it('maps every status to exactly one segment', () => {
    const expected: Record<DownloadStatus, string> = {
      downloading: 'active',
      installing: 'active',
      cancelling: 'active',
      queued: 'queued',
      completed: 'completed',
      failed: 'completed',
      cancelled: 'completed',
    };
    for (const [status, seg] of Object.entries(expected)) {
      expect(segmentOf(status as DownloadStatus)).toBe(seg);
    }
  });

  it('labels the segments', () => {
    expect(segmentLabel('active')).toBe('Active');
    expect(segmentLabel('queued')).toBe('Queued');
    expect(segmentLabel('completed')).toBe('Completed');
  });

  it('has an empty line per segment', () => {
    expect(segmentEmptyText('active')).toBe('No active transfers');
    expect(segmentEmptyText('queued')).toBe('Nothing waiting');
    expect(segmentEmptyText('completed')).toBe('Nothing finished yet');
  });

  it('groups entries by segment and keeps the snapshot order inside each group', () => {
    const entries = [
      entry({ id: 5, status: 'completed' }),
      entry({ id: 4, status: 'queued' }),
      entry({ id: 3, status: 'installing' }),
      entry({ id: 2, status: 'failed' }),
      entry({ id: 1, status: 'downloading' }),
    ];
    const groups = groupBySegment(entries);
    expect(groups.active.map((e) => e.id)).toEqual([3, 1]);
    expect(groups.queued.map((e) => e.id)).toEqual([4]);
    expect(groups.completed.map((e) => e.id)).toEqual([5, 2]);
  });

  it('always returns all three keys, empty when nothing matches', () => {
    expect(groupBySegment([])).toEqual({ active: [], queued: [], completed: [] });
  });
});
