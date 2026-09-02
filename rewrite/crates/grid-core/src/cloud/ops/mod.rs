//! Cloud save/state orchestration: the `cloud_mixin.py` control flow
//! ported onto the pure `cloud::*` modules.
//!
//! Everything here takes plain data plus a `&RommClient`; nothing touches
//! Tauri and nothing prints. Dialog text is RETURNED as [`CloudMessage`]s
//! so the app layer decides how (and whether) to show it — Python's
//! `show_dialogs` flag becomes "the caller ignores the messages".
//!
//! Split by concern:
//! - `ops` (this file): context, caches, emulator/scope/rom-id resolution,
//!   the ten-branch candidate dispatch, local mtimes, record listing.
//! - [`upload`][]: `_upload_cloud_files_for_game`.
//! - [`restore`][]: `_restore_cloud_save_for_game` /
//!   `_restore_cloud_state_for_game`.
//! - [`native`][]: `_upload_native_saves_for_game` /
//!   `_restore_native_cloud_save_for_game`.
//!
//! See `docs/porting/06-cloud-saves.md` and
//! `docs/superpowers/specs/2026-09-02-cloud-saves-design.md` (deviations
//! D1, D2, D4, D6, D7).

pub mod native;
pub mod restore;
pub mod upload;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::autoconfig::cores::{core_entries, core_flags, core_flags_for_platform, CoreFlags};
use crate::autoconfig::paths::expand_user;
use crate::autoconfig::{self, name_matches_any_token_substring, readers};
use crate::config::{Config, EmulatorEntry};
use crate::launch::profiles::{profile_for_entry, EmulatorProfile};
use crate::launch::selection::{
    default_emulator_name_for_platform, emulator_entry_by_name, mapping_value_for_platform,
};
use crate::romm::RommClient;

use super::candidates::{
    cemu_save_directories, directory_candidates, file_candidates, pcsx2_save_directories,
    ppsspp_save_directories, resolved_ignore_sets, resolved_save_strategy, rpcs3_save_directories,
};
use super::dirs::{self, PathKey, ResolveContext};
use super::scope::{
    cloud_save_block_reason, is_emulators_platform, is_native_executable_platform,
    shared_sync_owner, SaveScope,
};
use super::state::{game_key, sync_entry_for};
use super::tokens::{game_save_match_tokens, ps2_serial_tokens, ps3_id_tokens, psp_id_tokens};
use super::transfer::{MessageSeverity, SUPPORTED_IMAGE_EXTENSIONS};
use super::window::{
    session_filtered_directory_candidates, session_filtered_file_candidates,
    session_window_for_state_upload, ActiveSessionRef, Window,
};
use super::xemu_sync::{block_reason_for_status, classify_hdd_image, xemu_hdd_path_from_config};
use super::{latest_mtime_under, CloudGame, IgnoreSets, SaveType};

// ---------------------------------------------------------------------
// Context, caches, messages
// ---------------------------------------------------------------------

/// One user-facing message the Python original would have shown in a
/// `QMessageBox`. `show_dialogs=False` in Python is "drop these".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudMessage {
    pub text: String,
    pub severity: MessageSeverity,
}

impl CloudMessage {
    pub fn info(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            severity: MessageSeverity::Info,
        }
    }

    pub fn warning(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            severity: MessageSeverity::Warning,
        }
    }
}

/// Everything the ops layer reads that isn't the RomM client itself.
///
/// Two fields have no Python counterpart in the pinned interface block but
/// are required for the ported control flow (see the task report):
/// `pcgw_paths` and `wine_prefix`. Python reaches both off `self` /
/// `game["native_wineprefix"]`; [`CloudGame`] carries neither, and
/// `_upload_cloud_files_for_game` / `_restore_cloud_save_for_game`
/// DELEGATE to the native flows for a Windows platform (doc 06 "Upload
/// planning" precondition 1, "Restore — saves" step 1), so the delegating
/// functions need them.
pub struct CloudContext<'a> {
    pub config: &'a Config,
    pub profiles: &'a [EmulatorProfile],
    /// Registry rows plus the server cache, when the caller has one — the
    /// candidate pool [`shared_sync_owner`] scans.
    pub all_games: &'a [CloudGame],
    /// The template every sync path is expanded against. `emulator_dir` is
    /// IGNORED: this layer recomputes it per resolved entry (Python does
    /// the same, `cloud_mixin.py:927-930`).
    pub resolve_ctx: ResolveContext<'a>,
    pub active_sessions: &'a [ActiveSessionRef],
    pub now: f64,
    /// PCGamingWiki-sourced native save paths for this game, in order.
    /// Empty for emulator games.
    pub pcgw_paths: &'a [String],
    /// The game's `native_wineprefix`, when it has one.
    pub wine_prefix: Option<&'a Path>,
}

