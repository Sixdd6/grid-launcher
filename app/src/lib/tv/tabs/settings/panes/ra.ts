// Settings › RetroAchievements in TV mode (checklist items 250–251;
// tvmode.js `paneRows('ra')`; ruling 9). The username and the token are
// typed on the on-screen keyboard and held only by the pane until Save; the
// token row is write-only: it opens empty and shows dots once a token is
// typed or saved, never the token. Clear acts at once, as on the desktop.
import { canSubmit } from '../../../../emulators/retroachievements';
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
};

export type RaActions = { editUsername: () => void; editToken: () => void; save: () => void; clear: () => void };

export const RA_SAVED = 'RetroAchievements login saved';
export const SAVE_NEEDS = 'Enter a username and a token first.';

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
      ...(s.canSave ? {} : { disabled: true, why: SAVE_NEEDS }),
      run: a.save,
    },
    { type: 'action', key: 'ra-clear', label: 'Clear', danger: true, run: a.clear },
  ];
}
