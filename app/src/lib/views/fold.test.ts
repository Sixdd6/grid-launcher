import { describe, expect, it } from 'vitest';
import {
  FOLD_AT,
  UNFOLD_AT,
  foldedClearance,
  gameCountLine,
  librarySearchPlaceholder,
  nextFolded,
  serverSearchPlaceholder,
  unfoldedHeight,
} from './fold';

describe('nextFolded', () => {
  it('folds only past 32px', () => {
    expect(nextFolded(false, 0)).toBe(false);
    expect(nextFolded(false, FOLD_AT)).toBe(false);
    expect(nextFolded(false, FOLD_AT + 1)).toBe(true);
  });

  it('unfolds only back within 8px of the top', () => {
    expect(nextFolded(true, FOLD_AT)).toBe(true);
    expect(nextFolded(true, UNFOLD_AT)).toBe(true);
    expect(nextFolded(true, UNFOLD_AT - 1)).toBe(false);
    expect(nextFolded(true, 0)).toBe(false);
  });

  it('holds its state between the two thresholds', () => {
    expect(nextFolded(false, 20)).toBe(false);
    expect(nextFolded(true, 20)).toBe(true);
  });
});

describe('header heights', () => {
  it('reserves the title row, gap, toolbar, padding and border', () => {
    expect(unfoldedHeight(30, 32)).toBe(30 + 12 + 32 + 28 + 2);
  });

  it('keeps a card clear of the folded bar', () => {
    expect(foldedClearance(32)).toBe(32 + 16 + 2 + 8);
  });
});

describe('serverSearchPlaceholder', () => {
  it('keeps the general hint while unfolded', () => {
    expect(serverSearchPlaceholder(false, 'PlayStation')).toBe('Search title, platform or genre');
  });

  it('names the platform while folded', () => {
    expect(serverSearchPlaceholder(true, 'PlayStation')).toBe('Search PlayStation…');
  });

  it('falls back to the general hint with no platform yet', () => {
    expect(serverSearchPlaceholder(true, '  ')).toBe('Search title, platform or genre');
  });
});

describe('librarySearchPlaceholder', () => {
  it('keeps the general hint while unfolded', () => {
    expect(librarySearchPlaceholder(false, 'recent', 'Recent')).toBe('Search installed games');
  });

  it('names the rail entry while folded', () => {
    expect(librarySearchPlaceholder(true, 'all', 'All games')).toBe('Search your library…');
    expect(librarySearchPlaceholder(true, 'recent', 'Recent')).toBe('Search recent games…');
    expect(librarySearchPlaceholder(true, 'updates', 'Updates')).toBe('Search games with updates…');
    expect(librarySearchPlaceholder(true, 'platform:sega-saturn', 'Sega Saturn')).toBe('Search Sega Saturn…');
  });

  it('searches a backup drive\'s games, by pack name once folded', () => {
    expect(librarySearchPlaceholder(false, 'drive:/m/T7', 'Couch kit')).toBe('Search games on this drive');
    expect(librarySearchPlaceholder(true, 'drive:/m/T7', 'Couch kit')).toBe('Search Couch kit…');
  });
});

describe('gameCountLine', () => {
  it('counts games', () => {
    expect(gameCountLine(0)).toBe('0 games');
    expect(gameCountLine(1)).toBe('1 game');
    expect(gameCountLine(12)).toBe('12 games');
  });
});
