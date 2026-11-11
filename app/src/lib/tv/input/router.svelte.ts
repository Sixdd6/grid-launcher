// Every input TV mode reads, turned into presses of logical buttons
// (checklist items 34–47, 71–74): the native `gamepad` event (P4) and the
// keyboard. TV mode is keyboard and controller only (README § Decided,
// 2026-11-11): the mouse presses, points at and scrolls nothing, and its
// cursor shows over the window only while it moves. Also the state the
// prompts, the hint bar's name and the Button assignments row follow: the
// controller pressed last (item 47), and whether a key was pressed after it.
// Started by TvShell on mount, stopped on unmount.
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { api, GAMEPAD_EVENT, type PadEvent, type PadInfo } from '../../api';
import { uiSettings } from '../../stores/uiSettings.svelte';
import type { PromptSet } from '../glyphs/glyphs';
import { reducedMotion, remPx, vScroller } from '../focus/dom';
import { focused, handlePress, nav, scope, topLayer } from '../nav.svelte';
import { buttonForKey, isDirection, type TvButton } from './buttons';
import { assignmentFor, guessAssign, logicalButton, padBrand, padDisplayName } from './controllers';
import { promptSet } from './prompts';
import { Causes, Repeater, type RepeatKind } from './repeat';
import { leftStickDirections, pageScroll, rightStickActive, rightStickScroll } from './sticks';

const state = $state<{ pads: PadInfo[]; currentId: number | null; keyboard: boolean }>({ pads: [], currentId: null, keyboard: false });

function currentPad(): PadInfo | null {
  return state.pads.find((p) => p.id === state.currentId) ?? null;
}

export const controllers = {
  /** Connected controllers, in the order they connected. */
  get pads(): readonly PadInfo[] {
    return state.pads;
  },
  /** The controller pressed last (or the only one), or null. */
  get current(): PadInfo | null {
    return currentPad();
  },
  /** Its name (the TV mode pane's rows), or null with none (item 38). */
  get name(): string | null {
    const p = currentPad();
    return p ? padDisplayName(p.name) : null;
  },
  /** Whether a key was pressed after the last controller press. */
  get keyboard(): boolean {
    return state.keyboard;
  },
  /** The hint bar's left side: "Keyboard" while the keyboard leads, else the
   *  controller's name, or null with none (item 38). */
  get hintName(): string | null {
    const p = currentPad();
    if (!p) return null;
    return state.keyboard ? KEYBOARD_NAME : padDisplayName(p.name);
  },
  /** Its button assignment; Xbox style with none (item 56). */
  get assignment() {
    const p = currentPad();
    return p ? assignmentFor(p, uiSettings.tvAssignments) : 'xbox';
  },
  /** The prompt set every glyph draws from (items 67–69): the controller's
   *  own layout, whatever its button assignment. */
  get promptSet(): PromptSet {
    const p = currentPad();
    // The keyboard's keys while it leads, or with no controller, whatever
    // Button prompts says (that setting is for controllers).
    if (!p || state.keyboard) return 'keyboard';
    return promptSet(uiSettings.tvPrompts, guessAssign(p), padBrand(p));
  },
};

/** The hint bar's left side while the keyboard leads. */
export const KEYBOARD_NAME = 'Keyboard';

/** A key was pressed (`true`) or a controller (`false`): it leads the
 *  prompts, and either hides the cursor. */
function lead(keyboard: boolean): void {
  if (state.keyboard !== keyboard) state.keyboard = keyboard;
  hideCursor();
}

/** The keyboard's source id in the repeater (controllers use their ids). */
const KEYBOARD = -1;

/**
 * Which held buttons repeat: directions, the on-screen keyboard's typing
 * buttons while it is the top overlay (A on a typing key, X, Y, LB and RB;
 * items 42–43), and the media viewer's LT / RT volume while a video shows
 * (timed like a direction). Asked at the press and at every repeat.
 */
function repeatKind(b: TvButton): RepeatKind {
  const layer = topLayer();
  if (layer?.typingRepeat?.(b, focused())) return 'typing';
  if (layer?.dirRepeat?.(b)) return 'dir';
  return isDirection(b) ? 'dir' : null;
}

const repeater = new Repeater<number>((b) => handlePress(b), repeatKind);
const causes = new Map<number, Causes>();
const rightY = new Map<number, number>();

function causesOf(id: number): Causes {
  let c = causes.get(id);
  if (!c) {
    c = new Causes();
    causes.set(id, c);
  }
  return c;
}

