import { describe, expect, it } from 'vitest';
import type { DownloadEntry } from '../api';
import { BackupBatches, BATCH_SETTLE_MS, osChangeHint, osNames } from './notices';

function entry(overrides: Partial<DownloadEntry>): DownloadEntry {
  return {
    id: 1,
    job: 'backup',
    kind: 'backup_game',
    rom_id: 0,
    source_id: '',
    title: 'Velvet Static',
    platform: 'PlayStation',
    status: 'queued',
    downloaded_bytes: 0,
    total_bytes: 0,
    speed_bps: 0,
    install_processed_bytes: 0,
    install_total_bytes: 0,
    error: '',
    detail: '',
    drive_id: '/run/media/sam/SAMSUNG T7',
    drive_name: 'Couch kit',
    drive_label: 'SAMSUNG T7',
    batch: 3,
    ...overrides,
  };
}

const game = (id: number, status: DownloadEntry['status'], extra: Partial<DownloadEntry> = {}) =>
  entry({ id, title: `Game ${id}`, status, ...extra });
const emulator = (id: number, title: string, status: DownloadEntry['status'], extra: Partial<DownloadEntry> = {}) =>
  entry({ id, kind: 'backup_emulator', title, platform: 'Emulator', status, ...extra });

/**
 * Runs `snapshots` (newest entry first, as the queue lists them), one every
 * 100 ms, letting due queued cards show between them, and returns every card
 * text. The first snapshot is the baseline.
 */
function run(snapshots: DownloadEntry[][], b = new BackupBatches()): string[] {
  const out: string[] = [];
  let now = 0;
  snapshots.forEach((snap, i) => {
    for (const n of b.observe(snap, i === 0, now)) out.push(n.text);
    now += 100;
    for (const n of b.due(now)) out.push(n.text);
  });
  now += BATCH_SETTLE_MS;
  for (const n of b.due(now)) out.push(n.text);
  return out;
}

