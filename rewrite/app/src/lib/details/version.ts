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
 * first tag found in `romFileNames` (server fs_name first, then the
 * installed row's rom_file_name) renders as `Version: v…`; otherwise the
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
