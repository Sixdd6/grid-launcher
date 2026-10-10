import { describe, expect, it } from 'vitest';
import {
  canMove,
  canStartFresh,
  formatBytes,
  outcomeNotice,
  moveFinishedToast,
  moveItemLine,
  moveInProgress,
  movePreviewSummary,
  moveStartNotice,
  paneMoveText,
  previewSummary,
  resumeToast,
} from './library';
import type { LibraryMoveStatus, MovePreview } from '../api';

describe('formatBytes', () => {
  it('uses the largest unit below 1024, one decimal past bytes', () => {
    expect(formatBytes(0)).toBe('0 B');
    expect(formatBytes(1023)).toBe('1023 B');
    expect(formatBytes(1024)).toBe('1.0 KB');
    expect(formatBytes(1536)).toBe('1.5 KB');
    expect(formatBytes(5 * 1024 * 1024)).toBe('5.0 MB');
    expect(formatBytes(3.25 * 1024 ** 3)).toBe('3.3 GB');
    expect(formatBytes(2 * 1024 ** 4)).toBe('2.0 TB');
  });
});

describe('canStartFresh', () => {
  it('needs an accepted check for the typed path', () => {
    expect(canStartFresh(null, '/new')).toBe(false);
    expect(canStartFresh({ status: 'ok', path: '/new' }, '/new')).toBe(true);
    expect(canStartFresh({ status: 'ok', path: '/new' }, '  /new  ')).toBe(true);
    expect(canStartFresh({ status: 'ok', path: '/new' }, '/other')).toBe(false);
    expect(
      canStartFresh({ status: 'refused', reason: 'inside_current', message: 'x' }, '/new'),
    ).toBe(false);
  });
});

describe('previewSummary', () => {
  it('counts games and sums the size', () => {
    expect(
      previewSummary({
        old_root: '/old',
        games: [
          { title: 'A', platform: 'SNES', paths: ['/old/games/SNES/A'], bytes: 1024 },
          { title: 'B', platform: 'SNES', paths: ['/old/games/SNES/B.sfc'], bytes: 512 },
        ],
        total_bytes: 1536,
        left_outside: [],
      }),
    ).toBe('2 games, 1.5 KB');
    expect(
      previewSummary({
        old_root: '/old',
        games: [{ title: 'A', platform: 'SNES', paths: [], bytes: 0 }],
        total_bytes: 0,
        left_outside: [],
      }),
    ).toBe('1 game, 0 B');
    expect(previewSummary({ old_root: '/old', games: [], total_bytes: 0, left_outside: [] })).toBe(
      'No games',
    );
  });
});

describe('outcomeNotice', () => {
  it('reports a switch, with failures as an error', () => {
    expect(
      outcomeNotice({ status: 'switched', library_path: '/new', rows_removed: 3, failures: [] }, false),
    ).toEqual({ kind: 'success', text: 'Library folder changed. 3 games left the library; their files stay on disk.' });
    expect(
      outcomeNotice({ status: 'switched', library_path: '/new', rows_removed: 1, failures: [] }, true),
    ).toEqual({ kind: 'success', text: 'Library folder changed. 1 game was deleted.' });
    expect(
      outcomeNotice({ status: 'switched', library_path: '/new', rows_removed: 0, failures: [] }, false),
    ).toEqual({ kind: 'success', text: 'Library folder changed.' });
    expect(
      outcomeNotice(
        { status: 'switched', library_path: '/new', rows_removed: 1, failures: ['a', 'b'] },
        true,
      ),
    ).toEqual({
      kind: 'error',
      text: 'Library folder changed. 1 game was deleted. 2 files or folders could not be removed; those games stay in the library.',
    });
  });

  it('passes a refusal or a stale root through', () => {
    expect(
      outcomeNotice({ status: 'refused', reason: 'game_running', message: 'Close the game.' }, false),
    ).toEqual({ kind: 'error', text: 'Close the game.' });
    expect(outcomeNotice({ status: 'stale', message: 'Changed.' }, true)).toEqual({
      kind: 'error',
      text: 'Changed.',
    });
  });
});

