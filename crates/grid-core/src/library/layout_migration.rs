//! The one-shot startup migration from the flat (pre-v1) library root to the
//! `games/` / `emulators/` / `saves/` layout.
//!
//! The migration is REFERENCE-DRIVEN: the set of legacy platform directories
//! it moves comes from the registry's path columns and the config's save-path
//! fields, never from a directory listing. An unreferenced top-level
//! directory is left exactly where it is, so pointing the config at a
//! populated non-GRID directory moves nothing and still stamps the version.
//!
//! Every step is idempotent and every move is a `rename` inside the library
//! directory, so an interrupted run resumes on the next start. A
//! `CrossesDevices` rename aborts the run with both paths in the message.

use std::collections::BTreeSet;
use std::path::Path;

use super::paths::{
    expand_home, sanitize_component, saves_dir, EMULATORS_DIR, GAMES_DIR, LAYOUT_VERSION_V1,
    LEGACY_EMULATORS_DIR, SAVES_DIR,
};
use super::registry::{InstalledGame, Registry};
use super::user_data_links::ensure_user_data_links;
use super::LibraryError;
use crate::config::Config;
use crate::launch::profiles::EmulatorProfile;

/// The fixed name `Emulators` is renamed through when a direct
/// `Emulators` → `emulators` rename fails (case-insensitive filesystems).
/// Fixed, not random, so a run interrupted between the two renames finds it
/// and finishes.
const TEMP_EMULATORS_DIR: &str = ".grid-layout-emulators-tmp";

/// Top-level names the migration never treats as a game platform directory.
const RESERVED_TOP_LEVEL: [&str; 4] = [GAMES_DIR, EMULATORS_DIR, LEGACY_EMULATORS_DIR, SAVES_DIR];

/// Every spelling of the library root a stored path may use, and the two
/// name sets the rewrite maps.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RewritePlan {
    /// The library root as the config types it, its `~`-expanded form, and
    /// each with `\` replaced by `/`; trailing separators trimmed, blanks
    /// and duplicates dropped.
    library_forms: Vec<String>,
    /// The reference-derived legacy platform directory names.
    game_dirs: BTreeSet<String>,
    /// `(legacy tagged directory name, new profile-named directory)` pairs,
    /// kept only for a rename that has already happened on disk.
    emulator_dirs: Vec<(String, String)>,
}

/// Whether `c` separates path components in either spelling.
fn is_separator(c: char) -> bool {
    c == '/' || c == '\\'
}

/// `(first component, the rest including its leading separator)`.
fn split_component(rest: &str) -> (&str, &str) {
    match rest.find(is_separator) {
        Some(index) => (&rest[..index], &rest[index..]),
        None => (rest, ""),
    }
}

/// Every spelling of the library root to match stored paths against.
fn library_forms(config: &Config, library: &Path) -> Vec<String> {
    let mut forms = Vec::new();
    for raw in [
        config.library_path.trim().to_string(),
        library.to_string_lossy().into_owned(),
    ] {
        for form in [raw.clone(), raw.replace('\\', "/")] {
            let trimmed = form.trim_end_matches(is_separator).to_string();
            if !trimmed.is_empty() && !forms.contains(&trimmed) {
                forms.push(trimmed);
            }
        }
    }
    forms
}

/// The top-level directory name `value` names under the library root, i.e.
/// the `X` of `<library>/X/...`. `None` when `value` is outside the library,
/// names the root itself, has no component after `X` (Decision 15 requires
/// the trailing separator), or `X` is reserved or dot-prefixed.
fn top_level_dir(value: &str, forms: &[String]) -> Option<String> {
    for form in forms {
        let Some(after) = value.strip_prefix(form.as_str()) else {
            continue;
        };
        let Some(separator) = after.chars().next().filter(|c| is_separator(*c)) else {
            continue;
        };
        let (name, tail) = split_component(&after[separator.len_utf8()..]);
        if name.is_empty() || tail.is_empty() || name.starts_with('.') {
            return None;
        }
        if RESERVED_TOP_LEVEL.contains(&name) {
            return None;
        }
        return Some(name.to_string());
    }
    None
}

