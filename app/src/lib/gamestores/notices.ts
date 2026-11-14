// The words of every store notification card (GameStores README, "Downloads
// and notifications", "Launch", "Login window", "RomM games with a GOG
// installer"). The backend sends most of them as `store-notice` events with
// their text; this module names their action buttons and words the cards the
// frontend raises itself (launch, cloud choices, refusals before a command
// runs, and the RomM installer-archive cards it reads from Downloads rows).
// Pure and UI-agnostic: the desktop's cards and TV mode's both use it.
import type { DownloadEntry, StoreNotice, StoreNoticeAction, StoreSource } from '../api';
import { hasCloud, managedByClient, storeName, storeTool } from './marks';
import { CLIENT_FOUND, STEAM_NOT_INSTALLED } from './steam';

/** One card: its text, whether it is an error, and at most one action. */
export type StoreCard = { text: string; error: boolean; action: StoreNoticeAction | null; sub?: string };

/** The button a `store-notice` action shows. */
export function noticeActionLabel(action: StoreNoticeAction, source: StoreSource | null = null): string {
  switch (action.kind) {
    case 'play':
      return 'Play';
    case 'open_downloads':
      return 'Open Downloads';
    case 'try_again':
      return 'Try again';
    case 'sign_in':
      return 'Sign in again';
    case 'install_tool': {
      const tool = source === null ? null : storeTool(source);
      return tool === null ? 'Install the store tool' : `Install ${tool.name}`;
    }
    case 'show_updates':
      return 'Show';
    case 'install_base':
      return action.title && action.title.trim() !== '' ? `Install ${action.title.trim()}` : 'Install the base game';
    case 'open_settings':
      return 'Open Settings';
    default:
      return '';
  }
}

/** A backend notice as a card: its message (and detail as the second line). */
export function noticeCard(n: StoreNotice): StoreCard {
  const card: StoreCard = { text: n.message, error: n.error, action: n.action };
  if (n.detail.trim() !== '') card.sub = n.detail.trim();
  return card;
}

const ok = (text: string, action: StoreNoticeAction | null = null): StoreCard => ({ text, error: false, action });
const err = (text: string, action: StoreNoticeAction | null = null): StoreCard => ({ text, error: true, action });

// ── Launch and cloud saves ──────────────────────────────────────────────

export const syncingNotice = (title: string) => ok(`Syncing cloud saves for ${title}…`);
export const launchingNotice = (title: string) => ok(`Launching ${title}…`);
export const steamStartNotice = (title: string) => ok(`Starting ${title} through Steam…`);
export const stoppedNotice = (title: string) => ok(`Stopped ${title}`);
export const cloudSyncedNotice = (title: string) => ok(`Cloud saves synced for ${title}`);
export const keptLocalNotice = (title: string) => ok(`Kept this computer’s save for ${title} and uploaded it`);
export const restoredCloudNotice = (title: string) => ok(`Restored the cloud save for ${title}`);
export const launchFailedNotice = (title: string, reason: string) =>
  err(reason.trim() === '' ? `Couldn’t launch ${title}` : `Couldn’t launch ${title}: ${reason.trim()}`);

// ── Install, update, repair, DLC, uninstall ─────────────────────────────

export const queuedNotice = (title: string) => ok(`${title} added to download queue`, { kind: 'open_downloads' });
export const updateQueuedNotice = (title: string) => ok(`Update for ${title} added to download queue`, { kind: 'open_downloads' });
export const verifyingNotice = (title: string) => ok(`Verifying ${title}’s files…`);
export const dlcQueuedNotice = (dlcTitle: string) => ok(`${dlcTitle} added to download queue`, { kind: 'open_downloads' });
export const dlcRemovedNotice = (dlcTitle: string) => ok(`${dlcTitle} removed`);
export const cancelledNotice = (title: string) => ok(`Cancelled ${title}`);

/** "Fogline uninstalled"; a store with cloud saves adds where they stay (never Amazon's, which has none). */
export function uninstalledNotice(source: StoreSource, title: string, cloud: boolean): StoreCard {
  return ok(`${title} uninstalled${cloud && hasCloud(source) ? `. Its cloud saves stay in your ${storeName(source)} account.` : ''}`);
}

/** Install pressed while signed out: the dialog doesn't open. */
export function signInToInstallNotice(source: StoreSource, title: string): StoreCard {
  return err(`Sign in to ${storeName(source)} again to install ${title}`, { kind: 'sign_in' });
}

/** Install pressed with the store tool missing. */
export function toolNeededNotice(source: StoreSource): StoreCard {
  const tool = storeTool(source);
  return err(`${storeName(source)} needs ${tool?.name ?? 'its tool'} to install games`, { kind: 'install_tool' });
}