/// `(emulator name, emulator path, `"save_paths"`/`"state_paths"`)` —
/// `_sync_directory_paths_cache`'s tuple key (grid-launcher.py:433).
type SyncDirKey = (String, String, &'static str);

/// `(resolved directories, the subset that are explicit files)`.
pub type ResolvedDirs = (Vec<PathBuf>, Vec<PathBuf>);

/// The two per-config memos `cloud_mixin` keeps: the cloud emulator entry
/// cache (`grid-launcher.py:432`) and the resolved sync-directory cache
/// (`grid-launcher.py:433`). Both are cleared on every config save
/// (`grid-launcher.py:3151-3152`) — hence [`CloudCaches::clear`].
#[derive(Debug, Default)]
pub struct CloudCaches {
    /// `"<title>::<platform>::<save_type>"` -> `(emulator name, entry)`.
    /// QUIRK (doc 06 "Emulator resolution for cloud operations", recorded
    /// quirk 9): the key omits the ROM id, so two library rows with the
    /// same title and platform share one entry.
    emulator_entries: HashMap<String, (String, Option<EmulatorEntry>)>,
    /// `(name, path, key)` -> `(resolved dirs, explicit file roots)`,
    /// matching `_sync_directory_paths_cache`'s tuple key.
    sync_dirs: HashMap<SyncDirKey, ResolvedDirs>,
}

impl CloudCaches {
    pub fn clear(&mut self) {
        self.emulator_entries.clear();
        self.sync_dirs.clear();
    }
}

// ---------------------------------------------------------------------
// Small shared helpers
// ---------------------------------------------------------------------

fn profile_slice(profile: Option<&EmulatorProfile>) -> &[EmulatorProfile] {
    match profile {
        Some(p) => std::slice::from_ref(p),
        None => &[],
    }
}

fn profile_for<'a>(ctx: &CloudContext<'a>, entry: &EmulatorEntry) -> Option<&'a EmulatorProfile> {
    profile_for_entry(&entry.name, &entry.path, ctx.profiles)
}

/// `_emulator_matches_tokens` (cloud_mixin.py:1349-1363): try the
/// entry-and-autoprofile match first (looking the entry up by NAME when
/// the caller has none), then fall back to a plain case-folded substring
/// test of each token against the emulator NAME.
fn matches_tokens(
    ctx: &CloudContext,
    name: &str,
    entry: Option<&EmulatorEntry>,
    tokens: &[&str],
) -> bool {
    let looked_up = match entry {
        Some(_) => None,
        None if name.trim().is_empty() => None,
        None => emulator_entry_by_name(&ctx.config.emulators, name),
    };
    let resolved = entry.or(looked_up);
    if let Some(resolved) = resolved {
        let profile = profile_for(ctx, resolved);
        if autoconfig::emulator_matches_tokens(resolved, tokens, profile_slice(profile)) {
            return true;
        }
    }
    name_matches_any_token_substring(name, tokens)
}

macro_rules! token_predicate {
    ($fn_name:ident, $($token:literal),+) => {
        fn $fn_name(ctx: &CloudContext, name: &str, entry: Option<&EmulatorEntry>) -> bool {
            matches_tokens(ctx, name, entry, &[$($token),+])
        }
    };
}

token_predicate!(is_retroarch, "retroarch");
token_predicate!(is_xemu, "xemu", "xemu.exe");
token_predicate!(is_redream, "redream");
token_predicate!(is_rpcs3, "rpcs3");
token_predicate!(is_ppsspp, "ppsspp");
token_predicate!(is_pcsx2, "pcsx2");
token_predicate!(is_cemu, "cemu");
token_predicate!(is_dolphin, "dolphin");

/// The stub entry `_latest_local_*_mtime_for_game` synthesises when the
/// named emulator is not configured (cloud_mixin.py:1547, :1568).
fn stub_entry(name: &str) -> EmulatorEntry {
    EmulatorEntry {
        name: name.to_string(),
        path: String::new(),
        args: "%rom%".to_string(),
        save_strategy: "auto".to_string(),
        ..Default::default()
    }
}

/// `Path(emulator["path"]).expanduser().parent`, or `None` for a blank
/// path (cloud_mixin.py:927-930's `Path()` fallback).
fn emulator_dir_for(entry: &EmulatorEntry) -> Option<PathBuf> {
    if entry.path.is_empty() {
        return None;
    }
    expand_user(&entry.path).parent().map(Path::to_path_buf)
}

fn resolve_ctx_for<'a>(
    ctx: &'a CloudContext<'a>,
    emulator_dir: Option<&'a Path>,
) -> ResolveContext<'a> {
    ResolveContext {
        emulator_dir,
        library_dir: ctx.resolve_ctx.library_dir,
        config_dir: ctx.resolve_ctx.config_dir,
        windows_documents: ctx.resolve_ctx.windows_documents,
    }
}

/// The memoized `_resolved_sync_directory_paths` (cloud_mixin.py:618-981).
/// Returns `(resolved directories, the subset that are explicit files)`.
///
/// This layer deliberately does NOT call the milestone-5 `ensure_*`
/// writers that `cloud_mixin.py:646` invokes: the M5 D1 policy is
/// "`ensure_*` runs only for new entries" (controller ruling, recorded).
pub fn resolved_sync_dirs(
    ctx: &CloudContext,
    caches: &mut CloudCaches,
    entry: &EmulatorEntry,
    key: PathKey,
) -> ResolvedDirs {
    let cache_key = (
        entry.name.clone(),
        entry.path.clone(),
        match key {
            PathKey::SavePaths => "save_paths",
            PathKey::StatePaths => "state_paths",
        },
    );
    if let Some(cached) = caches.sync_dirs.get(&cache_key) {
        return cached.clone();
    }
    let profile = profile_for(ctx, entry);
    let emulator_dir = emulator_dir_for(entry);
    let rctx = resolve_ctx_for(ctx, emulator_dir.as_deref());
    let resolved = dirs::resolved_sync_directory_paths(entry, profile, key, &rctx);
    caches.sync_dirs.insert(cache_key, resolved.clone());
    resolved
}

fn path_key_for(save_type: SaveType) -> PathKey {
    match save_type {
        SaveType::Save => PathKey::SavePaths,
        SaveType::State => PathKey::StatePaths,
    }
}

