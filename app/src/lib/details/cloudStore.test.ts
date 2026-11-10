import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { CloudPanelInfo, CloudRecord, InstalledGame } from '../api';
import type { CloudInput } from './cloudStore.svelte';

type Fn = ReturnType<typeof vi.fn>;

const game = { title: 'Zelda', platform: 'SNES', rom_id: 3 } as InstalledGame;

function info(over: Partial<CloudPanelInfo> = {}): CloudPanelInfo {
  return { supported: true, block_reason: '', scope: 'game', ...over } as CloudPanelInfo;
}

function record(id: number, over: Partial<CloudRecord> = {}): CloudRecord {
  return { id, restorable: true, restore_tooltip: '', emulator: 'RetroArch', ...over } as CloudRecord;
}

describe('CloudStore', () => {
  let api: Record<string, Fn>;

  beforeEach(() => {
    vi.resetModules();
    api = {
      cloudRecords: vi.fn(async () => [record(1)]),
      nativeSavePaths: vi.fn(async () => ({ pcgw: ['a'], manual: [] })),
      cloudUpload: vi.fn(async () => ({ uploaded: 1, messages: [{ text: 'Uploaded' }] })),
      cloudRestore: vi.fn(async () => ({ ok: true, messages: [], skipped_folders: [] })),
      cloudDelete: vi.fn(async () => {}),
      nativeAddManualSavePath: vi.fn(async () => {}),
      nativeRemoveSavePath: vi.fn(async () => {}),
    };
    vi.doMock('../api', () => ({ api }));
  });

  afterEach(() => vi.doUnmock('../api'));

  async function store(over: Partial<CloudInput> = {}) {
    const { CloudStore } = await import('./cloudStore.svelte');
    const input: CloudInput = { game, gameTitle: 'Zelda', saveType: 'save', panelInfo: info(), isNative: false, ...over };
    return new CloudStore(() => input);
  }

  it('loads the records', async () => {
    const c = await store();
    c.load();
    await vi.waitFor(() => expect(c.recordsLoading).toBe(false));
    expect(c.records.map((r) => r.id)).toEqual([1]);
    expect(api.nativeSavePaths).not.toHaveBeenCalled();
  });

  it('fetches nothing for an unsupported emulator', async () => {
    const c = await store({ panelInfo: info({ supported: false, block_reason: 'No emulator' }) });
    c.load();
    expect(c.recordsBlocked).toBe(true);
    expect(c.recordsLoading).toBe(false);
    expect(api.cloudRecords).not.toHaveBeenCalled();
    expect(c.uploadEnabled).toBe(false);
    expect(c.uploadTooltip).toBe('No emulator');
  });

  it('loads native save folders and enables upload once one is known', async () => {
    const c = await store({ isNative: true, panelInfo: info({ supported: false }) });
    expect(c.nativePhase).toBe('loading');
    c.load();
    await vi.waitFor(() => expect(c.nativePhase).toBe('loaded'));
    expect(c.nativePathCount).toBe(1);
    expect(c.uploadEnabled).toBe(true);
  });

  it('drops a records answer a newer load superseded', async () => {
    let first!: (v: CloudRecord[]) => void;
    api.cloudRecords.mockImplementationOnce(() => new Promise((res) => (first = res)));
    api.cloudRecords.mockResolvedValueOnce([record(2)]);
    const c = await store();
    const a = c.loadRecords();
    await c.loadRecords();
    first([record(1)]);
    await a;
    expect(c.records.map((r) => r.id)).toEqual([2]);
  });

  it('upload keeps the report lines and reloads when anything went up', async () => {
    const c = await store();
    await c.upload();
    expect(c.uploadMessages).toEqual([{ text: 'Uploaded' }]);
    expect(api.cloudRecords).toHaveBeenCalledTimes(1);
    api.cloudUpload.mockRejectedValueOnce(new Error('offline'));
    await c.upload();
    expect(c.uploadError).toBe('offline');
  });

  it('a restore that skipped folders offers restore-all with them', async () => {
    api.cloudRestore.mockResolvedValueOnce({
      ok: true,
      messages: [{ text: 'Skipped one folder.' }],
      skipped_folders: ['/saves/other'],
    });
    const c = await store();
    c.requestRestore(record(5, { restorable: false }));
    expect(c.confirmState).toBeNull();
    c.requestRestore(record(5));
    expect(c.confirmText).not.toBeNull();
    await c.confirmAction();
    expect(api.cloudRestore).toHaveBeenCalledWith(game, 'save', '5', false);
    expect(c.actionNotice).toBe('Skipped one folder.');
    expect(c.confirmState).toEqual({ kind: 'restore-all', record: record(5), folders: ['/saves/other'] });
    await c.confirmAction();
    expect(api.cloudRestore).toHaveBeenLastCalledWith(game, 'save', '5', true);
    expect(c.confirmState).toBeNull();
  });

  it('a failed restore says so; delete reloads', async () => {
    api.cloudRestore.mockResolvedValueOnce({ ok: false, messages: [], skipped_folders: [] });
    const c = await store();
    c.requestRestore(record(5));
    await c.confirmAction();
    expect(c.actionError).toBe('Restore failed.');
    c.requestDelete(record(6));
    await c.confirmAction();
    expect(api.cloudDelete).toHaveBeenCalledWith('save', 6);
    expect(c.actionPendingId).toBeNull();
    c.requestDelete(record(6));
    c.cancelConfirm();
    expect(c.confirmState).toBeNull();
  });

  it('adds and removes save folders', async () => {
    const c = await store({ isNative: true });
    await c.addManualPath();
    expect(api.nativeAddManualSavePath).not.toHaveBeenCalled();
    c.manualPathInput = '  /home/me/saves ';
    await c.addManualPath();
    expect(api.nativeAddManualSavePath).toHaveBeenCalledWith(game, '/home/me/saves');
    expect(c.manualPathInput).toBe('');
    api.nativeRemoveSavePath.mockRejectedValueOnce(new Error('nope'));
    await c.removeSavePath('/home/me/saves');
    expect(c.manualPathError).toBe('nope');
    expect(c.manualPathPending).toBe(false);
  });
});
