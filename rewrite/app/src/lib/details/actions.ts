// Pure helpers for the Details "install specials" surface (task-16-brief.md):
// platform predicates, the primary button's label, which content buttons to
// show, and native-executable-candidate display. No API/store imports here
// so this stays trivially unit-testable — Details.svelte and
// NativeSettings.svelte own the fetching/wiring.

/**
 * `is_native_platform` (grid-core's launch/native.rs, ported from
 * `grid_launcher/emulator/selection.py:11-52`) — the same rule Details
 * already uses for cloud saves (`isNativeExecutablePlatform`,
 * details/cloud.ts): trimmed, case-folded platform string starting with
 * "windows". This is the SERVER's platform name, not the host OS the app
 * itself runs on — see `isWindowsHost` below for that.
 */
export function isNativePlatform(platform: string): boolean {
  return platform.trim().toLowerCase().startsWith('windows');
}

/**
 * Splits `platform` into its normalized (lowercased, non-alphanumeric runs
 * collapsed to a single space, trimmed) form, that form's space-free
 * "compact" run, and the normalized form's whitespace-separated tokens.
 * Ported from `normalized_tokens` (grid-core/src/library/platforms.rs).
 */
function normalizedTokens(platform: string): { normalized: string; compact: string; tokens: string[] } {
  const normalized = platform
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, ' ')
    .trim();
  const compact = normalized.replace(/ /g, '');
  const tokens = normalized.length > 0 ? normalized.split(/\s+/) : [];
  return { normalized, compact, tokens };
}

/**
 * `is_ps4_platform` (grid-core/src/library/platforms.rs): "playstation 4" /
 * "ps4" (whole normalized string or a whole token), or a compact
 * "playstation4" run.
 */
function isPs4Platform(platform: string): boolean {
  const { normalized, compact, tokens } = normalizedTokens(platform);
  if (normalized === '') return false;
  if (normalized === 'playstation 4' || normalized === 'ps4') return true;
  if (tokens.includes('ps4')) return true;
  return compact.includes('playstation4');
}

/**
 * `is_xbox360_platform` (grid-core/src/library/platforms.rs): must mention
 * "xbox" (as a token or within a compact "xbox360" run) and separately carry
 * a "360" marker.
 */
function isXbox360Platform(platform: string): boolean {
  const { normalized, compact, tokens } = normalizedTokens(platform);
  if (normalized === '') return false;
  const hasXbox = tokens.includes('xbox') || compact.includes('xbox360');
  if (!hasXbox) return false;
  return compact.includes('xbox360') || tokens.includes('360');
}

/**
 * Whether `platform` is one of the two "extra content" platforms (PS4
 * update/DLC files, Xbox 360 STFS content) — ported from `is_ps4_platform`
 * / `is_xbox360_platform` (grid-core/src/library/platforms.rs).
 */
export function isContentPlatform(platform: string): boolean {
  return isPs4Platform(platform) || isXbox360Platform(platform);
}

/**
 * The primary install button's label (`details-install`, testid unchanged):
 * a native (Windows) game installs a `game.json` + prefix rather than a
 * plain archive, so the label reads "Install App"; every other platform
 * keeps the existing "Install".
 */
export function installLabel(platform: string): 'Install App' | 'Install' {
  return isNativePlatform(platform) ? 'Install App' : 'Install';
}

export type ContentButtons = { update: boolean; dlc: boolean };

/**
 * Which of the Install Update / Install DLC buttons Details should render.
 * `busy` is true while a live download-drawer entry exists for this rom —
 * after clicking either button the drawer grows a `ps4_content` /
 * `xbox360_content` row, so both buttons hide until that entry clears.
 * `avail` is `null` before the availability fetch resolves, or if it
 * failed — Details renders no buttons either way rather than guessing.
 */
export function contentButtons(
  avail: { update: boolean; dlc: boolean } | null,
  installed: boolean,
  busy: boolean
): ContentButtons {
  if (!installed || busy || avail === null) return { update: false, dlc: false };
  return { update: avail.update, dlc: avail.dlc };
}

function dirnameOf(path: string): string {
  const normalized = path.replace(/\\/g, '/');
  const idx = normalized.lastIndexOf('/');
  return idx === -1 ? '' : normalized.slice(0, idx);
}

/**
 * A display-only stand-in for the native install directory:
 * `NativeGameSettings.candidates` are sorted shallowest-first by the backend
 * (grid-core's `executable_candidates`), so the first entry's own directory
 * is the install root every other candidate nests under.
 */
export function installDirOf(candidates: string[]): string {
  return candidates.length > 0 ? dirnameOf(candidates[0]) : '';
}

/**
 * The executable select's option label for a candidate path: the portion of
 * `candidate` under `installDir` when it is actually inside it (so a nested
 * candidate reads as a short relative path instead of a full absolute one);
 * the full path unchanged otherwise. Separators are normalized to `/` only
 * for the containment check and the relative slice — an out-of-`installDir`
 * candidate is returned exactly as given.
 */
export function candidateLabel(candidate: string, installDir: string): string {
  if (!installDir) return candidate;
  const dir = installDir.replace(/\\/g, '/').replace(/\/+$/, '');
  const cand = candidate.replace(/\\/g, '/');
  if (cand === dir) return candidate;
  if (cand.startsWith(`${dir}/`)) return cand.slice(dir.length + 1);
  return candidate;
}

/**
 * Whether the app itself is running on a Windows host, per Tauri/browser
 * `navigator.platform` strings (e.g. `"Win32"`) — distinct from
 * `isNativePlatform`, which reads the SERVER's platform name. Pure so
 * NativeSettings.svelte can pass `navigator.platform` through a testable
 * seam instead of branching on the global directly.
 */
export function isWindowsHost(platform: string): boolean {
  return platform.toLowerCase().startsWith('win');
}
