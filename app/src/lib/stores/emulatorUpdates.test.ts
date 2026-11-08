import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { EmulatorEntry, VersionCheck } from '../api';

const entry = (name: string, source_id?: string): EmulatorEntry => ({ name, path: '', args: '', source_id });

describe('emulator update checks', () => {
  let calls: string[];

  beforeEach(() => {
    vi.resetModules();
    calls = [];
    vi.doMock('../api', () => ({
      api: {
        checkEmulatorUpdate: (name: string): Promise<VersionCheck> => {
          calls.push(name);
          if (name === 'Offline') return Promise.reject(new Error('offline'));
          return Promise.resolve({
            installed_display: 'v1',
            available_display: name === 'PCSX2' ? 'v2' : 'v1',
            up_to_date: name !== 'PCSX2',
          });
        },
      },
    }));
  });

  afterEach(() => {
    vi.doUnmock('../api');
  });

  it('checks once per session, only emulators installed from a source', async () => {
    const store = await import('./emulatorUpdates.svelte');
    const entries = [entry('PCSX2', 'PCSX2/pcsx2'), entry('Manual'), entry('Redream', 'redream/redream')];
    await store.checkOncePerSession(entries);
    await store.checkOncePerSession(entries);
    expect(calls).toEqual(['PCSX2', 'Redream']);
    expect(store.emulatorUpdates.hasUpdate('PCSX2')).toBe(true);
    expect(store.emulatorUpdates.hasUpdate('Redream')).toBe(false);
    expect(store.emulatorUpdates.check('Manual')).toBeUndefined();
  });

  it('re-checks when asked, and an unanswered check claims nothing', async () => {
    const store = await import('./emulatorUpdates.svelte');
    await store.checkOncePerSession([entry('PCSX2', 'x')]);
    await store.recheck([entry('PCSX2', 'x'), entry('Offline', 'y')]);
    expect(calls).toEqual(['PCSX2', 'PCSX2', 'Offline']);
    expect(store.emulatorUpdates.check('Offline')).toBeUndefined();
  });

  it('an update that finished makes the emulator current', async () => {
    const store = await import('./emulatorUpdates.svelte');
    await store.checkOncePerSession([entry('PCSX2', 'x')]);
    store.markCurrent('PCSX2');
    expect(store.emulatorUpdates.hasUpdate('PCSX2')).toBe(false);
    expect(store.emulatorUpdates.check('PCSX2')?.installed_display).toBe('v2');
  });
});

describe('updateIsOut', () => {
  it('counts only a known newer version', async () => {
    const { updateIsOut } = await import('./emulatorUpdates.svelte');
    const check = (installed_display: string, available_display: string, up_to_date = false) => ({
      installed_display,
      available_display,
      up_to_date,
    });
    expect(updateIsOut(check('v1', 'v2'))).toBe(true);
    expect(updateIsOut(check('v2', 'v2', true))).toBe(false);
    expect(updateIsOut(check('v1', 'Unknown (direct source)'))).toBe(false);
    expect(updateIsOut(check('unknown', 'v2'))).toBe(false);
  });
});
