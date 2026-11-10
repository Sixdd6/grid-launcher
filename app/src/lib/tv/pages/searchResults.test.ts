import { describe, expect, it } from 'vitest';
import { summary } from '../fixtures.test-util';
import { LOAD_FAILED_TITLE, loadFailedLine, NO_RESULTS, PARTIAL_NOTE, RETRY, resultsHeading, searchGames, searchingHeading, searchPageTitle } from './searchResults';

describe('Search everything (items 353–355)', () => {
  it('the heading, the breadcrumb and the empty text', () => {
    expect(resultsHeading(1, 'star')).toBe('1 result for “star”');
    expect(resultsHeading(4, 'star')).toBe('4 results for “star”');
    expect(searchPageTitle('star')).toBe('Search: star');
    expect(NO_RESULTS).toBe('Nothing matches. Try fewer letters.');
  });

  it('titles containing the text, across platforms, by title', () => {
    const all = [
      { game: summary({ id: 2, name: 'Starlit Run' }), platform: 'SNES', slug: 'snes' },
      { game: summary({ id: 3, name: 'Arcade Star' }), platform: 'Arcade', slug: 'arcade' },
      { game: summary({ id: 4, name: 'Neon Drift' }), platform: 'SNES', slug: 'snes' },
    ];
    expect(searchGames(all, 'STAR').map((g) => g.game.id)).toEqual([3, 2]);
    expect(searchGames(all, '  ')).toEqual([]);
  });

  it('the loading, failed and partial states (consultant ruling)', () => {
    expect(searchingHeading('star')).toBe('Searching for “star”…');
    expect(LOAD_FAILED_TITLE).toBe("Can't reach your server");
    expect(loadFailedLine('https://romm.lan')).toBe("https://romm.lan isn't answering, so nothing could be searched.");
    expect(PARTIAL_NOTE).toBe("Some systems didn't answer, so their games aren't in these results.");
    expect(RETRY).toBe('Try again');
  });
});
