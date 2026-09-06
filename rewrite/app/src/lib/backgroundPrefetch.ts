// The one place a dwell timer touches the backend, and the memo of what it
// built. Split out of `lastViewedHover.ts` so that module stays trivially
// mockable in vitest (it already mocks the store the same way) and so nothing
// else can start a background fetch by accident.
import { api } from './api';
import { backgroundUrls, type BackgroundSubject } from './background';
import { uiSettings } from './stores/uiSettings.svelte';

/** How many resolved paths to keep: enough for a long browse of one view,
 *  small enough that an all-day session cannot grow without bound. */
export const VARIANT_MEMO_CAP = 64;

/**
 * `variantKey(blur, url)` -> local variant path. MODULE scoped, unlike a
 * `<script>` const in the component, so a Shell remount (a reconnect) keeps
 * it: the path is stable — the backend's cache is keyed by URL and sigma —
 * and asking again costs an IPC round trip, so re-selecting a card the user
 * has already dwelled on must not pay for it twice. The on-enter prefetch, the
 * scroll warmer and `BackgroundArt`'s swap all read and fill it, which is the
 * whole point of warming ahead.
 */
export const variantPaths = new Map<string, string>();

/** The memo key. The sigma is baked into the variant's file name, so one URL
 *  at two blur levels is two different files and must be two entries. The
 *  newline cannot appear in a sigma, so the two halves cannot run together. */
export function variantKey(blur: number, url: string): string {
  return `${blur}\n${url}`;
}

/** Records `path` for `key` (from `variantKey`), dropping the oldest entries
 *  past the cap. `Map` iterates in insertion order, so the oldest key comes
 *  out first. */
export function rememberVariant(key: string, path: string): void {
  variantPaths.set(key, path);
  for (const oldest of variantPaths.keys()) {
    if (variantPaths.size <= VARIANT_MEMO_CAP) break;
    variantPaths.delete(oldest);
  }
}

/** What building `subject`'s first background image would take, or `null`
 *  when there is nothing worth building: the art is switched off (design §10,
 *  nothing would be shown), the subject has no art at all, or the path is
 *  already memoised. The single place the URL priority and the sigma are
 *  read, so the hover prefetch and the scroll warmer can never disagree about
 *  which file a card means. */
type BuildTarget = { url: string; blur: number; key: string };

function buildTarget(subject: BackgroundSubject): BuildTarget | null {
  if (uiSettings.backgroundFade === 0) return null;
  const url = backgroundUrls(subject)[0];
  if (url === undefined) return null;
  const blur = uiSettings.backgroundBlur;
  const key = variantKey(blur, url);
  if (variantPaths.has(key)) return null;
  return { url, blur, key };
}

/** Asks the backend for one variant and memoises it. ALWAYS settles: a
 *  failure resolves rather than rejecting, so a caller counting builds in
 *  flight cannot be left holding a slot. */
function build(target: BuildTarget): Promise<void> {
  return api.ensureBackgroundVariant(target.url, target.blur).then(
    (path) => {
      rememberVariant(target.key, path);
    },
    () => {}
  );
}

/**
 * Starts building `subject`'s first background image, without waiting for it.
 * Called the moment the pointer enters a card, while the actual swap still
 * waits out `HOVER_DELAY_MS`, so the fetch + decode + blur has a head start
 * and the swap usually finds the path already memoised. A failure is silent:
 * the swap path asks again and, if that also fails, the current art stays.
 */
export function prefetchBackground(subject: BackgroundSubject): void {
  const target = buildTarget(subject);
  if (target === null) return;
  void build(target);
}

/**
 * How many warm builds may be in flight at once. Two: the warmer competes
 * with the covers the grid is still downloading for the same backend slots,
 * and a background nobody has hovered yet must never be the reason a visible
 * cover arrives late.
 */
export const WARM_CONCURRENCY = 2;

/** Every key a warm has been started for, successful or not. Kept after the
 *  build settles on purpose: a refused warm is DROPPED, never retried, so an
 *  offline session asks once per card instead of again on every re-observe.
 *  `clearVariantMemo` empties it — that is the one event that makes the
 *  memoised paths worth rebuilding. */
const warmed = new Set<string>();
const warmQueue: BuildTarget[] = [];
let warmInFlight = 0;

/**
 * Queues `subject`'s first background image to be built ahead of any hover,
 * so a card the user has only scrolled past already has its art on disk.
 * Called by `visibleWarm.ts` as cards enter the viewport. Silent about
 * everything: nothing is on screen yet, so nothing can go visibly wrong.
 */
export function warmBackground(subject: BackgroundSubject): void {
  const target = buildTarget(subject);
  if (target === null || warmed.has(target.key)) return;
  warmed.add(target.key);
  warmQueue.push(target);
  drainWarmQueue();
}

/** Starts queued builds up to `WARM_CONCURRENCY`, and again as each settles.
 *  A loop, not one recursion per item: a whole screen of cards arrives in a
 *  single observer callback. */
function drainWarmQueue(): void {
  while (warmInFlight < WARM_CONCURRENCY) {
    const target = warmQueue.shift();
    if (target === undefined) return;
    // Re-checked here, not only at enqueue time: an earlier warm — or the
    // hover prefetch — may have memoised this exact key while the entry
    // waited its turn.
    if (variantPaths.has(target.key)) continue;
    warmInFlight += 1;
    void build(target).then(() => {
      warmInFlight -= 1;
      drainWarmQueue();
    });
  }
}

/** Test seam: empties the queue and forgets what has been warmed. */
export function resetWarmQueue(): void {
  warmQueue.length = 0;
  warmInFlight = 0;
  warmed.clear();
}

/**
 * Drops every memoised path. Call this whenever the blur sigma changes: the
 * backend keeps ONE variant per source (`remove_stale_variants` in
 * `images/background.rs` deletes the other sigmas after every successful
 * build), so each path memoised at another sigma names a file that build has
 * already deleted. Without this a blur value the user returns to would hit
 * the memo and paint a background that is no longer on disk.
 */
export function clearVariantMemo(): void {
  variantPaths.clear();
  // The warm set holds keys whose paths have just been dropped. Left alone,
  // a blur level the user returns to could never warm again: the record of
  // the earlier warm would block it, and the file that warm produced is the
  // one this call exists to disown.
  warmed.clear();
}
