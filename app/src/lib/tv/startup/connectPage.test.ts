import { describe, expect, it } from 'vitest';
import {
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
