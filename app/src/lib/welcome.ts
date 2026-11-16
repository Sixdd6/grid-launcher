// The first-run welcome's rules and words (RommStorefront README, "Welcome
// (replaces Connect)"; prototype `romm.js` `rwRender` / `rwRow`). Pure, so the
// step rules, the row order and what Skip writes are tested without mounting
// `Welcome.svelte`.
//
// Step 1 is the library folder (required). Step 2 is the storefront rows in
// name order (`STOREFRONT_ORDER`: Amazon, Epic, GOG, RomM, Steam), each
// optional. One way on: Skip for now while nothing is connected (Continue
// shows, disabled), then Continue alone. No emulator step without RomM
// (plan 0.11); with RomM connected, Choose your systems follows Continue.
import type { RommStatus, StoreSource, StoreStatus } from './api';
import { ENABLED_STORES, storeName } from './gamestores/marks';
import { steamIdLine } from './gamestores/steam';
import { STOREFRONT_ORDER, type Storefront } from './romm/order';
import { rommHost, WELCOME_ROMM_ROW } from './romm/words';

export const WELCOME_TITLE = 'Welcome to GRID';
export const WELCOME_LEDE = 'One Library for the games on your RomM server and the stores you buy from.';

/** Over the welcome after a Python import (decision 12). */
export const IMPORT_NOTICE = {
  bold: 'Your settings from the previous version are filled in.',
  rest: 'Enter your RomM token to reconnect, or skip it for now.',
} as const;

export const FOLDER_STEP = {
  title: 'Library folder',
  note: 'Games, emulators and saves go here.',
  label: 'Library folder',
  browse: 'Browse…',
  pickerTitle: 'Select Library Folder',
  missing: 'Choose a folder to continue.',
} as const;

export const STORES_STEP = {
  title: 'Connect your games',
  optional: '· optional',
  note: 'Connect the ones you use. Add or change them any time in Settings › Game stores.',
} as const;

export const FOOTER = {
  skip: 'Skip for now',
  continue: 'Continue',
  nothingYet: 'Nothing connected yet. Skip for now opens GRID with your library folder.',
  nextSystems: 'Next: choose the systems you play.',
  opensLibrary: 'GRID opens on your Library.',
} as const;

export const STEAM_ROW = {
  sub: 'Installed Steam games show as soon as Steam is set up here.',
  action: 'Set up…',
  close: 'Close',
} as const;

export const CONNECTED_PILL = 'Connected';
export const ROMM_EDIT = 'Edit';
export const STORE_CONNECT = 'Connect…';

/** How a row connects: the store's login window (Amazon, Epic, GOG), or a
 *  form inline in the row (RomM's sign-in form, Steam's card). */
export type WelcomeRowKind = 'login' | 'romm' | 'steam';

export type WelcomeRow = {
  id: Storefront;
  name: string;
  kind: WelcomeRowKind;
  connected: boolean;
  sub: string;
};

/** "Sign in on GOG's own page. GRID never sees your password." */
export function loginSub(source: StoreSource): string {
  return `Sign in on ${storeName(source)}’s own page. GRID never sees your password.`;
}

function storeRow(source: StoreSource, status: StoreStatus | undefined): WelcomeRow {
  const connected = status?.connection === 'connected';
  if (source === 'steam') {
    return { id: source, name: storeName(source), kind: 'steam', connected, sub: connected && status ? steamIdLine(status) : STEAM_ROW.sub };
  }
  return {
    id: source,
    name: storeName(source),
    kind: 'login',
    connected,
    sub: connected && status ? `Signed in as ${status.username}` : loginSub(source),
  };
}

function rommRow(status: RommStatus): WelcomeRow {
  const connected = status.connection === 'connected';
  return {
    id: 'romm',
    name: WELCOME_ROMM_ROW.name,
    kind: 'romm',
    connected,
    sub: connected ? WELCOME_ROMM_ROW.connected(status.username, rommHost(status)) : WELCOME_ROMM_ROW.sub,
  };
}

/**
 * The storefront rows by name. Every enabled store has a row even before its
 * status has loaded (it then reads not connected).
 */
export function welcomeRows(romm: RommStatus, stores: readonly StoreStatus[]): WelcomeRow[] {
  return STOREFRONT_ORDER.filter((id) => id === 'romm' || (ENABLED_STORES as readonly string[]).includes(id)).map((id) =>
    id === 'romm' ? rommRow(romm) : storeRow(id, stores.find((s) => s.source === id)),
  );
}

/** Step 1 is the one required step: an empty folder holds both buttons. */
export function folderMissing(folder: string): boolean {
  return folder.trim() === '';
}

/** What the field starts with: the stored folder (a Python import carries
 *  it over), else the suggested one. */
export function initialFolder(stored: string, suggested: string): string {
  return stored.trim() !== '' ? stored.trim() : suggested.trim();
}

/** What Continue or Skip writes as `library_path`: the typed folder when it
 *  differs from the stored one, else nothing (`null`). */
export function folderToSave(folder: string, stored: string): string | null {
  const next = folder.trim();
  if (next === '' || next === stored.trim()) return null;
  return next;
}

export type WelcomeFooter = {
  /** Skip for now shows while nothing is connected. */
  skipShown: boolean;
  skipEnabled: boolean;
  continueEnabled: boolean;
  help: string;
};

/**
 * The footer (designer decision 5): while nothing is connected, Skip for now
 * is the way on and Continue is disabled; once something is connected, only
 * Continue shows. Both need the library folder, and neither runs twice.
 */
export function welcomeFooter(f: {
  folder: string;
  rows: readonly WelcomeRow[];
  /** `setup_status().needed`, read after RomM connected here. */
  setupNeeded: boolean;
  busy: boolean;
}): WelcomeFooter {
  const any = f.rows.some((r) => r.connected);
  const ready = !folderMissing(f.folder) && !f.busy;
  const romm = f.rows.some((r) => r.id === 'romm' && r.connected);
  return {
    skipShown: !any,
    skipEnabled: !any && ready,
    continueEnabled: any && ready,
    help: !any ? FOOTER.nothingYet : romm && f.setupNeeded ? FOOTER.nextSystems : FOOTER.opensLibrary,
  };
}

/** Which row starts open: RomM's after a Python import (its server and
 *  username are filled in, the token is left to type), else none. */
export function initialOpenRow(imported: boolean, rommConnected: boolean): Storefront | null {
  return imported && !rommConnected ? 'romm' : null;
}
