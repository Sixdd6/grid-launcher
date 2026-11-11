import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

// The TV stack fed by the app's pushed cards, with its timers.
describe('tvNotices', () => {
  beforeEach(() => {
    vi.resetModules();
    vi.useFakeTimers();
  });
  afterEach(() => vi.useRealTimers());

  it('shows the same pushed cards while attached, and closes them after their life', async () => {
    const { pushToast } = await import('../stores/toasts.svelte');
    const n = await import('./notices.svelte');
    pushToast('before TV mode');
    expect(n.tvNotices.cards).toHaveLength(0);
    expect(n.tvNotices.history.map((h) => h.title)).toEqual(['before TV mode']);
    n.attachNotices(true);
    pushToast('Zelda installed');
    pushToast("Couldn't install Mario: 503", 'error');
    expect(n.tvNotices.cards.map((c) => c.title)).toEqual(["Couldn't install Mario: 503", 'Zelda installed']);
    vi.advanceTimersByTime(10_000);
    expect(n.tvNotices.cards.map((c) => c.title)).toEqual(["Couldn't install Mario: 503"]);
    vi.advanceTimersByTime(5_000);
    expect(n.tvNotices.cards).toHaveLength(0);
    expect(n.tvNotices.history).toHaveLength(3);
  });

  it('View closes the newest card with an action and runs it', async () => {
    const { pushToast } = await import('../stores/toasts.svelte');
    const n = await import('./notices.svelte');
    n.attachNotices(true);
    const run = vi.fn();
    pushToast('Zelda added to download queue', 'success', undefined, { action: { label: 'Open Downloads', run } });
    pushToast('Zelda installed');
    expect(n.viewNewest()).toBe(true);
    expect(run).toHaveBeenCalledOnce();
    expect(n.tvNotices.cards.map((c) => c.title)).toEqual(['Zelda installed']);
    expect(n.viewNewest()).toBe(false);
  });

  it('a held card counts down once released, still pinned, and history clears', async () => {
    const n = await import('./notices.svelte');
    n.attachNotices(true);
    const id = n.showNotice({ title: 'Setting up 2 installs · 0 done', hold: true, pinned: true });
    vi.advanceTimersByTime(60_000);
    expect(n.tvNotices.cards).toHaveLength(1);
    n.updateNotice(id, { title: '2 installs done' });
    n.releaseNotice(id, 10_000);
    expect(n.tvNotices.cards[0]).toMatchObject({ title: '2 installs done', pinned: true, countdown: 10_000 });
    // Later cards go under it and never push it out during the countdown.
    for (const t of ['a', 'b', 'c', 'd']) n.showNotice({ title: t });
    expect(n.tvNotices.cards[0]).toMatchObject({ id, pinned: true });
    expect(n.tvNotices.cards).toHaveLength(3);
    vi.advanceTimersByTime(10_000);
    expect(n.tvNotices.cards.some((c) => c.id === id)).toBe(false);
    vi.advanceTimersByTime(10_000);
    expect(n.tvNotices.cards).toHaveLength(0);
    n.clearHistory();
    expect(n.tvNotices.history).toHaveLength(0);
  });

  it('while a video plays cards wait with no countdown, then show with their full life', async () => {
    const n = await import('./notices.svelte');
    n.attachNotices(true);
    n.showNotice({ title: 'before' });
    vi.advanceTimersByTime(4_000);
    n.holdNotices(true);
    expect(n.tvNotices.held).toBe(true);
    n.showNotice({ title: 'during' });
    expect(n.viewNewest()).toBe(false);
    vi.advanceTimersByTime(60_000);
    expect(n.tvNotices.cards.map((c) => c.title)).toEqual(['during', 'before']);
    expect(n.tvNotices.history.map((h) => h.title)).toEqual(['during', 'before']);
    n.holdNotices(false);
    vi.advanceTimersByTime(9_000);
    expect(n.tvNotices.cards).toHaveLength(2);
    vi.advanceTimersByTime(1_000);
    expect(n.tvNotices.cards).toHaveLength(0);
  });

  it('detaching clears the cards but keeps the history', async () => {
    const n = await import('./notices.svelte');
    n.attachNotices(true);
    n.showNotice({ title: 'a' });
    n.attachNotices(false);
    expect(n.tvNotices.cards).toHaveLength(0);
    expect(n.tvNotices.history).toHaveLength(1);
  });
});
