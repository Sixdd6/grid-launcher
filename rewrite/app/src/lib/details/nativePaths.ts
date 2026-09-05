// Pure strings and enablement rules for the details cloud panel's native
// (PC game) save-location section. No API/store imports so this stays
// trivially unit-testable — CloudPanel.svelte owns the fetching.
//
// Every string here is ruled VERBATIM from `_refresh_native_save_panel`
// (grid_launcher/ui/mixins/details_view_mixin.py:1143-1185), including the
// "(s)" plural form and the ellipsis character, which the Python original
// spells as U+2026 in the two lookup messages.

/** Whether the PCGamingWiki lookup for this game has answered yet. */
export type NativePathsPhase = 'loading' | 'loaded';

/**
 * The section's status line (`details_cloud_status_label`, :1160/:1174/:1178).
 * `count` is PCGW rows plus manual rows, after de-duplication.
 */
export function nativePathsStatusLine(phase: NativePathsPhase, count: number): string {
  if (phase === 'loading') return 'Looking up save locations on PCGamingWiki…';
  if (count <= 0) return 'No save locations found on PCGamingWiki.';
  return `${count} save location(s) configured.`;
}

/**
 * The placeholder shown where the list will be (`details_cloud_empty_label`,
 * :1163). `''` once loaded: the list — or the status line's own
 * "No save locations found on PCGamingWiki." — says it instead.
 */
export function nativePathsEmptyLabel(phase: NativePathsPhase): string {
  return phase === 'loading' ? 'Fetching save locations from PCGamingWiki…' : '';
}

/**
 * The upload button's tooltip (:1162, :1176, :1181-1183). Order matters:
 * the lookup, then "no paths", then the missing rom id, then the happy path.
 */
export function nativeUploadTooltip(
  phase: NativePathsPhase,
  count: number,
  hasRomId: boolean
): string {
  if (phase === 'loading') return 'Waiting for save location lookup…';
  if (count <= 0) return 'Add a save location to enable uploads.';
  return hasRomId ? 'Upload save files from the listed locations.' : 'Missing ROM id for this game.';
}

/** The upload button's enablement, the same four gates as the tooltip (:1161-1180). */
export function nativeUploadEnabled(
  phase: NativePathsPhase,
  count: number,
  hasRomId: boolean,
  pending: boolean
): boolean {
  return phase === 'loaded' && count > 0 && hasRomId && !pending;
}
