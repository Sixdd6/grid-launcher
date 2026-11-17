import { describe, expect, it, vi } from 'vitest';
import { rommStatus } from '../../romm/fixtures.test-util';
import {
  canPasteInto,
  errorField,
  installedChip,
  installedCountLine,
  installedEmpty,
  needsPanel,
  reconnectCard,
  rommFix,
  rommPageMain,
  rommPageRows,
  rommPageSub,
  signInFields,
  signInHelp,
  signInWhy,
  signInWords,
} from './rommTv';
import type { PageRow } from '../pages/pageRows';

const acts = () => ({ edit: vi.fn(), showInLibrary: vi.fn(), openInBrowser: vi.fn(), disconnect: vi.fn() });
const summary = (rows: PageRow[]) => rows.map((r) => (r.kind === 'note' ? `note:${r.text.slice(0, 24)}` : r.kind === 'heading' ? `h:${r.text}` : r.label));
const sub = (s: ReturnType<typeof rommPageSub>) => `${s.before}${s.user}${s.after}`;

describe('the RomM page (TVRommStorefront README § The RomM page)', () => {
  it('connected: Server, User, Sign-in, Installed RomM games, Edit sign-in…, Show in Library, Open in browser, Disconnect…; no main button', () => {
    const s = rommStatus({ username: 'sam' });
    expect(sub(rommPageSub(s))).toBe('Signed in as sam · romm.home.lan');
    expect(summary(rommPageRows(s, 12, acts()))).toEqual([
      'Server',
      'User',
      'Sign-in',
      'Installed RomM games',
      'Edit sign-in…',
      'Show in Library',
      'Open in browser',
      'Disconnect…',
    ]);
    expect(rommPageMain(s)).toBeNull();
  });

  it("Can't reach: the warning, the same rows, Reconnect (Reconnecting… while it runs)", () => {
    const s = rommStatus({ username: 'sam', connection: 'unreachable', last_error: 'connection refused' });
    expect(sub(rommPageSub(s))).toBe("Signed in as sam · can't reach romm.home.lan");
    const rows = rommPageRows(s, 2, acts());
    expect(rows[0]).toMatchObject({ kind: 'note', warn: true });
    expect((rows[0] as { text: string }).text).toContain("Can't reach RomM. romm.home.lan: connection refused.");
    expect(rommPageMain(s)).toMatchObject({ label: 'Reconnect', action: 'reconnect', disabled: false });
    expect(rommPageMain({ ...s, busy: true })).toMatchObject({ label: 'Reconnecting…', disabled: true, why: 'GRID is trying RomM now.' });
  });

  it('Sign-in expired: the warning and Sign in again…', () => {
    const s = rommStatus({ username: 'sam', connection: 'expired' });
    expect(sub(rommPageSub(s))).toBe('Sign-in expired · romm.home.lan');
    expect((rommPageRows(s, 0, acts())[0] as { text: string }).text).toBe(
      'Your RomM sign-in expired. Sign in again to browse, install and sync saves. Installed games still play.',
    );
    expect(rommPageMain(s)).toMatchObject({ label: 'Sign in again…', action: 'signin' });
  });

  it('not connected: the note, server and user kept, Show in Library while games remain, Connect RomM…', () => {
    const s = rommStatus({ username: 'sam', connection: 'not_connected', in_rail: true });
    expect(sub(rommPageSub(s))).toBe('Not signed in · romm.home.lan');
    expect(summary(rommPageRows(s, 12, acts()))).toEqual(['note:Connect your RomM server', 'Server', 'User', 'Installed RomM games', 'Show in Library', 'Open in browser']);
    expect((rommPageRows(s, 12, acts())[0] as { text: string }).text).toContain('The 12 RomM games installed here still play.');
    expect(rommPageMain(s)).toMatchObject({ label: 'Connect RomM…', action: 'connect' });
  });

  it('never connected: "Your own game server", no server rows, no Show in Library', () => {
    const s = rommStatus({ connection: 'not_connected', server_url: '', username: '' });
    expect(sub(rommPageSub(s))).toBe('Your own game server');
    expect(summary(rommPageRows(s, 0, acts()))).toEqual(['note:Connect your RomM server', 'Installed RomM games']);
  });

  it('the rows run their actions', () => {
    const a = acts();
    const rows = rommPageRows(rommStatus(), 1, a);
    for (const r of rows) if (r.kind === 'row') r.run();
    expect(a.edit).toHaveBeenCalled();
    expect(a.showInLibrary).toHaveBeenCalled();
    expect(a.openInBrowser).toHaveBeenCalled();
    expect(a.disconnect).toHaveBeenCalled();
  });

  it('Reconnect cards per outcome', () => {
    expect(reconnectCard(rommStatus({ username: 'sam' }))).toEqual({ title: 'Connected to RomM as sam', sub: 'romm.home.lan', error: false, signIn: false });
    expect(reconnectCard(rommStatus({ connection: 'unreachable' }))).toMatchObject({ title: "Can't reach RomM", sub: "romm.home.lan didn't answer. Installed games still play.", error: true });
    expect(reconnectCard(rommStatus({ connection: 'expired' }))).toMatchObject({ title: 'RomM sign-in expired', signIn: true });
  });
});

