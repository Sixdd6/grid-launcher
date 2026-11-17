import { describe, expect, it } from 'vitest';
import type { DownloadEntry, DownloadStatus } from '../../api';
import { activeCount, menuItems, startupMenuItems } from './menu';

describe('the Menu (items 363–364; TVRommStorefront: no Reconnect)', () => {
  it('counts active and queued rows', () => {
    const rows = (['queued', 'downloading', 'installing', 'completed', 'failed', 'cancelled', 'cancelling'] as DownloadStatus[]).map(
      (status) => ({ status }) as DownloadEntry,
    );
    expect(activeCount(rows)).toBe(3);
  });

  it('lists the items in order, with the counts, and no Reconnect or Discover (2026-11-17)', () => {
    expect(menuItems(2, 5).map((i) => i.label)).toEqual([
      'Downloads · 2 active',
      'Notifications · 5',
      'Settings',
      'Controls',
      'Switch to desktop mode',
      'Quit GRID',
    ]);
    expect(menuItems(0, 0)[0].label).toBe('Downloads');
    expect(menuItems(0, 0)[1].label).toBe('Notifications · 0');
  });

  it('leaves Discover out by default (removed for now, 2026-11-17)', () => {
    expect(menuItems(0, 0).map((i) => i.key)).toEqual(['downloads', 'notifications', 'settings', 'controls', 'desktop', 'quit']);
  });

  it('on a startup screen offers only Switch to desktop mode and Quit GRID (user decision 2026-11-10)', () => {
    expect(startupMenuItems().map((i) => [i.key, i.label])).toEqual([
      ['desktop', 'Switch to desktop mode'],
      ['quit', 'Quit GRID'],
    ]);
  });
});
