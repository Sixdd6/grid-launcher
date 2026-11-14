import { describe, expect, it, vi } from 'vitest';

vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

import { installerCards, isCloudConflict } from './ui.svelte';
import { FakeStores } from '../fakeApi';
import { row } from '../fixtures.test-util';
import { entryDetail, footerLine, kindLabel } from '../../downloads/format';
import { noticesFor } from '../../notifications/notices';
import type { DownloadEntry } from '../../api';

describe('installer cards from Downloads rows', () => {
  const fake = new FakeStores();
  it('raises the wizard card once, when a RomM install’s installer opens its window', () => {
    const before = fake.rommInstallerRow('Foxglove', 'Windows', 'installer_silent', 0.7);
    const after: DownloadEntry = { ...before, phase: 'installer_wizard' };
    const prev = new Map([[before.id, before]]);
    expect(installerCards(prev, [after]).map((c) => c.text)).toEqual([
      'Foxglove’s installer couldn’t run silently, so its setup window opened. Finish it there; GRID adds the game when it closes.',
    ]);
    expect(installerCards(new Map([[after.id, after]]), [after])).toEqual([]);
  });
  it('raises the failure card with Install <base>', () => {
    const failed = fake.failedInstallerRow('Low Tide Radio', 'Windows', { code: 'installer_dlc_base_missing', base_title: 'Pale Summit', base_gog_id: '', kept: false }, '');
    const prev = new Map([[failed.id, { ...failed, status: 'installing' as const, failure: null }]]);
    const cards = installerCards(prev, [failed]);
    expect(cards).toHaveLength(1);
    expect(cards[0].action).toMatchObject({ kind: 'install_base', title: 'Pale Summit' });
  });
  it('leaves store rows to the backend’s store-notice', () => {
    const prev = new Map([[1, row({ id: 1 })]]);
    expect(installerCards(prev, [row({ id: 1, status: 'failed' })])).toEqual([]);
  });
  it('knows a cloud conflict refusal', () => {
    expect(isCloudConflict({ code: 'store_cloud_conflict', message: 'x' })).toBe(true);
    expect(isCloudConflict(new Error('nope'))).toBe(false);
  });
});

describe('the Downloads formatter with store rows', () => {
  it('words store rows, kinds and the strip', () => {
    expect(entryDetail(row())).toBe('Downloading 45% · 41.2 MB/s');
    expect(kindLabel('store_tool')).toBe('Store tool');
    expect(kindLabel('store_install')).toBe('');
    expect(footerLine([row({ downloaded_bytes: 63 })])).toBe('Copper Saints · GOG · Downloading 63% · 41.2 MB/s');
    const fake = new FakeStores();
    const r = fake.rommInstallerRow('Grey Meridian', 'Windows', 'installer_silent', 0.4);
    expect(entryDetail(r, { compat: 'GE-Proton10-17' })).toBe('Windows · Running the GOG installer silently under GE-Proton10-17 · 40%');
  });
  it('gives a failed installer archive no generic failure card', () => {
    const fake = new FakeStores();
    const failed = fake.failedInstallerRow('Iron Lullaby', 'Windows', { code: 'installer_unrecognised', base_title: '', base_gog_id: '', kept: false }, 'x');
    expect(noticesFor({ entry: failed, from: 'installing' }, { setupActive: false })).toEqual([]);
    // Nor one whose installer couldn't run silently (TV mode words it).
    const silent: DownloadEntry = {
      ...failed,
      failure: null,
      error: "Couldn't run the installer silently: the installer cancelled itself. The installer was kept.",
    };
    expect(noticesFor({ entry: silent, from: 'installing' }, { setupActive: false })).toEqual([]);
  });
});
