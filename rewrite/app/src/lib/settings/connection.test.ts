import { describe, expect, it } from 'vitest';
import {
  canConnect,
  CREDENTIAL_STORED,
  credentialStatusLabel,
  OPEN_CONFIG_FOLDER_LABEL,
  reconnectEnabled,
  serverLine,
} from './connection';

describe('credentialStatusLabel', () => {
  it('reports presence only, never a value (token secrecy)', () => {
    expect(credentialStatusLabel(true)).toBe(`${CREDENTIAL_STORED} · session verified`);
    expect(credentialStatusLabel(false)).toBe(`${CREDENTIAL_STORED} · not verified (server unreachable)`);
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
