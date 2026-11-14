// Amazon Games (Phase 3): the first store with a tool but no cloud saves, no
// DLC and opaque version ids. The shared store words with Amazon's
// capabilities — no Cloud saves chip, tab, line or Settings row, no DLC, no
// Version, "Update" with no version, nile as the tool. The fake's Amazon data
// is the prototype's (`stores.js` a1–a3).
import { describe, expect, it } from 'vitest';
import { storeEntryLine, storeStripLine } from './downloads';
import { noticeActionLabel, toolNeededNotice, uninstalledNotice } from './notices';
import { initialChoice, installDialogView, installRequest, optionsForStore } from './install';
import { dlcRows, storeButtons, storeDetailChips, storeFacts, storeLines, storeTabs } from './details';
import { accountLine, disconnectConfirm, licenseLine, notConnectedLede, toolLine } from './settings';
import { storeChips, storeHeaderActions, storePanel } from './view';
import { coverPrimary, coverTags, showCoverCloud } from './cover';
import { storeRailGroups } from './rail';
import { FAKE_AMAZON_UPDATE, FAKE_NILE_GLIBC_ERROR, FakeStores } from './fakeApi';
import { amazonGame, amazonStatus, epicStatus, row, status } from './fixtures.test-util';

const now = 10_000_000;

describe('Amazon Games fake data', () => {
  it('has the prototype’s three games and sam.plays, starting expired', () => {
    const fake = new FakeStores();
    expect(fake.gamesOf('amazon').map((g) => g.title)).toEqual(['Fogline', 'Kettle Drum', 'Lowsun']);
    expect(fake.statuses.amazon.username).toBe('sam.plays');
    // The prototype's own default for Amazon is a login that expired.
    expect(fake.statuses.amazon.connection).toBe('expired');
    fake.scenario('normal', 'amazon');
    expect(fake.statuses.amazon.connection).toBe('connected');
    expect(amazonGame('Fogline')).toMatchObject({ installed: true, origin: 'nile', installed_platform: 'windows', cloud: 'unknown', dlc: [] });
    expect(amazonGame('Fogline').key.startsWith('amazon:amzn1.adg.product.')).toBe(true);
    expect(fake.statuses.amazon.tool).toMatchObject({ name: 'nile', version: '1.2.0', size_bytes: 11113264, license: 'GPL-3.0' });
    expect(fake.statuses.amazon.cloud_saves).toBe(false);
  });
  it('lists Amazon first and has no cloud conflict to make', () => {
    const fake = new FakeStores();
    expect((fake.handle('stores_status', {}) as { source: string }[]).map((s) => s.source)).toEqual(['amazon', 'epic', 'gog']);
    fake.scenario('cloudConflict', 'amazon');
    expect(fake.gamesOf('amazon').every((g) => g.cloud !== 'conflict')).toBe(true);
    fake.scenario('update', 'amazon');
    expect(fake.gamesOf('amazon').find((g) => g.installed)?.update_version).toBe(FAKE_AMAZON_UPDATE);
  });
});

