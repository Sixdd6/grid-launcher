// What the Connect form starts with. Pure, so the rules are tested without
// mounting the component. Nothing here can carry a secret: the session store
// never holds one, and the returned shape has no field for it.
import type { AuthKind } from './api';

export type ConnectPrefill = {
  serverUrl: string;
  username: string;
  /** Token mode unless the rejected credential was a password. */
  useToken: boolean;
  /** Why the form is back (Q6: the server rejected the stored credential), or null. */
  reason: string | null;
};

export const REJECTED_TOKEN_REASON = 'The server rejected your token. Enter a new one.';
export const REJECTED_PASSWORD_REASON = 'The server rejected your password. Enter it again.';

export function connectPrefill(s: { serverUrl: string; username: string; rejected?: AuthKind }): ConnectPrefill {
  const reason =
    s.rejected === 'token' ? REJECTED_TOKEN_REASON : s.rejected === 'basic' ? REJECTED_PASSWORD_REASON : null;
  return { serverUrl: s.serverUrl, username: s.username, useToken: s.rejected !== 'basic', reason };
}