/// `cloud/dirs.rs::split_entry_list` (:189-196), copied rather than shared:
/// that one is private to the cloud module and this is the only other reader
/// of the same `;`/newline-separated config format.
fn split_entry_list(value: &str) -> Vec<String> {
    value
        .split([';', '\r', '\n'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}

/// The `ps3_trophy_paths` JSON array, or an empty list for anything that is
/// not one (same leniency as `library::mod::parse_trophy_paths`).
fn trophy_paths(raw: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(raw).unwrap_or_default()
}

/// The reference-derived set of legacy platform directory names: the
/// top-level name of every registry path column value and of every config
/// save-path value, plus `PlayStation 3` when the RPCS3 vfs directory is on
/// disk.
pub fn derive_game_dirs(
    config: &Config,
    rows: &[InstalledGame],
    library: &Path,
) -> BTreeSet<String> {
    let forms = library_forms(config, library);
    let mut dirs = BTreeSet::new();

    for row in rows {
        let mut values: Vec<&str> = vec![
            row.archive_path.as_str(),
            row.extracted_path.as_str(),
            row.extracted_dir.as_str(),
            row.multi_file_game_dir.as_str(),
            row.native_executable_path.as_str(),
            row.native_wineprefix.as_str(),
            row.native_game_dir.as_str(),
            row.ps3_iso_path.as_str(),
        ];
        let trophies = trophy_paths(&row.ps3_trophy_paths);
        values.extend(trophies.iter().map(String::as_str));
        for value in values {
            if let Some(name) = top_level_dir(value, &forms) {
                dirs.insert(name);
            }
        }
    }

    for map in [
        &config.native_manual_save_paths,
        &config.native_pcgw_save_paths,
        &config.native_removed_save_paths,
    ] {
        for values in map.values() {
            for value in values {
                if let Some(name) = top_level_dir(value, &forms) {
                    dirs.insert(name);
                }
            }
        }
    }

    for entry in &config.emulators {
        for value in split_entry_list(&entry.save_paths)
            .into_iter()
            .chain(split_entry_list(&entry.state_paths))
        {
            if let Some(name) = top_level_dir(&value, &forms) {
                dirs.insert(name);
            }
        }
    }

    // The RPCS3 virtual filesystem lives under a fixed platform name and is
    // referenced by `vfs.yml`, not by any row or config value.
    if library.join("PlayStation 3").join(".vfs").is_dir() {
        dirs.insert("PlayStation 3".to_string());
    }

    dirs
}

/// The plan the rewrite and the moves are driven by: the root spellings, the
/// derived game directories, and the emulator directory renames that have
/// already landed on disk.
///
/// A pair `(D, N)` is kept only when `D != N`, `emulators/N` exists and
/// neither `emulators/D` nor `Emulators/D` does — that is what makes the
/// rewrite safe to run before, during and after the renames: a directory
/// that was not renamed keeps its name in every rewritten path.
pub fn build_rewrite_plan(
    config: &Config,
    registry: &Registry,
    library: &Path,
    profiles: &[EmulatorProfile],
) -> Result<RewritePlan, LibraryError> {
    let forms = library_forms(config, library);
    let game_dirs = derive_game_dirs(config, &registry.all()?, library);

    let emulator_dirs = kept_emulator_pairs(config, library, &forms, profiles);

    Ok(RewritePlan {
        library_forms: forms,
        game_dirs,
        emulator_dirs,
    })
}

/// The `(D, N)` pairs whose rename is already ON DISK: `emulators/N` exists
/// and neither `emulators/D` nor `Emulators/D` does. Anything else keeps its
/// directory name in every rewritten path.
fn kept_emulator_pairs(
    config: &Config,
    library: &Path,
    forms: &[String],
    profiles: &[EmulatorProfile],
) -> Vec<(String, String)> {
    let new_names = entry_names(&library.join(EMULATORS_DIR));
    let legacy_names = entry_names(&library.join(LEGACY_EMULATORS_DIR));

    let mut pairs: Vec<(String, String)> = Vec::new();
    for (legacy_dir, new_dir) in emulator_dir_pairs(config, forms, profiles) {
        if legacy_dir == new_dir
            || pairs.iter().any(|(d, _)| *d == legacy_dir)
            || !new_names.contains(&new_dir)
            || new_names.contains(&legacy_dir)
            || legacy_names.contains(&legacy_dir)
        {
            continue;
        }
        pairs.push((legacy_dir, new_dir));
    }
    pairs
}

/// Every `(legacy directory name, new directory name)` the config's emulator
/// entries imply, in config order and before any on-disk check. The new name
/// is the sanitized PROFILE name when one matches, otherwise the legacy name
/// unchanged (a hand-added emulator keeps its directory).
fn emulator_dir_pairs(
    config: &Config,
    forms: &[String],
    profiles: &[EmulatorProfile],
) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    for entry in &config.emulators {
        let Some(legacy_dir) = emulator_dir_of(&entry.path, forms) else {
            continue;
        };
        let new_dir =
            match crate::launch::profiles::profile_for_entry(&entry.name, &entry.path, profiles) {
                Some(profile) => sanitize_component(&profile.name, "emulator"),
                None => legacy_dir.clone(),
            };
        pairs.push((legacy_dir, new_dir));
    }
    pairs
}

/// The emulator install directory name `path` sits in, for a path under
/// either `<library>/Emulators/` or `<library>/emulators/`.
fn emulator_dir_of(path: &str, forms: &[String]) -> Option<String> {
    for form in forms {
        let Some(after) = path.strip_prefix(form.as_str()) else {
            continue;
        };
        let Some(separator) = after.chars().next().filter(|c| is_separator(*c)) else {
            continue;
        };
        let (root, tail) = split_component(&after[separator.len_utf8()..]);
        if root != LEGACY_EMULATORS_DIR && root != EMULATORS_DIR {
            return None;
        }
        let separator = tail.chars().next().filter(|c| is_separator(*c))?;
        let (dir, rest) = split_component(&tail[separator.len_utf8()..]);
        if dir.is_empty() || rest.is_empty() {
            return None;
        }
        return Some(dir.to_string());
    }
    None
}

/// The exact entry names of `dir` as `read_dir` reports them. Empty when the
/// directory does not exist or cannot be read.
///
/// Exact names, never `Path::exists`: on a case-insensitive filesystem
/// `<library>/emulators` "exists" whenever `<library>/Emulators` does.
fn entry_names(dir: &Path) -> BTreeSet<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return BTreeSet::new();
    };
    entries
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect()
}

/// The v1 form of one stored path, or `None` when it names nothing the
/// migration moves (a path outside the library, the root itself, a reserved
/// or dot-prefixed top-level name, or an unreferenced directory).
pub fn rewrite_path(value: &str, plan: &RewritePlan) -> Option<String> {
    for form in &plan.library_forms {
        let Some(after) = value.strip_prefix(form.as_str()) else {
            continue;
        };
        let Some(separator) = after.chars().next().filter(|c| is_separator(*c)) else {
            continue;
        };
        let (name, tail) = split_component(&after[separator.len_utf8()..]);
        if name.is_empty() || name.starts_with('.') {
            return None;
        }
        if name == LEGACY_EMULATORS_DIR {
            let inner = tail.chars().next().filter(|c| is_separator(*c))?;
            let (legacy_dir, rest) = split_component(&tail[inner.len_utf8()..]);
            if legacy_dir.is_empty() {
                return None;
            }
            let new_dir = plan
                .emulator_dirs
                .iter()
                .find(|(d, _)| d == legacy_dir)
                .map(|(_, n)| n.as_str())
                .unwrap_or(legacy_dir);
            return Some(format!(
                "{form}{separator}{EMULATORS_DIR}{inner}{new_dir}{rest}"
            ));
        }
        if plan.game_dirs.contains(name) {
            return Some(format!(
                "{form}{separator}{GAMES_DIR}{separator}{name}{tail}"
            ));
        }
        return None;
    }
    None
}

/// What one [`run`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MigrationOutcome {
    /// Nothing to do: already at layout v1, no library configured, or the
    /// library directory is not there (an unmounted drive retries next
    /// start). The version is left untouched.
    Skipped,
    /// Every step finished and the version was stamped. All four counts are
    /// zero for a library with nothing referenced in it, which is still a
    /// completed run.
    Completed {
        games_moved: usize,
        emulators_renamed: usize,
        links_changed: usize,
        rows_rewritten: usize,
    },
    /// A step failed. The version stays at 0, the library stays readable
    /// through the legacy fallbacks, and the next start resumes.
    Failed { message: String },
}

/// Migrates the configured library to layout v1, once.
///
/// Reads and writes `config_path` itself (the caller has no config handle at
/// this point in startup) and rewrites `registry` in place. Never panics and
/// never returns an error type: startup continues either way, and a
/// `Failed` outcome is what the UI turns into one notice.
pub fn run(
    config_path: &Path,
    registry: &Registry,
    profiles: &[EmulatorProfile],
) -> MigrationOutcome {
    let config = match Config::load(config_path) {
        Ok(config) => config,
        Err(e) => {
            tracing::warn!(path = %config_path.display(), error = %e, "library layout migration could not read the config");
            return MigrationOutcome::Failed {
                message: format!("could not read {}: {e}", config_path.display()),
            };
        }
    };
    if config.library_layout_version >= LAYOUT_VERSION_V1 {
        return MigrationOutcome::Skipped;
    }
    if config.library_path.trim().is_empty() {
        return MigrationOutcome::Skipped;
    }
    let library = expand_home(config.library_path.trim());
    if !library.is_dir() {
        tracing::debug!(
            path = %library.display(),
            "library directory is not present; layout migration deferred"
        );
        return MigrationOutcome::Skipped;
    }

    match migrate(config_path, &config, &library, registry, profiles) {
        Ok(outcome) => {
            tracing::info!(path = %library.display(), outcome = ?outcome, "library layout migration finished");
            outcome
        }
        Err(message) => {
            tracing::warn!(
                path = %library.display(),
                message = %message,
                "library layout migration did not finish"
            );
            MigrationOutcome::Failed { message }
        }
    }
}

