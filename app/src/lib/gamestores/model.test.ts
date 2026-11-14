import { describe, expect, it } from 'vitest';
import { agoText, plural, sizeGb, dlcShortTitle } from './text';
import {
  canBackup,
  ENABLED_STORES,
  hasCloud,
  hasDlc,
  hasInstaller,
  hasTool,
  installedByTool,
  isEnabledStore,
  noBackupReason,
  readableVersions,
  shownVersion,
  storeCaps,
  storeLauncher,
  storeName,
  storeOfKey,
  storeOfPlatform,
  storesLede,
  storeTool,
  updateLabel,
} from './marks';
import { availBlock, parseStoreRailKey, storeFlag, storeRailGroups, storeRailKey, storeShown } from './rail';
import { coverPrimary, coverProgress, coverTags, showCoverCloud } from './cover';
import { jobPercent, liveJobFor, liveToolJob } from './jobs';
import { noMatchText, storeChips, storeCountLine, storeHeaderActions, storePanel } from './view';
import { selectedToWrite } from './library';
import { installedGame, row, status, storeGame } from './fixtures.test-util';

const GB = 1024 ** 3;

describe('text', () => {
  it('writes sizes the prototype’s way', () => {
    expect(sizeGb(412 * GB)).toBe('412 GB');
    expect(sizeGb(22 * GB)).toBe('22.0 GB');
    expect(sizeGb(0.118 * GB)).toBe('118 MB');
    expect(sizeGb(10)).toBe('<1 MB');
  });
  it('says how long ago', () => {
    expect(agoText(0, 100)).toBe('never');
    expect(agoText(1000, 1030)).toBe('just now');
    expect(agoText(1000, 1300)).toBe('5 min ago');
    expect(agoText(0 + 1, 2 * 3600 + 1)).toBe('2 hours ago');
    expect(agoText(1, 86400 + 1)).toBe('yesterday');
    expect(agoText(1, 3 * 86400 + 1)).toBe('3 days ago');
  });
  it('pluralises and shortens DLC titles', () => {
    expect(plural(1, 'game')).toBe('1 game');
    expect(plural(3, 'game')).toBe('3 games');
    expect(dlcShortTitle('Ashen Tide — Digital Artbook', 'Ashen Tide')).toBe('Digital Artbook');
  });
});

