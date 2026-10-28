import { describe, expect, it } from 'vitest';
import { canConnect, libraryField, type ConnectFields } from './connectForm';

describe('libraryField', () => {
  it('asks for a folder on a first run, pre-filled with the suggestion', () => {
    expect(libraryField('', '/home/me/Games/GRID')).toEqual({ show: true, initial: '/home/me/Games/GRID' });
  });

  it('leaves the field empty when there is no suggestion', () => {
    expect(libraryField('  ', '')).toEqual({ show: true, initial: '' });
  });

  it('does not ask again when a folder is already stored', () => {
    expect(libraryField('/data/library', '/home/me/Games/GRID')).toEqual({ show: false, initial: '' });
  });
});

describe('canConnect', () => {
  const filled: ConnectFields = {
    serverUrl: 'https://romm.example',
    username: '',
    secret: 'token',
    useToken: true,
    libraryPath: '/home/me/Games/GRID',
  };

  it('is enabled when every shown field is filled', () => {
    expect(canConnect(filled)).toBe(true);
  });

  it('needs the server URL and the secret', () => {
    expect(canConnect({ ...filled, serverUrl: ' ' })).toBe(false);
    expect(canConnect({ ...filled, secret: '' })).toBe(false);
  });

  it('needs a username only in password mode', () => {
    expect(canConnect({ ...filled, useToken: false })).toBe(false);
    expect(canConnect({ ...filled, useToken: false, username: 'me' })).toBe(true);
  });

  it('needs a library folder only when it asks for one', () => {
    expect(canConnect({ ...filled, libraryPath: '  ' })).toBe(false);
    expect(canConnect({ ...filled, libraryPath: null })).toBe(true);
  });
});
