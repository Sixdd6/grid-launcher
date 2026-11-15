import { describe, expect, it } from 'vitest';
import { canSignIn, reconnectedToast, signedInToast, signInError, signInModeFor, signInStart, usernameToSend } from './signIn';
import { signInCopy } from './words';
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

describe('usernameToSend', () => {
  it('sends the typed name for a password, the stored account for a token in Edit and Sign in again, none in Connect', () => {
    expect(usernameToSend({ username: ' sam ', useToken: false }, 'connect', 'player')).toBe('sam');
    expect(usernameToSend({ username: 'sam', useToken: true }, 'connect', 'player')).toBe('');
    expect(usernameToSend({ username: '', useToken: true }, 'edit', 'player')).toBe('player');
    expect(usernameToSend({ username: '', useToken: true }, 'again', ' player ')).toBe('player');
  });
});

describe('notifications', () => {
  it('words a sign-in by mode', () => {
    const s = rommStatus();
    expect(signedInToast('connect', s)).toBe('Connected to RomM at romm.home.lan as player');
    expect(signedInToast('edit', s)).toBe('Connected to RomM at romm.home.lan as player');
    expect(signedInToast('again', s)).toBe('Signed in to RomM again as player');
  });
  it('words a reconnect by where it came from, and nothing when it failed', () => {
    expect(reconnectedToast('connected', rommStatus())).toBe('Reconnected to romm.home.lan');
    expect(reconnectedToast('unreachable', rommStatus())).toBe('Connected to RomM again · romm.home.lan');
    expect(reconnectedToast('expired', rommStatus())).toBe('Connected to RomM again · romm.home.lan');
    expect(reconnectedToast('connected', rommStatus({ connection: 'unreachable' }))).toBeNull();
  });
});

describe('signInCopy', () => {
  it('names the password in Sign in again when the sign-in is a password', () => {
    expect(signInCopy('again', 'player').lede).toBe("RomM no longer accepts GRID's token for player. Enter a new one.");
    expect(signInCopy('again', 'player', false).lede).toBe("RomM no longer accepts GRID's password for player. Enter a new one.");
  });
});