describe('marks', () => {
  it('shows all four stores in Phase 4', () => {
    expect(ENABLED_STORES).toEqual(['amazon', 'epic', 'gog', 'steam']);
    expect(isEnabledStore('epic')).toBe(true);
    expect(isEnabledStore('amazon')).toBe(true);
    expect(isEnabledStore('steam')).toBe(true);
    expect(storeName('gog')).toBe('GOG');
    expect(storeName('epic')).toBe('Epic Games');
    expect(storeName('amazon')).toBe('Amazon Games');
    expect(storeTool('gog')).toEqual({ name: 'gogdl', pkg: 'heroic-gogdl' });
    expect(storeTool('epic')).toEqual({ name: 'legendary', pkg: 'legendary' });
    expect(storeTool('amazon')).toEqual({ name: 'nile', pkg: 'nile' });
    expect(storeTool('steam')).toBeNull();
  });
  it('gives each store its capabilities', () => {
    expect(storeCaps('gog')).toEqual({
      hasTool: true, hasCloud: true, canBackup: true, hasInstaller: true, hasVersions: true, hasLanguages: true, hasDlc: true, readableVersions: true, managedByClient: false,
    });
    expect(storeCaps('epic')).toEqual({
      hasTool: true, hasCloud: true, canBackup: false, hasInstaller: false, hasVersions: false, hasLanguages: false, hasDlc: true, readableVersions: true, managedByClient: false,
    });
    // Amazon is the first store with a tool but no cloud saves, no DLC and opaque versions.
    expect(storeCaps('amazon')).toEqual({
      hasTool: true, hasCloud: false, canBackup: false, hasInstaller: false, hasVersions: false, hasLanguages: false, hasDlc: false, readableVersions: false, managedByClient: false,
    });
    // Steam's own client installs, updates and starts its games.
    expect(storeCaps('steam')).toEqual({
      hasTool: false, hasCloud: false, canBackup: false, hasInstaller: false, hasVersions: false, hasLanguages: false, hasDlc: false, readableVersions: false, managedByClient: true,
    });
    expect([hasTool('steam'), hasCloud('amazon'), canBackup('epic'), hasInstaller('epic')]).toEqual([false, false, false, false]);
    expect([hasTool('epic'), hasCloud('epic'), canBackup('gog'), hasInstaller('gog')]).toEqual([true, true, true, true]);
  });
  it('knows which installs the store tool made', () => {
    expect(installedByTool({ source: 'gog', origin: 'gogdl' })).toBe(true);
    expect(installedByTool({ source: 'gog', origin: 'offline' })).toBe(false);
    expect(installedByTool({ source: 'epic', origin: 'legendary' })).toBe(true);
    expect(installedByTool({ source: 'epic', origin: 'gogdl' })).toBe(false);
    expect(installedByTool({ source: 'amazon', origin: 'nile' })).toBe(true);
    expect(installedByTool({ source: 'steam', origin: null })).toBe(false);
  });
  it('names the launcher and reads store platforms', () => {
    expect(storeLauncher('epic')).toBe('Epic Games Launcher');
    expect(storeOfPlatform('Epic Games')).toBe('epic');
    expect(storeOfPlatform('Amazon Games')).toBe('amazon');
    expect(storeOfPlatform('GOG')).toBe('gog');
    expect(storeOfPlatform('epic')).toBe('epic');
    expect(storeOfPlatform('PlayStation')).toBeNull();
    expect(storeOfPlatform('')).toBeNull();
  });
  it('words the Settings › Stores lede from the stores shown', () => {
    const start = 'Connect the stores you buy games from. Each connected store gets a group in the Library with its Installed and Available games.';
    expect(storesLede(['gog'])).toBe(
      `${start} GRID installs GOG games with the open-source tool Heroic Games Launcher uses; its login stays in that tool’s files in GRID’s data folder.`,
    );
    expect(storesLede(['epic', 'gog'])).toBe(
      `${start} GRID installs GOG and Epic games with the open-source tools Heroic Games Launcher uses; their logins stay in those tools’ files in GRID’s data folder.`,
    );
    // Phase 3's line, without Steam.
    expect(storesLede(['amazon', 'epic', 'gog'])).toBe(
      `${start} GRID installs GOG, Epic and Amazon games with the open-source tools Heroic Games Launcher uses; their logins stay in those tools’ files in GRID’s data folder.`,
    );
    // Phase 4 shows every store: the prototype's full line.
    expect(storesLede()).toBe(storesLede(['amazon', 'epic', 'gog', 'steam']));
    // The prototype's full line, every store shown.
    expect(storesLede(['amazon', 'epic', 'gog', 'steam'])).toBe(
      `${start} GRID installs GOG, Epic and Amazon games with the open-source tools Heroic Games Launcher uses; their logins stay in those tools’ files in GRID’s data folder. Steam games install and start through the Steam client.`,
    );
  });
  it('says why Epic, Amazon and Steam games can’t be backed up', () => {
    expect(noBackupReason('gog')).toBe('');
    expect(noBackupReason('epic')).toContain('legendary installs only from Epic’s servers');
    expect(noBackupReason('amazon')).toBe('Amazon Games games can’t be backed up: nile installs only from Amazon’s servers.');
  });
  it('reads a store from a game key, splitting at the first colon only', () => {
    expect(storeOfKey('amazon:amzn1.adg.product.x')).toBe('amazon');
    expect(storeOfKey('epic:Starlight')).toBe('epic');
    expect(storeOfKey('gog:1001')).toBe('gog');
    expect(storeOfKey('nonsense')).toBe('gog');
  });
  it('shows readable versions only: Amazon’s update reads “Update”', () => {
    expect(updateLabel({ source: 'gog', update_version: '2.2' })).toBe('Update to 2.2');
    expect(updateLabel({ source: 'epic', update_version: '1.13' })).toBe('Update to 1.13');
    expect(updateLabel({ source: 'amazon', update_version: '9d2a6c31-fake' })).toBe('Update');
    expect(shownVersion({ source: 'gog', version: ' 2.1 ' })).toBe('2.1');
    expect(shownVersion({ source: 'amazon', version: '4b1f0e2c-fake' })).toBe('');
    expect([hasDlc('gog'), hasDlc('epic'), hasDlc('amazon'), readableVersions('amazon')]).toEqual([true, true, false, false]);
  });
});

