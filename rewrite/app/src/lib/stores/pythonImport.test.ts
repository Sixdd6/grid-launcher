import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

describe('initPythonImport', () => {
  beforeEach(() => {
    vi.resetModules();
  });

  afterEach(() => {
    vi.doUnmock('../api');
    vi.doUnmock('./toasts.svelte');
  });

  it('shows one toast for a report and never a second one', async () => {
    const shown: string[] = [];
    vi.doMock('../api', () => ({
      api: {
        pythonImportNotice: () =>
          Promise.resolve({ emulators: 2, games: 14, skipped_games: 1, retroachievements: false }),
      },
    }));
    vi.doMock('./toasts.svelte', () => ({ pushToast: (text: string) => shown.push(text) }));

    const { initPythonImport } = await import('./pythonImport.svelte');
    await initPythonImport();
    await initPythonImport();

    expect(shown).toEqual([
      'Imported 2 emulators and 14 games from the previous version. Enter your RomM token to reconnect.',
    ]);
  });

  it('shows the notice for the long import duration, not the routine one', async () => {
    const calls: unknown[][] = [];
    vi.doMock('../api', () => ({
      api: {
        pythonImportNotice: () =>
          Promise.resolve({ emulators: 1, games: 2, skipped_games: 0, retroachievements: false }),
      },
    }));
    vi.doMock('./toasts.svelte', () => ({
      pushToast: (...args: unknown[]) => calls.push(args),
      TOAST_DURATION_MS: 4000,
    }));

    const { initPythonImport, IMPORT_TOAST_DURATION_MS } = await import('./pythonImport.svelte');
    await initPythonImport();

    expect(IMPORT_TOAST_DURATION_MS).toBe(20000);
    expect(calls).toHaveLength(1);
    expect(calls[0][1]).toBe('success');
    expect(calls[0][2]).toBe(20000);
  });

  it('pushes one toast when two callers race in the same tick', async () => {
    const shown: string[] = [];
    vi.doMock('../api', () => ({
      api: {
        pythonImportNotice: () =>
          Promise.resolve({ emulators: 1, games: 1, skipped_games: 0, retroachievements: false }),
      },
    }));
    vi.doMock('./toasts.svelte', () => ({ pushToast: (text: string) => shown.push(text) }));

    const { initPythonImport } = await import('./pythonImport.svelte');
    await Promise.all([initPythonImport(), initPythonImport()]);

    expect(shown).toEqual([
      'Imported 1 emulator and 1 game from the previous version. Enter your RomM token to reconnect.',
    ]);
  });

  it('shows nothing when there was no import', async () => {
    const shown: string[] = [];
    vi.doMock('../api', () => ({ api: { pythonImportNotice: () => Promise.resolve(null) } }));
    vi.doMock('./toasts.svelte', () => ({ pushToast: (text: string) => shown.push(text) }));

    const { initPythonImport } = await import('./pythonImport.svelte');
    await initPythonImport();

    expect(shown).toEqual([]);
  });

  it('swallows a failed pull', async () => {
    const shown: string[] = [];
    vi.doMock('../api', () => ({
      api: { pythonImportNotice: () => Promise.reject(new Error('no backend')) },
    }));
    vi.doMock('./toasts.svelte', () => ({ pushToast: (text: string) => shown.push(text) }));

    const { initPythonImport } = await import('./pythonImport.svelte');
    await expect(initPythonImport()).resolves.toBeUndefined();
    expect(shown).toEqual([]);
  });
});
