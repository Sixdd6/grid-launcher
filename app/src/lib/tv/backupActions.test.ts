import { beforeEach, describe, expect, it, vi } from 'vitest';

const { pushPage } = vi.hoisted(() => ({ pushPage: vi.fn() }));
vi.mock('./nav.svelte', () => ({ pushPage }));
vi.mock('./pages/TvBackupPage.svelte', () => ({ default: {} }));

import { openBackupPage } from './backupActions';

describe('openBackupPage', () => {
  beforeEach(() => pushPage.mockClear());

  it('pushes the Backup page with the request and its title, whatever RomM is doing', () => {
    openBackupPage({ kind: 'games', count: 2 }, [901, 902]);
    expect(pushPage).toHaveBeenCalledTimes(1);
    const page = pushPage.mock.calls[0][0];
    expect(page.title).toBe('Backup 2 games');
    expect(page.props).toEqual({ open: { request: { rom_ids: [901, 902], title: 'Backup 2 games' } } });
  });

  it('passes the platform ids along for a platform', () => {
    openBackupPage({ kind: 'platform', platform: 'PlayStation', count: 1 }, [5], [7]);
    expect(pushPage.mock.calls[0][0].props.open.request).toEqual({
      rom_ids: [5],
      platform_ids: [7],
      title: 'Backup PlayStation · 1 game',
    });
  });
});