describe('rail', () => {
  it('round-trips store rail keys', () => {
    expect(storeRailKey('gog', 'installed')).toBe('store:gog:installed');
    expect(parseStoreRailKey('store:gog:available')).toEqual({ source: 'gog', part: 'available' });
    expect(parseStoreRailKey('drive:/media/x')).toBeNull();
    expect(parseStoreRailKey('store:nope:installed')).toBeNull();
  });
  it('blocks Available for each reason in order', () => {
    expect(availBlock(status({ connection: 'not_connected' }))).toBe('off');
    expect(availBlock(status({ connection: 'expired' }))).toBe('expired');
    expect(availBlock(status({ tool: { state: 'failed' } as never }))).toBe('tool');
    expect(availBlock(status({ owned: 'loading' }))).toBe('loading');
    expect(availBlock(status({ owned: 'error' }))).toBe('error');
    expect(availBlock(status())).toBe('');
  });
  it('badges the heading', () => {
    expect(storeFlag(status())).toBeNull();
    expect(storeFlag(status({ connection: 'expired' }))?.text).toBe('!');
    expect(storeFlag(status({ connection: 'not_connected' }))?.text).toBe('off');
    expect(storeFlag(status({ tool: { state: 'not_installed' } as never }))?.text).toBe('!');
  });
  it('keeps a disconnected store while games are installed here', () => {
    expect(storeShown(status({ connection: 'not_connected', in_rail: false, installed_count: 2 }))).toBe(true);
    expect(storeShown(status({ connection: 'not_connected', in_rail: false, installed_count: 0 }))).toBe(false);
  });
  it('builds the GOG group with counts, a spinner while loading and ! when blocked', () => {
    const [g] = storeRailGroups([status()]);
    expect(g.heading).toBe('GOG');
    expect(g.rows.map((r) => r.label)).toEqual(['Installed', 'Available']);
    expect(g.rows[0].count).toEqual({ kind: 'number', n: 3 });
    expect(g.rows[1].count).toEqual({ kind: 'number', n: 5 });
    expect(storeRailGroups([status({ owned: 'loading' })])[0].rows[1].count).toEqual({ kind: 'loading' });
    expect(storeRailGroups([status({ connection: 'expired' })])[0].rows[1].count).toEqual({ kind: 'attention' });
  });
  it('keeps a status the backend sends for a store it has no group for out', () => {
    expect(storeRailGroups([status({ source: 'steam', connection: 'not_connected', in_rail: false })])).toEqual([]);
  });
});

describe('jobs and covers', () => {
  it('finds the live row of a game, active before queued', () => {
    const q = row({ id: 1, status: 'queued' });
    const a = row({ id: 2 });
    expect(liveJobFor([q, a], 'gog:1004')?.id).toBe(2);
    expect(liveJobFor([row({ status: 'completed' })], 'gog:1004')).toBeNull();
    expect(liveToolJob([row({ kind: 'store_tool', source_id: 'gog' })], 'gog')).not.toBeNull();
  });
  it('measures progress from the half that runs', () => {
    expect(jobPercent(row())).toBe(45);
    expect(jobPercent(row({ status: 'installing', install_processed_bytes: 9, install_total_bytes: 10 }))).toBe(90);
    expect(jobPercent(row({ status: 'queued' }))).toBe(0);
  });
  it('tags covers: %, QUEUED, UPDATE, SAVE CONFLICT, NOT SUPPORTED', () => {
    const g = storeGame();
    expect(coverTags(g, row())).toEqual([{ text: '45%', tone: 'queue' }]);
    expect(coverTags(g, row({ status: 'queued' }))[0].text).toBe('QUEUED');
    const up = installedGame({ update_version: '2.2', cloud: 'conflict' });
    expect(coverTags(up, null).map((t) => t.text)).toEqual(['UPDATE', 'SAVE CONFLICT']);
    expect(coverTags(storeGame({ not_supported: 'the EA app' }), null).map((t) => t.text)).toEqual(['NOT SUPPORTED']);
    expect(coverProgress(row({ status: 'queued' }))).toBe(0);
    expect(coverProgress(null)).toBeNull();
  });
  it('picks the hover action', () => {
    const s = status();
    expect(coverPrimary(storeGame(), s, row())).toEqual({ label: 'Installing…', act: 'details' });
    expect(coverPrimary(storeGame(), s, row({ status: 'queued' })).label).toBe('Queued');
    expect(coverPrimary(installedGame(), s, row({ kind: 'store_update' })).label).toBe('Updating…');
    expect(coverPrimary(installedGame(), s, row({ kind: 'store_repair' })).label).toBe('Verifying…');
    expect(coverPrimary(installedGame(), s, null)).toEqual({ label: 'Play', act: 'play' });
    expect(coverPrimary(storeGame(), s, null)).toEqual({ label: 'Install', act: 'install' });
    expect(coverPrimary(storeGame(), status({ connection: 'expired' }), null).act).toBe('details');
    expect(showCoverCloud(installedGame())).toBe(true);
    expect(showCoverCloud(storeGame())).toBe(false);
  });
});

