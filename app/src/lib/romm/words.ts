// RomM's words, shared by desktop and TV (decision 17: where the two
// prototypes differ, desktop wins). The source is
// docs/design/system/components/RommStorefront/README.md; change both together.
// Pure: every function takes the status (or plain values) and returns text.
import type { RommStatus } from '../api';
import { hostOf } from '../shell';

/** The server's host for a line ("romm.home.lan"); `''` when none is stored. */
export function rommHost(status: Pick<RommStatus, 'server_url'>): string {
  return status.server_url === '' ? '' : hostOf(status.server_url);
}

/**
 * The state the card, the rail flag and the needs-RomM lines are worded for.
 * `connecting`: a probe runs while RomM is not connected (the startup
 * restore of a stored connection, or a connect); `reconnecting`: a probe runs
 * from Can't reach or Sign-in expired.
 */
export type RommState = 'connected' | 'connecting' | 'unreachable' | 'expired' | 'not_connected' | 'never';

export function rommState(status: RommStatus): RommState {
  if (status.connection === 'connected') return 'connected';
  if (status.busy) return 'connecting';
  if (status.connection === 'unreachable') return 'unreachable';
  if (status.connection === 'expired') return 'expired';
  return status.server_url === '' ? 'never' : 'not_connected';
}

// --- Settings › Game stores › RomM card -----------------------------------

export type RommPillTone = 'ok' | 'busy' | 'warn' | 'off';
export type RommPill = { text: string; tone: RommPillTone };

export function rommPill(status: RommStatus): RommPill {
  switch (rommState(status)) {
    case 'connected':
      return { text: 'Connected', tone: 'ok' };
    case 'connecting':
      return { text: 'Connecting…', tone: 'busy' };
    case 'unreachable':
      return { text: "Can't reach", tone: 'warn' };
    case 'expired':
      return { text: 'Sign-in expired', tone: 'warn' };
    default:
      return { text: 'Not connected', tone: 'off' };
  }
}

/** The card's sub-line under "RomM". */
export function rommAccountLine(status: RommStatus): string {
  const host = rommHost(status);
  if (host === '') return 'Your own game server';
  if (status.connection === 'not_connected' && !status.busy) return `Not signed in · ${host}`;
  return status.username !== '' ? `Signed in as ${status.username} · ${host}` : host;
}

/** The card's Sign-in field. */
export function rommSignInKind(status: Pick<RommStatus, 'auth_kind'>): string {
  return status.auth_kind === 'basic' ? 'Password · kept in the OS keyring' : 'API token · kept in the OS keyring';
}

/** The card's Games field: "45 on the server · 11 installed here". */
export function rommGamesLine(onServer: number, installed: number): string {
  return `${onServer} on the server · ${installed} installed here`;
}

/** The card's warning box for Can't reach and Sign-in expired; `null` otherwise. */
export function rommCardWarning(status: RommStatus): { strong: string; text: string } | null {
  const host = rommHost(status);
  if (status.connection === 'unreachable') {
    const why = status.last_error !== '' ? `${host}: ${status.last_error}.` : `${host} didn't answer.`;
    return {
      strong: "Can't reach RomM.",
      text: `${why} Installed RomM games still play; browsing, installs, cloud saves and firmware downloads wait until it's back.`,
    };
  }
  if (status.connection === 'expired') {
    return {
      strong: 'Your RomM sign-in expired.',
      text: 'Sign in again to browse, install and sync saves. Installed games still play.',
    };
  }
  return null;
}

/** The card's Status field. */
export function rommStatusLine(status: RommStatus): string {
  if (status.connection === 'connected') return 'Connected';
  if (status.connection === 'unreachable') {
    return status.last_error !== '' ? `Can't reach · ${status.last_error}` : "Can't reach";
  }
  if (status.connection === 'expired') return 'Sign-in expired · RomM answered 401';
  return 'Not connected';
}

/** The not-connected card's body. */
export function rommNotConnectedText(installed: number): string {
  const base =
    'Connect your RomM server to browse its games in the Library, install them with an emulator, and sync their saves and firmware.';
  if (installed <= 0) return base;
  const games = installed === 1 ? 'The 1 RomM game installed here still plays.' : `The ${installed} RomM games installed here still play.`;
  return `${base} ${games}`;
}

export const ROMM_CARD_ACTIONS = {
  showInLibrary: 'Show in Library',
  edit: 'Edit…',
  reconnect: 'Reconnect',
  reconnecting: 'Reconnecting…',
  retry: 'Retry',
  retrying: 'Retrying…',
  signInAgain: 'Sign in again…',
  openInBrowser: 'Open in browser',
  disconnect: 'Disconnect…',
  connect: 'Connect RomM…',
} as const;

