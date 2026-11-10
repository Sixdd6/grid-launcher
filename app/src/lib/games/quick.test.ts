import { describe, expect, it } from 'vitest';
import type { DownloadStatus } from '../api';
import { discoverAction } from '../discover/feed';
import { cardPrimaryLabel } from '../library/missing';
import { detailsPrimary, quickAction, type DetailsInput } from './quick';

describe('quickAction', () => {
  it('plays an installed game', () => {
    expect(quickAction({ installed: true, romId: 7 })).toEqual({ kind: 'play', label: 'Play' });
  });

  it('plays an installed game offline too', () => {
    expect(quickAction({ installed: true, romId: 7, connected: false }).kind).toBe('play');
  });

  it('locates a missing game', () => {
    expect(quickAction({ installed: true, romId: 7, missing: true })).toEqual({ kind: 'locate', label: 'Locate…' });
  });

  it('offers nothing for an unlinked installed row', () => {
    expect(quickAction({ installed: true, romId: null })).toEqual({ kind: 'none', label: '' });
  });

  it('says Queued and Installing… while an install is live', () => {
    expect(quickAction({ installed: false, romId: 7, live: 'queued' })).toEqual({ kind: 'queued', label: 'Queued' });
    for (const live of ['downloading', 'installing', 'cancelling'] as DownloadStatus[]) {
      expect(quickAction({ installed: false, romId: 7, live })).toEqual({ kind: 'installing', label: 'Installing…' });
    }
  });

  it('installs from the server when connected, and offers nothing offline', () => {
    expect(quickAction({ installed: false, romId: 7 })).toEqual({ kind: 'install', label: 'Install' });
    expect(quickAction({ installed: false, romId: 7, connected: false })).toEqual({ kind: 'none', label: '' });
  });

  it('installs from a drive, connected or not', () => {
    const drive = { kind: 'install-drive', label: 'Install from drive' };
    expect(quickAction({ installed: false, romId: 7, fromDrive: true })).toEqual(drive);
    expect(quickAction({ installed: false, romId: 7, fromDrive: true, connected: false })).toEqual(drive);
  });

  it('plays a drive game that is installed already', () => {
    expect(quickAction({ installed: true, romId: 7, fromDrive: true }).kind).toBe('play');
  });

  it('agrees with the Discover button for every state', () => {
    const lives: (DownloadStatus | null)[] = [null, 'queued', 'downloading', 'installing', 'cancelling'];
    for (const installed of [true, false]) {
      for (const live of lives) {
        expect(quickAction({ installed, romId: 1, live }).label).toBe(discoverAction(installed, live));
      }
    }
  });

  it('agrees with the card button label', () => {
    for (const [installed, missing] of [
      [true, false],
      [true, true],
      [false, false],
    ]) {
      expect(quickAction({ installed, missing, romId: 1 }).label).toBe(cardPrimaryLabel(installed, missing));
    }
  });
});

describe('detailsPrimary', () => {
  const base: DetailsInput = { romId: 7, unlinked: false, live: false, playing: false, installed: false, fromDrive: false };

  it('links an unlinked row and explains a row with no id', () => {
    expect(detailsPrimary({ ...base, romId: null, unlinked: true })).toBe('link');
    expect(detailsPrimary({ ...base, romId: null })).toBe('no-id');
  });

  it('puts a live install before everything else', () => {
    expect(detailsPrimary({ ...base, live: true, playing: true, installed: true })).toBe('cancel');
  });

  it('stops a running game before offering Play', () => {
    expect(detailsPrimary({ ...base, playing: true, installed: true })).toBe('stop');
  });

  it('plays an installed game', () => {
    expect(detailsPrimary({ ...base, installed: true, fromDrive: true })).toBe('play');
  });

  it('installs from a drive or the server', () => {
    expect(detailsPrimary({ ...base, fromDrive: true })).toBe('install-drive');
    expect(detailsPrimary(base)).toBe('install');
  });
});
