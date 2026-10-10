// @vitest-environment node
//
// A Python import writes `server_url`/`username` into config.toml but no
// credential, so restore comes back `no_session` with both fields and the
// form must show them: the notice promises that only the token is missing.
import { afterEach, describe, expect, it } from 'vitest';
import { render } from 'svelte/server';
import Connect from './Connect.svelte';
import { session } from './stores/session.svelte';
import { connectPrefill, REJECTED_PASSWORD_REASON } from './connectPrefill';

/** The rendered `<input …data-testid="connect-<name>">` tag, or ''. */
function input(body: string, name: string): string {
  return body.match(new RegExp(`<input[^>]*data-testid="connect-${name}"[^>]*>`))?.[0] ?? '';
}

afterEach(() => {
  session.serverUrl = '';
  session.username = '';
});

describe('Connect', () => {
  it('prefills the server url from the restored session', () => {
    session.serverUrl = 'https://romm.example.test';
    session.username = 'importer';
    const { body } = render(Connect);
    expect(input(body, 'server-url')).toContain('value="https://romm.example.test"');
  });

  it('starts blank when there is nothing stored', () => {
    const { body } = render(Connect);
    expect(input(body, 'server-url')).not.toMatch(/value="[^"]+"/);
  });

  // The username input is behind `useToken` (on by default), so its prefill
  // is asserted by the `python-import` e2e stage, which clears the checkbox.

  // Q6: a rejected token re-opens this form pre-filled, with the reason.
  it('renders a rejected-token prefill: URL filled, secret empty, reason shown', () => {
    const { body } = render(Connect, {
      props: { prefill: connectPrefill({ serverUrl: 'https://romm.example.test', username: 'six', rejected: 'token' }) },
    });
    expect(input(body, 'server-url')).toContain('value="https://romm.example.test"');
    expect(input(body, 'secret')).not.toMatch(/value="[^"]+"/);
    expect(body).toMatch(/data-testid="connect-reason"[^>]*>The server rejected your token\. Enter a new one\.</);
    expect(input(body, 'use-token')).toMatch(/checked/);
  });

  it('renders a rejected-password prefill in password mode with the username', () => {
    const { body } = render(Connect, {
      props: { prefill: connectPrefill({ serverUrl: 'https://h', username: 'six', rejected: 'basic' }) },
    });
    expect(input(body, 'username')).toContain('value="six"');
    expect(input(body, 'use-token')).not.toMatch(/checked/);
    expect(body).toContain('Password');
    expect(body).toContain(REJECTED_PASSWORD_REASON);
  });

  it('shows no reason line on a first run', () => {
    const { body } = render(Connect);
    expect(body).not.toContain('data-testid="connect-reason"');
  });

  it('leaves the library path blank so an imported one survives', () => {
    session.serverUrl = 'https://romm.example.test';
    session.username = 'importer';
    const { body } = render(Connect);
    expect(input(body, 'library-path')).not.toMatch(/value="[^"]+"/);
  });
});
