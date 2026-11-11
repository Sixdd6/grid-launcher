import { describe, expect, it, vi } from 'vitest';
import type { MovePreview, StartFreshPreview } from '../../../../api';
import type { ChoiceRow, Row, SliderRow, SwitchRow } from '../../../rows';
import { appearanceRows, blurFromPercent, blurPercent, fadeText, THEME_LINE } from './appearance';
import type { ChoiceSpec } from './choice';
import { cloudRows, delayText, retainText } from './cloud';
import { connectionRows, credentialLine, disconnectWords } from './connection';
import {
  browserStart,
  deleteLabel,
  deleteWords,
  freshNotice,
  libraryRows,
  MOVE_BLOCKED,
  moveListSub,
  moveListTitle,
  moveStartedNotice,
  refusalNotice,
} from './library';
import { LOGIN_NEEDS, LOGIN_NEEDS_PASSWORD, LOGIN_NEEDS_USER, loginWhy, raRows, SAVE_NEEDS, SAVE_NEEDS_TOKEN, SAVE_NEEDS_USER, saveWhy } from './ra';
import { ASSIGN_NEEDS_PAD, assignLine, NO_CONTROLLER, tvRows } from './tv';
import { lastCheckedText, updatesRows, versionText } from './updates';

const summary = (rows: Row[]) =>
  rows.map((r) => {
    const v = r.type === 'info' ? r.value : r.type === 'switch' ? r.on : r.type === 'choice' ? r.value : r.type === 'slider' ? r.format(r.value) : r.type === 'text' ? r.value : '';
    return [r.type, r.label, r.sub ?? '', v];
  });
const byKey = <T extends Row>(rows: Row[], key: string) => rows.find((r) => r.key === key) as T;
const noop = () => {};

describe('Appearance pane (items 234–240, 527)', () => {
  const acts = { open: vi.fn(), setOpenOn: vi.fn(), setTheme: vi.fn(), setCardDisplay: vi.fn(), setArt: vi.fn(), setFade: vi.fn(), setBlur: vi.fn(), setHardwareAcceleration: vi.fn() };
  const rows = appearanceRows({ openOn: 'automatic', theme: 'system', cardDisplay: 'rows', fade: 50, blur: 8, hardwareAcceleration: true }, acts);

  it('lists the rows in order with their lines, ending with the desktop-only info row', () => {
    expect(summary(rows)).toEqual([
      ['choice', 'Open GRID on', 'Here and in desktop mode', 'automatic'],
      ['choice', 'Theme', THEME_LINE, 'system'],
      ['choice', 'Game card display', 'Here and in desktop mode', 'rows'],
      ['switch', 'Background art', "The focused game's art, blurred, behind everything", true],
      ['slider', 'Background fade', '', '50%'],
      ['slider', 'Background blur', '', '20%'],
      ['switch', 'Hardware acceleration', 'Uses the graphics card. Restart to apply.', true],
      ['info', 'More grid options', '', 'In desktop mode'],
    ]);
    expect(THEME_LINE).toBe('Dark in TV mode');
  });

  it('offers the Open GRID on and Theme options, with a check on the current value', () => {
    const open = byKey<ChoiceRow>(rows, 'open-on');
    expect(open.options.map((o) => o[1])).toEqual(['Automatic', 'Server › Discover', 'Library']);
    expect(byKey<ChoiceRow>(rows, 'theme').options.map((o) => o[1])).toEqual(['Follow system', 'Dark', 'Light']);
    byKey<ChoiceRow>(rows, 'theme').choose();
    const spec = acts.open.mock.calls[0][0] as ChoiceSpec;
    expect(spec.title).toBe('Theme');
    expect(spec.value).toBe('system');
    spec.pick('light');
    expect(acts.setTheme).toHaveBeenCalledWith('light');
  });

  it('offers Game card display, shared with the desktop', () => {
    const display = byKey<ChoiceRow>(rows, 'card-display');
    expect(display.options.map((o) => o[1])).toEqual(['Equal height', 'Equal width']);
    display.choose();
    const spec = acts.open.mock.calls.at(-1)?.[0] as ChoiceSpec;
    expect(spec.title).toBe('Game card display');
    spec.pick('columns');
    expect(acts.setCardDisplay).toHaveBeenCalledWith('columns');
  });

  it('formats the sliders and maps blur to the stored 0–40 (items 237–238)', () => {
    const fade = byKey<SliderRow>(rows, 'fade');
    expect([fade.min, fade.max, fade.step]).toEqual([0, 100, 5]);
    expect(fadeText(100)).toBe('100% · off');
    expect(fadeText(35)).toBe('35%');
    const blur = byKey<SliderRow>(rows, 'blur');
    expect([blur.min, blur.max, blur.step]).toEqual([0, 100, 10]);
    expect(blurPercent(40)).toBe(100);
    expect(blurFromPercent(30)).toBe(12);
    blur.set(30);
    expect(acts.setBlur).toHaveBeenCalledWith(12);
    // Background art is off at a full fade (item 12).
    const off = appearanceRows({ openOn: 'library', theme: 'dark', cardDisplay: 'columns', fade: 100, blur: 0, hardwareAcceleration: false }, acts);
    expect(byKey<SwitchRow>(off, 'art').on).toBe(false);
    // "Dark in TV mode" describes Follow system only.
    expect(byKey<ChoiceRow>(off, 'theme').sub).toBeUndefined();
  });
});

