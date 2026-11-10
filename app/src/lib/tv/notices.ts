// TV mode's notification cards (checklist items 139–150, 356–359; tvmode.js
// `toast`, `newestAction`, `notesPage`). Pure: `notices.svelte.ts` keeps the
// stack in `$state`, runs the timers and feeds it the desktop's events.
//
// A card goes on top of the stack, under the pinned setup card when one
// shows. At most three show: a fourth drops the oldest card that isn't
// pinned. A card closes on its own after 10 s, or 15 s for an error, unless
// it is held (the setup card holds still until its installs are done, then
// counts down, still pinned, so it leaves only when it closes itself). Every card also goes to the session's history (Menu ›
// Notifications), which keeps the whole session (ruling 16).
import type { GlyphButton } from './glyphs/glyphs';

export const MAX_CARDS = 3;
export const NOTICE_MS = 10_000;
export const ERROR_NOTICE_MS = 15_000;

/** A card's second line: text, with button prompts drawn inline (the
 *  welcome card's "Press (Menu) for the menu."). */
export type NoticePart = string | { glyph: GlyphButton };
export type NoticeLine = string | readonly NoticePart[];

/** One segment of the setup card's bar (P12): its fill 0–100, done, failed. */
export type NoticeSeg = { pct: number; state: 'wait' | 'run' | 'done' | 'fail' };

export type NoticeInput = {
  title: string;
  sub?: NoticeLine;
  error?: boolean;
  /** View runs it; the card shows the View prompt and its label. */
  action?: { label: string; run: () => void };
  /** Its life; default 10 s, 15 s for an error. */
  ms?: number;
  /** No countdown until `release` (the setup card while it installs). */
  hold?: boolean;
  /** Stays on top and is never the card dropped (the setup card). */
  pinned?: boolean;
  segs?: NoticeSeg[];
  /** A second line under the bar (the setup card's step). */
  step?: string;
};

export type Card = {
  id: number;
  title: string;
  sub: NoticeLine | null;
  error: boolean;
  action: { label: string; run: () => void } | null;
  pinned: boolean;
  segs: NoticeSeg[] | null;
  step: string | null;
  /** The countdown's length once it runs, or null while held. */
  countdown: number | null;
};

export type HistoryEntry = { id: number; title: string; sub: NoticeLine | null; error: boolean; at: Date };

/** A card's life: errors stay 15 s, others 10 s; a longer life asked for
 *  (a desktop notice that needs reading) is kept. */
export function lifeOf(n: Pick<NoticeInput, 'error' | 'ms'>): number {
  const base = n.error ? ERROR_NOTICE_MS : NOTICE_MS;
  return n.ms !== undefined && n.ms > base ? n.ms : base;
}

export function cardOf(id: number, n: NoticeInput): Card {
  return {
    id,
    title: n.title,
    sub: n.sub === undefined || n.sub === '' ? null : n.sub,
    error: !!n.error,
    action: n.action ?? null,
    pinned: !!n.pinned,
    segs: n.segs ?? null,
    step: n.step ?? null,
    countdown: n.hold ? null : lifeOf(n),
  };
}

/**
 * The stack after `card` arrives, top first, and the cards it dropped. The
 * new card goes on top, under the last pinned card; while three show, the
 * oldest unpinned card (the lowest) makes room.
 */
export function addCard(stack: readonly Card[], card: Card, max = MAX_CARDS): { stack: Card[]; dropped: Card[] } {
  const next = [...stack];
  const dropped: Card[] = [];
  while (next.length >= max) {
    let i = -1;
    for (let j = next.length - 1; j >= 0; j--) {
      if (!next[j].pinned) {
        i = j;
        break;
      }
    }
    if (i < 0) break;
    dropped.push(...next.splice(i, 1));
  }
  let at = 0;
  for (let j = 0; j < next.length; j++) if (next[j].pinned) at = j + 1;
  next.splice(at, 0, card);
  return { stack: next, dropped };
}

export function removeCard(stack: readonly Card[], id: number): Card[] {
  return stack.filter((c) => c.id !== id);
}

/** The newest card that offers an action (the highest one), or null:
 *  what View acts on (item 148). */
export function newestAction(stack: readonly Card[]): Card | null {
  return stack.find((c) => c.action !== null) ?? null;
}

/** The history after a notice, newest first, never capped (ruling 16). */
export function addHistory(history: readonly HistoryEntry[], e: HistoryEntry): HistoryEntry[] {
  return [e, ...history];
}

/** A line as plain text, with each prompt named (`label(button)`). */
export function lineText(line: NoticeLine | null, label: (b: GlyphButton) => string): string {
  if (line === null) return '';
  if (typeof line === 'string') return line;
  return line.map((p) => (typeof p === 'string' ? p : label(p.glyph))).join('');
}

/** "HH:MM" for a history row. */
export function timeOf(d: Date): string {
  return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`;
}

/** The welcome card shown each time TV mode opens, once the main screen is
 *  up (ruling 15; tvmode.js start). */
export const WELCOME: NoticeInput = {
  title: 'Welcome to TV mode',
  sub: ['Press ', { glyph: 'MENU' }, ' for the menu. Desktop mode is one press away there.'],
};
