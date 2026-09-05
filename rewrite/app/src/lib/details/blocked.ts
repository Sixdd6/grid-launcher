// Why an install action is blocked, as the details popup's button tooltips.
// Ported verbatim from `_ps4_content_install_block_reason` /
// `_xbox360_content_install_block_reason`
// (grid_launcher/ui/mixins/install_mixin.py:300-318), applied at
// `game_views.py:687,699`. The PRIMARY button's reason is not here: it needs
// the configured emulator list, so it comes from the backend's
// `install_block_reason` command.
//
// No API/store imports so this stays trivially unit-testable.
import { isContentPlatform, isPs4Platform } from './actions';

/**
 * `is_emulators_platform` (selection.py:138-142 / grid-core
 * `cloud::scope::is_emulators_platform`): trimmed, case-folded platform
 * equal to the literal "emulators".
 */
export function isEmulatorsPlatform(platform: string): boolean {
  return platform.trim().toLowerCase() === 'emulators';
}

/** `_ps4_content_install_block_reason` (install_mixin.py:300-309). */
export function ps4ContentBlockReason(
  platform: string,
  installed: boolean,
  romId: number | null,
  hasContent: boolean
): string {
  if (!isPs4Platform(platform)) return '';
  if (!installed) return 'Install the base PS4 game before applying update or DLC content.';
  if (romId === null) return 'This game is missing a ROM id, so update/DLC content cannot be downloaded.';
  if (!hasContent) return 'No update or DLC content is available for this PS4 game on the server.';
  return '';
}

/** `_xbox360_content_install_block_reason` (install_mixin.py:312-318). */
export function xbox360ContentBlockReason(installed: boolean, romId: number | null): string {
  if (!installed) return 'Game must be installed before content can be applied.';
  if (romId === null) return 'Game is missing a ROM ID.';
  return '';
}

/**
 * The tooltip for the Install Update / Install DLC button on `platform`.
 * PS4 platforms answer with the PS4 reasons; every other extra-content
 * platform (Xbox 360) answers with the Xbox 360 reasons; a platform with no
 * extra content at all has no button and therefore no reason.
 */
export function contentBlockReason(
  kind: 'update' | 'dlc',
  platform: string,
  installed: boolean,
  romId: number | null,
  hasContent: boolean
): string {
  void kind; // the reference's reasons do not distinguish update from DLC
  if (!isContentPlatform(platform)) return '';
  if (isPs4Platform(platform)) return ps4ContentBlockReason(platform, installed, romId, hasContent);
  return xbox360ContentBlockReason(installed, romId);
}
