import { describe, expect, it } from 'vitest';
import type { RestoreOutcome } from './api';
import {
  applyRestore,
  applyRetry,
  applyUnauthorizedEvent,
  chipLabel,
  hostOf,
  initialView,
  offlineSession,
  viewForDigit,
  viewLabel,
} from './shell';

describe('applyRestore', () => {
  it('maps no_session to the connect screen', () => {
    expect(applyRestore({ kind: 'no_session', server_url: '', username: '' }).phase).toBe('none');
  });
  it('carries a no_session server url and username through for the connect form', () => {
    expect(applyRestore({ kind: 'no_session', server_url: 'https://romm.example.test', username: 'importer' })).toEqual({
      phase: 'none', connected: false, serverUrl: 'https://romm.example.test', username: 'importer', lastError: null, offline: false,
    });
  });
  it('maps connected to the shell, connected', () => {
    const s = applyRestore({ kind: 'connected', state: { connected: true, username: 'u', server_url: 'https://h:1' } });
    expect(s).toEqual({ phase: 'shell', connected: true, serverUrl: 'https://h:1', username: 'u', lastError: null, offline: false });
  });
  it('maps unreachable to the shell, offline, with the error', () => {
    const s = applyRestore({ kind: 'unreachable', server_url: 'https://h', username: 'u', error: 'boom' });
    expect(s.phase).toBe('shell');
    expect(s.connected).toBe(false);
    expect(s.lastError).toBe('boom');
    expect(s.rejected).toBeUndefined();
    // An unreachable server is not offline mode: it keeps the error line.
    expect(s.offline).toBe(false);
  });
  it('maps offline mode to the shell, not connected, offline, with no error', () => {
    expect(applyRestore({ kind: 'offline', server_url: 'https://h', username: 'u' })).toEqual({
      phase: 'shell', connected: false, serverUrl: 'https://h', username: 'u', lastError: null, offline: true,
    });
  });
  it('sets offline only for the offline outcome', () => {
    const others: RestoreOutcome[] = [
      { kind: 'no_session', server_url: '', username: '' },
      { kind: 'connected', state: { connected: true, username: 'u', server_url: 'https://h' } },
      { kind: 'unauthorized', server_url: 'https://h', username: 'u', auth_kind: 'basic' },
      { kind: 'unreachable', server_url: 'https://h', username: 'u', error: 'boom' },
    ];
    for (const outcome of others) expect(applyRestore(outcome).offline).toBe(false);
  });
  // Q6: a rejected credential is not "offline" — it goes to Connect with the
  // server, the username and the credential kind, and no error line.
  it('maps unauthorized to the connect screen with the rejected credential kind', () => {
    expect(applyRestore({ kind: 'unauthorized', server_url: 'https://h', username: 'u', auth_kind: 'token' })).toEqual({
      phase: 'none', connected: false, serverUrl: 'https://h', username: 'u', lastError: null, offline: false, rejected: 'token',
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
      phase: 'none', connected: false, serverUrl: 'https://h', username: 'u', lastError: null, offline: false, rejected: 'token',
    });
  });
  it('takes the first-run platform picker to the connect screen too', () => {
    expect(applyUnauthorizedEvent('setup', info)?.phase).toBe('none');
  });
  it('ignores the event outside a live session, so a form being typed into is never reset', () => {
    expect(applyUnauthorizedEvent('none', info)).toBeNull();
    expect(applyUnauthorizedEvent('loading', info)).toBeNull();
  });
});

describe('applyRetry / offlineSession', () => {
  const connected: RestoreOutcome = { kind: 'connected', state: { connected: true, username: 'u', server_url: 'https://h' } };
  const unreachable: RestoreOutcome = { kind: 'unreachable', server_url: 'https://h', username: 'u', error: 'boom' };
  it('leaves offline mode on any successful connection', () => {
    expect(applyRetry(true, connected)).toEqual(applyRestore(connected));
    expect(applyRetry(true, connected).offline).toBe(false);
  });
  it('keeps offline mode when the retry did not connect', () => {
    expect(applyRetry(true, unreachable)).toEqual({ ...applyRestore(unreachable), offline: true });
    expect(applyRetry(false, unreachable).offline).toBe(false);
    expect(applyRetry(true, { kind: 'unauthorized', server_url: 'https://h', username: 'u', auth_kind: 'token' }).offline).toBe(true);
  });
  it('opens the shell offline after "Use without a server", keeping server and username', () => {
    expect(offlineSession('https://h', 'u')).toEqual({
      phase: 'shell', connected: false, serverUrl: 'https://h', username: 'u', lastError: null, offline: true,
    });
  });
});

describe('initialView / viewLabel / viewForDigit / chipLabel / hostOf', () => {
  it('opens Server when connected and Library when not (R2)', () => {
    expect(initialView(true)).toEqual({ view: 'server', drive: null });
    expect(initialView(false)).toEqual({ view: 'library', drive: null });
  });
  it('opens Library on the first ready drive in offline mode', () => {
    expect(initialView(false, true, '/media/kit')).toEqual({ view: 'library', drive: '/media/kit' });
    expect(initialView(false, true, null)).toEqual({ view: 'library', drive: null });
    // Not offline (an unreachable server): no drive is picked.
    expect(initialView(false, false, '/media/kit')).toEqual({ view: 'library', drive: null });
    // Connected wins over a stale offline flag.
    expect(initialView(true, true, '/media/kit')).toEqual({ view: 'server', drive: null });
  });
  it('labels every pill', () => {
    expect(viewLabel('library')).toBe('Library');
    expect(viewLabel('server')).toBe('Server');
    expect(viewLabel('downloads')).toBe('Downloads');
    expect(viewLabel('settings')).toBe('Settings');
  });
  it('maps Ctrl+1..5 onto the pill order (design §3)', () => {
    expect(viewForDigit('1')).toBe('library');
    expect(viewForDigit('2')).toBe('server');
    expect(viewForDigit('3')).toBe('downloads');
    expect(viewForDigit('4')).toBe('settings');
  });
  it('ignores every other key, including 0, 6 and non-digits', () => {
    expect(viewForDigit('0')).toBeNull();
    expect(viewForDigit('5')).toBeNull();
    expect(viewForDigit('f')).toBeNull();
    expect(viewForDigit('')).toBeNull();
    expect(viewForDigit('11')).toBeNull();
  });
  it('labels the chip', () => {
    expect(
      chipLabel({ phase: 'shell', connected: true, serverUrl: 'https://romm.example:8080/base', username: 'six', lastError: null, offline: false }),
    ).toBe('six @ romm.example:8080');
    expect(chipLabel({ phase: 'shell', connected: false, serverUrl: 'https://x', username: 'six', lastError: 'e', offline: false })).toBe(
      'Not connected',
    );
    expect(chipLabel(offlineSession('https://x', 'six'))).toBe('Offline · no server');
    // A connection wins over a stale offline flag.
    expect(chipLabel({ connected: true, offline: true, serverUrl: 'https://x', username: 'six' })).toBe('six @ x');
  });
  it('hostOf falls back to the raw string', () => {
    expect(hostOf('not a url')).toBe('not a url');
  });
});
