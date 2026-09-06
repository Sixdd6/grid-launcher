//! Cloud save scope, block reasons, and the shared-sync owner search.
//!
//! Ported from `grid_launcher/emulator/selection.py` (`cloud_save_scope_for_game`
//! :56-95, `cloud_save_block_reason_for_game` :96-135, `is_native_executable_platform`
//! :138-143) and `grid_launcher/ui/mixins/cloud_mixin.py`
//! (`_emulator_game_matches_shared_sync` / `_shared_cloud_sync_owner_game` :392-421).
//! See `docs/porting/06-cloud-saves.md` ("Save scope", "Block reasons").
//!
//! Both `selection.py` functions receive `is_xemu_emulator_name` /
//! `is_redream_emulator_name` / `is_retroarch_emulator_name` as
//! `Callable[[str], bool]` — plain name-only predicates, NOT the
//! entry-and-profile-aware `_is_xemu_emulator_name(name, emulator)` methods
//! on `cloud_mixin.py` (those live at the `autoconfig` layer as
//! `is_xemu`/`is_redream`/`is_retroarch`, keyed on `EmulatorEntry` +
//! `EmulatorProfile`, and are NOT reusable here — different inputs
//! entirely). The only standalone one-argument version in the Python tree
//! is `is_retroarch_emulator_name` (`grid_launcher/tv/bridge/game_backend.py:33`):
//! `"retroarch" in emulator_name.strip().casefold()`. This module ports that
//! verbatim and extends the same substring rule to xemu/redream.
//!
//! Note this is NOT the full behavior of `_is_xemu_emulator_name`/
//! `_is_redream_emulator_name`: those (`_emulator_matches_tokens`,
//! cloud_mixin.py:1349-1363) still resolve `_emulator_entry_by_name(name)`
//! and try an entry-and-profile-aware match FIRST, only falling back to the
//! bare substring test when no configured entry matches. That entry-aware
//! path is unreachable at this pure layer by design — these functions take
//! `fn(&str) -> bool` with no config access at all, per this task's pinned
//! signatures. A configured entry's aliases (e.g. an emulator literally
//! named "My Flycast Build" with an autoprofile `match_tokens` including
//! `"redream"`) could still widen the match beyond plain substring — that
//! widening, if ever needed, belongs at the `ops` layer, which has the
//! config and can call `autoconfig::is_xemu`/`is_redream`/`is_retroarch`
//! directly instead of these name-only reductions.
//!
//! `is_xemu_emulator_name`/`is_redream_emulator_name`/
//! `is_retroarch_emulator_name` all reuse
//! `autoconfig::name_matches_any_token_substring` rather than duplicating
//! the substring logic three times.

use crate::autoconfig::cores::CoreFlags;
use crate::autoconfig::name_matches_any_token_substring;

use super::{CloudGame, SaveType};

/// `cloud_save_scope_for_game`'s three return values (selection.py:56-95).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveScope {
    PerGame,
    SharedSingle,
    SharedSlotted,
}

impl SaveScope {
    pub fn as_str(&self) -> &'static str {
        match self {
            SaveScope::PerGame => "per-game",
            SaveScope::SharedSingle => "shared-single",
            SaveScope::SharedSlotted => "shared-slotted",
        }
    }
}

/// `is_native_executable_platform` (selection.py:138-143): trimmed,
/// case-folded `platform` starting with `"windows"`. The Python function
/// takes the whole `game` dict and reads `game.get("platform", "")`; this
/// port takes the already-extracted platform string directly, matching this
/// module's other pure functions.
pub fn is_native_executable_platform(platform: &str) -> bool {
    platform.trim().to_lowercase().starts_with("windows")
}

/// `is_emulators_platform` (selection.py:138-142): trimmed, case-folded
/// `platform` equal to the literal `"emulators"`. Ported for
/// [`shared_sync_owner`]'s platform gate; later tasks (the sync-directory
/// resolution and cloud-emulator-resolution wrappers, doc 06 "Emulator
/// resolution for cloud operations") need the same predicate.
pub fn is_emulators_platform(platform: &str) -> bool {
    platform.trim().to_lowercase() == "emulators"
}

/// `_is_xemu_emulator_name` (cloud_mixin.py:1389) reduced to its name-only
/// substring rule — see module doc comment.
pub fn is_xemu_emulator_name(name: &str) -> bool {
    name_matches_any_token_substring(name, &["xemu"])
}

/// `_is_redream_emulator_name` (cloud_mixin.py:1395) reduced to its
/// name-only substring rule — see module doc comment.
pub fn is_redream_emulator_name(name: &str) -> bool {
    name_matches_any_token_substring(name, &["redream"])
}

