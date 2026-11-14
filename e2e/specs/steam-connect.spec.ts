import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { APP_START_TIMEOUT } from '../helpers/env.js';
import { clearStoreEvents, filesUnder, recordStoreEvents, storeEvents, type StoreNotice } from '../helpers/gogLogin.js';
import {
  FAKE_ACCOUNT_NAME,
  GAMES,
  GUEST,
  invoke,
  keyringHoldsFixtureKey,
  keyringSteamSlotEmpty,
  MAIN_WINDOW,
  NO_SUCH,
  SAM,
  STEAM_KEY,
  STEAM_REVOKED_KEY,
  steamControl,
  steamLibrary2,
  steamRequests,
  steamRoot,
  steamSave,
  steamStatus,
  steamTest,
  waitSteam,
} from '../helpers/steamSetup.js';

const testId = (id: string) => `[data-testid="${id}"]`;

type StoreGame = { key: string; title: string; installed: boolean; origin: string | null };
type Library = { installed: StoreGame[]; available: StoreGame[] };

async function steamNotices(): Promise<StoreNotice[]> {
  return (await storeEvents<StoreNotice>('store-notice')).filter((n) => n.source === 'steam');
}

const library = (): Promise<Library> => invoke<Library>('store_library', { source: 'steam' });

/** Every file under the fake Steam folder, with a hash of its bytes. */
function steamFiles(): Record<string, string> {
  const base = path.dirname(steamRoot());
  const out: Record<string, string> = {};
  for (const file of filesUnder(base)) {
    out[path.relative(base, file)] = createHash('sha256').update(readFileSync(file)).digest('hex');
  }
  return out;
}

/** The registry rows of Steam games (`list_installed`, every row). */
async function steamRows(): Promise<unknown[]> {
  const rows = await invoke<{ game: { source: string; store_id: string } }[]>('list_installed');
  return rows.filter((row) => JSON.stringify(row).includes('"steam"'));
}

/**
 * Stage `steam`, the Settings › Stores › Steam flow
 * (docs/stores-phase4-plan.md package C; GameStores "Settings card"),
 * driven through `invoke` on a profile used without a server: the Steam
 * client check, the local accounts (persona only), Test against every
 * answer of the mock Steam Web API (mock-romm/mock-steam.mjs), Connect,
 * Remove key, Save, Disconnect. The fake root (seed/steam-seed.mjs) has
 * three games and three tools installed; the mock owns four games, a piece
 * of software and a tool for SamPlays.
 *
 * Needs package A: the client and the accounts (`find_client`,
 * `local_accounts`), the Web API (`owned`, `persona`, `app_info`) and the
 * scan. With Step 0's stubs only the local SteamID64 check, the no-network
 * answer and the refusals pass.
 */
