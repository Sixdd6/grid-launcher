import { describe, expect, it } from 'vitest';
import { entry, row, summary } from './fixtures.test-util';
import { gameFromRow, gameFromSummary, liveInstall, platformTag, queueTag, quickFor } from './games';

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

  it('X: Play when installed, Install when not, nothing while installing, missing, unlinked or offline', () => {
    const server = gameFromSummary(summary(), 'SNES', 'snes', null, new Set());
    expect(quickFor(server, null, true)?.label).toBe('Install');
    expect(quickFor(server, null, false)).toBeNull();
    expect(quickFor(server, { queued: true, pct: 0 }, true)).toBeNull();
    expect(quickFor(gameFromRow(row(), new Set()), null, false)?.label).toBe('Play');
    expect(quickFor(gameFromRow(row({ missing: true }), new Set()), null, true)).toBeNull();
    expect(quickFor(gameFromRow(row({ rom_id: null }), new Set()), null, true)).toBeNull();
  });
});