/// `is_retroarch_emulator_name` (game_backend.py:33), verbatim: `"retroarch"
/// in emulator_name.strip().casefold()`.
pub fn is_retroarch_emulator_name(name: &str) -> bool {
    name_matches_any_token_substring(name, &["retroarch"])
}

/// `cloud_save_scope_for_game` (selection.py:56-95). The `game` parameter is
/// deliberately absent: Python's version takes `game` only to `del game` it
/// immediately (line 65) — it is never read.
///
/// Trigger chain, in order: `save_type != Save` -> `PerGame`; trimmed
/// non-blank `emulator_name` and xemu -> `SharedSingle`; trimmed non-blank
/// `emulator_name` and Redream -> `SharedSlotted`; trimmed non-blank
/// `emulator_name` and RetroArch and `core_flags` is `Some` and
/// `vmu_shared_saves` -> `SharedSlotted`; else `PerGame`.
///
/// Note the emptiness check here is on the TRIMMED name
/// (`emulator_name.strip()` in Python) — contrast with
/// [`cloud_save_block_reason`], whose RetroArch gate checks the RAW string
/// (bare `emulator_name`, no `.strip()`), a genuine asymmetry in the
/// reference (selection.py:71 vs selection.py:114).
pub fn cloud_save_scope(
    save_type: SaveType,
    emulator_name: &str,
    core_flags: Option<&CoreFlags>,
) -> SaveScope {
    if save_type != SaveType::Save {
        return SaveScope::PerGame;
    }

    let has_name = !emulator_name.trim().is_empty();

    if has_name && is_xemu_emulator_name(emulator_name) {
        return SaveScope::SharedSingle;
    }
    if has_name && is_redream_emulator_name(emulator_name) {
        return SaveScope::SharedSlotted;
    }
    if has_name
        && is_retroarch_emulator_name(emulator_name)
        && core_flags.is_some_and(|flags| flags.vmu_shared_saves)
    {
        return SaveScope::SharedSlotted;
    }

    SaveScope::PerGame
}

/// `cloud_save_block_reason_for_game` (selection.py:96-135). Returns `""`
/// when the operation is allowed. The `is_xemu_emulator_name` /
/// `is_redream_emulator_name` callbacks Python accepts are `del`eted
/// unused (selection.py:107-108) — this port has no parameters for them at
/// all, since it never called them to begin with.
///
/// Trigger chain: native platform and `save_type == State` (checked first)
/// -> a fixed message pointing at the native save-locations panel (shown
/// on the Saves tab only once the game is installed) and inviting a manual
/// save folder when PCGamingWiki did not supply one — it says nothing
/// about save states, which are simply not offered for PC games (`Save`
/// is never blocked on this ground; only `State` reaches this branch);
/// else, for `State`, RetroArch-gated `supports_save_states` then
/// `cloud_sync_safe`; else, for `Save`, RetroArch-gated `supports_saves`;
/// else `""`.
///
/// The three RetroArch-gated reasons require: a non-empty `emulator_name`
/// (bare `emulator_name`, NOT trimmed — selection.py's `and emulator_name`
/// has no `.strip()`, unlike [`cloud_save_scope`]'s gate), that name
/// matching RetroArch, and `core_flags` being `Some`. `CoreFlags::default()`
/// (all-`true` except `vmu_shared_saves`) is what an unknown core reports,
/// so a `None` `core_flags` and a `Some(CoreFlags::default())` both block
/// nothing — the difference only matters once a real core's flags are
/// looked up (that lookup, and its asymmetric fallback vs. the scope
/// wrapper, is `ops.rs`'s job, not this pure function's).
pub fn cloud_save_block_reason(
    platform: &str,
    save_type: SaveType,
    emulator_name: &str,
    core_flags: Option<&CoreFlags>,
) -> String {
    if is_native_executable_platform(platform) && save_type == SaveType::State {
        return "Save sync for PC games uses the save locations shown here once the game is installed. If none was filled in from PCGamingWiki, add the game's save folder.".to_string();
    }

    let retroarch_gated = !emulator_name.is_empty() && is_retroarch_emulator_name(emulator_name);

    if save_type == SaveType::State {
        if let Some(flags) = core_flags.filter(|_| retroarch_gated) {
            if !flags.supports_save_states {
                return "This core does not support save states.".to_string();
            }
            if !flags.cloud_sync_safe {
                return "Save state format for this core may not be stable across devices."
                    .to_string();
            }
        }
    }

    if save_type == SaveType::Save {
        if let Some(flags) = core_flags.filter(|_| retroarch_gated) {
            if !flags.supports_saves {
                return "This core does not support battery saves.".to_string();
            }
        }
    }

    String::new()
}

