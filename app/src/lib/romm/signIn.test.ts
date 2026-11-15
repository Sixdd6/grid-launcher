import { describe, expect, it } from 'vitest';
import { canSignIn, signInError, signInModeFor, signInStart } from './signIn';
import { rommStatus } from './fixtures.test-util';

describe('canSignIn', () => {
  it('needs the URL and the secret, plus the username for a password', () => {
    const f = { serverUrl: 'https://romm.example', username: '', secret: 'x', useToken: true };
    expect(canSignIn(f)).toBe(true);
    expect(canSignIn({ ...f, serverUrl: ' ' })).toBe(false);
    expect(canSignIn({ ...f, secret: '' })).toBe(false);
    expect(canSignIn({ ...f, useToken: false })).toBe(false);
    expect(canSignIn({ ...f, useToken: false, username: 'player' })).toBe(true);
  });
});

describe('signInStart', () => {
  it('fills the server and user, never the secret; auth_kind picks the field', () => {
    expect(signInStart(rommStatus({ auth_kind: 'basic' }), 'edit')).toEqual({
      serverUrl: 'https://romm.home.lan',
      username: 'player',
      useToken: false,
      accountReadOnly: false,
    });
    expect(signInStart(rommStatus({ auth_kind: null }), 'connect').useToken).toBe(true);
    expect(signInStart(rommStatus({ connection: 'expired' }), 'again').accountReadOnly).toBe(true);
  });
  it('picks the mode from the status', () => {
    expect(signInModeFor(rommStatus({ connection: 'expired' }))).toBe('again');
    expect(signInModeFor(rommStatus({ connection: 'expired' }), true)).toBe('edit');
    expect(signInModeFor(rommStatus({ connection: 'not_connected' }))).toBe('connect');
  });
});

describe('signInError', () => {
  it('words the backend errors', () => {
    expect(signInError('could not reach the server: connection refused', 'https://romm.example', true)).toBe(
      "Couldn't reach https://romm.example. Check the address, and that the server is running.",
    );
    expect(signInError('the server rejected the credentials', '', true)).toBe('RomM rejected this token (401). Check it, or make a new one.');
    expect(signInError('the server rejected the credentials', '', false)).toBe('RomM rejected the username or password (401).');
    expect(signInError("the token belongs to account 'guest', not 'player'", '', true)).toBe(
      'This token belongs to "guest", not "player". Use a token for player, or disconnect RomM first to sign in as someone else.',
    );
    expect(signInError('server error 500', '', true)).toBe('server error 500');
  });
});
