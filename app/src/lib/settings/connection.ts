// Settings › Connection (design §10): the server URL, the credential's
// presence, and when Reconnect applies. Pure. Nothing here can hold or
// format a secret: the session store never has one, and the label below
// states presence only.

export const CREDENTIAL_STORED = 'Stored in the OS keyring';

/**
 * Once the shell is up a credential is in the keyring by construction —
 * `restore_session` answers `connected` or `unreachable` only when one is
 * stored. The two states differ in whether the server has accepted it.
 * "Not verified" names no cause: offline can mean an unreachable server, a
 * 403 or a server error, and the Status line and error banner carry the
 * reason. A rejected credential (401) never reaches this page — it opens the
 * Connect form instead (Q6).
 */
export function credentialStatusLabel(connected: boolean, offline = false): string {
  // Offline mode may have no credential at all (Use without a server on a
  // fresh profile), and any stored one goes unused: say that, not "stored".
  if (offline && !connected) return 'Not used offline';
  return connected
    ? `${CREDENTIAL_STORED} · session verified`
    : `${CREDENTIAL_STORED} · not verified`;
}

/** The Status line. Offline mode is chosen, so it reads as the top bar's
 *  chip does rather than as a failure. */
export function statusLabel(connected: boolean, offline: boolean): string {
  if (connected) return 'Connected';
  return offline ? 'Offline · no server' : 'Not connected';
}

export type ConnectionAction = 'edit' | 'reconnect' | 'disconnect' | 'connect';

/**
 * The page's session buttons, in order (Open Config Folder always follows).
 * Offline mode mirrors the chip menu: its only way out is Connect to a
 * server…, back to the Connect screen; there is no session to edit, retry
 * or end.
 */
export function connectionActions(connected: boolean, offline: boolean): ConnectionAction[] {
  return offline && !connected ? ['connect'] : ['edit', 'reconnect', 'disconnect'];
}

/** Mirrors the server menu: Reconnect exists only while offline, and not mid-retry. */
export function reconnectEnabled(connected: boolean, busy: boolean): boolean {
  return !connected && !busy;
}

export function serverLine(serverUrl: string): string {
  const trimmed = serverUrl.trim();
  return trimmed === '' ? 'Not set' : trimmed;
}

/** `_open_config_folder`'s button text (grid-launcher.py:3163), verbatim. */
export const OPEN_CONFIG_FOLDER_LABEL = 'Open Config Folder';

/**
 * The Edit-connection form's submit gate. Mirrors the `required` attributes
 * on `Connect.svelte`'s two fields: a URL that is more than whitespace and a
 * non-empty secret. The secret is not trimmed — a token or password may
 * legitimately end in one.
 */
export function canConnect(serverUrl: string, secret: string): boolean {
  return serverUrl.trim() !== '' && secret !== '';
}
