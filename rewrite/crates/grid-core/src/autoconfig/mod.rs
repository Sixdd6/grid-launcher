//! Emulator autoconfiguration: the `ensure_*` writers that seed an
//! emulator's own settings files so a launched game finds its saves,
//! firmware, controller profile and RetroAchievements login where GRID
//! expects them.
//!
//! Ports `grid_launcher/emulator/*.py`'s `ensure_*` functions. See
//! `docs/porting/05-emulator-autoconfig.md` for the behavior contract; the
//! shared section-writer families live in [`writers`] and the path helpers
//! every module's candidate list is built from live in [`paths`].

pub mod azahar;
pub mod cemu;
pub mod cores;
pub mod dolphin;
pub mod duckstation;
pub mod eden;
pub mod entry;
pub mod paths;
pub mod pcsx2;
pub mod ppsspp;
pub mod readers;
pub mod redream;
pub mod retroarch;
pub mod rpcs3;
pub mod writers;
pub mod xemu;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use secrecy::{ExposeSecret, SecretString};

use crate::config::{Config, ConfigError, EmulatorEntry};
use crate::launch::profiles::{profile_for_entry, EmulatorProfile};
use crate::launch::selection::emulator_entry_by_name;

/// RetroAchievements credentials as GRID holds them for the `ensure_*`
/// writers that log RetroArch (and the other RA-aware emulators) into
/// RetroAchievements.
///
/// `token()` is the ONLY `expose_secret()` call site outside
/// `secrets.rs`/`romm/mod.rs` — `scripts/check_secret_hygiene.sh` allowlists
/// this file for exactly that reason. `Debug` is hand-written rather than
/// derived so the redaction is stated here rather than inherited from
/// `SecretString`, and can never regress if the field type changes.
#[derive(Clone)]
pub struct RaCredentials {
    username: String,
    token: SecretString,
}

impl RaCredentials {
    pub fn new(username: impl Into<String>, token: impl Into<SecretString>) -> Self {
        Self {
            username: username.into(),
            token: token.into(),
        }
    }

    pub fn username(&self) -> &str {
        &self.username
    }

    /// The token in the clear. Every call site must be a write straight to
    /// disk (retroarch.cfg's `cheevos_token` line) or an equally narrow,
    /// audited sink — never a log, an error, or an IPC payload.
    pub fn token(&self) -> &str {
        self.token.expose_secret()
    }

    /// `Some(self)` only when BOTH fields are non-blank after trimming — the
    /// gate every RA-aware writer applies before it writes a credential
    /// (doc 05 invariant 5; emulator_ui_mixin.py:386 reads both and each
    /// writer re-checks them). `None` means "no RetroAchievements login",
    /// which is not an error.
    pub fn usable(&self) -> Option<&Self> {
        if self.username.trim().is_empty() || self.token().trim().is_empty() {
            return None;
        }
        Some(self)
    }
}

impl std::fmt::Debug for RaCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RaCredentials")
            .field("username", &self.username)
            .field("token", &"[REDACTED]")
            .finish()
    }
}

/// Every `ensure_*` writer's return value.
///
/// Spec deviation D8: Python returns a `str`, a `Path`, or a `dict`
/// depending on the module — a dynamic-typing artifact, not a behavior. One
/// struct carries all three shapes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnsureResult {
    /// True when this call wrote at least one file.
    pub changed: bool,
    /// The primary file the writer targeted. `None` when the writer bailed
    /// out (blank path, missing executable, unreadable or unwritable file).
    pub config_path: Option<PathBuf>,
    /// Secondary files a writer also owns. Documented keys, and only these:
    ///   dolphin -> "gfx_ini_path", "gcpad_ini_path"
    ///   rpcs3   -> "gui_config_path", "current_settings_path", "vfs_path"
    ///   cemu    -> "profile_path"
    ///   ppsspp  -> "ra_token_path"
    pub extras: BTreeMap<String, PathBuf>,
}

impl EnsureResult {
    /// The bail-out result: nothing written, no path to report.
    pub fn unchanged() -> Self {
        Self::default()
    }

    /// A result naming the primary config file the writer targeted.
    pub fn at(path: impl Into<PathBuf>, changed: bool) -> Self {
        Self {
            changed,
            config_path: Some(path.into()),
            extras: BTreeMap::new(),
        }
    }

    /// Record a secondary file this writer owns. Chainable.
    pub fn with_extra(mut self, key: &str, path: impl Into<PathBuf>) -> Self {
        self.extras.insert(key.to_string(), path.into());
        self
    }

    /// Fold another write's outcome in: `self.changed |= other`.
    pub fn merge_changed(&mut self, other: bool) {
        self.changed |= other;
    }
}

// --- emulator identification --------------------------------------------------

