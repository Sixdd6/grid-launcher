// RomM as a storefront (romm-storefront-plan 0.7): its status, pulled once
// and then kept by `romm-changed`, and the connect / reconnect / disconnect
// actions the welcome, the RomM card and the rail banners call. No secret is
// kept here: a typed one goes straight to the backend.
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { api, ROMM_CHANGED_EVENT, type RommStatus } from '../api';
import type { SessionPhase, SetupReturn } from '../shell';
import { session } from './session.svelte';

export const NOT_CONNECTED: RommStatus = {
  connection: 'not_connected',
  server_url: '',
  username: '',
  auth_kind: null,
  last_error: '',
  installed_count: 0,
  in_rail: false,
  busy: false,
};

export const romm = $state<{ status: RommStatus; loaded: boolean }>({ status: { ...NOT_CONNECTED }, loaded: false });

/** The backend calls; replaced in tests. */
export type RommApi = Pick<typeof api, 'rommStatus' | 'rommRestore' | 'rommConnect' | 'rommReconnect' | 'rommDisconnect' | 'setupStatus'>;
let rommApi: RommApi = api;
let listening: Promise<UnlistenFn> | null = null;

export function configureRomm(next: RommApi) {
  rommApi = next;
}

export function setRommStatus(status: RommStatus) {
  romm.status = status;
  romm.loaded = true;
}

/**
 * Starts listening for `romm-changed` (once for the app's life) and pulls the
 * status. Safe to call more than once; every call pulls again.
 */
export async function initRomm(): Promise<void> {
  listening ??= listen<RommStatus>(ROMM_CHANGED_EVENT, (e) => setRommStatus(e.payload));
  await listening;
  await refreshRomm();
}

/** Pulls the status now (after a legacy command, whose events may still be in flight). */
export async function refreshRomm(): Promise<void> {
  try {
    setRommStatus(await rommApi.rommStatus());
  } catch {
    // Keep what we have; the next event or pull corrects it.
  }
}

/** The startup restore, in the background once the shell is up. */
export async function restoreRomm(): Promise<void> {
  try {
    setRommStatus(await rommApi.rommRestore());
  } catch {
    await refreshRomm();
  }
}

/** Where a connect came from: the welcome's RomM row, or the RomM card. */
export type ConnectFrom = SetupReturn;

/**
 * The phase after a successful RomM connect (decision 9). From the card,
 * Choose your systems opens at once when setup is needed and returns to
 * Settings › Game stores. From the welcome nothing changes yet: Choose your
 * systems follows the welcome's Continue (`phaseAfterWelcome`). `null`: stay.
 */
export function phaseAfterConnect(
  from: ConnectFrom,
  setupNeeded: boolean,
): { phase: SessionPhase; setupReturn: SetupReturn } | null {
  if (from === 'settings' && setupNeeded) return { phase: 'setup', setupReturn: 'settings' };
  return null;
}

/** The phase after the welcome's Continue or Skip for now: Choose your
 *  systems when RomM was connected there and setup is needed, else the shell. */
export function phaseAfterWelcome(
  rommConnected: boolean,
  setupNeeded: boolean,
): { phase: SessionPhase; setupReturn: SetupReturn | null } {
  return rommConnected && setupNeeded ? { phase: 'setup', setupReturn: 'welcome' } : { phase: 'shell', setupReturn: null };
}

/** `setup_status().needed`, false when the check itself fails (Settings ›
 *  Systems is always there later). */
export async function setupNeeded(): Promise<boolean> {
  try {
    return (await rommApi.setupStatus()).needed;
  } catch {
    return false;
  }
}

/**
 * Connects RomM. Resolves to `null` on success, or the backend's error text
 * (secret-free; `romm/signIn.ts` `signInError` words it). On success from
 * the card, Choose your systems opens when setup is needed
 * (`session.phase = 'setup'`, `session.setupReturn = 'settings'`).
 */
export async function connectRomm(
  serverUrl: string,
  username: string,
  secret: string,
  useToken: boolean,
  from: ConnectFrom,
): Promise<string | null> {
  try {
    setRommStatus(await rommApi.rommConnect(serverUrl, username, secret, useToken));
  } catch (e) {
    return String(e);
  }
  const next = phaseAfterConnect(from, from === 'settings' ? await setupNeeded() : false);
  if (next !== null) {
    session.setupReturn = next.setupReturn;
    session.phase = next.phase;
  }
  return null;
}

/** Retry / Reconnect with the stored credential. Resolves to the error text, or `null`. */
export async function reconnectRomm(): Promise<string | null> {
  try {
    setRommStatus(await rommApi.rommReconnect());
    return null;
  } catch (e) {
    await refreshRomm();
    return String(e);
  }
}

/** Disconnect RomM: the shell stays (decision 5). Resolves to the error text, or `null`. */
export async function disconnectRomm(): Promise<string | null> {
  try {
    setRommStatus(await rommApi.rommDisconnect());
    return null;
  } catch (e) {
    await refreshRomm();
    return String(e);
  }
}

/** Tests only. */
export function resetRommForTest() {
  romm.status = { ...NOT_CONNECTED };
  romm.loaded = false;
  rommApi = api;
  listening = null;
}
