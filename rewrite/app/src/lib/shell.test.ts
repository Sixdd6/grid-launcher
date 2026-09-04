import { describe, expect, it } from 'vitest';
import { applyRestore, chipLabel, hostOf, initialView, viewForDigit, viewLabel } from './shell';

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
