// Epic Games (Phase 2): the shared store words with Epic's capabilities — no
// offline installers, no drive backups, no Version or Language, legendary as
// the tool. The fake's Epic data is the prototype's (`stores.js` e1–e6).
import { describe, expect, it } from 'vitest';
import { storeEntryLine, storeStripLine } from './downloads';
import { noticeActionLabel, toolNeededNotice, uninstalledNotice } from './notices';
import { initialChoice, installDialogView, installRequest } from './install';
import { conflictSides, storeButtons, storeDetailChips, storeLines, storeTabs } from './details';
import { accountLine, disconnectConfirm, licenseLine, notConnectedLede, toolLine } from './settings';
import { storeChips, storeHeaderActions, storePanel } from './view';
import { coverPrimary, coverTags, showCoverCloud } from './cover';
import { storeRailGroups } from './rail';
import { FakeStores } from './fakeApi';
import { epicGame, epicStatus, row, status } from './fixtures.test-util';

const MB = 1024 ** 2;
const now = 10_000_000;

describe('Epic Games fake data', () => {
  it('has the prototype’s six games and SamPlays', () => {
    const fake = new FakeStores();
    expect(fake.gamesOf('epic').map((g) => g.title)).toEqual([
      'Starlight Courier',
      'Vector Bloom',
      'Glimmer Deep',
      'Iron Orchard',
      'Northern Static',
      'Rustbelt Rally',
    ]);
    expect(fake.statuses.epic.username).toBe('SamPlays');
    expect(epicGame('Starlight').origin).toBe('legendary');
    expect(epicGame('IronOrchard').not_supported).toBe('the EA app');
    expect(epicGame('NorthernStatic').not_supported).toBe('Ubisoft Connect');
  });
  it('keeps each store’s scenario apart', () => {
    const fake = new FakeStores();
    fake.scenario('expired', 'epic');
    expect(fake.statuses.epic.connection).toBe('expired');
    expect(fake.statuses.gog.connection).toBe('connected');
    fake.scenario('emptyInstalled', 'epic');
    expect(fake.library('epic').installed).toEqual([]);
    expect(fake.library('gog').installed.length).toBe(3);
    fake.scenario('normal', 'epic');
    expect(fake.library('epic').installed.map((g) => g.title)).toEqual(['Starlight Courier', 'Vector Bloom']);
    expect(fake.handle('stores_status', {})).toHaveLength(2);
  });
});

describe('Epic Games rail and view', () => {
  it('puts Epic Games before GOG in the rail', () => {
    expect(storeRailGroups([status(), epicStatus()]).map((g) => g.heading)).toEqual(['EPIC GAMES', 'GOG']);
    expect(storeRailGroups([status(), epicStatus({ connection: 'not_connected', in_rail: false, installed_count: 0 })]).map((g) => g.source)).toEqual(['gog']);
  });
  it('has Refresh only: no Install from installer…, Select or Backup all', () => {
    expect(storeHeaderActions(epicStatus(), 'installed', 2)).toEqual({ backupAll: 0, installer: false, refresh: false, select: false });
    expect(storeHeaderActions(epicStatus(), 'available', 2)).toEqual({ backupAll: 0, installer: false, refresh: true, select: false });
  });
  it('words the chips', () => {
    expect(storeChips(epicStatus()).map((c) => c.text)).toEqual(['Signed in as SamPlays', 'Cloud saves: on']);
    expect(storeChips(epicStatus({ tool: { state: 'not_installed' } as never })).map((c) => c.text)).toContain('legendary not installed');
    expect(storeChips(epicStatus({ connection: 'expired' }))[0].text).toBe('Login expired · Sign in again');
  });
  it('words the panels', () => {
    expect(storePanel(epicStatus(), 'installed', [])).toMatchObject({
      title: 'No Epic Games games installed yet',
      body: 'Install one from Available.',
      actions: [{ label: 'Open Available' }],
    });
    expect(storePanel(epicStatus(), 'available', [])?.title).toBe('Every Epic Games game you own is installed');
    expect(storePanel(epicStatus({ connection: 'not_connected' }), 'available', [])?.actions[0].label).toBe('Connect Epic Games…');
    const tool = storePanel(epicStatus({ tool: { state: 'not_installed' } as never }), 'available', []);
    expect(tool).toMatchObject({
      title: 'Epic Games needs legendary',
      body: 'GRID installs and updates Epic Games games with legendary (legendary, GPL-3.0). It’s 3 MB.',
      actions: [{ label: 'Install legendary (3 MB)' }],
    });
    expect(storePanel(epicStatus({ tool: { state: 'installing' } as never }), 'available', [])?.actions[0]).toMatchObject({ label: 'Installing legendary…', disabled: true });
    expect(storePanel(epicStatus({ tool: { state: 'failed', error: 'checksum' } as never }), 'available', [])?.title).toBe('Couldn’t install legendary');
    expect(storePanel(epicStatus({ owned: 'loading' }), 'available', [])).toMatchObject({ kind: 'loading', title: 'Loading your Epic Games games…' });
  });
  it('fades a game GRID can’t install and opens Details for it', () => {
    const ns = epicGame('NorthernStatic');
    expect(coverTags(ns, null).map((t) => t.text)).toEqual(['NOT SUPPORTED']);
    expect(coverPrimary(ns, epicStatus(), null)).toEqual({ label: 'Details', act: 'details' });
    expect(coverPrimary(epicGame('RustbeltRally'), epicStatus(), null)).toEqual({ label: 'Install', act: 'install' });
    expect(showCoverCloud(epicGame('Starlight'))).toBe(true);
  });
});

