// TV mode's welcome (first run; replaces Connect. TVRommStorefront README §
// The welcome; prototype `rWelcome`): step 1 the library folder (required),
// step 2 "Connect your games" (optional), the storefronts in name order with
// their pills, and one button that reads Skip for now until something is
// connected, then Continue. Pure: its words, rows and rules. Where the TV and
// desktop cards' words differ, desktop's win (decision 17).
import type { RommStatus, StoreSource, StoreStatus } from '../../api';
import { storeName } from '../../gamestores/marks';
import { STOREFRONT_ORDER, type Storefront } from '../../romm/order';
import { rommHost, WELCOME_ROMM_ROW } from '../../romm/words';
import { storeRowPill, storeRowSub } from '../stores/storeWords';

export const STEP_1 = 'Step 1 of 2';
export const STEP_2 = 'Step 2 of 2';
export const WELCOME_TITLE = 'Welcome to GRID';
export const WELCOME_LEDE = 'One Library for the games on your RomM server and the stores you buy from.';
export const LIBRARY_LABEL = 'Library folder';
export const LIBRARY_PLACEHOLDER = 'Choose a folder';
export const LIBRARY_LINE = 'Games, emulators and saves go here.';
export const LIBRARY_BROWSER_TITLE = 'Library folder';
export const NEXT = 'Next';
export const NEXT_WHY = 'Choose a folder to continue.';
export const IMPORTED_NOTE = 'Filled in from your old GRID settings.';
/** The dialog after a Python import (desktop's notice). */
export const IMPORTED_DIALOG = {
  title: 'Your settings from the previous version are filled in.',
  body: 'Enter your RomM token to reconnect, or skip it for now.',
} as const;

export const CONNECT_TITLE = 'Connect your games';
export const CONNECT_LEDE = 'Connect the ones you use. Add or change them any time in Settings › Game stores.';

/** Next is enabled once a folder is chosen. */
export function nextReady(lib: string): boolean {
  return lib.trim() !== '';
}

/** Where the folder browser opens for a path: the folder that holds it
 *  ('' is the home folder; the browser falls back to it when a folder is
 *  missing). */
export function parentOf(path: string): string {
  const p = path.trim().replace(/[\\/]+$/, '');
  const i = Math.max(p.lastIndexOf('/'), p.lastIndexOf('\\'));
  if (i < 0) return '';
  if (i === 0) return p.slice(0, 1);
  return p.slice(0, i);
}

export type WelcomeRow = {
  id: Storefront;
  name: string;
  sub: string;
  connected: boolean;
  /** The A hint: "Connect", "Change" when connected. */
  a: string;
};

/** The store rows' lines while not connected (desktop's words). */
export function storeOffLine(source: StoreSource): string {
  return source === 'steam' ? 'Installed Steam games show as soon as Steam is set up here.' : `Sign in on ${storeName(source)}'s own page. GRID never sees your password.`;
}

/** Whether a store counts as connected on the welcome: its Game stores pill is green. */
export function storeConnected(status: StoreStatus | null): boolean {
  return status !== null && storeRowPill(status).tone === 'ok';
}

/** The RomM row's line: connected, filled in from a Python import, or the pitch. */
export function rommLine(status: RommStatus, imported: boolean): string {
  if (status.connection === 'connected') return WELCOME_ROMM_ROW.connected(status.username, rommHost(status));
  if (imported && status.server_url !== '') {
    const who = status.username !== '' ? `${status.username} · ${rommHost(status)}` : rommHost(status);
    return `From your old GRID: ${who}. Sign in again.`;
  }
  return WELCOME_ROMM_ROW.sub;
}

/** The five rows in name order: Amazon Games, Epic Games, GOG, RomM, Steam. */
export function welcomeRows(romm: RommStatus, stores: (source: StoreSource) => StoreStatus | null, imported: boolean): WelcomeRow[] {
  return STOREFRONT_ORDER.map((id) => {
    if (id === 'romm') {
      const on = romm.connection === 'connected';
      return { id, name: 'RomM', sub: rommLine(romm, imported), connected: on, a: on ? 'Change' : 'Connect' };
    }
    const st = stores(id);
    const on = storeConnected(st);
    return { id, name: storeName(id), sub: on && st ? storeRowSub(st) : storeOffLine(id), connected: on, a: on ? 'Change' : 'Connect' };
  });
}

/** The one button: Skip for now until something is connected, then Continue. */
export function welcomeButton(rows: readonly WelcomeRow[]): string {
  return rows.some((r) => r.connected) ? 'Continue' : 'Skip for now';
}
