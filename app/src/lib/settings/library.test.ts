import { describe, expect, it } from 'vitest';
import {
  MOVE_AVAILABLE,
  canStartFresh,
  formatBytes,
  outcomeNotice,
  previewSummary,
} from './library';

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

describe('move existing files', () => {
  it('stays unavailable until phase 9b lands', () => {
    expect(MOVE_AVAILABLE).toBe(false);
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
