import { describe, expect, it } from 'vitest';
import {
  addCard,
  addHistory,
  cardOf,
  ERROR_NOTICE_MS,
  lifeOf,
  lineText,
  MAX_CARDS,
  newestAction,
  NOTICE_MS,
  removeCard,
  timeOf,
  WELCOME,
  type Card,
  type HistoryEntry,
} from './notices';

const card = (id: number, o: Partial<Parameters<typeof cardOf>[1]> = {}): Card => cardOf(id, { title: `n${id}`, ...o });
const ids = (s: Card[]) => s.map((c) => c.id);

describe('the stack (items 141–142)', () => {
  it('puts the newest card on top', () => {
    let s: Card[] = [];
    for (const id of [1, 2, 3]) s = addCard(s, card(id)).stack;
    expect(ids(s)).toEqual([3, 2, 1]);
  });

  it('shows at most three: a fourth drops the oldest', () => {
    let s: Card[] = [];
    for (const id of [1, 2, 3]) s = addCard(s, card(id)).stack;
    const r = addCard(s, card(4));
    expect(MAX_CARDS).toBe(3);
    expect(ids(r.stack)).toEqual([4, 3, 2]);
    expect(ids(r.dropped)).toEqual([1]);
  });

  it('keeps the pinned setup card on top and never drops it', () => {
    let s: Card[] = addCard([], card(1, { pinned: true, hold: true })).stack;
    for (const id of [2, 3, 4, 5]) s = addCard(s, card(id)).stack;
    expect(ids(s)).toEqual([1, 5, 4]);
    expect(s[0].countdown).toBeNull();
  });

  it('removes a card', () => {
    const s = addCard(addCard([], card(1)).stack, card(2)).stack;
    expect(ids(removeCard(s, 1))).toEqual([2]);
  });
});

describe('a card (items 144–146)', () => {
  it('lives 10 s, 15 s for an error, and keeps a longer life asked for', () => {
    expect(NOTICE_MS).toBe(10_000);
    expect(ERROR_NOTICE_MS).toBe(15_000);
    expect(card(1).countdown).toBe(10_000);
    expect(card(1, { error: true }).countdown).toBe(15_000);
    expect(card(1, { error: true, ms: 10_000 }).countdown).toBe(15_000);
    expect(lifeOf({ ms: 20_000 })).toBe(20_000);
  });

  it('has no countdown while held', () => {
    expect(card(1, { hold: true }).countdown).toBeNull();
  });

  it('drops an empty second line', () => {
    expect(card(1, { sub: '' }).sub).toBeNull();
    expect(card(1, { sub: 'Used for PlayStation' }).sub).toBe('Used for PlayStation');
  });
});

describe('View (item 148)', () => {
  it('acts on the newest card that offers an action', () => {
    const run = () => {};
    let s: Card[] = [];
    s = addCard(s, card(1, { action: { label: 'Open Downloads', run } })).stack;
    s = addCard(s, card(2, { action: { label: 'Open Downloads', run } })).stack;
    s = addCard(s, card(3)).stack;
    expect(newestAction(s)?.id).toBe(2);
    expect(newestAction([card(4)])).toBeNull();
  });
});

describe('history (items 150, 356–357; ruling 16)', () => {
  it('keeps every notice of the session, newest first, with no cap', () => {
    let h: HistoryEntry[] = [];
    for (let i = 0; i < 50; i++) h = addHistory(h, { id: i, title: `n${i}`, sub: null, error: false, at: new Date(2026, 10, 10, 9, 5) });
    expect(h).toHaveLength(50);
    expect(h[0].id).toBe(49);
  });

  it('times a row as HH:MM', () => {
    expect(timeOf(new Date(2026, 10, 10, 9, 5))).toBe('09:05');
  });

  it('words a line with prompts by their names', () => {
    expect(lineText(WELCOME.sub ?? null, (b) => (b === 'MENU' ? 'Menu' : b))).toBe('Press Menu for the menu. Desktop mode is one press away there.');
    expect(lineText(null, () => '')).toBe('');
  });

  it('the welcome card (ruling 15)', () => {
    expect(WELCOME.title).toBe('Welcome to TV mode');
  });
});
