import { describe, expect, it } from 'vitest';
import {
  canConnect,
  connectionActions,
  CREDENTIAL_STORED,
  credentialStatusLabel,
  OPEN_CONFIG_FOLDER_LABEL,
  reconnectEnabled,
  serverLine,
  statusLabel,
} from './connection';

describe('credentialStatusLabel', () => {
  it('reports presence only, never a value (token secrecy)', () => {
    expect(credentialStatusLabel(true)).toBe(`${CREDENTIAL_STORED} · session verified`);
    // Offline can mean unreachable, 403 or a server error: the label states
    // only that the session is not verified, and the Status line and error
    // banner carry the reason. A 401 never lands here — it opens Connect.
    expect(credentialStatusLabel(false)).toBe(`${CREDENTIAL_STORED} · not verified`);
  });

  it('says the credential is unused in offline mode', () => {
    expect(credentialStatusLabel(false, true)).toBe('Not used offline');
    expect(credentialStatusLabel(true, false)).toBe(`${CREDENTIAL_STORED} · session verified`);
  });
});

describe('statusLabel', () => {
  it('reads like the top bar chip', () => {
    expect(statusLabel(true, false)).toBe('Connected');
    expect(statusLabel(false, false)).toBe('Not connected');
    expect(statusLabel(false, true)).toBe('Offline · no server');
  });
});

describe('connectionActions', () => {
  it('offers only Connect to a server in offline mode', () => {
    expect(connectionActions(false, true)).toEqual(['connect']);
  });

  it('keeps edit, reconnect and disconnect otherwise', () => {
    expect(connectionActions(true, false)).toEqual(['edit', 'reconnect', 'disconnect']);
    expect(connectionActions(false, false)).toEqual(['edit', 'reconnect', 'disconnect']);
  });
});

describe('reconnectEnabled', () => {
  it('offers Reconnect only while offline and idle', () => {
    expect(reconnectEnabled(false, false)).toBe(true);
    expect(reconnectEnabled(false, true)).toBe(false);
    expect(reconnectEnabled(true, false)).toBe(false);
  });
});

describe('serverLine', () => {
  it('shows the stored URL, or Not set', () => {
    expect(serverLine('https://romm.example:8080/base')).toBe('https://romm.example:8080/base');
    expect(serverLine('')).toBe('Not set');
    expect(serverLine('   ')).toBe('Not set');
  });
});

describe('OPEN_CONFIG_FOLDER_LABEL', () => {
  it('is the reference button text verbatim', () => {
    expect(OPEN_CONFIG_FOLDER_LABEL).toBe('Open Config Folder');
  });
});

describe('canConnect', () => {
  it('needs a server URL and a secret', () => {
    expect(canConnect('https://romm.example', 'tok')).toBe(true);
    expect(canConnect('', 'tok')).toBe(false);
    expect(canConnect('   ', 'tok')).toBe(false);
    expect(canConnect('https://romm.example', '')).toBe(false);
  });

  it('does not trim the secret', () => {
    expect(canConnect('https://romm.example', '  ')).toBe(true);
  });
});
