//! The one-shot startup migration from the flat (pre-v1) library root to the
//! `games/` / `emulators/` / `saves/` layout.
//!
//! The migration is REFERENCE-DRIVEN: the set of legacy platform directories
//! it moves comes from the registry's path columns and the config's save-path
//! fields, never from a directory listing. An unreferenced top-level
//! directory is left exactly where it is, so pointing the config at a
//! populated non-GRID directory moves nothing and still stamps the version.
//!
//! Layout v2 adds one more step on top of that, for a library that already
//! went through v1: the `user_data` links move from beside the executable to
//! the emulator's DATA root ([`super::user_data_links::user_data_root`]),
//! which is `<exe dir>/PCSX2` for PCSX2's AppImage and unchanged for
//! everything else.
//!
//! Every step is idempotent and every move is a `rename` inside the library
//! directory, so an interrupted run resumes on the next start. A
//! `CrossesDevices` rename aborts the run with both paths in the message.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use super::paths::{
    expand_home, sanitize_component, saves_dir, EMULATORS_DIR, GAMES_DIR, LAYOUT_VERSION_CURRENT,
    LAYOUT_VERSION_V1, LEGACY_EMULATORS_DIR, SAVES_DIR,
};
use super::registry::{InstalledGame, Registry};
use super::user_data_links::{ensure_user_data_links, user_data_root};
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
    /// each of those with the separators swapped both ways; trailing
    /// separators trimmed, blanks and duplicates dropped.
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