describe('store view', () => {
  const now = 10_000;
  it('words the count line', () => {
    const inst = [installedGame({ disk_bytes: 21.6 * GB }), installedGame({ disk_bytes: 45.4 * GB })];
    expect(storeCountLine(status(), 'installed', inst, now)).toBe('2 games · 67.0 GB');
    expect(storeCountLine(status({ refreshed_at: now - 300 }), 'available', [storeGame()], now)).toBe(
      '1 game you own and haven’t installed · updated 5 min ago',
    );
    expect(storeCountLine(status({ owned: 'loading' }), 'available', [], now)).toBe('Games you own and haven’t installed · loading');
    expect(storeCountLine(status({ connection: 'expired' }), 'available', [], now)).toBe('Games you own and haven’t installed');
  });
  it('lists the chips', () => {
    expect(storeChips(status()).map((c) => c.text)).toEqual(['Signed in as sam_plays', 'Cloud saves: on']);
    expect(storeChips(status({ connection: 'expired' }))[0]).toMatchObject({ text: 'Login expired · Sign in again', act: 'sign_in', warn: true });
    expect(storeChips(status({ tool: { state: 'not_installed' } as never })).map((c) => c.text)).toContain('gogdl not installed');
  });
  it('offers Backup all N…, Install from installer… and Refresh', () => {
    expect(storeHeaderActions(status(), 'installed', 3)).toEqual({ backupAll: 3, installer: true, refresh: false, select: true });
    expect(storeHeaderActions(status(), 'available', 3)).toMatchObject({ backupAll: 0, refresh: true, select: false });
    expect(storeHeaderActions(status({ connection: 'expired' }), 'available', 3).refresh).toBe(false);
  });
  it('replaces the grid with each state panel', () => {
    expect(storePanel(status(), 'installed', [])?.title).toBe('No GOG games installed yet');
    expect(storePanel(status(), 'installed', [])?.actions.map((a) => a.label)).toEqual(['Open Available', 'Install from installer…']);
    expect(storePanel(status(), 'installed', [installedGame()])).toBeNull();
    expect(storePanel(status({ connection: 'not_connected' }), 'available', [])?.title).toBe('GOG isn’t connected');
    expect(storePanel(status({ connection: 'expired' }), 'available', [])).toMatchObject({ tone: 'warn', title: 'Your GOG login expired' });
    const tool = storePanel(status({ tool: { state: 'not_installed', size_bytes: 12_000_000 } as never }), 'available', []);
    expect(tool?.title).toBe('GOG needs gogdl');
    expect(tool?.actions[0].label).toBe('Install gogdl (12 MB)');
    expect(storePanel(status({ tool: { state: 'installing' } as never }), 'available', [])?.actions[0]).toMatchObject({ label: 'Installing gogdl…', disabled: true });
    const failed = storePanel(status({ tool: { state: 'failed', error: 'checksum mismatch' } as never }), 'available', []);
    expect(failed).toMatchObject({ tone: 'err', title: 'Couldn’t install gogdl' });
    expect(failed?.body).toBe('The download didn’t match its checksum, so GRID deleted it. Nothing else changed.');
    expect(storePanel(status({ owned: 'loading' }), 'available', [])?.kind).toBe('loading');
    expect(storePanel(status(), 'available', [])?.title).toBe('Every GOG game you own is installed');
    expect(noMatchText('gog', 'installed', ' ash ')).toBe('No installed GOG games match “ash”');
  });
});

describe('select bar', () => {
  it('sums the picked games as "N GB to write"', () => {
    const games = [installedGame({ key: 'gog:1', disk_bytes: 21.6 * 1024 ** 3 }), installedGame({ key: 'gog:2', disk_bytes: 2.9 * 1024 ** 3 }), installedGame({ key: 'gog:3', disk_bytes: 5 * 1024 ** 3 })];
    expect(selectedToWrite(games, ['gog:1', 'gog:2'])).toBe('24.5 GB to write');
  });
});