describe('steam: connect in Settings', () => {
  let filesBefore: Record<string, string> = {};

  before(async () => {
    await $(testId('shell-topbar')).waitForExist({ timeout: APP_START_TIMEOUT, timeoutMsg: 'the desktop shell never appeared' });
    await browser.switchToWindow(MAIN_WINDOW);
    await recordStoreEvents();
    filesBefore = steamFiles();
  });

  afterEach(async () => {
    await steamControl('up');
    await clearStoreEvents();
  });

  it('starts not connected and outside the rail, with no tool and no key', async () => {
    const status = await steamStatus();
    expect(status.connection).toBe('not_connected');
    expect(status.in_rail).toBe(false);
    expect(status.tool.name).toBe('');
    expect(status.steam.key).toBe('none');
    expect(status.steam.key_tail).toBe('');
    expect(status.steam.steam_id).toBe('');
    expect(keyringSteamSlotEmpty()).not.toBe(false);
  });

  it('finds the Steam client and the two accounts on this computer, by persona only', async () => {
    const status = await steamStatus();
    expect(status.steam.client_path).toBe(process.env.E2E_FAKE_STEAM ?? '');
    expect(status.steam.accounts).toEqual([
      { persona: 'SamPlays', steam_id: SAM, most_recent: true },
      { persona: 'couchguest', steam_id: GUEST, most_recent: false },
    ]);
    expect(JSON.stringify(await invoke('stores_status'))).not.toContain(FAKE_ACCOUNT_NAME);
  });

  it('a SteamID64 that is not one is refused locally, with no request', async () => {
    const before = (await steamRequests()).length;
    for (const id of ['123', '86561198000000001', 'https://steamcommunity.com/id/sam', '']) {
      expect(await steamTest(id, STEAM_KEY)).toEqual({ kind: 'bad_steam_id' });
    }
    expect((await steamRequests()).length).toBe(before);
  });

  it('Test with no key and none saved asks for one', async () => {
    await expect(steamTest(SAM, null)).rejects.toThrow(/Paste your key first/);
  });

  it('Test with no network answers network', async () => {
    await steamControl('fail');
    expect(await steamTest(SAM, STEAM_KEY)).toEqual({ kind: 'network' });
  });

  it('Test with a revoked key answers rejected', async () => {
    expect(await steamTest(SAM, STEAM_REVOKED_KEY)).toEqual({ kind: 'bad_key', status: 401 });
  });

  it('Test with private game details answers private', async () => {
    expect(await steamTest(GUEST, STEAM_KEY)).toEqual({ kind: 'private' });
  });

  it('Test with the good key counts the games only, and saves nothing', async () => {
    expect(await steamTest(SAM, STEAM_KEY)).toEqual({ kind: 'ok', owned_count: 4 });
    const status = await steamStatus();
    expect(status.connection).toBe('not_connected');
    expect(keyringSteamSlotEmpty()).not.toBe(false);
  });

  it('a redirect from Steam is no network, and is never followed', async () => {
    await steamControl('redirect');
    expect(await steamTest(SAM, STEAM_KEY)).toEqual({ kind: 'network' });
    await steamControl('no-redirect');
    expect((await steamRequests()).filter((r) => r.path === '/redirected')).toEqual([]);
  });

  it('a manual SteamID64 Steam has no account for is not a SteamID64', async () => {
    expect(await steamSave(NO_SUCH, STEAM_KEY)).toEqual({ kind: 'bad_steam_id' });
    expect((await steamStatus()).connection).toBe('not_connected');
  });

  it('Connect Steam with the good key: connected, 4 games owned, the rail group', async () => {
    expect(await steamSave(SAM, STEAM_KEY)).toEqual({ kind: 'ok', owned_count: 4 });
    const status = await waitSteam((s) => s.installed_count === 3, 'the installed games never showed');
    expect(status.connection).toBe('connected');
    expect(status.username).toBe('SamPlays');
    expect(status.in_rail).toBe(true);
    expect(status.owned).toBe('ready');
    expect(status.owned_count).toBe(4);
    expect(status.steam.key).toBe('ok');
    expect(status.steam.steam_id).toBe(SAM);
    expect(status.steam.key_tail).toBe('');
    expect((await steamNotices()).map((n) => n.message)).toContain('Steam connected · 4 games owned');
    const lib = await library();
    expect(lib.available.map((g) => g.title).sort()).toEqual(['Harbor of Bells', 'Quiet Comet', 'Rime Assembly', 'Signal Lost']);
    expect(lib.installed.map((g) => g.title).sort()).toEqual(['Deep Field Radio', 'Moonlit Ferry', 'Old Brick Road']);
    expect(lib.installed.every((g) => g.origin === 'steam_outside')).toBe(true);
    expect(JSON.stringify(lib)).not.toMatch(/Proton|Runtime|Redistributables|Desk Lights/);
    // The key is in the keyring, and nowhere in the config.
    const held = keyringHoldsFixtureKey();
    if (held !== null) expect(held).toBe(true);
  });

  it('Remove key leaves Steam connected with installed games only', async () => {
    await invoke('steam_remove_key');
    const status = await steamStatus();
    expect(status.connection).toBe('connected');
    expect(status.steam.key).toBe('none');
    expect(status.owned_count).toBe(0);
    const lib = await library();
    expect(lib.available).toEqual([]);
    expect(lib.installed.length).toBe(3);
    expect(keyringSteamSlotEmpty()).not.toBe(false);
    await expect(steamTest(SAM, null)).rejects.toThrow(/Paste your key first/);
  });

  it('Save a new key brings Available back; a Save of only the SteamID64 keeps the key', async () => {
    expect(await steamSave(SAM, STEAM_KEY)).toEqual({ kind: 'ok', owned_count: 4 });
    expect((await steamStatus()).steam.key).toBe('ok');
    expect(await steamSave(GUEST, null)).toEqual({ kind: 'private' });
    let status = await steamStatus();
    expect(status.steam.steam_id).toBe(GUEST);
    expect(status.username).toBe('couchguest');
    expect(status.steam.key).toBe('private');
    expect((await library()).available).toEqual([]);
    const notices = await steamNotices();
    expect(notices.map((n) => n.message)).toContain('Steam connected, but your game details are private, so Available is empty');
    expect(await steamSave(SAM, null)).toEqual({ kind: 'ok', owned_count: 4 });
    status = await steamStatus();
    expect(status.steam.key).toBe('ok');
    expect(status.username).toBe('SamPlays');
  });

  it('Refresh with no network keeps the list; back online it announces the list', async () => {
    await steamControl('fail');
    await invoke('store_refresh', { source: 'steam' });
    let status = await waitSteam(
      (s) => s.owned !== 'loading' && s.owned_error.includes('api.steampowered.com'),
      'the refresh never failed',
    );
    expect(status.steam.key).toBe('ok');
    expect(status.owned).toBe('ready');
    expect((await library()).available.length).toBe(4);
    await steamControl('up');
    await clearStoreEvents();
    await invoke('store_refresh', { source: 'steam' });
    await browser.waitUntil(async () => (await steamNotices()).some((n) => n.message === 'Steam: 4 games found · 4 to install'), {
      timeout: 20_000,
      timeoutMsg: 'the refresh card never came',
    });
    status = await steamStatus();
    expect(status.steam.key).toBe('ok');
    expect(status.owned_error).toBe('');
  });

  it('Disconnect: the group goes, no Steam row is left, the slot is empty, Steam’s files are untouched', async () => {
    await invoke('store_disconnect', { source: 'steam' });
    const status = await steamStatus();
    expect(status.connection).toBe('not_connected');
    expect(status.in_rail).toBe(false);
    expect(status.steam.key).toBe('none');
    expect(status.steam.steam_id).toBe('');
    expect(status.installed_count).toBe(0);
    expect(await steamRows()).toEqual([]);
    expect(keyringSteamSlotEmpty()).not.toBe(false);
    await expect(steamTest(SAM, null)).rejects.toThrow(/Paste your key first/);
    expect(steamFiles()).toEqual(filesBefore);
    // The installed games are still in Steam.
    expect(Object.keys(filesBefore).some((f) => f.endsWith(`appmanifest_${GAMES.moonlitFerry}.acf`))).toBe(true);
    expect(steamLibrary2()).toContain('library2');
  });
});