/** The controller pressed last drives prompts and names (item 47). */
function usePad(id: number): void {
  if (state.currentId !== id && state.pads.some((p) => p.id === id)) state.currentId = id;
  lead(false);
}

function addPad(pad: PadInfo): void {
  if (!state.pads.some((p) => p.id === pad.id)) state.pads = [...state.pads, pad];
  // With none in use, the first connected one is (tvmode.js `pollPads`).
  if (state.currentId === null || !state.pads.some((p) => p.id === state.currentId)) state.currentId = state.pads[0]?.id ?? null;
}

function dropPad(id: number): void {
  // A disconnected pad's held buttons get no up events (P4 handover).
  repeater.releaseSource(id);
  causes.delete(id);
  rightY.delete(id);
  state.pads = state.pads.filter((p) => p.id !== id);
  // The next connected controller takes over; with none, the name clears
  // (item 39).
  if (state.currentId === id) state.currentId = state.pads[0]?.id ?? null;
}

function down(id: number, b: TvButton, cause: string): void {
  if (causesOf(id).add(b, cause)) repeater.down(id, b);
}

function up(id: number, b: TvButton, cause: string): void {
  if (causesOf(id).remove(b, cause)) repeater.up(id, b);
}

function onPadEvent(e: PadEvent): void {
  switch (e.kind) {
    case 'connected':
      addPad(e.pad);
      return;
    case 'disconnected':
      dropPad(e.id);
      return;
    case 'released_all':
      // The gate closed (a game started, the window lost focus): nothing
      // stays held or repeats (item 44).
      repeater.releaseAll();
      causes.clear();
      rightY.clear();
      return;
    case 'button': {
      const pad = state.pads.find((p) => p.id === e.id);
      if (!pad) return;
      const c = causesOf(e.id);
      if (e.down) {
        // Each controller reads its own presses with its own assignment
        // (item 46); the assignment is fixed while the button is held.
        const b = logicalButton(e.button, assignmentFor(pad, uiSettings.tvAssignments));
        if (b === null) return;
        usePad(e.id);
        down(e.id, b, e.button);
      } else {
        const b = c.heldBy(e.button);
        if (b !== null) up(e.id, b, e.button);
      }
      return;
    }
    case 'stick': {
      if (!state.pads.some((p) => p.id === e.id)) return;
      if (e.stick === 'left') {
        const want = new Set(leftStickDirections(e.x, e.y));
        const c = causesOf(e.id);
        for (const b of ['UP', 'DOWN', 'LEFT', 'RIGHT'] as const) {
          const cause = `ls:${b}`;
          if (want.has(b) && c.heldBy(cause) === null) {
            usePad(e.id);
            down(e.id, b, cause);
          } else if (!want.has(b) && c.heldBy(cause) !== null) up(e.id, b, cause);
        }
      } else {
        rightY.set(e.id, e.y);
        if (rightStickActive(e.y)) {
          usePad(e.id);
          startScrollLoop();
        }
      }
      return;
    }
    default:
      return;
  }
}

// ---- The right stick scrolls what you're reading (items 36–37).

let raf: number | null = null;
let lastFrame = 0;

/** The area the right stick and Page Up / Down scroll: Details' tab body
 *  (`data-read-area`), else the area holding the focused item; nothing
 *  while an overlay is open. */
function readingArea(): HTMLElement | null {
  if (nav.layers.length) return null;
  const root = scope();
  const area = root?.querySelector<HTMLElement>('[data-read-area]');
  if (area) return area;
  const f = focused();
  return f ? vScroller(f) : null;
}

function startScrollLoop(): void {
  if (raf !== null) return;
  lastFrame = performance.now();
  const frame = (now: number) => {
    const dt = Math.min(64, now - lastFrame);
    lastFrame = now;
    let y = 0;
    for (const v of rightY.values()) if (Math.abs(v) > Math.abs(y)) y = v;
    if (!rightStickActive(y)) {
      raf = null;
      return;
    }
    const px = rightStickScroll(y, dt, remPx());
    readingArea()?.scrollBy({ top: px });
    raf = requestAnimationFrame(frame);
  };
  raf = requestAnimationFrame(frame);
}

// ---- Keyboard (items 71–73).

