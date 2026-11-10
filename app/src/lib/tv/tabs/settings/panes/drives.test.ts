import { describe, expect, it } from 'vitest';
import type { DriveInfo, PackSummary, RememberedDrive } from '../../../../api';
import { connectedRow, drivesPane, forgetList, rememberedRow } from './drives';

const GB = 1024 ** 3;

function pack(overrides: Partial<PackSummary> = {}): PackSummary {
  return { name: 'Couch kit', os: ['linux'], game_count: 8, gridpack_bytes: 13 * GB, emulators: [], firmware: [], updated: '', rom_ids: [], ...overrides };
}

function drive(overrides: Partial<DriveInfo> = {}): DriveInfo {
  return {
    key: '/m/T7',
    mount: '/m/T7',
    label: 'SAMSUNG T7',
    fs: 'exFAT',
    fat32: false,
    total_bytes: 1024 * GB,
    free_bytes: 61 * GB,
    read_only: false,
    status: 'ready',
    problem: '',
    pack: pack(),
    ...overrides,
  };
}

const NOW = new Date('2026-11-10T12:00:00Z');

describe('Settings › Backup drives rows (items 495–498)', () => {
  it('shows a set-up drive with its games and the green pill', () => {
    expect(connectedRow(drive())).toEqual({
      key: 'dr-/m/T7',
      title: 'Couch kit',
      sub: 'SAMSUNG T7 · exFAT · 1.0 TB · 8 games',
      pill: { text: 'Ready for backups', tone: 'ok' },
      opens: 'page',
      why: undefined,
    });
  });

  it('shows a drive that is not set up in the warning style, opening Set up for GRID', () => {
    const r = connectedRow(drive({ status: 'not_set_up', pack: null, label: 'USB-1TB', fs: 'FAT32' }));
    expect(r).toMatchObject({ title: 'USB-1TB', sub: 'USB-1TB · FAT32 · 1.0 TB', pill: { text: 'Not set up', tone: 'warn' }, opens: 'setup' });
  });

  it('a drive GRID can’t use says why instead of opening', () => {
    const r = connectedRow(drive({ status: 'read_only', read_only: true, pack: null }));
    expect(r.opens).toBe('none');
    expect(r.why).toMatch(/read-only/);
  });

  it('a remembered drive reads when it was last seen, its games and size (497)', () => {
    const away: RememberedDrive = { mount: '/m/K', name: 'Travel stick', label: 'KINGSTON', fs: 'exFAT', total_bytes: 128 * GB, game_count: 5, last_seen: '2026-11-07T10:00:00Z' };
    expect(rememberedRow(away, NOW)).toMatchObject({ title: 'Travel stick', sub: 'Last seen 3 days ago · 5 games · 128.0 GB', pill: { text: 'Not connected', tone: 'plain' }, opens: 'remembered' });
    expect(forgetList(away, NOW)).toMatchObject({ title: 'Travel stick', forget: 'Forget Travel stick', done: 'Travel stick forgotten' });
  });

  it('lists connected drives, then remembered ones', () => {
    const m = drivesPane([drive()], [], NOW);
    expect(m.connected).toHaveLength(1);
    expect(m.remembered).toEqual([]);
  });
});
