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