/// `"save"` / `"state"` — the `kind_label` the missing-directories warning
/// interpolates (cloud_mixin.py:2474).
fn kind_label(save_type: SaveType) -> &'static str {
    save_type.as_str()
}

fn ignore_for(
    ctx: &CloudContext,
    entry: &EmulatorEntry,
    name: &str,
    save_type: SaveType,
) -> IgnoreSets {
    let profile = profile_for(ctx, entry);
    let pcsx2 = is_pcsx2(ctx, name, Some(entry));
    resolved_ignore_sets(Some(entry), profile, save_type, pcsx2)
}

/// The session mtime window for `game` (cloud_mixin.py's
/// `_session_window_for_state_upload`, used for save AND state candidate
/// filtering alike).
fn window_for(ctx: &CloudContext, game: &CloudGame) -> Option<Window> {
    let entry = sync_entry_for(ctx.config, &game_key(game));
    session_window_for_state_upload(ctx.active_sessions, game, &entry, ctx.now)
}

fn record_str(record: &Value, key: &str) -> String {
    record
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string()
}

// ---------------------------------------------------------------------
// Emulator resolution / scope / block reasons
// ---------------------------------------------------------------------

/// `_resolved_cloud_emulator_entry_for_game` (cloud_mixin.py:175-235),
/// keeping Python's `(name, entry)` pair — the public
/// [`resolved_cloud_emulator_entry`] drops the name.
fn resolved_cloud_emulator_pair(
    ctx: &CloudContext,
    caches: &mut CloudCaches,
    game: &CloudGame,
    save_type: SaveType,
) -> (String, Option<EmulatorEntry>) {
    let cache_key = format!("{}::{}::{}", game.title, game.platform, save_type.as_str());
    if let Some(cached) = caches.emulator_entries.get(&cache_key) {
        return cached.clone();
    }

    let platform = game.platform.trim();
    let default_name = if platform.is_empty() {
        String::new()
    } else {
        default_emulator_name_for_platform(
            &ctx.config.emulators,
            &ctx.config.default_emulators,
            platform,
            ctx.profiles,
            &ctx.config.retroarch_cores,
        )
    };
    let default_entry = if default_name.is_empty() {
        None
    } else {
        emulator_entry_by_name(&ctx.config.emulators, &default_name).cloned()
    };
    let default_pair = if default_name.is_empty() {
        (String::new(), None)
    } else {
        (default_name, default_entry)
    };

    if default_pair.1.is_some() || !is_emulators_platform(&game.platform) {
        caches
            .emulator_entries
            .insert(cache_key, default_pair.clone());
        return default_pair;
    }

    // The `Emulators`-platform shared-token scan (cloud_mixin.py:212-229):
    // every configured emulator whose shared-sync token appears in the
    // game's free text, skipping any whose save scope is per-game when
    // uploading/restoring saves.
    for candidate in ctx.config.emulators.iter() {
        let candidate_name = candidate.name.trim().to_string();
        if candidate_name.is_empty() {
            continue;
        }
        if !emulator_game_matches_shared_sync(ctx, game, &candidate_name, Some(candidate)) {
            continue;
        }
        if save_type == SaveType::Save
            && scope_for(
                ctx,
                &candidate_name,
                Some(candidate),
                &game.platform,
                save_type,
            ) == SaveScope::PerGame
        {
            continue;
        }
        let shared = (candidate_name, Some(candidate.clone()));
        caches.emulator_entries.insert(cache_key, shared.clone());
        return shared;
    }

    caches
        .emulator_entries
        .insert(cache_key, default_pair.clone());
    default_pair
}

/// `_resolved_cloud_emulator_entry_for_game` (cloud_mixin.py:175). Cache
/// key `"<title>::<platform>::<save_type>"` — QUIRK: no ROM id.
pub fn resolved_cloud_emulator_entry(
    ctx: &CloudContext,
    caches: &mut CloudCaches,
    game: &CloudGame,
    save_type: SaveType,
) -> Option<EmulatorEntry> {
    resolved_cloud_emulator_pair(ctx, caches, game, save_type).1
}

/// `_emulator_game_matches_shared_sync` (cloud_mixin.py:376-395): the
/// game must be on the `Emulators` platform, and the emulator must be
/// xemu (then `"xemu"` must appear in the game's free text) or Redream
/// (then `"redream"`).
fn emulator_game_matches_shared_sync(
    ctx: &CloudContext,
    game: &CloudGame,
    emulator_name: &str,
    entry: Option<&EmulatorEntry>,
) -> bool {
    match shared_sync_token(ctx, emulator_name, entry) {
        Some(token) => contains_shared_token(game, token),
        None => false,
    }
}

/// The free-text containment half of `_emulator_game_matches_shared_sync`
/// — [`shared_sync_owner`] additionally requires a resolvable ROM id,
/// which this *matching* predicate does not (cloud_mixin.py:380-395 vs
/// :421-433, where the id check is a separate later pass).
fn contains_shared_token(game: &CloudGame, token: &str) -> bool {
    if !is_emulators_platform(&game.platform) {
        return false;
    }
    let text = [
        game.title.as_str(),
        game.platform.as_str(),
        game.description.as_str(),
        game.rom_file_name.as_str(),
    ]
    .iter()
    .map(|f| f.trim())
    .collect::<Vec<_>>()
    .join(" ")
    .to_lowercase();
    text.contains(token)
}