describe('Epic Games install dialog', () => {
  const fake = new FakeStores();
  const glimmer = epicGame('GlimmerDeep');
  it('offers no Version and no Language', () => {
    const o = fake.installOptions(glimmer);
    const c = initialChoice(o);
    const v = installDialogView(glimmer, 'Epic Games', o, c);
    expect(v.versions).toEqual([]);
    expect(v.showLanguage).toBe(false);
    expect(v.showCompat).toBe(true);
    expect(v.sub).toBe('Epic Games · 45.0 GB download');
    expect(c.dlcIds).toEqual(['GlimmerDeepTrench']);
    expect(installRequest(glimmer, c)).toMatchObject({ platform: 'windows', language: '', compat_tool: 'GE-Proton10-17' });
    // Even an answer with two builds and languages shows neither for Epic.
    const both = {
      ...o,
      versions: [...o.versions, { platform: 'linux' as const, recommended: false, download_bytes: 1, disk_bytes: 1 }],
      languages: ['English', 'Deutsch'],
    };
    expect(installDialogView(glimmer, 'Epic Games', both, c)).toMatchObject({ versions: [], showLanguage: false });
  });
});

describe('Epic Games Details', () => {
  const epic = epicStatus();
  const opts = { nowSeconds: now, lastPlayedAt: 0, compat: 'GE-Proton10-17', onDrives: [], syncing: false };
  it('lists buttons without Backup to drive, with Verify for legendary installs', () => {
    const b = storeButtons({ game: epicGame('Starlight', { update_version: '1.13' }), status: epic, job: null, playing: false, confirm: null });
    expect(b.map((x) => x.label)).toEqual(['Play', 'Update to 1.13', 'Game Settings', 'Verify and repair', 'Uninstall']);
    expect(b.find((x) => x.act === 'verify')?.disabled).toBe(false);
    expect(storeButtons({ game: epicGame('IronOrchard'), status: epic, job: null, playing: false, confirm: null })[0]).toMatchObject({
      label: 'Not supported',
      disabled: true,
    });
  });
  it('writes the lines: runs through, cloud, the backup refusal and Not supported', () => {
    const starlight = epicGame('Starlight', { compat_tool: 'GE-Proton10-17', last_sync: now - 4 * 86400 });
    expect(storeLines({ game: starlight, status: epic, job: null, playing: false, confirm: null }, { ...opts, lastPlayedAt: now - 4 * 86400 }).map((l) => l.text)).toEqual([
      'Last played 4 days ago',
      'Runs through GE-Proton10-17 · Windows version',
      'Cloud saves synced 4 days ago',
      'Epic Games games can’t be backed up: legendary installs only from Epic’s servers.',
    ]);
    expect(storeLines({ game: epicGame('VectorBloom'), status: epic, job: null, playing: false, confirm: null }, opts).map((l) => l.text)).toEqual([
      'Never played',
      'Runs through GE-Proton10-17 · Windows version',
      'This game has no cloud saves',
      'Epic Games games can’t be backed up: legendary installs only from Epic’s servers.',
    ]);
    expect(storeLines({ game: epicGame('IronOrchard'), status: epic, job: null, playing: false, confirm: null }, opts).map((l) => l.text)).toEqual([
      'Needs the EA app. GRID can’t install games that start through another company’s launcher; install it from the Epic Games Launcher instead.',
      'Windows only · runs through GE-Proton10-17',
      'Epic Games games can’t be backed up: legendary installs only from Epic’s servers.',
    ]);
    const expired = storeLines({ game: epicGame('GlimmerDeep'), status: epicStatus({ connection: 'expired' }), job: null, playing: false, confirm: null }, opts);
    expect(expired[0]).toMatchObject({ text: 'Your Epic Games login expired.', link: { label: 'Sign in again' } });
    expect(storeDetailChips(epicGame('IronOrchard'), false).map((c) => c.text)).toEqual(['Epic Games', 'Not supported']);
    expect(storeTabs(epicGame('Starlight'))).toEqual(['overview', 'media', 'cloud']);
  });
  it('words a conflict with no cloud file count', () => {
    const c = { local_at: now - 60, local_files: 3, local_bytes: 2.1 * MB, cloud_at: now - 7200, cloud_files: null, cloud_bytes: 2.0 * MB };
    const sides = conflictSides('epic', c, now);
    expect(sides[0].after).toBe('Uploads it to Epic Games.');
    expect(sides[1].label).toBe('Epic Games cloud');
    expect(sides[1].note).toMatch(/ on another computer · 2.0 MB$/);
  });
});

