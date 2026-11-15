// TV mode's RomM words and rules (TVRommStorefront README): the Installed
// view's header and empty states, the needs-RomM panels, the RomM page's
// rows and main button, and the sign-in page's fields. Pure. The words both
// modes share come from `romm/words.ts` (decision 17: desktop wins); what is
// here is TV's own (its buttons, hints, the A-says-why lines and the
// pointers an empty grid with no button keeps).
import type { RommStatus } from '../../api';
import {
  ROMM_INSTALLED,
  rommAccountLine,
  rommCardWarning,
  rommHost,
  rommInstalledChip,
  rommNotConnectedText,
  rommSignInKind,
  rommState,
  rommWhy,
  signInCopy,
  SIGN_IN_FIELDS,
  type SignInMode,
} from '../../romm/words';
import type { PageRow } from '../pages/pageRows';

const plural = (n: number, one: string) => `${n} ${one}${n === 1 ? '' : 's'}`;

/** What a needs-RomM line says after its first sentence: where to fix it.
 *  TV keeps these pointers where nothing on screen has a button. */
export function rommFix(status: RommStatus): string {
  if (status.connection === 'expired') return 'Your RomM sign-in expired. Sign in again in Settings › Game stores.';
  if (status.connection === 'unreachable') return "Can't reach RomM right now. GRID tries again when you press Retry.";
  return 'Connect RomM in Settings › Game stores.';
}

// ---- Library › RomM › Installed ----------------------------------------

export const INSTALLED_SEARCH_TITLE = 'Search RomM · Installed';

/** "12 games", or "3 of 12 games match" while a search narrows it. */
export function installedCountLine(total: number, shown: number, isSearching: boolean): string {
  return isSearching ? `${shown} of ${plural(total, 'game')} match` : plural(total, 'game');
}

/** The header chip: "Signed in as sam", "Connecting to …", "Not connected";
 *  none for Can't reach and Sign-in expired (the banner says it). */
export function installedChip(status: RommStatus): string | null {
  const state = rommState(status);
  if (state === 'not_connected' || state === 'never') return 'Not connected';
  return rommInstalledChip(status);
}

/** The empty grid: a search with no match, nothing installed while
 *  connected, or the needs-RomM pointer. "(Y)" stands for the Y glyph. */
export function installedEmpty(status: RommStatus, query: string): { title: string; body: string } {
  if (query.trim() !== '') return { title: ROMM_INSTALLED.noMatch(query.trim()), body: 'Press (Y) to change the search.' };
  const live = status.connection === 'connected' || rommState(status) === 'connecting';
  return { title: ROMM_INSTALLED.emptyTitle, body: live ? ROMM_INSTALLED.emptyConnected : rommFix(status) };
}

// ---- Needs RomM panels ---------------------------------------------------

export type NeedsAction = { kind: 'page' | 'signin' | 'retry'; label: string; al: string; disabled: boolean; why?: string };
export type NeedsPanel = { title: string; text: string; warn: boolean; action: NeedsAction };

export const BUSY_WHY = 'GRID is trying RomM now.';

/** The panel's button: Set up RomM… (not connected; opens the RomM page),
 *  Sign in again… (expired), Retry (can't reach; "Retrying…" while it runs). */
export function needsAction(status: RommStatus): NeedsAction {
  if (status.connection === 'expired' && !status.busy) return { kind: 'signin', label: 'Sign in again…', al: 'Sign in again', disabled: false };
  if (status.connection === 'unreachable' || (status.busy && status.connection !== 'not_connected')) {
    return status.busy
      ? { kind: 'retry', label: 'Retrying…', al: 'Retry', disabled: true, why: BUSY_WHY }
      : { kind: 'retry', label: 'Retry', al: 'Retry', disabled: false };
  }
  if (status.busy) return { kind: 'page', label: 'Set up RomM…', al: 'Open', disabled: true, why: BUSY_WHY };
  return { kind: 'page', label: 'Set up RomM…', al: 'Open', disabled: false };
}

/** A needs-RomM panel: its heading, why plus what still works, warning-edged
 *  unless RomM is simply not connected. Null while connected. */
export function needsPanel(status: RommStatus, title: string, rest: string): NeedsPanel | null {
  if (status.connection === 'connected') return null;
  const state = rommState(status);
  return {
    title,
    text: `${rommWhy(status)} ${rest}`,
    warn: state !== 'not_connected' && state !== 'never',
    action: needsAction(status),
  };
}

