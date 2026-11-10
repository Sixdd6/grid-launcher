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