fn shared_sync_token(
    ctx: &CloudContext,
    emulator_name: &str,
    entry: Option<&EmulatorEntry>,
) -> Option<&'static str> {
    if is_xemu(ctx, emulator_name, entry) {
        return Some("xemu");
    }
    if is_redream(ctx, emulator_name, entry) {
        return Some("redream");
    }
    None
}

/// The RetroArch core flags the block-reason wrapper supplies
/// (cloud_mixin.py:116-121): ONLY when the emulator is RetroArch AND a
/// default core is configured for the platform. No
/// `core_flags_for_platform` fallback — that asymmetry is deliberate
/// (doc 06 "Block reasons").
fn block_reason_flags(
    ctx: &CloudContext,
    name: &str,
    entry: Option<&EmulatorEntry>,
    platform: &str,
) -> Option<CoreFlags> {
    if name.is_empty() || !is_retroarch(ctx, name, entry) {
        return None;
    }
    let core_id = mapping_value_for_platform(&ctx.config.retroarch_cores, platform)?;
    Some(core_flags(core_id, core_entries()))
}

/// The scope wrapper's flags (cloud_mixin.py:157-163): same, PLUS the
/// `core_flags_for_platform` fallback when no core is configured.
fn scope_flags(
    ctx: &CloudContext,
    name: &str,
    entry: Option<&EmulatorEntry>,
    platform: &str,
) -> Option<CoreFlags> {
    if name.is_empty() || !is_retroarch(ctx, name, entry) {
        return None;
    }
    match mapping_value_for_platform(&ctx.config.retroarch_cores, platform) {
        Some(core_id) => Some(core_flags(core_id, core_entries())),
        None if !platform.trim().is_empty() => core_flags_for_platform(platform, core_entries()),
        None => None,
    }
}

/// The emulator name the two wrappers resolve (cloud_mixin.py:104-110,
/// :143-149): the entry's own name, else the platform default.
fn wrapper_emulator_name(
    ctx: &CloudContext,
    entry: Option<&EmulatorEntry>,
    platform: &str,
) -> String {
    let from_entry = entry.map(|e| e.name.trim().to_string()).unwrap_or_default();
    if !from_entry.is_empty() {
        return from_entry;
    }
    let platform = platform.trim();
    if platform.is_empty() {
        return String::new();
    }
    default_emulator_name_for_platform(
        &ctx.config.emulators,
        &ctx.config.default_emulators,
        platform,
        ctx.profiles,
        &ctx.config.retroarch_cores,
    )
}

fn scope_for(
    ctx: &CloudContext,
    name: &str,
    entry: Option<&EmulatorEntry>,
    platform: &str,
    save_type: SaveType,
) -> SaveScope {
    let flags = scope_flags(ctx, name, entry, platform);
    scope_with_names(ctx, name, entry, save_type, flags.as_ref())
}

/// [`cloud_save_scope`] with the entry-aware xemu/Redream/RetroArch
/// predicates this layer has (the pure module's are name-only).
fn scope_with_names(
    ctx: &CloudContext,
    name: &str,
    entry: Option<&EmulatorEntry>,
    save_type: SaveType,
    flags: Option<&CoreFlags>,
) -> SaveScope {
    if save_type != SaveType::Save {
        return SaveScope::PerGame;
    }
    let has_name = !name.trim().is_empty();
    if has_name && is_xemu(ctx, name, entry) {
        return SaveScope::SharedSingle;
    }
    if has_name && is_redream(ctx, name, entry) {
        return SaveScope::SharedSlotted;
    }
    if has_name && is_retroarch(ctx, name, entry) && flags.is_some_and(|f| f.vmu_shared_saves) {
        return SaveScope::SharedSlotted;
    }
    SaveScope::PerGame
}

/// `_cloud_save_scope_for_game` (cloud_mixin.py:135-172).
pub fn scope_for_game(
    ctx: &CloudContext,
    game: &CloudGame,
    save_type: SaveType,
    entry: Option<&EmulatorEntry>,
) -> SaveScope {
    let name = wrapper_emulator_name(ctx, entry, &game.platform);
    scope_for(ctx, &name, entry, &game.platform, save_type)
}

/// `_cloud_save_block_reason_for_game` (cloud_mixin.py:96-133) EXACTLY —
/// no xemu image reasons. This is the compatibility gate
/// `details_cloud_mode_supported` consults (cloud_mixin.py:353): whether
/// the emulator/core can do cloud saves at all, which is a property of
/// the emulator, not of today's HDD image.
///
/// Fix round 1 (controller ruling): keeping the two separate is what
/// stops a qcow2 `hdd_path` from HIDING the cloud panel outright. The
/// panel must still appear so the user can read the conversion guidance
/// [`block_reason_for_game`] returns.
pub fn base_block_reason_for_game(
    ctx: &CloudContext,
    game: &CloudGame,
    save_type: SaveType,
    entry: Option<&EmulatorEntry>,
) -> String {
    let name = wrapper_emulator_name(ctx, entry, &game.platform);
    let flags = block_reason_flags(ctx, &name, entry, &game.platform);
    cloud_save_block_reason(&game.platform, save_type, &name, flags.as_ref())
}

