// Settings › Connection (design §10): the server URL, the credential's
// presence, and when Reconnect applies. Pure. Nothing here can hold or
// format a secret: the session store never has one, and the label below
// states presence only.

export const CREDENTIAL_STORED = 'Stored in the OS keyring';

/**
 * Once the shell is up a credential is in the keyring by construction —
 * `restore_session` answers `connected` or `unreachable` only when one is
 * stored. The two states differ in whether the server has accepted it.
 */
export function credentialStatusLabel(connected: boolean): string {
  return connected
    ? `${CREDENTIAL_STORED} · session verified`
    : `${CREDENTIAL_STORED} · not verified (server unreachable)`;
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
