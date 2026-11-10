import { describe, expect, it } from 'vitest';
import { driveGame, entry, row, summary } from './fixtures.test-util';
import { gameFromDrive, gameFromRow, gameFromSummary, liveInstall, platformTag, queueTag, quickFor } from './games';

describe('TV games', () => {
  it('a server game: its cover, installed state from its row, update only when installed', () => {
    const g = gameFromSummary(summary(), 'SNES', 'snes', null, new Set([501]));
    expect(g).toMatchObject({ id: '501', romId: 501, cover: 's.png', installed: false, update: false, releasedAt: 1_794_000_000 });
    const i = gameFromSummary(summary(), 'SNES', 'snes', row({ rom_id: 501, missing: true, last_played_at: 9 }), new Set([501]));
    expect(i).toMatchObject({ installed: true, missing: true, update: true, lastPlayedAt: 9 });
  });

  it('an installed row: large cover first, release date read from epoch seconds, unlinked rows keyed by title', () => {
    const g = gameFromRow(row(), new Set());
    expect(g).toMatchObject({ id: '42', cover: 'l.png', installed: true, releasedAt: 795_571_200 });
    expect(gameFromRow(row({ rom_id: null }), new Set()).id.startsWith('i:')).toBe(true);
  });

  it('the platform tag is the desktop chip text', () => {
    expect(platformTag(gameFromRow(row(), new Set()))).toBe('SNES');
  });

  it('a cover tag: Queued, then the percent (download, then install progress)', () => {
    expect(queueTag(liveInstall([entry({ status: 'queued' })], 501))).toBe('Queued');
    expect(queueTag(liveInstall([entry({ status: 'downloading', downloaded_bytes: 62, total_bytes: 100 })], 501))).toBe('62%');
    expect(queueTag(liveInstall([entry({ status: 'installing', install_processed_bytes: 1, install_total_bytes: 4 })], 501))).toBe('25%');
    expect(queueTag(liveInstall([entry({ status: 'completed' })], 501))).toBeNull();
    expect(queueTag(liveInstall([entry({ status: 'downloading', job: 'backup' })], 501))).toBeNull();
  });

  it('X (item 192): Play when installed, Install when not, nothing while installing or updating, missing, unlinked or offline', () => {
    const server = gameFromSummary(summary(), 'SNES', 'snes', null, new Set());
    expect(quickFor(server, null, true)?.label).toBe('Install');
    expect(quickFor(server, null, false)).toBeNull();
    expect(quickFor(server, { queued: true, pct: 0 }, true)).toBeNull();
    expect(quickFor(gameFromRow(row(), new Set()), null, false)?.label).toBe('Play');
    // Item 192: an installed game with an update downloading or queued has no X.
    expect(quickFor(gameFromRow(row(), new Set([42])), { queued: false, pct: 40 }, true)).toBeNull();
    expect(quickFor(gameFromRow(row(), new Set([42])), { queued: true, pct: 0 }, true)).toBeNull();
    expect(quickFor(gameFromRow(row({ missing: true }), new Set()), null, true)).toBeNull();
    expect(quickFor(gameFromRow(row({ rom_id: null }), new Set()), null, true)).toBeNull();
  });

  it('a drive game: its cover from the drive, installed from this computer, Install from drive', () => {
    const g = gameFromDrive('/m/t7', driveGame(), null, new Set([901]));
    expect(g).toMatchObject({ romId: 901, title: 'Cinder Peaks', platform: 'PlayStation 2', installed: false, update: false, row: null });
    expect(g.cover).toContain('covers/901.png');
    expect(g.drive?.key).toBe('/m/t7');
    expect(quickFor(g, null, false)?.label).toBe('Install from drive');
    expect(quickFor(g, null, false)?.kind).toBe('install-drive');
    expect(quickFor(g, { queued: true, pct: 0 }, false)).toBeNull();
    const i = gameFromDrive('/m/t7', driveGame(), row({ rom_id: 901, last_played_at: 7 }), new Set([901]));
    expect(i).toMatchObject({ installed: true, update: true, lastPlayedAt: 7 });
    expect(quickFor(i, null, false)?.label).toBe('Play');
    expect(gameFromDrive('/m/t7', driveGame({ installed: true }), null, new Set()).installed).toBe(true);
  });
});
