import { describe, expect, it } from 'vitest';
import { applyRestore, chipLabel, hostOf, initialSection } from './shell';

describe('applyRestore', () => {
  it('maps no_session to the connect screen', () => {
    expect(applyRestore({ kind: 'no_session' }).phase).toBe('none');
  });
  it('maps connected to the shell, connected', () => {
    const s = applyRestore({ kind: 'connected', state: { connected: true, username: 'u', server_url: 'https://h:1' } });
    expect(s).toEqual({ phase: 'shell', connected: true, serverUrl: 'https://h:1', username: 'u', lastError: null });
  });
  it('maps unreachable to the shell, offline, with the error', () => {
    const s = applyRestore({ kind: 'unreachable', server_url: 'https://h', username: 'u', error: 'boom' });
    expect(s.phase).toBe('shell');
    expect(s.connected).toBe(false);
    expect(s.lastError).toBe('boom');
  });
});

describe('initialSection / chipLabel / hostOf', () => {
  it('opens Server when connected and Library when offline (R2)', () => {
    expect(initialSection(true)).toBe('server');
    expect(initialSection(false)).toBe('library');
  });
  it('labels the chip', () => {
    expect(chipLabel({ phase: 'shell', connected: true, serverUrl: 'https://romm.example:8080/base', username: 'six', lastError: null })).toBe('six @ romm.example:8080');
    expect(chipLabel({ phase: 'shell', connected: false, serverUrl: 'https://x', username: 'six', lastError: 'e' })).toBe('Not connected');
  });
  it('hostOf falls back to the raw string', () => {
    expect(hostOf('not a url')).toBe('not a url');
  });
});