/// One candidate value for [`entry_matches_tokens`], normalized the way
/// `add_candidate` does (profiles.py:245-252): trimmed and casefolded, then
/// its file stem added alongside it. Blank values contribute nothing.
fn add_candidate(value: &str, out: &mut BTreeSet<String>) {
    let normalized = value.trim().to_lowercase();
    if normalized.is_empty() {
        return;
    }
    let stem = Path::new(&normalized)
        .file_stem()
        .map(|s| s.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    out.insert(normalized);
    if !stem.is_empty() {
        out.insert(stem);
    }
}

/// `emulator_entry_matches_tokens` (profiles.py:227-275) — the autoprofile
/// half of the token match.
///
/// The candidate set is the entry name, the executable's file name and stem,
/// and (when a profile matches the entry) that profile's name and every one
/// of its `match_tokens` — each of those also contributing its own stem. A
/// token matches when it EQUALS a candidate or is a substring of one.
fn entry_matches_tokens(
    entry: &EmulatorEntry,
    tokens: &[&str],
    profiles: &[EmulatorProfile],
) -> bool {
    let normalized: Vec<String> = tokens
        .iter()
        .map(|token| token.trim().to_lowercase())
        .filter(|token| !token.is_empty())
        .collect();
    if normalized.is_empty() {
        return false;
    }

    let mut candidates: BTreeSet<String> = BTreeSet::new();
    add_candidate(&entry.name, &mut candidates);

    let path_value = entry.path.trim();
    if !path_value.is_empty() {
        let executable = Path::new(path_value);
        if let Some(name) = executable.file_name() {
            add_candidate(&name.to_string_lossy(), &mut candidates);
        }
        if let Some(stem) = executable.file_stem() {
            add_candidate(&stem.to_string_lossy(), &mut candidates);
        }
    }

    if let Some(profile) = profile_for_entry(&entry.name, &entry.path, profiles) {
        add_candidate(&profile.name, &mut candidates);
        for token in &profile.match_tokens {
            add_candidate(token, &mut candidates);
        }
    }

    normalized.iter().any(|token| {
        candidates
            .iter()
            .any(|candidate| candidate == token || candidate.contains(token.as_str()))
    })
}

/// `_emulator_matches_tokens` (cloud_mixin.py:1349-1363): autoprofile token
/// matching on the entry first, then a plain case-folded SUBSTRING test of
/// each token against the entry NAME. So an entry literally named
/// "My DuckStation build" matches `duckstation` with no profile at all.
pub fn emulator_matches_tokens(
    entry: &EmulatorEntry,
    tokens: &[&str],
    profiles: &[EmulatorProfile],
) -> bool {
    if entry_matches_tokens(entry, tokens, profiles) {
        return true;
    }
    let normalized_name = entry.name.trim().to_lowercase();
    tokens.iter().any(|token| {
        let token = token.trim().to_lowercase();
        !token.is_empty() && normalized_name.contains(&token)
    })
}

/// The name-only form the defaults assignment needs
/// (`_is_retroarch_emulator_name` with `emulator=None`,
/// emulator_ui_mixin.py:1916): the entry is looked up by name first, and a
/// name that matches nothing registered falls through to the substring test
/// alone — exactly as `_emulator_matches_tokens` does for a `None` entry.
fn matches_tokens_by_name(
    name: &str,
    tokens: &[&str],
    emulators: &[EmulatorEntry],
    profiles: &[EmulatorProfile],
) -> bool {
    if let Some(entry) = emulator_entry_by_name(emulators, name) {
        if entry_matches_tokens(entry, tokens, profiles) {
            return true;
        }
    }
    let normalized_name = name.trim().to_lowercase();
    tokens.iter().any(|token| {
        let token = token.trim().to_lowercase();
        !token.is_empty() && normalized_name.contains(&token)
    })
}

/// `_is_retroarch_emulator_name` (emulator_ui_mixin.py:1916).
pub fn is_retroarch(entry: &EmulatorEntry, profiles: &[EmulatorProfile]) -> bool {
    emulator_matches_tokens(entry, &["retroarch"], profiles)
}

/// `_is_duckstation_emulator_name` (emulator_ui_mixin.py:1920).
pub fn is_duckstation(entry: &EmulatorEntry, profiles: &[EmulatorProfile]) -> bool {
    emulator_matches_tokens(entry, &["duckstation"], profiles)
}

/// `_is_xemu_emulator_name` (cloud_mixin.py:1381).
pub fn is_xemu(entry: &EmulatorEntry, profiles: &[EmulatorProfile]) -> bool {
    emulator_matches_tokens(entry, &["xemu"], profiles)
}

/// `_is_pcsx2_emulator_name` (cloud_mixin.py:1378).
pub fn is_pcsx2(entry: &EmulatorEntry, profiles: &[EmulatorProfile]) -> bool {
    emulator_matches_tokens(entry, &["pcsx2"], profiles)
}

/// `_is_dolphin_emulator_name` (cloud_mixin.py:1372).
pub fn is_dolphin(entry: &EmulatorEntry, profiles: &[EmulatorProfile]) -> bool {
    emulator_matches_tokens(entry, &["dolphin"], profiles)
}

/// `_is_azahar_emulator_name` (cloud_mixin.py:1369).
pub fn is_azahar(entry: &EmulatorEntry, profiles: &[EmulatorProfile]) -> bool {
    emulator_matches_tokens(entry, &["azahar"], profiles)
}

/// `_is_eden_emulator_name` (cloud_mixin.py:1384).
pub fn is_eden(entry: &EmulatorEntry, profiles: &[EmulatorProfile]) -> bool {
    emulator_matches_tokens(entry, &["eden"], profiles)
}

/// `_is_rpcs3_emulator_name` (install_mixin.py:410): the token match OR-ed
/// with the standalone `is_rpcs3_emulator_name` name check
/// (selection.py:153-154), which is `"rpcs3" in name.strip().casefold()`.
/// The OR arm is redundant with [`emulator_matches_tokens`]'s own substring
/// fallback; it is kept so the port reads against the reference line for
/// line and stays correct if either half ever changes.
pub fn is_rpcs3(entry: &EmulatorEntry, profiles: &[EmulatorProfile]) -> bool {
    emulator_matches_tokens(entry, &["rpcs3"], profiles)
        || entry.name.trim().to_lowercase().contains("rpcs3")
}

/// `_is_ppsspp_emulator_name` (cloud_mixin.py:1366).
pub fn is_ppsspp(entry: &EmulatorEntry, profiles: &[EmulatorProfile]) -> bool {
    emulator_matches_tokens(entry, &["ppsspp"], profiles)
}

/// `_is_cemu_emulator_name` (cloud_mixin.py:1375).
pub fn is_cemu(entry: &EmulatorEntry, profiles: &[EmulatorProfile]) -> bool {
    emulator_matches_tokens(entry, &["cemu"], profiles)
}

/// `_is_redream_emulator_name` (cloud_mixin.py:1387).
pub fn is_redream(entry: &EmulatorEntry, profiles: &[EmulatorProfile]) -> bool {
    emulator_matches_tokens(entry, &["redream"], profiles)
}

/// Spec deviation D2 (RA-keys-only fan-out) — no direct Python counterpart:
/// the RA-capable predicates, in dispatch order. DuckStation is
/// deliberately NOT here even though it takes RetroAchievements-adjacent
/// suppression keys: `ensure_duckstation_memory_card_settings` takes no
/// credential parameters and writes only `[Cheevos]` suppression keys, never
/// `Username`/`Token` — DuckStation encrypts its token per machine, so
/// pre-filling is not possible (doc 05 open question, ruled: follow the
/// code).
pub fn ra_capable(entry: &EmulatorEntry, profiles: &[EmulatorProfile]) -> bool {
    is_retroarch(entry, profiles) || is_pcsx2(entry, profiles) || is_ppsspp(entry, profiles)
}

/// Spec deviation D2 (RA-keys-only fan-out) — no direct Python counterpart:
/// `_on_ra_login_finished` (grid-launcher.py:2730-2754) re-runs the FULL
/// `_ensure_emulator_sync_settings` for every registered emulator after a
/// RetroAchievements login. Doing that here would re-apply every managed
/// key (save directories, fullscreen, controller profiles, ...) just to
/// deliver a credential pair, so this fan-out instead calls only the three
/// narrow `ensure_ra_credentials` writers — one per [`ra_capable`] entry,
/// in config order. `None` (from [`RaCredentials::usable`]) short-circuits
/// to an empty vec before touching any entry: a blank pair writes nothing,
/// matching every narrow writer's own gate.
pub fn fan_out_ra_credentials(
    config: &Config,
    profiles: &[EmulatorProfile],
    ra: &RaCredentials,
) -> Vec<(String, bool)> {
    let Some(ra) = ra.usable() else {
        return Vec::new();
    };

    let mut rows = Vec::new();
    for entry in &config.emulators {
        let path = entry.path.trim();
        if is_retroarch(entry, profiles) {
            let result = retroarch::ensure_ra_credentials(path, ra);
            rows.push((entry.name.clone(), result.changed));
        }
        if is_pcsx2(entry, profiles) {
            let result = pcsx2::ensure_ra_credentials(path, ra);
            rows.push((entry.name.clone(), result.changed));
        }
        if is_ppsspp(entry, profiles) {
            let result = ppsspp::ensure_ra_credentials(path, ra);
            rows.push((entry.name.clone(), result.changed));
        }
    }
    rows
}

// --- layer 2 orchestration ------------------------------------------------------

/// Everything [`sync_new_emulator`] needs that grid-core cannot derive from
/// the config file alone, assembled by the caller.
pub struct SyncContext<'a> {
    /// The config file to load, mutate and save.
    pub config_path: &'a Path,
    /// Assignable server platform names (already filtered by
    /// [`entry::assignable_platforms`]). Empty when no session is connected,
    /// which makes the platform-defaults step a no-op — matching the
    /// reference's behavior with an empty platform list.
    pub platforms: &'a [String],
    /// `<library>/PlayStation 3`, or `""` when no library path is set. Build
    /// it with [`ps3_library_path`].
    pub ps3_library_path: String,
    /// The RetroAchievements pair, when the user has one.
    pub ra: Option<RaCredentials>,
    /// The autoprofile catalog entry matching and the defaults backfill both
    /// resolve against.
    pub profiles: &'a [EmulatorProfile],
}

