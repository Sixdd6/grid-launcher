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
 * How many builds may be in flight at once, hover and scroll warming
 * TOGETHER. Three, out of the six download slots
 * `images/cache.rs`'s `MAX_CONCURRENT_DOWNLOADS` shares with the covers the
 * grid is still fetching: speculative background art must never be the reason
 * a visible cover arrives late. One queue for both paths, not one each, so
 * the cap is a real ceiling rather than a per-caller suggestion — at
 * `PREFETCH_DELAY_MS` of 0 a pointer sweep across a row of cold cards would
 * otherwise start a build per card with nothing to stop it.
 */
export const PREFETCH_CONCURRENCY = 3;

/** Every key a build has been ASKED for, successful or not. Kept after the
 *  build settles on purpose: a refused build is DROPPED, never retried, so an
 *  offline session asks once per card instead of again on every re-observe or
 *  every re-hover. `clearVariantMemo` empties it — that is the one event that
 *  makes the memoised paths worth rebuilding. */
const requested = new Set<string>();
const pending: BuildTarget[] = [];
let inFlight = 0;
/** Bumped by `resetPrefetchQueue`, captured by each build. A build started
 *  before a reset must not decrement the counter the reset zeroed, or the
 *  queue would run more than `PREFETCH_CONCURRENCY` at once afterwards. */
let generation = 0;

/** Adds `subject`'s first image to the queue. `front` puts it ahead of
 *  everything still waiting — the hover path, which is the card the user is
 *  actually looking at.
 *
 *  A key already asked for is not queued twice, but the front lane still
 *  PROMOTES one that has not started: a card warmed on scroll and then
 *  hovered before its turn came up would otherwise wait behind every other
 *  card the user merely scrolled past, which is the one case the front lane
 *  exists for. A key already in flight, already memoised, or already tried
 *  and failed is left alone — there is nothing left to reorder, and a
 *  failure is never retried. */
function queueBuild(subject: BackgroundSubject, front: boolean): void {
  const target = buildTarget(subject);
  if (target === null) return;
  if (requested.has(target.key)) {
    if (front) promote(target.key);
    return;
  }
  requested.add(target.key);
  if (front) pending.unshift(target);
  else pending.push(target);
  drainQueue();
}

/** Moves the pending entry for `key` to the front of the queue, if it is
 *  still waiting. A no-op for a key that has already started or finished. */
function promote(key: string): void {
  const at = pending.findIndex((entry) => entry.key === key);
  if (at <= 0) return; // not waiting, or already first
  const [entry] = pending.splice(at, 1);
  pending.unshift(entry);
}

/**
 * Queues `subject`'s first background image AHEAD of any pending warm.
 * Called the moment the pointer enters a card, while the actual swap still
 * waits out `HOVER_DELAY_MS`, so the fetch + decode + blur has a head start
 * and the swap usually finds the path already memoised. Dropping one loses
 * nothing visible: `BackgroundArt`'s swap path asks the backend directly, so
 * the card the pointer actually stops on still gets its image.
 */
export function prefetchBackground(subject: BackgroundSubject): void {
  queueBuild(subject, true);
}

/**
 * Queues `subject`'s first background image BEHIND anything already waiting,
 * so a card the user has only scrolled past already has its art on disk by
 * the time it is hovered. Called by `visibleWarm.ts` as cards enter the
 * viewport.
 *
 * Returns `false` only when the background art is switched off (design §10) —
 * nothing would be shown, so nothing is worth building, and the caller should
 * keep watching the card in case the setting comes back. `true` means the
 * card is dealt with: queued, already queued, or already on disk.
 */
export function warmBackground(subject: BackgroundSubject): boolean {
  if (uiSettings.backgroundFade === 0) return false;
  queueBuild(subject, false);
  return true;
}

/** Starts queued builds up to `PREFETCH_CONCURRENCY`, and again as each
 *  settles. A loop, not one recursion per item: a whole screen of cards
 *  arrives in a single observer callback. */
function drainQueue(): void {
  while (inFlight < PREFETCH_CONCURRENCY) {
    const target = pending.shift();
    if (target === undefined) return;
    // Re-checked here, not only at enqueue time: another build may have
    // memoised this exact key while the entry waited its turn. No slot is
    // taken, so the loop simply moves on to the next entry.
    if (variantPaths.has(target.key)) continue;
    const era = generation;
    inFlight += 1;
    void build(target).then(() => {
      if (era !== generation) return;
      inFlight -= 1;
      drainQueue();
    });
  }
}

/** Test seam: empties the queue and forgets what has been asked for. */
export function resetPrefetchQueue(): void {
  pending.length = 0;
  inFlight = 0;
  generation += 1;
  requested.clear();
}

/** Test seam: how many builds the queue currently has in flight. */
export function inFlightBuilds(): number {
  return inFlight;
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
  // The requested set holds keys whose paths have just been dropped. Left
  // alone, a blur level the user returns to could never be built again: the
  // record of the earlier ask would block it, and the file that ask produced
  // is the one this call exists to disown.
  requested.clear();
}
