import { describe, expect, it } from 'vitest';
import { rommStatus } from '../romm/fixtures.test-util';
import { rommCardGamesLine, rommCardStatusLine, rommCardView } from './rommCard';

const ids = (v: ReturnType<typeof rommCardView>) => v.actions.map((a) => a.id);
const field = (v: ReturnType<typeof rommCardView>, key: string) => v.fields.find((f) => f.key === key)?.value;

describe('rommCardView', () => {
  it('Connected: pill, sub-line, five fields, Show in Library first', () => {
    const v = rommCardView(rommStatus({ installed_count: 11 }), 45);
    expect(v.off).toBe(false);
    expect(v.pill).toEqual({ text: 'Connected', tone: 'ok' });
    expect(v.account).toBe('Signed in as player · romm.home.lan');
    expect(v.warning).toBeNull();
    expect(v.lede).toBeNull();
    expect(v.fields.map((f) => f.label)).toEqual(['Server', 'User', 'Sign-in', 'Status', 'Games']);
    expect(v.fields[0]).toMatchObject({ value: 'https://romm.home.lan', link: true });
    expect(field(v, 'signin')).toBe('API token · kept in the OS keyring');
    expect(field(v, 'status')).toBe('Connected');
    expect(field(v, 'games')).toBe('45 on the server · 11 installed here');
    expect(ids(v)).toEqual(['show', 'edit', 'reconnect', 'open', 'disconnect']);
    expect(v.actions[0]).toMatchObject({ label: 'Show in Library', primary: true });
    expect(v.actions[2].label).toBe('Reconnect');
    expect(v.actions[4]).toMatchObject({ label: 'Disconnect…', destructive: true });
  });

  it('Reconnecting: the pill spins and Reconnect reads Reconnecting…, disabled', () => {
    const v = rommCardView(rommStatus(), 45, true);
    expect(v.actions.find((a) => a.id === 'reconnect')).toMatchObject({ label: 'Reconnecting…', disabled: true });
    expect(v.pill).toEqual({ text: 'Connecting…', tone: 'busy' });
    const busy = rommCardView(rommStatus({ busy: true }), 45);
    expect(busy.pill).toEqual({ text: 'Connecting…', tone: 'busy' });
    expect(field(busy, 'status')).toBe('Connecting to romm.home.lan…');
  });

  it("Can't reach: warning with the reason, Retry first, no Reconnect or Show", () => {
    const v = rommCardView(rommStatus({ connection: 'unreachable', last_error: 'connection refused', installed_count: 3 }), null);
    expect(v.pill).toEqual({ text: "Can't reach", tone: 'warn' });
    expect(v.warning?.strong).toBe("Can't reach RomM.");
    expect(v.warning?.text).toContain('romm.home.lan: connection refused.');
    expect(field(v, 'status')).toBe("Can't reach · connection refused");
    expect(field(v, 'games')).toBe('3 installed here');
    expect(ids(v)).toEqual(['retry', 'edit', 'open', 'disconnect']);
    expect(rommCardView(rommStatus({ connection: 'unreachable' }), null, true).actions[0]).toMatchObject({
      label: 'Retrying…',
      disabled: true,
    });
  });

  it('Sign-in expired: warning, Sign in again… first', () => {
    const v = rommCardView(rommStatus({ connection: 'expired' }), null);
    expect(v.pill).toEqual({ text: 'Sign-in expired', tone: 'warn' });
    expect(v.warning?.strong).toBe('Your RomM sign-in expired.');
    expect(field(v, 'status')).toBe('Sign-in expired · RomM answered 401');
    expect(ids(v)).toEqual(['signin', 'edit', 'open', 'disconnect']);
    expect(v.actions[0]).toMatchObject({ label: 'Sign in again…', primary: true });
  });

  it('Not connected with a stored server: dashed, lede, three fields, Connect and Open in browser', () => {
    const v = rommCardView(rommStatus({ connection: 'not_connected', installed_count: 11 }), 45);
    expect(v.off).toBe(true);
    expect(v.pill).toEqual({ text: 'Not connected', tone: 'off' });
    expect(v.account).toBe('Not signed in · romm.home.lan');
    expect(v.lede).toBe(
      'Connect your RomM server to browse its games in the Library, install them with an emulator, and sync their saves and firmware. The 11 RomM games installed here still play.',
    );
    expect(v.fields.map((f) => [f.label, f.value, f.link])).toEqual([
      ['Server', 'https://romm.home.lan', false],
      ['User', 'player', false],
      ['Status', 'Not connected', false],
    ]);
    expect(ids(v)).toEqual(['connect', 'open']);
    expect(v.actions[0]).toMatchObject({ label: 'Connect RomM…', primary: true });
  });

  it('Never connected: dashed, "Your own game server", no fields, Connect only', () => {
    const v = rommCardView(rommStatus({ connection: 'not_connected', server_url: '', username: '' }), null);
    expect(v.off).toBe(true);
    expect(v.state).toBe('never');
    expect(v.account).toBe('Your own game server');
    expect(v.lede).toBe(
      'Connect your RomM server to browse its games in the Library, install them with an emulator, and sync their saves and firmware.',
    );
    expect(v.fields).toEqual([]);
    expect(ids(v)).toEqual(['connect']);
  });

  it('a connect from a card with no stored server stays dashed while it runs', () => {
    const v = rommCardView(rommStatus({ connection: 'not_connected', server_url: '', username: '', busy: true }), null);
    expect(v.off).toBe(true);
    expect(v.pill.tone).toBe('busy');
  });

  it('Connecting at startup (stored server): not dashed, Edit / Open / Disconnect, all but Open disabled', () => {
    const v = rommCardView(rommStatus({ connection: 'not_connected', busy: true }), null);
    expect(v.off).toBe(false);
    expect(v.pill).toEqual({ text: 'Connecting…', tone: 'busy' });
    expect(v.account).toBe('Signed in as player · romm.home.lan');
    expect(ids(v)).toEqual(['edit', 'open', 'disconnect']);
    expect(v.actions.map((a) => a.disabled)).toEqual([true, false, true]);
  });
});

describe('card lines', () => {
  it('Status and Games', () => {
    expect(rommCardStatusLine(rommStatus({ busy: true }))).toBe('Connecting to romm.home.lan…');
    expect(rommCardGamesLine(rommStatus({ installed_count: 1 }), null)).toBe('1 installed here');
    expect(rommCardGamesLine(rommStatus({ installed_count: 1 }), 0)).toBe('0 on the server · 1 installed here');
  });
});
