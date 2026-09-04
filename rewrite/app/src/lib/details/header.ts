// The details popup's right-hand header (design §7): title, platform, first
// release date, developer, genres, rating, region/language flags and the
// verification state. Pure — the `.svelte` shell only renders what these
// return.

/**
 * The four-digit year of `first_release_date`. The backend sends IGDB's
 * epoch SECONDS as a string (`romm/mod.rs`'s `into_detail`), so the year is
 * read in UTC: the value is a release date, not a local timestamp, and
 * rendering it in the viewer's zone would move it a day either way.
 */
export function releaseYear(firstReleaseDate: string): string {
  const trimmed = firstReleaseDate.trim();
  if (trimmed === '') return '';
  const epoch = Number(trimmed);
  if (!Number.isFinite(epoch)) return '';
  return String(new Date(epoch * 1000).getUTCFullYear());
}

/**
 * The developer: the first entry of the comma-joined `companies` field.
 * RomM does not separate developer from publisher in `metadatum.companies`
 * — it lists the developer first — so the header names the first and the
 * Overview metadata grid lists all of them.
 */
export function developerOf(companies: string): string {
  return companies.split(',')[0]?.trim() ?? '';
}

/** The header's rating chip, or `''` when the server has no rating. */
export function ratingText(rating: string): string {
  const trimmed = rating.trim();
  return trimmed === '' ? '' : `★ ${trimmed}`;
}

/** Splits a comma-joined backend field (`regions`, `languages`) into flags. */
export function flagList(value: string): string[] {
  return value
    .split(',')
    .map((part) => part.trim())
    .filter((part) => part.length > 0);
}

/** RomM's `is_identified`, in words. */
export function verificationLabel(isIdentified: boolean): string {
  return isIdentified ? 'Identified' : 'Unidentified';
}

export type HeaderInput = {
  platformName: string;
  firstReleaseDate: string;
  companies: string;
  genres: string;
  rating: string;
};

/**
 * The one line under the title. Every part the server has nothing for is
 * dropped, so the separator never dangles on a sparse rom.
 */
export function headerLine(input: HeaderInput): string {
  return [
    input.platformName.trim(),
    releaseYear(input.firstReleaseDate),
    developerOf(input.companies),
    input.genres.trim(),
    ratingText(input.rating),
  ]
    .filter((part) => part !== '')
    .join(' · ');
}
