import { describe, expect, it } from 'vitest';
import { applyRestore, applyUnauthorizedEvent, chipLabel, hostOf, initialView, viewForDigit, viewLabel } from './shell';

describe('applyRestore', () => {
  it('maps no_session to the connect screen', () => {
    expect(applyRestore({ kind: 'no_session', server_url: '', username: '' }).phase).toBe('none');
  });
  it('carries a no_session server url and username through for the connect form', () => {
    expect(applyRestore({ kind: 'no_session', server_url: 'https://romm.example.test', username: 'importer' })).toEqual({
      phase: 'none', connected: false, serverUrl: 'https://romm.example.test', username: 'importer', lastError: null,
    });
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
    expect(s.rejected).toBeUndefined();
  });
  // Q6: a rejected credential is not "offline" — it goes to Connect with the
  // server, the username and the credential kind, and no error line.
  it('maps unauthorized to the connect screen with the rejected credential kind', () => {
    expect(applyRestore({ kind: 'unauthorized', server_url: 'https://h', username: 'u', auth_kind: 'token' })).toEqual({
      phase: 'none', connected: false, serverUrl: 'https://h', username: 'u', lastError: null, rejected: 'token',
    });
    expect(applyRestore({ kind: 'unauthorized', server_url: 'https://h', username: 'u', auth_kind: 'basic' }).rejected).toBe('basic');
  });
  it('leaves rejected unset for every other outcome', () => {
    expect(applyRestore({ kind: 'no_session', server_url: '', username: '' }).rejected).toBeUndefined();
    expect(applyRestore({ kind: 'connected', state: { connected: true, username: 'u', server_url: 'h' } }).rejected).toBeUndefined();
  });
});

describe('applyUnauthorizedEvent', () => {
  const info = { server_url: 'https://h', username: 'u', auth_kind: 'token' } as const;
  it('takes a live shell to the connect screen', () => {
    expect(applyUnauthorizedEvent('shell', info)).toEqual({
      phase: 'none', connected: false, serverUrl: 'https://h', username: 'u', lastError: null, rejected: 'token',
    });
  });
  it('ignores the event outside the shell, so a form being typed into is never reset', () => {
    expect(applyUnauthorizedEvent('none', info)).toBeNull();
    expect(applyUnauthorizedEvent('loading', info)).toBeNull();
  });
});

describe('initialView / viewLabel / viewForDigit / chipLabel / hostOf', () => {
  it('opens Server when connected and Library when offline (R2)', () => {
    expect(initialView(true)).toBe('server');
    expect(initialView(false)).toBe('library');
  });
  it('labels every pill', () => {
    expect(viewLabel('library')).toBe('Library');
    expect(viewLabel('server')).toBe('Server');
    expect(viewLabel('downloads')).toBe('Downloads');
    expect(viewLabel('emulators')).toBe('Emulators');
    expect(viewLabel('settings')).toBe('Settings');
  });
  it('maps Ctrl+1..5 onto the pill order (design §3)', () => {
    expect(viewForDigit('1')).toBe('library');
    expect(viewForDigit('2')).toBe('server');
    expect(viewForDigit('3')).toBe('downloads');
    expect(viewForDigit('4')).toBe('emulators');
    expect(viewForDigit('5')).toBe('settings');
  });
  it('ignores every other key, including 0, 6 and non-digits', () => {
    expect(viewForDigit('0')).toBeNull();
    expect(viewForDigit('6')).toBeNull();
    expect(viewForDigit('f')).toBeNull();
    expect(viewForDigit('')).toBeNull();
    expect(viewForDigit('11')).toBeNull();
  });
  it('labels the chip', () => {
    expect(chipLabel({ phase: 'shell', connected: true, serverUrl: 'https://romm.example:8080/base', username: 'six', lastError: null })).toBe('six @ romm.example:8080');
    expect(chipLabel({ phase: 'shell', connected: false, serverUrl: 'https://x', username: 'six', lastError: 'e' })).toBe('Not connected');
  });
  it('hostOf falls back to the raw string', () => {
    expect(hostOf('not a url')).toBe('not a url');
  });
});
