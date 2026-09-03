// Pure helpers for the Details cloud panel (task-19-brief.md). No API/store
// imports here so this stays trivially unit-testable — Details.svelte and
// CloudPanel.svelte own the fetching/wiring.
//
// Text ruled verbatim from the Python original (details_view_mixin.py,
// cloud_mixin.py — see docs/porting/06-cloud-saves.md "Manual actions" /
// "Save scope"): row title fallback, row summary composition, the
// restore/delete confirmation copy, and the shared-scope Warning paragraph.
// `block_reason` and per-row `restore_tooltip` are NOT reproduced here —
// those strings come from the backend verbatim and must be rendered as-is.
import type { CloudRecord, GameSummary, InstalledGame, SaveScope, SaveType } from '../api';

export type CloudMode = 'overview' | 'save' | 'state';

/**
 * `_toggle_details_cloud_mode` (details_view_mixin.py:566): selecting the
 * mode that is already active returns to the overview; selecting the other
 * mode switches straight to it.
 */
export function toggleCloudMode(current: CloudMode, requested: 'save' | 'state'): CloudMode {
  return current === requested ? 'overview' : requested;
}

/**
 * `_details_cloud_button_text` (cloud_mixin.py:246): states are always
 * "Manage States"; saves are "Emulator Saves" for either shared scope, else
 * "Manage Saves".
 */
export function cloudButtonLabel(saveType: SaveType, scope: SaveScope): string {
  if (saveType === 'state') return 'Manage States';
  return scope === 'per_game' ? 'Manage Saves' : 'Emulator Saves';
}

/**
 * `_refresh_details_cloud_panel` (details_view_mixin.py:1023-1027): the
 * upload button reads "Upload Emulator Saves" only when the panel's own
 * title is "Emulator Saves"; otherwise "Upload Latest Save"/"Upload Latest
 * State".
 */
export function uploadButtonLabel(saveType: SaveType, panelLabel: string): string {
  if (saveType === 'save' && panelLabel === 'Emulator Saves') return 'Upload Emulator Saves';
  return saveType === 'save' ? 'Upload Latest Save' : 'Upload Latest State';
}

/** `_details_cloud_record_title` (details_view_mixin.py:614). */
export function cloudRecordTitle(record: CloudRecord, saveType: SaveType): string {
  const fileName = record.file_name.trim();
  if (fileName) return fileName;
  return `Cloud ${saveType === 'save' ? 'Save' : 'State'} #${record.id}`;
}

/**
 * `_make_details_cloud_record_widget` (details_view_mixin.py:723-734):
 * `emulator • size_text`, plus ` • Slot <slot>` only for a save record with
 * a non-blank slot.
 */
export function cloudRecordSummary(record: CloudRecord, saveType: SaveType): string {
  const emulator = record.emulator.trim() || 'Unknown emulator';
  const parts = [emulator, record.size_text];
  const slot = (record.slot ?? '').trim();
  if (saveType === 'save' && slot) parts.push(`Slot ${slot}`);
  return parts.join(' • ');
}

/**
 * The status line Python writes under the record list
 * (`details_view_mixin.py:889`): `Showing {n} cloud {saves|states}.`
 *
 * Pluralization is Python's — the kind label is a fixed plural word
 * ("saves"/"states"), so a single record still reads "Showing 1 cloud
 * saves." Reproduced verbatim rather than corrected.
 *
 * Returns `''` when there are no records: Python renders the empty-state
 * text there instead, which `CloudPanel` already has its own branch for.
 */
export function recordsStatusLine(count: number, saveType: SaveType): string {
  if (count <= 0) return '';
  return `Showing ${count} cloud ${saveType === 'save' ? 'saves' : 'states'}.`;
}

/** `_details_cloud_uploaded_text` composed into its row line (:738-739). */
export function uploadedLine(record: CloudRecord): string {
  return `Uploaded ${record.absolute_time} (${record.relative_time})`;
}

/**
 * `is_native_executable_platform` (selection.py:145-150 /
 * grid-core cloud::scope::is_native_executable_platform): trimmed,
 * case-folded platform string starting with "windows".
 */
