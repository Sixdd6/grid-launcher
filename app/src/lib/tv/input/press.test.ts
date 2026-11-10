import { describe, expect, it } from 'vitest';
import { press, stepTab, type PressContext } from './press';

const ctx = (o: Partial<PressContext> = {}): PressContext => ({
  layer: null,
  pages: 0,
  startup: false,
  selMode: false,
  focus: { adjust: false, read: false, disabled: false, inRail: false, cover: false },
  hasRail: true,
  screen: { x: false, y: false, lt: false, rt: false, lb: false, rb: false, main: null },
  ...o,
});
const focus = (o: Partial<NonNullable<PressContext['focus']>>) => ({ adjust: false, read: false, disabled: false, inRail: false, cover: false, ...o });

describe('directions', () => {
  it('move', () => {
    expect(press('UP', ctx())).toEqual({ kind: 'move', dir: 'up' });
  });
  it('Left / Right adjust a focused slider', () => {
    expect(press('LEFT', ctx({ focus: focus({ adjust: true }) }))).toEqual({ kind: 'adjust', delta: -1 });
    expect(press('RIGHT', ctx({ focus: focus({ adjust: true }) }))).toEqual({ kind: 'adjust', delta: 1 });
    expect(press('UP', ctx({ focus: focus({ adjust: true }) }))).toEqual({ kind: 'move', dir: 'up' });
  });
  it('Up / Down read through a summary first', () => {
    expect(press('DOWN', ctx({ focus: focus({ read: true }) }))).toEqual({ kind: 'read', dir: 'down' });
  });
});

describe('A', () => {
  it('activates, or says why on a disabled control', () => {
    expect(press('A', ctx())).toEqual({ kind: 'activate' });
    expect(press('A', ctx({ focus: focus({ disabled: true }) }))).toEqual({ kind: 'why' });
    expect(press('A', ctx({ focus: null }))).toEqual({ kind: 'none' });
  });
  it('picks a cover in select mode', () => {
    expect(press('A', ctx({ selMode: true, focus: focus({ cover: true }) }))).toEqual({ kind: 'selPick' });
  });
});

describe('B (items 92–97)', () => {
  it('closes an overlay unless it is blocking', () => {
    expect(press('B', ctx({ layer: { blocking: false, binds: [] } }))).toEqual({ kind: 'closeLayer' });
    expect(press('B', ctx({ layer: { blocking: true, binds: [] } }))).toEqual({ kind: 'none' });
  });
  it('pops a page', () => {
    expect(press('B', ctx({ pages: 1 }))).toEqual({ kind: 'pop' });
  });
  it('on startup only steps out of a page pushed from the first screen', () => {
    expect(press('B', ctx({ startup: true, pages: 1 }))).toEqual({ kind: 'none' });
    expect(press('B', ctx({ startup: true, pages: 2 }))).toEqual({ kind: 'pop' });
  });
  it('leaves select mode first', () => {
    expect(press('B', ctx({ selMode: true }))).toEqual({ kind: 'selToggle' });
  });
  it('from a grid or pane goes to the rail', () => {
    expect(press('B', ctx())).toEqual({ kind: 'toRail' });
  });
  it('does nothing on a rail, or on a screen with no rail', () => {
    expect(press('B', ctx({ focus: focus({ inRail: true }) }))).toEqual({ kind: 'none' });
    expect(press('B', ctx({ hasRail: false }))).toEqual({ kind: 'none' });
  });
});

describe('LB / RB (items 98–99)', () => {
  it('switch tabs at a tab top level', () => {
    expect(press('LB', ctx())).toEqual({ kind: 'tab', step: -1 });
    expect(press('RB', ctx())).toEqual({ kind: 'tab', step: 1 });
  });
  it('do nothing on pages and startup screens, unless the page uses them', () => {
    expect(press('LB', ctx({ pages: 1 }))).toEqual({ kind: 'none' });
    expect(press('RB', ctx({ startup: true }))).toEqual({ kind: 'none' });
    expect(press('RB', ctx({ pages: 1, screen: { ...ctx().screen, rb: true } }))).toEqual({ kind: 'screen', handler: 'rb' });
  });
  it('wrap in the order Library, Server, Downloads, Settings', () => {
    expect(stepTab('library', -1)).toBe('settings');
    expect(stepTab('settings', 1)).toBe('library');
    expect(stepTab('server', 1)).toBe('downloads');
  });
});

