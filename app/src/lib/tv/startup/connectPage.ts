// TV mode's Connect screen (checklist items 392–399, 515–516; README § Startup
// screens › Connect, § Use without a server; tvmode.js `connectPage`,
// `ACT.cfield`, `ACT.connectGo`, `ACT.noServer`). Pure: its words, which
// fields it shows, and when Connect and Use without a server can be pressed.
// No secret passes through here: only whether one is typed.

export type SignIn = 'token' | 'password';

export const CONNECT_TITLE = 'Connect to your RomM server';
export const CONNECT_SUB = 'Everything here works with a controller. A keyboard works too.';
export const NO_SERVER_NOTE = 'Without a server, GRID needs only the library folder. Your Library and Backup drives work offline.';
export const SIGN_IN_LABELS: Record<SignIn, string> = { token: 'API token', password: 'Username and password' };
export const PASTED = 'Pasted from the clipboard';
/** View with an empty clipboard on a field, and View off a field (consultant, after P12). */
export const NOTHING_TO_PASTE = 'Nothing to paste: the clipboard has no text.';
export const PASTE_NEEDS_FIELD = 'Focus a field to paste into it.';
/** Connect's label and both buttons' reason while a connect runs (consultant, after P12). */
export const CONNECTING = 'Connecting…';
export const CONNECTING_WHY = 'Still connecting. Wait for it to finish.';
export const NO_SERVER_TITLE = 'Using GRID without a server';
export const NO_SERVER_SUB = 'Your Library and Backup drives work offline';
export const LIBRARY_BROWSER_TITLE = 'Library folder';

export type FieldKey = 'url' | 'how' | 'token' | 'user' | 'password' | 'lib';

export type ConnectField = {
  key: FieldKey;
  label: string;
  /** What shows: the value, dots for a typed secret, or the placeholder. */
  value: string;
  placeholder: boolean;
  /** The A hint: "Edit", or "Change" for Sign in with. */
  a: string;
};

export type ConnectState = {
  url: string;
  how: SignIn;
  user: string;
  /** Whether a token or password is typed (its value stays out of here). */
  secretTyped: boolean;
  lib: string;
};

/** Dots for a typed secret: never its length or value. */
export const MASK = '••••••••••••';

function field(key: FieldKey, label: string, value: string, placeholder: string, a = 'Edit'): ConnectField {
  return value !== '' ? { key, label, value, placeholder: false, a } : { key, label, value: placeholder, placeholder: true, a };
}

/** The rows in order (item 393): Server, Sign in with, then the API token or
 *  the username and password, then Library folder. */
export function connectFields(s: ConnectState): ConnectField[] {
  const secret = s.secretTyped ? MASK : '';
  return [
    field('url', 'Server', s.url, 'https://…'),
    { key: 'how', label: 'Sign in with', value: SIGN_IN_LABELS[s.how], placeholder: false, a: 'Change' },
    ...(s.how === 'token'
      ? [field('token', 'API token', secret, 'Paste or type your token')]
      : [field('user', 'Username', s.user, 'Your RomM user'), field('password', 'Password', secret, 'Your password')]),
    field('lib', 'Library folder', s.lib, ''),
  ];
}

/** The keyboard's title for a text field (item 394). */
export function keyboardTitle(key: 'url' | 'token' | 'user' | 'password'): string {
  return { url: 'Server address', token: 'API token', user: 'Username', password: 'Password' }[key];
}

/** The token and the password are masked and open empty (item 394). */
export function isSecret(key: FieldKey): boolean {
  return key === 'token' || key === 'password';
}

/** Connect is enabled once the server, the credentials and the library
 *  folder are filled (item 397). */
export function connectReady(s: ConnectState): boolean {
  if (s.url.trim() === '' || !s.secretTyped || s.lib.trim() === '') return false;
  return s.how === 'token' || s.user.trim() !== '';
}

/** What A (or Start) on a disabled Connect says (item 397; ruling 6). */
export function connectWhy(how: SignIn): string {
  return `Fill in the server, the ${how === 'token' ? 'API token' : 'username and password'} and the library folder first.`;
}

export type ConnectButton = { label: string; disabled: boolean; why?: string };

/** Connect and Use without a server (items 397, 515; consultant, after P12):
 *  while a connect runs, Connect reads "Connecting…" and both are disabled
 *  with the same reason; otherwise each is enabled once its fields are. */
export function connectButtons(s: ConnectState, connecting: boolean): { go: ConnectButton; offline: ConnectButton } {
  if (connecting) {
    return {
      go: { label: CONNECTING, disabled: true, why: CONNECTING_WHY },
      offline: { label: 'Use without a server', disabled: true, why: CONNECTING_WHY },
    };
  }
  return {
    go: { label: 'Connect', disabled: !connectReady(s), why: connectWhy(s.how) },
    offline: { label: 'Use without a server', disabled: !offlineReady(s) },
  };
}

/** What View says when there's nothing to paste: off a text field, or an
 *  empty clipboard (consultant, after P12). Null when it can paste. */
export function pasteRefusal(field: FieldKey | null, text: string): string | null {
  if (field === null || field === 'how') return PASTE_NEEDS_FIELD;
  if (text.trim() === '') return NOTHING_TO_PASTE;
  return null;
}

/** Use without a server needs only the library folder (item 515). */
export function offlineReady(s: Pick<ConnectState, 'lib'>): boolean {
  return s.lib.trim() !== '';
}

/** "Connected to <URL>" (item 399). */
export function connectedText(url: string): string {
  return `Connected to ${url}`;
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

// ---- Settings › Connection › Edit connection… (consultant ruling E3): the
// Connect screen as a page over Settings. The server, the sign-in method and
// the username are filled in; the token or password is typed again. No
// library folder (the library stays) and no Use without a server.

export const EDIT_CONNECTION_LABEL = 'Edit connection…';
export const EDIT_CONNECTION_SUB = 'Change the server or sign in again. Your library and settings stay.';

/** The edit form's start: the session's server and username, signing in
 *  with a token as the desktop's edit form does (the session doesn't say
 *  which kind it used; a token session has a username too). */
export function editPrefill(s: { serverUrl: string; username: string }): { serverUrl: string; username: string; useToken: boolean; reason: null } {
  return { serverUrl: s.serverUrl, username: s.username, useToken: true, reason: null };
}

/** The edit form's rows: Connect's without the library folder. */
export function editFields(s: ConnectState): ConnectField[] {
  return connectFields(s).filter((f) => f.key !== 'lib');
}

/** Connect on the edit form: the server and the credentials, no folder. */
export function editButton(s: ConnectState, connecting: boolean): ConnectButton {
  if (connecting) return { label: CONNECTING, disabled: true, why: CONNECTING_WHY };
  const ready = connectReady({ ...s, lib: '-' });
  return { label: 'Connect', disabled: !ready, why: `Fill in the server and the ${s.how === 'token' ? 'API token' : 'username and password'} first.` };
}
