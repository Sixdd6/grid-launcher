import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { DownloadEntry } from '../api';

const api = vi.hoisted(() => ({
  libraryMoveCancel: vi.fn(),
  libraryMoveResume: vi.fn(),
  dismissDownload: vi.fn(async () => {}),
  cancelInstall: vi.fn(async () => {}),
  retryInstall: vi.fn(async () => {}),
}));
vi.mock('../api', () => ({ api }));

import { cancelEntry, dismissEntry, retryEntry, runRowAction } from './actions';

const row = (overrides: Partial<DownloadEntry>) => ({ id: 4, kind: 'base', status: 'downloading', ...overrides }) as DownloadEntry;

describe('downloads/actions', () => {
  beforeEach(() => {
    for (const fn of Object.values(api)) fn.mockClear();
  });

  it('cancels and retries an install by its entry', async () => {
    await cancelEntry(row({}));
    await retryEntry(row({ status: 'failed' }));
    await dismissEntry(4);
    expect(api.cancelInstall).toHaveBeenCalledWith(4);
    expect(api.retryInstall).toHaveBeenCalledWith(4);
    expect(api.dismissDownload).toHaveBeenCalledWith(4);
    expect(api.libraryMoveCancel).not.toHaveBeenCalled();
  });

  it('cancels a library move, and dismisses its failed row once undone', async () => {
    api.libraryMoveCancel.mockResolvedValueOnce({ status: 'cancelling' });
    await cancelEntry(row({ kind: 'library_move' }));
    expect(api.dismissDownload).not.toHaveBeenCalled();
    api.libraryMoveCancel.mockResolvedValueOnce({ status: 'cancelled' });
    await cancelEntry(row({ kind: 'library_move', status: 'failed' }));
    expect(api.dismissDownload).toHaveBeenCalledWith(4);
  });

  it('throws a refused move cancel with its message', async () => {
    api.libraryMoveCancel.mockResolvedValueOnce({ status: 'not_cancellable', message: 'Almost done' });
    await expect(cancelEntry(row({ kind: 'library_move' }))).rejects.toThrow('Almost done');
    api.libraryMoveCancel.mockResolvedValueOnce({ status: 'no_move', message: 'No move' });
    await expect(cancelEntry(row({ kind: 'library_move' }))).rejects.toThrow('No move');
  });

  it('resumes a library move, dismissing a failed row, and throws a refusal', async () => {
    api.libraryMoveResume.mockResolvedValueOnce({ status: 'resumed', new_root: '/new' });
    await retryEntry(row({ kind: 'library_move', status: 'failed' }));
    expect(api.dismissDownload).toHaveBeenCalledWith(4);
    api.libraryMoveResume.mockResolvedValueOnce({ status: 'refused', message: 'Target is full' });
    await expect(retryEntry(row({ kind: 'library_move', status: 'failed' }))).rejects.toThrow('Target is full');
  });

  it('tracks a row action: clear, pending, error, then not pending', async () => {
    const log: unknown[][] = [];
    const track = {
      clearError: (id: number) => log.push(['clear', id]),
      setError: (id: number, m: string) => log.push(['error', id, m]),
      setPending: (id: number, on: boolean) => log.push(['pending', id, on]),
    };
    await runRowAction(4, async () => {}, track);
    expect(log).toEqual([
      ['clear', 4],
      ['pending', 4, true],
      ['pending', 4, false],
    ]);
    log.length = 0;
    await runRowAction(4, () => Promise.reject('Refused'), track);
    expect(log).toEqual([
      ['clear', 4],
      ['pending', 4, true],
      ['error', 4, 'Refused'],
      ['pending', 4, false],
    ]);
  });
});