/// Every spelling of the library root to match stored paths against: the
/// typed form, the `~`-expanded form, and each of those with the separators
/// swapped both ways — a registry value written on Windows may use `\` where
/// the config was typed with `/`, and the other way round.
fn library_forms(config: &Config, library: &Path) -> Vec<String> {
    let mut forms = Vec::new();
    for raw in [
        config.library_path.trim().to_string(),
        library.to_string_lossy().into_owned(),
    ] {
        for form in [raw.clone(), raw.replace('\\', "/"), raw.replace('/', "\\")] {
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
    emulator_dir_and_rest(path, forms).map(|(dir, _)| dir)
}

/// [`emulator_dir_of`] plus the remainder of the path below that directory,
/// with its leading separator: `<library>/Emulators/X/bin/emu` gives
/// `("X", "/bin/emu")`. The remainder is what re-roots a stored executable
/// path onto the install directory's new name.
fn emulator_dir_and_rest(path: &str, forms: &[String]) -> Option<(String, String)> {
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
        return Some((dir.to_string(), rest.to_string()));
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
    /// Nothing to do: already at the current layout, no library configured,
    /// or the library directory is not there (an unmounted drive retries
    /// next start). The version is left untouched.
    Skipped,
    /// Every step finished and the version was stamped. Every count is zero
    /// and `relinked` is empty for a library with nothing referenced in it,
    /// which is still a completed run. A library that was already at v1 also
    /// reports zeros: only the data-root step ran.
    Completed {
        games_moved: usize,
        emulators_renamed: usize,
        links_changed: usize,
        /// Install directories left unlinked because another install of the
        /// same emulator already owns `saves/<Emulator>`.
        skipped_duplicates: usize,
        /// Entries left unlinked because the directory holding their
        /// executable is not on disk.
        skipped_missing: usize,
        rows_rewritten: usize,
        /// The names of the config entries whose `user_data` links moved to
        /// the emulator's data root (layout v2). The app layer resyncs each
        /// one, so the managed keys name the directory the emulator reads.
        relinked: Vec<String>,
    },
    /// A step failed. The version stays at 0, the library stays readable
    /// through the legacy fallbacks, and the next start resumes.
    Failed { message: String },
}

/// Migrates the configured library to the current layout, once.
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
    if config.library_layout_version >= LAYOUT_VERSION_CURRENT {
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

/// The steps, in order. Every one is idempotent, so a run interrupted
/// anywhere resumes here on the next start.
///
/// The four v1 steps run only for a library that never had them: a v1
/// library's directories, names and stored paths are already in place, and
/// only the data-root step is left. That step always runs, for both.
fn migrate(
    config_path: &Path,
    config: &Config,
    library: &Path,
    registry: &Registry,
    profiles: &[EmulatorProfile],
) -> Result<MigrationOutcome, String> {
    let mut games_moved = 0;
    let mut emulators_renamed = 0;
    let mut links_changed = 0;
    let mut skipped_duplicates = 0;
    let mut skipped_missing = 0;
    let mut rows_rewritten = 0;
    let mut current = config.clone();

    if config.library_layout_version < LAYOUT_VERSION_V1 {
        let mut plan =
            build_rewrite_plan(config, registry, library, profiles).map_err(|e| e.to_string())?;
        preflight(library, &plan)?;

        games_moved = step_games(library, &plan)?;
        emulators_renamed = step_emulators(library, config, &plan, profiles)?;
        (links_changed, skipped_duplicates, skipped_missing) =
            step_user_data(library, config, &plan, profiles)?;
        // The pairs describe renames that have LANDED, so they are only
        // complete once step 2 has run. The game set is derived from the
        // registry and the config, which step 1 does not touch, so it stands.
        plan.emulator_dirs = kept_emulator_pairs(config, library, &plan.library_forms, profiles);
        rows_rewritten = step_rewrite(config_path, config, registry, &plan, profiles)?;
        // Step 4 rewrote every stored path and saved the config; the
        // data-root step below must read the NEW paths, not the legacy ones.
        current = Config::load(config_path)
            .map_err(|e| format!("could not read {}: {e}", config_path.display()))?;
    }

    let relinked = step_data_root_links(library, &current, profiles)?;
    write_version(config_path)?;

    Ok(MigrationOutcome::Completed {
        games_moved,
        emulators_renamed,
        links_changed,
        skipped_duplicates,
        skipped_missing,
        rows_rewritten,
        relinked,
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
///
/// Returns `(links changed, installs skipped as duplicates, installs skipped
/// because the executable's directory is not on disk)`.
fn step_user_data(
    library: &Path,
    config: &Config,
    plan: &RewritePlan,
    profiles: &[EmulatorProfile],
) -> Result<(usize, usize, usize), String> {
    let root = library.join(EMULATORS_DIR);
    let names = entry_names(&root);
    let mut changed = 0;
    let mut skipped_duplicates = 0;
    let mut skipped_missing = 0;
    let mut seen: BTreeSet<String> = BTreeSet::new();
    // `saves/<Profile>` → the install directory that filled it. A second
    // install of the same profile must NOT be linked: its own directories
    // would be merged destination-wins into a saves directory the first one
    // just filled, which deletes the second copy.
    let mut populated: BTreeMap<String, PathBuf> = BTreeMap::new();
    for entry in &config.emulators {
        let Some((legacy_dir, rest)) = emulator_dir_and_rest(&entry.path, &plan.library_forms)
        else {
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
        let target = saves_dir(library, &profile.name);

        // Nothing is linked at a guessed location: the install root is not
        // where any reader looks when the entry names a nested executable,
        // so an entry whose directory is not on disk is left alone rather
        // than given links nothing will follow.
        let Some(links_at) = links_directory(&install_dir, &rest) else {
            tracing::warn!(
                expected = %install_dir.join(rest.trim_start_matches(is_separator)).display(),
                "the executable of this emulator entry is not in the install directory; its user data is not linked"
            );
            skipped_missing += 1;
            continue;
        };

        // The links belong at the emulator's DATA root: `<exe dir>/PCSX2`
        // for PCSX2's AppImage, the executable's own directory for every
        // other emulator. `links_at` is that directory, so the executable's
        // file name is all `user_data_root` still needs.
        let exe_name = rest.rsplit(is_separator).next().unwrap_or_default();
        let root = user_data_root(profile, &links_at.join(exe_name));

        if let Some(first) = populated.get(&profile.name) {
            tracing::warn!(
                kept = %first.display(),
                skipped = %install_dir.display(),
                "a second install of this emulator would share one saves directory; it keeps its own files and is not linked"
            );
            skipped_duplicates += 1;
            continue;
        }
        if has_entries(&target) && !owns_links(&root, &target, &profile.user_data) {
            tracing::warn!(
                saves = %target.display(),
                skipped = %install_dir.display(),
                "the saves directory of this emulator already holds files from another install; it keeps its own files and is not linked"
            );
            populated.insert(profile.name.clone(), install_dir);
            skipped_duplicates += 1;
            continue;
        }

        // At the data root, not at the install root: every reader derives
        // the emulator's directory from the executable, never from the
        // directory the archive extracted into.
        std::fs::create_dir_all(&root)
            .map_err(|e| format!("could not create {}: {e}", root.display()))?;
        match ensure_user_data_links(&root, &target, &profile.user_data) {
            Ok(true) => changed += 1,
            Ok(false) => {}
            Err(e) => {
                return Err(format!(
                    "could not link the user data of {}: {e}",
                    root.display()
                ))
            }
        }
        populated.insert(profile.name.clone(), install_dir);
    }
    Ok((changed, skipped_duplicates, skipped_missing))
}

/// The directory the links belong in: the parent of the stored executable
/// re-rooted onto `install_dir`. `None` when that directory is not on disk —
/// the entry points at a tree this migration does not recognize, and the
/// install root is a guess no reader would follow.
fn links_directory(install_dir: &Path, rest: &str) -> Option<PathBuf> {
    let mut components: Vec<&str> = rest.split(is_separator).filter(|s| !s.is_empty()).collect();
    components.pop(); // the executable's own file name
    let mut dir = install_dir.to_path_buf();
    for component in components {
        dir.push(component);
    }
    dir.is_dir().then_some(dir)
}

/// Whether `dir` exists and holds at least one entry.
fn has_entries(dir: &Path) -> bool {
    std::fs::read_dir(dir).is_ok_and(|mut entries| entries.next().is_some())
}

/// Whether ANY `user_data` name under `dir` is a link that resolves INSIDE
/// `target` — the mark of the install that filled that saves directory. A
/// run interrupted part way through one install's list must finish it, so
/// one such link is enough.
///
/// Where the link points is what carries this: a legacy install can be
/// carrying a stale link of its own (to a dead path, or to somewhere else
/// entirely), and treating that as ownership would merge its real
/// directories into a saves directory another install already filled. The
/// only case this cannot tell apart is a duplicate whose links already point
/// at THIS saves directory — which is exactly the interrupted first install.
fn owns_links(dir: &Path, target: &Path, user_data: &[String]) -> bool {
    let Ok(saves) = std::fs::canonicalize(target) else {
        return false;
    };
    user_data.iter().any(|name| {
        let link = dir.join(name.trim());
        super::user_data_links::is_link(&link)
            && super::user_data_links::read_link_target(&link)
                .ok()
                .and_then(|path| std::fs::canonicalize(path).ok())
                .is_some_and(|resolved| resolved.starts_with(&saves))
    })
}

/// Layout v2: move every `user_data` link from beside the executable to the
/// emulator's DATA root, and return the names of the entries that were
/// relinked.
///
/// PCSX2's AppImage reads `<exe dir>/PCSX2` and nothing else, so the links
/// layout v1 left beside the binary named directories that build never
/// opens. Every other emulator's data root IS the executable's directory,
/// which this step recognizes and skips.
///
/// Managed installs only (under `<library>/emulators/`), and only for a
/// matched profile with a non-empty `user_data`. The merge into
/// `saves/<Profile>` keeps the DESTINATION's copy of every collision (user
/// ruling 2026-09-19): those bytes are the managed ones that survived
/// earlier runs. A `portable.ini` beside the executable is left alone — it
/// is inert for an AppImage and load-bearing for every other PCSX2 build.
fn step_data_root_links(
    library: &Path,
    config: &Config,
    profiles: &[EmulatorProfile],
) -> Result<Vec<String>, String> {
    let forms = library_forms(config, library);
    let mut relinked = Vec::new();
    // `saves/<Profile>` → the install that owns it, exactly as in
    // [`step_user_data`]: a second install of one profile would have its own
    // files merged destination-wins into a directory the first one filled.
    let mut populated: BTreeMap<String, PathBuf> = BTreeMap::new();
    for entry in &config.emulators {
        // An entry pointing outside `<library>/emulators/` (or the legacy
        // `Emulators/`) is a hand-configured emulator whose directories GRID
        // does not move.
        if emulator_dir_of(&entry.path, &forms).is_none() {
            continue;
        }
        let Some(profile) =
            crate::launch::profiles::profile_for_entry(&entry.name, &entry.path, profiles)
        else {
            continue;
        };
        if profile.user_data.is_empty() {
            continue;
        }
        // The EXPANDED path: an entry typed with `~/` would otherwise name a
        // literal `~` directory that does not exist.
        let exe = expand_home(entry.path.trim());
        // Nothing is created at a guessed location: an entry whose
        // executable directory is not on disk is left alone, the same rule
        // [`links_directory`] applies in the v1 step.
        let Some(exe_dir) = exe.parent().filter(|dir| dir.is_dir()) else {
            tracing::warn!(
                expected = %exe.display(),
                "the executable of this emulator entry is not on disk; its user data is not relinked"
            );
            continue;
        };
        let root = user_data_root(profile, &exe);
        if root == exe_dir {
            continue; // the data root is the executable's own directory
        }
        if let Some(first) = populated.get(&profile.name) {
            tracing::warn!(
                kept = %first.display(),
                skipped = %exe_dir.display(),
                "a second install of this emulator would share one saves directory; it keeps its own files and is not linked"
            );
            continue;
        }

        remove_stale_links(exe_dir, &profile.user_data);
        let target = saves_dir(library, &profile.name);
        std::fs::create_dir_all(&root)
            .map_err(|e| format!("could not create {}: {e}", root.display()))?;
        ensure_user_data_links(&root, &target, &profile.user_data)
            .map_err(|e| format!("could not link the user data of {}: {e}", root.display()))?;
        populated.insert(profile.name.clone(), exe_dir.to_path_buf());
        relinked.push(entry.name.clone());
    }
    Ok(relinked)
}

/// Removes every `user_data` name beside the executable that is a LINK —
/// layout v1's links into `saves/`, which this emulator never followed.
///
/// A real directory or file is left in place with a warning: those bytes are
/// the user's, `ensure_user_data_links` at the data root does not reach
/// them, and deleting one here would be the only destructive act in the
/// whole migration.
fn remove_stale_links(exe_dir: &Path, user_data: &[String]) {
    for name in user_data {
        let path = exe_dir.join(name.trim());
        if super::user_data_links::is_link(&path) {
            match super::user_data_links::remove_link(&path) {
                Ok(()) => tracing::debug!(
                    path = %path.display(),
                    "user data link beside the executable removed; it is remade at the data root"
                ),
                Err(e) => tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "user data link beside the executable could not be removed; left in place"
                ),
            }
            continue;
        }
        if path.exists() {
            tracing::warn!(
                path = %path.display(),
                "user data beside the executable is a real directory, not a link; left in place"
            );
        }
    }
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
    let updated = rewritten_config(config, plan);

    // ORDER IS LOAD-BEARING. `game_dirs` is derived from exactly two
    // sources: the registry's path columns and the config's save-path
    // fields. Persisting either one first would make a crash before the
    // emulator config files are rewritten unrecoverable — the next run would
    // read paths that already say `games/`, derive an EMPTY game set (the
    // `.vfs` probe cannot fire either, step 1 moved it), rewrite nothing and
    // stamp version 1, leaving RPCS3 and PCSX2 pointing at directories that
    // no longer exist. So the text files go first: once rewritten they are
    // idempotent no-ops, and until then the plan's sources still describe
    // the legacy layout and the whole run resumes.
    rewrite_emulator_configs(&updated, plan, profiles)?;

    let rows = registry
        .rewrite_paths(&|value| rewrite_path(value, plan))
        .map_err(|e| e.to_string())?;

    updated
        .save(config_path)
        .map_err(|e| format!("could not write {}: {e}", config_path.display()))?;
    Ok(rows)
}

/// The config with every library path in v1 form. Pure: nothing is written.
fn rewritten_config(config: &Config, plan: &RewritePlan) -> Config {
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
}

/// The emulator config files that store absolute library paths, rewritten in
/// place. `config` must already be [`rewritten_config`]'s output: the files
/// are located through the NEW install directory path, and read through the
/// links step 3 left behind. Entries outside `<library>/emulators/` are
/// skipped: GRID never writes a config file it does not own.
fn rewrite_emulator_configs(
    config: &Config,
    plan: &RewritePlan,
    profiles: &[EmulatorProfile],
) -> Result<(), String> {
    for entry in &config.emulators {
        // Managed installs only: an entry pointing outside
        // `<library>/emulators/` (or the legacy `Emulators/`) is a
        // hand-configured emulator whose config file GRID does not own.
        if emulator_dir_of(&entry.path, &plan.library_forms).is_none() {
            continue;
        }
        // The DATA root, not the executable's directory: PCSX2's AppImage
        // reads `inis/PCSX2.ini` under `<exe dir>/PCSX2` and nowhere else.
        // `emulator_data_root` expands a `~/` entry path itself, which an
        // entry typed that way needs to name a real file at all.
        let Some(root) = crate::autoconfig::emulator_data_root(entry, profiles) else {
            continue;
        };
        for relative in emulator_config_files(entry, profiles) {
            rewrite_text_file(&root.join(relative), plan)?;
        }
    }
    Ok(())
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

/// The last step: stamp the version. Reloaded from disk so the value lands
/// on the config [`step_rewrite`] just wrote.
fn write_version(config_path: &Path) -> Result<(), String> {
    let mut config = Config::load(config_path)
        .map_err(|e| format!("could not read {}: {e}", config_path.display()))?;
    config.library_layout_version = LAYOUT_VERSION_CURRENT;
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

/// Whether a path token may start after `previous`: the start of the text,
/// any whitespace (space, tab, `\r`, `\n`), a quote, or one of the
/// key/value punctuation marks the three emulator config formats use.
fn starts_a_value(previous: Option<char>) -> bool {
    match previous {
        None => true,
        Some(c) => c.is_whitespace() || matches!(c, '"' | '\'' | '=' | ':' | '('),
    }
}

/// [`rewrite_path`] applied to every library path inside a text file,
/// leaving every other byte untouched. `None` when nothing changed.
///
/// A path token starts at a library-root spelling that begins a value —
/// the byte before it is the start of the text, whitespace, a quote, `=`,
/// `:` or `(` — and runs to the next quote, carriage return or newline:
/// spaces are part of a path (`PCSX2 (Playstation 2)-latest`), quotes and
/// line ends never are.
///
/// The left boundary is what keeps a library at `/lib` from rewriting an
/// unrelated `/other/lib/Windows/x` that merely ENDS with the library's
/// spelling.
pub fn rewrite_paths_in_text(text: &str, plan: &RewritePlan) -> Option<String> {
    let mut out = String::with_capacity(text.len());
    let mut changed = false;
    let mut index = 0;
    let mut previous: Option<char> = None;
    while index < text.len() {
        let rest = &text[index..];
        if starts_a_value(previous)
            && plan
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
            previous = token.chars().next_back();
            continue;
        }
        let next = rest.chars().next().unwrap_or_default();
        out.push(next);
        index += next.len_utf8();
        previous = Some(next);
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

        // The AppImage build's data root is `<exe dir>/PCSX2`, never the
        // executable's own directory (`autoconfig::paths::pcsx2_data_root`),
        // so that is where a legacy install's files really are.
        let pcsx2_dir = library.join("Emulators/PCSX2 (Playstation 2)-latest");
        write_file(&pcsx2_dir.join("pcsx2.AppImage"), "pcsx2");
        write_file(&pcsx2_dir.join("PCSX2/memcards/slot1.mcd"), "card");
        write_file(
            &pcsx2_dir.join("PCSX2/inis/PCSX2.ini"),
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

    /// Every entry the migrated fixture library holds, in `tree` form
    /// (`/` for a directory, `@` for a link). Asserted whole so a stray file
    /// or a missing link fails the test, not just the paths spelled out in
    /// [`assert_migrated_tree`].
    const MIGRATED_TREE: [&str; 56] = [
        "Misc Stuff/",
        "Misc Stuff/readme.txt",
        "bare.zip",
        "emulators/",
        "emulators/PCSX2 (Playstation 2)/",
        "emulators/PCSX2 (Playstation 2)/PCSX2/",
        "emulators/PCSX2 (Playstation 2)/PCSX2/bios@",
        "emulators/PCSX2 (Playstation 2)/PCSX2/cheats@",
        "emulators/PCSX2 (Playstation 2)/PCSX2/inis@",
        "emulators/PCSX2 (Playstation 2)/PCSX2/memcards@",
        "emulators/PCSX2 (Playstation 2)/PCSX2/snaps@",
        "emulators/PCSX2 (Playstation 2)/PCSX2/sstates@",
        "emulators/PCSX2 (Playstation 2)/PCSX2/textures@",
        "emulators/PCSX2 (Playstation 2)/pcsx2.AppImage",
        "emulators/RPCS3 (Playstation 3)/",
        "emulators/RPCS3 (Playstation 3)/portable@",
        "emulators/RPCS3 (Playstation 3)/rpcs3.AppImage",
        "emulators/Shared-latest/",
        "emulators/Shared-latest/shared",
        "games/",
        "games/PlayStation 3/",
        "games/PlayStation 3/.vfs/",
        "games/PlayStation 3/.vfs/dev_hdd0/",
        "games/PlayStation 3/.vfs/dev_hdd0/game/",
        "games/PlayStation 3/.vfs/dev_hdd0/game/BLUS00001/",
        "games/PlayStation 3/.vfs/dev_hdd0/game/BLUS00001/USRDIR/",
        "games/PlayStation 3/.vfs/dev_hdd0/game/BLUS00001/USRDIR/EBOOT.BIN",
        "games/PlayStation 3/.vfs/games/",
        "games/PlayStation 3/.vfs/games/BLUS00001/",
        "games/Sony PlayStation 2/",
        "games/Sony PlayStation 2/Game B/",
        "games/Sony PlayStation 2/Game B/game.iso",
        "games/Super Nintendo Entertainment System/",
        "games/Super Nintendo Entertainment System/Game A/",
        "games/Super Nintendo Entertainment System/Game A/game.sfc",
        "games/Windows/",
        "games/Windows/My Game/",
        "games/Windows/My Game/game/",
        "games/Windows/My Game/game/MyGame/",
        "games/Windows/My Game/game/MyGame/mygame.exe",
        "saves/",
        "saves/PCSX2 (Playstation 2)/",
        "saves/PCSX2 (Playstation 2)/bios/",
        "saves/PCSX2 (Playstation 2)/cheats/",
        "saves/PCSX2 (Playstation 2)/inis/",
        "saves/PCSX2 (Playstation 2)/inis/PCSX2.ini",
        "saves/PCSX2 (Playstation 2)/memcards/",
        "saves/PCSX2 (Playstation 2)/memcards/slot1.mcd",
        "saves/PCSX2 (Playstation 2)/snaps/",
        "saves/PCSX2 (Playstation 2)/sstates/",
        "saves/PCSX2 (Playstation 2)/textures/",
        "saves/RPCS3 (Playstation 3)/",
        "saves/RPCS3 (Playstation 3)/portable/",
        "saves/RPCS3 (Playstation 3)/portable/config/",
        "saves/RPCS3 (Playstation 3)/portable/config/games.yml",
        "saves/RPCS3 (Playstation 3)/portable/config/vfs.yml",
    ];

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
            &library.join("emulators/PCSX2 (Playstation 2)/PCSX2/memcards")
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
        assert_eq!(config.library_layout_version, LAYOUT_VERSION_CURRENT);

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
                skipped_duplicates: 0,
                skipped_missing: 0,
                rows_rewritten: 4,
                relinked: vec!["PCSX2 (Playstation 2)".to_string()],
            }
        );
        assert_migrated_tree(&fixture);
        assert_eq!(tree(&fixture.library), MIGRATED_TREE);
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
    fn a_failure_after_the_text_rewrites_still_resumes_to_the_same_end_state() {
        let fixture = legacy_fixture();
        let config = fixture.config();
        let library = &fixture.library;
        let profiles = load_profiles();

        // Steps 1-3 plus the emulator text files only — exactly the state a
        // crash between the text rewrites and the registry transaction
        // leaves behind: the config and the registry still describe the
        // legacy layout, and the version is still 0.
        let mut plan = build_rewrite_plan(&config, &fixture.registry, library, profiles).unwrap();
        preflight(library, &plan).unwrap();
        step_games(library, &plan).unwrap();
        step_emulators(library, &config, &plan, profiles).unwrap();
        step_user_data(library, &config, &plan, profiles).unwrap();
        plan.emulator_dirs = kept_emulator_pairs(&config, library, &plan.library_forms, profiles);
        rewrite_emulator_configs(&rewritten_config(&config, &plan), &plan, profiles).unwrap();
        assert_eq!(fixture.config().library_layout_version, 0);
        assert_eq!(
            fixture.config().emulators[0].path,
            format!(
                "{}/Emulators/PCSX2 (Playstation 2)-latest/pcsx2.AppImage",
                fixture.lib()
            ),
            "the config must still be the legacy one at this point"
        );

        let outcome = fixture.run();
        assert!(
            matches!(outcome, MigrationOutcome::Completed { .. }),
            "{outcome:?}"
        );
        assert_migrated_tree(&fixture);
        assert_eq!(tree(&fixture.library), MIGRATED_TREE);

        // The other persistence boundary: the text files AND the registry
        // transaction landed, the config did not. This is the state that was
        // unrecoverable while the config was written first.
        let second = legacy_fixture();
        let config = second.config();
        let library = &second.library;
        let mut plan = build_rewrite_plan(&config, &second.registry, library, profiles).unwrap();
        preflight(library, &plan).unwrap();
        step_games(library, &plan).unwrap();
        step_emulators(library, &config, &plan, profiles).unwrap();
        step_user_data(library, &config, &plan, profiles).unwrap();
        plan.emulator_dirs = kept_emulator_pairs(&config, library, &plan.library_forms, profiles);
        rewrite_emulator_configs(&rewritten_config(&config, &plan), &plan, profiles).unwrap();
        second
            .registry
            .rewrite_paths(&|value| rewrite_path(value, &plan))
            .unwrap();

        let outcome = second.run();
        assert!(
            matches!(outcome, MigrationOutcome::Completed { .. }),
            "{outcome:?}"
        );
        assert_migrated_tree(&second);
        assert_eq!(tree(&second.library), MIGRATED_TREE);
    }

    /// The regression pin for the ordering inside step 4: a text-file write
    /// that fails must leave the config and the registry — the two sources
    /// `game_dirs` is derived from — exactly as they were. Persisting either
    /// one first would make this state unrecoverable: the next run would
    /// read paths that already say `games/`, derive an empty game set and
    /// stamp version 1 with `vfs.yml` still pointing at the old directory.
    #[cfg(unix)]
    #[test]
    fn a_failed_text_rewrite_leaves_the_config_and_registry_unwritten() {
        use std::os::unix::fs::PermissionsExt;

        let fixture = legacy_fixture();
        let lib = fixture.lib();
        let vfs = fixture
            .library
            .join("Emulators/RPCS3 (Playstation 3)-latest/portable/config/vfs.yml");
        std::fs::set_permissions(&vfs, std::fs::Permissions::from_mode(0o444)).unwrap();

        let outcome = fixture.run();
        assert!(
            matches!(outcome, MigrationOutcome::Failed { .. }),
            "{outcome:?}"
        );
        let config = fixture.config();
        assert_eq!(config.library_layout_version, 0);
        assert_eq!(
            config.emulators[0].path,
            format!("{lib}/Emulators/PCSX2 (Playstation 2)-latest/pcsx2.AppImage"),
            "the config must not be persisted before the text files are rewritten"
        );
        assert_eq!(
            config.native_pcgw_save_paths.get("my game|windows"),
            Some(&vec![format!("{lib}/Windows/My Game/saves")])
        );
        let rows = fixture.registry.all().unwrap();
        assert_eq!(
            rows.iter()
                .find(|r| r.title == "Game A")
                .unwrap()
                .extracted_path,
            format!("{lib}/Super Nintendo Entertainment System/Game A/game.sfc"),
            "the registry must not be rewritten before the text files are"
        );

        // With the file writable again the next run finishes everything.
        let moved_vfs = fixture
            .library
            .join("saves/RPCS3 (Playstation 3)/portable/config/vfs.yml");
        std::fs::set_permissions(&moved_vfs, std::fs::Permissions::from_mode(0o644)).unwrap();
        let outcome = fixture.run();
        assert!(
            matches!(outcome, MigrationOutcome::Completed { .. }),
            "{outcome:?}"
        );
        assert_migrated_tree(&fixture);
        assert_eq!(tree(&fixture.library), MIGRATED_TREE);
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
    fn a_library_with_no_references_becomes_the_current_version_without_moving_anything() {
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
                skipped_duplicates: 0,
                skipped_missing: 0,
                rows_rewritten: 0,
                relinked: Vec::new(),
            }
        );
        assert_eq!(tree(&library), before);
        assert_eq!(
            Config::load(&config_path).unwrap().library_layout_version,
            LAYOUT_VERSION_CURRENT
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
    fn a_library_at_the_current_version_is_skipped() {
        let fixture = legacy_fixture();
        let mut config = fixture.config();
        config.library_layout_version = LAYOUT_VERSION_CURRENT;
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
        assert_eq!(config.library_layout_version, LAYOUT_VERSION_CURRENT);
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
    fn text_rewrite_ignores_an_unrelated_path_that_contains_the_library_as_a_suffix() {
        let plan = plan_with(&["Windows"], &[]);
        // `/other/lib` merely ENDS with the library's spelling; the path
        // token starts at `/other`, so nothing in it is a library path.
        assert_eq!(
            rewrite_paths_in_text("Path = /other/lib/Windows/x\n", &plan),
            None
        );
        // The same line with a real library path is still rewritten.
        assert_eq!(
            rewrite_paths_in_text("Path = /lib/Windows/x\n", &plan).as_deref(),
            Some("Path = /lib/games/Windows/x\n")
        );
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

    // --- user data links (item 1 and 2) ----------------------------------

    /// A library with no games and the emulator entries `build` writes, so a
    /// single behavior can be exercised without the whole legacy fixture.
    fn emulator_fixture(build: impl FnOnce(&Path, &Path) -> Vec<EmulatorEntry>) -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let library = dir.path().join("library");
        std::fs::create_dir_all(&library).unwrap();
        let config_path = dir.path().join("config.toml");
        let registry = Registry::open(&dir.path().join("registry.db")).unwrap();

        let emulators = build(dir.path(), &library);
        Config {
            library_path: library.to_string_lossy().into_owned(),
            emulators,
            ..Config::default()
        }
        .save(&config_path)
        .unwrap();

        Fixture {
            _dir: dir,
            library,
            config_path,
            registry,
        }
    }

    fn pcsx2_entry(name: &str, path: String) -> EmulatorEntry {
        EmulatorEntry {
            name: name.into(),
            path,
            ..Default::default()
        }
    }

    /// The links belong at the data root of the EXECUTABLE, which
    /// `select_executable` may have found in a subdirectory — that is the
    /// path every reader (`autoconfig::emulator_data_root`,
    /// `cloud::ops::emulator_dir_for`) derives from the entry, never the
    /// install root.
    #[cfg(unix)]
    #[test]
    fn a_nested_executable_gets_its_links_at_the_data_root_beside_the_binary() {
        let fixture = emulator_fixture(|_root, library| {
            let install = library.join("Emulators/PCSX2 (Playstation 2)-latest");
            write_file(&install.join("bin/pcsx2.AppImage"), "pcsx2");
            write_file(&install.join("bin/PCSX2/memcards/slot1.mcd"), "card");
            vec![pcsx2_entry(
                "PCSX2 (Playstation 2)",
                format!(
                    "{}/Emulators/PCSX2 (Playstation 2)-latest/bin/pcsx2.AppImage",
                    library.to_string_lossy()
                ),
            )]
        });

        assert!(matches!(fixture.run(), MigrationOutcome::Completed { .. }));

        let install = fixture.library.join("emulators/PCSX2 (Playstation 2)");
        assert!(
            is_link(&install.join("bin/PCSX2/memcards")),
            "the link belongs at the data root beside the binary: {}",
            tree(&install).join(", ")
        );
        assert!(
            !install.join("memcards").exists(),
            "nothing may be created at the install root: {}",
            tree(&install).join(", ")
        );
        assert_eq!(
            read(
                &fixture
                    .library
                    .join("saves/PCSX2 (Playstation 2)/memcards/slot1.mcd")
            ),
            "card"
        );
    }

    /// Two legacy installs of ONE profile share one `saves/<Emulator>`, and
    /// the merge into it is destination-wins — so linking the second would
    /// delete its own files. It is skipped and counted instead.
    #[cfg(unix)]
    #[test]
    fn a_second_install_of_one_profile_is_skipped_rather_than_merged() {
        let fixture = emulator_fixture(|_root, library| {
            let lib = library.to_string_lossy().into_owned();
            for (dir, card) in [
                ("PCSX2 (Playstation 2)-latest", "NEW"),
                ("PCSX2 (Playstation 2)-v1.2", "OLD"),
            ] {
                let install = library.join("Emulators").join(dir);
                write_file(&install.join("pcsx2.AppImage"), "pcsx2");
                write_file(&install.join("PCSX2/memcards/slot1.mcd"), card);
            }
            vec![
                pcsx2_entry(
                    "PCSX2 (Playstation 2)",
                    format!("{lib}/Emulators/PCSX2 (Playstation 2)-latest/pcsx2.AppImage"),
                ),
                pcsx2_entry(
                    "PCSX2 1.2",
                    format!("{lib}/Emulators/PCSX2 (Playstation 2)-v1.2/pcsx2.AppImage"),
                ),
            ]
        });

        let outcome = fixture.run();
        let MigrationOutcome::Completed {
            links_changed,
            skipped_duplicates,
            ..
        } = outcome
        else {
            panic!("{outcome:?}");
        };
        assert_eq!((links_changed, skipped_duplicates), (1, 1));

        let second = fixture
            .library
            .join("emulators/PCSX2 (Playstation 2)-v1.2/PCSX2/memcards");
        assert!(
            !is_link(&second) && second.is_dir(),
            "the second install keeps its own directory: {}",
            tree(&fixture.library).join(", ")
        );
        assert_eq!(read(&second.join("slot1.mcd")), "OLD");
        assert_eq!(
            read(
                &fixture
                    .library
                    .join("saves/PCSX2 (Playstation 2)/memcards/slot1.mcd")
            ),
            "NEW",
            "the first install's card is the one under saves/"
        );
    }

    /// A run interrupted part way through one install's `user_data` list
    /// left the saves directory populated and ONE link behind. That install
    /// still owns the saves directory, so the next run finishes its list
    /// rather than mistaking it for a second install.
    #[cfg(unix)]
    #[test]
    fn a_partly_linked_install_is_finished_not_skipped() {
        let fixture = emulator_fixture(|_root, library| {
            let install = library.join("emulators/PCSX2 (Playstation 2)");
            write_file(&install.join("pcsx2.AppImage"), "pcsx2");
            write_file(&install.join("PCSX2/memcards/slot1.mcd"), "card");
            // `inis` is already linked; `memcards` is not.
            let saves = library.join("saves/PCSX2 (Playstation 2)/inis");
            std::fs::create_dir_all(&saves).unwrap();
            std::os::unix::fs::symlink(&saves, install.join("PCSX2/inis")).unwrap();
            vec![pcsx2_entry(
                "PCSX2 (Playstation 2)",
                format!(
                    "{}/emulators/PCSX2 (Playstation 2)/pcsx2.AppImage",
                    library.to_string_lossy()
                ),
            )]
        });

        let outcome = fixture.run();
        let MigrationOutcome::Completed {
            skipped_duplicates, ..
        } = outcome
        else {
            panic!("{outcome:?}");
        };
        assert_eq!(skipped_duplicates, 0);
        assert!(is_link(
            &fixture
                .library
                .join("emulators/PCSX2 (Playstation 2)/PCSX2/memcards")
        ));
        assert_eq!(
            read(
                &fixture
                    .library
                    .join("saves/PCSX2 (Playstation 2)/memcards/slot1.mcd")
            ),
            "card"
        );
    }

    /// A legacy install can carry a stale link of its own. Ownership of a
    /// populated `saves/<Emulator>` is only ever proved by a link that
    /// RESOLVES there — otherwise this install's real directories would be
    /// merged destination-wins into another install's saves.
    #[cfg(unix)]
    #[test]
    fn a_stale_link_does_not_grant_ownership_of_a_populated_saves_dir() {
        let fixture = emulator_fixture(|_root, library| {
            // The first install already filled the saves directory and is
            // gone from the config.
            write_file(
                &library.join("saves/PCSX2 (Playstation 2)/memcards/slot1.mcd"),
                "FIRST",
            );

            let install = library.join("Emulators/PCSX2 (Playstation 2)-v1.2");
            write_file(&install.join("pcsx2.AppImage"), "pcsx2");
            write_file(&install.join("inis/PCSX2.ini"), "[UI]\n");
            std::os::unix::fs::symlink("/nonexistent", install.join("memcards")).unwrap();

            vec![pcsx2_entry(
                "PCSX2 1.2",
                format!(
                    "{}/Emulators/PCSX2 (Playstation 2)-v1.2/pcsx2.AppImage",
                    library.to_string_lossy()
                ),
            )]
        });

        let outcome = fixture.run();
        let MigrationOutcome::Completed {
            links_changed,
            skipped_duplicates,
            ..
        } = outcome
        else {
            panic!("{outcome:?}");
        };
        assert_eq!((links_changed, skipped_duplicates), (0, 1));

        // The only entry, so step 2 renamed it to the profile name.
        let install = fixture.library.join("emulators/PCSX2 (Playstation 2)");
        assert!(
            !is_link(&install.join("inis")) && install.join("inis").is_dir(),
            "the install keeps its own directories: {}",
            tree(&install).join(", ")
        );
        assert_eq!(read(&install.join("inis/PCSX2.ini")), "[UI]\n");
        assert_eq!(
            read(
                &fixture
                    .library
                    .join("saves/PCSX2 (Playstation 2)/memcards/slot1.mcd")
            ),
            "FIRST",
            "the other install's card is untouched"
        );
    }

    /// The install root is a guess no reader follows, so an entry whose
    /// executable directory is not on disk gets no links at all.
    #[cfg(unix)]
    #[test]
    fn an_entry_whose_executable_directory_is_missing_is_skipped_not_linked_at_the_root() {
        let fixture = emulator_fixture(|_root, library| {
            // The binary is at the install root; the entry says `bin/`.
            let install = library.join("Emulators/PCSX2 (Playstation 2)-latest");
            write_file(&install.join("pcsx2.AppImage"), "pcsx2");
            vec![pcsx2_entry(
                "PCSX2 (Playstation 2)",
                format!(
                    "{}/Emulators/PCSX2 (Playstation 2)-latest/bin/pcsx2.AppImage",
                    library.to_string_lossy()
                ),
            )]
        });

        let outcome = fixture.run();
        let MigrationOutcome::Completed {
            links_changed,
            skipped_missing,
            ..
        } = outcome
        else {
            panic!("{outcome:?}");
        };
        assert_eq!((links_changed, skipped_missing), (0, 1));

        let install = fixture.library.join("emulators/PCSX2 (Playstation 2)");
        let entries = tree(&install);
        assert!(
            !entries.iter().any(|e| e.ends_with('@')),
            "nothing may be linked at a guessed location: {}",
            entries.join(", ")
        );
        assert!(!fixture.library.join("saves").exists());
    }

    // --- config rewriting (item 3 and 4) ---------------------------------

    /// A hand-configured emulator outside the library is not GRID's to
    /// write: its config file is left byte-identical even when it names a
    /// path the migration is moving.
    #[test]
    fn a_config_file_outside_the_library_is_never_rewritten() {
        let fixture = emulator_fixture(|root, library| {
            std::fs::create_dir_all(library.join("PlayStation 3/.vfs/dev_hdd0")).unwrap();
            let outside = root.join("elsewhere");
            write_file(&outside.join("rpcs3.AppImage"), "rpcs3");
            write_file(
                &outside.join("portable/config/vfs.yml"),
                &format!(
                    "\"/dev_hdd0/\": \"{}/PlayStation 3/.vfs/dev_hdd0/\"\n",
                    library.to_string_lossy()
                ),
            );
            vec![EmulatorEntry {
                name: "RPCS3 (Playstation 3)".into(),
                path: outside
                    .join("rpcs3.AppImage")
                    .to_string_lossy()
                    .into_owned(),
                ..Default::default()
            }]
        });
        let vfs = fixture
            ._dir
            .path()
            .join("elsewhere/portable/config/vfs.yml");
        let before = read(&vfs);

        assert!(matches!(fixture.run(), MigrationOutcome::Completed { .. }));

        assert_eq!(read(&vfs), before, "an unmanaged config file is untouched");
    }

    /// An entry path typed with `~/` names a real file only once expanded —
    /// the config file beside it must still be found and rewritten.
    #[cfg(unix)]
    #[test]
    fn a_tilde_entry_path_still_has_its_config_rewritten() {
        let _lock = crate::test_env::lock();
        let fixture = emulator_fixture(|_root, library| {
            let install = library.join("Emulators/PCSX2 (Playstation 2)-latest");
            write_file(&install.join("pcsx2.AppImage"), "pcsx2");
            write_file(
                &install.join("PCSX2/inis/PCSX2.ini"),
                "[Folders]\nBios = ~/library/Emulators/PCSX2 (Playstation 2)-latest/bios\n",
            );
            vec![pcsx2_entry(
                "PCSX2 (Playstation 2)",
                "~/library/Emulators/PCSX2 (Playstation 2)-latest/pcsx2.AppImage".to_string(),
            )]
        });
        // The library the config points at is `$HOME/library`, which is
        // exactly where the fixture built it.
        let home = fixture._dir.path().to_string_lossy().into_owned();
        let mut config = fixture.config();
        config.library_path = "~/library".into();
        config.save(&fixture.config_path).unwrap();

        let _guard = crate::test_env::EnvGuard::set(&[("HOME", Some(home.as_str()))]);
        assert!(matches!(fixture.run(), MigrationOutcome::Completed { .. }));
        drop(_guard);

        assert_eq!(
            read(
                &fixture
                    .library
                    .join("saves/PCSX2 (Playstation 2)/inis/PCSX2.ini")
            ),
            "[Folders]\nBios = ~/library/emulators/PCSX2 (Playstation 2)/bios\n"
        );
    }
    // --- the data root step (layout v2) ----------------------------------

    /// PCSX2's `user_data` directory names, in catalog order.
    const PCSX2_USER_DATA: [&str; 7] = [
        "bios", "cheats", "inis", "memcards", "snaps", "sstates", "textures",
    ];

    /// Stamps the fixture as a finished layout v1 library, so only the
    /// data-root step runs.
    fn stamp_v1(fixture: &Fixture) {
        let mut config = fixture.config();
        config.library_layout_version = LAYOUT_VERSION_V1;
        config.save(&fixture.config_path).unwrap();
    }

    /// A legacy (v0) library whose PCSX2 install is an AppImage: the links
    /// belong under `PCSX2/`, which is the only data root that build reads,
    /// and nothing may be linked beside the executable.
    #[cfg(unix)]
    #[test]
    fn a_v0_appimage_install_is_linked_at_its_pcsx2_data_root() {
        let fixture = emulator_fixture(|_root, library| {
            let install = library.join("Emulators/PCSX2 (Playstation 2)-latest");
            write_file(&install.join("pcsx2-2.5.0.AppImage"), "pcsx2");
            write_file(&install.join("PCSX2/memcards/Mcd001.ps2"), "card");
            vec![pcsx2_entry(
                "PCSX2 (Playstation 2)",
                format!(
                    "{}/Emulators/PCSX2 (Playstation 2)-latest/pcsx2-2.5.0.AppImage",
                    library.to_string_lossy()
                ),
            )]
        });

        assert!(matches!(fixture.run(), MigrationOutcome::Completed { .. }));

        let install = fixture.library.join("emulators/PCSX2 (Playstation 2)");
        for name in PCSX2_USER_DATA {
            assert!(
                is_link(&install.join("PCSX2").join(name)),
                "{name} belongs under PCSX2/: {}",
                tree(&install).join(", ")
            );
            assert!(
                !install.join(name).exists(),
                "nothing may be linked beside the executable: {}",
                tree(&install).join(", ")
            );
        }
        assert_eq!(
            read(
                &fixture
                    .library
                    .join("saves/PCSX2 (Playstation 2)/memcards/Mcd001.ps2")
            ),
            "card"
        );
    }

    /// The repair a layout v1 library needs: the links beside the AppImage
    /// are removed, the real directories under `PCSX2/` are moved into
    /// `saves/` destination-wins (user ruling 2026-09-19), and the entry is
    /// reported so the app layer can resync its config file.
    #[cfg(unix)]
    #[test]
    fn a_v1_appimage_install_has_its_links_moved_to_the_data_root() {
        let fixture = emulator_fixture(|_root, library| {
            let install = library.join("emulators/PCSX2 (Playstation 2)");
            let saves = library.join("saves/PCSX2 (Playstation 2)");
            write_file(&install.join("pcsx2-2.5.0.AppImage"), "pcsx2");
            // Layout v1's links, beside the executable, where PCSX2 never
            // looked.
            for name in PCSX2_USER_DATA {
                std::fs::create_dir_all(saves.join(name)).unwrap();
                std::os::unix::fs::symlink(
                    format!("../../saves/PCSX2 (Playstation 2)/{name}"),
                    install.join(name),
                )
                .unwrap();
            }
            write_file(&saves.join("inis/PCSX2.ini"), "GRID\n");
            write_file(&saves.join("bios/x.bin"), "bios");
            // What PCSX2 itself wrote, at the root it really uses.
            write_file(&install.join("PCSX2/inis/PCSX2.ini"), "LIVE\n");
            write_file(&install.join("PCSX2/memcards/Mcd001.ps2"), "card");
            vec![pcsx2_entry(
                "PCSX2 (Playstation 2)",
                format!(
                    "{}/emulators/PCSX2 (Playstation 2)/pcsx2-2.5.0.AppImage",
                    library.to_string_lossy()
                ),
            )]
        });
        stamp_v1(&fixture);

        assert_eq!(
            fixture.run(),
            MigrationOutcome::Completed {
                games_moved: 0,
                emulators_renamed: 0,
                links_changed: 0,
                skipped_duplicates: 0,
                skipped_missing: 0,
                rows_rewritten: 0,
                relinked: vec!["PCSX2 (Playstation 2)".to_string()],
            }
        );

        let install = fixture.library.join("emulators/PCSX2 (Playstation 2)");
        let saves = fixture.library.join("saves/PCSX2 (Playstation 2)");
        for name in PCSX2_USER_DATA {
            assert!(
                !install.join(name).exists() && !is_link(&install.join(name)),
                "the link beside the executable must be gone: {}",
                tree(&install).join(", ")
            );
            assert!(
                is_link(&install.join("PCSX2").join(name)),
                "{name} belongs under PCSX2/: {}",
                tree(&install).join(", ")
            );
        }
        // The managed copy wins the collision; the emulator's own is dropped.
        assert_eq!(read(&saves.join("inis/PCSX2.ini")), "GRID\n");
        assert_eq!(read(&saves.join("bios/x.bin")), "bios");
        assert_eq!(read(&saves.join("memcards/Mcd001.ps2")), "card");
        assert_eq!(
            fixture.config().library_layout_version,
            LAYOUT_VERSION_CURRENT
        );

        // Already at the current version: the second run does nothing.
        let before_tree = tree(&fixture.library);
        let before_contents = contents(&fixture.library);
        assert_eq!(fixture.run(), MigrationOutcome::Skipped);
        assert_eq!(tree(&fixture.library), before_tree);
        assert_eq!(contents(&fixture.library), before_contents);
    }

    /// Two installs of one profile share one `saves/<Emulator>`, and the
    /// merge into it is destination-wins — so the second is skipped here for
    /// the same reason the v1 step skips it.
    #[cfg(unix)]
    #[test]
    fn the_data_root_step_skips_a_second_install_of_one_profile() {
        let fixture = emulator_fixture(|_root, library| {
            let lib = library.to_string_lossy().into_owned();
            for (dir, card) in [("PCSX2 (Playstation 2)", "NEW"), ("PCSX2-v1.2", "OLD")] {
                let install = library.join("emulators").join(dir);
                write_file(&install.join("pcsx2-2.5.0.AppImage"), "pcsx2");
                write_file(&install.join("PCSX2/memcards/Mcd001.ps2"), card);
            }
            vec![
                pcsx2_entry(
                    "PCSX2 (Playstation 2)",
                    format!("{lib}/emulators/PCSX2 (Playstation 2)/pcsx2-2.5.0.AppImage"),
                ),
                pcsx2_entry(
                    "PCSX2 1.2",
                    format!("{lib}/emulators/PCSX2-v1.2/pcsx2-2.5.0.AppImage"),
                ),
            ]
        });
        stamp_v1(&fixture);

        let outcome = fixture.run();
        let MigrationOutcome::Completed { relinked, .. } = outcome else {
            panic!("{outcome:?}");
        };
        assert_eq!(relinked, vec!["PCSX2 (Playstation 2)".to_string()]);

        let second = fixture.library.join("emulators/PCSX2-v1.2/PCSX2/memcards");
        assert!(
            !is_link(&second) && second.is_dir(),
            "the second install keeps its own directory: {}",
            tree(&fixture.library).join(", ")
        );
        assert_eq!(read(&second.join("Mcd001.ps2")), "OLD");
        assert_eq!(
            read(
                &fixture
                    .library
                    .join("saves/PCSX2 (Playstation 2)/memcards/Mcd001.ps2")
            ),
            "NEW",
            "the first install's card is the one under saves/"
        );
    }

    /// A PCSX2 build that is not an AppImage reads the executable's own
    /// directory, which is where layout v1 already put its links: nothing to
    /// relink, and the version is still stamped.
    #[cfg(unix)]
    #[test]
    fn a_non_appimage_install_is_left_alone_by_the_data_root_step() {
        let fixture = emulator_fixture(|_root, library| {
            let install = library.join("emulators/PCSX2 (Playstation 2)");
            let saves = library.join("saves/PCSX2 (Playstation 2)");
            write_file(&install.join("pcsx2-qt.exe"), "pcsx2");
            for name in PCSX2_USER_DATA {
                std::fs::create_dir_all(saves.join(name)).unwrap();
                std::os::unix::fs::symlink(
                    format!("../../saves/PCSX2 (Playstation 2)/{name}"),
                    install.join(name),
                )
                .unwrap();
            }
            write_file(&saves.join("inis/PCSX2.ini"), "GRID\n");
            vec![pcsx2_entry(
                "PCSX2 (Playstation 2)",
                format!(
                    "{}/emulators/PCSX2 (Playstation 2)/pcsx2-qt.exe",
                    library.to_string_lossy()
                ),
            )]
        });
        stamp_v1(&fixture);
        let before = tree(&fixture.library);

        let outcome = fixture.run();
        let MigrationOutcome::Completed { relinked, .. } = outcome else {
            panic!("{outcome:?}");
        };
        assert!(relinked.is_empty(), "{relinked:?}");
        assert_eq!(tree(&fixture.library), before);
        assert_eq!(
            fixture.config().library_layout_version,
            LAYOUT_VERSION_CURRENT
        );
    }
}
