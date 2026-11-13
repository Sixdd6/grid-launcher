// The desktop's store screens: which store dialog is open (Details, Install,
// Game Settings, the cloud conflict prompt, the offline installer, a GOG
// backup) and the actions their buttons run, with the notification cards
// they raise. Desktop only: TV mode (lib/tv) has its own; both read the pure
// modules in `gamestores/` and the rune store `stores/gameStores.svelte.ts`.
import {
  api,
  type DownloadEntry,
  type DriveStoreGame,
  type StoreCloudState,
  type StoreGame,
  type StoreNotice,
  type StoreNoticeAction,
  type StoreSource,
} from '../../api';
import { toLaunchFailure } from '../../launchError';
import { gameStores, clearLoginError, onStoreLogin, onStoreNotice } from '../../stores/gameStores.svelte';
import { sessions } from '../../stores/sessions.svelte';
import { navigation } from '../../stores/navigation.svelte';
import { selectRail } from '../../library/selection.svelte';
import { pushToast, TOAST_DURATION_MS } from '../../stores/toasts.svelte';
import { downloads, onSnapshot } from '../../stores/downloads.svelte';
import { hasStoreCloud } from '../marks';
import {
  cancelledNotice,
  connectedNotice,
  dlcQueuedNotice,
  dlcRemovedNotice,
  installerFailedNotice,
  keptLocalNotice,
  launchFailedNotice,
  launchingNotice,
  loginFailedNotice,
  noticeActionLabel,
  noticeCard,
  queuedNotice,
  refusedNotice,
  restoredCloudNotice,
  signInToInstallNotice,
  stoppedNotice,
  syncingNotice,
  toolNeededNotice,
  updateQueuedNotice,
  verifyingNotice,
  wizardNotice,
  type StoreCard,
} from '../notices';
import { storeRailKey } from '../rail';
import type { StoreTab } from '../details';

export type DetailsOpen = {
  key: string;
  tab: StoreTab;
  /** Opened from a backup drive's grid. */
  drive?: { key: string; name: string; label: string; game: DriveStoreGame } | null;
};

type UiState = {
  details: DetailsOpen | null;
  install: StoreGame | null;
  conflict: { game: StoreGame; state: StoreCloudState; launch: boolean } | null;
  settings: StoreGame | null;
  installer: boolean;
  backup: { keys: string[]; title: string; onQueued?: () => void } | null;
  /** Games whose "Sync now" is running. */
  syncing: string[];
};

const ui = $state<UiState>({ details: null, install: null, conflict: null, settings: null, installer: false, backup: null, syncing: [] });

export const storeUi = {
  get details() {
    return ui.details;
  },
  get install() {
    return ui.install;
  },
  get conflict() {
    return ui.conflict;
  },
  get settings() {
    return ui.settings;
  },
  get installer() {
    return ui.installer;
  },
  get backup() {
    return ui.backup;
  },
  isSyncing(key: string): boolean {
    return ui.syncing.includes(key);
  },
};

// ── Cards ──────────────────────────────────────────────────────────────

/** Shows a store card, with its one action button on the desktop. */
export function showCard(card: StoreCard, source: StoreSource | null = 'gog'): void {
  const action = card.action;
  pushToast(card.text, card.error ? 'error' : 'success', TOAST_DURATION_MS, {
    sub: card.sub,
    ...(action ? { desktopAction: { label: noticeActionLabel(action, source), run: () => runNoticeAction(action, source) } } : {}),
  });
}

function errText(err: unknown): string {
  return err instanceof Error ? err.message : typeof err === 'string' ? err : String((err as { message?: unknown })?.message ?? err);
}

/** What a card's action button does. */
export function runNoticeAction(action: StoreNoticeAction, source: StoreSource | null): void {
  const src = source ?? 'gog';
  switch (action.kind) {
    case 'play': {
      const g = action.key ? gameStores.game(action.key) : null;
      if (g) void playStore(g);
      return;
    }
    case 'open_downloads':
      navigation.go('downloads');
      return;
    case 'try_again':
    case 'sign_in':
      void signIn(src);
      return;
    case 'install_tool':
      void installTool(src);
      return;
    case 'show_updates':
      selectRail('updates');
      navigation.go('library');
      return;
    case 'install_base': {
      const g = action.key ? gameStores.game(action.key) : null;
      if (g) openInstall(g);
      else navigation.go('downloads');
      return;
    }
    case 'open_settings':
      navigation.goSettings('stores');
      return;
  }
}

