import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  appendToast,
  dismissToast,
  pauseToast,
  pushToast,
  removeToast,
  resumeToast,
  setPaused,
  toasts,
  TOAST_DURATION_MS,
  TOAST_LIMIT,
  type Toast,
} from './toasts.svelte';

const toast = (id: number, text: string): Toast => ({
  id,
  text,
  level: 'success',
  durationMs: TOAST_DURATION_MS,
  paused: false,
});

describe('appendToast', () => {
  it('appends to the end so the newest toast is last', () => {
    const list = appendToast([toast(1, 'first')], toast(2, 'second'));
    expect(list.map((t) => t.text)).toEqual(['first', 'second']);
  });

  it('drops the oldest once the limit is reached', () => {
    let list: Toast[] = [];
    for (let i = 1; i <= TOAST_LIMIT + 2; i += 1) list = appendToast(list, toast(i, `t${i}`));
    expect(list).toHaveLength(TOAST_LIMIT);
    expect(list[0].text).toBe(`t${3}`);
  });

  it('honours an explicit limit', () => {
    const list = appendToast([toast(1, 'a'), toast(2, 'b')], toast(3, 'c'), 2);
    expect(list.map((t) => t.text)).toEqual(['b', 'c']);
  });

  it('ignores a blank message, matching ToastWidget.show_message', () => {
    const before = [toast(1, 'a')];
    expect(appendToast(before, toast(2, '   '))).toBe(before);
  });

  it('does not mutate the input list', () => {
    const before = [toast(1, 'a')];
    appendToast(before, toast(2, 'b'));
    expect(before).toHaveLength(1);
  });
});

describe('removeToast', () => {
  it('removes only the matching id', () => {
    const list = removeToast([toast(1, 'a'), toast(2, 'b')], 1);
    expect(list.map((t) => t.id)).toEqual([2]);
  });

  it('is a no-op for an unknown id', () => {
    const list = removeToast([toast(1, 'a')], 99);
    expect(list.map((t) => t.id)).toEqual([1]);
  });
});

describe('setPaused', () => {
  it('flags only the matching card', () => {
    const list = setPaused([toast(1, 'a'), toast(2, 'b')], 2, true);
    expect(list.map((t) => t.paused)).toEqual([false, true]);
  });

  it('keeps an unchanged card as the same object', () => {
    const before = [toast(1, 'a')];
    expect(setPaused(before, 1, false)[0]).toBe(before[0]);
  });
});

describe('the stack', () => {
  it('holds four cards, and a fifth drops the oldest', () => {
    expect(TOAST_LIMIT).toBe(4);
  });

  it('cards last ten seconds by default', () => {
    expect(TOAST_DURATION_MS).toBe(10_000);
  });
});

describe('pushToast duration', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    for (const t of [...toasts.list]) dismissToast(t.id);
    vi.useRealTimers();
  });

  it('dismisses after TOAST_DURATION_MS by default', () => {
    pushToast('routine');
    vi.advanceTimersByTime(TOAST_DURATION_MS - 1);
    expect(toasts.list.map((t) => t.text)).toEqual(['routine']);
    vi.advanceTimersByTime(1);
    expect(toasts.list).toEqual([]);
  });

  it('honours an explicit longer duration', () => {
    pushToast('one-time upgrade notice', 'success', 20000);
    vi.advanceTimersByTime(TOAST_DURATION_MS);
    expect(toasts.list.map((t) => t.text)).toEqual(['one-time upgrade notice']);
    vi.advanceTimersByTime(20000 - TOAST_DURATION_MS);
    expect(toasts.list).toEqual([]);
  });

  it('records the duration on the card for its countdown bar', () => {
    pushToast('long', 'success', 20000);
    expect(toasts.list[0].durationMs).toBe(20000);
  });

  it('holds the countdown while paused and finishes the rest on resume', () => {
    const id = pushToast('pointed at')!;
    vi.advanceTimersByTime(6_000);
    pauseToast(id);
    expect(toasts.list[0].paused).toBe(true);
    vi.advanceTimersByTime(60_000);
    expect(toasts.list.map((t) => t.text)).toEqual(['pointed at']);
    resumeToast(id);
    expect(toasts.list[0].paused).toBe(false);
    vi.advanceTimersByTime(3_999);
    expect(toasts.list).toHaveLength(1);
    vi.advanceTimersByTime(1);
    expect(toasts.list).toEqual([]);
  });

  it('drops the oldest of five, and the rest still close on time', () => {
    for (let i = 1; i <= 5; i += 1) pushToast(`t${i}`);
    expect(toasts.list.map((t) => t.text)).toEqual(['t2', 't3', 't4', 't5']);
    vi.advanceTimersByTime(TOAST_DURATION_MS);
    expect(toasts.list).toEqual([]);
  });

  it('a card dismissed by hand does not come back or close another', () => {
    const first = pushToast('first')!;
    vi.advanceTimersByTime(5_000);
    dismissToast(first);
    pushToast('second');
    vi.advanceTimersByTime(5_000);
    expect(toasts.list.map((t) => t.text)).toEqual(['second']);
    vi.advanceTimersByTime(5_000);
    expect(toasts.list).toEqual([]);
  });
});

describe('onToastPushed (TV mode sees the same cards)', () => {
  it('hands every non-blank card, with its extras, to subscribers until they unsubscribe', async () => {
    const { onToastPushed, pushToast } = await import('./toasts.svelte');
    const seen: unknown[] = [];
    const off = onToastPushed((t) => seen.push(t));
    const run = () => {};
    pushToast('  Zelda added to download queue ', 'success', 10_000, { action: { label: 'Open Downloads', run } });
    pushToast('   ');
    off();
    pushToast('after');
    expect(seen).toEqual([{ text: 'Zelda added to download queue', level: 'success', durationMs: 10_000, action: { label: 'Open Downloads', run } }]);
  });
});
