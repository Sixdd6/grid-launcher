// Steam, the store with no tool (GameStores README, "Steam"; prototype
// `stSteamCard`, `stSteamInstall`, the Steam branches of `stRenderView` and
// `stRenderDetails`; TVStores `sSteamPage`). The Steam client installs,
// uninstalls and starts its games; GRID needs a SteamID64 and the user's own
// Web API key only to list the games they own and haven't installed. Pure:
// the Settings card's model and every Steam sentence, for the desktop and TV.
//
// The Web API key never passes through here: a card shows a saved key as dots
// only (user decision 2), and the key field's value goes straight from the
// input to `steamTest` / `steamSave`.
import type { SteamCard, SteamTestResult, StoreStatus } from '../api';
import { plural } from './text';

/** Steam's card from a status, or `null` (another store, or a status from before the field existed). */
export function steamCard(status: StoreStatus | null): SteamCard | null {
  return status !== null && status.source === 'steam' ? (status.steam ?? null) : null;
}

/** Whether Steam's status says the client isn't installed (no Steam found on this computer). */
export function steamClientMissing(status: StoreStatus | null): boolean {
  const card = steamCard(status);
  return card !== null && card.client_path.trim() === '';
}

/** The Web API key's state (`none` for a status with no card). */
export function steamKeyState(status: StoreStatus | null): SteamCard['key'] {
  return steamCard(status)?.key ?? 'none';
}

// ── Library and Details sentences ───────────────────────────────────────

export const MANAGED_BY_STEAM = 'Managed by Steam';
export const MANAGED_NOTE = 'Managed by Steam. Pause or cancel it in Steam.';
export const INSTALL_IN_STEAM = 'Install in Steam';
export const INSTALLING_IN_STEAM = 'Installing in Steam';
export const RUNNING_IN_STEAM = 'Running in Steam';
export const VERIFY_IN_STEAM = 'Verify in Steam';
export const UNINSTALL_IN_STEAM = 'Uninstall in Steam…';
export const OUTSIDE_GRID = 'Installed in Steam, outside GRID';
export const INSTALLED_BY_STEAM = 'Installed by Steam';
export const IN_STEAM_LIBRARIES = 'in your Steam libraries';
export const STEAM_NOT_INSTALLED = 'Steam isn’t installed. Install Steam, then try again.';

/** The managed box in Details: "Installing in Steam · 52%". */
export function installingInSteam(pct: number): string {
  return `${INSTALLING_IN_STEAM} · ${pct}%`;
}

// ── Settings › Stores › Steam ───────────────────────────────────────────

/** The card's lede (prototype `stSteamCard`; TV's page sub). */
export const STEAM_LEDE =
  'Installed Steam games show in the Library as soon as Steam is set up here, including games you install in Steam itself. To list the games you own but haven’t installed, GRID needs your SteamID64 and your own Steam Web API key.';
export const STEAM_TV_LEDE =
  'Installed Steam games show as soon as Steam is set up here, including games you install in Steam itself. To list the games you own but haven’t installed, GRID needs your SteamID64 and your own Web API key.';

/** The pill: Connected, Installed games only (connected without a working key), Not connected. */
export function steamPill(status: StoreStatus): { text: string; tone: 'ok' | 'warn' | 'off' } {
  if (status.connection === 'not_connected') return { text: 'Not connected', tone: 'off' };
  return steamKeyState(status) === 'ok' ? { text: 'Connected', tone: 'ok' } : { text: 'Installed games only', tone: 'warn' };
}

/** The line under "Steam": "SteamID64 76561198000000001 · SamPlays", or "Not set up". */
export function steamIdLine(status: StoreStatus): string {
  if (status.connection === 'not_connected') return 'Not set up';
  const id = steamCard(status)?.steam_id ?? '';
  const persona = status.username.trim();
  return persona === '' ? `SteamID64 ${id}` : `SteamID64 ${id} · ${persona}`;
}

/** TV's Settings › Stores row: "SteamID64 76561198000000001", or "Not set up". */
export function steamRowSub(status: StoreStatus): string {
  if (status.connection === 'not_connected') return 'Not set up';
  return `SteamID64 ${steamCard(status)?.steam_id ?? ''}`;
}

/**
 * The client line's place: "/usr/bin/steam", or for a Flatpak or Snap Steam
 * the backend's "Flatpak (com.valvesoftware.Steam)" (user decision 3).
 * `null` when Steam isn't installed.
 */
export function steamClientPlace(status: StoreStatus): string | null {
  const p = steamCard(status)?.client_path.trim() ?? '';
  return p === '' ? null : p;
}

/** Whether the client line's place is a path (shown as code) rather than a package ("Flatpak (…)"). */
export function clientPlaceIsPath(place: string): boolean {
  return place.startsWith('/') || /^[A-Za-z]:[\\/]/.test(place) || place.startsWith('~');
}

export const CLIENT_FOUND = 'Steam client found';
export const CLIENT_MISSING_LINE =
  'Steam isn’t installed. GRID installs, starts and uninstalls Steam games through the Steam client. Install Steam, sign in once, then come back.';

/** The account radios' notes (prototype `ST.stores.steam.accounts`). */
export function accountNote(mostRecent: boolean): string {
  return mostRecent ? 'signed in to Steam on this computer' : 'has signed in on this computer before';
}

