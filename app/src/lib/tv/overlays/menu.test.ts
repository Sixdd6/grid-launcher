import { describe, expect, it } from 'vitest';
import type { DownloadEntry, DownloadStatus } from '../../api';
import { activeCount, menuHead, menuItems } from './menu';

describe('the Menu (items 363–364)', () => {
  it('heads with the user and server host, or Offline', () => {
    expect(menuHead({ connected: true, username: 'sam', serverUrl: 'https://romm.home.lan' })).toBe('sam · romm.home.lan');
    expect(menuHead({ connected: false, username: 'sam', serverUrl: 'https://romm.home.lan' })).toBe('Offline');
  });

  it('counts active and queued rows', () => {
    const rows = (['queued', 'downloading', 'installing', 'completed', 'failed', 'cancelled', 'cancelling'] as DownloadStatus[]).map(
      (status) => ({ status }) as DownloadEntry,
    );
    expect(activeCount(rows)).toBe(3);
  });

  it('lists the items in order, with the counts', () => {
    expect(menuItems(2, 5).map((i) => i.label)).toEqual([
      'Discover',
      'Downloads · 2 active',
      'Notifications · 5',
      'Settings',
      'Reconnect',
      'Switch to desktop mode',
      'Quit GRID',
    ]);
    expect(menuItems(0, 0)[1].label).toBe('Downloads');
    expect(menuItems(0, 0)[2].label).toBe('Notifications · 0');
  });
});
