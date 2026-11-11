// The app's half of the YouTube relay (app/src-tauri/src/youtube_relay.rs):
// the message protocol and the embed's fallback state machine. Pure, so
// both are tested without a webview.
//
// A trailer plays in an iframe of a page on GRID's loopback media server,
// which frames YouTube's player. The two talk by postMessage only, and each
// end checks the other's origin AND window before it acts on a message.
//
// relay → app: boot (until the app says hello), linked, ready, state (the YT
// player state), volume (0–100, muted) and error (the YT error code, or
// "load" when YouTube's player script did not load).
// app → relay: hello, play, pause, toggle, seek (by seconds), volume.

export const RELAY_SOURCE = 'grid-yt';

/** No ready signal by then: fall back to the poster and "Watch on YouTube". */
export const READY_TIMEOUT_MS = 10_000;
/** How long a trailer asked to play (on ready, or A) may take to report
 *  playing before TV mode offers "Watch on YouTube" (user decision
 *  2026-11-11). */
export const STALL_MS = 8_000;

/** YouTube's player errors (2 bad id, 5 HTML5 player, 100 removed or
 *  private, 101 / 150 embedding disallowed, 153 no referrer). Every one of
 *  them falls back, as does any code YouTube adds later. */
export const FALLBACK_ERRORS: readonly number[] = [2, 5, 100, 101, 150, 153];

export type RelayEvent =
  | { type: 'boot' }
  | { type: 'linked' }
  | { type: 'ready' }
  | { type: 'state'; data: number }
  | { type: 'volume'; value: number; muted: boolean }
  | { type: 'error'; code: number | 'load' };

export type RelayCommand =
  | { cmd: 'hello' }
  | { cmd: 'play' }
  | { cmd: 'pause' }
  | { cmd: 'toggle' }
  | { cmd: 'seek'; by: number }
  | { cmd: 'volume'; value: number };

/** The relay's origin, `http://127.0.0.1:<port>`, from the URL the backend
 *  answered; null for anything else, which the embed then refuses. */
export function relayOrigin(url: string): string | null {
  let u: URL;
  try {
    u = new URL(url);
  } catch {
    return null;
  }
  if (u.protocol !== 'http:' || u.hostname !== '127.0.0.1' || u.port === '') return null;
  return u.origin;
}

function isObject(v: unknown): v is Record<string, unknown> {
  return typeof v === 'object' && v !== null;
}

/** A relay message's payload, or null when it is not one. */
export function parseRelayEvent(data: unknown): RelayEvent | null {
  if (!isObject(data) || data.source !== RELAY_SOURCE || typeof data.type !== 'string') return null;
  switch (data.type) {
    case 'boot':
    case 'linked':
    case 'ready':
      return { type: data.type };
    case 'state':
      return typeof data.data === 'number' && Number.isFinite(data.data) ? { type: 'state', data: data.data } : null;
    case 'volume':
      return typeof data.value === 'number' && Number.isFinite(data.value)
        ? { type: 'volume', value: Math.max(0, Math.min(100, Math.round(data.value))), muted: data.muted === true }
        : null;
    case 'error':
      if (data.code === 'load') return { type: 'error', code: 'load' };
      return typeof data.code === 'number' && Number.isFinite(data.code) ? { type: 'error', code: data.code } : null;
    default:
      return null;
  }
}

/** A message event, accepted only from the relay's origin AND its window
 *  (another frame on the same port must not drive the viewer). */
export function acceptRelayMessage(
  e: { origin: string; source: unknown; data: unknown },
  expected: { origin: string | null; source: unknown },
): RelayEvent | null {
  if (expected.origin === null || expected.source === null || expected.source === undefined) return null;
  if (e.origin !== expected.origin || e.source !== expected.source) return null;
  return parseRelayEvent(e.data);
}

/** A command as it is posted to the relay. */
export function relayMessage(c: RelayCommand): RelayCommand & { source: string } {
  return { ...c, source: RELAY_SOURCE };
}

// ---- The fallback state machine

/** resolving: asking the backend for the relay URL; loading: the iframe is
 *  up and the player is not ready yet; ready: it plays; fallback: the
 *  poster and "Watch on YouTube" (absorbing). */
export type EmbedPhase = 'resolving' | 'loading' | 'ready' | 'fallback';

export type EmbedState = {
  phase: EmbedPhase;
  /** The relay answered the app's hello: messages pass both ways. */
  linked: boolean;
  /** YouTube's player state (-1 unstarted, 0 ended, 1 playing, 2 paused,
   *  3 buffering, 5 cued). */
  player: number;
  /** Why it fell back: a relay error, the ready timeout, or no relay URL. */
  reason: 'error' | 'timeout' | 'url' | null;
  code: number | 'load' | null;
};

export type EmbedInput = RelayEvent | { type: 'url' } | { type: 'urlError' } | { type: 'timeout' };

export function initialEmbed(): EmbedState {
  return { phase: 'resolving', linked: false, player: -1, reason: null, code: null };
}

export function embedStep(s: EmbedState, input: EmbedInput): EmbedState {
  if (s.phase === 'fallback') return s;
  switch (input.type) {
    case 'url':
      return s.phase === 'resolving' ? { ...s, phase: 'loading' } : s;
    case 'urlError':
      return { ...s, phase: 'fallback', reason: 'url' };
    case 'linked':
      return { ...s, linked: true };
    case 'ready':
      return s.phase === 'loading' ? { ...s, phase: 'ready', linked: true } : s;
    case 'state':
      return { ...s, player: input.data };
    case 'error':
      // Embedding refused (101 / 150), removed (100), no referrer (153), a
      // bad id (2), the HTML5 player (5), or the API script did not load:
      // all mean this trailer won't play here.
      return { ...s, phase: 'fallback', reason: 'error', code: input.code };
    case 'timeout':
      return s.phase === 'ready' ? s : { ...s, phase: 'fallback', reason: 'timeout' };
    default:
      return s;
  }
}

/** The player is on screen (starting, loading or playing) rather than the
 *  poster: everything but the fallback. */
export function embedShowing(s: EmbedState): boolean {
  return s.phase !== 'fallback';
}

/** Paused for A's label: anything but playing or buffering. */
export function embedPaused(s: EmbedState): boolean {
  return !(s.player === 1 || s.player === 3);
}
