import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { TvButton } from './buttons';
import { Causes, Repeater, type RepeatKind } from './repeat';

let fired: [TvButton, number][];
const kindOf = (b: TvButton): RepeatKind => (['UP', 'DOWN', 'LEFT', 'RIGHT'].includes(b) ? 'dir' : null);

beforeEach(() => {
  vi.useFakeTimers();
  fired = [];
});
afterEach(() => vi.useRealTimers());

const make = (kind = kindOf) => new Repeater<number>((b, s) => fired.push([b, s]), kind);

describe('Repeater', () => {
  it('a direction fires on press, after 400 ms, then every 90 ms (item 40)', () => {
    const r = make();
    r.down(1, 'DOWN');
    expect(fired.length).toBe(1);
    vi.advanceTimersByTime(399);
    expect(fired.length).toBe(1);
    vi.advanceTimersByTime(1);
    expect(fired.length).toBe(2);
    vi.advanceTimersByTime(90);
    expect(fired.length).toBe(3);
    vi.advanceTimersByTime(900);
    expect(fired.length).toBe(13);
  });

  it('other buttons fire once (item 41)', () => {
    const r = make();
    r.down(1, 'A');
    vi.advanceTimersByTime(5000);
    expect(fired).toEqual([['A', 1]]);
  });

  it('releasing ends the repeat at once (item 44)', () => {
    const r = make();
    r.down(1, 'UP');
    vi.advanceTimersByTime(500);
    const n = fired.length;
    r.up(1, 'UP');
    vi.advanceTimersByTime(1000);
    expect(fired.length).toBe(n);
  });

  it('a held button is not re-fired by a second down', () => {
    const r = make();
    r.down(1, 'A');
    r.down(1, 'A');
    expect(fired.length).toBe(1);
  });

  it('each source keeps its own held state (item 45)', () => {
    const r = make();
    r.down(1, 'RIGHT');
    vi.advanceTimersByTime(100);
    // Pad 2 presses and releases the same button: pad 1 keeps repeating.
    r.down(2, 'RIGHT');
    r.up(2, 'RIGHT');
    vi.advanceTimersByTime(300);
    expect(fired.filter(([, s]) => s === 1).length).toBe(2);
    expect(fired.filter(([, s]) => s === 2).length).toBe(1);
    expect(r.isHeld(1, 'RIGHT')).toBe(true);
  });

  it('releaseAll (the gate closing) stops everything', () => {
    const r = make();
    r.down(1, 'UP');
    r.down(2, 'LEFT');
    r.releaseAll();
    vi.advanceTimersByTime(2000);
    expect(fired.length).toBe(2);
    expect(r.isHeld(1, 'UP')).toBe(false);
  });

  it('releaseSource (a disconnect) stops only that source', () => {
    const r = make();
    r.down(1, 'UP');
    r.down(2, 'UP');
    r.releaseSource(1);
    vi.advanceTimersByTime(400);
    expect(fired.filter(([, s]) => s === 1).length).toBe(1);
    expect(fired.filter(([, s]) => s === 2).length).toBe(2);
  });

  it('typing buttons repeat after 500 ms, then every 50 ms', () => {
    const r = make((b) => (b === 'X' ? 'typing' : kindOf(b)));
    r.down(1, 'X');
    vi.advanceTimersByTime(499);
    expect(fired.length).toBe(1);
    vi.advanceTimersByTime(1);
    expect(fired.length).toBe(2);
    vi.advanceTimersByTime(100);
    expect(fired.length).toBe(4);
  });

  it('a button that stops repeating while held fires no more', () => {
    let typing = true;
    const r = make((b) => (b === 'A' && typing ? 'typing' : null));
    r.down(1, 'A');
    typing = false;
    vi.advanceTimersByTime(2000);
    expect(fired.length).toBe(1);
  });
});

describe('Causes', () => {
  it('a button is down while any cause holds it', () => {
    const c = new Causes();
    expect(c.add('UP', 'dpad_up')).toBe(true);
    expect(c.add('UP', 'ls:UP')).toBe(false);
    expect(c.remove('UP', 'dpad_up')).toBe(false);
    expect(c.remove('UP', 'ls:UP')).toBe(true);
  });

  it('knows which button a cause holds', () => {
    const c = new Causes();
    c.add('A', 'east');
    expect(c.heldBy('east')).toBe('A');
    expect(c.heldBy('south')).toBeNull();
  });
});