describe('BackupBatches', () => {
  it('shows the queued card once, after the batch stops growing', () => {
    const b = new BackupBatches();
    expect(b.observe([], true, 0)).toEqual([]);
    expect(b.observe([game(1, 'downloading')], false, 0)).toEqual([]);
    expect(b.observe([game(2, 'queued'), game(1, 'downloading')], false, 100)).toEqual([]);
    expect(b.observe([game(3, 'queued'), game(2, 'queued'), game(1, 'downloading')], false, 200)).toEqual([]);
    expect(b.nextDueIn(200)).toBe(BATCH_SETTLE_MS);
    expect(b.due(200 + BATCH_SETTLE_MS - 1)).toEqual([]);
    expect(b.due(200 + BATCH_SETTLE_MS).map((n) => n.text)).toEqual([
      '3 games added to the backup queue for Couch kit',
    ]);
    expect(b.due(5_000)).toEqual([]);
    expect(b.nextDueIn(5_000)).toBeNull();
    // Progress is not news.
    expect(b.observe([game(3, 'queued'), game(2, 'downloading'), game(1, 'installing')], false, 6_000)).toEqual([]);
  });

  it('names a single game in the queued card', () => {
    expect(run([[], [game(1, 'downloading', { title: 'Velvet Static' })]])).toEqual([
      'Velvet Static added to the backup queue for Couch kit',
    ]);
  });

  it('shows the done card once, when every row is terminal', () => {
    const live = [emulator(4, 'DuckStation for Linux', 'queued'), game(3, 'queued'), game(2, 'queued'), game(1, 'downloading')];
    const texts = run([
      [],
      live,
      [emulator(4, 'DuckStation for Linux', 'queued'), game(3, 'queued'), game(2, 'downloading'), game(1, 'completed')],
      [emulator(4, 'DuckStation for Linux', 'installing'), game(3, 'completed'), game(2, 'completed'), game(1, 'completed')],
      [emulator(4, 'DuckStation for Linux', 'completed'), game(3, 'completed'), game(2, 'completed'), game(1, 'completed')],
      [emulator(4, 'DuckStation for Linux', 'completed'), game(3, 'completed'), game(2, 'completed'), game(1, 'completed')],
    ]);
    expect(texts).toEqual([
      '3 games added to the backup queue for Couch kit',
      '3 games and 1 emulator backed up to Couch kit',
    ]);
  });

  it('counts an emulator built for two computers once, and names the compat tools apart', () => {
    const done = (rows: DownloadEntry[]) => rows.map((r) => ({ ...r, status: 'completed' as const }));
    const rows = [
      emulator(6, 'Steam Linux Runtime', 'queued', { platform: 'Compatibility Tool' }),
      emulator(5, 'umu-launcher for Linux', 'queued', { platform: 'Compatibility Tool' }),
      emulator(4, 'DuckStation for Windows', 'queued'),
      emulator(3, 'DuckStation for Linux', 'queued'),
      entry({ id: 2, kind: 'backup_firmware', title: 'PlayStation firmware', status: 'queued' }),
      game(1, 'downloading'),
    ];
    expect(run([[], rows, done(rows)])).toEqual([
      'Game 1 added to the backup queue for Couch kit',
      'Game 1, 1 emulator, 2 compatibility tools and PlayStation firmware backed up to Couch kit',
    ]);
  });

  it('shows a lone queued card before the done card when a batch ends before it settles', () => {
    const b = new BackupBatches();
    b.observe([], true, 0);
    expect(b.observe([emulator(1, 'DuckStation for Linux', 'downloading')], false, 0)).toEqual([]);
    expect(b.observe([emulator(1, 'DuckStation for Linux', 'completed')], false, 50).map((n) => n.text)).toEqual([
      'DuckStation added to the backup queue for Couch kit',
      'DuckStation backed up to Couch kit',
    ]);
    expect(b.due(5_000)).toEqual([]);
  });

  it('shows one error card when the drive is removed mid-backup', () => {
    // 17 items: 4 written, the 5th was being written, 12 more waited.
    const written = [1, 2, 3, 4].map((id) => game(id, 'completed'));
    const waiting = Array.from({ length: 12 }, (_, i) => game(6 + i, 'queued'));
    const before = [...waiting.slice().reverse(), game(5, 'installing'), ...written.slice().reverse()];
    const summary = entry({ id: 40, kind: 'backup_cancelled', status: 'cancelled', title: '12 more items', error: 'SAMSUNG T7 was removed' });
    const after = [summary, game(5, 'failed', { error: 'SAMSUNG T7 was removed' }), ...written.slice().reverse()];
    const texts = run([[], before, [summary, game(5, 'installing'), ...written.slice().reverse()], after, after]);
    expect(texts).toEqual([
      '17 games added to the backup queue for Couch kit',
      'SAMSUNG T7 was removed. 4 of 17 items were written to Couch kit.',
    ]);
  });

  it('gives the error card the error level and no done card', () => {
    const b = new BackupBatches();
    b.observe([], true, 0);
    b.observe([game(2, 'queued'), game(1, 'downloading')], false, 0);
    b.due(1_000);
    const end = b.observe([game(2, 'completed'), game(1, 'failed', { error: 'HTTP 404' })], false, 2_000);
    expect(end).toEqual([
      { group: 'backup-failed', text: "Couldn't backup Game 1: HTTP 404. 1 of 2 items were written to Couch kit.", level: 'error' },
    ]);
  });

  it('reports another batch for the same drive whose rows were folded away', () => {
    const b = new BackupBatches();
    b.observe([], true, 0);
    b.observe([game(2, 'queued', { batch: 9 }), game(1, 'downloading')], false, 0);
    b.due(1_000);
    const summary = entry({ id: 3, batch: 9, kind: 'backup_cancelled', status: 'cancelled', title: 'Game 2', error: 'SAMSUNG T7 was removed' });
    const texts = b.observe([summary, game(1, 'failed', { error: 'SAMSUNG T7 was removed' })], false, 2_000).map((n) => n.text);
    expect(texts).toEqual([
      'SAMSUNG T7 was removed. 0 of 1 item was written to Couch kit.',
      'SAMSUNG T7 was removed. 0 of 1 item was written to Couch kit.',
    ]);
  });

  it('shows no card for a batch every row of which was cancelled by hand', () => {
    expect(run([[], [game(1, 'downloading')], [game(1, 'cancelled')]])).toEqual([
      'Game 1 added to the backup queue for Couch kit',
    ]);
  });

  it('treats the baseline as old news, and never repeats a finished batch', () => {
    const b = new BackupBatches();
    expect(b.observe([game(2, 'queued'), game(1, 'downloading')], true, 0)).toEqual([]);
    expect(b.due(5_000)).toEqual([]);
    const done = [game(2, 'completed'), game(1, 'completed')];
    expect(b.observe(done, false, 6_000).map((n) => n.text)).toEqual(['2 games backed up to Couch kit']);
    expect(b.observe(done, false, 7_000)).toEqual([]);
    // Dismissing rows of a finished batch is not news either.
    expect(b.observe([game(1, 'completed')], false, 8_000)).toEqual([]);
  });

  it('gives a backup row outside a batch its own outcome card', () => {
    const b = new BackupBatches();
    b.observe([], true, 0);
    const summary = entry({ id: 7, batch: 0, kind: 'backup_cancelled', status: 'cancelled', title: '3 more items', error: 'SAMSUNG T7 was removed' });
    expect(b.observe([summary], false, 100)).toEqual([
      { group: 'drive-failed', text: 'Cancelled 3 more items: SAMSUNG T7 was removed', level: 'error' },
    ]);
    expect(b.observe([summary], false, 200)).toEqual([]);
  });

  it('ignores rows that are not backups', () => {
    expect(run([[], [entry({ job: 'game', kind: 'base', batch: 0 })], [entry({ job: 'game', kind: 'base', batch: 0, status: 'completed' })]])).toEqual([]);
  });

  it('words an OS-change batch by its hint', () => {
    const b = new BackupBatches();
    b.observe([], true, 0);
    const rows = [emulator(2, 'PCSX2 for Windows', 'queued', { batch: 5 }), emulator(1, 'DuckStation for Windows', 'downloading', { batch: 5 })];
    b.observe(rows, false, 0);
    const hint = osChangeHint({ batch: 5, queued: 2, removed: 0 }, ['windows']);
    expect(hint).not.toBeNull();
    b.hint(5, hint!);
    expect(b.due(1_000).map((n) => n.text)).toEqual(['Windows builds of 2 emulators added to the backup queue for Couch kit']);
    expect(b.observe(rows.map((r) => ({ ...r, status: 'completed' as const })), false, 2_000).map((n) => n.text)).toEqual([
      'Windows builds of 2 emulators backed up to Couch kit',
    ]);
  });
});

describe('helpers', () => {
  it('has no hint when nothing was queued', () => {
    expect(osChangeHint({ batch: 0, queued: 0, removed: 2 }, ['linux'])).toBeNull();
    expect(osChangeHint({ batch: 3, queued: 1, removed: 0 }, [])).toBeNull();
  });

  it('names OSes', () => {
    expect(osNames(['windows'])).toBe('Windows');
    expect(osNames(['linux', 'windows'])).toBe('Linux and Windows');
  });
});