/// [`base_block_reason_for_game`] PLUS the xemu raw-image block reasons
/// (spec "xemu flow"): when the resolved emulator is xemu and `save_type
/// == Save`, the configured `hdd_path` is classified and its reason, if
/// any, returned. The base reason wins when both fire.
///
/// This is the gate for the ACTIONS — upload, restore, and the panel's
/// Upload button/refusal text — not for whether the panel exists; see
/// [`base_block_reason_for_game`].
pub fn block_reason_for_game(
    ctx: &CloudContext,
    game: &CloudGame,
    save_type: SaveType,
    entry: Option<&EmulatorEntry>,
) -> String {
    let reason = base_block_reason_for_game(ctx, game, save_type, entry);
    if !reason.is_empty() {
        return reason;
    }

    let name = wrapper_emulator_name(ctx, entry, &game.platform);
    if save_type == SaveType::Save && is_xemu(ctx, &name, entry) {
        // D11 first: it is the transient, immediately actionable one, and
        // it must fire before anything opens the image.
        if xemu_session_is_running(ctx, &name) {
            return XEMU_RUNNING_REASON.to_string();
        }
        let hdd_path = entry
            .and_then(|e| xemu_hdd_path_from_config(&e.path))
            .unwrap_or_default();
        if let Some(reason) = block_reason_for_status(&classify_hdd_image(&hdd_path)) {
            return reason;
        }
    }

    String::new()
}

/// D11 (doc 06): xemu save sync writes into the HDD image IN PLACE, which
/// an image xemu still holds open can cross-link. Python replaced the
/// whole file and had no such hazard, so this string has no Python
/// original.
const XEMU_RUNNING_REASON: &str = "xemu is running — close it before syncing its saves.";

/// True when any active session resolves to the same emulator entry as
/// `name` — the xemu the caller is about to write into.
///
/// The scratch [`CloudCaches`] is deliberate: this walks other games'
/// entries, which have nothing to do with the caller's own cache keys,
/// and the resolution it runs is pure config/profile work (no disk, no
/// network), so a per-call memo costs nothing worth keeping.
fn xemu_session_is_running(ctx: &CloudContext, name: &str) -> bool {
    let name = name.trim();
    if name.is_empty() {
        return false;
    }
    let mut scratch = CloudCaches::default();
    ctx.active_sessions.iter().any(|session| {
        let (_, session_entry) =
            resolved_cloud_emulator_pair(ctx, &mut scratch, &session.game, SaveType::Save);
        let session_name =
            wrapper_emulator_name(ctx, session_entry.as_ref(), &session.game.platform);
        session_name.trim().eq_ignore_ascii_case(name)
    })
}

/// `_cloud_sync_rom_id_for_game` (cloud_mixin.py:439-458). For saves a
/// shared-sync owner's ROM id wins over the game's own; states never take
/// this indirection.
pub fn cloud_sync_rom_id(
    ctx: &CloudContext,
    caches: &mut CloudCaches,
    game: &CloudGame,
    save_type: SaveType,
) -> Option<String> {
    let (name, entry) = resolved_cloud_emulator_pair(ctx, caches, game, save_type);
    cloud_sync_rom_id_with(ctx, game, save_type, &name, entry.as_ref())
}

fn cloud_sync_rom_id_with(
    ctx: &CloudContext,
    game: &CloudGame,
    save_type: SaveType,
    name: &str,
    entry: Option<&EmulatorEntry>,
) -> Option<String> {
    if save_type == SaveType::Save {
        if let Some(owner) = shared_cloud_sync_owner(ctx, game, name, entry, save_type) {
            if !owner.rom_id.trim().is_empty() {
                return Some(owner.rom_id.trim().to_string());
            }
        }
    }
    let own = game.rom_id.trim();
    (!own.is_empty()).then(|| own.to_string())
}

/// `_shared_cloud_sync_owner_game` (cloud_mixin.py:398-437), minus the
/// `_matching_installed_emulator_games` last resort
/// (install_mixin.py:1106 -> install_registry.py:65).
///
/// Fix round 1 — the real blocker, corrected: that last resort scans the
/// SAME pool this function already walks (`self.library_games`, i.e.
/// `ctx.all_games`); what it does differently is match by INSTALL PATH
/// rather than by free text — it looks for the library game whose own
/// archive/extracted files ARE the emulator binary at `entry.path`, via
/// `candidate_archive_paths_for_game` /
/// `candidate_extracted_paths_for_game` /
/// `candidate_extracted_dirs_for_game`. Those three install-path
/// derivations are not ported into this crate, so the fallback cannot be
/// expressed here yet. It only matters when no `Emulators`-platform row
/// matches the emulator by free text but one is installed AS that
/// emulator.
fn shared_cloud_sync_owner<'a>(
    ctx: &'a CloudContext,
    game: &CloudGame,
    name: &str,
    entry: Option<&EmulatorEntry>,
    save_type: SaveType,
) -> Option<&'a CloudGame> {
    if scope_for(ctx, name, entry, &game.platform, save_type) == SaveScope::PerGame {
        return None;
    }
    let token = shared_sync_token(ctx, name, entry)?;
    shared_sync_owner(token, ctx.all_games)
}

// ---------------------------------------------------------------------
// Candidate discovery — the ten-branch dispatch
// ---------------------------------------------------------------------

/// `_cloud_sync_targets_for_game` (cloud_mixin.py:506-616): the
/// ten-branch dispatch table from doc 06 "Candidate discovery", the
/// explicit-file-root rescan, and session filtering. Returns
/// `(files, folder_targets)`.
///
/// D1: xemu contributes NO generic candidates for saves — its save data
/// lives inside the HDD image, which [`upload`] reads directly via
/// `xemu_sync::build_xemu_save_archive`.
pub fn cloud_sync_targets(
    ctx: &CloudContext,
    caches: &mut CloudCaches,
    game: &CloudGame,
    entry: &EmulatorEntry,
    save_type: SaveType,
) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let name = entry.name.trim().to_string();
    let (directories, explicit_file_roots) =
        resolved_sync_dirs(ctx, caches, entry, path_key_for(save_type));
    cloud_sync_targets_in(
        ctx,
        game,
        entry,
        &name,
        &directories,
        &explicit_file_roots,
        save_type,
    )
}

