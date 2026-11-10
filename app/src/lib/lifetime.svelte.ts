// The listeners and loads that live as long as the shell does, whichever
// shell (desktop or TV) is on screen. `App.svelte` starts them when the
// session reaches the shell and stops them when it leaves (back to Connect,
// or behind the blocking unreachable-library dialog); a switch between the
// two shells keeps them running, so each event still shows exactly one card.
//
// A `.svelte.ts` module because the background-art seed follows the
// installed registry, which takes an effect.
import { listen } from '@tauri-apps/api/event';
import {
  CLOUD_RESTORE_NOTICE_EVENT,
  CLOUD_UPLOAD_FINISHED_EVENT,
  type CloudRestoreNotice,
  type CloudUploadFinished,
} from './api';
import { initNotifications } from './notifications/notifier';
import { installed, refresh as refreshInstalled } from './stores/installed.svelte';
import { seedLastViewed } from './stores/lastViewed.svelte';
import { pushToast } from './stores/toasts.svelte';
import { initEmulatorEvents } from './systems/events';

/** The card an automatic cloud upload ends with. */
export function uploadFinishedToast(e: CloudUploadFinished): { text: string; level: 'error' | 'success' } {
  return { text: e.title === '' ? e.message : `${e.title} — ${e.message}`, level: e.failed ? 'error' : 'success' };
}

/**
 * Starts the shell-lifetime wiring and answers the function that stops it.
 * Call it once per shell lifetime, not once per mounted view.
 */
export function startShellLifetime(): () => void {
  const stops: (() => void)[] = [];

  // Independent of the connection: Library shows installed games with
  // cached covers offline, so the registry loads once here even when the
  // shell comes up unreachable. The `images-replenished` listener is not
  // here: the replenish job is spawned during restore/connect, before the
  // shell exists, so App.svelte registers it at its own mount.
  refreshInstalled();

  // Startup fallback for the background art. Re-runs as the registry loads;
  // `seedLastViewed` is idempotent and never overwrites a real view.
  stops.push(
    $effect.root(() => {
      $effect(() => {
        seedLastViewed(installed.list);
      });
    }),
  );

  // The only report an auto upload has: it runs after the game has exited,
  // with no command in flight and usually no popup open.
  const offUpload = listen<CloudUploadFinished>(CLOUD_UPLOAD_FINISHED_EVENT, (e) => {
    const { text, level } = uploadFinishedToast(e.payload);
    pushToast(text, level);
  });
  stops.push(() => void offUpload.then((off) => off()));

  // Emulator install/update notices, app-lifetime rather than tied to
  // Settings › Systems being open, so each toasts once.
  stops.push(initEmulatorEvents());

  // The download, install and launch cards (queued, installed, failed,
  // cancelled, launching, early exits): app-lifetime for the same reason.
  stops.push(initNotifications());

  // G1-Q: the pre-launch restore skipped save folders it could not match to
  // the game. It runs inside `launch_game`, with no panel open to report in.
  // The message already names the game ("Cloud save for <title> …").
  const offRestore = listen<CloudRestoreNotice>(CLOUD_RESTORE_NOTICE_EVENT, (e) => {
    pushToast(e.payload.message, e.payload.failed ? 'error' : 'success');
  });
  stops.push(() => void offRestore.then((off) => off()));

  return () => {
    for (const stop of stops) stop();
  };
}
