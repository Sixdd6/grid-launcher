import { describe, expect, it } from 'vitest';
import type { DownloadEntry, DownloadKind } from '../api';
import { actionFor, aggregate, currentTransfer, entryDetail, etaText, footerLine, formatSize, graphCaption, kindLabel, percent } from './format';

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

describe('formatSize', () => {
  it('formats bytes with 0 decimals', () => {
    expect(formatSize(0)).toBe('0 B');
    expect(formatSize(512)).toBe('512 B');
    expect(formatSize(1023)).toBe('1023 B');
  });

  it('divides by 1024 at the boundary and switches to 1 decimal', () => {
    expect(formatSize(1024)).toBe('1.0 KB');
    expect(formatSize(1536)).toBe('1.5 KB');
  });

  it('formats MB with 1 decimal', () => {
    expect(formatSize(1024 * 1024)).toBe('1.0 MB');
    expect(formatSize(10.5 * 1024 * 1024)).toBe('10.5 MB');
    expect(formatSize(25 * 1024 * 1024)).toBe('25.0 MB');
  });

  it('formats GB and stops dividing at TB (the last unit)', () => {
    expect(formatSize(1024 ** 3)).toBe('1.0 GB');
    expect(formatSize(1024 ** 4)).toBe('1.0 TB');
    expect(formatSize(1024 ** 5)).toBe('1024.0 TB');
  });

  it('clamps negative input to 0 B', () => {
    expect(formatSize(-100)).toBe('0 B');
  });
});

describe('percent', () => {
  it('is 0 when total is 0 or negative', () => {
    expect(percent(0, 0)).toBe(0);
    expect(percent(5, 0)).toBe(0);
    expect(percent(5, -10)).toBe(0);
  });

  it('truncates rather than rounds', () => {
    expect(percent(1, 3)).toBe(33);
    expect(percent(10.5 * 1024 * 1024, 25 * 1024 * 1024)).toBe(42);
  });

  it('clamps to 0..100', () => {
    expect(percent(150, 100)).toBe(100);
    expect(percent(-10, 100)).toBe(0);
  });

  it('handles exact values', () => {
    expect(percent(50, 100)).toBe(50);
    expect(percent(100, 100)).toBe(100);
  });
});

describe('entryDetail', () => {
  it('queued', () => {
    expect(entryDetail(entry({ status: 'queued' }))).toBe('Queued');
  });

  it('downloading with known total', () => {
    const e = entry({
      status: 'downloading',
      downloaded_bytes: 10.5 * 1024 * 1024,
      total_bytes: 25 * 1024 * 1024,
      speed_bps: 1.2 * 1024 * 1024,
    });
    expect(entryDetail(e)).toBe('Downloading 42% • 10.5 MB / 25.0 MB • 1.2 MB/s');
  });

  it('downloading with unknown total', () => {
    const e = entry({
      status: 'downloading',
      downloaded_bytes: 10.5 * 1024 * 1024,
      total_bytes: 0,
      speed_bps: 1.2 * 1024 * 1024,
    });
    expect(entryDetail(e)).toBe('Downloading • 10.5 MB • 1.2 MB/s');
  });

  it('installing with known total', () => {
    const e = entry({
      status: 'installing',
      install_processed_bytes: 10.5 * 1024 * 1024,
      install_total_bytes: 25 * 1024 * 1024,
    });
    expect(entryDetail(e)).toBe('Installing 42% • 10.5 MB / 25.0 MB');
  });

  it('installing with unknown total', () => {
    const e = entry({ status: 'installing', install_total_bytes: 0 });
    expect(entryDetail(e)).toBe('Installing...');
  });

  it('cancelling', () => {
    expect(entryDetail(entry({ status: 'cancelling' }))).toBe('Cancelling...');
  });

  it('completed with known size', () => {
    const e = entry({ status: 'completed', downloaded_bytes: 25 * 1024 * 1024 });
    expect(entryDetail(e)).toBe('Completed • 25.0 MB');
  });

  it('completed with zero bytes shows unknown size', () => {
    const e = entry({ status: 'completed', downloaded_bytes: 0 });
    expect(entryDetail(e)).toBe('Completed • Unknown size');
  });

  it('failed with an error message', () => {
    const e = entry({ status: 'failed', error: 'Disk full' });
    expect(entryDetail(e)).toBe('Failed • Disk full');
  });

  it('failed with a blank error falls back to Unknown error', () => {
    const e = entry({ status: 'failed', error: '' });
    expect(entryDetail(e)).toBe('Failed • Unknown error');
  });

  it('failed with a whitespace-only error falls back to Unknown error', () => {
    const e = entry({ status: 'failed', error: '   ' });
    expect(entryDetail(e)).toBe('Failed • Unknown error');
  });

  it('cancelled', () => {
    expect(entryDetail(entry({ status: 'cancelled' }))).toBe('Cancelled');
  });
});

