// TV mode's notification stack and the session's notice history (TV mode
// plan P6; checklist items 139–150, 356–359). The desktop's events reach it
// through `onToastPushed` (stores/toasts.svelte.ts), so TV shows the same
// cards with the same wording, with its own limit and place. TvToasts draws
// the stack, TvNotifications the history.
//
// The history records from app start, whichever mode is up: it is the
// session's (ruling 16). Cards show only while a TV shell is attached.
import { onToastPushed } from '../stores/toasts.svelte';
import {
  addCard,
  addHistory,
  cardOf,
  lifeOf,
  newestAction,
  removeCard,
  type Card,
  type HistoryEntry,
  type NoticeInput,
  type NoticeLine,
  type NoticeSeg,
} from './notices';

const state = $state<{ cards: Card[]; history: HistoryEntry[] }>({ cards: [], history: [] });
const timers = new Map<number, ReturnType<typeof setTimeout>>();
let attached = false;
let nextId = 1;

export const tvNotices = {
  /** The cards on show, top first. */
  get cards(): readonly Card[] {
    return state.cards;
  },
  /** The session's notices, newest first. */
  get history(): readonly HistoryEntry[] {
    return state.history;
  },
};

function stop(id: number): void {
  const t = timers.get(id);
  if (t !== undefined) clearTimeout(t);
  timers.delete(id);
}

function arm(card: Card): void {
  if (card.countdown === null) return;
  stop(card.id);
  timers.set(
    card.id,
    setTimeout(() => closeNotice(card.id), card.countdown),
  );
}

/**
 * Shows a notice: a card (while TV mode is up) and a history entry.
 * Answers the card's id, for `updateNotice`, `releaseNotice` and
 * `closeNotice`.
 */
export function showNotice(n: NoticeInput): number {
  const id = nextId++;
  state.history = addHistory(state.history, { id, title: n.title, sub: n.sub === undefined || n.sub === '' ? null : n.sub, error: !!n.error, at: new Date() });
  if (!attached) return id;
  const card = cardOf(id, n);
  const { stack, dropped } = addCard(state.cards, card);
  for (const d of dropped) stop(d.id);
  state.cards = stack;
  arm(card);
  return id;
}

/** Changes a card on show (the setup card's title, bar and step). */
export function updateNotice(id: number, patch: { title?: string; sub?: NoticeLine | null; segs?: NoticeSeg[] | null; step?: string | null }): void {
  state.cards = state.cards.map((c) => (c.id === id ? { ...c, ...patch } : c));
}

/** Starts a held card's countdown (`ms`, default its life) and unpins it. */
export function releaseNotice(id: number, ms?: number): void {
  const c = state.cards.find((x) => x.id === id);
  if (!c) return;
  const next: Card = { ...c, pinned: false, countdown: ms ?? lifeOf({ error: c.error }) };
  state.cards = state.cards.map((x) => (x.id === id ? next : x));
  arm(next);
}

export function closeNotice(id: number): void {
  stop(id);
  state.cards = removeCard(state.cards, id);
}

/** View: the newest card with an action closes and its action runs
 *  (item 148). Answers whether there was one. */
export function viewNewest(): boolean {
  const c = newestAction(state.cards);
  if (!c?.action) return false;
  closeNotice(c.id);
  c.action.run();
  return true;
}

/** Menu › Notifications › Clear all (item 359). */
export function clearHistory(): void {
  state.history = [];
}

/** A TV shell shows the cards while attached; detaching clears them. */
export function attachNotices(on: boolean): void {
  attached = on;
  if (!on) {
    for (const id of [...timers.keys()]) stop(id);
    state.cards = [];
  }
}

// Every card the app pushes, from app start (the history is the session's).
onToastPushed((t) =>
  showNotice({
    title: t.text,
    sub: t.sub,
    error: t.level === 'error',
    action: t.action,
    ms: t.durationMs,
  }),
);
