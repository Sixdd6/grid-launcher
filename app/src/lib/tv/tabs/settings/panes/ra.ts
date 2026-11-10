// Settings › RetroAchievements in TV mode (checklist items 250–251;
// tvmode.js `paneRows('ra')`; ruling 9). The username and the token are
// typed on the on-screen keyboard and held only by the pane until Save; the
// token row is write-only: it opens empty and shows dots once a token is
// typed or saved, never the token. The desktop's Log In follows Save
// (consultant decision; checklist 533): a write-only Password row and Log In,
// which trades the password for a token. Clear acts at once, as on the desktop.
import { canLogin, canSubmit } from '../../../../emulators/retroachievements';
import type { Row } from '../../../rows';

export type RaState = {
  /** The typed username, else the saved one. */
  username: string;
  /** Whether a token is typed in the pane (its value stays out of here). */
  tokenTyped: boolean;
  /** Whether a token is saved. */
  tokenPresent: boolean;
  /** Whether Save can send: a username and a typed token. */
  canSave: boolean;
  /** Whether a password is typed in the pane (its value stays out of here). */
  passwordTyped: boolean;
  /** Whether Log In can send: a username and a typed password. */
  canLogin: boolean;
};

export type RaActions = {
  editUsername: () => void;
  editToken: () => void;
  save: () => void;
  editPassword: () => void;
  login: () => void;
  clear: () => void;
};

export const RA_SAVED = 'RetroAchievements login saved';
/** Save's why while disabled, naming what is missing (consultant decision; 533). */
export const SAVE_NEEDS = 'Enter a username and a token first.';
export const SAVE_NEEDS_TOKEN = 'Enter a token first.';
export const SAVE_NEEDS_USER = 'Enter a username first.';
export const LOGIN_NEEDS = 'Enter a username and a password first.';
export const LOGIN_NEEDS_PASSWORD = 'Enter a password first.';
export const LOGIN_NEEDS_USER = 'Enter a username first.';

/** What Save is missing, or null when it can send. */
export function saveWhy(username: string, tokenTyped: boolean): string | null {
  const user = username.trim() !== '';
  if (user && tokenTyped) return null;
  if (!user && !tokenTyped) return SAVE_NEEDS;
  return user ? SAVE_NEEDS_TOKEN : SAVE_NEEDS_USER;
}

/** What Log In is missing, or null when it can send. */
export function loginWhy(username: string, passwordTyped: boolean): string | null {
  const user = username.trim() !== '';
  if (user && passwordTyped) return null;
  if (!user && !passwordTyped) return LOGIN_NEEDS;
  return user ? LOGIN_NEEDS_PASSWORD : LOGIN_NEEDS_USER;
}

/** Log In's gate: the desktop's (a username and a non-empty password). */
export function raCanLogin(username: string, password: string): boolean {
  return canLogin(username, password);
}

/** Save's gate: the desktop's (both fields non-blank). */
export function raCanSave(username: string, token: string): boolean {
  return canSubmit(username, token);
}

/** A stand-in the row shows as dots: never the token. */
const DOTS = '•';

export function raRows(s: RaState, a: RaActions): Row[] {
  return [
    { type: 'text', key: 'ra-user', label: 'Username', value: s.username, edit: a.editUsername },
    {
      type: 'text',
      key: 'ra-token',
      label: 'Token',
      sub: 'Write-only: GRID never shows it again',
      mask: true,
      value: s.tokenTyped || s.tokenPresent ? DOTS : '',
      edit: a.editToken,
    },
    {
      type: 'action',
      key: 'ra-save',
      label: 'Save',
      sub: 'Sets the login in every emulator that supports it',
      ...(s.canSave ? {} : { disabled: true, why: saveWhy(s.username, s.tokenTyped) ?? SAVE_NEEDS }),
      run: a.save,
    },
    {
      type: 'text',
      key: 'ra-password',
      label: 'Password',
      sub: 'Write-only: used once to log in, never kept',
      mask: true,
      value: s.passwordTyped ? DOTS : '',
      edit: a.editPassword,
    },
    {
      type: 'action',
      key: 'ra-login',
      label: 'Log In',
      sub: 'Signs in with your password and saves the token it gets',
      ...(s.canLogin ? {} : { disabled: true, why: loginWhy(s.username, s.passwordTyped) ?? LOGIN_NEEDS }),
      run: a.login,
    },
    { type: 'action', key: 'ra-clear', label: 'Clear', danger: true, run: a.clear },
  ];
}
