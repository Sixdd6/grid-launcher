import { describe, expect, it } from 'vitest';
import type { GameSummary } from '../api';
import { filterServerGames } from './search';

function game(over: Partial<GameSummary> & { id: number; name: string }): GameSummary {
  return {
    platform_id: 1,
    path_cover_small: null,
    path_cover_large: null,
    screenshot_urls: [],
    fanart_urls: [],
    platform_display_name: 'Super Nintendo',
    genres: [],
    ...over,
  };
}

const mario = game({ id: 1, name: 'Super Mario World', genres: ['Platform', 'Adventure'] });
const zelda = game({
  id: 2,
  name: 'The Legend of Zelda',
  platform_display_name: 'Nintendo Entertainment System',
  genres: ['Action-Adventure'],
});
const bare = game({ id: 3, name: 'Mystery Game', platform_display_name: 'Sega Saturn' });
const noGenres = { ...bare, genres: undefined } as unknown as GameSummary;
const all = [mario, zelda, bare];

const ids = (list: GameSummary[]) => list.map((g) => g.id);

describe('filterServerGames', () => {
  it.each([
    ['title hit', 'zelda', [2]],
    ['genre hit, first genre', 'platform', [1]],
    ['genre hit, any genre', 'adventure', [1, 2]],
    ['platform hit', 'saturn', [3]],
    ['platform hit on a shared label', 'nintendo', [1, 2]],
    ['title, platform and genre at once', 'super', [1]],
    ['case-insensitive', 'ZELDA', [2]],
    ['case-insensitive genre', 'PLATFORM', [1]],
    ['surrounding whitespace is trimmed', '  saturn  ', [3]],
    ['no hit', 'zzz-nothing', []],
  ])('%s', (_label, query, expected) => {
    expect(ids(filterServerGames(all, query))).toEqual(expected);
  });

  it.each([[''], ['   ']])('a blank query (%j) returns every game', (query) => {
    expect(filterServerGames(all, query)).toEqual(all);
  });

  it('a game with no genres list still matches by title and platform', () => {
    expect(ids(filterServerGames([noGenres], 'mystery'))).toEqual([3]);
    expect(ids(filterServerGames([noGenres], 'sega'))).toEqual([3]);
    expect(filterServerGames([noGenres], 'action')).toEqual([]);
  });

  it('a game with no platform label still matches by title', () => {
    const unlabeled = { ...mario, platform_display_name: undefined } as unknown as GameSummary;
    expect(ids(filterServerGames([unlabeled], 'mario'))).toEqual([1]);
  });
});