export function isNativeExecutablePlatform(platform: string): boolean {
  return platform.trim().toLowerCase().startsWith('windows');
}

/**
 * Builds an `InstalledGame`-shaped object for a game that has no registry
 * row (e.g. a shared-scope entry on the synthetic `Emulators` platform that
 * was never installed through GRID). The cloud commands only read
 * title/platform/rom_id/rom_file_name/archive_path/extracted_path/
 * description (api.ts's own comment on `CloudPanelInfo`) and resolve
 * "installed" themselves by identity match, so the remaining fields are
 * harmless placeholders.
 */
export function syntheticCloudGame(game: GameSummary, platformName: string): InstalledGame {
  return {
    title: game.name,
    platform: platformName,
    rom_id: game.id,
    rom_file_name: '',
    archive_path: '',
    extracted_path: '',
    extracted_dir: '',
    multi_file_game_dir: '',
    description: '',
    rating: '',
    genres: '',
    regions: '',
    languages: '',
    tags: '',
    revision: '',
    companies: '',
    first_release_date: '',
    filesize_bytes: 0,
    server_updated_at: '',
    installed_at: 0,
    cover_small_path: '',
    cover_large_path: '',
    screenshot_urls: '',
    native_executable_path: '',
    native_launch_parameters: '',
    native_compat_tool: '',
    native_wineprefix: '',
    native_game_dir: '',
    included_dlc: '',
    ps3_trophy_paths: '',
    ps3_game_id: '',
    ps3_iso_path: '',
    ps4_game_id: '',
    ps4_content: '',
    ra_id: '',
  };
}

/**
 * `_details_cloud_scope_notice` (cloud_mixin.py:255-293), text ported
 * verbatim. Only meaningful for saves — states never carry a shared scope.
 */
export function sharedScopeWarning(scope: SaveScope, emulatorName: string): string {
  const label = emulatorName.trim() || 'this emulator';
  if (scope === 'shared_single') {
    return (
      `These cloud saves are shared ${label} media. Restoring or deleting one affects every game ` +
      'using this emulator.'
    );
  }
  if (scope === 'shared_slotted') {
    return (
      `These cloud saves are shared ${label} memory-card backups. Deleting one removes the ` +
      'backup for every game using that emulator slot.'
    );
  }
  return '';
}

export type ConfirmText = { title: string; message: string };

function withWarning(message: string, warning: string): string {
  return warning ? `${message}\n\nWarning: ${warning}` : message;
}

/**
 * `_confirm_restore_details_cloud_record` (details_view_mixin.py:1244-1263),
 * copy ported verbatim. `warning` is the shared-scope notice, or `''` when
 * none applies — pass the record's own `restore_tooltip` when `restorable`.
 */
export function restoreConfirmText(saveType: SaveType, gameTitle: string, warning: string): ConfirmText {
  const kind = saveType === 'save' ? 'save' : 'state';
  const message = withWarning(
    `Restore the selected cloud ${kind} for '${gameTitle}' and overwrite the local ${kind} data?`,
    warning
  );
  return { title: `Restore Cloud ${kind === 'save' ? 'Save' : 'State'}`, message };
}

/**
 * `_confirm_delete_details_cloud_record` (details_view_mixin.py:1270-1289),
 * copy ported verbatim. `recordTitle` is `cloudRecordTitle`'s result;
 * `warning` is `sharedScopeWarning` for the panel's scope.
 */
export function deleteConfirmText(saveType: SaveType, recordTitle: string, warning: string): ConfirmText {
  const kindTitle = saveType === 'save' ? 'Save' : 'State';
  const message = withWarning(`Delete '${recordTitle}' from the server? This cannot be undone.`, warning);
  return { title: `Delete Cloud ${kindTitle}`, message };
}

/**
 * Stale-response guard (parity with `details_cloud_request_id`,
 * details_view_mixin.py:797): a monotonically increasing id per fetch,
 * with only the most recently issued id considered current. Call `next()`
 * before starting a fetch, and `isCurrent(id)` before applying its result.
 */
export function createRequestGuard() {
  let latest = 0;
  return {
    next(): number {
      latest += 1;
      return latest;
    },
    isCurrent(id: number): boolean {
      return id === latest;
    },
  };
}
