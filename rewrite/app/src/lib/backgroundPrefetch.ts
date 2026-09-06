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
 * has already dwelled on must not pay for it twice. Both the 150ms prefetch
 * and `BackgroundArt`'s 500ms swap read and fill it, which is the whole point
 * of the prefetch.
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

/**
 * Starts building `subject`'s first background image, without waiting for it.
 * Called at 150ms of dwell while the actual swap still waits for 500ms, so
 * the fetch + decode + blur has a head start and the swap usually finds the
 * path already memoised. A failure is silent: the swap path asks again and,
 * if that also fails, the current art simply stays.
 */
export function prefetchBackground(subject: BackgroundSubject): void {
  // Background art off (design §10): nothing would be shown, so nothing is
  // worth downloading, decoding and blurring.
  if (uiSettings.backgroundFade === 0) return;
  const url = backgroundUrls(subject)[0];
  if (url === undefined) return;
  const blur = uiSettings.backgroundBlur;
  const key = variantKey(blur, url);
  if (variantPaths.has(key)) return;
  void api
    .ensureBackgroundVariant(url, blur)
    .then((path) => rememberVariant(key, path))
    .catch(() => {});
}
