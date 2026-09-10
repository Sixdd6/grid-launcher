import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

// Overlapping `refresh()` calls (downloads watcher, `images-replenished`, the
// Library activation effect) must resolve in START order: the newest call's
// answer sticks even when an older call resolves later (final review G1).
describe('installed.refresh ordering', () => {
  beforeEach(() => {
    vi.resetModules();
  });

  afterEach(() => {
    vi.doUnmock('../api');
    vi.doUnmock('@tauri-apps/api/event');
    vi.doUnmock('./downloads.svelte');
  });

  it('keeps the newest refresh result when an older call resolves later', async () => {
    const answers: Array<(rows: unknown[]) => void> = [];
    vi.doMock('../api', () => ({
      api: {
        listInstalled: () =>
          new Promise<unknown[]>((resolve) => {
            answers.push(resolve);
          }),
      },
    }));
    vi.doMock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));
    vi.doMock('./downloads.svelte', () => ({ downloads: { entries: [] } }));

    const { installed, refresh } = await import('./installed.svelte');

    const first = refresh();
    const second = refresh();
    expect(answers).toHaveLength(2);

    // The newer call answers first, then the older one arrives late.
    answers[1]([{ title: 'newest' }]);
    await second;
    answers[0]([{ title: 'stale' }]);
    await first;

    expect(installed.list.map((r: { title: string }) => r.title)).toEqual(['newest']);
  });
});
