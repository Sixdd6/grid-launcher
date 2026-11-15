import { describe, expect, it } from 'vitest';
import type { DownloadEntry } from '../api';
import { activeDownload, TAB_NAMES } from './topbar';

const entry = (o: Partial<DownloadEntry>): DownloadEntry =>
  ({
    id: 1, job: 'install', kind: 'game', rom_id: 1, source_id: '', title: 'Game', platform: 'PS', status: 'queued',
    downloaded_bytes: 0, total_bytes: 0, speed_bps: 0, install_processed_bytes: 0, install_total_bytes: 0,
    error: '', detail: '', drive_id: '', drive_name: '', drive_label: '', batch: 0, phase: 'none', failure: null, ...o,
  }) as DownloadEntry;

describe('activeDownload (item 124; ruling 21)', () => {
  it('none while nothing transfers', () => {
    expect(activeDownload([entry({ status: 'queued' })])).toBeNull();
  });
  it('the downloading row, with its percent', () => {
    expect(activeDownload([entry({ id: 2, status: 'queued' }), entry({ id: 3, title: 'Cinder Peaks', status: 'downloading', downloaded_bytes: 38, total_bytes: 100 })])).toEqual({
      id: 3,
      title: 'Cinder Peaks',
      pct: 38,
    });
  });
  it('else the installing row, by its install progress', () => {
    expect(activeDownload([entry({ status: 'installing', install_processed_bytes: 1, install_total_bytes: 4 })])?.pct).toBe(25);
  });
  it('a backup row shows like any other', () => {
    expect(activeDownload([entry({ job: 'backup', title: 'Velvet Static', status: 'downloading', downloaded_bytes: 1, total_bytes: 2 })])?.title).toBe('Velvet Static');
  });
});

describe('TAB_NAMES', () => {
  it('names three tabs, no Server', () => {
    expect(Object.keys(TAB_NAMES)).toEqual(['library', 'downloads', 'settings']);
  });
});