#[allow(clippy::too_many_arguments)]
fn cloud_sync_targets_in(
    ctx: &CloudContext,
    game: &CloudGame,
    entry: &EmulatorEntry,
    name: &str,
    directories: &[PathBuf],
    explicit_file_roots: &[PathBuf],
    save_type: SaveType,
) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let entry_ref = Some(entry);
    let profile = profile_for(ctx, entry);
    let ignore = ignore_for(ctx, entry, name, save_type);
    let strategy = resolved_save_strategy(Some(entry), profile, save_type);
    let tokens = game_save_match_tokens(game);
    let window = window_for(ctx, game);

    if save_type == SaveType::State {
        let files = file_candidates(
            directories,
            &tokens,
            SaveType::State,
            &ignore,
            explicit_file_roots,
        );
        return (session_filtered_file_candidates(files, window), Vec::new());
    }

    // D1: xemu saves live inside the raw HDD image.
    if is_xemu(ctx, name, entry_ref) {
        return (Vec::new(), Vec::new());
    }

    let mut files: Vec<PathBuf> = Vec::new();
    let mut folder_targets: Vec<PathBuf> = Vec::new();

    if is_cemu(ctx, name, entry_ref) {
        folder_targets = cemu_save_directories(directories, &tokens, &ignore);
    } else if is_dolphin(ctx, name, entry_ref) {
        files = file_candidates(
            directories,
            &tokens,
            SaveType::Save,
            &ignore,
            explicit_file_roots,
        );
        folder_targets = directory_candidates(directories, &tokens, &ignore);
    } else if strategy == "folder" {
        folder_targets = directory_candidates(directories, &tokens, &ignore);
    } else if is_retroarch(ctx, name, entry_ref)
        && scope_for(ctx, name, entry_ref, &game.platform, SaveType::Save)
            == SaveScope::SharedSlotted
    {
        let vmu_files = readers::flycast_vmu_file_candidates(directories);
        let scan: &[PathBuf] = if vmu_files.is_empty() {
            directories
        } else {
            &vmu_files
        };
        files = file_candidates(scan, &tokens, SaveType::Save, &ignore, explicit_file_roots);
    } else if strategy == "single_file" {
        files = file_candidates(
            directories,
            &tokens,
            SaveType::Save,
            &ignore,
            explicit_file_roots,
        );
    } else if is_ppsspp(ctx, name, entry_ref) {
        folder_targets = ppsspp_save_directories(directories, &psp_id_tokens(game));
    } else if is_rpcs3(ctx, name, entry_ref) {
        folder_targets = rpcs3_save_directories(directories, &ps3_id_tokens(game));
    } else if is_pcsx2(ctx, name, entry_ref) {
        folder_targets = pcsx2_save_directories(directories, &ps2_serial_tokens(game), &ignore);
    } else {
        files = file_candidates(
            directories,
            &tokens,
            SaveType::Save,
            &ignore,
            explicit_file_roots,
        );
    }

    if files.is_empty() && folder_targets.is_empty() && !explicit_file_roots.is_empty() {
        files = file_candidates(
            explicit_file_roots,
            &tokens,
            SaveType::Save,
            &ignore,
            explicit_file_roots,
        );
    }

    if !files.is_empty() {
        files = session_filtered_file_candidates(files, window);
    }
    if !folder_targets.is_empty() {
        folder_targets = session_filtered_directory_candidates(folder_targets, window, &ignore);
    }

    (files, folder_targets)
}

// ---------------------------------------------------------------------
// Local mtimes
// ---------------------------------------------------------------------

fn entry_or_stub(ctx: &CloudContext, entry_name: &str) -> EmulatorEntry {
    emulator_entry_by_name(&ctx.config.emulators, entry_name)
        .cloned()
        .unwrap_or_else(|| stub_entry(entry_name))
}

/// `_latest_local_save_mtime_for_game` (cloud_mixin.py:1559-1589): the
/// maximum over file candidates' own mtimes and the newest non-blocked
/// file under each folder target.
///
/// D1: for xemu the raw HDD image file's own mtime stands in — there are
/// no generic candidates to scan.
pub fn latest_local_save_mtime(
    ctx: &CloudContext,
    caches: &mut CloudCaches,
    game: &CloudGame,
    entry_name: &str,
) -> f64 {
    let entry = entry_or_stub(ctx, entry_name);
    let (directories, explicit_file_roots) =
        resolved_sync_dirs(ctx, caches, &entry, PathKey::SavePaths);

    if is_xemu(ctx, entry_name, Some(&entry)) {
        let hdd = xemu_hdd_path_from_config(&entry.path).unwrap_or_default();
        if hdd.trim().is_empty() {
            return 0.0;
        }
        return latest_mtime_under(Path::new(hdd.trim()), &IgnoreSets::default());
    }

    if directories.is_empty() {
        return 0.0;
    }

    let (files, folder_targets) = cloud_sync_targets_in(
        ctx,
        game,
        &entry,
        entry_name,
        &directories,
        &explicit_file_roots,
        SaveType::Save,
    );
    let ignore = ignore_for(ctx, &entry, entry_name, SaveType::Save);

    let mut latest = 0.0_f64;
    for candidate in &files {
        latest = latest.max(super::file_mtime_secs(candidate).unwrap_or(0.0));
    }
    for directory in &folder_targets {
        latest = latest.max(latest_mtime_under(directory, &ignore));
    }
    latest
}

