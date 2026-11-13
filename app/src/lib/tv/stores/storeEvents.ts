// TV mode's side of the store events (TVStores README § Downloads and
// notifications, § Signing in, § RomM games with a GOG installer): while the
// TV shell is up it shows each `store-notice` as a card (an action taken
// with View), closes the login layer when `store-login` says how the
// sign-in ended (connected, failed with Try again, or cancelled silently),
// and raises the installer failure cards from Downloads rows ("Couldn’t
// install Foxglove silently" with Retry). The shared `gameStores` store
// (E1) owns the data; the desktop shell subscribes its own way.
import type { DownloadEntry, StoreNoticeAction, StoreSource } from '../../api';
import { connectedNotice, loginFailedNotice, noticeActionLabel } from '../../gamestores/notices';
import { gameStores, initGameStores, onStoreLogin, onStoreNotice } from '../../stores/gameStores.svelte';
import { onSnapshot } from '../../stores/downloads.svelte';
import { pushToast, TOAST_DURATION_MS, type ToastAction } from '../../stores/toasts.svelte';
import { installGame } from '../gameActions';
import { showTab } from '../nav.svelte';
import { selectLibRail } from '../tabs/library/library.svelte';
import { loadAllGames } from '../tabs/server/server.svelte';
import { bumpDrives } from './storeState.svelte';
import { card, fail, goStoreSettings, installTool, loginFinished, openInstall, playStore, startLogin, toDownloads } from './storeActions';
import { tvFailureCard } from './storeWords';
import { api } from '../../api';

/** What a notice's action does in TV mode (View takes it). */
export function noticeAction(action: StoreNoticeAction | null, source: StoreSource | null): ToastAction | undefined {
  if (action === null) return undefined;
  const label = noticeActionLabel(action, source);
  const src: StoreSource = source ?? 'gog';
  switch (action.kind) {
    case 'play':
      return {
        label,
        run: () => {
          const g = action.key ? gameStores.game(action.key) : null;
          if (g) void playStore(g);
        },
      };
    case 'open_downloads':
      return toDownloads;
    case 'try_again':
    case 'sign_in':
      return { label, run: () => startLogin(src) };
    case 'install_tool':
      return { label, run: () => installTool(src) };
    case 'show_updates':
      return {
        label,
        run: () => {
          selectLibRail('updates');
          void showTab('library', { again: true });
        },
      };
    case 'install_base':
      return { label, run: () => void installBase(action.title ?? '', action.key) };
    case 'open_settings':
      return { label, run: goStoreSettings };
    default:
      return undefined;
  }
}

/** "Install Pale Summit": the GOG game's install page, else the RomM game of that name. */
async function installBase(title: string, key?: string): Promise<void> {
  const g = key ? gameStores.game(key) : null;
  if (g) {
    openInstall(g);
    return;
  }
  try {
    const all = await loadAllGames();
    const hit = all.games.find((x) => x.game.name.trim().toLowerCase() === title.trim().toLowerCase());
    if (hit) {
      await installGame(hit.game.id, hit.game.name);
      return;
    }
  } catch {
    // Falls through to the card below.
  }
  card(`Couldn’t find ${title} on the server`, 'error');
}

/** The card a failed installer row raises (TV mode's words; `null` when none). */
function failureCard(e: DownloadEntry): void {
  const c = tvFailureCard(e);
  if (!c) return;
  let action: ToastAction | undefined;
  if (c.action?.kind === 'retry') action = { label: 'Retry', run: () => void api.retryInstall(e.id).catch(fail) };
  else if (c.action?.kind === 'open_downloads') action = toDownloads;
  else if (c.action?.kind === 'install_base') {
    const a = c.action;
    action = { label: `Install ${a.title}`, run: () => void installBase(a.title, a.key) };
  }
  pushToast(c.text, 'error', TOAST_DURATION_MS, { sub: c.sub, ...(action ? { action } : {}) });
}

/** Starts the store events for the TV shell's life; answers the function that stops them. */
export function startStoreEvents(): () => void {
  // The store's own listeners live as long as the app (either shell may
  // start them; the second call shares the first).
  void initGameStores().catch(() => {});
  const offNotice = onStoreNotice((n) => {
    pushToast(n.message, n.error ? 'error' : 'success', TOAST_DURATION_MS, {
      ...(n.detail.trim() ? { sub: n.detail.trim() } : {}),
      ...(noticeAction(n.action, n.source) ? { action: noticeAction(n.action, n.source) } : {}),
    });
  });
  const offLogin = onStoreLogin((e, wasExpired) => {
    void loginFinished(e.source);
    if (e.outcome === 'connected') {
      const c = connectedNotice(e.source, e.username, wasExpired);
      card(c.text);
    } else if (e.outcome === 'failed') {
      const c = loginFailedNotice(e.source, e.error);
      card(c.text, 'error', undefined, { label: 'Try again', run: () => startLogin(e.source) });
    }
  });
  // Rows that newly failed in an installer, and drive rows that ended.
  let book = new Map<number, string>();
  const offSnap = onSnapshot((entries, first) => {
    if (!first) {
      for (const e of entries) {
        const was = book.get(e.id);
        if (was === undefined || was === e.status) continue;
        // A RomM row's installer failure has no shared card (the shared
        // notifier skips a row with `failure`), so TV mode words each one.
        // A store row's failures are the backend's `store-notice` cards,
        // except the silent-only rule, which the backend leaves to TV mode.
        if (e.status === 'failed' && e.failure && (e.job === 'game' || e.failure.code === 'installer_needs_wizard')) failureCard(e);
        if (e.status === 'completed' && (e.kind === 'store_restore' || e.kind === 'backup_store_game')) bumpDrives();
      }
    }
    book = new Map(entries.map((e) => [e.id, e.status]));
  });
  return () => {
    offNotice();
    offLogin();
    offSnap();
  };
}
