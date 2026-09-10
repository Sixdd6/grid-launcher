// Design §7 Overview: the Related row is "filtered to titles present on the
// server". The filter is client-side against the platform's already-loaded
// game list — RomM's search endpoint is out of scope (design §13) — so this
// module owns the title match and nothing else.
import type { RelatedGame } from '../api';

/**
 * A title reduced to what two sources can be compared on: case-folded,
 * whitespace-collapsed, and without the trailing `(USA)`/`(Rev 1)` style
 * parenthetical that server titles derived from file names carry. A
 * parenthetical in the MIDDLE of a title is left alone — it is part of the
 * name there, not a tag.
 */
export function normalizeTitle(title: string): string {
  return title
    .replace(/\s*\([^()]*\)\s*$/g, '')
    .trim()
    .toLowerCase()
    .replace(/\s+/g, ' ');
}

export const RELATED_KIND_LABELS: Record<string, string> = {
  similar: 'Similar',
  remake: 'Remake',
  remaster: 'Remaster',
  dlc: 'DLC',
  expansion: 'Expansion',
};

/** The chip's kind label; a kind a newer backend adds still renders. */
export function relatedKindLabel(kind: string): string {
  return RELATED_KIND_LABELS[kind] ?? 'Related';
}

/**
 * `related` filtered to the entries whose title appears in `serverTitles`,
 * in backend order, with duplicates (IGDB repeats a title across its lists,
 * and normalization can collide two spellings) reduced to the first hit.
 * An empty `serverTitles` yields an empty row: before the platform list
 * loads, the honest answer is "nothing to show", not "everything".
 */
export function relatedOnServer(
  related: RelatedGame[],
  serverTitles: Iterable<string>
): RelatedGame[] {
  const present = new Set<string>();
  for (const title of serverTitles) present.add(normalizeTitle(title));
  const seen = new Set<string>();
  const out: RelatedGame[] = [];
  for (const entry of related) {
    const key = normalizeTitle(entry.name);
    if (key === '' || !present.has(key) || seen.has(key)) continue;
    seen.add(key);
    out.push(entry);
  }
  return out;
}
