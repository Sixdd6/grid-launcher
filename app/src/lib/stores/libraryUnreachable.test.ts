import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

type Fn = ReturnType<typeof vi.fn>;

describe('relocateTo', () => {
  let api: Record<string, Fn>;
  let toasts: [string, string | undefined][];

  beforeEach(() => {
    vi.resetModules();
    toasts = [];
    api = {
      libraryRelocate: vi.fn(async (path: string) => ({ status: 'relocated', library_path: path, report: [] })),
      libraryUnreachableStatus: vi.fn(async () => ({ status: 'ok' })),
    };
    vi.doMock('../api', () => ({ api }));
    vi.doMock('./toasts.svelte', () => ({
      pushToast: (text: string, level?: string) => toasts.push([text, level]),
    }));
  });

  afterEach(() => {
    vi.doUnmock('../api');
    vi.doUnmock('./toasts.svelte');
  });

  it('relocates to the trimmed folder and answers no error line', async () => {
    const { relocateTo } = await import('./libraryUnreachable.svelte');
    expect(await relocateTo('  /mnt/games ')).toBeNull();
    expect(api.libraryRelocate).toHaveBeenCalledWith('/mnt/games');
    expect(toasts).toEqual([['Library relocated to /mnt/games.', 'success']]);
  });

  it('answers a refusal or a failed call as the error line', async () => {
    const { relocateTo } = await import('./libraryUnreachable.svelte');
    api.libraryRelocate.mockResolvedValueOnce({ status: 'refused', message: 'Not your library' });
    expect(await relocateTo('/x')).toBe('Not your library');
    api.libraryRelocate.mockRejectedValueOnce(new Error('offline'));
    expect(await relocateTo('/x')).toBe('offline');
  });
});
