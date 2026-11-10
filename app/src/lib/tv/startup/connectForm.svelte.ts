// What Connect's fields hold while it is on show (tvmode.js `connectPage`'s
// `F`). It lives outside the screen because the folder browser is a page
// pushed over Connect, which remounts the screen when it closes.
//
// The token or password is never in `$state`, never logged and never shown:
// a plain module local, read once by `takeSecret` as Connect presses, and
// dropped when the form is reset. Only whether one is typed is reactive.
import { api } from '../../api';
import type { ConnectPrefill } from '../../connectPrefill';
import type { SignIn } from './connectPage';

export const connectForm = $state<{
  url: string;
  how: SignIn;
  user: string;
  secretTyped: boolean;
  lib: string;
  /** The library folder already stored ('' when none): a Connect that keeps
   *  it doesn't store it again. */
  libStored: string;
  /** Why the screen is here (a rejected credential), or a refusal. */
  reason: string | null;
  busy: boolean;
}>({ url: '', how: 'token', user: '', secretTyped: false, lib: '', libStored: '', reason: null, busy: false });

let secret = '';

export function setSecret(value: string): void {
  secret = value;
  connectForm.secretTyped = value !== '';
}

/** The typed secret, once: the field is blank again straight after. */
export function takeSecret(): string {
  const s = secret;
  secret = '';
  connectForm.secretTyped = false;
  return s;
}

/** A fresh form, as Connect opens: the stored server and username, the
 *  rejected credential's kind and reason, and the library folder (the
 *  stored one, else the suggested one). */
export function resetConnectForm(prefill: ConnectPrefill): void {
  secret = '';
  Object.assign(connectForm, {
    url: prefill.serverUrl,
    how: prefill.useToken ? 'token' : 'password',
    user: prefill.username,
    secretTyped: false,
    lib: '',
    libStored: '',
    reason: prefill.reason,
    busy: false,
  });
  void Promise.all([api.getLibraryPath(), api.suggestedLibraryPath()])
    .then(([stored, suggested]) => {
      // Typed into already: keep what was typed.
      if (connectForm.lib !== '') return;
      connectForm.libStored = stored.trim();
      connectForm.lib = stored.trim() !== '' ? stored.trim() : suggested.trim();
    })
    .catch(() => {});
}

/** The folder to store before connecting: `null` when the stored one is kept. */
export function libraryToSave(): string | null {
  const lib = connectForm.lib.trim();
  return lib === connectForm.libStored ? null : lib;
}
