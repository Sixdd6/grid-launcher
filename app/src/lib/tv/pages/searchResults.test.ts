import { describe, expect, it } from 'vitest';
import { summary } from '../fixtures.test-util';
import { NO_RESULTS, resultsHeading, searchGames, searchPageTitle } from './searchResults';

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
});