describe('aggregate', () => {
  it('is empty with no entries', () => {
    expect(aggregate([])).toBe('');
  });

  it('is empty when nothing is live (only completed/failed/cancelled)', () => {
    const entries = [
      entry({ status: 'completed' }),
      entry({ status: 'failed' }),
      entry({ status: 'cancelled' }),
    ];
    expect(aggregate(entries)).toBe('');
  });

  it('singular active download', () => {
    expect(aggregate([entry({ status: 'downloading' })])).toBe('1 active download');
  });

  it('plural active downloads (downloading + cancelling)', () => {
    const entries = [entry({ status: 'downloading' }), entry({ status: 'cancelling' })];
    expect(aggregate(entries)).toBe('2 active downloads');
  });

  it('adds a singular queued suffix', () => {
    const entries = [entry({ status: 'downloading' }), entry({ status: 'queued' })];
    expect(aggregate(entries)).toBe('1 active download (1 queued download)');
  });

  it('adds a plural queued suffix', () => {
    const entries = [
      entry({ status: 'downloading' }),
      entry({ status: 'queued' }),
      entry({ status: 'queued' }),
    ];
    expect(aggregate(entries)).toBe('1 active download (2 queued downloads)');
  });

  it('reports queued-only entries as 0 active downloads', () => {
    const entries = [entry({ status: 'queued' }), entry({ status: 'queued' })];
    expect(aggregate(entries)).toBe('0 active downloads (2 queued downloads)');
  });

  it('finalize running with no active downloads: Installing 1 game', () => {
    expect(aggregate([entry({ status: 'installing' })])).toBe('Installing 1 game');
  });

  it('finalize running plus queued entries', () => {
    const entries = [entry({ status: 'installing' }), entry({ status: 'queued' })];
    expect(aggregate(entries)).toBe('Installing 1 game (1 queued download)');
  });

  it('finalize running but an active download exists: falls through to active download text', () => {
    const entries = [entry({ status: 'installing' }), entry({ status: 'downloading' })];
    expect(aggregate(entries)).toBe('1 active download');
  });
});