describe('Cloud saves pane (items 241–242)', () => {
  const s = { download_on_launch: true, upload_on_exit: false, skip_if_local_newer: true, upload_delay_seconds: 10, retention_limit: 0 };

  it('has three switches and two sliders with their ranges and words', () => {
    const rows = cloudRows(s, null, noop);
    expect(summary(rows)).toEqual([
      ['switch', 'Restore before launch', 'Fetch the latest cloud save before a game starts', true],
      ['switch', 'Upload after exit', 'Send new saves when a game closes', false],
      ['switch', 'Skip when the local save is newer', '', true],
      ['slider', 'Upload delay', '', '10 s'],
      ['slider', 'Saves to keep per game', '', 'Unlimited'],
    ]);
    const d = byKey<SliderRow>(rows, 'delay');
    const r = byKey<SliderRow>(rows, 'retain');
    expect([d.min, d.max, d.step, r.min, r.max, r.step]).toEqual([0, 60, 5, 0, 50, 1]);
    expect(delayText(0)).toBe('0 s');
    expect(retainText(7)).toBe('7');
  });

  it('saves the whole settings on a change', () => {
    const save = vi.fn();
    byKey<SwitchRow>(cloudRows(s, null, save), 'upload').set(true);
    expect(save).toHaveBeenCalledWith({ ...s, upload_on_exit: true });
  });

  it('shows nothing while loading and the reason when the read fails', () => {
    expect(cloudRows(null, null, noop)).toEqual([]);
    expect(summary(cloudRows(null, 'boom', noop))).toEqual([['info', "Couldn't read the cloud save settings", '', 'boom']]);
  });
});

describe('Connection pane (items 243–244)', () => {
  const acts = { reconnect: vi.fn(), connectToServer: vi.fn(), disconnect: vi.fn(), editConnection: vi.fn(), openServerPage: vi.fn(), openConfigFolder: vi.fn() };

  it('shows the server, user and credential line, then Reconnect, Open RomM in browser, Edit connection…, Disconnect and Open settings folder (ruling E1–3)', () => {
    const rows = connectionRows({ connected: true, offline: false, serverUrl: 'http://romm.local', username: 'sam' }, acts);
    expect(summary(rows)).toEqual([
      ['info', 'Server', '', 'http://romm.local'],
      ['info', 'User', '', 'sam'],
      ['info', 'Sign-in', '', 'Saved · verified'],
      ['action', 'Reconnect', '', ''],
      ['action', 'Open RomM in browser', "Opens your server's site in desktop mode", ''],
      ['action', 'Edit connection…', 'Change server or sign in again', ''],
      ['action', 'Disconnect', 'Sign out of this server', ''],
      ['action', 'Open settings folder', 'Switches to desktop mode', ''],
    ]);
    (byKey(rows, 'edit') as { run: () => void }).run();
    expect(acts.editConnection).toHaveBeenCalled();
    (byKey(rows, 'server-page') as { run: () => void }).run();
    expect(acts.openServerPage).toHaveBeenCalled();
    (byKey(rows, 'config-folder') as { run: () => void }).run();
    expect(acts.openConfigFolder).toHaveBeenCalled();
    expect(byKey(rows, 'disconnect').danger).toBe(true);
    (byKey(rows, 'disconnect') as { run: () => void }).run();
    expect(acts.disconnect).toHaveBeenCalledWith(disconnectWords('http://romm.local'));
    expect(disconnectWords('http://romm.local')).toEqual({
      title: 'Disconnect from http://romm.local?',
      body: "You'll need to sign in again. Installed games, saves and settings stay.",
      ok: 'Disconnect',
    });
  });

  it('says not verified while offline, and offers only Connect to a server… without one', () => {
    expect(credentialLine({ connected: false, offline: false })).toBe('Saved · not verified');
    const rows = connectionRows({ connected: false, offline: true, serverUrl: '', username: '' }, acts);
    expect(summary(rows)).toEqual([
      ['info', 'Server', '', 'Not set'],
      ['info', 'User', '', 'Not set'],
      ['info', 'Sign-in', '', 'Not used offline'],
      ['action', 'Connect to a server…', '', ''],
      ['action', 'Open settings folder', 'Switches to desktop mode', ''],
    ]);
  });
});

