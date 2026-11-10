import { describe, expect, it } from 'vitest';
import { inPlace, move, restore, type Container, type Item, type Rect, type Snapshot } from './engine';

// A little layout builder: items are boxes; containers are named groups.
const R = (left: number, top: number, w: number, h: number): Rect => ({ left, top, right: left + w, bottom: top + h });

type ItemSpec = { key: string; rect: Rect; in?: string[]; disabled?: boolean; autofocus?: boolean; rail?: boolean; sel?: boolean; links?: Item['links'] };
type ContSpec = { name: string; rect: Rect; nav?: Container['nav']; enter?: Container['enter']; edge?: boolean; links?: Container['links']; last?: string };

function build(conts: ContSpec[], specs: ItemSpec[], current: string | null): Snapshot & { k: (key: string) => number } {
  const cIndex = new Map(conts.map((c, i) => [c.name, i]));
  const items: Item[] = specs.map((s) => ({
    key: s.key,
    rect: s.rect,
    disabled: !!s.disabled,
    autofocus: !!s.autofocus,
    links: s.links ?? {},
    // innermost first: the order given in `in`
    chain: (s.in ?? []).map((n) => cIndex.get(n)!),
    railOpen: !!s.rail,
  }));
  const k = (key: string) => specs.findIndex((s) => s.key === key);
  const containers: Container[] = conts.map((c, ci) => {
    const members = items.map((it, i) => (it.chain.includes(ci) ? i : -1)).filter((i) => i >= 0);
    return {
      rect: c.rect,
      nav: c.nav ?? null,
      enter: c.enter ?? null,
      edge: !!c.edge,
      links: c.links ?? {},
      last: c.last ? k(c.last) : null,
      selected: members.find((i) => specs[i].sel) ?? null,
      items: members,
    };
  });
  return { items, containers, current: current === null ? null : k(current), k };
}

const keyOf = (snap: Snapshot, r: ReturnType<typeof move>) => (r.kind === 'focus' ? snap.items[r.item].key : r.kind);

/** A Library-like screen: a rail (3 rows, the second selected) beside a 3×2 grid. */
function railAndGrid(current: string, gridLast?: string) {
  return build(
    [
      { name: 'rail', rect: R(0, 0, 100, 400), nav: 'v', enter: 'sel' },
      { name: 'grid', rect: R(120, 0, 300, 400), nav: 'grid', enter: 'first', last: gridLast },
    ],
    [
      { key: 'r0', rect: R(0, 0, 100, 40), in: ['rail'], rail: true },
      { key: 'r1', rect: R(0, 50, 100, 40), in: ['rail'], rail: true, sel: true },
      { key: 'r2', rect: R(0, 100, 100, 40), in: ['rail'], rail: true },
      { key: 'g0', rect: R(130, 10, 80, 100), in: ['grid'] },
      { key: 'g1', rect: R(230, 10, 80, 100), in: ['grid'] },
      { key: 'g2', rect: R(330, 10, 80, 100), in: ['grid'] },
      { key: 'g3', rect: R(130, 130, 80, 100), in: ['grid'] },
      { key: 'g4', rect: R(230, 130, 80, 100), in: ['grid'] },
    ],
    current,
  );
}