/// `_latest_local_state_mtime_for_game` (cloud_mixin.py:1534-1557):
/// `0.0` for RPCS3, else the maximum over state file candidates.
pub fn latest_local_state_mtime(
    ctx: &CloudContext,
    caches: &mut CloudCaches,
    game: &CloudGame,
    entry_name: &str,
) -> f64 {
    let entry = entry_or_stub(ctx, entry_name);
    let (directories, explicit_file_roots) =
        resolved_sync_dirs(ctx, caches, &entry, PathKey::StatePaths);
    if directories.is_empty() {
        return 0.0;
    }
    if is_rpcs3(ctx, entry_name, Some(&entry)) {
        return 0.0;
    }

    let (files, _) = cloud_sync_targets_in(
        ctx,
        game,
        &entry,
        entry_name,
        &directories,
        &explicit_file_roots,
        SaveType::State,
    );
    let mut latest = 0.0_f64;
    for candidate in &files {
        latest = latest.max(super::file_mtime_secs(candidate).unwrap_or(0.0));
    }
    latest
}

// ---------------------------------------------------------------------
// Server record listing / deletion
// ---------------------------------------------------------------------

/// `_server_save_records_for_rom` (:1592) / `_server_state_records_for_rom`
/// (:1650-1665). State listings drop records whose `file_name` ends with a
/// supported image extension, so screenshot assets are never treated as
/// states.
pub async fn fetch_cloud_records(
    client: &RommClient,
    ctx: &CloudContext<'_>,
    caches: &mut CloudCaches,
    game: &CloudGame,
    save_type: SaveType,
) -> Result<Vec<Value>, String> {
    let Some(rom_id) = cloud_sync_rom_id(ctx, caches, game, save_type) else {
        return Err(MISSING_ROM_ID.to_string());
    };
    fetch_cloud_records_for_rom(client, &rom_id, save_type).await
}

pub(crate) async fn fetch_cloud_records_for_rom(
    client: &RommClient,
    rom_id: &str,
    save_type: SaveType,
) -> Result<Vec<Value>, String> {
    let payload = match save_type {
        SaveType::Save => client.saves_for_rom(rom_id).await,
        SaveType::State => client.states_for_rom(rom_id).await,
    }
    .map_err(|e| e.to_string())?;
    let records = super::restore::server_records_from_payload(&payload);
    if save_type == SaveType::Save {
        return Ok(records);
    }
    Ok(records
        .into_iter()
        .filter(|record| {
            let file_name = record_str(record, "file_name").to_lowercase();
            file_name.is_empty()
                || !SUPPORTED_IMAGE_EXTENSIONS
                    .iter()
                    .any(|ext| file_name.ends_with(ext))
        })
        .collect())
}

/// `POST /api/{saves,states}/delete` for one record. HTTP 404/410 count as
/// a successful deletion, matching retention pruning
/// (cloud_mixin.py:1752-1758).
pub async fn delete_cloud_record(
    client: &RommClient,
    save_type: SaveType,
    id: i64,
) -> Result<(), String> {
    let status = match save_type {
        SaveType::Save => client.delete_save(id).await,
        SaveType::State => client.delete_state(id).await,
    }
    .map_err(|e| e.to_string())?;
    if (200..300).contains(&status) || status == 404 || status == 410 {
        return Ok(());
    }
    Err(format!("Server returned HTTP {status}."))
}

// ---------------------------------------------------------------------
// Panel gate
// ---------------------------------------------------------------------

/// `_details_cloud_mode_supported` (cloud_mixin.py:296-373). `installed`
/// is Python's `self._installed_game_record(game) is not None`, which
/// [`CloudGame`] does not carry.
pub fn details_cloud_mode_supported(
    ctx: &CloudContext,
    caches: &mut CloudCaches,
    game: &CloudGame,
    save_type: SaveType,
    installed: bool,
) -> bool {
    if is_native_executable_platform(&game.platform) {
        if save_type == SaveType::State {
            return false;
        }
        return installed;
    }

    let emulators_platform = is_emulators_platform(&game.platform);
    if !installed && !emulators_platform {
        return false;
    }

    let (name, entry) = resolved_cloud_emulator_pair(ctx, caches, game, save_type);
    let Some(entry) = entry else {
        return false;
    };

    let scope = scope_for(ctx, &name, Some(&entry), &game.platform, save_type);
    if save_type == SaveType::Save && emulators_platform && scope == SaveScope::PerGame {
        return false;
    }
    if save_type == SaveType::State && (emulators_platform || is_rpcs3(ctx, &name, Some(&entry))) {
        return false;
    }
    // The BASE reason only (fix round 1): an xemu image problem must not
    // hide the panel — the user needs it to read the guidance.
    if !base_block_reason_for_game(ctx, game, save_type, Some(&entry)).is_empty() {
        return false;
    }

    let (directories, _) = resolved_sync_dirs(ctx, caches, &entry, path_key_for(save_type));
    !directories.is_empty()
}

// ---------------------------------------------------------------------
// Per-record restore-enabled gate (manual "Restore" button)
// ---------------------------------------------------------------------

