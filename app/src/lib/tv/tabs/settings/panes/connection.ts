// Settings › Connection in TV mode (checklist items 243–244; tvmode.js
// `paneRows('connection')`): the server, the user and the credential's
// presence (never the credential: the session store has none), then
// Reconnect, Open RomM in browser, Edit connection… (the Connect screen as a
// page), Disconnect, which asks first and returns to Connect, and Open
// settings folder (ruling E1–3). Used without a server, as on the desktop, the
// way on is "Connect to a server…".
import { serverLine } from '../../../../settings/connection';
import { OPEN_FOLDER_SUB } from '../../../pages/drivePages';
import type { Row } from '../../../rows';
import { EDIT_CONNECTION_LABEL, EDIT_CONNECTION_SUB } from '../../../startup/connectPage';

export type ConnectionState = { connected: boolean; offline: boolean; serverUrl: string; username: string };

export type ConfirmWords = { title: string; body: string; ok: string };

export type ConnectionActions = {
  reconnect: () => void;
  connectToServer: () => void;
  /** Asks with `words`, then disconnects. */
  disconnect: (words: ConfirmWords) => void;
  /** The Connect screen as a page (ruling E3). */
  editConnection: () => void;
  /** Desktop mode, then the server's site / the config folder (ruling E1–2). */
  openServerPage: () => void;
  openConfigFolder: () => void;
};

export const OPEN_SERVER_SUB = "Opens your server's site in desktop mode";

export const CREDENTIAL_VERIFIED = 'Saved · verified';
export const CREDENTIAL_NOT_VERIFIED = 'Saved · not verified';

/** The credential line: present and accepted, or present while offline. */
export function credentialLine(s: Pick<ConnectionState, 'connected' | 'offline'>): string {
  if (s.offline && !s.connected) return 'Not used offline';
  return s.connected ? CREDENTIAL_VERIFIED : CREDENTIAL_NOT_VERIFIED;
}

export function disconnectWords(serverUrl: string): ConfirmWords {
  return {
    title: `Disconnect from ${serverUrl}?`,
    body: "You'll need to sign in again. Installed games, saves and settings stay.",
    ok: 'Disconnect',
  };
}

export function connectionRows(s: ConnectionState, a: ConnectionActions): Row[] {
  const head: Row[] = [
    { type: 'info', key: 'server', label: 'Server', value: serverLine(s.serverUrl) },
    { type: 'info', key: 'user', label: 'User', value: s.username.trim() === '' ? 'Not set' : s.username },
    { type: 'info', key: 'credential', label: 'Sign-in', value: credentialLine(s) },
  ];
  const configFolder: Row = { type: 'action', key: 'config-folder', label: 'Open settings folder', sub: OPEN_FOLDER_SUB, run: a.openConfigFolder };
  const serverPage: Row[] =
    s.serverUrl.trim() === '' ? [] : [{ type: 'action', key: 'server-page', label: 'Open RomM in browser', sub: OPEN_SERVER_SUB, run: a.openServerPage }];
  if (s.offline && !s.connected) {
    return [...head, { type: 'action', key: 'connect', label: 'Connect to a server…', run: a.connectToServer }, ...serverPage, configFolder];
  }
  return [
    ...head,
    { type: 'action', key: 'reconnect', label: 'Reconnect', run: a.reconnect },
    ...serverPage,
    { type: 'action', key: 'edit', label: EDIT_CONNECTION_LABEL, sub: EDIT_CONNECTION_SUB, run: a.editConnection },
    {
      type: 'action',
      key: 'disconnect',
      label: 'Disconnect',
      sub: 'Sign out of this server',
      danger: true,
      run: () => a.disconnect(disconnectWords(s.serverUrl)),
    },
    configFolder,
  ];
}