describe('move', () => {
  it('restores when nothing is focused', () => {
    expect(move(build([], [{ key: 'a', rect: R(0, 0, 10, 10) }], null), 'down')).toEqual({ kind: 'restore' });
  });

  it('picks by gap along plus 2.2 × offset across (item 81)', () => {
    // From `a`, Right: `near` is 50 away along, 40 across (50 + 88 = 138);
    // `far` is 100 along, 0 across (100). `far` wins.
    const snap = build(
      [],
      [
        { key: 'a', rect: R(0, 0, 20, 20) },
        { key: 'near', rect: R(70, 40, 20, 20) },
        { key: 'far', rect: R(120, 0, 20, 20) },
      ],
      'a',
    );
    expect(keyOf(snap, move(snap, 'right'))).toBe('far');
  });

  it('only takes items lying in the pressed direction', () => {
    const snap = build([], [
      { key: 'a', rect: R(100, 100, 20, 20) },
      { key: 'above', rect: R(100, 0, 20, 20) },
    ], 'a');
    expect(move(snap, 'down')).toEqual({ kind: 'none' });
    expect(keyOf(snap, move(snap, 'up'))).toBe('above');
  });

  it('keeps focus inside a list along its axis, with no wrapping (item 82)', () => {
    const snap = build(
      [{ name: 'list', rect: R(0, 0, 100, 100), nav: 'v' }],
      [
        { key: 'top', rect: R(0, 0, 100, 40), in: ['list'] },
        { key: 'bottom', rect: R(0, 50, 100, 40), in: ['list'] },
      ],
      'top',
    );
    expect(move(snap, 'up')).toEqual({ kind: 'none' });
    expect(keyOf(snap, move(snap, 'down'))).toBe('bottom');
  });

  it('carries on past a list end to the buttons under it, in its column, first in reading order (items 83–84)', () => {
    const snap = build(
      [
        { name: 'list', rect: R(0, 0, 200, 100), nav: 'v' },
        { name: 'btns', rect: R(0, 120, 300, 40), nav: 'h' },
      ],
      [
        { key: 'row0', rect: R(0, 0, 200, 40), in: ['list'] },
        { key: 'row1', rect: R(0, 50, 200, 40), in: ['list'] },
        // A neighbouring panel to the right, nearer than the buttons: never taken.
        { key: 'side', rect: R(400, 60, 100, 40) },
        { key: 'b1', rect: R(120, 120, 80, 40), in: ['btns'] },
        { key: 'b0', rect: R(10, 120, 80, 40), in: ['btns'] },
      ],
      'row1',
    );
    expect(keyOf(snap, move(snap, 'down'))).toBe('b0');
  });

  it('back into a button group lands on the button left there (item 87)', () => {
    const snap = build(
      [
        { name: 'list', rect: R(0, 200, 200, 100), nav: 'v' },
        { name: 'btns', rect: R(0, 120, 300, 40), nav: 'h', last: 'b1' },
      ],
      [
        { key: 'b0', rect: R(10, 120, 80, 40), in: ['btns'] },
        { key: 'b1', rect: R(120, 120, 80, 40), in: ['btns'] },
        { key: 'row0', rect: R(0, 200, 200, 40), in: ['list'] },
      ],
      'row0',
    );
    expect(keyOf(snap, move(snap, 'up'))).toBe('b1');
  });

  it('Right from a rail row is A (item 85)', () => {
    const snap = railAndGrid('r1');
    expect(move(snap, 'right')).toEqual({ kind: 'railOpen' });
  });

  it('moving into a rail lands on its selected row (item 86)', () => {
    // From g3 (left column, second row) Left: geometry says r2, the rail says r1.
    const snap = railAndGrid('g3');
    expect(keyOf(snap, move(snap, 'left'))).toBe('r1');
  });

  it('a rail with no selected row is entered where it was left, else geometrically', () => {
    const snap = build(
      [
        { name: 'rail', rect: R(0, 0, 100, 400), nav: 'v', enter: 'sel' },
        { name: 'grid', rect: R(120, 0, 300, 400), nav: 'grid' },
      ],
      [
        { key: 'r0', rect: R(0, 0, 100, 40), in: ['rail'] },
        { key: 'g0', rect: R(130, 10, 80, 100), in: ['grid'] },
      ],
      'g0',
    );
    expect(keyOf(snap, move(snap, 'left'))).toBe('r0');
  });

  it('off a grid edge only goes straight past it; nothing there stays put (items 88–89)', () => {
    // Up from the top row: the rail is up-left, never taken.
    const snap = railAndGrid('g1');
    expect(move(snap, 'up')).toEqual({ kind: 'none' });
    // Down from the last row: nothing below.
    const snap2 = railAndGrid('g4');
    expect(move(snap2, 'down')).toEqual({ kind: 'none' });
  });

  it('Up from a grid reaches header chips straight above it (item 89)', () => {
    const snap = build(
      [{ name: 'grid', rect: R(120, 100, 300, 300), nav: 'grid' }],
      [
        { key: 'chip', rect: R(300, 20, 80, 30) },
        { key: 'rail', rect: R(0, 20, 100, 30) },
        { key: 'g0', rect: R(130, 110, 80, 100), in: ['grid'] },
      ],
      'g0',
    );
    expect(keyOf(snap, move(snap, 'up'))).toBe('chip');
  });

  it('inside a grid, moves between covers', () => {
    const snap = railAndGrid('g0');
    expect(keyOf(snap, move(snap, 'right'))).toBe('g1');
    expect(keyOf(snap, move(snap, 'down'))).toBe('g3');
  });

  it('a grid entered from outside lands where it was left, else on its first cover', () => {
    const fresh = railAndGrid('r1');
    // Spatially Right from r1 is railOpen; check `enter=first` from a free item instead.
    const snap = build(
      [{ name: 'grid', rect: R(120, 0, 300, 400), nav: 'grid', enter: 'first', last: 'g1' }],
      [
        { key: 'free', rect: R(0, 200, 50, 40) },
        { key: 'g0', rect: R(130, 10, 80, 100), in: ['grid'] },
        { key: 'g1', rect: R(230, 10, 80, 100), in: ['grid'] },
        { key: 'g3', rect: R(130, 180, 80, 100), in: ['grid'] },
      ],
      'free',
    );
    expect(keyOf(snap, move(snap, 'right'))).toBe('g1');
    snap.containers[0].last = null;
    expect(keyOf(snap, move(snap, 'right'))).toBe('g0');
    void fresh;
  });

  it('an explicit link wins over geometry (item 91)', () => {
    const snap = build(
      [{ name: 'related', rect: R(0, 300, 400, 100), nav: 'h', links: { up: 'prose' } }],
      [
        { key: 'prose', rect: R(0, 0, 300, 200) },
        { key: 'facts', rect: R(320, 200, 80, 80) },
        { key: 'rel0', rect: R(330, 310, 60, 80), in: ['related'] },
      ],
      'rel0',
    );
    expect(keyOf(snap, move(snap, 'up'))).toBe('prose');
  });

  it("an area of rows (data-edge) is left only straight past its top or bottom, never into the rail (item 174)", () => {
    const snap = build(
      [
        { name: 'rail', rect: R(0, 0, 100, 600), nav: 'v', enter: 'sel' },
        { name: 'rows', rect: R(120, 200, 600, 400), edge: true },
        { name: 'row0', rect: R(120, 200, 600, 150), nav: 'h' },
        { name: 'row1', rect: R(120, 400, 600, 150), nav: 'h' },
      ],
      [
        { key: 'disc', rect: R(0, 0, 100, 40), in: ['rail'], sel: true },
        { key: 'plat', rect: R(0, 50, 100, 40), in: ['rail'] },
        { key: 'a0', rect: R(130, 220, 80, 100), in: ['row0', 'rows'] },
        { key: 'a1', rect: R(230, 220, 80, 100), in: ['row0', 'rows'] },
        { key: 'b0', rect: R(130, 420, 80, 100), in: ['row1', 'rows'] },
      ],
      'a0',
    );
    expect(move(snap, 'up')).toEqual({ kind: 'none' });
    expect(keyOf(snap, move(snap, 'down'))).toBe('b0');
    snap.current = snap.k('b0');
    expect(move(snap, 'down')).toEqual({ kind: 'none' });
    // Left from a row's first cover: the rail's selected row.
    snap.current = snap.k('b0');
    expect(keyOf(snap, move(snap, 'left'))).toBe('disc');
  });
});

describe('restore', () => {
  const items = [
    { key: 'a', disabled: true, autofocus: false },
    { key: 'b', disabled: false, autofocus: false },
    { key: 'c', disabled: false, autofocus: true },
  ];

  it('opens on the remembered item', () => {
    expect(restore(items, 'b')).toBe(1);
  });

  it('else on the designated default', () => {
    expect(restore(items, 'gone')).toBe(2);
  });

  it('else on the first item, skipping a disabled one when an enabled one exists (item 78)', () => {
    const plain = items.map((i) => ({ ...i, autofocus: false }));
    expect(restore(plain, null)).toBe(1);
    expect(restore([{ key: 'x', disabled: true, autofocus: false }], null)).toBe(0);
    expect(restore([], null)).toBeNull();
  });

  it('a remembered item that is disabled gives way too', () => {
    expect(restore(items, 'a')).toBe(1);
  });
});

describe('inPlace', () => {
  it('takes the item now at the same position, clamped to the end (item 115)', () => {
    expect(inPlace([10, 11, 12], 1)).toBe(11);
    expect(inPlace([10, 11], 5)).toBe(11);
    expect(inPlace([], 0)).toBeNull();
  });
});
