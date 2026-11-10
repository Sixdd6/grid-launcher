// Which interface is on screen: the desktop Shell or TvShell (TV mode plan
// § 1.1). The Rust side owns the window (fullscreen) and the controller
// gate (`set_ui_mode`); this store mirrors it and remembers where each
// interface should open after a switch, so the switch lands on the same view
// (checklist item 523). The mode is never persisted: GRID starts in desktop
// mode unless started with `--tv` (item 524), which Rust has already applied
// by the time the UI asks (`get_ui_mode`).
import { api, type UiMode } from '../api';
import type { SettingsPage } from '../settings/pages';
import type { View } from '../shell';
import type { TvTab } from '../tv/input/press';
import { desktopViewFor, tvPaneFor, tvTabFor, type TvPane } from '../uiMode';

const state = $state<{
  mode: UiMode;
  startedWithTv: boolean;
  /** `get_ui_mode` has answered (or failed: desktop stands). */
  loaded: boolean;
  /** Where TvShell opens after the desktop's TV button; null on a `--tv`
   *  start (the start rule decides). */
  tvStart: { tab: TvTab; pane: TvPane | null } | null;
  /** Where the desktop Shell opens after Switch to desktop mode; null on a
   *  normal start (the start rule decides). */
  desktopStart: { view: View; settingsPage: SettingsPage | null } | null;
  busy: boolean;
}>({ mode: 'desktop', startedWithTv: false, loaded: false, tvStart: null, desktopStart: null, busy: false });

export const uiMode = {
  get mode(): UiMode {
    return state.mode;
  },
  get startedWithTv(): boolean {
    return state.startedWithTv;
  },
  get loaded(): boolean {
    return state.loaded;
  },
  get tvStart() {
    return state.tvStart;
  },
  get desktopStart() {
    return state.desktopStart;
  },
};

/** Reads the mode Rust started in. */
export async function initUiMode(): Promise<void> {
  try {
    const info = await api.getUiMode();
    state.mode = info.mode;
    state.startedWithTv = info.started_with_tv;
  } catch {
    // No answer: the desktop is the safe interface.
  }
  state.loaded = true;
}

/** The desktop top bar's TV button (item 521): TV mode on the tab matching
 *  the desktop view (and the Settings page, when it is Settings). */
export async function enterTv(view: View, settingsPage: SettingsPage | null = null): Promise<void> {
  if (state.busy || state.mode === 'tv') return;
  state.busy = true;
  try {
    await api.setUiMode('tv');
    state.tvStart = { tab: tvTabFor(view), pane: settingsPage === null ? null : tvPaneFor(settingsPage) };
    state.mode = 'tv';
  } finally {
    state.busy = false;
  }
}

/** Menu › Switch to desktop mode and Settings › TV mode › Switch to desktop
 *  mode (item 523): the desktop window, on the same view. */
export async function leaveTv(tab: TvTab, pane: TvPane): Promise<void> {
  if (state.busy || state.mode === 'desktop') return;
  state.busy = true;
  try {
    await api.setUiMode('desktop');
    state.desktopStart = desktopViewFor(tab, pane);
    state.mode = 'desktop';
  } finally {
    state.busy = false;
  }
}
