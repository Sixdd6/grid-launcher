// Settings › Connection in TV mode (checklist items 243–244; tvmode.js
// `paneRows('connection')`): the server, the user and the credential's
// presence (never the credential: the session store has none), then
// Reconnect and Disconnect, which asks first and returns to Connect. Used
// without a server, as on the desktop, the only way on is "Connect to a
// server…".
import { serverLine } from '../../../../settings/connection';
import type { Row } from '../../../rows';

export type ConnectionState = { connected: boolean; offline: boolean; serverUrl: string; username: string };

export type ConfirmWords = { title: string; body: string; ok: string };

export type ConnectionActions = {
  reconnect: () => void;
  connectToServer: () => void;
  /** Asks with `words`, then disconnects. */
  disconnect: (words: ConfirmWords) => void;
};

export const CREDENTIAL_VERIFIED = 'Stored in the system keyring · verified';
export const CREDENTIAL_NOT_VERIFIED = 'Stored in the system keyring · not verified';

/** The credential line: present and accepted, or present while offline. */
export function credentialLine(s: Pick<ConnectionState, 'connected' | 'offline'>): string {
  if (s.offline && !s.connected) return 'Not used offline';
  return s.connected ? CREDENTIAL_VERIFIED : CREDENTIAL_NOT_VERIFIED;
}

export function disconnectWords(serverUrl: string): ConfirmWords {
  return {
    title: `Disconnect from ${serverUrl}?`,
    body: 'GRID returns to Connect. Installed games, saves and settings stay on this computer.',
    ok: 'Disconnect',
  };
}

export function connectionRows(s: ConnectionState, a: ConnectionActions): Row[] {
  const head: Row[] = [
    { type: 'info', key: 'server', label: 'Server', value: serverLine(s.serverUrl) },
    { type: 'info', key: 'user', label: 'User', value: s.username.trim() === '' ? 'Not set' : s.username },
    { type: 'info', key: 'credential', label: 'Credential', value: credentialLine(s) },
  ];
  if (s.offline && !s.connected) {
    return [...head, { type: 'action', key: 'connect', label: 'Connect to a server…', run: a.connectToServer }];
  }
  return [
    ...head,
    { type: 'action', key: 'reconnect', label: 'Reconnect', run: a.reconnect },
    {
      type: 'action',
      key: 'disconnect',
      label: 'Disconnect',
      sub: 'Returns to Connect. Your library and settings stay.',
      danger: true,
      run: () => a.disconnect(disconnectWords(s.serverUrl)),
    },
  ];
}
