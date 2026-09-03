// The self-update banner's state. Module-scoped so a dismissal survives
// Shell remounts for the rest of the process.
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { api, APP_UPDATE_EVENT, type AppUpdateNotice } from '../api';

const state = $state<{ notice: AppUpdateNotice | null; dismissed: boolean }>({ notice: null, dismissed: false });

export const appUpdate = {
  get notice() {
    return state.dismissed ? null : state.notice;
  },
};

export function dismiss(): void {
  state.dismissed = true;
}

export async function initAppUpdate(): Promise<UnlistenFn> {
  // Listener FIRST, then pull: the startup check runs from Tauri's `setup`
  // and can emit before the webview mounts, and Tauri buffers nothing for a
  // window with no listener. `app_update_notice` holds whatever the check
  // already found, so the banner survives that race. An event that arrived
  // in between is newer and wins.
  const unlisten = await listen<AppUpdateNotice>(APP_UPDATE_EVENT, (e) => {
    state.notice = e.payload;
  });
  try {
    const notice = await api.appUpdateNotice();
    if (notice !== null && state.notice === null) state.notice = notice;
  } catch {
    // No notice is the normal outcome; a failed pull is never surfaced.
  }
  return unlisten;
}
