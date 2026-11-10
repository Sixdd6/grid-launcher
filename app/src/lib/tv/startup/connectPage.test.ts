import { describe, expect, it } from 'vitest';
import {
  editButton,
  editFields,
  editPrefill,
  connectButtons,
  connectedText,
  connectFields,
  connectReady,
  connectWhy,
  isSecret,
  keyboardTitle,
  MASK,
  NO_SERVER_NOTE,
  offlineReady,
  parentOf,
  pasteRefusal,
  type ConnectState,
} from './connectPage';

const blank: ConnectState = { url: '', how: 'token', user: '', secretTyped: false, lib: '' };

describe('Connect rows (items 393–394)', () => {
  it('shows Server, Sign in with, API token and Library folder with their placeholders', () => {
    expect(connectFields(blank).map((f) => [f.key, f.label, f.value, f.placeholder, f.a])).toEqual([
      ['url', 'Server', 'https://…', true, 'Edit'],
      ['how', 'Sign in with', 'API token', false, 'Change'],
      ['token', 'API token', 'Paste or type your token', true, 'Edit'],
      ['lib', 'Library folder', '', true, 'Edit'],
    ]);
  });

  it('shows Username and Password for that sign-in', () => {
    const f = connectFields({ ...blank, how: 'password' });
    expect(f.map((x) => [x.label, x.value])).toEqual([
      ['Server', 'https://…'],
      ['Sign in with', 'Username and password'],
      ['Username', 'Your RomM user'],
      ['Password', 'Your password'],
      ['Library folder', ''],
    ]);
  });

  it('shows a typed secret only as dots, never its value or length', () => {
    const f = connectFields({ ...blank, url: 'http://romm.local', secretTyped: true, lib: '/games' });
    expect(f.find((x) => x.key === 'token')).toMatchObject({ value: MASK, placeholder: false });
    expect(f.find((x) => x.key === 'url')!.value).toBe('http://romm.local');
  });

  it('titles the keyboard per field; the token and password are masked', () => {
    expect(keyboardTitle('url')).toBe('Server address');
    expect(keyboardTitle('token')).toBe('API token');
    expect(keyboardTitle('user')).toBe('Username');
    expect(keyboardTitle('password')).toBe('Password');
    expect(['url', 'how', 'token', 'user', 'password', 'lib'].filter((k) => isSecret(k as never))).toEqual(['token', 'password']);
  });
});

describe('Connect enablement (item 397) and Use without a server (item 515)', () => {
  const filled: ConnectState = { url: 'http://romm.local', how: 'token', user: '', secretTyped: true, lib: '/games' };

  it('is ready once the server, the credentials and the library folder are filled', () => {
    expect(connectReady(filled)).toBe(true);
    expect(connectReady({ ...filled, url: ' ' })).toBe(false);
    expect(connectReady({ ...filled, secretTyped: false })).toBe(false);
    expect(connectReady({ ...filled, lib: '' })).toBe(false);
    expect(connectReady({ ...filled, how: 'password' })).toBe(false);
    expect(connectReady({ ...filled, how: 'password', user: 'sam' })).toBe(true);
  });

  it('says what is missing while disabled', () => {
    expect(connectWhy('token')).toBe('Fill in the server, the API token and the library folder first.');
    expect(connectWhy('password')).toBe('Fill in the server, the username and password and the library folder first.');
  });

  it('disables both buttons while a connect runs, Connect reading "Connecting…" (consultant, after P12)', () => {
    const why = 'Still connecting. Wait for it to finish.';
    expect(connectButtons(filled, true)).toEqual({
      go: { label: 'Connecting…', disabled: true, why },
      offline: { label: 'Use without a server', disabled: true, why },
    });
    expect(connectButtons(filled, false).go).toEqual({ label: 'Connect', disabled: false, why: connectWhy('token') });
    expect(connectButtons(filled, false).offline).toEqual({ label: 'Use without a server', disabled: false });
    expect(connectButtons(blank, false).go.disabled).toBe(true);
    expect(connectButtons(blank, false).offline.disabled).toBe(true);
  });

  it('says why View pasted nothing (consultant, after P12)', () => {
    expect(pasteRefusal(null, 'x')).toBe('Focus a field to paste into it.');
    expect(pasteRefusal('how', 'x')).toBe('Focus a field to paste into it.');
    expect(pasteRefusal('url', '  ')).toBe('Nothing to paste: the clipboard has no text.');
    expect(pasteRefusal('token', 'abc')).toBeNull();
  });

  it('needs only the library folder without a server', () => {
    expect(offlineReady({ lib: '' })).toBe(false);
    expect(offlineReady({ lib: '/games' })).toBe(true);
    expect(NO_SERVER_NOTE).toBe('Without a server, GRID needs only the library folder. Your Library and Backup drives work offline.');
  });

  it('notifies "Connected to <URL>" (item 399)', () => {
    expect(connectedText('http://romm.local')).toBe('Connected to http://romm.local');
  });

  it('opens the folder browser on the folder that holds the path', () => {
    expect(parentOf('/home/sam/Games/GRID')).toBe('/home/sam/Games');
    expect(parentOf('/home/sam/Games/GRID/')).toBe('/home/sam/Games');
    expect(parentOf('/GRID')).toBe('/');
    expect(parentOf('')).toBe('');
    expect(parentOf('C:\\Games\\GRID')).toBe('C:\\Games');
  });
});

describe('Edit connection… (ruling E3)', () => {
  const st = { url: 'https://romm.lan', how: 'token' as const, user: '', secretTyped: false, lib: '' };

  it('starts from the session, signing in with a token as the desktop edit form does', () => {
    expect(editPrefill({ serverUrl: 'https://romm.lan', username: 'sam' })).toEqual({ serverUrl: 'https://romm.lan', username: 'sam', useToken: true, reason: null });
  });

  it("shows Connect's rows without the library folder; Connect needs no folder", () => {
    expect(editFields(st).map((f) => f.key)).toEqual(['url', 'how', 'token']);
    expect(editFields({ ...st, how: 'password' }).map((f) => f.key)).toEqual(['url', 'how', 'user', 'password']);
    expect(editButton(st, false)).toMatchObject({ label: 'Connect', disabled: true, why: 'Fill in the server and the API token first.' });
    expect(editButton({ ...st, secretTyped: true }, false).disabled).toBe(false);
    expect(editButton({ ...st, secretTyped: true }, true)).toMatchObject({ label: 'Connecting…', disabled: true });
  });
});