/// `_emulator_game_matches_shared_sync` + `_shared_cloud_sync_owner_game`'s
/// candidate scan (cloud_mixin.py:376-421), reduced to a pure substring +
/// rom-id search: the first `game` in `games` that is on the `Emulators`
/// platform ([`is_emulators_platform`], the reference's FIRST gate — line
/// 380: `if not self._is_emulators_platform(game): return False`) AND whose
/// `title`/`platform`/`description`/`rom_file_name` fields, each trimmed,
/// joined with a single space, and lowercased, contain `token`
/// (case-insensitively), AND which has a non-blank `rom_id`.
///
/// `token` is the caller's pre-selected match word ("xemu" or "redream" —
/// selection is by [`cloud_save_scope`]'s branch, which this function does
/// not recompute). The Python original also de-duplicates candidates by a
/// game key before scanning for a resolvable rom id; that never changes
/// which game is returned FIRST for a token search over an
/// already-appropriate `games` slice, so it is the caller's responsibility
/// (`ops.rs`), not this pure function's — this task's brief pins the search
/// itself, not the de-dup bookkeeping.
///
/// One reference quirk preserved for documentation, though it can never
/// change behavior here: Python's `" ".join(...)` over the fixed four-field
/// tuple always yields at least three spaces even when every field is
/// blank, so its `if not candidate_text: return False` guard is
/// unreachable dead code in the reference. `token` is always a non-blank
/// literal ("xemu"/"redream"), so "   ".contains(token) is false regardless
/// — the guard's absence here changes nothing observable.
pub fn shared_sync_owner<'a>(token: &str, games: &'a [CloudGame]) -> Option<&'a CloudGame> {
    let token = token.trim().to_lowercase();
    if token.is_empty() {
        return None;
    }

    games.iter().find(|game| {
        if !is_emulators_platform(&game.platform) {
            return false;
        }
        if game.rom_id.is_empty() {
            return false;
        }
        let candidate_text = [
            game.title.as_str(),
            game.platform.as_str(),
            game.description.as_str(),
            game.rom_file_name.as_str(),
        ]
        .iter()
        .map(|field| field.trim())
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
        candidate_text.contains(&token)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flags(supports_save_states: bool, supports_saves: bool, cloud_sync_safe: bool) -> CoreFlags {
        CoreFlags {
            supports_save_states,
            supports_saves,
            cloud_sync_safe,
            vmu_shared_saves: false,
        }
    }

    const SAFE: CoreFlags = CoreFlags {
        supports_save_states: true,
        supports_saves: true,
        cloud_sync_safe: true,
        vmu_shared_saves: false,
    };
    const MAME: CoreFlags = CoreFlags {
        supports_save_states: false,
        supports_saves: false,
        cloud_sync_safe: false,
        vmu_shared_saves: false,
    };
    const FBNEO: CoreFlags = CoreFlags {
        supports_save_states: true,
        supports_saves: true,
        cloud_sync_safe: false,
        vmu_shared_saves: false,
    };

    // --- ported from tests/test_cloud_save_block_reason.py:38-117 (one test per branch) ---

    #[test]
    fn blocks_state_when_core_does_not_support_save_states() {
        let reason = cloud_save_block_reason("Arcade", SaveType::State, "RetroArch", Some(&MAME));
        assert_eq!(reason, "This core does not support save states.");
    }

    #[test]
    fn blocks_state_when_core_is_not_cloud_sync_safe() {
        let reason = cloud_save_block_reason("Arcade", SaveType::State, "RetroArch", Some(&FBNEO));
        assert_eq!(
            reason,
            "Save state format for this core may not be stable across devices."
        );
    }

    #[test]
    fn does_not_block_state_for_safe_core() {
        let reason = cloud_save_block_reason("Arcade", SaveType::State, "RetroArch", Some(&SAFE));
        assert_eq!(reason, "");
    }

    #[test]
    fn blocks_save_when_core_does_not_support_saves() {
        let reason = cloud_save_block_reason("Arcade", SaveType::Save, "RetroArch", Some(&MAME));
        assert_eq!(reason, "This core does not support battery saves.");
    }

    #[test]
    fn does_not_block_save_for_safe_core() {
        let reason = cloud_save_block_reason("Arcade", SaveType::Save, "RetroArch", Some(&SAFE));
        assert_eq!(reason, "");
    }

    #[test]
    fn flag_check_skipped_when_not_retroarch_emulator() {
        let reason = cloud_save_block_reason("Arcade", SaveType::State, "Mesen", Some(&MAME));
        assert_eq!(reason, "");
    }

    #[test]
    fn flag_check_skipped_when_no_flags_passed() {
        let reason = cloud_save_block_reason("Arcade", SaveType::State, "RetroArch", None);
        assert_eq!(reason, "");
    }

    #[test]
    fn native_platform_blocks_save_states_regardless_of_core_flags() {
        let reason = cloud_save_block_reason("Windows", SaveType::State, "RetroArch", Some(&SAFE));
        assert_eq!(
            reason,
            "Save sync for PC games uses the save locations shown here once the game is installed. If none was filled in from PCGamingWiki, add the game's save folder."
        );
    }

    #[test]
    fn native_platform_does_not_block_save_folder_sync() {
        let reason = cloud_save_block_reason("Windows", SaveType::Save, "RetroArch", Some(&SAFE));
        assert_eq!(reason, "");
    }

    // --- brief's four extras ---

    #[test]
    fn scope_state_is_always_per_game() {
        // Even an xemu/Redream/RetroArch-with-vmu name must not escape
        // PerGame when save_type is State.
        assert_eq!(
            cloud_save_scope(SaveType::State, "xemu", None),
            SaveScope::PerGame
        );
        assert_eq!(
            cloud_save_scope(SaveType::State, "Redream", None),
            SaveScope::PerGame
        );
        let vmu = flags(true, true, true);
        let mut vmu_on = vmu;
        vmu_on.vmu_shared_saves = true;
        assert_eq!(
            cloud_save_scope(SaveType::State, "RetroArch", Some(&vmu_on)),
            SaveScope::PerGame
        );
    }

    #[test]
    fn scope_xemu_shared_single() {
        assert_eq!(
            cloud_save_scope(SaveType::Save, "xemu", None),
            SaveScope::SharedSingle
        );
    }

    #[test]
    fn scope_redream_and_vmu_flag_shared_slotted() {
        assert_eq!(
            cloud_save_scope(SaveType::Save, "Redream", None),
            SaveScope::SharedSlotted
        );

        let mut vmu_flags = flags(true, true, true);
        vmu_flags.vmu_shared_saves = true;
        assert_eq!(
            cloud_save_scope(SaveType::Save, "RetroArch", Some(&vmu_flags)),
            SaveScope::SharedSlotted
        );

        // RetroArch without the vmu flag stays per-game.
        assert_eq!(
            cloud_save_scope(SaveType::Save, "RetroArch", Some(&SAFE)),
            SaveScope::PerGame
        );
        // RetroArch with the vmu flag but no flags supplied at all stays per-game.
        assert_eq!(
            cloud_save_scope(SaveType::Save, "RetroArch", None),
            SaveScope::PerGame
        );
    }

    #[test]
    fn shared_owner_requires_a_rom_id_and_matches_substrings_case_insensitively() {
        let no_rom_id = CloudGame {
            title: "Some xemu game".to_string(),
            platform: "Emulators".to_string(),
            rom_id: "".to_string(),
            ..Default::default()
        };
        let wrong_word = CloudGame {
            title: "Nothing relevant here".to_string(),
            platform: "Emulators".to_string(),
            rom_id: "7".to_string(),
            ..Default::default()
        };
        let matches_via_description = CloudGame {
            platform: "Emulators".to_string(),
            description: "Runs great under XEMU".to_string(),
            rom_id: "42".to_string(),
            ..Default::default()
        };

        let games = vec![no_rom_id, wrong_word, matches_via_description];
        let owner = shared_sync_owner("xemu", &games);
        assert_eq!(owner, Some(&games[2]));
        assert_eq!(owner.unwrap().rom_id, "42");

        // No match at all.
        assert_eq!(shared_sync_owner("redream", &games), None);
    }

    #[test]
    fn shared_owner_requires_the_emulators_platform() {
        // Same title/rom_id, only the platform differs.
        let on_n64 = CloudGame {
            title: "Plays great in xemu".to_string(),
            platform: "Nintendo 64".to_string(),
            rom_id: "9".to_string(),
            ..Default::default()
        };
        assert_eq!(
            shared_sync_owner("xemu", std::slice::from_ref(&on_n64)),
            None,
            "a matching title/rom_id on a non-Emulators platform must not be an owner"
        );

        let on_emulators = CloudGame {
            platform: "Emulators".to_string(),
            ..on_n64.clone()
        };
        assert_eq!(
            shared_sync_owner("xemu", std::slice::from_ref(&on_emulators)),
            Some(&on_emulators),
            "the identical game IS an owner once its platform is literally Emulators"
        );

        // The platform match is trimmed and case-folded, same as
        // is_native_executable_platform's rule.
        let padded_case = CloudGame {
            platform: "  EMULATORS  ".to_string(),
            ..on_n64
        };
        assert_eq!(
            shared_sync_owner("xemu", std::slice::from_ref(&padded_case)),
            Some(&padded_case)
        );
    }

    #[test]
    fn is_emulators_platform_trims_and_casefolds() {
        assert!(is_emulators_platform("Emulators"));
        assert!(is_emulators_platform("  emulators  "));
        assert!(is_emulators_platform("EMULATORS"));
        assert!(!is_emulators_platform("Nintendo 64"));
        assert!(!is_emulators_platform(""));
    }
}