describe('Library pane (items 245–249)', () => {
  it('shows the folder and Change library folder…, disabled while a move runs', () => {
    expect(summary(libraryRows({ path: '/games/GRID', move: null }, noop))).toEqual([
      ['info', 'Library folder', '', '/games/GRID'],
      ['action', 'Change library folder…', 'Move your games, emulators and saves, or start fresh', ''],
    ]);
    const moving = libraryRows({ path: '/games/GRID', move: { phase: 'copy', running: true, old_root: '/games/GRID', new_root: '/mnt/GRID', resumed: false, error: null } }, noop);
    expect(moving[1]).toMatchObject({ disabled: true, why: MOVE_BLOCKED, sub: 'Moving to /mnt/GRID…' });
  });

  it('opens the browser beside the current library', () => {
    expect(browserStart('/home/sam/Games/GRID')).toBe('/home/sam/Games');
    expect(browserStart('/home/sam/Games/GRID/')).toBe('/home/sam/Games');
    expect(browserStart(null)).toBe('');
  });

  it('refuses the current library or a folder inside it with the prototype words (item 246)', () => {
    expect(refusalNotice({ status: 'refused', reason: 'inside_current', message: 'x' })).toEqual({
      title: "Can't use that folder",
      sub: 'It is the current library or inside it.',
    });
    expect(refusalNotice({ status: 'refused', reason: 'same_as_current', message: 'x' })?.sub).toBe('It is the current library or inside it.');
    expect(refusalNotice({ status: 'refused', reason: 'not_writable', message: 'GRID cannot write there.' })?.sub).toBe('GRID cannot write there.');
    expect(refusalNotice({ status: 'ok', path: '/x' })).toBeNull();
  });

  it('words the move list, Start fresh and Delete (items 247–249)', () => {
    const GB = 1024 ** 3;
    const preview: MovePreview = {
      old_root: '/a',
      new_root: '/b',
      items: [
        { name: 'games', strategy: 'copy', bytes: 84 * GB, files: 3 },
        { name: 'saves', strategy: 'rename', bytes: GB, files: 1 },
      ],
      total_bytes: 85 * GB,
      copy_bytes: 84 * GB,
      required_bytes: 90 * GB,
      free_bytes: 1200 * GB,
    };
    expect(moveListTitle('/mnt/storage')).toBe('Move to /mnt/storage?');
    expect(moveListSub(preview, null)).toBe('games/ · 84.0 GB · copied and checked · saves/ · 1.0 GB · same drive. Copies 84.0 GB; 1.2 TB free.');
    expect(moveListSub(null, 'The library layout is out of date.')).toBe('The library layout is out of date.');
    const fresh: StartFreshPreview = { old_root: '/a', games: [], total_bytes: 84 * GB } as unknown as StartFreshPreview;
    expect(deleteLabel(fresh)).toBe('Delete them (84.0 GB)');
    expect(deleteWords(fresh, '/home/sam/GRID')).toEqual({
      title: 'Delete 84.0 GB of game files?',
      body: "games/ in /home/sam/GRID is removed. Emulators and saves stay. This can't be undone.",
      ok: 'Delete',
    });
    expect(moveStartedNotice('/mnt/storage')).toEqual({ title: 'Moving the library to /mnt/storage…', sub: "Games can't install or start until it's done" });
  });

  it('says where the library is now after Start fresh, or why it did not change', () => {
    expect(freshNotice({ status: 'switched', library_path: '/b', rows_removed: 2, failures: [], skipped: [] }, false)).toEqual({
      title: 'Library is now /b',
      sub: '2 games left the library; their files stay on disk.',
      error: false,
    });
    expect(freshNotice({ status: 'switched', library_path: '/b', rows_removed: 0, failures: [], skipped: [] }, true).sub).toBe('Old game files deleted');
    expect(freshNotice({ status: 'stale', message: 'The library changed.' }, false)).toEqual({
      title: "Couldn't change the library folder",
      sub: 'The library changed.',
      error: true,
    });
  });
});

