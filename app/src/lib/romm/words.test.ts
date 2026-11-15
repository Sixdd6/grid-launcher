import { describe, expect, it } from 'vitest';
import {
  rommAccountLine,
  rommBanner,
  rommCardWarning,
  rommFixAction,
  rommInstalledChip,
  rommInstalledLine,
  rommNotConnectedText,
  rommPill,
  rommPlatformCountsLine,
  rommSignInKind,
  rommState,
  rommStatusLine,
  rommWhy,
  signInCopy,
} from './words';
import { rommStatus } from './fixtures.test-util';

const never = rommStatus({ connection: 'not_connected', server_url: '', username: '', auth_kind: null });
const off = rommStatus({ connection: 'not_connected' });
const connecting = rommStatus({ connection: 'not_connected', busy: true });
const unreachable = rommStatus({ connection: 'unreachable', last_error: 'connection refused' });
const expired = rommStatus({ connection: 'expired' });

describe('card', () => {
  it('pill per state', () => {
    expect(rommPill(rommStatus())).toEqual({ text: 'Connected', tone: 'ok' });
    expect(rommPill(connecting)).toEqual({ text: 'Connecting…', tone: 'busy' });
    expect(rommPill(unreachable)).toEqual({ text: "Can't reach", tone: 'warn' });
    expect(rommPill(expired)).toEqual({ text: 'Sign-in expired', tone: 'warn' });
    expect(rommPill(off)).toEqual({ text: 'Not connected', tone: 'off' });
    expect(rommPill(never)).toEqual({ text: 'Not connected', tone: 'off' });
  });

  it('account line per state', () => {
    expect(rommAccountLine(rommStatus())).toBe('Signed in as player · romm.home.lan');
    expect(rommAccountLine(unreachable)).toBe('Signed in as player · romm.home.lan');
    expect(rommAccountLine(off)).toBe('Not signed in · romm.home.lan');
    expect(rommAccountLine(never)).toBe('Your own game server');
  });

  it('warnings, status lines and the sign-in kind', () => {
    expect(rommCardWarning(unreachable)).toEqual({
      strong: "Can't reach RomM.",
      text: "romm.home.lan: connection refused. Installed RomM games still play; browsing, installs, cloud saves and firmware downloads wait until it's back.",
    });
    expect(rommCardWarning(expired)?.strong).toBe('Your RomM sign-in expired.');
    expect(rommCardWarning(rommStatus())).toBeNull();
    expect(rommStatusLine(unreachable)).toBe("Can't reach · connection refused");
    expect(rommStatusLine(expired)).toBe('Sign-in expired · RomM answered 401');
    expect(rommSignInKind({ auth_kind: 'basic' })).toBe('Password · kept in the OS keyring');
    expect(rommSignInKind({ auth_kind: null })).toBe('API token · kept in the OS keyring');
    expect(rommNotConnectedText(11)).toContain('The 11 RomM games installed here still play.');
    expect(rommNotConnectedText(0)).not.toContain('installed here');
  });

  it('states', () => {
    expect([rommStatus(), connecting, unreachable, expired, off, never].map(rommState)).toEqual([
      'connected',
      'connecting',
      'unreachable',
      'expired',
      'not_connected',
      'never',
    ]);
  });
});

describe('Library words', () => {
  it('banner only for Can\'t reach and Sign-in expired', () => {
    expect(rommBanner(unreachable)).toMatchObject({ kind: 'unreachable', strong: "Can't reach RomM", text: "romm.home.lan didn't answer. Installed games still play.", action: 'Retry' });
    expect(rommBanner(expired)).toMatchObject({ kind: 'expired', action: 'Sign in again…' });
    expect(rommBanner(off)).toBeNull();
    expect(rommBanner(rommStatus())).toBeNull();
  });

  it('Installed header chip and line', () => {
    expect(rommInstalledChip(rommStatus())).toBe('Signed in as player');
    expect(rommInstalledChip(connecting)).toBe('Connecting to romm.home.lan…');
    expect(rommInstalledChip(off)).toBe('Not connected · Connect…');
    expect(rommInstalledChip(unreachable)).toBeNull();
    expect(rommInstalledLine(11, '21.2 GB', 'romm.home.lan')).toBe('11 games · 21.2 GB · from romm.home.lan');
  });

  it('platform counts line', () => {
    expect(rommPlatformCountsLine(4, 2, 1)).toBe('4 games on RomM · 2 installed · 1 only on this computer');
    expect(rommPlatformCountsLine(1, 0, 0)).toBe('1 game on RomM');
  });
});

describe('needs-RomM', () => {
  it('why and fix action per state', () => {
    expect(rommWhy(off)).toBe("RomM isn't connected.");
    expect(rommWhy(unreachable)).toBe("GRID can't reach romm.home.lan right now.");
    expect(rommWhy(expired)).toBe('Your RomM sign-in expired.');
    expect(rommWhy(connecting)).toBe('GRID is still connecting to romm.home.lan.');
    expect(rommFixAction(off)).toBe('Open Game stores');
    expect(rommFixAction(unreachable)).toBe('Retry');
    expect(rommFixAction(expired)).toBe('Sign in again…');
    expect(rommFixAction(connecting)).toBeNull();
  });
});

describe('sign-in copy', () => {
  it('per mode', () => {
    expect(signInCopy('connect').button).toBe('Connect');
    expect(signInCopy('edit')).toMatchObject({ title: 'Edit RomM sign-in', button: 'Save and connect' });
    expect(signInCopy('again', 'player')).toMatchObject({
      title: 'Sign in to RomM again',
      lede: "RomM no longer accepts GRID's token for player. Enter a new one.",
      button: 'Sign in',
    });
  });
});
