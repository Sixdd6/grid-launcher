import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { BackupPlan, DriveInfo } from '../api';
import type { BackupDialogOpen } from './dialogState.svelte';

type Fn = ReturnType<typeof vi.fn>;

function drive(key: string): DriveInfo {
  return { key, label: key, mount: key, free_bytes: 100, total_bytes: 1000, status: 'ready', pack: null } as unknown as DriveInfo;
}

function plan(over: Partial<BackupPlan> = {}): BackupPlan {
  return {
    drive: '/a',
    games_to_write: [1],
    skipped: [],
    too_big: [],
    emulators: [],
    firmware: [],
    firmware_reason: '',
    runtime: null,
    bytes_to_write: 10,
    free_bytes: 100,
    reserved_bytes: 0,
    fits: true,
    local_available: false,
    use_local: true,
    sources: [],
    outdated: [],
    needs_choice: 0,
    local_bytes: 0,
    download_bytes: 10,
    ...over,
  };
}

describe('BackupPlanStore', () => {
  let api: Record<string, Fn>;
  let ready: DriveInfo[];

  beforeEach(() => {
    vi.resetModules();
    ready = [drive('/a'), drive('/b')];
    api = {
      getBackupSettings: vi.fn(async () => ({ include_firmware: false })),
      planBackup: vi.fn(async (key: string) => plan({ drive: key })),
      startBackup: vi.fn(async () => {}),
    };
    vi.doMock('../api', () => ({ api }));
    vi.doMock('../stores/drives.svelte', () => ({
      drives: {
        get ready() {
          return ready;
        },
        get connected() {
          return ready;
        },
      },
    }));
    vi.doMock('../stores/installed.svelte', () => ({ installed: { list: [] } }));
    vi.doMock('../stores/session.svelte', () => ({ session: { serverUrl: 'https://romm.test' } }));
  });

  afterEach(() => {
    for (const m of ['../api', '../stores/drives.svelte', '../stores/installed.svelte', '../stores/session.svelte']) vi.doUnmock(m);
  });

  async function store(open: BackupDialogOpen = { request: { kind: 'game', name: 'Zelda', rom_ids: [1] } as never }) {
    const { BackupPlanStore } = await import('./backupPlan.svelte');
    return new BackupPlanStore(() => open);
  }

  it('reads the firmware default before planning', async () => {
    const b = await store();
    b.planAll();
    expect(api.planBackup).not.toHaveBeenCalled();
    b.loadSettings();
    await vi.waitFor(() => expect(b.settingsLoaded).toBe(true));
    expect(b.includeFirmware).toBe(false);
  });

  it('keeps a drive chosen as drives come and go', async () => {
    const b = await store();
    b.keepChosen();
    expect(b.chosen).toBe('/a');
    b.choose('/b');
    ready = [drive('/a')];
    b.keepChosen();
    expect(b.chosen).toBe('/a');
  });

  it('plans the chosen drive first, then the rest', async () => {
    const b = await store();
    b.settingsLoaded = true;
    b.choose('/b');
    b.planAll();
    expect(b.planning).toEqual(new Set(['/a', '/b']));
    await vi.waitFor(() => expect(b.planning.size).toBe(0));
    expect(api.planBackup.mock.calls.map((c) => c[0])).toEqual(['/b', '/a']);
    expect(b.plan?.drive).toBe('/b');
    expect(b.footer.disabled).toBe(false);
  });

  it('keeps a plan refusal per drive and drops an older round', async () => {
    const b = await store();
    b.settingsLoaded = true;
    b.keepChosen();
    let release!: (p: BackupPlan) => void;
    api.planBackup.mockImplementationOnce(() => new Promise((res) => (release = res)));
    b.planAll();
    api.planBackup.mockRejectedValueOnce(new Error('read-only drive'));
    b.planAll();
    await vi.waitFor(() => expect(b.error).toBe('read-only drive'));
    release(plan({ drive: 'stale' }));
    await Promise.resolve();
    expect(b.plans['/a']).toBeUndefined();
  });

  it('start queues the chosen plan and runs onQueued; a refusal is the start error', async () => {
    const onQueued = vi.fn();
    const b = await store({ request: { kind: 'game' } as never, onQueued });
    b.settingsLoaded = true;
    b.keepChosen();
    b.planAll();
    await vi.waitFor(() => expect(b.planning.size).toBe(0));
    b.pick(1, 'download');
    expect(await b.start()).toBe(true);
    expect(onQueued).toHaveBeenCalledTimes(1);
    expect(api.startBackup.mock.calls[0][0]).toBe('/a');
    api.startBackup.mockRejectedValueOnce(new Error('drive full'));
    expect(await b.start()).toBe(false);
    expect(b.startError).toBe('drive full');
    expect(b.starting).toBe(false);
  });

  it('start does nothing without a ready plan', async () => {
    const b = await store();
    expect(await b.start()).toBe(false);
    expect(api.startBackup).not.toHaveBeenCalled();
  });
});
