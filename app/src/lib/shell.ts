import type { AuthKind, OpenOn, RestoreOutcome, SessionUnauthorized } from './api';

/** The four first-class views, in pill order (design §3; the Emulators view
 *  became Settings › Systems). The index in this array is also the
 *  `Ctrl+<n>` accelerator. */
export const VIEWS = ['library', 'server', 'downloads', 'settings'] as const;
export type View = (typeof VIEWS)[number];
/** `setup`: connected, showing the first-run platform picker before the shell. */
export type SessionPhase = 'loading' | 'none' | 'setup' | 'shell';
export type ShellSession = {
  phase: SessionPhase;
  connected: boolean;
  serverUrl: string;
  username: string;
  lastError: string | null;
  /** Offline mode (`Config.offline`): the user chose to run without a
   *  server. Never true while `connected`; any successful connection clears
   *  it. Distinct from an unreachable server, which keeps `lastError`. */
  offline: boolean;
  /** Set only when the server rejected the stored credential (401): which
   *  kind it was, so Connect asks for the right one. Never the value. */
  rejected?: AuthKind;
};

export function applyRestore(outcome: RestoreOutcome): ShellSession {
  switch (outcome.kind) {
    case 'no_session':
      // Carried through so `Connect` can prefill them: a config written by
      // the Python importer holds a server and a username but no credential.
      return { phase: 'none', connected: false, serverUrl: outcome.server_url, username: outcome.username, lastError: null, offline: false };
    case 'connected':
      return {
        phase: 'shell', connected: true, serverUrl: outcome.state.server_url, username: outcome.state.username, lastError: null,
        offline: false,
      };
    case 'unauthorized':
      // Q6: a rejected credential is not "offline" — Retry would resend it
      // forever. Back to Connect, pre-filled; the keyring item stays until
      // a new connect succeeds.
      return {
        phase: 'none', connected: false, serverUrl: outcome.server_url, username: outcome.username, lastError: null,
        offline: false, rejected: outcome.auth_kind,
      };
    case 'unreachable':
      return {
        phase: 'shell', connected: false, serverUrl: outcome.server_url, username: outcome.username, lastError: outcome.error,
        offline: false,
      };
    case 'offline':
      // Offline mode: a not-connected shell like `unreachable`, but chosen,
      // so there is no error and no probe ran. The stored server and
      // username are kept for "Connect to a server…".
      return { phase: 'shell', connected: false, serverUrl: outcome.server_url, username: outcome.username, lastError: null, offline: true };
  }
}

/**
 * The chip's Retry outcome. A connection always leaves offline mode; any
 * other outcome (still unreachable, or a rejected credential) keeps the
 * offline flag it had, since nothing connected.
 */
export function applyRetry(prevOffline: boolean, outcome: RestoreOutcome): ShellSession {
  const next = applyRestore(outcome);
  return next.connected ? next : { ...next, offline: next.offline || prevOffline };
}

/** The shell after "Use without a server": not connected, offline, no error.
 *  The stored server and username are carried through for a later connect. */
export function offlineSession(serverUrl: string, username: string): ShellSession {
  return { phase: 'shell', connected: false, serverUrl, username, lastError: null, offline: true };
}

/**
 * The `session-unauthorized` event (a 401 mid-session). It acts only on a
 * live session — the shell, or the first-run platform picker, which talks to
 * the server too: on any other phase there is no session for it to end, and
 * resetting a Connect form the user is typing into would lose their input.
 */
export function applyUnauthorizedEvent(phase: SessionPhase, info: SessionUnauthorized): ShellSession | null {
  if (phase !== 'shell' && phase !== 'setup') return null;
  return applyRestore({ kind: 'unauthorized', ...info });
}

/** The view the shell opens on, and the drive key Library selects, if any. */
export type InitialView = { view: View; drive: string | null };

/**
 * Where GRID opens (TV mode README § Decided "Where GRID opens", the same
 * rule in desktop and TV mode). `automatic`: Server (on Discover, its first
 * rail row) when connected to a RomM server, the Library when not — an
 * unreachable server, or GRID used without one, where the Library opens on
 * `firstDrive` (the key of the first connected, set-up Backup drive) when
 * there is one. `discover` and `library` force the place; forced Discover
 * with no server lands on the Server tab's not-connected panel, and forced
 * Library without a server still picks the first drive.
 */
export function initialView(
  connected: boolean,
  offline = false,
  firstDrive: string | null = null,
  openOn: OpenOn = 'automatic',
): InitialView {
  const library: InitialView = { view: 'library', drive: !connected && offline ? firstDrive : null };
  if (openOn === 'discover') return { view: 'server', drive: null };
  if (openOn === 'library') return library;
  return connected ? { view: 'server', drive: null } : library;
}

export function viewLabel(view: View): string {
  return view.charAt(0).toUpperCase() + view.slice(1);
}

/**
 * The view a `Ctrl+<key>` accelerator selects, or `null` when the key is
 * not one of `1`..`4`. Takes the raw `KeyboardEvent.key` so the caller does
 * no parsing of its own; a multi-character key (`"F1"`, `"11"`) never
 * matches because the lookup is by exact string.
 */
export function viewForDigit(key: string): View | null {
  const index = VIEWS.findIndex((_, i) => String(i + 1) === key);
  return index === -1 ? null : VIEWS[index];
}

export function hostOf(serverUrl: string): string {
  try {
    return new URL(serverUrl).host || serverUrl;
  } catch {
    return serverUrl;
  }
}

export function chipLabel(s: Pick<ShellSession, 'connected' | 'offline' | 'username' | 'serverUrl'>): string {
  if (s.connected) return `${s.username} @ ${hostOf(s.serverUrl)}`;
  return s.offline ? 'Offline · no server' : 'Not connected';
}