export const ROMM_DISCONNECT_CONFIRM = {
  title: 'Disconnect RomM?',
  text: 'GRID deletes your RomM sign-in from the OS keyring and stays open. Installed RomM games stay and still play. Cloud saves, firmware downloads and updates stop.',
  confirm: 'Disconnect',
  cancel: 'Cancel',
} as const;

// --- Notifications ----------------------------------------------------------

export const rommToasts = {
  connected: (host: string, user: string) => `Connected to RomM at ${host} as ${user}`,
  signedInAgain: (user: string) => `Signed in to RomM again as ${user}`,
  reconnected: (host: string) => `Reconnected to ${host}`,
  backAgain: (host: string) => `Connected to RomM again · ${host}`,
  disconnected: 'RomM disconnected. Installed RomM games stay and still play.',
} as const;

// --- Library › RomM group ---------------------------------------------------

export const ROMM_HEADING = 'ROMM';

/** The rail heading's flag title per state (the flag itself is in `rail.ts`). */
export const ROMM_FLAG_TITLES = {
  connecting: 'Connecting to RomM',
  unreachable: "Can't reach RomM",
  expired: 'RomM sign-in expired',
  off: 'Not connected',
} as const;

/** The one-line banner above RomM › Installed; `null` when there is none. */
export type RommBanner = { kind: 'unreachable' | 'expired'; strong: string; text: string; action: string; busyAction: string | null };

export function rommBanner(status: RommStatus): RommBanner | null {
  const host = rommHost(status);
  if (status.connection === 'unreachable') {
    return {
      kind: 'unreachable',
      strong: "Can't reach RomM",
      text: `${host} didn't answer. Installed games still play.`,
      action: 'Retry',
      busyAction: 'Retrying…',
    };
  }
  if (status.connection === 'expired') {
    return {
      kind: 'expired',
      strong: 'RomM sign-in expired',
      text: 'Sign in again to browse, install and sync saves. Installed games still play.',
      action: 'Sign in again…',
      busyAction: null,
    };
  }
  return null;
}

/** RomM › Installed header chip; `null` for Can't reach and Sign-in expired (the banner carries them). */
export function rommInstalledChip(status: RommStatus): string | null {
  const state = rommState(status);
  if (state === 'connected') return `Signed in as ${status.username}`;
  if (state === 'connecting') return `Connecting to ${rommHost(status)}…`;
  if (state === 'not_connected' || state === 'never') return 'Not connected · Connect…';
  return null;
}

export const ROMM_INSTALLED = {
  title: 'RomM · Installed',
  search: 'Search by title',
  searchFolded: 'Search installed RomM games…',
  emptyTitle: 'No RomM games installed yet',
  emptyConnected: 'Install games from Discover or a platform under it.',
  emptyOff: "Install games from RomM once it's connected again.",
  openDiscover: 'Open Discover',
  noMatch: (query: string) => `No installed RomM games match "${query}"`,
} as const;

/** "11 games · 21.2 GB · from romm.home.lan" (size already formatted). */
export function rommInstalledLine(count: number, size: string, host: string): string {
  const games = count === 1 ? '1 game' : `${count} games`;
  const parts = [games];
  if (size !== '') parts.push(size);
  if (host !== '') parts.push(`from ${host}`);
  return parts.join(' · ');
}

/** Discover while the startup restore runs. */
export function rommConnectingPanel(host: string): { title: string; text: string } {
  return {
    title: `Connecting to ${host}…`,
    text: 'Discover and the platforms show when RomM answers. Installed games play now.',
  };
}

export const ROMM_DISCOVER_LEDE = 'New on RomM.';

/** The platform screen's counts line: "4 games on RomM · 2 installed · 1 only on this computer". */
export function rommPlatformCountsLine(onRomm: number, installed: number, onlyHere: number): string {
  const parts = [onRomm === 1 ? '1 game on RomM' : `${onRomm} games on RomM`];
  if (installed > 0) parts.push(`${installed} installed`);
  if (onlyHere > 0) parts.push(`${onlyHere} only on this computer`);
  return parts.join(' · ');
}

/** Cover tags for installed rows the server doesn't have (decision 15). */
export const ROMM_COVER_TAGS = { not_linked: 'NOT LINKED', not_on_romm: 'NOT ON ROMM' } as const;

// --- Needs-RomM states --------------------------------------------------------