/** TV's account sheet writes its notes with a capital. */
export function accountNoteTv(mostRecent: boolean): string {
  const n = accountNote(mostRecent);
  return n.charAt(0).toUpperCase() + n.slice(1);
}

export const ENTER_STEAM_ID = 'Enter a SteamID64';
export const STEAM_ID_PLACEHOLDER = '17 digits, starting 7656';
export const ACCOUNT_HELP = 'Found in Steam’s files on this computer. A SteamID64 is the long number in your profile’s address.';
export const KEY_PLACEHOLDER = 'Paste your 32-character key';
export const KEY_URL = 'https://steamcommunity.com/dev/apikey';
export const KEY_HELP_BEFORE = 'Make one at ';
export const KEY_HELP_LINK = 'steamcommunity.com/dev/apikey';
export const KEY_HELP_AFTER = ' (any domain name works). GRID keeps it in the OS keyring and sends it only to Steam.';
export const KEY_TV_SUB = 'From steamcommunity.com/dev/apikey (any domain name works). Kept in the OS keyring, sent only to Steam.';
/** A saved key: dots only, never any part of the key (user decision 2). */
export const SAVED_KEY_DOTS = '••••••••••••••••••••••••••••••••';
export const SAVED_KEY_TV = '••••••••';

/**
 * Whether the card shows the saved key (dots + Change… / Remove) or the
 * password field: the saved row only while the key works (prototype
 * `editKey: s.key !== 'ok'`). A rejected or private key shows the field, so a
 * new key can be pasted; Save with the field empty keeps the saved one.
 */
export function keySaved(status: StoreStatus): boolean {
  return status.connection !== 'not_connected' && steamKeyState(status) === 'ok';
}

/** The SteamID64 the radios start on: the saved one, else the most recent local account, else the first. */
export function initialPick(status: StoreStatus): string {
  const card = steamCard(status);
  if (card === null) return 'manual';
  if (card.steam_id.trim() !== '') {
    return card.accounts.some((a) => a.steam_id === card.steam_id) ? card.steam_id : 'manual';
  }
  return card.accounts.find((a) => a.most_recent)?.steam_id ?? card.accounts[0]?.steam_id ?? 'manual';
}

/** The manual field's first value: the saved SteamID64 when no local account has it. */
export function initialManual(status: StoreStatus): string {
  const card = steamCard(status);
  if (card === null) return '';
  return card.accounts.some((a) => a.steam_id === card.steam_id) ? '' : card.steam_id;
}

/** A Test's result as the card's line (desktop) or a card's two lines (TV). */
export type TestLine = { ok: boolean; text: string; help?: string };

/**
 * The five Test results in the design's words. A rejected key carries no
 * status code (user decision 1).
 */
export function testLine(r: SteamTestResult): TestLine {
  switch (r.kind) {
    case 'ok':
      return { ok: true, text: `The key works · ${plural(r.owned_count, 'game')} owned` };
    case 'bad_key':
      return { ok: false, text: 'Steam rejected the key. Check that you copied all 32 characters, or make a new key.' };
    case 'private':
      return {
        ok: false,
        text: 'The key works, but this profile’s game details are private, so Steam lists no games.',
        help: 'In Steam: your profile › Edit Profile › Privacy Settings › set Game details to Public. Then test again.',
      };
    case 'bad_steam_id':
      return { ok: false, text: 'That isn’t a SteamID64. It has 17 digits and starts with 7656.' };
    default:
      return { ok: false, text: 'Couldn’t reach api.steampowered.com. Check your connection and try again.' };
  }
}

/** TV shows a Test result as a card: a title and its second line. */
export function testCardTv(r: SteamTestResult): { text: string; sub?: string; error: boolean } {
  switch (r.kind) {
    case 'ok':
      return { text: 'The key works', sub: `${plural(r.owned_count, 'game')} owned`, error: false };
    case 'bad_key':
      return { text: 'Steam rejected the key', sub: 'Check that you entered all 32 characters, or make a new key.', error: true };
    case 'private':
      return {
        text: 'This profile’s game details are private',
        sub: 'In Steam: your profile › Edit Profile › Privacy Settings › Game details: Public. Then test again.',
        error: true,
      };
    case 'bad_steam_id':
      return { text: 'That isn’t a SteamID64', sub: 'It has 17 digits and starts with 7656.', error: true };
    default:
      return { text: 'Couldn’t reach api.steampowered.com', sub: 'Check your connection and try again.', error: true };
  }
}

export const ASKING_STEAM = 'Asking Steam…';

/** The SteamID64 a Save or Test sends: the picked account, or the manual field trimmed. */
export function pickedSteamId(pick: string, manual: string): string {
  return pick === 'manual' ? manual.trim() : pick;
}

/** Save / Connect Steam: the main button's label. */
export function saveLabel(status: StoreStatus): string {
  return status.connection === 'not_connected' ? 'Connect Steam' : 'Save';
}

export const PASTE_KEY_FIRST = 'Paste your key first.';

/** The inline Disconnect confirm (desktop) and TV's confirm box. */
export const STEAM_DISCONNECT = {
  bold: 'Disconnect Steam?',
  rest: 'GRID forgets your SteamID64 and deletes your Web API key from the OS keyring. Steam games leave the Library; they stay installed in Steam.',
};