describe('Start (items 100–102; ruling 6)', () => {
  it('presses the page main button', () => {
    expect(press('MENU', ctx({ screen: { ...ctx().screen, main: 'press' } }))).toEqual({ kind: 'main' });
  });
  it('says why while it is disabled', () => {
    expect(press('MENU', ctx({ screen: { ...ctx().screen, main: 'why' } }))).toEqual({ kind: 'mainWhy' });
  });
  it('opens the Menu elsewhere, never on startup', () => {
    expect(press('MENU', ctx())).toEqual({ kind: 'menu' });
    expect(press('MENU', ctx({ startup: true }))).toEqual({ kind: 'none' });
  });
});

describe('other buttons', () => {
  it('X is the item quick action, Y / LT / RT the screen own', () => {
    expect(press('X', ctx())).toEqual({ kind: 'x' });
    expect(press('Y', ctx())).toEqual({ kind: 'none' });
    expect(press('Y', ctx({ screen: { ...ctx().screen, y: true } }))).toEqual({ kind: 'screen', handler: 'y' });
    expect(press('RT', ctx({ screen: { ...ctx().screen, rt: true } }))).toEqual({ kind: 'screen', handler: 'rt' });
    expect(press('LT', ctx({ screen: { ...ctx().screen, lt: true } }))).toEqual({ kind: 'screen', handler: 'lt' });
    expect(press('VIEW', ctx())).toEqual({ kind: 'view' });
  });
  it('select mode: X selects all, Y acts', () => {
    expect(press('X', ctx({ selMode: true }))).toEqual({ kind: 'selAll' });
    expect(press('Y', ctx({ selMode: true }))).toEqual({ kind: 'selAct' });
  });
  it('an open overlay suppresses X, Y, LT, RT, LB, RB, Menu and View (item 103)', () => {
    const c = ctx({ layer: { blocking: false, binds: [] }, screen: { x: true, y: true, lt: true, rt: true, lb: true, rb: true, main: 'press' } });
    for (const b of ['X', 'Y', 'LT', 'RT', 'LB', 'RB', 'MENU', 'VIEW'] as const) expect(press(b, c)).toEqual({ kind: 'none' });
  });
  it('an overlay that binds a button gets it first (the keyboard, item 383)', () => {
    const binds = ['X', 'Y', 'LB', 'RB', 'LT', 'MENU'] as const;
    const c = ctx({ layer: { blocking: false, binds }, screen: { x: true, y: true, lt: true, rt: true, lb: true, rb: true, main: 'press' } });
    for (const b of binds) expect(press(b, c)).toEqual({ kind: 'layerButton', button: b });
    expect(press('RT', c)).toEqual({ kind: 'none' });
    expect(press('VIEW', c)).toEqual({ kind: 'none' });
    expect(press('B', c)).toEqual({ kind: 'closeLayer' });
    expect(press('A', c)).toEqual({ kind: 'activate' });
    expect(press('UP', c)).toEqual({ kind: 'move', dir: 'up' });
  });
  it('a bound direction goes to the overlay (the media viewer)', () => {
    expect(press('LEFT', ctx({ layer: { blocking: false, binds: ['LEFT'] } }))).toEqual({ kind: 'layerButton', button: 'LEFT' });
  });
  it('a slider does not adjust under an overlay', () => {
    expect(press('LEFT', ctx({ layer: { blocking: false, binds: [] }, focus: focus({ adjust: true }) }))).toEqual({ kind: 'move', dir: 'left' });
  });
});
