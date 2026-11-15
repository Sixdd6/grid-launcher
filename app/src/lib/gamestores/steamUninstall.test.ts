// Uninstall in Steam (user decision 5, 2026-11-15; GameStores and TVStores
// READMEs, "Steam uninstall"): GRID opens the game's page in Steam's Library
// with no confirm of its own, the card says where Steam's Uninstall is, and
// while GRID watches (up to 10 minutes) Details shows the "Uninstall it in
// Steam" box and Show in Steam. The desktop's and TV's words and states.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { storeButtons } from './details';
import { steamUninstallNotice } from './notices';
import {
  GEAR_STEPS,
  SHOW_IN_STEAM,
  steamUninstalling,
  steamUninstallText,
  steamUninstallTv,
  UNINSTALL_IN_STEAM,
  UNINSTALL_IT_IN_STEAM,
  uninstallBoxNote,
} from './steam';
import { status, steamGame, steamRow, steamStatus } from './fixtures.test-util';
import { FakeStores } from './fakeApi';
import { tvStoreButtons, tvUninstallBox } from '../tv/stores/storeWords';

const FERRY = steamGame('Moonlit Ferry');
const watching = () => steamStatus({}, { uninstalling: [FERRY.store_id] });
const ctx = (st = steamStatus(), over: { job?: ReturnType<typeof steamRow> | null; game?: typeof FERRY } = {}) => ({
  game: over.game ?? FERRY,
  status: st,
  job: over.job ?? null,
  playing: false,
  confirm: null,
});

describe('the words', () => {
  it('the cards say where Steam’s Uninstall is, in Steam’s menu words', () => {
    expect(steamUninstallText('Moonlit Ferry')).toBe('Moonlit Ferry is open in Steam’s Library. To uninstall it, select the gear icon, then Manage › Uninstall.');
    expect(steamUninstallNotice('Moonlit Ferry')).toEqual({ text: steamUninstallText('Moonlit Ferry'), error: false, action: null });
    expect(steamUninstallTv('Moonlit Ferry')).toBe('Moonlit Ferry is open in Steam');
    expect(GEAR_STEPS).toBe('Select the gear icon, then Manage › Uninstall.');
  });

  it('the box while GRID watches', () => {
    expect(UNINSTALL_IT_IN_STEAM).toBe('Uninstall it in Steam');
    expect(uninstallBoxNote('Moonlit Ferry')).toBe(
      'Steam opened Moonlit Ferry in its Library. Select the gear icon, then Manage › Uninstall. GRID removes it here when Steam is done.',
    );
    expect(SHOW_IN_STEAM).toBe('Show in Steam');
    expect(UNINSTALL_IN_STEAM).toBe('Uninstall in Steam…');
  });
});

describe('which games GRID watches', () => {
  it('reads the status’s watched store ids', () => {
    expect(steamUninstalling(watching(), FERRY.store_id)).toBe(true);
    expect(steamUninstalling(watching(), '9100002')).toBe(false);
    expect(steamUninstalling(steamStatus(), FERRY.store_id)).toBe(false);
    expect(steamUninstalling(null, FERRY.store_id)).toBe(false);
    expect(steamUninstalling(status(), FERRY.store_id)).toBe(false);
  });
});

describe('desktop Details', () => {
  it('Uninstall in Steam… is one secondary click, not destructive, with no box', () => {
    const b = storeButtons(ctx()).find((x) => x.act === 'uninstall')!;
    expect(b).toEqual({ label: 'Uninstall in Steam…', act: 'uninstall', primary: false });
  });

  it('while watched: the box and Show in Steam replace it; Play and Verify in Steam stay', () => {
    const bs = storeButtons(ctx(watching()));
    expect(bs.map((b) => b.label)).toEqual(['Play', 'Verify in Steam', 'Show in Steam']);
    expect(bs[2]).toEqual({
      label: 'Show in Steam',
      act: 'uninstall',
      primary: false,
      box: {
        title: 'Uninstall it in Steam',
        note: 'Steam opened Moonlit Ferry in its Library. Select the gear icon, then Manage › Uninstall. GRID removes it here when Steam is done.',
      },
    });
    expect(bs.some((b) => b.destructive)).toBe(false);
  });

  it('once Steam removed it: Install in Steam, no box', () => {
    expect(storeButtons(ctx(watching(), { game: { ...FERRY, installed: false, origin: null } }))).toEqual([
      { label: 'Install in Steam', act: 'install', primary: true },
    ]);
  });
});

describe('TV game page', () => {
  const tv = (st = steamStatus(), game = FERRY) => tvStoreButtons({ game, status: st, job: null, fromDrive: false });

  it('Uninstall in Steam… is not red', () => {
    expect(tv().find((b) => b.act === 'uninstall')).toEqual({ act: 'uninstall', label: 'Uninstall in Steam…', primary: false });
  });

  it('while watched: Show in Steam with the same act (focus stays on it), and the left column’s box', () => {
    expect(tv(watching()).map((b) => [b.act, b.label])).toEqual([
      ['play', 'Play'],
      ['verify', 'Verify in Steam'],
      ['uninstall', 'Show in Steam'],
    ]);
    expect(tv(watching()).some((b) => b.danger)).toBe(false);
    expect(tvUninstallBox(FERRY, watching(), null)).toEqual({
      bold: 'Uninstall it in Steam.',
      rest: ' Steam opened Moonlit Ferry in its Library. Select the gear icon, then Manage › Uninstall. GRID removes it here when Steam is done.',
    });
    expect(tvUninstallBox(FERRY, steamStatus(), null)).toBeNull();
  });

  it('once Steam removed it: Install in Steam, no box', () => {
    const gone = { ...FERRY, installed: false, origin: null };
    expect(tv(watching(), gone).map((b) => b.label)).toEqual(['Install in Steam']);
    expect(tvUninstallBox(gone, watching(), null)).toBeNull();
  });
});

describe('the fake backend', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('watches once, then "Steam" removes the game with the uninstalled card', () => {
    const cards: string[] = [];
    const fake = new FakeStores((event, payload) => {
      if (event === 'store-notice') cards.push((payload as { message: string }).message);
    });
    fake.handle('steam_open', { key: FERRY.key, action: 'uninstall' });
    expect(fake.statuses.steam.steam!.uninstalling).toEqual([FERRY.store_id]);
    // Show in Steam: no second watch.
    fake.handle('steam_open', { key: FERRY.key, action: 'uninstall' });
    expect(fake.statuses.steam.steam!.uninstalling).toEqual([FERRY.store_id]);
    vi.advanceTimersByTime(8000);
    expect(fake.statuses.steam.steam!.uninstalling).toEqual([]);
    expect(fake.games.find((g) => g.key === FERRY.key)!.installed).toBe(false);
    expect(cards.filter((c) => c === 'Moonlit Ferry uninstalled')).toHaveLength(1);
  });
});
