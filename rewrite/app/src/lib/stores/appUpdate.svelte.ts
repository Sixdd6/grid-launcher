// The self-update banner's state. Module-scoped so a dismissal survives
// Shell remounts for the rest of the process.
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { APP_UPDATE_EVENT, type AppUpdateNotice } from '../api';

const state = $state<{ notice: AppUpdateNotice | null; dismissed: boolean }>({ notice: null, dismissed: false });

export const appUpdate = {
  get notice() {
    return state.dismissed ? null : state.notice;
  },
};

export function dismiss(): void {
  state.dismissed = true;
}

export function initAppUpdate(): Promise<UnlistenFn> {
  return listen<AppUpdateNotice>(APP_UPDATE_EVENT, (e) => {
    state.notice = e.payload;
  });
}