// ---- Settings › Game stores › RomM ----------------------------------------

export const GAME_STORES_TITLE = 'Game stores';
export const GAME_STORES_NOTE = 'Each one you connect gets a group in the Library, in this order.';

/** The RomM row's line and pill (desktop's card words). */
export { rommAccountLine as rommRowSub } from '../../romm/words';

/** The RomM page's sub line; `user` is set in bold by the page. */
export function rommPageSub(status: RommStatus): { before: string; user: string; after: string } {
  const host = rommHost(status);
  const state = rommState(status);
  if (state === 'connected') return { before: 'Signed in as ', user: status.username, after: ` · ${host}` };
  if (state === 'connecting') return { before: '', user: '', after: rommAccountLine(status) };
  if (state === 'unreachable') return { before: 'Signed in as ', user: status.username, after: ` · can't reach ${host}` };
  if (state === 'expired') return { before: '', user: '', after: `Sign-in expired · ${host}` };
  return { before: '', user: '', after: rommAccountLine(status) };
}

export type RommPageActions = {
  edit: () => void;
  showInLibrary: () => void;
  openInBrowser: () => void;
  disconnect: () => void;
};

/** The RomM page's rows per state (README § The RomM page). */
export function rommPageRows(status: RommStatus, installed: number, a: RommPageActions): PageRow[] {
  const R: PageRow[] = [];
  const host = rommHost(status);
  const warning = rommCardWarning(status);
  const off = status.connection === 'not_connected';
  if (warning) R.push({ kind: 'note', key: 'warn', text: `${warning.strong} ${warning.text}`, warn: true });
  if (off && !status.busy) R.push({ kind: 'note', key: 'off', text: rommNotConnectedText(installed) });
  if (!off || status.server_url !== '') {
    R.push({ kind: 'info', key: 'server', label: 'Server', value: status.server_url });
    if (status.username !== '') R.push({ kind: 'info', key: 'user', label: 'User', value: status.username });
  }
  if (!off) R.push({ kind: 'info', key: 'signin', label: 'Sign-in', value: rommSignInKind(status) });
  R.push({ kind: 'info', key: 'installed', label: 'Installed RomM games', value: String(installed) });
  if (!off) R.push({ kind: 'row', key: 'edit', label: 'Edit sign-in…', sub: 'Server, user and token', al: 'Edit', run: a.edit });
  if (installed > 0 || status.connection === 'connected') R.push({ kind: 'row', key: 'library', label: 'Show in Library', run: a.showInLibrary });
  if (host !== '') {
    R.push({ kind: 'row', key: 'browser', label: 'Open in browser', sub: `Opens ${host} in your web browser, over TV mode`, run: a.openInBrowser });
  }
  if (!off) R.push({ kind: 'row', key: 'disconnect', label: 'Disconnect…', sub: 'Installed RomM games stay', danger: true, run: a.disconnect });
  return R;
}

export type RommMain = { label: string; hint: string; action: 'reconnect' | 'signin' | 'connect' | null; disabled: boolean; why?: string };

/** The page's main button (Start): Reconnect, Sign in again…, Connect
 *  RomM…, or none while connected. */
export function rommPageMain(status: RommStatus): RommMain | null {
  const c = status.connection;
  if (status.busy) return { label: c === 'not_connected' ? 'Connecting…' : 'Reconnecting…', hint: '', action: null, disabled: true, why: BUSY_WHY };
  if (c === 'unreachable') return { label: 'Reconnect', hint: 'Reconnect', action: 'reconnect', disabled: false };
  if (c === 'expired') return { label: 'Sign in again…', hint: 'Sign in again', action: 'signin', disabled: false };
  if (c === 'not_connected') return { label: 'Connect RomM…', hint: 'Connect', action: 'connect', disabled: false };
  return null;
}

/** "Opening romm.home.lan in your browser…" / "It opens over TV mode. …" */
export function openingCard(host: string): { title: string; sub: string } {
  return { title: `Opening ${host} in your browser…`, sub: 'It opens over TV mode. Close it to come back.' };
}