describe('Amazon Games rail and view', () => {
  it('puts Amazon Games first in the rail, with the expired badge', () => {
    expect(storeRailGroups([status(), epicStatus(), amazonStatus()]).map((g) => g.heading)).toEqual(['AMAZON GAMES', 'EPIC GAMES', 'GOG']);
    expect(storeRailGroups([amazonStatus({ connection: 'expired' })])[0].flag?.text).toBe('!');
    expect(storeRailGroups([amazonStatus({ connection: 'not_connected', in_rail: false, installed_count: 0 })])).toEqual([]);
  });
  it('has Refresh only and no Cloud saves chip', () => {
    expect(storeHeaderActions(amazonStatus(), 'installed', 1)).toEqual({ backupAll: 0, installer: false, refresh: false, select: false });
    expect(storeHeaderActions(amazonStatus(), 'available', 1)).toEqual({ backupAll: 0, installer: false, refresh: true, select: false });
    // Even a status that says cloud saves are on shows no chip: Amazon has none.
    expect(storeChips(amazonStatus({ cloud_saves: true })).map((c) => c.text)).toEqual(['Signed in as sam.plays']);
    expect(storeChips(amazonStatus({ tool: { state: 'not_installed' } as never })).map((c) => c.text)).toContain('nile not installed');
    expect(storeChips(amazonStatus({ connection: 'expired' })).map((c) => c.text)).toEqual(['Login expired · Sign in again']);
  });
  it('words the panels', () => {
    expect(storePanel(amazonStatus({ connection: 'expired' }), 'available', [])).toMatchObject({
      tone: 'warn',
      title: 'Your Amazon Games login expired',
      body: 'Sign in again to see the games you own and install them. Installed games still play.',
      actions: [{ label: 'Sign in again…' }],
    });
    expect(storePanel(amazonStatus(), 'installed', [])).toMatchObject({ title: 'No Amazon Games games installed yet', body: 'Install one from Available.' });
    expect(storePanel(amazonStatus({ connection: 'not_connected' }), 'available', [])?.actions[0].label).toBe('Connect Amazon Games…');
    expect(storePanel(amazonStatus({ tool: { state: 'not_installed' } as never }), 'available', [])).toMatchObject({
      title: 'Amazon Games needs nile',
      body: 'GRID installs and updates Amazon Games games with nile (nile, GPL-3.0). It’s 11 MB.',
      actions: [{ label: 'Install nile (11 MB)' }],
    });
    expect(storePanel(amazonStatus({ tool: { state: 'failed', error: 'checksum' } as never }), 'available', [])).toMatchObject({
      title: 'Couldn’t install nile',
      body: 'The download didn’t match its checksum, so GRID deleted it. Nothing else changed.',
    });
    // nile's glibc floor: the tool error shows as given.
    expect(storePanel(amazonStatus({ tool: { state: 'failed', error: FAKE_NILE_GLIBC_ERROR } as never }), 'available', [])?.body).toBe(
      'nile needs a newer system (glibc 2.35 or later)',
    );
    expect(storePanel(amazonStatus({ owned: 'loading' }), 'available', [])).toMatchObject({ kind: 'loading', title: 'Loading your Amazon Games games…' });
  });
  it('tags covers with UPDATE but never a save conflict or the cloud button', () => {
    const fog = amazonGame('Fogline', { update_version: FAKE_AMAZON_UPDATE });
    expect(coverTags(fog, null).map((t) => t.text)).toEqual(['UPDATE']);
    expect(coverTags(amazonGame('Fogline', { cloud: 'conflict' }), null)).toEqual([]);
    expect(showCoverCloud(fog)).toBe(false);
    expect(coverPrimary(amazonGame('Kettle Drum'), amazonStatus(), null)).toEqual({ label: 'Install', act: 'install' });
  });
});

describe('Amazon Games install dialog', () => {
  const fake = new FakeStores();
  const lowsun = amazonGame('Lowsun');
  it('offers no Version, no Language and no DLC', () => {
    // Even a backend answer with DLC shows none for Amazon.
    const raw = { ...fake.installOptions(lowsun), dlc: [{ id: 'x', title: 'Lowsun — Extra', download_bytes: 1, installed: false }] };
    const o = optionsForStore('amazon', raw);
    expect(o.dlc).toEqual([]);
    const c = initialChoice(o);
    expect(c.dlcIds).toEqual([]);
    const v = installDialogView(lowsun, 'Amazon Games', o, c);
    expect(v).toMatchObject({ versions: [], showLanguage: false, showCompat: true, sub: 'Amazon Games · 15.0 GB download', button: 'Install' });
    expect(installRequest(lowsun, c)).toMatchObject({ platform: 'windows', language: '', dlc_ids: [], compat_tool: 'GE-Proton10-17' });
    expect(optionsForStore('epic', raw).dlc).toHaveLength(1);
  });
});