// ── Opening and closing ────────────────────────────────────────────────

export function openStoreDetails(key: string, tab: StoreTab = 'overview', drive: DetailsOpen['drive'] = null): void {
  ui.details = { key, tab, drive };
}

export function closeStoreDetails(): void {
  ui.details = null;
}

export function openInstaller(): void {
  ui.installer = true;
}

export function closeInstaller(): void {
  ui.installer = false;
}

export function openGameSettings(game: StoreGame): void {
  ui.settings = game;
}

export function closeGameSettings(): void {
  ui.settings = null;
}

export function closeInstall(): void {
  ui.install = null;
}

export function closeConflict(): void {
  ui.conflict = null;
}

export function openStoreBackup(keys: string[], title: string, onQueued?: () => void): void {
  ui.backup = { keys, title, onQueued };
}

export function closeStoreBackup(): void {
  ui.backup = null;
}

/** Library › GOG › Installed (Settings › Stores' "Show in Library"). */
export function showInLibrary(source: StoreSource, part: 'installed' | 'available' = 'installed'): void {
  selectRail(storeRailKey(source, part));
  navigation.go('library');
}

// ── Actions ────────────────────────────────────────────────────────────

/** Install / Install…: the dialog, or the card that says why it can't open. */
export function openInstall(game: StoreGame): void {
  const status = gameStores.status(game.source);
  if (game.not_supported !== '') {
    openStoreDetails(game.key);
    return;
  }
  if (status === null || status.connection !== 'connected') {
    showCard(signInToInstallNotice(game.source, game.title), game.source);
    return;
  }
  if (status.tool.state !== 'installed') {
    showCard(toolNeededNotice(game.source), game.source);
    return;
  }
  ui.install = game;
}

/** Whether a launch was refused for a cloud save conflict (`launch_game_key`'s store branch). */
export function isCloudConflict(err: unknown): boolean {
  return toLaunchFailure(err).code === 'store_cloud_conflict' || (err as { code?: unknown })?.code === 'store_cloud_conflict';
}

/**
 * Play: "Syncing cloud saves for … " then "Launching …". A conflict opens
 * "Which save should … use?" before the game starts.
 */
export async function playStore(game: StoreGame): Promise<void> {
  const status = gameStores.status(game.source);
  const cloudOn = hasStoreCloud(game.source) && (status?.cloud_saves ?? false) && game.cloud !== 'off' && game.cloud !== 'none';
  if (cloudOn && game.cloud !== 'conflict') showCard(syncingNotice(game.title));
  try {
    await api.launchGameKey(game.key);
    showCard(launchingNotice(game.title));
  } catch (err) {
    if (isCloudConflict(err)) {
      try {
        const state = await api.storeCloudState(game.key);
        ui.conflict = { game, state, launch: true };
      } catch (e) {
        showCard(launchFailedNotice(game.title, errText(e)));
      }
      return;
    }
    showCard(launchFailedNotice(game.title, toLaunchFailure(err).message));
  }
}

export async function stopStore(game: StoreGame): Promise<void> {
  const s = sessions.sessionForKey(game.key);
  if (!s) return;
  try {
    await api.stopGame(s.id);
    showCard(stoppedNotice(game.title));
  } catch (err) {
    showCard(refusedNotice(`Couldn’t stop ${game.title}`, errText(err)));
  }
}

/** Keep this computer's save / Use the cloud save; a launch prompt then starts the game. */
export async function resolveConflict(choice: 'upload' | 'download'): Promise<void> {
  const c = ui.conflict;
  if (!c) return;
  ui.conflict = null;
  await resolveCloud(c.game, choice);
  if (c.launch) await playStore({ ...c.game, cloud: 'synced' });
}

export async function resolveCloud(game: StoreGame, choice: 'upload' | 'download'): Promise<StoreCloudState | null> {
  try {
    const state = await api.storeCloudResolve(game.key, choice);
    showCard(choice === 'upload' ? keptLocalNotice(game.title) : restoredCloudNotice(game.title));
    return state;
  } catch (err) {
    showCard(refusedNotice(`Couldn’t sync ${game.title}’s cloud saves`, errText(err)));
    return null;
  }
}

/** Opens the conflict from Details (no launch after). */
export function openConflict(game: StoreGame, state: StoreCloudState): void {
  ui.conflict = { game, state, launch: false };
}

