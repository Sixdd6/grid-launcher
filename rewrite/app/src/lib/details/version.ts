// Details version label (grid-launcher.py:3273-3297, doc 10 "Native game
// version detection"). A TS mirror of grid-core's update_detection tag
// rules — kept in sync by version.test.ts, which pins the same cases.

export type VersionTag = { kind: 'numeric'; value: number } | { kind: 'semver'; parts: number[] };

const NUMERIC = /\(v(\d{5})\)/i;
const SEMVER = /\(v(\d+(?:\.\d+)+)\)/i;

export function parseVersionTag(romFileName: string): VersionTag | null {
  const numeric = NUMERIC.exec(romFileName);
  if (numeric) return { kind: 'numeric', value: Number(numeric[1]) };
  const semver = SEMVER.exec(romFileName);
  if (!semver) return null;
  return { kind: 'semver', parts: semver[1].split('.').map(Number) };
}

export function formatVersionTag(tag: VersionTag): string {
  return tag.kind === 'numeric' ? `v${String(tag.value).padStart(5, '0')}` : `v${tag.parts.join('.')}`;
}

export function isWindowsPcPlatform(platform: string): boolean {
  const normalized = platform.trim().toLowerCase();
  return normalized !== '' && (normalized.includes('windows') || normalized === 'pc');
}

/**
 * `_details_version_label_text_for_game`: for a Windows/PC platform, the
 * first tag found in `romFileNames` (see `romFileNamesFor` for the order)
 * renders as `Version: v…`; otherwise the
 * trimmed `revision` verbatim (no prefix — Python parity); `''` hides the row.
 */
export function versionLabel(platform: string, romFileNames: string[], revision: string): string {
  if (isWindowsPcPlatform(platform)) {
    for (const name of romFileNames) {
      const tag = parseVersionTag(name);
      if (tag) return `Version: ${formatVersionTag(tag)}`;
    }
  }
  return revision.trim();
}

/**
 * The order `versionLabel` reads the two candidate file names in, following
 * the subject's source: Python's `_default_details_version_label_text`
 * (game_views.py:259-268) reads the details game first and only then the
 * installed record, so a Library-opened game names the version it HAS rather
 * than the newer one waiting on the server.
 */
export function romFileNamesFor(
  source: 'server' | 'installed',
  installedName: string,
  serverName: string
): string[] {
  return source === 'installed' ? [installedName, serverName] : [serverName, installedName];
}

/**
 * The `YYYY-MM-DD` head of an ISO 8601 timestamp, or `''` when the value is
 * not one. Sliced rather than parsed through `Date`: the server sends the
 * file's own stated timestamp and D-UI-10 shows the date it states, not
 * that instant re-rendered in the viewer's time zone.
 */
export function isoDate(value: string): string {
  const match = /^(\d{4}-\d{2}-\d{2})/.exec(value.trim());
  return match ? match[1] : '';
}

/**
 * D-UI-10: one file's version — "the parsed version tag when the file name
 * carries one, else the file's `last_modified` date". Unlike
 * [`versionLabel`], which is the header's whole-game row and is
 * platform-gated, this is per file and applies on every platform: the Files
 * tab states what each file IS, and a tagged file name is as informative on
 * a PS2 rom as on a PC one. `''` when the server offers neither.
 */
export function fileVersionLabel(fileName: string, lastModified: string): string {
  const tag = parseVersionTag(fileName);
  if (tag) return formatVersionTag(tag);
  return isoDate(lastModified);
}

/**
 * Whether the Files tab shows D-UI-10's installed-vs-server comparison line.
 *
 * The two sides are not the same quantity: the installed side falls back to
 * the date the install landed, the server side to the date the server file
 * was last modified. On a console rom, where neither file name carries a
 * version tag, that reads as "Installed 2026-09-04 · Server 2026-01-02" —
 * a comparison the user cannot act on and which suggests the local copy is
 * newer. D-UI-10 scopes the version rule to PC games, so the line follows
 * the same platform gate `versionLabel` uses. Per-file rows are not gated:
 * see [`fileVersionLabel`].
 */
export function showsFilesVersionLine(platform: string): boolean {
  return isWindowsPcPlatform(platform);
}