describe('RomM › Installed (README § RomM › Installed)', () => {
  it('counts, and how many match while searching', () => {
    expect(installedCountLine(12, 12, false)).toBe('12 games');
    expect(installedCountLine(12, 3, true)).toBe('3 of 12 games match');
    expect(installedCountLine(1, 1, false)).toBe('1 game');
  });

  it('the chip: signed in, Not connected; none for the banner states', () => {
    expect(installedChip(rommStatus({ username: 'sam' }))).toBe('Signed in as sam');
    expect(installedChip(rommStatus({ connection: 'not_connected' }))).toBe('Not connected');
    expect(installedChip(rommStatus({ connection: 'unreachable' }))).toBeNull();
    expect(installedChip(rommStatus({ connection: 'expired' }))).toBeNull();
  });

  it('empty: Install from a platform while connected, else where to fix RomM; a search with no match', () => {
    expect(installedEmpty(rommStatus(), '')).toEqual({ title: 'No RomM games installed yet', body: 'Install games from a platform under it.' });
    expect(installedEmpty(rommStatus({ connection: 'not_connected' }), '').body).toBe('Connect RomM in Settings › Game stores.');
    expect(installedEmpty(rommStatus({ connection: 'expired' }), '').body).toBe('Your RomM sign-in expired. Sign in again in Settings › Game stores.');
    expect(installedEmpty(rommStatus({ connection: 'unreachable' }), '').body).toBe("Can't reach RomM right now. GRID tries again when you press Retry.");
    expect(installedEmpty(rommStatus(), ' zelda ')).toEqual({ title: 'No installed RomM games match "zelda"', body: 'Press (Y) to change the search.' });
    expect(rommFix(rommStatus({ connection: 'not_connected' }))).toBe('Connect RomM in Settings › Game stores.');
  });
});

describe('needs-RomM panels (README § Cloud saves and RetroAchievements)', () => {
  const text = 'GRID keeps your emulator saves.';
  it('none while connected', () => {
    expect(needsPanel(rommStatus(), 'Cloud saves need RomM', text)).toBeNull();
  });
  it('not connected: plain, Set up RomM… opens the RomM page', () => {
    expect(needsPanel(rommStatus({ connection: 'not_connected' }), 'Cloud saves need RomM', text)).toEqual({
      title: 'Cloud saves need RomM',
      text: "RomM isn't connected. GRID keeps your emulator saves.",
      warn: false,
      action: { kind: 'page', label: 'Set up RomM…', al: 'Open', disabled: false },
    });
  });
  it("expired: Sign in again…; can't reach: Retry, Retrying… while it runs", () => {
    expect(needsPanel(rommStatus({ connection: 'expired' }), 't', text)).toMatchObject({ warn: true, action: { kind: 'signin', label: 'Sign in again…' } });
    const off = needsPanel(rommStatus({ connection: 'unreachable' }), 't', text);
    expect(off?.text).toBe("GRID can't reach romm.home.lan right now. GRID keeps your emulator saves.");
    expect(off?.action).toMatchObject({ kind: 'retry', label: 'Retry', disabled: false });
    expect(needsPanel(rommStatus({ connection: 'unreachable', busy: true }), 't', text)?.action).toMatchObject({ label: 'Retrying…', disabled: true });
  });
  it('connecting at startup says so', () => {
    expect(needsPanel(rommStatus({ connection: 'not_connected', busy: true }), 't', text)?.text).toBe('GRID is still connecting to romm.home.lan. GRID keeps your emulator saves.');
  });
});

describe('the sign-in page (README § The RomM sign-in page)', () => {
  it("titles and buttons are desktop's words", () => {
    expect(signInWords('connect', '')).toMatchObject({ title: 'Connect to RomM', button: 'Connect' });
    expect(signInWords('edit', 'sam')).toMatchObject({ title: 'Edit RomM sign-in', button: 'Save and connect' });
    expect(signInWords('again', 'sam')).toMatchObject({ title: 'Sign in to RomM again', sub: "RomM no longer accepts GRID's token for sam. Enter a new one." });
  });

  it('fields: the server, Sign in with, then the token or the username and password; the secret as dots', () => {
    const token = signInFields({ url: '', how: 'token', user: '', secretTyped: false });
    expect(token.map((f) => [f.key, f.label, f.value, f.a])).toEqual([
      ['url', 'Server URL', 'https://…', 'Edit'],
      ['how', 'Sign in with', 'API token', 'Change'],
      ['secret', 'API token', 'Paste or type your token', 'Edit'],
    ]);
    const pw = signInFields({ url: 'https://romm.home.lan', how: 'password', user: 'sam', secretTyped: true });
    expect(pw.map((f) => [f.key, f.value])).toEqual([
      ['url', 'https://romm.home.lan'],
      ['how', 'Username and password'],
      ['user', 'sam'],
      ['secret', '••••••••••••'],
    ]);
  });

  it('says what is missing first', () => {
    expect(signInWhy({ url: '', how: 'token', user: '', secretTyped: true })).toBe('Enter your RomM server address first.');
    expect(signInWhy({ url: 'u', how: 'password', user: '', secretTyped: true })).toBe('Enter your RomM username first.');
    expect(signInWhy({ url: 'u', how: 'token', user: '', secretTyped: false })).toBe('Enter your API token first.');
    expect(signInWhy({ url: 'u', how: 'password', user: 'sam', secretTyped: false })).toBe('Enter your password first.');
    expect(signInWhy({ url: 'u', how: 'token', user: '', secretTyped: true })).toBeNull();
    expect(signInHelp('token')).toBe('Make a token in RomM under your user settings. GRID keeps it in the OS keyring, never in a file.');
  });

  it('a failure sends focus to the field to fix; View pastes into text fields only', () => {
    expect(errorField('the server rejected the credentials')).toBe('secret');
    expect(errorField('could not reach the server: refused')).toBe('url');
    expect(errorField('the token belongs to account')).toBeNull();
    expect(canPasteInto('url')).toBe(true);
    expect(canPasteInto('secret')).toBe(true);
    expect(canPasteInto('how')).toBe(false);
    expect(canPasteInto(null)).toBe(false);
  });
});