export async function syncNow(game: StoreGame): Promise<StoreCloudState | null> {
  ui.syncing = [...ui.syncing, game.key];
  try {
    return await api.storeCloudSync(game.key);
  } catch (err) {
    showCard(refusedNotice(`Couldn’t sync ${game.title}’s cloud saves`, errText(err)));
    return null;
  } finally {
    ui.syncing = ui.syncing.filter((k) => k !== game.key);
  }
}

async function run(what: string, fn: () => Promise<unknown>, ok?: StoreCard): Promise<boolean> {
  try {
    await fn();
    if (ok) showCard(ok);
    return true;
  } catch (err) {
    showCard(refusedNotice(what, errText(err)));
    return false;
  }
}

export const updateStore = (g: StoreGame) => run(`Couldn’t update ${g.title}`, () => api.storeUpdate(g.key), updateQueuedNotice(g.title));
export const repairStore = (g: StoreGame) => run(`Couldn’t verify ${g.title}`, () => api.storeRepair(g.key), verifyingNotice(g.title));
export const uninstallStore = (g: StoreGame) => run(`Couldn’t uninstall ${g.title}`, () => api.storeUninstall(g.key));
export const installDlc = (g: StoreGame, id: string, title: string) =>
  run(`Couldn’t install ${title}`, () => api.storeInstallDlc(g.key, id), dlcQueuedNotice(title));
export const removeDlc = (g: StoreGame, id: string, title: string) =>
  run(`Couldn’t remove ${title}`, () => api.storeRemoveDlc(g.key, id), dlcRemovedNotice(title));

export async function cancelStoreJob(e: DownloadEntry, title: string): Promise<void> {
  if (await run(`Couldn’t cancel ${title}`, () => api.cancelInstall(e.id))) showCard(cancelledNotice(title));
}

/** Connect… / Sign in again…: the login window opens; the outcome is a `store-login` event. */
export async function signIn(source: StoreSource): Promise<void> {
  clearLoginError(source);
  await run('Couldn’t open the sign-in window', () => api.storeConnect(source));
}

export const installTool = (source: StoreSource) => run('Couldn’t install the store tool', () => api.storeInstallTool(source));
export const refreshStore = (source: StoreSource) => run('Couldn’t refresh the store', () => api.storeRefresh(source));

export async function queueInstall(game: StoreGame, fn: () => Promise<void>): Promise<boolean> {
  return run(`Couldn’t install ${game.title}`, fn, queuedNotice(game.title));
}

// ── App-lifetime wiring: store-notice, store-login and the RomM installer cards ──

/** The installer-phase cards Downloads rows earn (the wizard wait; RomM archive failures). */
export function installerCards(prev: ReadonlyMap<number, DownloadEntry>, entries: readonly DownloadEntry[]): StoreCard[] {
  const out: StoreCard[] = [];
  for (const e of entries) {
    const before = prev.get(e.id);
    if (before === undefined) continue;
    // Store rows' cards come from the backend's `store-notice`; only RomM
    // installs that run a GOG installer are worded here.
    if (e.job !== 'game') continue;
    if (e.phase === 'installer_wizard' && before.phase !== 'installer_wizard' && e.status !== 'failed') {
      out.push(wizardNotice(e.title, false));
    }
    if (e.status === 'failed' && before.status !== 'failed') {
      const c = installerFailedNotice(e);
      if (c) out.push(c);
    }
  }
  return out;
}

let wired: (() => void) | null = null;

/** Shows every `store-notice` and the sign-in outcomes as desktop cards. Once per shell. */
export function initStoreCards(): () => void {
  if (wired) return wired;
  const offNotice = onStoreNotice((n: StoreNotice) => showCard(noticeCard(n), n.source));
  const offLogin = onStoreLogin((e, wasExpired) => {
    if (e.outcome === 'connected') showCard(connectedNotice(e.source, e.username, wasExpired), e.source);
    else if (e.outcome === 'failed') showCard(loginFailedNotice(e.source, e.error), e.source);
  });
  let prev = new Map<number, DownloadEntry>(downloads.entries.map((e) => [e.id, e]));
  const offSnap = onSnapshot((entries, first) => {
    if (!first) for (const c of installerCards(prev, entries)) showCard(c);
    prev = new Map(entries.map((e) => [e.id, e]));
  });
  wired = () => {
    offNotice();
    offLogin();
    offSnap();
    wired = null;
  };
  return wired;
}