export const removedFromDriveNotice = (title: string, pack: string) => ok(`Removed ${title} from ${pack}`);
export const gameSettingsSavedNotice = () => ok('Game Settings saved');

export function storeCloudSwitchNotice(source: StoreSource, on: boolean): StoreCard {
  const name = storeName(source);
  return ok(on ? `Cloud saves on for ${name} games` : `Cloud saves off for ${name} games. Your saves stay in your account.`);
}

/** A command that failed before anything was queued. */
export function refusedNotice(what: string, reason: string): StoreCard {
  return err(reason.trim() === '' ? what : `${what}: ${reason.trim()}`);
}

// ── Login (the main window's side of it) ────────────────────────────────

export function connectedNotice(source: StoreSource, username: string, wasExpired: boolean): StoreCard {
  return ok(`${wasExpired ? 'Signed in to' : 'Connected to'} ${storeName(source)} as ${username}`);
}

/** A failed sign-in closes the window; the error card offers Try again. */
export function loginFailedNotice(source: StoreSource, error: string): StoreCard {
  const reason = error.trim().replace(/^Sign-in didn’t finish: /, '');
  return err(`Couldn’t sign in to ${storeName(source)}${reason === '' ? '' : `: ${reason}`}`, { kind: 'try_again' });
}

export function disconnectedNotice(source: StoreSource): StoreCard {
  if (managedByClient(source)) return ok(`${storeName(source)} disconnected. Your key was deleted from the keyring.`);
  const tool = storeTool(source);
  return ok(`${storeName(source)} disconnected. Its login and ${tool?.name ?? 'its tool'} were deleted; installed games stay.`);
}

// ── Offline installers and RomM installer archives ─────────────────────

/** The card when a Windows installer couldn't run silently and its own window opened. */
export function wizardNotice(title: string, offline: boolean): StoreCard {
  return offline
    ? ok(`The ${title} installer couldn’t run silently, so its setup window opened. Finish it there; GRID adds the game when it closes.`)
    : ok(`${title}’s installer couldn’t run silently, so its setup window opened. Finish it there; GRID adds the game when it closes.`);
}

/**
 * A RomM install that failed in its archive's installer: "Couldn’t install
 * Iron Lullaby: its archive holds an installer GRID doesn’t recognise" ·
 * Open Downloads, or "…: it’s DLC for Pale Summit, which isn’t installed" ·
 * Install Pale Summit. `null` for a row with no installer failure.
 */
export function installerFailedNotice(e: DownloadEntry): StoreCard | null {
  if (e.status !== 'failed' || e.failure === null) return null;
  const f = e.failure;
  switch (f.code) {
    case 'installer_unrecognised':
      return err(`Couldn’t install ${e.title}: its archive holds an installer GRID doesn’t recognise`, { kind: 'open_downloads' });
    case 'installer_dlc_base_missing': {
      const base = f.base_title.trim();
      const action: StoreNoticeAction =
        base === ''
          ? { kind: 'open_downloads' }
          : { kind: 'install_base', title: base, ...(f.base_gog_id.trim() ? { key: `gog:${f.base_gog_id.trim()}` } : {}) };
      return err(`Couldn’t install ${e.title}: it’s DLC for ${base || 'a game'}, which isn’t installed`, action);
    }
    case 'installer_exit':
      return err(`Couldn’t install ${e.title}: the installer stopped with an error`, { kind: 'open_downloads' });
    case 'installer_silent_failed':
      return err(`Couldn’t install ${e.title}: its installer couldn’t run silently`, { kind: 'open_downloads' });
    default:
      return null;
  }
}

// ── Steam: everything goes through the Steam client ────────────────────
// The cards for what the user pressed; Steam's background cards (installed
// · Play, connected, uninstalled) come from the backend's `store-notice`.

/** Install in Steam pressed: Steam's window opens; GRID follows the install. */
export const steamInstallNotice = (title: string) =>
  ok(`Steam opened to install ${title}. Choose the library folder there; GRID shows the progress.`);
/** Install in Steam (or Play) with no Steam on this computer. */
export const steamMissingNotice = () => err(STEAM_NOT_INSTALLED);
export const steamVerifyNotice = (title: string) => ok(`Steam is verifying ${title}’s files. Watch it in Steam.`);
export const steamUninstallNotice = (title: string) =>
  ok(`Steam opened its uninstall window for ${title}. It leaves the Library once Steam removes it.`);
/** "Running in Steam" pressed: GRID can't stop a game Steam runs. */
export const steamRunningNotice = (title: string) => ok(`${title} is running in Steam. Quit it from the game.`);
export const steamClientFoundNotice = () => ok(CLIENT_FOUND);
export const steamKeyRemovedNotice = () => ok('Web API key removed from the keyring. Steam’s Available list needs it.');