/** Why RomM can't do it right now: the first sentence of every needs-RomM panel. */
export function rommWhy(status: RommStatus): string {
  const host = rommHost(status);
  switch (rommState(status)) {
    case 'connecting':
      return `GRID is still connecting to ${host}.`;
    case 'unreachable':
      return `GRID can't reach ${host} right now.`;
    case 'expired':
      return 'Your RomM sign-in expired.';
    default:
      return "RomM isn't connected.";
  }
}

/** The fix action beside a needs-RomM panel; `null` while connecting. */
export function rommFixAction(status: RommStatus): 'Open Game stores' | 'Retry' | 'Sign in again…' | null {
  switch (rommState(status)) {
    case 'connected':
    case 'connecting':
      return null;
    case 'unreachable':
      return 'Retry';
    case 'expired':
      return 'Sign in again…';
    default:
      return 'Open Game stores';
  }
}

/** Per-feature needs-RomM titles and lines. */
export const NEEDS_ROMM = {
  cloudSaves: 'Cloud saves need RomM',
  achievements: 'Achievements need RomM',
  achievementProgress: 'Achievement progress needs RomM',
  updates: 'Updates need RomM',
  installFromRomm: 'Install from RomM needs RomM',
  linking: 'Linking needs RomM',
} as const;

/** Details › Saves panel text after the why. */
export const DETAILS_SAVES_TEXT =
  "This game's saves stay on this computer and still work; GRID uploads and restores them when RomM is back.";
/** Details › Achievements panel text after the why. */
export const DETAILS_ACHIEVEMENTS_TEXT =
  'Your progress shows here from RomM; achievements still unlock in the emulator.';
/** Settings › Cloud saves panel text after the why. */
export const SETTINGS_CLOUD_TEXT =
  "GRID keeps your emulator saves and states on your RomM server. These settings apply when it's connected. GOG and Epic games sync with their own store's cloud.";
/** Settings › RetroAchievements panel text after the why. */
export const SETTINGS_RA_TEXT =
  'Details › Achievements shows your progress from RomM. Signing in here still turns on achievements in RetroArch, DuckStation and Dolphin.';

/** Details line for an installed row with no RomM link. */
export function notLinkedLine(connected: boolean): { text: string; action: string | null } {
  return connected
    ? { text: 'Not linked to a RomM game', action: 'Link…' }
    : { text: 'Not linked to a RomM game · Linking needs RomM', action: null };
}

/** Details › Saves for an unlinked row while connected. */
export function linkToSyncPanel(title: string, host: string): { title: string; text: string; action: string } {
  return {
    title: 'Link this game to sync its saves',
    text: `Cloud saves are kept per RomM game. Link ${title} to its game on ${host} first.`,
    action: 'Link…',
  };
}

/** Details line for a row from another RomM server. */
export function otherServerLine(rowServerHost: string, liveHost: string): string {
  return `From another RomM server (${rowServerHost}) · not on ${liveHost}`;
}

/** Appearance › Open GRID on, when RomM is off and the choice isn't Library. */
export const OPEN_ON_ROMM_OFF = "RomM isn't connected, so GRID opens on All games.";

// --- Sign-in form (RommSignInForm) ---------------------------------------------

export type SignInMode = 'connect' | 'edit' | 'again';

export function signInCopy(mode: SignInMode, username = '', useToken = true): { title: string; lede: string; button: string } {
  switch (mode) {
    case 'edit':
      return {
        title: 'Edit RomM sign-in',
        lede: 'Change the server or the sign-in. Installed RomM games stay; GRID links them to the server\'s games when it connects.',
        button: 'Save and connect',
      };
    case 'again':
      return {
        title: 'Sign in to RomM again',
        lede: `RomM no longer accepts GRID's ${useToken ? 'token' : 'password'} for ${username}. Enter a new one.`,
        button: 'Sign in',
      };
    default:
      return {
        title: 'Connect to RomM',
        lede: 'GRID browses your RomM server, installs its games with an emulator and syncs their saves.',
        button: 'Connect',
      };
  }
}

export const SIGN_IN_FIELDS = {
  serverUrl: 'Server URL',
  serverPlaceholder: 'https://romm.example',
  useToken: 'Use an API token',
  username: 'Username',
  token: 'API token',
  password: 'Password',
  tokenHelp: 'Make a token in RomM under your user settings. GRID keeps it in the OS keyring, never in a file.',
  passwordHelp: 'GRID keeps your password in the OS keyring, never in a file.',
  busy: 'Connecting…',
} as const;

// --- Welcome ------------------------------------------------------------------

export const WELCOME_ROMM_ROW = {
  name: 'RomM',
  sub: 'Your own game server. GRID browses it and installs its games with an emulator.',
  action: 'Connect…',
  connected: (user: string, host: string) => `Connected as ${user} · ${host}`,
} as const;