/// The five steps, in order. Every one is idempotent, so a run interrupted
/// anywhere resumes here on the next start.
fn migrate(
    config_path: &Path,
    config: &Config,
    library: &Path,
    registry: &Registry,
    profiles: &[EmulatorProfile],
) -> Result<MigrationOutcome, String> {
    let mut plan =
        build_rewrite_plan(config, registry, library, profiles).map_err(|e| e.to_string())?;
    preflight(library, &plan)?;

    let games_moved = step_games(library, &plan)?;
    let emulators_renamed = step_emulators(library, config, &plan, profiles)?;
    let links_changed = step_user_data(library, config, &plan, profiles)?;
    // The pairs describe renames that have LANDED, so they are only
    // complete once step 2 has run. The game set is derived from the
    // registry and the config, which step 1 does not touch, so it stands.
    plan.emulator_dirs = kept_emulator_pairs(config, library, &plan.library_forms, profiles);
    let rows_rewritten = step_rewrite(config_path, config, registry, &plan, profiles)?;
    write_version(config_path)?;

    Ok(MigrationOutcome::Completed {
        games_moved,
        emulators_renamed,
        links_changed,
        rows_rewritten,
    })
}

/// Aborts before anything moves when a pending rename's destination is
/// already taken. Compares EXACT `read_dir` names, never `Path::exists`.
fn preflight(library: &Path, plan: &RewritePlan) -> Result<(), String> {
    let root = entry_names(library);
    let games = entry_names(&library.join(GAMES_DIR));
    for name in &plan.game_dirs {
        if root.contains(name) && games.contains(name) {
            return Err(collision_message(
                &library.join(name),
                &library.join(GAMES_DIR).join(name),
            ));
        }
    }

    let legacy = entry_names(&library.join(LEGACY_EMULATORS_DIR));
    let current = entry_names(&library.join(EMULATORS_DIR));
    if let Some(name) = legacy.intersection(&current).next() {
        return Err(collision_message(
            &library.join(LEGACY_EMULATORS_DIR).join(name),
            &library.join(EMULATORS_DIR).join(name),
        ));
    }
    Ok(())
}

fn collision_message(from: &Path, to: &Path) -> String {
    format!(
        "{} cannot be moved to {}: both already exist. Move or remove one of them and restart.",
        from.display(),
        to.display()
    )
}

/// Step 1: `<library>/X` → `<library>/games/X` for every derived name.
/// Files, symlinks and unreferenced directories at the root are left alone.
fn step_games(library: &Path, plan: &RewritePlan) -> Result<usize, String> {
    if plan.game_dirs.is_empty() {
        return Ok(0);
    }
    let mut moved = 0;
    let root = entry_names(library);
    for name in &plan.game_dirs {
        // The exact name first: on a case-insensitive filesystem a stat of
        // `<library>/Windows` succeeds while the entry is really `windows`.
        if !root.contains(name) {
            continue; // already moved by an earlier run, or never there
        }
        let source = library.join(name);
        let Ok(metadata) = std::fs::symlink_metadata(&source) else {
            continue;
        };
        if !metadata.is_dir() {
            tracing::warn!(
                path = %source.display(),
                "library entry is not a real directory; left in place"
            );
            continue;
        }
        let games_root = library.join(GAMES_DIR);
        std::fs::create_dir_all(&games_root)
            .map_err(|e| format!("could not create {}: {e}", games_root.display()))?;
        rename_in_library(&source, &games_root.join(name))?;
        moved += 1;
    }
    Ok(moved)
}

/// Step 2: the emulator root rename, then one `<tagged> → <profile name>`
/// rename per configured entry.
fn step_emulators(
    library: &Path,
    config: &Config,
    plan: &RewritePlan,
    profiles: &[EmulatorProfile],
) -> Result<usize, String> {
    ensure_emulators_root(library)?;

    let root = library.join(EMULATORS_DIR);
    let mut renamed = 0;
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for (legacy_dir, new_dir) in emulator_dir_pairs(config, &plan.library_forms, profiles) {
        if legacy_dir == new_dir || !seen.insert(legacy_dir.clone()) {
            continue;
        }
        let names = entry_names(&root);
        if !names.contains(&legacy_dir) {
            continue; // already renamed by an earlier run, or not installed
        }
        if names.contains(&new_dir) {
            tracing::warn!(
                from = %root.join(&legacy_dir).display(),
                to = %root.join(&new_dir).display(),
                "emulator directory already exists; this entry keeps its old name"
            );
            continue;
        }
        rename_in_library(&root.join(&legacy_dir), &root.join(&new_dir))?;
        renamed += 1;
    }
    Ok(renamed)
}

/// `<library>/Emulators` → `<library>/emulators`, resumable.
///
/// The direct rename is tried first; only when it fails (a case-insensitive
/// filesystem that refuses a case-only rename) does this go through the
/// FIXED temporary name, so a run interrupted between the two renames finds
/// the half-done state and finishes it.
fn ensure_emulators_root(library: &Path) -> Result<(), String> {
    let legacy = library.join(LEGACY_EMULATORS_DIR);
    let temp = library.join(TEMP_EMULATORS_DIR);
    let current = library.join(EMULATORS_DIR);

    let names = entry_names(library);
    if names.contains(LEGACY_EMULATORS_DIR) && !names.contains(EMULATORS_DIR) {
        if std::fs::rename(&legacy, &current).is_ok() {
            return Ok(());
        }
        rename_in_library(&legacy, &temp)?;
        return rename_in_library(&temp, &current);
    }
    if names.contains(TEMP_EMULATORS_DIR) && !names.contains(EMULATORS_DIR) {
        return rename_in_library(&temp, &current);
    }

    // Both roots exist (an interrupted run, or a library that already had
    // one): fold the legacy one in, entry by entry. The preflight has
    // already ruled out a name collision.
    for (name, source) in [(LEGACY_EMULATORS_DIR, legacy), (TEMP_EMULATORS_DIR, temp)] {
        if names.contains(name) {
            merge_emulator_roots(&source, &current)?;
        }
    }
    Ok(())
}

/// Moves every entry of `source` into `dest`, then removes `source` when it
/// is empty. A name already taken in `dest` is skipped with a warning.
fn merge_emulator_roots(source: &Path, dest: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dest)
        .map_err(|e| format!("could not create {}: {e}", dest.display()))?;
    for name in entry_names(source) {
        if entry_names(dest).contains(&name) {
            tracing::warn!(
                path = %source.join(&name).display(),
                "an emulator directory of the same name already exists; left in place"
            );
            continue;
        }
        rename_in_library(&source.join(&name), &dest.join(&name))?;
    }
    let _ = std::fs::remove_dir(source);
    Ok(())
}