describe('Amazon Games Details', () => {
  const amazon = amazonStatus();
  const opts = { nowSeconds: now, lastPlayedAt: 0, compat: 'GE-Proton10-17', onDrives: [], syncing: false };
  it('reads “Update” with no version', () => {
    const fog = amazonGame('Fogline', { update_version: FAKE_AMAZON_UPDATE });
    const b = storeButtons({ game: fog, status: amazon, job: null, playing: false, confirm: null });
    expect(b.map((x) => x.label)).toEqual(['Play', 'Update', 'Game Settings', 'Verify and repair', 'Uninstall']);
    expect(b.find((x) => x.act === 'verify')?.disabled).toBe(false);
    expect(storeButtons({ game: amazonGame('Kettle Drum'), status: amazonStatus({ connection: 'expired' }), job: null, playing: false, confirm: null })[0]).toMatchObject({
      label: 'Install…',
      disabled: true,
    });
  });
  it('writes the lines with no cloud line, and the backup refusal', () => {
    const fog = amazonGame('Fogline', { compat_tool: 'GE-Proton10-17' });
    expect(storeLines({ game: fog, status: amazon, job: null, playing: false, confirm: null }, { ...opts, lastPlayedAt: now - 30 * 86400 }).map((l) => l.text)).toEqual([
      'Last played last month',
      'Runs through GE-Proton10-17 · Windows version',
      'Amazon Games games can’t be backed up: nile installs only from Amazon’s servers.',
    ]);
    const expired = storeLines({ game: amazonGame('Lowsun'), status: amazonStatus({ connection: 'expired' }), job: null, playing: false, confirm: null }, opts);
    expect(expired[0]).toMatchObject({ text: 'Your Amazon Games login expired.', link: { label: 'Sign in again' } });
    const off = storeLines({ game: amazonGame('Lowsun'), status: amazonStatus({ connection: 'not_connected' }), job: null, playing: false, confirm: null }, opts);
    expect(off[0].text).toBe('Connect Amazon Games in Settings › Stores to install.');
  });
  it('has Overview and Media only, and no version chip, Version fact or DLC', () => {
    const fog = amazonGame('Fogline', { update_version: FAKE_AMAZON_UPDATE, dlc: [{ id: 'x', title: 'x', download_bytes: 1, installed: false }] });
    expect(storeTabs(fog)).toEqual(['overview', 'media']);
    expect(storeDetailChips(fog, false).map((c) => c.text)).toEqual(['Amazon Games', 'Windows version']);
    expect(storeFacts(fog).map((f) => f.term)).toEqual(['Store', 'Developer', 'Released', 'Genres', 'Size', 'Folder']);
    expect(dlcRows(fog, [])).toEqual([]);
  });
});

describe('Amazon Games Settings card', () => {
  const amazon = amazonStatus();
  it('words the account, tool, license and the Disconnect confirm without cloud saves', () => {
    expect(accountLine(amazon)).toBe('Signed in as sam.plays');
    expect(accountLine(amazonStatus({ connection: 'not_connected', installed_count: 1 }))).toBe('Not signed in');
    expect(toolLine(amazon)).toEqual({ kind: 'installed', text: 'nile 1.2.0 · installed' });
    expect(toolLine(amazonStatus({ connection: 'not_connected', tool: { state: 'not_installed' } as never })).text).toBe('Installs when you connect · 11 MB');
    expect(licenseLine(amazon)).toEqual({ text: 'nile · GPL-3.0', url: 'https://github.com/imLinguin/nile', label: 'github.com/imLinguin/nile' });
    expect(disconnectConfirm(amazonStatus({ installed_count: 1 }))).toEqual({
      bold: 'Disconnect Amazon Games?',
      rest: 'GRID deletes your Amazon Games login and nile from this computer. The 1 installed game stay and still play; they can’t install or update until you connect again.',
    });
  });
  it('words the not-connected lede', () => {
    expect(notConnectedLede(amazonStatus({ connection: 'not_connected' }))).toBe(
      'Sign in to Amazon Games to see the games you own in the Library and install them. GRID installs with nile (nile, GPL-3.0, 11 MB), which it downloads when you connect.',
    );
  });
});

describe('Amazon Games cards and Downloads', () => {
  it('words the cards with no cloud sentence', () => {
    expect(uninstalledNotice('amazon', 'Fogline', true).text).toBe('Fogline uninstalled');
    expect(noticeActionLabel({ kind: 'install_tool' }, 'amazon')).toBe('Install nile');
    expect(toolNeededNotice('amazon').text).toBe('Amazon Games needs nile to install games');
  });
  it('words the rows: Updated with no version, Verified with no count', () => {
    const base = { source_id: 'amazon:amzn1.adg.product.x', title: 'Fogline', platform: 'Amazon Games' };
    expect(storeEntryLine(row({ ...base, kind: 'store_update', status: 'completed' }), { version: FAKE_AMAZON_UPDATE })).toBe('Updated');
    expect(storeEntryLine(row({ ...base, kind: 'store_repair', status: 'completed' }))).toBe('Verified');
    expect(storeEntryLine(row({ ...base, kind: 'store_repair', phase: 'verifying' }))).toBe('Verifying files 45%');
    expect(storeStripLine(row({ ...base }))).toBe('Fogline · Amazon Games · Downloading 45% · 41.2 MB/s');
    expect(storeStripLine(row({ kind: 'store_tool', source_id: 'amazon', title: 'nile', platform: 'Amazon Games', phase: 'checksum' }))).toBe(
      'nile · Amazon Games · Downloading 45% · checking its SHA-256',
    );
    // GOG and Epic keep their version.
    expect(storeEntryLine(row({ kind: 'store_update', status: 'completed' }), { version: '2.2' })).toBe('Updated to 2.2');
  });
});
