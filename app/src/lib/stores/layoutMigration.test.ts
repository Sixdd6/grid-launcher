import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

describe('initLayoutMigration', () => {
  beforeEach(() => {
    vi.resetModules();
  });

  afterEach(() => {
    vi.doUnmock('../api');
    vi.doUnmock('./toasts.svelte');
  });

  it('shows one error toast for a notice and never a second one', async () => {
    const calls: unknown[][] = [];
    vi.doMock('../api', () => ({
      api: { layoutMigrationNotice: () => Promise.resolve({ message: 'disk full' }) },
    }));
    vi.doMock('./toasts.svelte', () => ({
      pushToast: (...args: unknown[]) => calls.push(args),
    }));

    const { initLayoutMigration, MIGRATION_TOAST_DURATION_MS } = await import(
      './layoutMigration.svelte'
    );
    await initLayoutMigration();
    await initLayoutMigration();

    expect(MIGRATION_TOAST_DURATION_MS).toBe(20000);
    expect(calls).toEqual([
      [
        'Library reorganization did not finish: disk full. It will retry on next launch.',
        'error',
        20000,
      ],
    ]);
  });

  it('pushes one toast when two callers race in the same tick', async () => {
    const shown: string[] = [];
    vi.doMock('../api', () => ({
      api: { layoutMigrationNotice: () => Promise.resolve({ message: 'disk full' }) },
    }));
    vi.doMock('./toasts.svelte', () => ({ pushToast: (text: string) => shown.push(text) }));

    const { initLayoutMigration } = await import('./layoutMigration.svelte');
    await Promise.all([initLayoutMigration(), initLayoutMigration()]);

    expect(shown).toHaveLength(1);
  });

  it('shows nothing when the migration did not fail', async () => {
    const shown: string[] = [];
    vi.doMock('../api', () => ({ api: { layoutMigrationNotice: () => Promise.resolve(null) } }));
    vi.doMock('./toasts.svelte', () => ({ pushToast: (text: string) => shown.push(text) }));

    const { initLayoutMigration } = await import('./layoutMigration.svelte');
    await initLayoutMigration();

    expect(shown).toEqual([]);
  });

  it('swallows a failed pull', async () => {
    const shown: string[] = [];
    vi.doMock('../api', () => ({
      api: { layoutMigrationNotice: () => Promise.reject(new Error('no backend')) },
    }));
    vi.doMock('./toasts.svelte', () => ({ pushToast: (text: string) => shown.push(text) }));

    const { initLayoutMigration } = await import('./layoutMigration.svelte');
    await expect(initLayoutMigration()).resolves.toBeUndefined();
    expect(shown).toEqual([]);
  });
});