describe('actionFor', () => {
  it('cancel for queued, downloading, cancelling', () => {
    expect(actionFor('queued')).toBe('cancel');
    expect(actionFor('downloading')).toBe('cancel');
    expect(actionFor('cancelling')).toBe('cancel');
  });

  it('installing for installing', () => {
    expect(actionFor('installing')).toBe('installing');
  });

  it('retry-dismiss for failed and cancelled', () => {
    expect(actionFor('failed')).toBe('retry-dismiss');
    expect(actionFor('cancelled')).toBe('retry-dismiss');
  });

  it('dismiss for completed', () => {
    expect(actionFor('completed')).toBe('dismiss');
  });

  it('keeps every non-firmware kind on the status-only rule', () => {
    expect(actionFor('downloading', 'base')).toBe('cancel');
    expect(actionFor('downloading', 'emulator')).toBe('cancel');
    expect(actionFor('downloading', 'ps4_content')).toBe('cancel');
    expect(actionFor('installing', 'native_update')).toBe('installing');
    expect(actionFor('failed', 'compat_tool')).toBe('retry-dismiss');
    expect(actionFor('completed', 'xbox360_content')).toBe('dismiss');
    expect(actionFor('downloading', 'update')).toBe('cancel');
  });

  // A firmware row is an external entry with no queue slot: Cancel and
  // Retry are both no-ops on it, so neither button may render.
  it('offers no action on a live firmware row', () => {
    expect(actionFor('queued', 'firmware')).toBe('none');
    expect(actionFor('downloading', 'firmware')).toBe('none');
    expect(actionFor('installing', 'firmware')).toBe('none');
    expect(actionFor('cancelling', 'firmware')).toBe('none');
  });

  it('offers only dismiss on a terminal firmware row', () => {
    expect(actionFor('completed', 'firmware')).toBe('dismiss');
    expect(actionFor('failed', 'firmware')).toBe('dismiss');
    expect(actionFor('cancelled', 'firmware')).toBe('dismiss');
  });
});

describe('kindLabel', () => {
  it('is empty for the base kind', () => {
    expect(kindLabel('base')).toBe('');
  });

  it('is Content for ps4_content and xbox360_content', () => {
    expect(kindLabel('ps4_content')).toBe('Content');
    expect(kindLabel('xbox360_content')).toBe('Content');
  });

  it('is Update for native_update and update', () => {
    expect(kindLabel('native_update')).toBe('Update');
    expect(kindLabel('update')).toBe('Update');
  });

  it('is Emulator for emulator', () => {
    expect(kindLabel('emulator')).toBe('Emulator');
  });

  it('is Compat tool for compat_tool', () => {
    expect(kindLabel('compat_tool')).toBe('Compat tool');
  });

  it('is Firmware for firmware', () => {
    expect(kindLabel('firmware')).toBe('Firmware');
  });

  it('covers every DownloadKind variant with a defined label', () => {
    const kinds: DownloadKind[] = [
      'base',
      'update',
      'ps4_content',
      'xbox360_content',
      'native_update',
      'emulator',
      'compat_tool',
      'firmware',
    ];
    for (const kind of kinds) {
      expect(() => kindLabel(kind)).not.toThrow();
    }
  });
});

describe('footerLine', () => {
  it('is null when nothing is live, so the strip can hide', () => {
    expect(footerLine([])).toBeNull();
    expect(footerLine([entry({ status: 'completed' })])).toBeNull();
    expect(footerLine([entry({ status: 'failed' })])).toBeNull();
  });

  it('shows the downloading transfer with percent and speed', () => {
    const line = footerLine([
      entry({ title: 'Chrono Trigger', status: 'downloading', downloaded_bytes: 512, total_bytes: 1024, speed_bps: 2048 }),
    ]);
    expect(line).toBe('⬇ Chrono Trigger · 50% · 2.0 KB/s');
  });

  it('shows an em dash for an unknown total', () => {
    const line = footerLine([
      entry({ title: 'Chrono Trigger', status: 'downloading', downloaded_bytes: 512, speed_bps: 0 }),
    ]);
    expect(line).toBe('⬇ Chrono Trigger · — · 0 B/s');
  });

  it('prefers a downloading entry over an installing one', () => {
    const line = footerLine([
      entry({ id: 1, title: 'Installing One', status: 'installing', install_processed_bytes: 1, install_total_bytes: 2 }),
      entry({ id: 2, title: 'Downloading One', status: 'downloading', downloaded_bytes: 1, total_bytes: 4, speed_bps: 1024 }),
    ]);
    expect(line).toBe('⬇ Downloading One · 25% · 1.0 KB/s');
  });

  it('reports the phase instead of a speed for installing and queued work', () => {
    expect(
      footerLine([entry({ title: 'A', status: 'installing', install_processed_bytes: 3, install_total_bytes: 4 })]),
    ).toBe('⬇ A · 75% · Installing');
    expect(footerLine([entry({ title: 'A', status: 'queued' })])).toBe('⬇ A · — · Queued');
    expect(footerLine([entry({ title: 'A', status: 'cancelling' })])).toBe('⬇ A · — · Cancelling');
  });
});

