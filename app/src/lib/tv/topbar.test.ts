import { describe, expect, it } from 'vitest';
import type { DownloadEntry } from '../api';
import { activeDownload, connection } from './topbar';

const entry = (o: Partial<DownloadEntry>): DownloadEntry =>
  ({
    id: 1, job: 'install', kind: 'game', rom_id: 1, source_id: '', title: 'Game', platform: 'PS', status: 'queued',
    downloaded_bytes: 0, total_bytes: 0, speed_bps: 0, install_processed_bytes: 0, install_total_bytes: 0,
    error: '', detail: '', drive_id: '', drive_name: '', drive_label: '', batch: 0, ...o,
  }) as DownloadEntry;

describe('connection (item 125, 517)', () => {
  it('reads the three states', () => {
    expect(connection({ connected: true, offline: false, username: 'sam' })).toEqual({ dot: 'online', text: 'sam' });
    expect(connection({ connected: false, offline: true, username: 'sam' })).toEqual({ dot: 'none', text: 'Offline · no server' });
    expect(connection({ connected: false, offline: false, username: 'sam' })).toEqual({ dot: 'offline', text: 'Offline' });
  });
});

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