/// `_details_cloud_restore_enabled` (`details_view_mixin.py:628-703`):
/// for ONE server record and save type, whether the manual "Restore"
/// button should be enabled, and the text to pair with it either way —
/// a refusal reason when disabled, or the shared-scope notice (possibly
/// empty) as a tooltip when enabled. Pure and deterministic for a given
/// `(ctx, game, save_type, record)`. Python memoizes this per UI
/// request, keyed `(save_type, game key, emulator name lowercased)` —
/// that memoization is the CALLER's job (this crate has no
/// request-lifetime object to hang it on); this function is the pure
/// computation the cache would store.
pub fn restore_enabled_for_record(
    ctx: &CloudContext,
    caches: &mut CloudCaches,
    game: &CloudGame,
    save_type: SaveType,
    record: &Value,
) -> (bool, String) {
    let record_emulator = record_str(record, "emulator");
    // Native multi-directory saves carry no per-emulator compatibility
    // question at all (`tests/test_details_cloud_native_panel.py:100`).
    if record_emulator == "native_multi_dir" {
        return (true, String::new());
    }

    let (resolved_name, resolved_entry) =
        resolved_cloud_emulator_pair(ctx, caches, game, save_type);
    let compatibility_name = if record_emulator.is_empty() {
        resolved_name.clone()
    } else {
        record_emulator.clone()
    };
    let record_entry = if record_emulator.is_empty() {
        None
    } else {
        emulator_entry_by_name(&ctx.config.emulators, &record_emulator).cloned()
    };
    let compatibility_entry = record_entry.clone().or_else(|| resolved_entry.clone());

    // `_cloud_save_block_reason_for_game`'s OWN name resolution, ported
    // directly rather than going through the public `block_reason_for_game`
    // wrapper: that wrapper's `wrapper_emulator_name` derives the name
    // FROM the entry, but this call site needs the opposite — a record
    // naming an emulator that isn't configured locally still runs the
    // platform/xemu/redream checks against the record's RAW name, even
    // though the ENTRY it is paired with here is the resolved default's.
    let flags = block_reason_flags(
        ctx,
        &compatibility_name,
        compatibility_entry.as_ref(),
        &game.platform,
    );
    let compatibility_reason = cloud_save_block_reason(
        &game.platform,
        save_type,
        &compatibility_name,
        flags.as_ref(),
    );
    if !compatibility_reason.is_empty() {
        return (false, compatibility_reason);
    }

    let shared_notice = scope_notice_for_game(
        ctx,
        game,
        save_type,
        &compatibility_name,
        compatibility_entry.as_ref(),
    );

    if save_type == SaveType::State
        && !record_emulator.is_empty()
        && is_rpcs3(ctx, &record_emulator, record_entry.as_ref())
    {
        return (
            false,
            "RPCS3 savestate restore is not supported yet.".to_string(),
        );
    }

    let mut emulator_name = record_emulator.clone();
    let mut emulator_entry = record_entry;
    if !record_emulator.is_empty() && emulator_entry.is_none() {
        return (
            false,
            format!("Configure emulator '{record_emulator}' in Emulators to restore this entry."),
        );
    }
    if emulator_entry.is_none() {
        emulator_name = resolved_name;
        emulator_entry = resolved_entry;
        if emulator_entry.is_none() {
            return (
                false,
                "No default emulator is configured for this platform.".to_string(),
            );
        }
    }

    let entry = emulator_entry.expect("checked immediately above");
    let (directories, _) = resolved_sync_dirs(ctx, caches, &entry, path_key_for(save_type));
    if directories.is_empty() {
        return (
            false,
            format!(
                "No configured {} directories were found for emulator '{}'.",
                kind_label(save_type),
                emulator_name
            ),
        );
    }

    (true, shared_notice)
}

/// `_details_cloud_scope_notice` (`cloud_mixin.py:255-291`), specialized
/// to the `(name, entry)` pair [`restore_enabled_for_record`] already
/// resolved (Python's own blank-name/blank-entry re-resolution branch
/// never triggers from that call site, since it always passes a
/// non-blank pair through). States never carry a notice.
fn scope_notice_for_game(
    ctx: &CloudContext,
    game: &CloudGame,
    save_type: SaveType,
    name: &str,
    entry: Option<&EmulatorEntry>,
) -> String {
    if save_type != SaveType::Save || entry.is_none() {
        return String::new();
    }
    let scope = scope_for(ctx, name, entry, &game.platform, save_type);
    let label = if name.trim().is_empty() {
        "this emulator"
    } else {
        name.trim()
    };
    match scope {
        SaveScope::SharedSingle => format!(
            "These cloud saves are shared {label} media. Restoring or deleting one affects every game using this emulator."
        ),
        SaveScope::SharedSlotted => format!(
            "These cloud saves are shared {label} memory-card backups. Deleting one removes the backup for every game using that emulator slot."
        ),
        SaveScope::PerGame => String::new(),
    }
}

// ---------------------------------------------------------------------
// Shared message strings
// ---------------------------------------------------------------------

pub(crate) const MISSING_ROM_ID: &str = "Missing ROM id for this game.";
pub(crate) const NO_DEFAULT_EMULATOR: &str =
    "No default emulator is configured for this game's platform.";
pub(crate) const RESTORE_SUCCESS: &str = "Cloud save restored successfully.";

pub(crate) fn no_directories_message(save_type: SaveType, emulator_name: &str) -> String {
    format!(
        "No {} directories were found for emulator '{}'. Configure them in Emulators.",
        kind_label(save_type),
        emulator_name
    )
}

#[cfg(test)]
mod tests;