/** The cards after Reconnect (or Retry), per outcome. */
export function reconnectCard(status: RommStatus): { title: string; sub: string; error: boolean; signIn: boolean } {
  const host = rommHost(status);
  if (status.connection === 'connected') return { title: `Connected to RomM as ${status.username}`, sub: host, error: false, signIn: false };
  if (status.connection === 'expired') {
    return { title: 'RomM sign-in expired', sub: 'Sign in again to browse, install and sync saves. Installed games still play.', error: true, signIn: true };
  }
  return { title: "Can't reach RomM", sub: `${host} didn't answer. Installed games still play.`, error: true, signIn: false };
}

// ---- the sign-in page --------------------------------------------------------

export type SignInHow = 'token' | 'password';
export type SignInFieldKey = 'url' | 'how' | 'user' | 'secret';
export type SignInForm = { url: string; how: SignInHow; user: string; secretTyped: boolean };

/** Title, sub line and the button (desktop's words: Connect, Save and
 *  connect, Sign in). */
export function signInWords(mode: SignInMode, username: string): { title: string; sub: string; button: string } {
  const c = signInCopy(mode, username || 'your user');
  return { title: c.title, sub: c.lede, button: c.button };
}

export const SIGN_IN_HOW_LABELS: Record<SignInHow, string> = { token: 'API token', password: 'Username and password' };
export const MASK = '••••••••••••';

export type SignInFieldView = { key: SignInFieldKey; label: string; value: string; placeholder: boolean; a: string };

function field(key: SignInFieldKey, label: string, value: string, placeholder: string, a = 'Edit'): SignInFieldView {
  return value !== '' ? { key, label, value, placeholder: false, a } : { key, label, value: placeholder, placeholder: true, a };
}

/** The fields in order: Server URL, Sign in with, then the token or the
 *  username and password. The secret shows as dots once typed, never its value. */
export function signInFields(f: SignInForm): SignInFieldView[] {
  const secret = f.secretTyped ? MASK : '';
  return [
    field('url', SIGN_IN_FIELDS.serverUrl, f.url, 'https://…'),
    { key: 'how', label: 'Sign in with', value: SIGN_IN_HOW_LABELS[f.how], placeholder: false, a: 'Change' },
    ...(f.how === 'token'
      ? [field('secret', SIGN_IN_FIELDS.token, secret, 'Paste or type your token')]
      : [field('user', SIGN_IN_FIELDS.username, f.user, 'Your RomM user'), field('secret', SIGN_IN_FIELDS.password, secret, 'Your password')]),
  ];
}

/** The keyboard's title for a text field. */
export function signInKeyboardTitle(key: 'url' | 'user' | 'secret', how: SignInHow): string {
  if (key === 'url') return 'RomM server address';
  if (key === 'user') return 'Username';
  return how === 'token' ? 'API token' : 'Password';
}

/** Why the button is disabled, or null when it can be pressed. */
export function signInWhy(f: SignInForm): string | null {
  if (f.url.trim() === '') return 'Enter your RomM server address first.';
  if (f.how === 'password' && f.user.trim() === '') return 'Enter your RomM username first.';
  if (!f.secretTyped) return `Enter your ${f.how === 'token' ? 'API token' : 'password'} first.`;
  return null;
}

/** The footer's line. */
export function signInHelp(how: SignInHow): string {
  return how === 'token' ? SIGN_IN_FIELDS.tokenHelp : SIGN_IN_FIELDS.passwordHelp;
}

export const SIGNING_IN_WHY = 'GRID is signing in to RomM.';
export const PASTED = 'Pasted from the clipboard';
export const NOTHING_TO_PASTE = { title: 'Nothing to paste', sub: 'Select the Server or token field first.' };

/** Which field a failed sign-in sends focus to: the secret after a 401
 *  (it is emptied), the server when it couldn't be reached, else none. */
export function errorField(raw: string): 'secret' | 'url' | null {
  if (raw.includes('rejected the credentials')) return 'secret';
  if (raw.includes('could not reach the server') || raw.includes('invalid server URL')) return 'url';
  return null;
}

/** Whether View can paste into `key` (the server, the username, the secret). */
export function canPasteInto(key: string | null): key is 'url' | 'user' | 'secret' {
  return key === 'url' || key === 'user' || key === 'secret';
}

/** The sign-in page's success card. */
export function connectedCard(status: RommStatus): { title: string; sub: string } {
  return { title: `Connected to RomM as ${status.username}`, sub: rommHost(status) };
}