/// Step 3: point every install directory's `user_data` names at
/// `saves/<Emulator>/`.
fn step_user_data(
    library: &Path,
    config: &Config,
    plan: &RewritePlan,
    profiles: &[EmulatorProfile],
) -> Result<usize, String> {
    let root = library.join(EMULATORS_DIR);
    let names = entry_names(&root);
    let mut changed = 0;
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for entry in &config.emulators {
        let Some(legacy_dir) = emulator_dir_of(&entry.path, &plan.library_forms) else {
            continue;
        };
        let Some(profile) =
            crate::launch::profiles::profile_for_entry(&entry.name, &entry.path, profiles)
        else {
            continue;
        };
        if profile.user_data.is_empty() {
            continue;
        }
        // The tagged name wins when it is still there: that is the entry
        // whose rename was skipped for a collision.
        let new_dir = sanitize_component(&profile.name, "emulator");
        let install_name = if names.contains(&legacy_dir) {
            legacy_dir
        } else {
            new_dir
        };
        if !names.contains(&install_name) || !seen.insert(install_name.clone()) {
            continue;
        }
        let install_dir = root.join(&install_name);
        match ensure_user_data_links(
            &install_dir,
            &saves_dir(library, &profile.name),
            &profile.user_data,
        ) {
            Ok(true) => changed += 1,
            Ok(false) => {}
            Err(e) => {
                return Err(format!(
                    "could not link the user data of {}: {e}",
                    install_dir.display()
                ))
            }
        }
    }
    Ok(changed)
}

/// Step 4: the config, the registry and the three emulator config files that
/// store absolute library paths.
fn step_rewrite(
    config_path: &Path,
    config: &Config,
    registry: &Registry,
    plan: &RewritePlan,
    profiles: &[EmulatorProfile],
) -> Result<usize, String> {
    let mut updated = config.clone();
    for entry in &mut updated.emulators {
        if let Some(path) = rewrite_path(&entry.path, plan) {
            entry.path = path;
        }
        entry.save_paths = rewrite_entry_list(&entry.save_paths, plan);
        entry.state_paths = rewrite_entry_list(&entry.state_paths, plan);
    }
    for map in [
        &mut updated.native_manual_save_paths,
        &mut updated.native_pcgw_save_paths,
        &mut updated.native_removed_save_paths,
    ] {
        for values in map.values_mut() {
            for value in values.iter_mut() {
                if let Some(rewritten) = rewrite_path(value, plan) {
                    *value = rewritten;
                }
            }
        }
    }
    updated
        .save(config_path)
        .map_err(|e| format!("could not write {}: {e}", config_path.display()))?;

    let rows = registry
        .rewrite_paths(&|value| rewrite_path(value, plan))
        .map_err(|e| e.to_string())?;

    // Read through the links: after step 3 these files live under `saves/`
    // and the install directory reaches them through the link.
    for entry in &updated.emulators {
        let Some(install_dir) = Path::new(&entry.path).parent() else {
            continue;
        };
        for relative in emulator_config_files(entry, profiles) {
            rewrite_text_file(&install_dir.join(relative), plan)?;
        }
    }
    Ok(rows)
}

/// The `;`/newline separated list rewritten item by item and re-joined with
/// `;`. A blank list stays blank.
fn rewrite_entry_list(value: &str, plan: &RewritePlan) -> String {
    let items = split_entry_list(value);
    if items.is_empty() {
        return value.to_string();
    }
    items
        .into_iter()
        .map(|item| rewrite_path(&item, plan).unwrap_or(item))
        .collect::<Vec<_>>()
        .join(";")
}

/// The emulator config files that store absolute library paths, relative to
/// the install directory. Everything else the `ensure_*` writers write is
/// either relative or outside the library.
fn emulator_config_files(
    entry: &crate::config::EmulatorEntry,
    profiles: &[EmulatorProfile],
) -> Vec<&'static str> {
    if crate::autoconfig::is_rpcs3(entry, profiles) {
        return vec!["portable/config/vfs.yml", "portable/config/games.yml"];
    }
    if crate::autoconfig::is_pcsx2(entry, profiles) {
        return vec!["inis/PCSX2.ini"];
    }
    if crate::autoconfig::is_shadps4_qt_launcher(entry, profiles) {
        return vec!["launcher/qt_ui.ini"];
    }
    Vec::new()
}

/// Rewrites every library path inside one text file. A missing file is
/// skipped silently (the emulator has not written it yet); an unreadable one
/// is warned about and skipped; a failed write fails the run, so the next
/// start retries.
fn rewrite_text_file(path: &Path, plan: &RewritePlan) -> Result<(), String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "emulator config file could not be read; left unchanged");
            return Ok(());
        }
    };
    let Some(rewritten) = rewrite_paths_in_text(&text, plan) else {
        return Ok(());
    };
    std::fs::write(path, rewritten)
        .map_err(|e| format!("could not write {}: {e}", path.display()))?;
    tracing::debug!(path = %path.display(), "emulator config file paths rewritten");
    Ok(())
}

/// Step 5: stamp the version. Reloaded from disk so the value lands on the
/// config [`step_rewrite`] just wrote.
fn write_version(config_path: &Path) -> Result<(), String> {
    let mut config = Config::load(config_path)
        .map_err(|e| format!("could not read {}: {e}", config_path.display()))?;
    config.library_layout_version = LAYOUT_VERSION_V1;
    config
        .save(config_path)
        .map_err(|e| format!("could not write {}: {e}", config_path.display()))
}

/// Every move the migration makes, with the cross-filesystem case named
/// explicitly: the library must live on one filesystem, and a `saves/` or
/// `games/` on another would silently be a copy, not a move.
fn rename_in_library(source: &Path, dest: &Path) -> Result<(), String> {
    std::fs::rename(source, dest).map_err(|e| {
        if e.kind() == std::io::ErrorKind::CrossesDevices {
            format!(
                "{} and {} are on different filesystems; the whole library must be on one",
                source.display(),
                dest.display()
            )
        } else {
            format!(
                "could not move {} to {}: {e}",
                source.display(),
                dest.display()
            )
        }
    })
}

