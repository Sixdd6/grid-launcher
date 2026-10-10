import type { GameSummary } from '../api';
import { titleContains } from '../library/sort';

/** The Server view's search box: one query matches a game's title, its
 *  platform label OR any of its genres, case-insensitively, as a substring
 *  (the previous app's `filter_server_games` rule). A blank query returns
 *  every game. Reuses `titleContains` so the fold is the Library's. */
export function filterServerGames(games: GameSummary[], query: string): GameSummary[] {
  if (query.trim() === '') return games;
  return games.filter(
    (game) =>
      titleContains(game.name, query) ||
      titleContains(game.platform_display_name ?? '', query) ||
      (game.genres ?? []).some((genre) => titleContains(genre, query)),
  );
}