/// What one [`sync_new_emulator`] pass produced. Every field is diagnostic
/// only; a caller turns a non-empty `warnings` into a user-visible warning
/// line. Neither field can carry a credential: `wrote` holds file paths and
/// `warnings` holds emulator and writer names.
#[derive(Debug, Default)]
pub struct SyncReport {
    /// The primary config files the writers actually changed.
    pub wrote: Vec<String>,
    /// One line per writer that reached no file at all.
    pub warnings: Vec<String>,
}

/// The RPCS3 PS3 library path: `<library_path>/PlayStation 3` with `~`
/// expanded, or `""` when no library path is configured
/// (emulator_ui_mixin.py:424).
pub fn ps3_library_path(library_path: &str) -> String {
    let trimmed = library_path.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    paths::expand_user(trimmed)
        .join("PlayStation 3")
        .to_string_lossy()
        .into_owned()
}

/// `_installed_retroarch_cores_for_platform` (emulator_ui_mixin.py:547-559),
/// resolved against the config's own emulator list: the platform's
/// compatible cores narrowed to the ones actually installed next to that
/// emulator's executable, and `[]` when none are.
///
/// The reference's slug-map fast path (emulator_ui_mixin.py:568) needs the
/// server's platform slugs, which grid-core does not hold; the fuzzy
/// [`cores::cores_for_platform`] fallback the reference uses offline is the
/// only branch here.
fn installed_cores_for_platform(
    platform: &str,
    emulator_name: &str,
    emulators: &[EmulatorEntry],
    compat: &cores::CompatMap,
) -> Vec<String> {
    let Some(entry) = emulator_entry_by_name(emulators, emulator_name) else {
        return Vec::new();
    };
    let installed = cores::installed_core_ids(&entry.path, None);
    if installed.is_empty() {
        return Vec::new();
    }
    cores::cores_for_platform(platform, compat)
        .into_iter()
        .filter(|core| installed.contains(core))
        .collect()
}

/// Fold one writer's outcome into `report`.
///
/// A writer REACHED its target when it names any file at all — `config_path`,
/// or, for a writer whose only target is a secondary file, an `extras` path.
/// [`cemu::ensure_controller_config`] is the one such writer: it always
/// reports `config_path = None` and names `extras["profile_path"]` instead,
/// so on its healthy idempotent branch (the profile already exists, nothing
/// to write) it returns `changed = false` with `config_path = None`. Testing
/// `config_path` alone would misread that as a failure.
///
/// So only a writer that named NOTHING and changed nothing bailed out; that
/// becomes a warning naming the emulator and the writer. Anything a writer
/// actually changed is recorded in `wrote`, falling back to the `extras`
/// paths when there is no primary. Nothing here aborts the remaining writers.
fn record(report: &mut SyncReport, emulator: &str, writer: &str, result: EnsureResult) {
    let named_a_file = result.config_path.is_some() || !result.extras.is_empty();
    if !named_a_file && !result.changed {
        report
            .warnings
            .push(format!("could not configure {emulator} ({writer})"));
        return;
    }
    if !result.changed {
        return;
    }
    match result.config_path {
        Some(path) => report.wrote.push(path.to_string_lossy().into_owned()),
        None => report.wrote.extend(
            result
                .extras
                .values()
                .map(|path| path.to_string_lossy().into_owned()),
        ),
    }
}