describe('currentTransfer', () => {
  it('is null when nothing is live', () => {
    expect(currentTransfer([])).toBeNull();
    expect(currentTransfer([entry({ status: 'completed' }), entry({ status: 'failed' })])).toBeNull();
  });

  it('prefers downloading, then installing, then the first other live entry', () => {
    const queued = entry({ id: 1, status: 'queued' });
    const installing = entry({ id: 2, status: 'installing' });
    const downloading = entry({ id: 3, status: 'downloading' });
    expect(currentTransfer([queued, installing, downloading])?.id).toBe(3);
    expect(currentTransfer([queued, installing])?.id).toBe(2);
    expect(currentTransfer([queued])?.id).toBe(1);
    expect(currentTransfer([entry({ id: 9, status: 'cancelling' })])?.id).toBe(9);
  });

  it('is the entry footerLine describes', () => {
    const entries = [entry({ id: 1, status: 'installing' }), entry({ id: 2, title: 'Two', status: 'downloading' })];
    expect(currentTransfer(entries)?.title).toBe('Two');
    expect(footerLine(entries)).toContain('⬇ Two ·');
  });
});

describe('etaText', () => {
  it('is empty unless downloading with a known total and a positive speed', () => {
    expect(etaText(entry({ status: 'queued' }))).toBe('');
    expect(etaText(entry({ status: 'installing', install_processed_bytes: 1, install_total_bytes: 2 }))).toBe('');
    expect(etaText(entry({ status: 'downloading', downloaded_bytes: 1, total_bytes: 0, speed_bps: 10 }))).toBe('');
    expect(etaText(entry({ status: 'downloading', downloaded_bytes: 1, total_bytes: 10, speed_bps: 0 }))).toBe('');
  });

  it('formats seconds, minutes and hours, rounding up', () => {
    expect(etaText(entry({ status: 'downloading', downloaded_bytes: 500, total_bytes: 1000, speed_bps: 100 }))).toBe('5s left');
    expect(etaText(entry({ status: 'downloading', downloaded_bytes: 500, total_bytes: 1000, speed_bps: 4 }))).toBe('2m 5s left');
    expect(etaText(entry({ status: 'downloading', downloaded_bytes: 500, total_bytes: 1000, speed_bps: 0.1 }))).toBe('1h 23m left');
    expect(etaText(entry({ status: 'downloading', downloaded_bytes: 999, total_bytes: 1000, speed_bps: 1000 }))).toBe('1s left');
  });

  it('never goes negative when downloaded exceeds total', () => {
    expect(etaText(entry({ status: 'downloading', downloaded_bytes: 1200, total_bytes: 1000, speed_bps: 10 }))).toBe('0s left');
  });
});

describe('graphCaption', () => {
  it('shows the rate and the ETA while downloading', () => {
    expect(graphCaption(entry({ status: 'downloading', downloaded_bytes: 0, total_bytes: 2048, speed_bps: 1024 }))).toBe(
      '1.0 KB/s · 2s left',
    );
  });

  it('shows only the rate when the ETA is unknown', () => {
    expect(graphCaption(entry({ status: 'downloading', downloaded_bytes: 0, total_bytes: 0, speed_bps: 512 }))).toBe('512 B/s');
    expect(graphCaption(entry({ status: 'cancelling', speed_bps: 512 }))).toBe('512 B/s');
  });

  it('names the disk phase while installing and is blank otherwise', () => {
    expect(graphCaption(entry({ status: 'installing' }))).toBe('Writing to disk');
    expect(graphCaption(entry({ status: 'queued' }))).toBe('');
    expect(graphCaption(entry({ status: 'completed' }))).toBe('');
    expect(graphCaption(entry({ status: 'failed' }))).toBe('');
  });
});
