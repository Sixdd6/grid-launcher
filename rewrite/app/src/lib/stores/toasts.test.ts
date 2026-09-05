import { describe, expect, it } from 'vitest';
import { appendToast, removeToast, TOAST_LIMIT, type Toast } from './toasts.svelte';

const toast = (id: number, text: string): Toast => ({ id, text, level: 'success' });

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
