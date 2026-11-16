import { api } from '../api';
import type { SessionPhase, SetupReturn } from '../shell';
import { initRomm, phaseAfterWelcome, restoreRomm, romm, setupNeeded } from './romm.svelte';

/**
 * The app's startup phase. `connected`, `serverUrl` and `username` are
 * getters over RomM's status (`romm.svelte.ts`), so their readers follow
 * the RomM storefront (romm-storefront-plan 0.7).
 */
class Session {
  phase = $state<SessionPhase>('loading');
  /** Where Choose your systems returns (plan 0.5). Set with `phase = 'setup'`;
   *  kept after Skip/Done so the shell can open Settings › Game stores. */
  setupReturn = $state<SetupReturn | null>(null);
  /** The welcome's Continue/Skip failed (`finishWelcome`). */
  error = $state<string | null>(null);

  get connected(): boolean {
    return romm.status.connection === 'connected';
  }

  /** RomM's stored server and user, from its status like `connected`
   *  (Details' "On <drive>" line, the Backup dialog's host, Discover's feed
   *  cache). */
  get serverUrl(): string {
    return romm.status.server_url;
  }
  get username(): string {
    return romm.status.username;
  }
}

export const session = new Session();

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
