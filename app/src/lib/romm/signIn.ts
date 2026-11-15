// The RomM sign-in form's rules (RommSignInForm: the welcome's RomM row and
// the card's Connect / Edit / Sign in again dialog), kept out of the
// component so they can be tested without mounting it. Taken from the old
// Connect form (`connectForm.ts`, which re-exports the legacy helpers until
// the welcome replaces Connect).
import type { RommStatus } from '../api';
import { rommHost, rommToasts, type SignInMode } from './words';

export type SignInFields = {
  serverUrl: string;
  username: string;
  secret: string;
  useToken: boolean;
};

/** The button is enabled once URL and secret are filled, plus the username
 *  for a password. */
export function canSignIn(f: SignInFields): boolean {
  if (f.serverUrl.trim() === '' || f.secret === '') return false;
  if (!f.useToken && f.username.trim() === '') return false;
  return true;
}

/** What the form starts with. The secret is never prefilled. */
export type SignInStart = {
  serverUrl: string;
  username: string;
  useToken: boolean;
  /** Sign in again: the server URL and username are shown read-only. */
  accountReadOnly: boolean;
};

export function signInStart(status: Pick<RommStatus, 'server_url' | 'username' | 'auth_kind'>, mode: SignInMode): SignInStart {
  return {
    serverUrl: status.server_url,
    username: status.username,
    // `auth_kind` decides; unknown means token (the default).
    useToken: status.auth_kind !== 'basic',
    accountReadOnly: mode === 'again',
  };
}

/** Which form mode a status opens from the card: Sign in again from Sign-in
 *  expired, Edit when connected or a server is stored, else Connect. */
export function signInModeFor(status: Pick<RommStatus, 'connection' | 'server_url'>, edit = false): SignInMode {
  if (status.connection === 'expired' && !edit) return 'again';
  if (edit) return 'edit';
  return 'connect';
}

/**
 * The red line under the fields for a failed connect. `raw` is the backend's
 * error text (`SessionError`/`RommError` Display, secret-free); the result
 * is RommStorefront's words.
 */
export function signInError(raw: string, serverUrl: string, useToken: boolean): string {
  const text = raw.trim();
  const mismatch = /the token belongs to account '(.*)', not '(.*)'/.exec(text);
  if (mismatch !== null) {
    const [, actual, entered] = mismatch;
    return `This token belongs to "${actual}", not "${entered}". Use a token for ${entered}, or disconnect RomM first to sign in as someone else.`;
  }
  if (text.includes('rejected the credentials')) {
    return useToken ? 'RomM rejected this token (401). Check it, or make a new one.' : 'RomM rejected the username or password (401).';
  }
  if (text.includes('could not reach the server') || text.includes('invalid server URL')) {
    const url = serverUrl.trim() !== '' ? serverUrl.trim() : 'the server';
    return `Couldn't reach ${url}. Check the address, and that the server is running.`;
  }
  return text;
}

/**
 * The username sent with a connect. A password always sends the typed one. A
 * token identifies the account by itself, so the username is only a claim
 * the backend checks (`UsernameMismatch`): Edit and Sign in again send the
 * stored account, so a token for someone else is refused; a fresh Connect
 * sends none.
 */
export function usernameToSend(f: Pick<SignInFields, 'username' | 'useToken'>, mode: SignInMode, storedUsername: string): string {
  if (!f.useToken) return f.username.trim();
  return mode === 'connect' ? '' : storedUsername.trim();
}

/** The notification after a successful sign-in from the form. */
export function signedInToast(mode: SignInMode, status: Pick<RommStatus, 'server_url' | 'username'>): string {
  return mode === 'again' ? rommToasts.signedInAgain(status.username) : rommToasts.connected(rommHost(status), status.username);
}

/** The notification after Reconnect or Retry: "Reconnected to <host>" from
 *  Connected, "Connected to RomM again · <host>" from Can't reach or
 *  Sign-in expired; `null` when it did not connect. */
export function reconnectedToast(was: RommStatus['connection'], status: Pick<RommStatus, 'connection' | 'server_url'>): string | null {
  if (status.connection !== 'connected') return null;
  const host = rommHost(status);
  return was === 'connected' ? rommToasts.reconnected(host) : rommToasts.backAgain(host);
}

// --- Legacy Connect form helpers (re-exported by `connectForm.ts`) ----------

/**
 * Whether the form asks for a library folder, and what the field starts
 * with. A first run has no folder stored: the field is shown, pre-filled
 * with the suggested folder, and required — nothing can install without one.
 * When a folder is already stored (a rejected credential brought the form
 * back, or a Python import carried it over) the field is not shown at all;
 * Settings › Library is where it changes.
 */
export function libraryField(stored: string, suggested: string): { show: boolean; initial: string } {
  return stored.trim() !== '' ? { show: false, initial: '' } : { show: true, initial: suggested.trim() };
}

export type ConnectFields = SignInFields & {
  /** `null` while the form does not ask for a folder. */
  libraryPath: string | null;
};

/** The Connect button is enabled only once every field it shows is filled.
 *  The username is asked for only in password mode. */
export function canConnect(f: ConnectFields): boolean {
  if (!canSignIn(f)) return false;
  if (f.libraryPath !== null && f.libraryPath.trim() === '') return false;
  return true;
}

/**
 * "Use without a server" needs only the library folder: enabled once the
 * stored folder is known (`libraryKnown`) and, when the form asks for one,
 * the field is filled. The server fields play no part.
 */
export function canUseOffline(libraryKnown: boolean, libraryPath: string | null): boolean {
  if (!libraryKnown) return false;
  return libraryPath === null || libraryPath.trim() !== '';
}
