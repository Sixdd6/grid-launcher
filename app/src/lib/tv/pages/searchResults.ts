// Search everything's results (item 355). Pure.
import type { LabelledGame } from '../tabs/server/server.svelte';
import { matchesTitle } from '../tabs/server/serverHeader';

/** "1 result for “x”", "3 results for “x”". */
export function resultsHeading(n: number, query: string): string {
  return `${n} result${n === 1 ? '' : 's'} for “${query}”`;
}

/** The keyboard's title (item 353). */
export const SEARCH_ALL_TITLE = 'Search everything';

/** The results page's breadcrumb. */
export function searchPageTitle(query: string): string {
  return `Search: ${query}`;
}

export const NO_RESULTS = 'Nothing matches. Try fewer letters.';

/** The heading while every system is being asked (consultant ruling). */
export function searchingHeading(query: string): string {
  return `Searching for “${query}”…`;
}

/** Nothing could be searched: the offline panel's words (consultant ruling). */
export const LOAD_FAILED_TITLE = "Can't reach your server";

export function loadFailedLine(url: string): string {
  return `${url} isn't answering, so nothing could be searched.`;
}

/** Some systems failed: the results that arrived show under this line. */
export const PARTIAL_NOTE = "Some systems didn't answer, so their games aren't in these results.";

export const RETRY = 'Try again';

/** The server games whose titles contain `query`, by title. */
export function searchGames(all: readonly LabelledGame[], query: string): LabelledGame[] {
  if (query.trim() === '') return [];
  return all
    .filter((g) => matchesTitle(g.game.name, query))
    .sort((a, b) => a.game.name.localeCompare(b.game.name, undefined, { sensitivity: 'base' }) || a.platform.localeCompare(b.platform));
}
