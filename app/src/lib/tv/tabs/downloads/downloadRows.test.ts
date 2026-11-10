import { describe, expect, it } from 'vitest';
import { entry } from '../../fixtures.test-util';
import { CANCEL_EXTRACTING } from '../../pages/details';
import {
  cancelWords,
  completedIds,
  hasCompleted,
  isFailed,
  kindBadge,
  listSub,
  progress,
  rowA,
  rowWhy,
  rowActions,
  speedText,
  statusLine,
  tvSegments,
} from './downloadRows';

const MB = 1024 * 1024;

describe('downloadRows', () => {
  it('splits rows into Active, Queued and Completed with counts and empty texts (items 209–211)', () => {
    const segs = tvSegments([
      entry({ id: 1, status: 'downloading' }),
      entry({ id: 2, status: 'queued' }),
      entry({ id: 3, status: 'completed' }),
      entry({ id: 4, status: 'failed' }),
      entry({ id: 5, status: 'cancelled' }),
      entry({ id: 6, status: 'installing' }),
    ]);
    expect(segs.map((s) => [s.name, s.rows.map((r) => r.id)])).toEqual([
      ['Active', [1, 6]],
      ['Queued', [2]],
      ['Completed', [3, 4, 5]],
    ]);
    expect(tvSegments([]).map((s) => s.empty)).toEqual(['Nothing active.', 'Nothing queued.', 'Nothing completed.']);
  });

  it('reads each status as "<Platform> · <status>" (items 212–214)', () => {
    expect(
      statusLine(entry({ status: 'downloading', downloaded_bytes: 45 * MB, total_bytes: 100 * MB, speed_bps: 5 * MB })),
    ).toBe('SNES · Downloading · 45% · 5.0 MB/s · 11s left');
    expect(statusLine(entry({ status: 'queued' }))).toBe('SNES · Queued');
    expect(statusLine(entry({ status: 'failed', error: 'the server returned 503' }))).toBe('SNES · Failed · the server returned 503');
    expect(statusLine(entry({ status: 'cancelled' }))).toBe('SNES · Cancelled');
    expect(statusLine(entry({ status: 'installing', install_processed_bytes: 1, install_total_bytes: 4 }))).toBe('SNES · Installing · 25%');
    expect(statusLine(entry({ status: 'completed', downloaded_bytes: 2 * MB }))).toBe('SNES · Completed · 2.0 MB');
    expect(statusLine(entry({ platform: '', status: 'queued' }))).toBe('Queued');
    // From a backup drive, and a backup row (Game backups card wording).
    expect(statusLine(entry({ status: 'downloading', drive_label: 'SAMSUNG T7', downloaded_bytes: 1, total_bytes: 2 }))).toBe(
      'SNES · Copying from SAMSUNG T7 · 50%',
    );
    expect(statusLine(entry({ status: 'queued', drive_label: 'SAMSUNG T7' }))).toBe('SNES · Queued · from SAMSUNG T7');
    expect(statusLine(entry({ status: 'completed', drive_label: 'SAMSUNG T7', downloaded_bytes: 10 }))).toBe('SNES · Installed from SAMSUNG T7');
    expect(statusLine(entry({ job: 'backup', kind: 'backup_game', status: 'queued', drive_name: 'Couch kit' }))).toBe('SNES · Waiting · to Couch kit');
    expect(isFailed(entry({ status: 'failed' }))).toBe(true);
    expect(isFailed(entry({ status: 'cancelled' }))).toBe(false);
  });

  it('badges the kind, with a library move as "Library" (item 212)', () => {
    expect(kindBadge(entry({ kind: 'base' }))).toBe('');
    expect(kindBadge(entry({ kind: 'update' }))).toBe('Update');
    expect(kindBadge(entry({ job: 'backup', kind: 'backup_game' }))).toBe('Backup');
    expect(kindBadge(entry({ job: 'library', kind: 'library_move' }))).toBe('Library');
    expect(listSub(entry({ kind: 'update' }))).toBe('SNES · Update');
    expect(listSub(entry({ kind: 'base' }))).toBe('SNES');
  });

  it('shows a bar and the speed only while active', () => {
    expect(progress(entry({ status: 'downloading', downloaded_bytes: 1, total_bytes: 4 }))).toBe(25);
    expect(progress(entry({ status: 'queued' }))).toBeNull();
    expect(speedText(entry({ status: 'downloading', speed_bps: 38.2 * MB }))).toBe('38.2 MB/s');
    expect(speedText(entry({ status: 'completed', speed_bps: 38.2 * MB }))).toBe('');
  });

  it('names the A action and lists the actions per state (items 216–217)', () => {
    const kinds = (e: ReturnType<typeof entry>) => rowActions(e, null).map((a) => a.label);
    const active = entry({ title: 'Kite Season', status: 'downloading' });
    expect(rowA(active, null)).toBe('Cancel');
    expect(kinds(active)).toEqual(['Cancel', 'Open Kite Season']);
    expect(rowActions(active, null)[0].danger).toBe(true);
    expect(rowA(entry({ status: 'queued' }), null)).toBe('Cancel');
    const failed = entry({ title: 'Kite Season', status: 'failed' });
    expect(rowA(failed, null)).toBe('Retry or dismiss');
    expect(kinds(failed)).toEqual(['Retry', 'Dismiss', 'Open Kite Season']);
    const cancelled = entry({ title: 'Kite Season', status: 'cancelled' });
    expect(rowA(cancelled, null)).toBe('Dismiss');
    expect(kinds(cancelled)).toEqual(['Retry', 'Dismiss', 'Open Kite Season']);
    const done = entry({ title: 'Kite Season', status: 'completed' });
    expect(rowA(done, null)).toBe('Dismiss');
    expect(kinds(done)).toEqual(['Dismiss', 'Open Kite Season']);
    // An extracting install can't be stopped: it still opens its game.
    const extracting = entry({ title: 'Kite Season', status: 'installing' });
    expect(kinds(extracting)).toEqual(['Open Kite Season']);
    expect(rowA(extracting, null)).toBe('Details');
    // An emulator row opens no game; a live firmware row offers nothing.
    expect(kinds(entry({ job: 'emulator', kind: 'emulator', rom_id: 0, status: 'downloading' }))).toEqual(['Cancel']);
    const firmware = entry({ job: 'firmware', kind: 'firmware', rom_id: 0, status: 'downloading' });
    expect(rowA(firmware, null)).toBeNull();
    expect(rowWhy(firmware)).toBe('Installing firmware — this step cannot be cancelled');
    expect(rowWhy(entry({ job: 'library', kind: 'library_move', rom_id: 0, status: 'downloading' }))).toBe('Updating paths — this step cannot be cancelled');
    expect(rowWhy(extracting)).toBe(CANCEL_EXTRACTING);
    // The library move's own commands: Retry and Cancel while its copy waits.
    const move = entry({ job: 'library', kind: 'library_move', rom_id: 0, status: 'failed' });
    expect(kinds(move)).toEqual(['Dismiss']);
    expect(rowActions(move, 'copy').map((a) => a.label)).toEqual(['Cancel', 'Retry']);
    expect(rowA(move, 'copy')).toBe('Retry or cancel');
    expect(rowActions(move, 'commit').map((a) => a.label)).toEqual(['Retry']);
  });

  it('asks before Cancel for every kind of row (ruling 8; "backup" one word, item 419)', () => {
    expect(cancelWords(entry({ title: 'Kite Season', status: 'downloading' }))).toEqual({
      title: 'Cancel Kite Season?',
      body: 'The download stops and its files are removed. You can install it again later.',
      ok: 'Cancel install',
    });
    expect(cancelWords(entry({ title: 'Kite Season', status: 'queued' }))).toEqual({
      title: 'Cancel Kite Season?',
      body: 'It leaves the queue. You can install it again later.',
      ok: 'Cancel install',
    });
    expect(cancelWords(entry({ job: 'backup', kind: 'backup_game', title: 'Kite Season', drive_name: 'Couch kit', drive_label: 'SAMSUNG T7' }))).toEqual({
      title: 'Cancel Kite Season?',
      body: 'The backup stops and what it wrote to SAMSUNG T7 is removed. You can backup it again later.',
      ok: 'Cancel backup',
    });
    expect(cancelWords(entry({ job: 'library', kind: 'library_move' }))).toEqual({
      title: 'Cancel the move?',
      body: 'The files copied so far are removed and the library stays where it is.',
      ok: 'Cancel move',
    });
  });

  it('clears every finished, failed and cancelled row (item 222)', () => {
    const list = [
      entry({ id: 1, status: 'downloading' }),
      entry({ id: 2, status: 'completed' }),
      entry({ id: 3, status: 'failed' }),
      entry({ id: 4, status: 'cancelled' }),
      entry({ id: 5, status: 'queued' }),
    ];
    expect(hasCompleted(list)).toBe(true);
    expect(completedIds(list)).toEqual([2, 3, 4]);
    expect(hasCompleted([entry({ status: 'queued' })])).toBe(false);
  });
});
