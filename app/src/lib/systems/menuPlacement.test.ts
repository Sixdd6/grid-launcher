import { describe, expect, it } from 'vitest';
import { MENU_EDGE, MENU_GAP, placeMenu, type Rect } from './menuPlacement';

const win: Rect = { top: 0, left: 0, right: 1000, bottom: 700 };
const menu = { width: 200, height: 120 };
const button = (left: number, top: number): Rect => ({ left, top, right: left + 28, bottom: top + 28 });

const overlaps = (a: Rect, b: Rect) => a.left < b.right && b.left < a.right && a.top < b.bottom && b.top < a.bottom;
const rectOf = (p: { top: number; left: number }): Rect => ({
  top: p.top,
  left: p.left,
  right: p.left + menu.width,
  bottom: p.top + menu.height,
});

describe('placeMenu', () => {
  it('opens below the button, right-aligned, when it fits', () => {
    const b = button(800, 100);
    const p = placeMenu(b, menu, win);
    expect(p).toEqual({ side: 'below', top: b.bottom + MENU_GAP, left: b.right - menu.width });
  });

  it('stays inside the window when right-aligning would cross its left edge', () => {
    const p = placeMenu(button(20, 100), menu, win);
    expect(p.side).toBe('below');
    expect(p.left).toBe(MENU_EDGE);
  });

  it('opens to the left near the bottom, shifted up to stay 8px inside', () => {
    const b = button(800, 650);
    const p = placeMenu(b, menu, win);
    expect(p.side).toBe('left');
    expect(p.left).toBe(b.left - MENU_GAP - menu.width);
    expect(p.top + menu.height).toBe(win.bottom - MENU_EDGE);
    expect(overlaps(rectOf(p), b)).toBe(false);
  });

  it('opens to the right when there is no room on the left', () => {
    const b = button(100, 650);
    const p = placeMenu(b, menu, win);
    expect(p.side).toBe('right');
    expect(p.left).toBe(b.right + MENU_GAP);
    expect(overlaps(rectOf(p), b)).toBe(false);
  });

  it('keeps level with the button when the menu fits below its top', () => {
    const b = button(800, 560);
    const p = placeMenu(b, menu, win);
    expect(p.side).toBe('left');
    expect(p.top).toBe(b.top);
  });

  it('never goes above the window top in a short window', () => {
    const p = placeMenu(button(800, 40), { width: 200, height: 400 }, { top: 0, left: 0, right: 1000, bottom: 300 });
    expect(p.top).toBe(MENU_EDGE);
    expect(p.side).toBe('left');
  });
});