describe('Epic Games Settings card', () => {
  const epic = epicStatus();
  it('words the account, tool, license and Disconnect confirm', () => {
    expect(accountLine(epic)).toBe('Signed in as SamPlays');
    // Epic has no offline installers: a signed-out card doesn't count games installed here.
    expect(accountLine(epicStatus({ connection: 'not_connected', installed_count: 2 }))).toBe('Not signed in');
    expect(toolLine(epic)).toEqual({ kind: 'installed', text: 'legendary 0.21.1 · installed' });
    expect(toolLine(epicStatus({ connection: 'not_connected', tool: { state: 'not_installed' } as never })).text).toBe('Installs when you connect · 3 MB');
    expect(licenseLine(epic)).toEqual({
      text: 'legendary · GPL-3.0',
      url: 'https://github.com/legendary-gl/legendary',
      label: 'github.com/legendary-gl/legendary',
    });
    expect(disconnectConfirm(epicStatus({ installed_count: 2 }))).toEqual({
      bold: 'Disconnect Epic Games?',
      rest: 'GRID deletes your Epic Games login and legendary from this computer. The 2 installed games stay and still play; they can’t install, update or sync cloud saves until you connect again.',
    });
  });
  it('says nothing about offline installers when not connected', () => {
    expect(notConnectedLede(epicStatus({ connection: 'not_connected' }))).toBe(
      'Sign in to Epic Games to see the games you own in the Library and install them. GRID installs with legendary (legendary, GPL-3.0, 3 MB), which it downloads when you connect.',
    );
    expect(notConnectedLede(status({ connection: 'not_connected' }))).toContain('You can install GOG offline installers without signing in. ');
  });
});

describe('Epic Games cards and Downloads', () => {
  it('words the cards', () => {
    expect(uninstalledNotice('epic', 'Vector Bloom', true).text).toBe('Vector Bloom uninstalled. Its cloud saves stay in your Epic Games account.');
    expect(noticeActionLabel({ kind: 'install_tool' }, 'epic')).toBe('Install legendary');
    expect(toolNeededNotice('epic').text).toBe('Epic Games needs legendary to install games');
  });
  it('words the strip and the failed row', () => {
    const failed = row({
      source_id: 'epic:GlimmerDeep',
      title: 'Glimmer Deep',
      platform: 'Epic Games',
      status: 'failed',
      error: 'legendary stopped: Epic’s download server didn’t answer',
    });
    expect(storeEntryLine(failed)).toBe('Failed · legendary stopped: Epic’s download server didn’t answer');
    expect(storeStripLine(row({ source_id: 'epic:Starlight', title: 'Starlight Courier', platform: 'Epic Games' }))).toBe(
      'Starlight Courier · Epic Games · Downloading 45% · 41.2 MB/s',
    );
    expect(storeStripLine(row({ kind: 'store_tool', source_id: 'epic', title: 'legendary', platform: 'Epic Games', phase: 'checksum' }))).toBe(
      'legendary · Epic Games · Downloading 45% · checking its SHA-256',
    );
  });
});