describe('move existing files', () => {
  const GB = 1024 ** 3;
  const preview = (overrides: Partial<MovePreview>): MovePreview => ({
    old_root: '/old',
    new_root: '/new',
    items: [
      { name: 'games', strategy: 'rename', bytes: 5 * GB, files: 10 },
      { name: 'saves', strategy: 'rename', bytes: 1024, files: 2 },
    ],
    total_bytes: 5 * GB + 1024,
    copy_bytes: 0,
    required_bytes: 0,
    free_bytes: 100 * GB,
    ...overrides,
  });

  it('is offered once the typed folder is accepted and a library exists', () => {
    const ok = { status: 'ok' as const, path: '/new' };
    expect(canMove(ok, '/new', '/old')).toBe(true);
    expect(canMove(ok, '/new', '  ')).toBe(false);
    expect(canMove(null, '/new', '/old')).toBe(false);
    expect(canMove(ok, '/other', '/old')).toBe(false);
  });

  it('summarizes a same-drive move as instant and a copy with its size and the free space', () => {
    expect(movePreviewSummary(preview({}))).toBe('Same drive — instant');
    expect(
      movePreviewSummary(
        preview({
          items: [{ name: 'games', strategy: 'copy', bytes: 5.6 * GB, files: 10 }],
          copy_bytes: 5.6 * GB,
          required_bytes: 6.6 * GB,
          free_bytes: 120 * GB,
        }),
      ),
    ).toBe('Copies 5.6 GB; 120.0 GB free');
    expect(
      movePreviewSummary(
        preview({
          items: [{ name: 'games', strategy: 'copy', bytes: GB, files: 1 }],
          copy_bytes: GB,
          free_bytes: null,
        }),
      ),
    ).toBe('Copies 1.0 GB; free space unknown');
    expect(movePreviewSummary(preview({ items: [], total_bytes: 0 }))).toBe('Nothing to move');
  });

  it('describes each folder', () => {
    expect(moveItemLine({ name: 'games', strategy: 'rename', bytes: 5 * GB, files: 10 })).toBe(
      'games/ · 5.0 GB · same drive',
    );
    expect(moveItemLine({ name: 'saves', strategy: 'copy', bytes: 1536, files: 2 })).toBe(
      'saves/ · 1.5 KB · copied and checked',
    );
  });

  const status = (overrides: Partial<LibraryMoveStatus>): LibraryMoveStatus => ({
    phase: 'copy',
    running: true,
    old_root: '/old',
    new_root: '/new',
    resumed: false,
    error: null,
    ...overrides,
  });

  it('gives the pane its moving state', () => {
    expect(paneMoveText(null)).toBeNull();
    expect(paneMoveText(status({ phase: null, running: false }))).toBeNull();
    expect(paneMoveText(status({}))).toBe('Moving to /new…');
    expect(paneMoveText(status({ phase: 'cleanup' }))).toBe('Moving to /new…');
    expect(paneMoveText(status({ running: false, error: 'Disk full.' }))).toBe(
      'The move to /new stopped: Disk full. Retry or cancel it in Downloads.',
    );
    expect(paneMoveText(status({ phase: 'commit', running: false, error: 'Disk full.' }))).toBe(
      'The move to /new stopped: Disk full. Retry it in Downloads.',
    );
    expect(paneMoveText(status({ running: false }))).toBe('The move to /new waits to resume.');
    expect(moveInProgress(null)).toBe(false);
    expect(moveInProgress(status({ phase: null }))).toBe(false);
    expect(moveInProgress(status({ running: false }))).toBe(true);
  });

  it('reports the start', () => {
    expect(moveStartNotice({ status: 'started', new_root: '/new' })).toEqual({
      kind: 'success',
      text: 'Moving the library to /new. Follow it in Downloads.',
    });
    expect(
      moveStartNotice({ status: 'refused', reason: 'not_enough_space', message: 'Not enough space.' }),
    ).toEqual({ kind: 'error', text: 'Not enough space.' });
  });

  it('turns a finished run into one toast', () => {
    expect(
      moveFinishedToast({ status: 'finished', new_root: '/new', report: [], old_root_removed: true }),
    ).toEqual({ level: 'success', text: 'Library moved to /new.' });
    expect(
      moveFinishedToast({
        status: 'finished',
        new_root: '/new',
        report: ['games changed during the move; left at /old/games.'],
        old_root_removed: false,
      }),
    ).toEqual({
      level: 'error',
      text: 'Library moved to /new. games changed during the move; left at /old/games.',
    });
    expect(moveFinishedToast({ status: 'cancelled' })).toEqual({
      level: 'success',
      text: 'Library move cancelled. Nothing changed.',
    });
    expect(moveFinishedToast({ status: 'failed', error: 'Disk full.' })).toEqual({
      level: 'error',
      text: 'Library move stopped: Disk full.',
    });
  });

  it('announces a resume once, only for a move that was resumed at startup', () => {
    expect(resumeToast(status({ resumed: true }))).toBe('Resuming the library move to /new');
    expect(resumeToast(status({ resumed: false }))).toBeNull();
    expect(resumeToast(status({ resumed: true, phase: null }))).toBeNull();
    expect(resumeToast(null)).toBeNull();
  });
});
