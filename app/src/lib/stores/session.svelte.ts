import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { api, SESSION_UNAUTHORIZED_EVENT, type AuthKind, type SessionUnauthorized } from '../api';
import {
  applyRestore,
  applyRetry,
  applyUnauthorizedEvent,
  offlineSession,
  type SessionPhase,
  type SetupReturn,
  type ShellSession,
} from '../shell';
import { initRomm, phaseAfterWelcome, refreshRomm, restoreRomm, romm, setupNeeded } from './romm.svelte';

/**
 * The app's startup phase and the legacy session fields. `connected`,
 * `serverUrl` and `username` are getters over RomM's status (`romm.svelte.ts`), so its ~30 readers do not
 * change (romm-storefront-plan 0.7). `offline`, `lastError` and `rejected`
 * stay for the legacy chip and Connect screen until Step 9; the RomM
 * storefront code does not write them.
 */
class Session {
  phase = $state<SessionPhase>('loading');
  /** Where Choose your systems returns (plan 0.5). Set with `phase = 'setup'`;
   *  kept after Skip/Done so the shell can open Settings › Game stores. */
  setupReturn = $state<SetupReturn | null>(null);
  lastError = $state<string | null>(null);
  offline = $state(false);
  rejected = $state<AuthKind | undefined>(undefined);
  error = $state<string | null>(null);
  busy = $state(false);

  get connected(): boolean {
    return romm.status.connection === 'connected';
  }

  /** RomM's stored server and user, from its status like `connected`: the
   *  RomM storefront flow (welcome, Game stores card, background restore)
   *  never runs the legacy `assign`, so a field of its own went stale (empty)
   *  and Details' "On <drive>" line, the Backup dialog's host and Discover's
   *  feed cache read nothing. The legacy writes are ignored until Step 9. */
  get serverUrl(): string {
    return romm.status.server_url;
  }
  set serverUrl(_value: string) {}
  get username(): string {
    return romm.status.username;
  }
  set username(_value: string) {}
}

export const session = new Session();

/** Applies a legacy session shape. `connected` comes from RomM's status. */
function assign(next: ShellSession) {
  session.phase = next.phase; session.serverUrl = next.serverUrl;
  session.username = next.username; session.lastError = next.lastError; session.offline = next.offline;
  session.rejected = next.rejected;
}

/**
 * A connected session goes to the first-run platform picker instead of the
 * shell when setup is needed (a fresh install: `setup_status`). Offline, or
 * when the check itself fails, it goes straight to the shell: the picker
 * needs the server's platforms, and Settings › Systems is always there
 * later.
 */
async function withSetupCheck(next: ShellSession): Promise<ShellSession> {
  if (next.phase !== 'shell' || !next.connected) return next;
  try {
    return (await api.setupStatus()).needed ? { ...next, phase: 'setup' } : next;
  } catch {
    return next;
  }
}

/** The picker was finished or skipped: on to the shell. `setupReturn` stays,
 *  so the shell can open where the connect came from (`'settings'`: Settings
 *  › Game stores); the shell clears it once used. */
export function finishSetup() {
  if (session.phase === 'setup') session.phase = 'shell';
}

// --- RomM storefront startup (plan 0.5) --------------------------------------

/**
 * Startup with RomM as a storefront: the welcome while `setup.welcomed` is
 * false, else the shell at once, with the RomM restore running in the
 * background, so an unreachable server never holds the window.
 */
export async function start() {
  await initRomm();
  let needed = false;
  try {
    needed = (await api.welcomeStatus()).needed;
  } catch {
    needed = false;
  }
  if (needed) {
    session.phase = 'welcome';
    return;
  }
  session.phase = 'shell';
  void restoreRomm();
}

/**
 * The welcome's Continue or Skip for now: marks it done, then Choose your
 * systems when RomM was connected there and setup is needed, else the shell.
 * A failure stays on the welcome with `session.error` set.
 */
export async function finishWelcome() {
  session.error = null;
  try {
    await api.finishWelcome();
  } catch (e) {
    session.error = String(e);
    return;
  }
  const connected = romm.status.connection === 'connected';
  const next = phaseAfterWelcome(connected, connected ? await setupNeeded() : false);
  session.setupReturn = next.setupReturn;
  session.phase = next.phase;
}

// --- Legacy session flow (Connect screen, chip) -----------------------------
// Wrappers over the legacy commands, which run on the RomM state. Each pulls
// the RomM status before it changes the phase, so `connected` is right then.

export async function restore() {
  try {
    await initRomm();
    const outcome = await api.restoreSession();
    await refreshRomm();
    assign(await withSetupCheck(applyRestore(outcome)));
  } catch {
    assign({ phase: 'none', connected: false, serverUrl: '', username: '', lastError: null, offline: false });
  }
}

/**
 * Q6: the backend emits `session-unauthorized` once, on the first 401 of a
 * live session. The payload is URL, username and auth kind — never a secret.
 * Legacy: the RomM storefront reads `romm-changed` (Sign-in expired) and
 * stays in the shell.
 */
export function listenUnauthorized(): Promise<UnlistenFn> {
  return listen<SessionUnauthorized>(SESSION_UNAUTHORIZED_EVENT, (e) => {
    const next = applyUnauthorizedEvent(session.phase, e.payload);
    if (next !== null) {
      session.error = null;
      assign(next);
    }
  });
}

export async function connect(serverUrl: string, username: string, secret: string, useToken: boolean) {
  session.busy = true; session.error = null;
  try {
    const state = await api.connect(serverUrl, username, secret, useToken);
    await refreshRomm();
    assign(
      await withSetupCheck({
        phase: 'shell', connected: true, serverUrl: state.server_url, username: state.username, lastError: null, offline: false,
      }),
    );
  } catch (e) { session.error = String(e); }
  finally { session.busy = false; }
}

/** The chip's Retry: a typed outcome, so a rejected credential opens Connect.
 *  A connection leaves offline mode; a failed retry keeps it. */
export async function retry() {
  session.busy = true;
  try {
    const outcome = await api.retryConnect();
    await refreshRomm();
    assign(applyRetry(session.offline, outcome));
  } catch (e) { session.lastError = String(e); }
  finally { session.busy = false; }
}

export async function disconnect() {
  try { await api.disconnect(); } finally {
    await refreshRomm();
    assign({ phase: 'none', connected: false, serverUrl: '', username: '', lastError: null, offline: false });
  }
}

/**
 * "Use without a server": the backend stops probing RomM at startup and
 * drops any live client; the shell opens not connected, in offline mode.
 * The stored server and username stay for a later connect. A failure stays
 * on the current screen with `session.error` set.
 */
export async function useOffline() {
  session.busy = true; session.error = null;
  try {
    await api.useWithoutServer();
    await refreshRomm();
    assign(offlineSession(session.serverUrl, session.username));
  } catch (e) { session.error = String(e); }
  finally { session.busy = false; }
}

/**
 * Offline mode's "Connect to a server…" (the chip menu, the Server panel,
 * Settings › Connection): back to the Connect screen, as Disconnect does,
 * but nothing is cleared. The library folder, the stored server and
 * username (they prefill the form) and the offline flag all stay: offline
 * mode ends only when a connection succeeds, so quitting from Connect
 * starts offline again.
 */
export function connectToServer() {
  session.error = null;
  session.phase = 'none';
}
