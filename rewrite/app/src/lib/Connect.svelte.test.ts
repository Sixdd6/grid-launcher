// @vitest-environment node
//
// A Python import writes `server_url`/`username` into config.toml but no
// credential, so restore comes back `no_session` with both fields and the
// form must show them: the notice promises that only the token is missing.
import { afterEach, describe, expect, it } from 'vitest';
import { render } from 'svelte/server';
import Connect from './Connect.svelte';
import { session } from './stores/session.svelte';

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

  it('leaves the library path blank so an imported one survives', () => {
    session.serverUrl = 'https://romm.example.test';
    session.username = 'importer';
    const { body } = render(Connect);
    expect(input(body, 'library-path')).not.toMatch(/value="[^"]+"/);
  });
});