describe('RetroAchievements pane (items 250–251)', () => {
  const acts = { editUsername: vi.fn(), editToken: vi.fn(), save: vi.fn(), editPassword: vi.fn(), login: vi.fn(), clear: vi.fn() };
  const base = { username: 'sam', tokenTyped: false, tokenPresent: false, canSave: false, passwordTyped: false, canLogin: false };

  it('has Username, Password, Log In, a write-only Token, Save and Clear (533; order by the user 2026-11-10)', () => {
    const rows = raRows(base, acts);
    expect(summary(rows)).toEqual([
      ['text', 'Username', '', 'sam'],
      ['text', 'Password', 'Used once to log in. Never saved.', ''],
      ['action', 'Log In', 'Sign in with your password', ''],
      ['text', 'Token', 'Hidden once saved', ''],
      ['action', 'Save', 'Sets the login in every emulator that supports it', ''],
      ['action', 'Clear', '', ''],
    ]);
    expect(byKey(rows, 'ra-token')).toMatchObject({ mask: true });
    expect(byKey(rows, 'ra-password')).toMatchObject({ mask: true });
    expect(byKey(rows, 'ra-save')).toMatchObject({ disabled: true, why: SAVE_NEEDS_TOKEN });
    expect(byKey(rows, 'ra-login')).toMatchObject({ disabled: true, why: LOGIN_NEEDS_PASSWORD });
    expect(byKey(rows, 'ra-clear').danger).toBe(true);
  });

  it('names the missing field on Save and Log In (533)', () => {
    expect(saveWhy('', false)).toBe(SAVE_NEEDS);
    expect(SAVE_NEEDS).toBe('Enter a username and a token first.');
    expect(saveWhy('sam', false)).toBe('Enter a token first.');
    expect(saveWhy('', true)).toBe('Enter a username first.');
    expect(SAVE_NEEDS_USER).toBe('Enter a username first.');
    expect(saveWhy('sam', true)).toBeNull();
    expect(loginWhy('', false)).toBe(LOGIN_NEEDS);
    expect(LOGIN_NEEDS).toBe('Enter a username and a password first.');
    expect(loginWhy('sam', false)).toBe('Enter a password first.');
    expect(loginWhy(' ', true)).toBe(LOGIN_NEEDS_USER);
    expect(loginWhy('sam', true)).toBeNull();
    const rows = raRows({ ...base, username: '', passwordTyped: true, canLogin: false }, acts);
    expect(byKey(rows, 'ra-login')).toMatchObject({ disabled: true, why: 'Enter a username first.' });
  });

  it('shows a typed password only as a stand-in and enables Log In', () => {
    const rows = raRows({ ...base, passwordTyped: true, canLogin: true }, acts);
    expect(byKey<Row & { value: string }>(rows, 'ra-password').value).toBe('•');
    expect(byKey(rows, 'ra-login').disabled).toBeUndefined();
  });

  it('shows a saved or typed token only as a stand-in, never its value', () => {
    const rows = raRows({ ...base, tokenTyped: true, canSave: true }, acts);
    const token = byKey<Row & { value: string }>(rows, 'ra-token');
    expect(token.value).toBe('•');
    expect(byKey(rows, 'ra-save').disabled).toBeUndefined();
  });
});

