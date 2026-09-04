// The self-update notice's state (design §3: a badge on the user menu plus
// an entry under Settings › Updates). Module-scoped so a dismissal survives
// Shell remounts for the rest of the process.
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { api, APP_UPDATE_EVENT, type AppUpdateNotice, type AppUpdateStatus } from '../api';

const state = $state<{ notice: AppUpdateNotice | null; dismissed: boolean; checkedAt: string | null }>({
  notice: null,
  dismissed: false,
  checkedAt: null,
});

export const appUpdate = {
  /** What the badge shows: nothing once dismissed. */
  get notice() {
    return state.dismissed ? null : state.notice;
  },
  /** What Settings › Updates shows: the stored notice, dismissed or not. */
  get stored() {
    return state.notice;
  },
  /** The backend's `checked_at` (RFC 3339 UTC): null while no check has completed. */
  get checkedAt() {
    return state.checkedAt;
  },
};

export function dismiss(): void {
  state.dismissed = true;
}

function applyStatus(status: AppUpdateStatus): void {
  // An event that arrived first is newer than the pull and keeps its notice;
  // `checked_at` comes only from the backend, never from a local clock.
  if (status.notice !== null && state.notice === null) state.notice = status.notice;
  state.checkedAt = status.checked_at;
}

export async function initAppUpdate(): Promise<UnlistenFn> {
  // Listener FIRST, then pull: the startup check runs from Tauri's `setup`
  // and can emit before the webview mounts, and Tauri buffers nothing for a
  // window with no listener. `app_update_notice` holds whatever the check
  // already found, so the badge survives that race. An event that arrived
  // in between is newer and wins.
  const unlisten = await listen<AppUpdateNotice>(APP_UPDATE_EVENT, (e) => {
    state.notice = e.payload;
    // The backend stamps `checked_at` before it emits, so one more pull
    // picks the stamp up for the Updates page.
    api.appUpdateNotice().then(applyStatus).catch(() => {});
  });
  try {
    applyStatus(await api.appUpdateNotice());
  } catch {
    // No notice is the normal outcome; a failed pull is never surfaced and
    // leaves `checkedAt` null, so the Updates page says "Not checked yet".
  }
  return unlisten;
}
