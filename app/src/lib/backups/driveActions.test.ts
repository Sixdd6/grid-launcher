import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { DriveInfo, RememberedDrive } from '../api';

type Fn = ReturnType<typeof vi.fn>;

function drive(over: Partial<DriveInfo> = {}): DriveInfo {
  return {
    key: '/media/kit',
    mount: '/media/kit',
    label: 'KIT',
    pack: { name: 'Couch kit', os: ['linux'], gridpack_bytes: 500 },
    ...over,
  } as unknown as DriveInfo;
}

describe('driveActions', () => {
  let api: Record<string, Fn>;
  let toasts: [string, string | undefined][];
  let refresh: Fn;
  let expectOsChangeBatch: Fn;

  beforeEach(() => {
    vi.resetModules();
    toasts = [];
    refresh = vi.fn(async () => {});
    expectOsChangeBatch = vi.fn();
    api = {
      setupDrive: vi.fn(async () => {}),
      renameDrive: vi.fn(async () => {}),
      setDriveOs: vi.fn(async () => ({ batch: null, queued: 0 })),
      removeGridFilesSize: vi.fn(async () => 1234),
      removeGridFiles: vi.fn(async () => {}),
      openDriveFolder: vi.fn(async () => {}),
      forgetDrive: vi.fn(async () => {}),
    };
    vi.doMock('../api', () => ({ api }));
    vi.doMock('../stores/drives.svelte', () => ({ refresh }));
    vi.doMock('../stores/toasts.svelte', () => ({
      pushToast: (text: string, level?: string) => toasts.push([text, level]),
    }));
    vi.doMock('../notifications/notifier', () => ({ expectOsChangeBatch }));
  });

  afterEach(() => {
    for (const m of ['../api', '../stores/drives.svelte', '../stores/toasts.svelte', '../notifications/notifier']) vi.doUnmock(m);
  });

  it('sets a drive up under the trimmed name, with its card, then refreshes', async () => {
    const a = await import('./driveActions');
    await a.setUpDrive(drive({ pack: null }), '  Couch kit ', ['linux']);
    expect(api.setupDrive).toHaveBeenCalledWith('/media/kit', 'Couch kit', ['linux']);
    expect(toasts).toEqual([['Couch kit is ready for backups', undefined]]);
    expect(refresh).toHaveBeenCalledTimes(1);
  });

  it('a refused set-up throws and refreshes nothing', async () => {
    api.setupDrive.mockRejectedValueOnce(new Error('read-only'));
    const a = await import('./driveActions');
    await expect(a.setUpDrive(drive(), 'Kit', ['linux'])).rejects.toThrow('read-only');
    expect(refresh).not.toHaveBeenCalled();
  });

  it('renames only a changed name', async () => {
    const a = await import('./driveActions');
    expect(await a.renameDrive(drive(), ' Couch kit ')).toBe(false);
    expect(api.renameDrive).not.toHaveBeenCalled();
    expect(await a.renameDrive(drive(), 'Den kit')).toBe(true);
    expect(api.renameDrive).toHaveBeenCalledWith('/media/kit', 'Den kit');
  });

  it('changes the computers only when the list changed, hinting the batch', async () => {
    const a = await import('./driveActions');
    expect(await a.changeDriveOs(drive(), ['linux'])).toBe(false);
    expect(await a.changeDriveOs(drive(), ['linux', 'windows'])).toBe(true);
    expect(api.setDriveOs).toHaveBeenCalledWith('/media/kit', ['linux', 'windows']);
    expect(expectOsChangeBatch).toHaveBeenCalledWith({ batch: null, queued: 0 }, ['windows']);
    expect(refresh).toHaveBeenCalledTimes(1);
  });

  it('measures the GRID files, falling back to the pack figure', async () => {
    const a = await import('./driveActions');
    expect(await a.gridFilesSize(drive())).toBe(1234);
    api.removeGridFilesSize.mockRejectedValueOnce(new Error('x'));
    expect(await a.gridFilesSize(drive())).toBe(500);
  });

  it('removes GRID files with a card, opens the folder, forgets a drive', async () => {
    const a = await import('./driveActions');
    await a.removeGridFiles(drive());
    expect(toasts).toEqual([['GRID files removed from KIT', undefined]]);
    api.openDriveFolder.mockRejectedValueOnce(new Error('no file manager'));
    a.openDriveFolder(drive());
    await vi.waitFor(() => expect(toasts.at(-1)).toEqual(['no file manager', 'error']));
    await a.forgetDrive({ mount: '/media/old' } as RememberedDrive);
    expect(api.forgetDrive).toHaveBeenCalledWith('/media/old');
  });
});
