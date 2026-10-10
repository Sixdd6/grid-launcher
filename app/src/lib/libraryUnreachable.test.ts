import { describe, expect, it } from 'vitest';
import {
  RELOCATE_PICKER_TITLE,
  isBlocking,
  relocateError,
  relocateNeedsPicker,
  relocateSuccessText,
  unreachableCopy,
} from './libraryUnreachable';
import type { LibraryUnreachableStatus } from './api';

const unreachable: LibraryUnreachableStatus = {
  status: 'unreachable',
  root: 'E:\\Games',
  reason: 'drive_missing',
  message: 'The drive that holds the library folder is not connected.',
};

describe('isBlocking', () => {
  it('blocks only an unreachable library', () => {
    expect(isBlocking(unreachable)).toBe(true);
    expect(isBlocking({ status: 'reachable' })).toBe(false);
    // Not read yet: never block on a guess.
    expect(isBlocking(null)).toBe(false);
  });
});

describe('unreachableCopy', () => {
  it('names the folder, says why, and offers exactly the two ways out', () => {
    expect(unreachableCopy(unreachable)).toEqual({
      title: 'GRID cannot reach your library',
      reason: 'The drive that holds the library folder is not connected.',
      root: 'E:\\Games',
      body: 'If you moved the library, relocate it to the folder where it is now: GRID updates its stored paths and moves no files. Otherwise connect the drive and start GRID again, or close GRID without changing anything.',
    });
  });

  it('has nothing to say for a reachable library', () => {
    expect(unreachableCopy({ status: 'reachable' })).toBeNull();
    expect(unreachableCopy(null)).toBeNull();
  });
});

describe('relocate', () => {
  it('opens the folder picker only when no folder was typed', () => {
    expect(relocateNeedsPicker('')).toBe(true);
    expect(relocateNeedsPicker('   ')).toBe(true);
    expect(relocateNeedsPicker('D:\\Games')).toBe(false);
    expect(RELOCATE_PICKER_TITLE).toBe('Select the folder where your library is now');
  });

  it('passes a refusal through and words a success', () => {
    expect(
      relocateError({
        status: 'refused',
        reason: 'no_library_folders',
        message: 'This folder has no games, emulators or saves folder.',
      }),
    ).toBe('This folder has no games, emulators or saves folder.');
    expect(relocateError({ status: 'relocated', library_path: 'D:\\Games', report: [] })).toBeNull();
    expect(relocateSuccessText({ status: 'relocated', library_path: 'D:\\Games', report: [] })).toBe(
      'Library relocated to D:\\Games.',
    );
    expect(
      relocateSuccessText({
        status: 'relocated',
        library_path: 'D:\\Games',
        report: ['a', 'b'],
      }),
    ).toBe('Library relocated to D:\\Games. 2 paths could not be updated; see the log.');
    expect(
      relocateSuccessText({ status: 'refused', reason: 'blank', message: 'Choose a folder.' }),
    ).toBeNull();
  });
});
