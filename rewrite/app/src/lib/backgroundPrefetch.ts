// The one place a dwell timer touches the backend. Split out of
// `lastViewedHover.ts` so that module stays trivially mockable in vitest (it
// already mocks the store the same way) and so nothing else can start a
// background fetch by accident.
import { api } from './api';
import { backgroundUrls, type BackgroundSubject } from './background';

/**
 * Starts building `subject`'s first background image, without waiting for it.
 * Called at 150ms of dwell while the actual swap still waits for 500ms, so
 * the fetch + decode + blur has a head start and the swap usually has
 * something ready. A failure is silent: the swap path asks again and, if that
 * also fails, the current art simply stays.
 */
export function prefetchBackground(subject: BackgroundSubject): void {
  const url = backgroundUrls(subject)[0];
  if (url === undefined) return;
  void api.ensureBackgroundVariant(url).catch(() => {});
}
