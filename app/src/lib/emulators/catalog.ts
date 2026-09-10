// Pure helpers for the Emulators panel's catalog Install tab and the manual
// tab's name-based auto-fill (task-7-brief.md). No store/API imports here so
// this stays trivially unit-testable.
import type { CatalogEntry, ProfileSummary } from '../api';

/**
 * Manual-add auto-fill matcher: trimmed, casefolded `name` against every
 * visible profile's name. An exact match wins outright over any substring
 * match; otherwise a name that is a substring of exactly one profile's name
 * matches that profile; a blank name, or a name that is a substring of more
 * than one profile's name, matches nothing.
 */
export function matchProfileByName(
  name: string,
  profiles: ProfileSummary[]
): ProfileSummary | null {
  const needle = name.trim().toLowerCase();
  if (!needle) return null;

  const exact = profiles.find((p) => p.name.toLowerCase() === needle);
  if (exact) return exact;

  const substringMatches = profiles.filter((p) => p.name.toLowerCase().includes(needle));
  return substringMatches.length === 1 ? substringMatches[0] : null;
}

/**
 * Whether the manual tab's name-based auto-fill may run for the form as it
 * stands. Add mode only — the flow is scoped to the Add form's Manual tab,
 * so renaming an existing entry whose path and args happen to be blank must
 * not rewrite its args — and only while path and args are both still empty,
 * so it never clobbers a typed or path-derived value.
 */
export function shouldAutoFillFromName(
  mode: 'add' | 'edit' | null,
  path: string,
  args: string
): boolean {
  return mode === 'add' && path.trim() === '' && args.trim() === '';
}

/**
 * Install tab search: AND-of-whitespace-tokens over casefolded name +
 * source_id (the reference's filter semantics, `filter_source_download_emulator_entries`,
 * ui/emulators.py:235-292, narrowed to the two fields the catalog row shows).
 * A blank query matches everything.
 */
export function filterCatalogEntries(query: string, entries: CatalogEntry[]): CatalogEntry[] {
  const tokens = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (tokens.length === 0) return entries;

  return entries.filter((entry) => {
    const searchable = `${entry.name} ${entry.source_id}`.toLowerCase();
    return tokens.every((token) => searchable.includes(token));
  });
}