describe('TV mode pane (items 261–267)', () => {
  const acts = { open: vi.fn(), setPrompts: vi.fn(), setAssignment: vi.fn(), toDesktop: vi.fn(), quit: vi.fn() };

  it('names the controller in Button assignments and Controller', () => {
    const rows = tvRows({ prompts: 'match', padName: 'Xbox Wireless Controller', assignment: 'xbox' }, acts);
    expect(summary(rows)).toEqual([
      ['choice', 'Button prompts', 'The button icons on screen', 'match'],
      ['choice', 'Button assignments', 'For Xbox Wireless Controller', 'xbox'],
      ['info', 'Controller', '', 'Xbox Wireless Controller'],
      ['action', 'Switch to desktop mode', 'Come back with the TV button in the top bar', ''],
      ['action', 'Quit GRID', '', ''],
    ]);
    expect(byKey<ChoiceRow>(rows, 'prompts').options.map((o) => o[1])).toEqual([
      'Match my controller',
      'Xbox',
      'PlayStation',
      'Nintendo',
      '8BitDo (Pro series)',
    ]);
  });

  it('opens the assignments list subtitled with the controller and each style described (item 263)', () => {
    const rows = tvRows({ prompts: 'xbox', padName: 'Pro Controller', assignment: 'nintendo' }, acts);
    byKey<ChoiceRow>(rows, 'assign').choose();
    const spec = acts.open.mock.calls.at(-1)![0] as ChoiceSpec;
    expect(spec.sub).toBe('Pro Controller');
    expect(spec.value).toBe('nintendo');
    expect(spec.options.map((o) => o[1])).toEqual(['Xbox style', 'Nintendo style']);
    expect(spec.subs).toEqual({ xbox: 'Bottom selects, right goes back', nintendo: 'Right selects, bottom goes back' });
    spec.pick('xbox');
    expect(acts.setAssignment).toHaveBeenCalledWith('xbox');
  });

  it('reads "For the controller in use" and "None found" with no controller', () => {
    expect(assignLine(null)).toBe('For the controller in use');
    const rows = tvRows({ prompts: 'match', padName: null, assignment: 'xbox' }, acts);
    expect(byKey<Row & { value: string }>(rows, 'tv-controller').value).toBe(NO_CONTROLLER);
    expect(NO_CONTROLLER).toBe('None connected');
  });

  it('disables Button assignments with no controller: "For the controller in use · Xbox style" (532)', () => {
    const rows = tvRows({ prompts: 'match', padName: null, assignment: 'nintendo' }, acts);
    const assign = byKey<ChoiceRow>(rows, 'assign');
    expect(assign).toMatchObject({ disabled: true, why: ASSIGN_NEEDS_PAD, sub: 'For the controller in use', value: 'xbox' });
    expect(ASSIGN_NEEDS_PAD).toBe('Connect a controller first.');
    const withPad = byKey<ChoiceRow>(tvRows({ prompts: 'match', padName: 'Pro Controller', assignment: 'nintendo' }, acts), 'assign');
    expect(withPad.disabled).toBeUndefined();
    expect(withPad.value).toBe('nintendo');
  });
});

describe('Updates pane (items 268–269)', () => {
  const now = new Date(2026, 10, 10, 12, 0);

  it('shows the version, the last check, the release row and Debug logging', () => {
    const rows = updatesRows(
      { version: '0.9.12', checkedAt: new Date(2026, 10, 10, 9, 14).toISOString(), now, release: { tag: 'v0.9.13', url: 'https://x' }, debug: true },
      { openRelease: noop, setDebug: noop },
    );
    expect(summary(rows)).toEqual([
      ['info', 'Version', '', 'GRID 0.9.12'],
      ['info', 'Last checked', '', 'Today at 09:14'],
      ['action', 'GRID 0.9.13 is out', 'Update with the installer you used', ''],
      ['switch', 'Debug logging', '', true],
    ]);
    const none = updatesRows({ version: '', checkedAt: null, now, release: null, debug: null }, { openRelease: noop, setDebug: noop });
    expect(summary(none)).toEqual([
      ['info', 'Version', '', 'GRID (version unknown)'],
      ['info', 'Last checked', '', 'Not checked yet'],
      ['switch', 'Debug logging', '', false],
    ]);
  });

  it('words the last check by day', () => {
    expect(lastCheckedText(new Date(2026, 10, 9, 18, 2).toISOString(), now)).toBe('Yesterday at 18:02');
    expect(lastCheckedText(new Date(2026, 10, 3, 9, 5).toISOString(), now)).toBe('3 Nov at 09:05');
    expect(lastCheckedText(new Date(2025, 0, 3, 9, 5).toISOString(), now)).toBe('3 Jan 2025 at 09:05');
    expect(versionText('v1.0.0')).toBe('GRID 1.0.0');
  });
});
