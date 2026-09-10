// @vitest-environment node
//
// The startup Python-import notice is pushed before any session exists, so
// the toast host must live above `App`'s phase branch — with it inside
// `Shell.svelte` the one message written for an upgrading user rendered
// nowhere (whole-branch review H1).
import { afterEach, describe, expect, it } from 'vitest';
import { render } from 'svelte/server';
import App from './App.svelte';
import { session } from './lib/stores/session.svelte';
import { dismissToast, pushToast, toasts } from './lib/stores/toasts.svelte';

const NOTICE = 'Imported 1 emulator and 2 games from the previous version. Enter your RomM token to reconnect.';

afterEach(() => {
  for (const t of [...toasts.list]) dismissToast(t.id);
  session.phase = 'loading';
});

describe('App', () => {
  it('renders a toast in the connect phase', () => {
    session.phase = 'none';
    pushToast(NOTICE);
    const { body } = render(App);
    expect(body).toContain('data-testid="toast-region"');
    expect(body).toContain(NOTICE);
  });

  it('renders a toast while the session is still loading', () => {
    pushToast(NOTICE);
    const { body } = render(App);
    expect(body).toContain(NOTICE);
  });
});