/// The D1 entry point: entry autoconfig, the defaults backfill, and the
/// native `ensure_*` writers for ONE newly created emulator entry. Loads,
/// mutates and saves the config itself.
///
/// Ports `_ensure_emulator_sync_settings` (emulator_ui_mixin.py:365-440)
/// minus the session cache, which D1 moots by binding this to the two
/// new-entry call sites only (catalog install finalize, manual add) rather
/// than to every launch.
///
/// Order: load the config and find `entry_name` (an EXACT match — a miss
/// returns an empty report and saves nothing); apply the matched profile's
/// manual-entry defaults (autoconfig.py:228, a no-op when no profile
/// matches); run [`entry::backfill_missing_defaults`] (D3); save ONCE, so a
/// writer failure cannot lose the entry work; then trim the entry's path and
/// return immediately when it is blank (doc 05 invariant 1). The dispatch
/// that follows is a FLAT SEQUENCE OF INDEPENDENT `if`s, not a chain, so an
/// entry name matching two predicates runs both writers.
///
/// Never returns `Err` for a writer failure — those land in
/// `report.warnings`. The only `Err` is a config load or save failure.
pub fn sync_new_emulator(entry_name: &str, ctx: &SyncContext) -> Result<SyncReport, ConfigError> {
    let mut report = SyncReport::default();
    let mut config = Config::load(ctx.config_path)?;

    let Some(index) = config
        .emulators
        .iter()
        .position(|existing| existing.name == entry_name)
    else {
        return Ok(report);
    };

    // Layer 1 for this entry. The resolved-profile miss case is a
    // pass-through: autoconfig.py:228 resolves the profile internally and
    // does nothing when there is none.
    if let Some(profile) = profile_for_entry(
        &config.emulators[index].name,
        &config.emulators[index].path,
        ctx.profiles,
    ) {
        config.emulators[index] =
            entry::apply_manual_emulator_profile_defaults(&config.emulators[index], profile);
    }

    // D3: the defaults backfill runs at the same two points, right after the
    // entry autoconfig. Both closures read a snapshot of the entry list taken
    // after the layer-1 pass, so they see the entry as it will be saved.
    let snapshot = config.emulators.clone();
    let compat = cores::compatibility_map();
    let installed_cores = |platform: &str, emulator_name: &str| -> Vec<String> {
        installed_cores_for_platform(platform, emulator_name, &snapshot, compat)
    };
    let is_retroarch_name = |name: &str| -> bool {
        matches_tokens_by_name(name, &["retroarch"], &snapshot, ctx.profiles)
    };
    let defaults_ctx = entry::DefaultsContext {
        platforms: ctx.platforms,
        installed_cores: &installed_cores,
        is_retroarch: &is_retroarch_name,
    };
    entry::backfill_missing_defaults(&mut config, ctx.profiles, &defaults_ctx);
    config.save(ctx.config_path)?;

    // The RomM username feeds ONLY RetroArch's netplay nickname
    // (emulator_ui_mixin.py:373), and is read before the path check.
    let romm_username = config.username.trim().to_string();
    let emulator_name = config.emulators[index].name.clone();
    let path_text = config.emulators[index].path.trim().to_string();
    if path_text.is_empty() {
        return Ok(report);
    }

    // The reference builds a synthetic `{"name", "path"}` entry for the
    // predicates rather than passing the registered one
    // (emulator_ui_mixin.py:385); the predicates read no other field.
    let subject = EmulatorEntry {
        name: emulator_name.clone(),
        path: path_text.clone(),
        ..Default::default()
    };
    let name = emulator_name.as_str();
    let path = path_text.as_str();
    let profiles = ctx.profiles;
    let ra = ctx.ra.as_ref().and_then(RaCredentials::usable);

    if is_retroarch(&subject, profiles) {
        record(
            &mut report,
            name,
            "retroarch",
            retroarch::ensure_settings(path, true, &romm_username, ra),
        );
    }
    if is_duckstation(&subject, profiles) {
        record(
            &mut report,
            name,
            "duckstation",
            duckstation::ensure_memory_card_settings(path, true),
        );
    }
    if is_xemu(&subject, profiles) {
        record(&mut report, name, "xemu", xemu::ensure_settings(path));
    }
    if is_pcsx2(&subject, profiles) {
        // No `bios_directory` — D6.
        record(
            &mut report,
            name,
            "pcsx2",
            pcsx2::ensure_settings(path, true, ra),
        );
    }
    if is_dolphin(&subject, profiles) {
        record(&mut report, name, "dolphin", dolphin::ensure_settings(path));
    }
    if is_azahar(&subject, profiles) {
        record(&mut report, name, "azahar", azahar::ensure_settings(path));
    }
    if is_eden(&subject, profiles) {
        record(&mut report, name, "eden", eden::ensure_settings(path));
    }
    if is_rpcs3(&subject, profiles) {
        // No background firmware download — D7.
        record(
            &mut report,
            name,
            "rpcs3",
            rpcs3::ensure_settings(path, &ctx.ps3_library_path),
        );
    }
    if is_ppsspp(&subject, profiles) {
        record(
            &mut report,
            name,
            "ppsspp",
            ppsspp::ensure_settings(path, ra),
        );
    }
    if is_cemu(&subject, profiles) {
        record(&mut report, name, "cemu", cemu::ensure_settings(path));
        record(
            &mut report,
            name,
            "cemu controller",
            cemu::ensure_controller_config(path),
        );
    }
    if is_redream(&subject, profiles) {
        record(&mut report, name, "redream", redream::ensure_settings(path));
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, EmulatorEntry};
    use crate::launch::profiles::EmulatorProfile;
    use crate::test_env::{lock, EnvGuard};
    use std::path::Path;

    // --- orchestration fixtures ---------------------------------------------

    fn entry(name: &str, path: &str) -> EmulatorEntry {
        EmulatorEntry {
            name: name.to_string(),
            path: path.to_string(),
            ..Default::default()
        }
    }

    fn profile(name: &str, tokens: &[&str]) -> EmulatorProfile {
        EmulatorProfile {
            name: name.to_string(),
            match_tokens: tokens.iter().map(|t| t.to_string()).collect(),
            ..Default::default()
        }
    }

    /// An empty file at `path`, parent directories created.
    fn touch(path: &Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"").unwrap();
    }

    /// `XDG_CONFIG_HOME`/`XDG_DATA_HOME`/`HOME` all pointed at `dir`, so a
    /// writer whose candidate list reaches into the home directory can never
    /// touch the developer's real one.
    fn isolated(dir: &Path) -> EnvGuard {
        let dir_str = dir.to_str().unwrap();
        EnvGuard::set(&[
            ("XDG_CONFIG_HOME", Some(dir_str)),
            ("XDG_DATA_HOME", Some(dir_str)),
            ("HOME", Some(dir_str)),
        ])
    }

    /// Writes `config` to `<dir>/config.toml` and returns that path.
    fn write_config(dir: &Path, config: &Config) -> PathBuf {
        let path = dir.join("config.toml");
        config.save(&path).unwrap();
        path
    }

    /// A config carrying `entries` plus `library_path` under `dir`.
    fn config_with(dir: &Path, entries: Vec<EmulatorEntry>) -> Config {
        Config {
            library_path: dir.join("library").to_string_lossy().into_owned(),
            emulators: entries,
            ..Default::default()
        }
    }

    #[test]
    fn ensure_result_unchanged_is_all_default() {
        let result = EnsureResult::unchanged();
        assert!(!result.changed);
        assert_eq!(result.config_path, None);
        assert!(result.extras.is_empty());
    }

    #[test]
    fn ensure_result_at_records_path_and_extras() {
        let result =
            EnsureResult::at("/tmp/PCSX2.ini", true).with_extra("gfx_ini_path", "/tmp/GFX.ini");
        assert!(result.changed);
        assert_eq!(result.config_path, Some(PathBuf::from("/tmp/PCSX2.ini")));
        assert_eq!(
            result.extras.get("gfx_ini_path"),
            Some(&PathBuf::from("/tmp/GFX.ini"))
        );
    }

    /// The writers are consumed from sibling modules (one per emulator), so
    /// `desired!` must resolve through the crate root, not only inside
    /// `writers.rs` where `macro_rules!` puts it in textual scope.
    #[test]
    fn desired_macro_is_usable_from_another_module() {
        let want = crate::desired![("Key", "value"), ("Other", "2"),];
        assert_eq!(
            want,
            vec![
                ("Key".to_string(), "value".to_string()),
                ("Other".to_string(), "2".to_string()),
            ]
        );
        let empty: writers::Desired = crate::desired![];
        assert!(empty.is_empty());
    }

    #[test]
    fn ra_credentials_debug_redacts_the_token() {
        let ra = RaCredentials::new("sixdd6", "FAKE-TEST-TOKEN-not-real");
        let debug = format!("{ra:?}");
        assert!(!debug.contains("FAKE-TEST-TOKEN-not-real"), "leak: {debug}");
        assert!(debug.contains("sixdd6"), "username should still print");
    }

    #[test]
    fn ra_credentials_accessors_round_trip() {
        let ra = RaCredentials::new("sixdd6", "FAKE-TEST-TOKEN-not-real");
        assert_eq!(ra.username(), "sixdd6");
        assert_eq!(ra.token(), "FAKE-TEST-TOKEN-not-real");
    }

    #[test]
    fn ensure_result_merge_changed_is_sticky() {
        let mut result = EnsureResult::at("/tmp/x.ini", false);
        result.merge_changed(false);
        assert!(!result.changed);
        result.merge_changed(true);
        assert!(result.changed);
        result.merge_changed(false);
        assert!(result.changed, "merge_changed must never clear a set flag");
    }

    #[test]
    fn ra_credentials_usable_requires_both_fields() {
        let both = RaCredentials::new("sixdd6", "FAKE-TEST-TOKEN-not-real");
        assert!(both.usable().is_some());

        for (user, token) in [
            ("", "FAKE-TEST-TOKEN-not-real"),
            ("sixdd6", ""),
            ("  ", "  "),
        ] {
            let creds = RaCredentials::new(user, token);
            assert!(
                creds.usable().is_none(),
                "blank halves must gate the pair out: {user:?}"
            );
        }
    }

    // --- predicates ---------------------------------------------------------

    #[test]
    fn matches_tokens_by_profile_then_by_substring() {
        // Stage 1: the matched profile's own name and match tokens.
        let profiles = vec![profile("DuckStation", &["duckstation*"])];
        let by_profile = entry("Emu", "/opt/duckstation/duckstation-qt.AppImage");
        assert!(is_duckstation(&by_profile, &profiles));

        // Stage 2: the plain casefolded substring test on the entry NAME,
        // with no profile in play at all.
        let by_name = entry("My DuckStation build", "");
        assert!(is_duckstation(&by_name, &[]));

        // And a name that carries neither matches nothing.
        assert!(!is_duckstation(&entry("Flycast", "/opt/flycast"), &[]));
    }

    #[test]
    fn is_rpcs3_matches_the_standalone_name_check() {
        // No profile, no path — only `is_rpcs3_emulator_name`'s casefolded
        // "rpcs3" in the name can carry this.
        assert!(is_rpcs3(&entry("RPCS3-nightly", ""), &[]));
        assert!(is_rpcs3(&entry("  rpcs3  ", ""), &[]));
        assert!(!is_rpcs3(&entry("PCSX2", ""), &[]));
    }

    // --- sync_new_emulator ---------------------------------------------------

    #[test]
    fn sync_returns_empty_for_an_unknown_entry_name() {
        let _lock = lock();
        let temp = tempfile::tempdir().unwrap();
        let _env = isolated(temp.path());

        let exe = temp.path().join("PCSX2").join("pcsx2.AppImage");
        touch(&exe);
        let config = config_with(temp.path(), vec![entry("PCSX2", exe.to_str().unwrap())]);
        let config_path = write_config(temp.path(), &config);

        let mut pcsx2 = profile("PCSX2", &["pcsx2*"]);
        pcsx2.save_directories = vec!["~/pcsx2/saves".to_string()];
        let profiles = vec![pcsx2];

        let ctx = SyncContext {
            config_path: &config_path,
            platforms: &[],
            ps3_library_path: String::new(),
            ra: None,
            profiles: &profiles,
        };
        let report = sync_new_emulator("Not An Emulator", &ctx).unwrap();

        assert!(report.wrote.is_empty());
        assert!(report.warnings.is_empty());
        // A miss returns BEFORE the entry work, so nothing was saved either.
        let saved = Config::load(&config_path).unwrap();
        assert_eq!(saved.emulators[0].save_paths, "");
        assert!(!exe.parent().unwrap().join("portable.ini").exists());
    }

    #[test]
    fn sync_returns_before_dispatch_for_a_blank_path() {
        let _lock = lock();
        let temp = tempfile::tempdir().unwrap();
        let _env = isolated(temp.path());

        let config = config_with(temp.path(), vec![entry("PCSX2", "   ")]);
        let config_path = write_config(temp.path(), &config);

        let mut pcsx2 = profile("PCSX2", &["pcsx2*"]);
        pcsx2.save_directories = vec!["~/pcsx2/saves".to_string()];
        let profiles = vec![pcsx2];

        let ctx = SyncContext {
            config_path: &config_path,
            platforms: &[],
            ps3_library_path: String::new(),
            ra: None,
            profiles: &profiles,
        };
        let report = sync_new_emulator("PCSX2", &ctx).unwrap();

        assert!(report.wrote.is_empty());
        assert!(report.warnings.is_empty());
        // The entry work still ran and was saved — only the dispatch is skipped.
        let saved = Config::load(&config_path).unwrap();
        assert_eq!(saved.emulators[0].save_paths, "~/pcsx2/saves");
    }

    #[test]
    fn sync_runs_entry_autoconfig_then_backfill_then_writers() {
        let _lock = lock();
        let temp = tempfile::tempdir().unwrap();
        let _env = isolated(temp.path());

        let exe = temp.path().join("PCSX2").join("pcsx2.AppImage");
        touch(&exe);
        let config = config_with(temp.path(), vec![entry("PCSX2", exe.to_str().unwrap())]);
        let config_path = write_config(temp.path(), &config);

        let mut pcsx2 = profile("PCSX2", &["pcsx2*"]);
        pcsx2.all_platforms = true;
        pcsx2.save_directories = vec!["~/pcsx2/saves".to_string()];
        let profiles = vec![pcsx2];
        let platforms = vec!["Sony PlayStation 2".to_string()];

        let ctx = SyncContext {
            config_path: &config_path,
            platforms: &platforms,
            ps3_library_path: String::new(),
            ra: None,
            profiles: &profiles,
        };
        let report = sync_new_emulator("PCSX2", &ctx).unwrap();
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);

        let emulator_dir = exe.parent().unwrap();
        assert!(emulator_dir.join("portable.ini").is_file());
        assert!(emulator_dir.join("inis").join("PCSX2.ini").is_file());

        let saved = Config::load(&config_path).unwrap();
        assert_eq!(saved.emulators[0].save_paths, "~/pcsx2/saves");
        assert_eq!(
            saved.default_emulators.get("Sony PlayStation 2"),
            Some(&"PCSX2".to_string())
        );
    }

    /// A path under a directory the process cannot write to: PCSX2 resolves
    /// its target, fails to create `inis/`, and reports the bail-out result.
    #[cfg(unix)]
    #[test]
    fn sync_writer_failure_is_reported_as_a_warning_not_an_error() {
        use std::os::unix::fs::PermissionsExt;

        let _lock = lock();
        let temp = tempfile::tempdir().unwrap();
        let _env = isolated(temp.path());

        let read_only = temp.path().join("read-only");
        let exe = read_only.join("pcsx2.AppImage");
        touch(&exe);
        std::fs::set_permissions(&read_only, std::fs::Permissions::from_mode(0o555)).unwrap();

        let config = config_with(temp.path(), vec![entry("PCSX2", exe.to_str().unwrap())]);
        let config_path = write_config(temp.path(), &config);
        let profiles = vec![profile("PCSX2", &["pcsx2*"])];

        let ctx = SyncContext {
            config_path: &config_path,
            platforms: &[],
            ps3_library_path: String::new(),
            ra: None,
            profiles: &profiles,
        };
        let report = sync_new_emulator("PCSX2", &ctx).unwrap();

        // Restore before any assertion can panic, so the tempdir still cleans up.
        std::fs::set_permissions(&read_only, std::fs::Permissions::from_mode(0o755)).unwrap();

        assert!(
            !report.warnings.is_empty(),
            "a writer that reached nothing must warn"
        );
        assert!(
            report.warnings[0].contains("PCSX2"),
            "the warning names the emulator: {:?}",
            report.warnings
        );
        assert!(report.wrote.is_empty());
    }

    #[test]
    fn predicates_are_independent_so_two_can_fire() {
        let _lock = lock();
        let temp = tempfile::tempdir().unwrap();
        let _env = isolated(temp.path());

        let exe = temp.path().join("Emu").join("emu.AppImage");
        touch(&exe);
        let config = config_with(
            temp.path(),
            vec![entry("RetroArch + PPSSPP", exe.to_str().unwrap())],
        );
        let config_path = write_config(temp.path(), &config);

        let ctx = SyncContext {
            config_path: &config_path,
            platforms: &[],
            ps3_library_path: String::new(),
            ra: None,
            profiles: &[],
        };
        let report = sync_new_emulator("RetroArch + PPSSPP", &ctx).unwrap();
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);

        let dir = exe.parent().unwrap();
        assert!(
            dir.join("retroarch.cfg").is_file(),
            "the retroarch writer must have run"
        );
        assert!(
            dir.join("memstick/PSP/SYSTEM/PPSSPP.INI").is_file(),
            "the ppsspp writer must have run too"
        );
    }

    #[test]
    fn sync_passes_the_romm_username_only_to_retroarch() {
        let _lock = lock();
        let temp = tempfile::tempdir().unwrap();
        let _env = isolated(temp.path());

        let exe = temp.path().join("Emu").join("emu.AppImage");
        touch(&exe);
        let mut config = config_with(
            temp.path(),
            vec![entry("RetroArch + PPSSPP", exe.to_str().unwrap())],
        );
        config.username = "romm-account".to_string();
        let config_path = write_config(temp.path(), &config);

        let ctx = SyncContext {
            config_path: &config_path,
            platforms: &[],
            ps3_library_path: String::new(),
            ra: None,
            profiles: &[],
        };
        sync_new_emulator("RetroArch + PPSSPP", &ctx).unwrap();

        let dir = exe.parent().unwrap();
        let cfg = std::fs::read_to_string(dir.join("retroarch.cfg")).unwrap();
        assert!(
            cfg.contains("netplay_nickname") && cfg.contains("romm-account"),
            "retroarch takes the RomM username as its netplay nickname:\n{cfg}"
        );

        let ini = std::fs::read_to_string(dir.join("memstick/PSP/SYSTEM/PPSSPP.INI")).unwrap();
        assert!(
            !ini.contains("romm-account"),
            "no other writer sees the RomM username:\n{ini}"
        );
    }

    #[test]
    fn sync_passes_the_ps3_library_path_to_rpcs3() {
        let _lock = lock();
        let temp = tempfile::tempdir().unwrap();
        let _env = isolated(temp.path());

        let exe = temp.path().join("RPCS3").join("rpcs3.AppImage");
        touch(&exe);
        let config = config_with(temp.path(), vec![entry("RPCS3", exe.to_str().unwrap())]);
        let config_path = write_config(temp.path(), &config);

        let ps3 = ps3_library_path(&config.library_path);
        assert!(ps3.ends_with("PlayStation 3"), "{ps3}");

        let ctx = SyncContext {
            config_path: &config_path,
            platforms: &[],
            ps3_library_path: ps3,
            ra: None,
            profiles: &[],
        };
        let report = sync_new_emulator("RPCS3", &ctx).unwrap();
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);

        let vfs = exe.parent().unwrap().join("portable/config/vfs.yml");
        let text = std::fs::read_to_string(&vfs).unwrap();
        assert!(
            text.contains("PlayStation 3"),
            "the PS3 library path must reach vfs.yml:\n{text}"
        );
    }

    /// `cemu::ensure_controller_config` always reports `config_path = None`
    /// and names `extras["profile_path"]` instead, so its healthy idempotent
    /// branch — an existing `controller0.xml`, nothing to write — returns
    /// `changed = false` with no `config_path`. That is a success, not a
    /// bail-out, and must never reach `report.warnings`.
    #[test]
    fn an_existing_cemu_controller_profile_is_not_reported_as_a_failure() {
        let _lock = lock();
        let temp = tempfile::tempdir().unwrap();
        let _env = isolated(temp.path());

        let exe = temp.path().join("Cemu").join("cemu.AppImage");
        touch(&exe);
        let profile_path = exe
            .parent()
            .unwrap()
            .join("portable")
            .join("controllerProfiles")
            .join("controller0.xml");
        touch(&profile_path);

        let config = config_with(temp.path(), vec![entry("Cemu", exe.to_str().unwrap())]);
        let config_path = write_config(temp.path(), &config);

        let ctx = SyncContext {
            config_path: &config_path,
            platforms: &[],
            ps3_library_path: String::new(),
            ra: None,
            profiles: &[],
        };
        let report = sync_new_emulator("Cemu", &ctx).unwrap();

        assert!(
            !report
                .warnings
                .iter()
                .any(|warning| warning.contains("cemu controller")),
            "the idempotent branch is a success: {:?}",
            report.warnings
        );
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    }

    /// The other half of the same fix: a controller profile this run actually
    /// wrote is recorded in `wrote` through its `extras` path, which the
    /// `config_path`-only reading dropped on the floor.
    #[test]
    fn a_written_cemu_controller_profile_is_recorded_from_its_extras_path() {
        let _lock = lock();
        let temp = tempfile::tempdir().unwrap();
        let _env = isolated(temp.path());

        let exe = temp.path().join("Cemu").join("cemu.AppImage");
        touch(&exe);
        let config = config_with(temp.path(), vec![entry("Cemu", exe.to_str().unwrap())]);
        let config_path = write_config(temp.path(), &config);

        let ctx = SyncContext {
            config_path: &config_path,
            platforms: &[],
            ps3_library_path: String::new(),
            ra: None,
            profiles: &[],
        };
        let report = sync_new_emulator("Cemu", &ctx).unwrap();

        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
        assert!(
            report.wrote.iter().any(|p| p.ends_with("controller0.xml")),
            "the freshly written controller profile must be recorded: {:?}",
            report.wrote
        );
    }

    #[test]
    fn ps3_library_path_is_empty_without_a_library() {
        assert_eq!(ps3_library_path(""), "");
        assert_eq!(ps3_library_path("   "), "");
    }

    /// Spec deviation D6: PCSX2's `[Folders] Bios` is not written this
    /// milestone, so the orchestrator passes no BIOS directory at all.
    #[test]
    fn sync_omits_pcsx2_bios_directory() {
        let _lock = lock();
        let temp = tempfile::tempdir().unwrap();
        let _env = isolated(temp.path());

        let exe = temp.path().join("PCSX2").join("pcsx2.AppImage");
        touch(&exe);
        let config = config_with(temp.path(), vec![entry("PCSX2", exe.to_str().unwrap())]);
        let config_path = write_config(temp.path(), &config);

        let ctx = SyncContext {
            config_path: &config_path,
            platforms: &[],
            ps3_library_path: String::new(),
            ra: None,
            profiles: &[],
        };
        sync_new_emulator("PCSX2", &ctx).unwrap();

        let ini =
            std::fs::read_to_string(exe.parent().unwrap().join("inis").join("PCSX2.ini")).unwrap();
        assert!(
            !ini.contains("[Folders]"),
            "D6: no [Folders] section:\n{ini}"
        );
        assert!(!ini.contains("Bios"), "D6: no Bios key:\n{ini}");
    }

    // --- D2: ra_capable / fan_out_ra_credentials -----------------------------

    /// `RetroArch`/`PCSX2`/`PPSSPP`-named entries are RA-capable; DuckStation
    /// (suppression keys only — no credential parameters, doc 05 open
    /// question, ruled: follow the code) and Dolphin (not RA-aware at all)
    /// are not.
    #[test]
    fn ra_capable_excludes_duckstation_and_dolphin() {
        assert!(ra_capable(
            &entry("RetroArch", "/opt/retroarch/retroarch"),
            &[]
        ));
        assert!(ra_capable(
            &entry("PCSX2", "/opt/pcsx2/pcsx2-qt.AppImage"),
            &[]
        ));
        assert!(ra_capable(&entry("PPSSPP", "/opt/ppsspp/PPSSPPSDL"), &[]));

        assert!(!ra_capable(
            &entry("DuckStation", "/opt/duckstation/duckstation-qt.AppImage"),
            &[]
        ));
        assert!(!ra_capable(
            &entry("Dolphin", "/opt/dolphin/dolphin-emu"),
            &[]
        ));
    }

    /// A config with a RetroArch, a PCSX2 and a Dolphin entry, each with a
    /// pre-existing config file carrying a sentinel unmanaged key: the
    /// fan-out writes ONLY the RA credential keys into the two RA-capable
    /// files, leaves every sentinel intact, and never touches the Dolphin
    /// file (Dolphin is not RA-capable at all — it is not merely skipped for
    /// blank credentials).
    #[test]
    fn fan_out_writes_only_the_ra_keys() {
        let _lock = lock();
        let temp = tempfile::tempdir().unwrap();
        let _env = isolated(temp.path());

        let ra_exe = temp.path().join("RetroArch").join("retroarch.exe");
        touch(&ra_exe);
        let ra_cfg = ra_exe.parent().unwrap().join("retroarch.cfg");
        std::fs::write(&ra_cfg, "my_unmanaged_key = \"keep\"\n").unwrap();

        let pcsx2_exe = temp.path().join("PCSX2").join("pcsx2-qt.AppImage");
        touch(&pcsx2_exe);
        let pcsx2_cfg = pcsx2_exe.parent().unwrap().join("inis").join("PCSX2.ini");
        std::fs::create_dir_all(pcsx2_cfg.parent().unwrap()).unwrap();
        std::fs::write(&pcsx2_cfg, "[UI]\nSentinelKey = keep\n").unwrap();

        let dolphin_exe = temp.path().join("Dolphin").join("dolphin-emu");
        touch(&dolphin_exe);
        let dolphin_cfg = dolphin_exe.parent().unwrap().join("sentinel.ini");
        std::fs::write(&dolphin_cfg, "[Sentinel]\nUnmanaged = keep\n").unwrap();
        let dolphin_before = std::fs::read(&dolphin_cfg).unwrap();

        let config = config_with(
            temp.path(),
            vec![
                entry("RetroArch", ra_exe.to_str().unwrap()),
                entry("PCSX2", pcsx2_exe.to_str().unwrap()),
                entry("Dolphin", dolphin_exe.to_str().unwrap()),
            ],
        );

        let ra = RaCredentials::new("retro_user", "FAKE-RA-TOKEN-not-real");
        let rows = fan_out_ra_credentials(&config, &[], &ra);

        assert_eq!(
            rows,
            vec![("RetroArch".to_string(), true), ("PCSX2".to_string(), true),]
        );

        let ra_text = std::fs::read_to_string(&ra_cfg).unwrap();
        assert!(
            ra_text.contains("my_unmanaged_key = \"keep\""),
            "sentinel must survive: {ra_text}"
        );
        assert!(ra_text.contains("cheevos_username = \"retro_user\""));
        assert!(ra_text.contains("cheevos_token = \"FAKE-RA-TOKEN-not-real\""));

        let pcsx2_text = std::fs::read_to_string(&pcsx2_cfg).unwrap();
        assert!(
            pcsx2_text.contains("SentinelKey = keep"),
            "sentinel must survive: {pcsx2_text}"
        );
        assert!(pcsx2_text.contains("[Achievements]"));
        assert!(pcsx2_text.contains("Username = retro_user"));

        assert_eq!(
            std::fs::read(&dolphin_cfg).unwrap(),
            dolphin_before,
            "Dolphin is not RA-capable; its file must be byte-identical"
        );
    }

    #[test]
    fn fan_out_is_a_no_op_when_either_field_is_blank() {
        let _lock = lock();
        let temp = tempfile::tempdir().unwrap();
        let _env = isolated(temp.path());

        let exe = temp.path().join("RetroArch").join("retroarch.exe");
        touch(&exe);
        let config = config_with(temp.path(), vec![entry("RetroArch", exe.to_str().unwrap())]);

        for ra in [
            RaCredentials::new("retro_user", ""),
            RaCredentials::new("", "FAKE-RA-TOKEN-not-real"),
        ] {
            let rows = fan_out_ra_credentials(&config, &[], &ra);
            assert!(rows.is_empty(), "{rows:?}");
        }
        assert!(!exe.parent().unwrap().join("retroarch.cfg").exists());
    }

    #[test]
    fn fan_out_reports_changed_false_on_a_second_run() {
        let _lock = lock();
        let temp = tempfile::tempdir().unwrap();
        let _env = isolated(temp.path());

        let exe = temp.path().join("RetroArch").join("retroarch.exe");
        touch(&exe);
        let config = config_with(temp.path(), vec![entry("RetroArch", exe.to_str().unwrap())]);
        let ra = RaCredentials::new("retro_user", "FAKE-RA-TOKEN-not-real");

        let first = fan_out_ra_credentials(&config, &[], &ra);
        assert_eq!(first, vec![("RetroArch".to_string(), true)]);

        let second = fan_out_ra_credentials(&config, &[], &ra);
        assert_eq!(second, vec![("RetroArch".to_string(), false)]);
    }

    /// Spec deviation D7: no RPCS3 firmware download is triggered, so nothing
    /// under the emulator directory names a PUP or a `dev_flash` tree.
    #[test]
    fn sync_starts_no_firmware_download() {
        let _lock = lock();
        let temp = tempfile::tempdir().unwrap();
        let _env = isolated(temp.path());

        let exe = temp.path().join("RPCS3").join("rpcs3.AppImage");
        touch(&exe);
        let config = config_with(temp.path(), vec![entry("RPCS3", exe.to_str().unwrap())]);
        let config_path = write_config(temp.path(), &config);

        let ctx = SyncContext {
            config_path: &config_path,
            platforms: &[],
            ps3_library_path: ps3_library_path(&config.library_path),
            ra: None,
            profiles: &[],
        };
        sync_new_emulator("RPCS3", &ctx).unwrap();

        let dir = exe.parent().unwrap();
        assert!(!dir.join("PS3UPDAT.PUP").exists());
        assert!(!dir.join("portable").join("dev_flash").exists());
        let names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert!(
            !names.iter().any(|n| n.to_uppercase().ends_with(".PUP")),
            "D7: no firmware download may start: {names:?}"
        );
    }
}
