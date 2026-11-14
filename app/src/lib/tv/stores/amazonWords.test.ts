// TV mode's Amazon Games words (Phase 3): the TVStores prototype's Amazon
// states, through the same capabilities as the desktop — no Cloud saves chip,
// tab or row, no DLC, "Update" with no version.
import { describe, expect, it } from 'vitest';
import { amazonGame, amazonStatus, row } from '../../gamestores/fixtures.test-util';
import { FAKE_AMAZON_UPDATE, FAKE_NILE_GLIBC_ERROR } from '../../gamestores/fakeApi';
import {
  disconnectedCard,
  disconnectWords,
  launchingCard,
  loginHost,
  notConnectedNote,
  storeRowSub,
  toolRow,
  tvChips,
  tvCoverTag,
  tvDetailChips,
  tvFacts,
  tvLeftLines,
  tvPanel,
  tvPlayingNote,
  tvStoreButtons,
  tvStoreLine,
  tvTabs,
  uninstallWords,
  updateWords,
} from './storeWords';

const now = 10_000_000;

describe('TV Amazon Games', () => {
  it('shows no Cloud saves chip', () => {
    expect(tvChips(amazonStatus({ cloud_saves: true })).map((c) => c.key)).toEqual(['account']);
    expect(tvChips(amazonStatus({ connection: 'expired' }))[0]).toMatchObject({ text: 'Login expired · Sign in again', signIn: true });
  });
  it('words the expired and tool panels', () => {
    expect(tvPanel(amazonStatus({ connection: 'expired' }), 'available', [])?.title).toBe('Your Amazon Games login expired');
    expect(tvPanel(amazonStatus({ tool: { state: 'failed', error: FAKE_NILE_GLIBC_ERROR } as never }), 'available', [])?.body).toBe(FAKE_NILE_GLIBC_ERROR);
  });
  it('lays out the game page: Update with no version, no cloud line, the backup refusal', () => {
    const fog = amazonGame('Fogline', { update_version: FAKE_AMAZON_UPDATE, compat_tool: 'GE-Proton10-17' });
    expect(tvStoreButtons({ game: fog, status: amazonStatus(), job: null, fromDrive: false }).map((b) => b.label)).toEqual([
      'Play',
      'Update',
      'Game settings',
      'Verify and repair',
      'Uninstall',
    ]);
    expect(tvLeftLines({ game: fog, nowSeconds: now, lastPlayedAt: 0, compat: 'GE-Proton10-17', drive: null, onDrives: [] }).map((l) => l.key)).toEqual([
      'played',
      'runs',
      'nobk',
    ]);
    expect(tvTabs(fog)).toEqual(['Overview', 'Media']);
    expect(tvDetailChips(fog).map((c) => c.text)).toEqual(['Installed', 'Update available']);
    expect(tvFacts(fog).map((f) => f.term)).toEqual(['Store', 'Developer', 'Release', 'Size']);
    expect(tvCoverTag(fog, null)?.text).toBe('UPDATE');
    expect(tvCoverTag(amazonGame('Fogline', { cloud: 'conflict' }), null)).toBeNull();
    expect(updateWords(fog).title).toBe('Update Fogline?');
    expect(uninstallWords(fog).body).toBe('Its files are deleted from this computer.');
    expect(launchingCard(fog)).toEqual({ text: 'Launching Fogline…' });
    expect(tvPlayingNote(fog)).toBeNull();
    const expired = tvStoreButtons({ game: amazonGame('Lowsun'), status: amazonStatus({ connection: 'expired' }), job: null, fromDrive: false })[0];
    expect(expired).toMatchObject({ label: 'Install…', disabled: true, why: 'Your Amazon Games login expired. Sign in again in Settings › Stores.' });
  });
  it('words Settings › Stores › Amazon Games', () => {
    expect(storeRowSub(amazonStatus())).toBe('Signed in as sam.plays');
    expect(toolRow(amazonStatus())).toEqual({ info: true, value: 'nile 1.2.0 · installed' });
    expect(notConnectedNote(amazonStatus({ connection: 'not_connected' }))).toBe(
      'Sign in to Amazon Games to see the games you own in the Library and install them. GRID installs with nile (nile, GPL-3.0, 11 MB), which it downloads when you connect.',
    );
    expect(disconnectWords(amazonStatus({ installed_count: 1 })).body).toBe(
      'GRID deletes your Amazon Games login and nile from this computer. The 1 installed games stay and still play; they can’t install or update until you connect again.',
    );
    expect(disconnectedCard('amazon')).toEqual({ text: 'Amazon Games disconnected', sub: 'Its login and nile were deleted; installed games stay.' });
    expect(loginHost('amazon')).toBe('www.amazon.com/ap/signin');
  });
  it('words the Downloads rows', () => {
    const done = row({ source_id: 'amazon:amzn1.adg.product.x', title: 'Fogline', platform: 'Amazon Games', kind: 'store_update', status: 'completed' });
    expect(tvStoreLine(done, 'GE-Proton10-17', FAKE_AMAZON_UPDATE)).toBe('Updated');
  });
});