/// [`rewrite_path`] applied to every library path inside a text file,
/// leaving every other byte untouched. `None` when nothing changed.
///
/// A path token starts at a library-root spelling and runs to the next
/// quote, carriage return or newline — spaces are part of a path (`PCSX2
/// (Playstation 2)-latest`), quotes and line ends never are.
pub fn rewrite_paths_in_text(text: &str, plan: &RewritePlan) -> Option<String> {
    let mut out = String::with_capacity(text.len());
    let mut changed = false;
    let mut index = 0;
    while index < text.len() {
        let rest = &text[index..];
        if plan
            .library_forms
            .iter()
            .any(|form| rest.starts_with(form.as_str()))
        {
            let end = rest.find(['"', '\'', '\n', '\r']).unwrap_or(rest.len());
            let token = &rest[..end];
            match rewrite_path(token, plan) {
                Some(rewritten) => {
                    out.push_str(&rewritten);
                    changed = true;
                }
                None => out.push_str(token),
            }
            index += token.len();
            continue;
        }
        let next = rest.chars().next().unwrap_or_default();
        out.push(next);
        index += next.len_utf8();
    }
    if changed {
        Some(out)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, EmulatorEntry};
    use crate::launch::profiles::load_profiles;
    use crate::library::registry::InstalledGame;
    use crate::library::user_data_links::is_link;
    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};

    const LIB: &str = "/lib";

    fn plan_with(game_dirs: &[&str], emulator_dirs: &[(&str, &str)]) -> RewritePlan {
        RewritePlan {
            library_forms: vec![LIB.to_string()],
            game_dirs: game_dirs.iter().map(|s| s.to_string()).collect(),
            emulator_dirs: emulator_dirs
                .iter()
                .map(|(d, n)| (d.to_string(), n.to_string()))
                .collect(),
        }
    }

    #[test]
    fn derive_game_dirs_table() {
        let dir = tempfile::tempdir().unwrap();
        let library = dir.path();
        let mut config = Config {
            library_path: library.to_string_lossy().into_owned(),
            ..Config::default()
        };
        let lib = library.to_string_lossy().into_owned();

        let rows = vec![InstalledGame {
            extracted_path: format!("{lib}/Sony PlayStation 2/Game B/game.iso"),
            ps3_trophy_paths: format!("[\"{lib}/PlayStation 3/.vfs/dev_hdd0/trophy\"]"),
            multi_file_game_dir: format!("{lib}/games/Already/Moved"),
            native_wineprefix: format!("{lib}/emulators/Thing/prefix"),
            native_game_dir: format!("{lib}/Emulators/Thing/dir"),
            archive_path: format!("{lib}/saves/Thing/x.zip"),
            ps3_iso_path: format!("{lib}/.hidden/game.iso"),
            native_executable_path: "/elsewhere/game.exe".to_string(),
            ..Default::default()
        }];
        config.native_pcgw_save_paths.insert(
            "my game|windows".into(),
            vec![format!("{lib}/Windows/My Game/saves")],
        );

        let derived = derive_game_dirs(&config, &rows, library);
        assert_eq!(
            derived,
            ["Sony PlayStation 2", "Windows", "PlayStation 3"]
                .iter()
                .map(|s| s.to_string())
                .collect::<BTreeSet<String>>()
        );

        // `<library>/PlayStation 3/.vfs` alone is enough, with no row at all.
        std::fs::create_dir_all(library.join("PlayStation 3").join(".vfs")).unwrap();
        let empty_config = Config {
            library_path: lib.clone(),
            ..Config::default()
        };
        assert_eq!(
            derive_game_dirs(&empty_config, &[], library),
            ["PlayStation 3"]
                .iter()
                .map(|s| s.to_string())
                .collect::<BTreeSet<String>>()
        );

        // A `~/lib/...` spelling counts when the config is typed that way.
        let tilde_config = Config {
            library_path: "~/lib".into(),
            ..Config::default()
        };
        let tilde_rows = vec![InstalledGame {
            extracted_dir: "~/lib/Nintendo 64/Game".to_string(),
            ..Default::default()
        }];
        assert!(
            derive_game_dirs(&tilde_config, &tilde_rows, Path::new("/home/u/lib"))
                .contains("Nintendo 64")
        );
    }

    #[test]
    fn rewrite_path_table() {
        let plan = plan_with(
            &["Sony PlayStation 2", "PlayStation 3", "Windows"],
            &[("PCSX2 (Playstation 2)-latest", "PCSX2 (Playstation 2)")],
        );

        assert_eq!(
            rewrite_path("/lib/Sony PlayStation 2/GT3/game.iso", &plan).as_deref(),
            Some("/lib/games/Sony PlayStation 2/GT3/game.iso")
        );
        assert_eq!(rewrite_path("/lib/Misc Stuff/notes.txt", &plan), None);
        assert_eq!(
            rewrite_path(
                "/lib/Emulators/PCSX2 (Playstation 2)-latest/pcsx2.AppImage",
                &plan
            )
            .as_deref(),
            Some("/lib/emulators/PCSX2 (Playstation 2)/pcsx2.AppImage")
        );
        assert_eq!(
            rewrite_path("/lib/Emulators/Unknown-1.0/x", &plan).as_deref(),
            Some("/lib/emulators/Unknown-1.0/x")
        );
        assert_eq!(rewrite_path("/lib/games/X/y", &plan), None);
        assert_eq!(rewrite_path("/lib/emulators/X/y", &plan), None);
        assert_eq!(rewrite_path("/lib/saves/X/y", &plan), None);
        assert_eq!(rewrite_path("/lib/.vfs/x", &plan), None);
        assert_eq!(rewrite_path("/lib", &plan), None);
        assert_eq!(rewrite_path("/elsewhere/x", &plan), None);

        // A `~` spelling is rewritten when the plan carries that form.
        let tilde_plan = RewritePlan {
            library_forms: vec!["~/lib".to_string(), "/home/u/lib".to_string()],
            ..plan_with(&["Windows"], &[])
        };
        assert_eq!(
            rewrite_path("~/lib/Windows/My Game/game.exe", &tilde_plan).as_deref(),
            Some("~/lib/games/Windows/My Game/game.exe")
        );

        // A backslash spelling keeps its backslashes.
        let windows_plan = RewritePlan {
            library_forms: vec!["C:\\lib".to_string()],
            ..plan_with(
                &["Windows"],
                &[("ShadPS4 (Playstation 4)-latest", "ShadPS4 (Playstation 4)")],
            )
        };
        assert_eq!(
            rewrite_path("C:\\lib\\Windows\\My Game\\game.exe", &windows_plan).as_deref(),
            Some("C:\\lib\\games\\Windows\\My Game\\game.exe")
        );
        assert_eq!(
            rewrite_path(
                "C:\\lib\\Emulators\\ShadPS4 (Playstation 4)-latest\\shadps4.exe",
                &windows_plan
            )
            .as_deref(),
            Some("C:\\lib\\emulators\\ShadPS4 (Playstation 4)\\shadps4.exe")
        );
    }

    #[test]
    fn rewrite_paths_in_text_rewrites_only_library_paths() {
        let plan = plan_with(
            &["PlayStation 3", "Windows"],
            &[
                ("PCSX2 (Playstation 2)-latest", "PCSX2 (Playstation 2)"),
                ("ShadPS4 (Playstation 4)-latest", "ShadPS4 (Playstation 4)"),
            ],
        );

        let vfs = concat!(
            "\"/dev_hdd0/\": \"/lib/PlayStation 3/.vfs/dev_hdd0/\"\n",
            "\"/games/\": \"/lib/PlayStation 3/.vfs/games/\"\n",
            "\"/dev_bdvd/\": \"$(EmulatorDir)dev_bdvd/\"\n"
        );
        assert_eq!(
            rewrite_paths_in_text(vfs, &plan).as_deref(),
            Some(concat!(
                "\"/dev_hdd0/\": \"/lib/games/PlayStation 3/.vfs/dev_hdd0/\"\n",
                "\"/games/\": \"/lib/games/PlayStation 3/.vfs/games/\"\n",
                "\"/dev_bdvd/\": \"$(EmulatorDir)dev_bdvd/\"\n"
            ))
        );

        let pcsx2 = "[Folders]\nBios = /lib/Emulators/PCSX2 (Playstation 2)-latest/bios\n";
        assert_eq!(
            rewrite_paths_in_text(pcsx2, &plan).as_deref(),
            Some("[Folders]\nBios = /lib/emulators/PCSX2 (Playstation 2)/bios\n")
        );

        let games_yml = "BLUS00001: /lib/PlayStation 3/.vfs/dev_hdd0/game/BLUS00001/\n";
        assert_eq!(
            rewrite_paths_in_text(games_yml, &plan).as_deref(),
            Some("BLUS00001: /lib/games/PlayStation 3/.vfs/dev_hdd0/game/BLUS00001/\n")
        );

        // A forward-slash value under a backslash-spelled library form.
        let windows_plan = RewritePlan {
            library_forms: vec!["C:\\lib".to_string(), "C:/lib".to_string()],
            ..plan_with(
                &[],
                &[("ShadPS4 (Playstation 4)-latest", "ShadPS4 (Playstation 4)")],
            )
        };
        let qt = "[General]\nversionSelected=C:/lib/Emulators/ShadPS4 (Playstation 4)-latest/shadps4.exe\n";
        assert_eq!(
            rewrite_paths_in_text(qt, &windows_plan).as_deref(),
            Some(
                "[General]\nversionSelected=C:/lib/emulators/ShadPS4 (Playstation 4)/shadps4.exe\n"
            )
        );

        // Nothing to rewrite is `None`, not `Some(same text)`.
        assert_eq!(rewrite_paths_in_text("nothing here\n", &plan), None);
    }

    // --- filesystem migration -------------------------------------------

    /// A temp legacy library in the pre-v1 shape, with the config and the
    /// registry that reference it.
    struct Fixture {
        _dir: tempfile::TempDir,
        library: PathBuf,
        config_path: PathBuf,
        registry: Registry,
    }

    impl Fixture {
        fn lib(&self) -> String {
            self.library.to_string_lossy().into_owned()
        }

        fn config(&self) -> Config {
            Config::load(&self.config_path).unwrap()
        }

        fn run(&self) -> MigrationOutcome {
            super::run(&self.config_path, &self.registry, load_profiles())
        }
    }

    fn write_file(path: &Path, contents: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    fn legacy_fixture() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let library = dir.path().join("library");
        let config_path = dir.path().join("config.toml");
        let registry = Registry::open(&dir.path().join("registry.db")).unwrap();
        let lib = library.to_string_lossy().into_owned();

        write_file(
            &library.join("Super Nintendo Entertainment System/Game A/game.sfc"),
            "snes",
        );
        write_file(&library.join("Sony PlayStation 2/Game B/game.iso"), "ps2");
        write_file(
            &library.join("Windows/My Game/game/MyGame/mygame.exe"),
            "exe",
        );
        write_file(
            &library.join("PlayStation 3/.vfs/dev_hdd0/game/BLUS00001/USRDIR/EBOOT.BIN"),
            "eboot",
        );
        std::fs::create_dir_all(library.join("PlayStation 3/.vfs/games/BLUS00001")).unwrap();
        write_file(&library.join("Misc Stuff/readme.txt"), "leave me alone");
        write_file(&library.join("bare.zip"), "archive");

        let pcsx2_dir = library.join("Emulators/PCSX2 (Playstation 2)-latest");
        write_file(&pcsx2_dir.join("pcsx2.AppImage"), "pcsx2");
        write_file(&pcsx2_dir.join("memcards/slot1.mcd"), "card");
        write_file(
            &pcsx2_dir.join("inis/PCSX2.ini"),
            &format!("[Folders]\nBios = {lib}/Emulators/PCSX2 (Playstation 2)-latest/bios\n"),
        );

        let rpcs3_dir = library.join("Emulators/RPCS3 (Playstation 3)-latest");
        write_file(&rpcs3_dir.join("rpcs3.AppImage"), "rpcs3");
        write_file(
            &rpcs3_dir.join("portable/config/vfs.yml"),
            &format!(
                "\"/dev_hdd0/\": \"{lib}/PlayStation 3/.vfs/dev_hdd0/\"\n\"/games/\": \"{lib}/PlayStation 3/.vfs/games/\"\n"
            ),
        );
        write_file(
            &rpcs3_dir.join("portable/config/games.yml"),
            &format!("BLUS00001: {lib}/PlayStation 3/.vfs/dev_hdd0/game/BLUS00001/\n"),
        );

        write_file(&library.join("Emulators/Shared-latest/shared"), "shared");
        write_file(&dir.path().join("elsewhere/retroarch"), "hand configured");

        let mut config = Config {
            library_path: lib.clone(),
            emulators: vec![
                EmulatorEntry {
                    name: "PCSX2 (Playstation 2)".into(),
                    path: format!("{lib}/Emulators/PCSX2 (Playstation 2)-latest/pcsx2.AppImage"),
                    save_paths: format!("{lib}/Emulators/PCSX2 (Playstation 2)-latest/memcards"),
                    ..Default::default()
                },
                EmulatorEntry {
                    name: "RPCS3 (Playstation 3)".into(),
                    path: format!("{lib}/Emulators/RPCS3 (Playstation 3)-latest/rpcs3.AppImage"),
                    ..Default::default()
                },
                EmulatorEntry {
                    name: "Shared A".into(),
                    path: format!("{lib}/Emulators/Shared-latest/shared"),
                    ..Default::default()
                },
                EmulatorEntry {
                    name: "Shared B".into(),
                    path: format!("{lib}/Emulators/Shared-latest/shared"),
                    ..Default::default()
                },
                EmulatorEntry {
                    name: "Hand Configured".into(),
                    path: dir
                        .path()
                        .join("elsewhere/retroarch")
                        .to_string_lossy()
                        .into_owned(),
                    ..Default::default()
                },
            ],
            ..Config::default()
        };
        config.native_pcgw_save_paths.insert(
            "my game|windows".into(),
            vec![format!("{lib}/Windows/My Game/saves")],
        );
        config.save(&config_path).unwrap();

        for row in [
            InstalledGame {
                title: "Game A".into(),
                platform: "Super Nintendo Entertainment System".into(),
                extracted_path: format!(
                    "{lib}/Super Nintendo Entertainment System/Game A/game.sfc"
                ),
                extracted_dir: format!("{lib}/Super Nintendo Entertainment System/Game A"),
                installed_at: 1,
                ..Default::default()
            },
            InstalledGame {
                title: "Game B".into(),
                platform: "Sony PlayStation 2".into(),
                extracted_path: format!("{lib}/Sony PlayStation 2/Game B/game.iso"),
                extracted_dir: format!("{lib}/Sony PlayStation 2/Game B"),
                installed_at: 1,
                ..Default::default()
            },
            InstalledGame {
                title: "My Game".into(),
                platform: "Windows".into(),
                native_game_dir: format!("{lib}/Windows/My Game/game/MyGame"),
                native_executable_path: format!("{lib}/Windows/My Game/game/MyGame/mygame.exe"),
                installed_at: 1,
                ..Default::default()
            },
            InstalledGame {
                title: "PS3 Game".into(),
                platform: "PlayStation 3".into(),
                ps3_iso_path: format!("{lib}/PlayStation 3/BLUS00001.iso"),
                ps3_trophy_paths: format!(
                    "[\"{lib}/PlayStation 3/.vfs/dev_hdd0/home/00000001/trophy/BLUS00001\"]"
                ),
                installed_at: 1,
                ..Default::default()
            },
        ] {
            registry.upsert(&row).unwrap();
        }

        Fixture {
            _dir: dir,
            library,
            config_path,
            registry,
        }
    }

    /// Every path under `root`, relative and sorted, with a `@` marker for a
    /// link and a `/` suffix for a directory.
    fn tree(root: &Path) -> Vec<String> {
        let mut out = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let relative = path
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned();
                if is_link(&path) {
                    out.push(format!("{relative}@"));
                } else if path.is_dir() {
                    out.push(format!("{relative}/"));
                    stack.push(path);
                } else {
                    out.push(relative);
                }
            }
        }
        out.sort();
        out
    }

    /// Every file under `root` as `(relative path, contents)`, sorted.
    fn contents(root: &Path) -> Vec<(String, String)> {
        let mut out = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if is_link(&path) {
                    continue;
                }
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                let relative = path
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned();
                out.push((relative, std::fs::read_to_string(&path).unwrap_or_default()));
            }
        }
        out.sort();
        out
    }

    fn read(path: &Path) -> String {
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    }

    /// The fixture's end state, asserted by both the straight run and the
    /// resumed run.
    fn assert_migrated_tree(fixture: &Fixture) {
        let library = &fixture.library;
        let lib = fixture.lib();

        assert!(library
            .join("games/Super Nintendo Entertainment System/Game A/game.sfc")
            .is_file());
        assert!(library
            .join("games/Sony PlayStation 2/Game B/game.iso")
            .is_file());
        assert!(library
            .join("games/Windows/My Game/game/MyGame/mygame.exe")
            .is_file());
        assert!(library
            .join("games/PlayStation 3/.vfs/dev_hdd0/game/BLUS00001/USRDIR/EBOOT.BIN")
            .is_file());
        for legacy in [
            "Super Nintendo Entertainment System",
            "Sony PlayStation 2",
            "Windows",
            "PlayStation 3",
            "Emulators",
        ] {
            assert!(
                !entry_names(library).contains(legacy),
                "{legacy} is still at the library root"
            );
        }

        // Unreferenced entries are left exactly where they were.
        assert_eq!(
            read(&library.join("Misc Stuff/readme.txt")),
            "leave me alone"
        );
        assert_eq!(read(&library.join("bare.zip")), "archive");

        assert!(library
            .join("emulators/PCSX2 (Playstation 2)/pcsx2.AppImage")
            .is_file());
        assert!(is_link(
            &library.join("emulators/PCSX2 (Playstation 2)/memcards")
        ));
        assert_eq!(
            read(&library.join("saves/PCSX2 (Playstation 2)/memcards/slot1.mcd")),
            "card"
        );
        assert_eq!(
            read(&library.join("saves/PCSX2 (Playstation 2)/inis/PCSX2.ini")),
            format!("[Folders]\nBios = {lib}/emulators/PCSX2 (Playstation 2)/bios\n")
        );
        assert_eq!(
            read(&library.join("saves/RPCS3 (Playstation 3)/portable/config/vfs.yml")),
            format!(
                "\"/dev_hdd0/\": \"{lib}/games/PlayStation 3/.vfs/dev_hdd0/\"\n\"/games/\": \"{lib}/games/PlayStation 3/.vfs/games/\"\n"
            )
        );
        assert_eq!(
            read(&library.join("saves/RPCS3 (Playstation 3)/portable/config/games.yml")),
            format!("BLUS00001: {lib}/games/PlayStation 3/.vfs/dev_hdd0/game/BLUS00001/\n")
        );
        // Two entries, one directory, no profile: the name is kept.
        assert!(library.join("emulators/Shared-latest/shared").is_file());

        let config = fixture.config();
        assert_eq!(
            config.emulators[0].path,
            format!("{lib}/emulators/PCSX2 (Playstation 2)/pcsx2.AppImage")
        );
        assert_eq!(
            config.emulators[0].save_paths,
            format!("{lib}/emulators/PCSX2 (Playstation 2)/memcards")
        );
        assert_eq!(
            config.emulators[1].path,
            format!("{lib}/emulators/RPCS3 (Playstation 3)/rpcs3.AppImage")
        );
        for index in [2, 3] {
            assert_eq!(
                config.emulators[index].path,
                format!("{lib}/emulators/Shared-latest/shared")
            );
        }
        // The hand-configured entry, outside the library, is untouched.
        assert!(config.emulators[4].path.ends_with("elsewhere/retroarch"));
        assert!(!config.emulators[4].path.contains("emulators/"));
        assert_eq!(
            config.native_pcgw_save_paths.get("my game|windows"),
            Some(&vec![format!("{lib}/games/Windows/My Game/saves")])
        );
        assert_eq!(config.library_layout_version, 1);

        let rows = fixture.registry.all().unwrap();
        let game_a = rows.iter().find(|r| r.title == "Game A").unwrap();
        assert_eq!(
            game_a.extracted_path,
            format!("{lib}/games/Super Nintendo Entertainment System/Game A/game.sfc")
        );
        let my_game = rows.iter().find(|r| r.title == "My Game").unwrap();
        assert_eq!(
            my_game.native_game_dir,
            format!("{lib}/games/Windows/My Game/game/MyGame")
        );
        assert_eq!(
            my_game.native_executable_path,
            format!("{lib}/games/Windows/My Game/game/MyGame/mygame.exe")
        );
        let ps3 = rows.iter().find(|r| r.title == "PS3 Game").unwrap();
        assert_eq!(
            ps3.ps3_iso_path,
            format!("{lib}/games/PlayStation 3/BLUS00001.iso")
        );
        assert_eq!(
            ps3.ps3_trophy_paths,
            format!("[\"{lib}/games/PlayStation 3/.vfs/dev_hdd0/home/00000001/trophy/BLUS00001\"]")
        );
    }

    #[test]
    fn run_migrates_the_fixture() {
        let fixture = legacy_fixture();
        let outcome = fixture.run();
        assert_eq!(
            outcome,
            MigrationOutcome::Completed {
                games_moved: 4,
                emulators_renamed: 2,
                links_changed: 2,
                rows_rewritten: 4,
            }
        );
        assert_migrated_tree(&fixture);
    }

    #[test]
    fn run_is_a_no_op_the_second_time() {
        let fixture = legacy_fixture();
        fixture.run();
        let before_tree = tree(&fixture.library);
        let before_contents = contents(&fixture.library);
        let before_config = read(&fixture.config_path);

        assert_eq!(fixture.run(), MigrationOutcome::Skipped);
        assert_eq!(tree(&fixture.library), before_tree);
        assert_eq!(contents(&fixture.library), before_contents);
        assert_eq!(read(&fixture.config_path), before_config);
    }

    #[test]
    fn an_interrupted_run_resumes_to_the_same_end_state() {
        let straight = legacy_fixture();
        straight.run();

        let fixture = legacy_fixture();
        // Half the platforms already moved, and the emulator root already
        // renamed with the tagged directory names still in place.
        std::fs::create_dir_all(fixture.library.join(GAMES_DIR)).unwrap();
        for name in ["Windows", "PlayStation 3"] {
            std::fs::rename(
                fixture.library.join(name),
                fixture.library.join(GAMES_DIR).join(name),
            )
            .unwrap();
        }
        std::fs::rename(
            fixture.library.join(LEGACY_EMULATORS_DIR),
            fixture.library.join(EMULATORS_DIR),
        )
        .unwrap();

        let outcome = fixture.run();
        assert!(
            matches!(outcome, MigrationOutcome::Completed { .. }),
            "{outcome:?}"
        );
        assert_migrated_tree(&fixture);
        assert_eq!(tree(&fixture.library), tree(&straight.library));
    }

    #[test]
    fn a_games_collision_aborts_without_changes() {
        let fixture = legacy_fixture();
        write_file(
            &fixture.library.join("games/Sony PlayStation 2/already.txt"),
            "in the way",
        );

        let outcome = fixture.run();
        let MigrationOutcome::Failed { message } = outcome else {
            panic!("expected Failed, got {outcome:?}");
        };
        assert!(
            message.contains(
                &fixture
                    .library
                    .join("Sony PlayStation 2")
                    .display()
                    .to_string()
            ) && message.contains(
                &fixture
                    .library
                    .join("games/Sony PlayStation 2")
                    .display()
                    .to_string()
            ),
            "message names both paths: {message}"
        );
        assert!(fixture
            .library
            .join("Sony PlayStation 2/Game B/game.iso")
            .is_file());
        assert!(fixture.library.join("Emulators").is_dir());
        assert!(!fixture.library.join("saves").exists());
        assert_eq!(fixture.config().library_layout_version, 0);
    }

    #[test]
    fn a_missing_library_directory_is_skipped_silently() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        let registry = Registry::open(&dir.path().join("registry.db")).unwrap();
        Config {
            library_path: dir
                .path()
                .join("not-mounted")
                .to_string_lossy()
                .into_owned(),
            ..Config::default()
        }
        .save(&config_path)
        .unwrap();

        assert_eq!(
            super::run(&config_path, &registry, load_profiles()),
            MigrationOutcome::Skipped
        );
        assert_eq!(
            Config::load(&config_path).unwrap().library_layout_version,
            0
        );
    }

    #[test]
    fn a_library_with_no_references_becomes_version_1_without_moving_anything() {
        let dir = tempfile::tempdir().unwrap();
        let library = dir.path().join("foreign");
        let config_path = dir.path().join("config.toml");
        let registry = Registry::open(&dir.path().join("registry.db")).unwrap();
        std::fs::create_dir_all(library.join("Foo")).unwrap();
        write_file(&library.join("Bar/x.txt"), "x");
        Config {
            library_path: library.to_string_lossy().into_owned(),
            ..Config::default()
        }
        .save(&config_path)
        .unwrap();
        let before = tree(&library);

        assert_eq!(
            super::run(&config_path, &registry, load_profiles()),
            MigrationOutcome::Completed {
                games_moved: 0,
                emulators_renamed: 0,
                links_changed: 0,
                rows_rewritten: 0,
            }
        );
        assert_eq!(tree(&library), before);
        assert_eq!(
            Config::load(&config_path).unwrap().library_layout_version,
            1
        );
    }

    #[test]
    fn a_blank_library_path_is_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        let registry = Registry::open(&dir.path().join("registry.db")).unwrap();
        Config::default().save(&config_path).unwrap();

        assert_eq!(
            super::run(&config_path, &registry, load_profiles()),
            MigrationOutcome::Skipped
        );
        assert_eq!(
            Config::load(&config_path).unwrap().library_layout_version,
            0
        );
    }

    #[test]
    fn a_version_1_library_is_skipped() {
        let fixture = legacy_fixture();
        let mut config = fixture.config();
        config.library_layout_version = 1;
        config.save(&fixture.config_path).unwrap();
        let before = tree(&fixture.library);

        assert_eq!(fixture.run(), MigrationOutcome::Skipped);
        assert_eq!(tree(&fixture.library), before);
    }

    #[test]
    fn an_emulator_target_collision_skips_that_entry_only() {
        let fixture = legacy_fixture();
        write_file(
            &fixture
                .library
                .join("emulators/PCSX2 (Playstation 2)/in-the-way.txt"),
            "mine",
        );
        let lib = fixture.lib();

        let outcome = fixture.run();
        assert!(
            matches!(outcome, MigrationOutcome::Completed { .. }),
            "{outcome:?}"
        );
        // PCSX2 keeps its tagged directory, moved only at the root level.
        assert!(fixture
            .library
            .join("emulators/PCSX2 (Playstation 2)-latest/pcsx2.AppImage")
            .is_file());
        assert_eq!(
            read(
                &fixture
                    .library
                    .join("emulators/PCSX2 (Playstation 2)/in-the-way.txt")
            ),
            "mine"
        );
        let config = fixture.config();
        assert_eq!(
            config.emulators[0].path,
            format!("{lib}/emulators/PCSX2 (Playstation 2)-latest/pcsx2.AppImage")
        );
        // Everything else still completed.
        assert!(fixture
            .library
            .join("emulators/RPCS3 (Playstation 3)/rpcs3.AppImage")
            .is_file());
        assert!(fixture
            .library
            .join("games/Sony PlayStation 2/Game B/game.iso")
            .is_file());
        assert_eq!(config.library_layout_version, 1);
    }

    #[test]
    fn the_temp_name_fallback_finishes_a_half_done_root_rename() {
        let fixture = legacy_fixture();
        std::fs::rename(
            fixture.library.join(LEGACY_EMULATORS_DIR),
            fixture.library.join(TEMP_EMULATORS_DIR),
        )
        .unwrap();

        let outcome = fixture.run();
        assert!(
            matches!(outcome, MigrationOutcome::Completed { .. }),
            "{outcome:?}"
        );
        assert!(!entry_names(&fixture.library).contains(TEMP_EMULATORS_DIR));
        assert_migrated_tree(&fixture);
    }

    #[test]
    fn build_rewrite_plan_pairs_only_renamed_directories() {
        let dir = tempfile::tempdir().unwrap();
        let library = dir.path();
        let lib = library.to_string_lossy().into_owned();
        let registry = Registry::open(&library.join("registry.db")).unwrap();

        // Already renamed on disk: `emulators/PCSX2 (Playstation 2)` exists,
        // neither legacy spelling of the tagged name does.
        std::fs::create_dir_all(library.join("emulators").join("PCSX2 (Playstation 2)")).unwrap();
        // Not renamed: the tagged directory is still there.
        std::fs::create_dir_all(library.join("emulators").join("Shared-latest")).unwrap();

        let config = Config {
            library_path: lib.clone(),
            emulators: vec![
                EmulatorEntry {
                    name: "PCSX2 (Playstation 2)".into(),
                    path: format!("{lib}/Emulators/PCSX2 (Playstation 2)-latest/pcsx2.AppImage"),
                    ..Default::default()
                },
                EmulatorEntry {
                    name: "Shared A".into(),
                    path: format!("{lib}/Emulators/Shared-latest/shared"),
                    ..Default::default()
                },
            ],
            ..Config::default()
        };

        let plan = build_rewrite_plan(&config, &registry, library, load_profiles()).unwrap();
        assert_eq!(
            plan.emulator_dirs,
            vec![(
                "PCSX2 (Playstation 2)-latest".to_string(),
                "PCSX2 (Playstation 2)".to_string()
            )]
        );
    }
}
