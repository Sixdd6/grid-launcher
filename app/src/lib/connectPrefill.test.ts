import { describe, expect, it } from 'vitest';
import { connectPrefill, REJECTED_PASSWORD_REASON, REJECTED_TOKEN_REASON } from './connectPrefill';

describe('connectPrefill', () => {
  it('starts blank, in token mode, with no reason when nothing is stored', () => {
    expect(connectPrefill({ serverUrl: '', username: '' })).toEqual({
      serverUrl: '', username: '', useToken: true, reason: null,
    });
  });

  it('carries an imported server and username with no reason (Python import)', () => {
    expect(connectPrefill({ serverUrl: 'https://romm.example.test', username: 'importer' })).toEqual({
      serverUrl: 'https://romm.example.test', username: 'importer', useToken: true, reason: null,
    });
  });

  it('asks for a new token after a rejected token', () => {
    expect(connectPrefill({ serverUrl: 'https://h', username: 'six', rejected: 'token' })).toEqual({
      serverUrl: 'https://h', username: 'six', useToken: true, reason: REJECTED_TOKEN_REASON,
    });
    expect(REJECTED_TOKEN_REASON).toBe('The server rejected your token. Enter a new one.');
  });

  it('switches to password mode after a rejected password', () => {
    expect(connectPrefill({ serverUrl: 'https://h', username: 'six', rejected: 'basic' })).toEqual({
      serverUrl: 'https://h', username: 'six', useToken: false, reason: REJECTED_PASSWORD_REASON,
    });
  });

  it('never has a secret field to fill', () => {
    const prefill = connectPrefill({ serverUrl: 'https://h', username: 'six', rejected: 'token' });
    expect(Object.keys(prefill).sort()).toEqual(['reason', 'serverUrl', 'useToken', 'username']);
  });
});