function onKeyDown(e: KeyboardEvent): void {
  if (e.key === 'PageDown' || e.key === 'PageUp') {
    e.preventDefault();
    lead(true);
    const v = readingArea();
    v?.scrollBy(pageScroll(e.key === 'PageDown', v.clientHeight, reducedMotion()));
    return;
  }
  // While the on-screen keyboard is open, a printable key types straight in
  // (Space too), Backspace deletes and Enter is Done after physical typing
  // (item 74). The system's key repeat types like a held key would.
  const layer = topLayer();
  if (layer?.keydown?.(e)) {
    e.preventDefault();
    lead(true);
    return;
  }
  const b = buttonForKey(e.key, { ctrl: e.ctrlKey, alt: e.altKey, meta: e.metaKey });
  if (!b) return;
  e.preventDefault();
  lead(true);
  // The repeater times a held key like a held button (item 72); the
  // system's own key repeat is ignored.
  if (e.repeat) return;
  down(KEYBOARD, b, `key:${e.key.toLowerCase()}`);
}

function onKeyUp(e: KeyboardEvent): void {
  const c = causes.get(KEYBOARD);
  const cause = `key:${e.key.toLowerCase()}`;
  const b = c?.heldBy(cause);
  if (b) up(KEYBOARD, b, cause);
}

function onBlur(): void {
  // A key released while the window was away sends no keyup.
  repeater.releaseSource(KEYBOARD);
  causes.delete(KEYBOARD);
}

// ---- The mouse: keyboard and controller only (README § Decided, 2026-11-11).

/** The class on <html> that hides the cursor over the window (tv.css). */
export const NO_CURSOR = 'tv-nocursor';
/** How long the cursor stays after the mouse stops. */
export const CURSOR_IDLE_MS = 1500;
let cursorTimer: ReturnType<typeof setTimeout> | null = null;
let lastPointer: { sx: number; sy: number; cx: number; cy: number } | null = null;

function hideCursor(): void {
  if (cursorTimer !== null) clearTimeout(cursorTimer);
  cursorTimer = null;
  document.documentElement.classList.add(NO_CURSOR);
}

// Only a move shows the cursor: WebKit sends a synthetic mousemove at the
// unchanged position after a scroll or a redraw. Either coordinate pair
// counts (WebDriver's pointer moves the window's, not the screen's); a
// window moving under a resting pointer shows it for 1.5 s, which is
// harmless.
function onMouseMove(e: MouseEvent): void {
  const p = lastPointer;
  const moved = p !== null && (p.sx !== e.screenX || p.sy !== e.screenY || p.cx !== e.clientX || p.cy !== e.clientY);
  lastPointer = { sx: e.screenX, sy: e.screenY, cx: e.clientX, cy: e.clientY };
  if (!moved) return;
  document.documentElement.classList.remove(NO_CURSOR);
  if (cursorTimer !== null) clearTimeout(cursorTimer);
  cursorTimer = setTimeout(hideCursor, CURSOR_IDLE_MS);
}

// TV mode's content takes no pointer events (tv.css), so nothing under the
// pointer reacts; these stop what would still reach the page itself.
function swallow(e: Event): void {
  e.preventDefault();
  e.stopPropagation();
}
const SWALLOWED = ['click', 'dblclick', 'auxclick', 'contextmenu', 'wheel'] as const;

/** Starts every input; answers the function that stops them. */
export function startRouter(): () => void {
  let un: UnlistenFn | null = null;
  let stopped = false;
  listen<PadEvent>(GAMEPAD_EVENT, (ev) => onPadEvent(ev.payload)).then((f) => {
    if (stopped) f();
    else un = f;
  });
  // Controllers already connected (TV mode may have been on before).
  api
    .gamepadList()
    .then((pads) => {
      if (!stopped) for (const p of pads) addPad(p);
    })
    .catch(() => {});
  window.addEventListener('keydown', onKeyDown);
  window.addEventListener('keyup', onKeyUp);
  window.addEventListener('blur', onBlur);
  document.addEventListener('mousemove', onMouseMove);
  for (const t of SWALLOWED) document.addEventListener(t, swallow, { capture: true, passive: false });
  hideCursor();
  return () => {
    stopped = true;
    un?.();
    window.removeEventListener('keydown', onKeyDown);
    window.removeEventListener('keyup', onKeyUp);
    window.removeEventListener('blur', onBlur);
    document.removeEventListener('mousemove', onMouseMove);
    for (const t of SWALLOWED) document.removeEventListener(t, swallow, { capture: true });
    if (cursorTimer !== null) clearTimeout(cursorTimer);
    cursorTimer = null;
    document.documentElement.classList.remove(NO_CURSOR);
    if (raf !== null) cancelAnimationFrame(raf);
    raf = null;
    repeater.releaseAll();
    causes.clear();
    rightY.clear();
    lastPointer = null;
    state.pads = [];
    state.currentId = null;
    state.keyboard = false;
  };
}
