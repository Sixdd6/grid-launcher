//! Sync-directory resolution: which local directories/files an emulator
//! entry's saves, states, and screenshots live in.
//!
//! Ports `grid_launcher/ui/mixins/cloud_mixin.py`'s
//! `_resolved_sync_directory_paths` (:618-981) and
//! `_resolved_screenshot_directories` (:983-1027). See
//! `docs/porting/06-cloud-saves.md` ("Local paths scanned", lines 95-152)
//! for the narrative contract.
//!
//! Resolution order, per `_resolved_sync_directory_paths`:
//! 1. The entry's own `save_paths`/`state_paths` string, split
//!    (`_split_configured_paths` / `profiles.py:133`'s
//!    `split_configured_paths`). Non-empty wins outright — every
//!    per-emulator override block below is skipped (`if not
//!    configured_paths and ...` guards every one, cloud_mixin.py:647-919).
//! 2. Otherwise the matched autoprofile's `save_directories`/
//!    `state_directories` list.
//! 3. Emulator-specific override paths are prepended (RetroArch prepends
//!    its configured `savefile_directory`/`savestate_directory` and
//!    APPENDS the literal fallbacks `saves`,`savefiles` /
//!    `states`,`savestates` — cloud_mixin.py:647-662; every other family
//!    below just prepends its reader's output and dedupes: Azahar (663),
//!    Dolphin (687), PCSX2 (711), RPCS3 (735, save only), Vita3k (750,
//!    save only), Cemu (765, save only), PICO-8 (780, save only), FBNeo
//!    (795), MAME (819), Eden (843, save only), Xenia (858), Redream
//!    (882), xemu (906, save only)).
//! 4. Each raw path is expanded: OS environment variables
//!    (`os.path.expandvars`, POSIX `$VAR`/`${VAR}` subset — this crate's
//!    established convention, see `autoconfig::readers::expand_vars`),
//!    then the four GRID-defined tokens `%EMULATOR_DIR%`, `%LIBRARY_DIR%`,
//!    `%CONFIG_DIR%`, `%DOCUMENTS%` (cloud_mixin.py:939-948).
//! 5. RetroArch-only: the literal (case-insensitive) value `default`
//!    becomes `<emulator_dir>/saves` or `<emulator_dir>/states`; a leading
//!    `:\` or `:/` marks a path relative to the emulator root and is
//!    stripped (cloud_mixin.py:950-958). Otherwise relative paths resolve
//!    against the emulator directory; absolute paths resolve as-is
//!    (cloud_mixin.py:959-964).
//! 6. A resolved candidate is kept only if it exists as a directory OR a
//!    file (cloud_mixin.py:966). Results are de-duplicated
//!    case-insensitively (cloud_mixin.py:969).
//!
//! `_resolved_screenshot_directories` reuses the same token expansion
//! MINUS `%DOCUMENTS%` and keeps only directories; it reads only the
//! autoprofile's `screenshot_directories` — there is no per-entry override
//! (cloud_mixin.py:983-1027).
//!
//! Two things this module deliberately does NOT do, both explicitly out of
//! scope per doc 06's "Out of scope" section:
//! - Memoize per `(name, path, key)` — Python's `_sync_directory_paths_cache`
//!   is a UI-layer concern; the ops layer (Task 16) owns the cache.
//! - Call `_ensure_emulator_sync_settings` (cloud_mixin.py:646) — that
//!   writer belongs to doc 05/the ops layer, not path *resolution*.
//!
//! **Brief-vs-Python signature deviations** (Python wins, per this task's
//! own pinned rule — both documented here and in the task report):
//! - [`expand_sync_path`] gained an `is_retroarch: bool` parameter beyond
//!   the brief's `(raw, key_is_save, ctx)` triple. Python only applies the
//!   `default`/`:\`/`:/ ` notations when
//!   `self._is_retroarch_emulator_name(...)` is true for the CURRENT
//!   entry (cloud_mixin.py:951,954) — applying them unconditionally, as
//!   the brief's inline comment ("+ retroarch notations") could be read to
//!   imply, would treat a non-RetroArch entry's literal path `"default"` as
//!   the sentinel instead of a literal relative directory named `default`,
//!   which is not what Python does. The bool is computed once by
//!   [`resolved_sync_directory_paths`] via [`autoconfig::is_retroarch`] and
//!   threaded through.
//! - RetroArch AppImages resolve `~`, `default` and `:/` against the
//!   AppImage's portable home (`<AppImage>.home/.config/retroarch`, the
//!   `$HOME` the AppImage runtime hands RetroArch) when one exists, and
//!   gain that directory's `saves`/`states` as extra fallbacks. Python
//!   expanded `~` against the REAL user home and only ever fell back to
//!   `<emulator_dir>/saves|states`, neither of which a portable install
//!   writes to, so no save or state was ever found for one — see
//!   `docs/porting/06-cloud-saves.md`.
//! - [`resolved_sync_directory_paths`] takes `profile: Option<&EmulatorProfile>`
//!   (the entry's ALREADY-MATCHED autoprofile), not the full profiles list
//!   `emulator_matches_tokens`/`is_retroarch`/etc. want. Every per-emulator
//!   predicate call here re-derives a 0-or-1-element profiles slice from
//!   that single `profile` via a local `profile_slice` helper. This is
//!   behaviorally sound ONLY because `profile` is assumed to be exactly
//!   what `launch::profiles::profile_for_entry(entry.name, entry.path,
//!   profiles)` already returned for this entry elsewhere — re-matching
//!   that same profile against a singleton slice containing only itself
//!   reproduces the same match. Passing an unrelated profile here would not
//!   reproduce Python's behavior; that precondition is the caller's job.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::autoconfig::{self, paths, readers, retroarch};
use crate::config::EmulatorEntry;
use crate::launch::{profiles::EmulatorProfile, template};

/// Which configured-path list `resolved_sync_directory_paths` resolves:
/// `save_paths`/`save_directories` or `state_paths`/`state_directories`
/// (cloud_mixin.py's `key` parameter, always the literal string
/// `"save_paths"` or `"state_paths"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathKey {
    SavePaths,
    StatePaths,
}

impl PathKey {
    fn is_save(self) -> bool {
        matches!(self, PathKey::SavePaths)
    }

    fn entry_field(self, entry: &EmulatorEntry) -> &str {
        match self {
            PathKey::SavePaths => &entry.save_paths,
            PathKey::StatePaths => &entry.state_paths,
        }
    }

    fn profile_field(self, profile: &EmulatorProfile) -> &[String] {
        match self {
            PathKey::SavePaths => &profile.save_directories,
            PathKey::StatePaths => &profile.state_directories,
        }
    }
}

/// The context every raw path is expanded against — `%EMULATOR_DIR%`,
/// `%LIBRARY_DIR%`, `%CONFIG_DIR%`, `%DOCUMENTS%`, and (for RetroArch) the
/// base directory relative paths resolve against.
pub struct ResolveContext<'a> {
    /// The parent directory of the emulator executable
    /// (`emulator_path.parent`, cloud_mixin.py:929-930) — `None` when the
    /// entry's `path` is blank (Python's `emulator_dir = Path()` in that
    /// case; callers get the same "current directory" fallback by passing
    /// `None`).
    pub emulator_dir: Option<&'a Path>,
    /// `config["library_path"]`, RAW and un-expanded (may be `""`) —
    /// this module applies `~` expansion itself, matching
    /// `Path(library_value).expanduser()` (cloud_mixin.py:932-933).
    pub library_dir: &'a str,
    /// The launcher config directory (`self._config_dir()`,
    /// cloud_mixin.py:934) — always given, never expanded further.
    pub config_dir: &'a Path,
    /// The Shell-resolved Windows Documents folder
    /// (`pcsx2_windows_documents_folder()`, cloud_mixin.py:935) — `None`
    /// off Windows, in which case `%DOCUMENTS%` falls back to plain
    /// `%USERPROFILE%\Documents` env expansion (cloud_mixin.py:936).
    pub windows_documents: Option<&'a Path>,
    /// The RetroArch AppImage's portable home
    /// (`autoconfig::paths::retroarch_portable_home`,
    /// `<AppImage>.home/.config/retroarch`) when the entry is a RetroArch
    /// AppImage — `None` for every other entry and for a normal RetroArch
    /// install. The AppImage runtime sets `$HOME` to `<AppImage>.home`, so
    /// RetroArch's own `~`, `default` and `:/` notations mean paths under
    /// there, not under the real user home; rewrite deviation, see
    /// `docs/porting/06-cloud-saves.md`.
    pub retroarch_portable_home: Option<&'a Path>,
}

/// `$VAR` / `${VAR}` — the POSIX subset of `os.path.expandvars` this
/// module needs, matching `autoconfig::readers`'s private copy of the same
/// pattern (duplicated here rather than exported across modules, following
/// this crate's existing precedent of small per-module copies of this
/// exact helper).
static ENV_VAR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\$\{([A-Za-z_][A-Za-z0-9_]*)\}|\$([A-Za-z_][A-Za-z0-9_]*)").unwrap()
});

fn expand_vars(text: &str) -> String {
    ENV_VAR_RE
        .replace_all(text, |caps: &regex::Captures| {
            let name = caps.get(1).or_else(|| caps.get(2)).unwrap().as_str();
            std::env::var(name).unwrap_or_else(|_| caps[0].to_string())
        })
        .into_owned()
}

/// `profiles.py:133`'s `split_configured_paths`: split on runs of `;`,
/// `\r`, `\n`; trim each piece; drop blanks. Matches
/// `cloud::candidates`'s private copy of the same helper
/// (`_split_configured_paths` at the Python call site).
fn split_entry_list(value: &str) -> Vec<String> {
    value
        .split([';', '\r', '\n'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}

/// The `[item.strip() for item in raw_profile_paths if isinstance(item,
/// str) and item.strip()]` filter cloud_mixin.py applies to every list
/// field it reads off a profile, EVEN THOUGH `launch::profiles::normalize_one`
/// already trims and drops blanks at catalog-load time — kept here for
/// exact parity with the Python re-filter, and as a defensive no-op if a
/// future caller ever hands in an unnormalized profile.
fn trimmed_nonblank(items: &[String]) -> Vec<String> {
    items
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// `raw_path not in merged_paths` dedupe over `[*first, *second]`, dropping
/// blanks — the exact list-building idiom repeated at every one of
/// cloud_mixin.py's 13 per-emulator override blocks (e.g. :656-662).
fn merge_dedup(first: Vec<String>, second: Vec<String>) -> Vec<String> {
    let mut merged: Vec<String> = Vec::new();
    for raw in first.into_iter().chain(second) {
        if raw.trim().is_empty() {
            continue;
        }
        if !merged.contains(&raw) {
            merged.push(raw);
        }
    }
    merged
}

/// The common shape of every override block EXCEPT RetroArch's
/// (Azahar/Dolphin/PCSX2/RPCS3/Vita3k/Cemu/PICO-8/FBNeo/MAME/Eden/Xenia/
/// Redream/xemu): `if overrides: all_paths = dedupe([*overrides,
/// *all_paths])`, a no-op when the reader found nothing.
fn apply_override(all_paths: Vec<String>, overrides: Vec<PathBuf>) -> Vec<String> {
    if overrides.is_empty() {
        return all_paths;
    }
    let override_strings: Vec<String> = overrides
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();
    merge_dedup(override_strings, all_paths)
}

/// A 0-or-1-element profiles slice built from an already-matched
/// `Option<&EmulatorProfile>` — see this module's doc comment for why this
/// substitution for the full profiles list is sound here.
fn profile_slice(profile: Option<&EmulatorProfile>) -> &[EmulatorProfile] {
    match profile {
        Some(p) => std::slice::from_ref(p),
        None => &[],
    }
}

/// `_emulator_matches_tokens(name, "vita3k", emulator=emulator)`
/// (cloud_mixin.py, Vita3k block) — no dedicated `_is_vita3k_emulator_name`
/// exists in Python; the inline call is ported as-is rather than adding a
/// one-off predicate to `autoconfig`.
fn is_vita3k(entry: &EmulatorEntry, profiles: &[EmulatorProfile]) -> bool {
    autoconfig::emulator_matches_tokens(entry, &["vita3k"], profiles)
}

/// `_is_pico8_emulator_name` (cloud_mixin.py:1377): tokens `"pico8"` AND
/// `"pico-8"` — NOT redundant with each other (unlike xemu's two tokens),
/// so both are needed; `autoconfig::mod` has no dedicated `is_pico8`.
fn is_pico8(entry: &EmulatorEntry, profiles: &[EmulatorProfile]) -> bool {
    autoconfig::emulator_matches_tokens(entry, &["pico8", "pico-8"], profiles)
}

/// `_is_fbneo_emulator_name` (cloud_mixin.py:1383): tokens `"fbneo"` AND
/// `"final burn"`.
fn is_fbneo(entry: &EmulatorEntry, profiles: &[EmulatorProfile]) -> bool {
    autoconfig::emulator_matches_tokens(entry, &["fbneo", "final burn"], profiles)
}

/// `_is_mame_emulator_name` (cloud_mixin.py:1386).
fn is_mame(entry: &EmulatorEntry, profiles: &[EmulatorProfile]) -> bool {
    autoconfig::emulator_matches_tokens(entry, &["mame"], profiles)
}

/// `_is_xenia_emulator_name` (cloud_mixin.py:1392).
fn is_xenia(entry: &EmulatorEntry, profiles: &[EmulatorProfile]) -> bool {
    autoconfig::emulator_matches_tokens(entry, &["xenia"], profiles)
}

/// `%DOCUMENTS%`'s replacement value (cloud_mixin.py:935-936):
/// Shell-resolved Windows Documents when available, else
/// `os.path.join(os.environ.get("USERPROFILE", ""), "Documents")`.
fn documents_dir(ctx: &ResolveContext) -> String {
    if let Some(win_docs) = ctx.windows_documents {
        win_docs.to_string_lossy().to_string()
    } else {
        let userprofile = std::env::var("USERPROFILE").unwrap_or_default();
        Path::new(&userprofile)
            .join("Documents")
            .to_string_lossy()
            .to_string()
    }
}

/// Steps 4 of the module doc comment's pipeline: `os.path.expandvars`
/// then the GRID-defined tokens, in Python's dict-literal insertion order
/// (`%EMULATOR_DIR%`, `%LIBRARY_DIR%`, `%CONFIG_DIR%`, `%DOCUMENTS%`
/// last — cloud_mixin.py:941-946). `include_documents=false` for
/// [`resolved_screenshot_directories`], which uses the same table minus
/// `%DOCUMENTS%` (cloud_mixin.py:983-987 has no `documents_str` at all).
///
/// A blank `%EMULATOR_DIR%%LIBRARY_DIR%` source becomes the string `"."`,
/// not `""` — `str(Path())` in Python is `"."`, not empty, since
/// `emulator_dir`/`library_path` fall back to a bare `Path()` rather than
/// `None` (cloud_mixin.py:930,933).
fn expand_tokens(raw: &str, ctx: &ResolveContext, include_documents: bool) -> String {
    let mut expanded = expand_vars(raw);

    let emulator_dir_str = ctx
        .emulator_dir
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| ".".to_string());
    let library_dir_str = if ctx.library_dir.trim().is_empty() {
        ".".to_string()
    } else {
        paths::expand_user(ctx.library_dir)
            .to_string_lossy()
            .to_string()
    };
    let config_dir_str = ctx.config_dir.to_string_lossy().to_string();

    expanded = expanded.replace("%EMULATOR_DIR%", &emulator_dir_str);
    expanded = expanded.replace("%LIBRARY_DIR%", &library_dir_str);
    expanded = expanded.replace("%CONFIG_DIR%", &config_dir_str);
    if include_documents {
        expanded = expanded.replace("%DOCUMENTS%", &documents_dir(ctx));
    }
    expanded
}

/// The generic (non-RetroArch-notation) candidate build shared by
/// [`expand_sync_path`]'s fallthrough and [`resolved_screenshot_directories`]
/// (cloud_mixin.py:959-964): `~`-expand, then resolve relative paths
/// against `ctx.emulator_dir` and absolute paths as-is, both through
/// `Path.resolve(strict=False)` semantics.
fn generic_candidate(expanded: &str, ctx: &ResolveContext) -> PathBuf {
    let candidate_base = paths::expand_user(expanded);
    if candidate_base.is_absolute() {
        paths::resolve_best_effort(&candidate_base)
    } else {
        let base = ctx.emulator_dir.map(Path::to_path_buf).unwrap_or_default();
        paths::resolve_best_effort(&base.join(&candidate_base))
    }
}

/// What the AppImage runtime sets `$HOME` to for a portable RetroArch:
/// the `.home` directory two levels above the portable
/// `<...>.home/.config/retroarch` config home.
fn portable_home_root(portable_home: &Path) -> Option<&Path> {
    portable_home.parent()?.parent()
}

/// Rewrites a leading `~`/`~/` in a RetroArch config path against the
/// AppImage's portable home instead of the real user home — rewrite
/// deviation (see [`ResolveContext::retroarch_portable_home`]). Paths that
/// do not start with `~` are returned unchanged.
fn rewrite_tilde(raw: &str, portable_root: &Path) -> String {
    if raw == "~" {
        return portable_root.to_string_lossy().to_string();
    }
    match raw.strip_prefix("~/") {
        Some(rest) => portable_root.join(rest).to_string_lossy().to_string(),
        None => raw.to_string(),
    }
}

/// Expands one raw sync-path entry against `ctx` and keeps it only if it
/// exists as a directory or a file (cloud_mixin.py:939-967, one loop
/// iteration). `key_is_save` picks `"saves"`/`"states"` for the RetroArch
/// `default` sentinel; `is_retroarch` gates the two RetroArch-only
/// notations — see this module's doc comment for why that parameter exists
/// beyond the brief's pinned 3-argument signature.
pub fn expand_sync_path(
    raw: &str,
    key_is_save: bool,
    is_retroarch: bool,
    ctx: &ResolveContext,
) -> Option<PathBuf> {
    let expanded = expand_tokens(raw, ctx, true);
    let stripped = expanded.trim();
    let base = ctx.emulator_dir.map(Path::to_path_buf).unwrap_or_default();
    // RetroArch's own notations are relative to what RetroArch sees as its
    // root: the AppImage portable home when there is one, else the
    // emulator directory (Python only ever knew the latter).
    let retroarch_base = ctx
        .retroarch_portable_home
        .map(Path::to_path_buf)
        .unwrap_or_else(|| base.clone());

    let candidate = if is_retroarch && stripped.to_lowercase() == "default" {
        paths::resolve_best_effort(&retroarch_base.join(if key_is_save {
            "saves"
        } else {
            "states"
        }))
    } else if is_retroarch && (stripped.starts_with(":\\") || stripped.starts_with(":/")) {
        paths::resolve_best_effort(&retroarch_base.join(&stripped[2..]))
    } else {
        generic_candidate(&expanded, ctx)
    };

    (candidate.is_dir() || candidate.is_file()).then_some(candidate)
}

/// `_resolved_sync_directory_paths` (cloud_mixin.py:618-981), minus the
/// memoization and `_ensure_emulator_sync_settings` call — see this
/// module's doc comment. Returns the deduped resolved paths, and (as a
/// convenience over Python, which computes this separately at each call
/// site, e.g. cloud_mixin.py:520) the subset of those that are explicit
/// files rather than directories.
pub fn resolved_sync_directory_paths(
    entry: &EmulatorEntry,
    profile: Option<&EmulatorProfile>,
    key: PathKey,
    ctx: &ResolveContext,
) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let key_is_save = key.is_save();
    let configured_paths = split_entry_list(key.entry_field(entry));
    let matched_profiles = profile_slice(profile);
    let is_retroarch_flag = autoconfig::is_retroarch(entry, matched_profiles);
    let portable_home = is_retroarch_flag
        .then(|| paths::retroarch_portable_home(&paths::expand_user(&entry.path)))
        .flatten();

    let mut all_paths: Vec<String> = if !configured_paths.is_empty() {
        configured_paths.clone()
    } else {
        profile
            .map(|p| trimmed_nonblank(key.profile_field(p)))
            .unwrap_or_default()
    };

    if configured_paths.is_empty() {
        let args_vec = template::split_template(&entry.args).unwrap_or_default();

        if is_retroarch_flag {
            let settings = retroarch::directory_settings(&entry.path);
            let override_path = if key_is_save {
                settings.savefile_directory
            } else {
                settings.savestate_directory
            };
            let portable_root = portable_home.as_deref().and_then(portable_home_root);
            let override_path = match portable_root {
                Some(root) => rewrite_tilde(override_path.trim(), root),
                None => override_path.trim().to_string(),
            };
            if !override_path.is_empty() {
                let mut prefixed = vec![override_path];
                prefixed.extend(all_paths);
                all_paths = prefixed;
            }
            let mut fallback: Vec<String> = if key_is_save {
                vec!["saves".to_string(), "savefiles".to_string()]
            } else {
                vec!["states".to_string(), "savestates".to_string()]
            };
            // The portable home's own saves/states, for a cfg that never
            // wrote the key: the relative fallbacks above resolve against
            // the emulator directory, which an AppImage never uses.
            if let Some(home) = portable_home.as_deref() {
                fallback.push(
                    home.join(if key_is_save { "saves" } else { "states" })
                        .to_string_lossy()
                        .to_string(),
                );
            }
            all_paths = merge_dedup(all_paths, fallback);
        }

        if autoconfig::is_azahar(entry, matched_profiles) {
            let overrides = if key_is_save {
                readers::azahar_save_path_overrides(&entry.path, &args_vec)
            } else {
                readers::azahar_state_path_overrides(&entry.path, &args_vec)
            };
            all_paths = apply_override(all_paths, overrides);
        }

        if autoconfig::is_dolphin(entry, matched_profiles) {
            let overrides = if key_is_save {
                readers::dolphin_save_path_overrides(&entry.path, &args_vec)
            } else {
                readers::dolphin_state_path_overrides(&entry.path, &args_vec)
            };
            all_paths = apply_override(all_paths, overrides);
        }

        if autoconfig::is_pcsx2(entry, matched_profiles) {
            let overrides = if key_is_save {
                readers::pcsx2_save_path_overrides(&entry.path, &args_vec)
            } else {
                readers::pcsx2_state_path_overrides(&entry.path, &args_vec)
            };
            all_paths = apply_override(all_paths, overrides);
        }

        if key_is_save && autoconfig::is_rpcs3(entry, matched_profiles) {
            let overrides = readers::rpcs3_save_path_overrides(&entry.path, &args_vec);
            all_paths = apply_override(all_paths, overrides);
        }

        if key_is_save && is_vita3k(entry, matched_profiles) {
            let overrides = readers::vita3k_save_path_overrides(&entry.path, &args_vec);
            all_paths = apply_override(all_paths, overrides);
        }

        if key_is_save && autoconfig::is_cemu(entry, matched_profiles) {
            let overrides = readers::cemu_save_path_overrides(&entry.path, &args_vec);
            all_paths = apply_override(all_paths, overrides);
        }

        if key_is_save && is_pico8(entry, matched_profiles) {
            let overrides = readers::pico8_save_path_overrides(&entry.path, &args_vec);
            all_paths = apply_override(all_paths, overrides);
        }

        if is_fbneo(entry, matched_profiles) {
            let overrides = if key_is_save {
                readers::fbneo_save_path_overrides(&entry.path, &args_vec)
            } else {
                readers::fbneo_state_path_overrides(&entry.path, &args_vec)
            };
            all_paths = apply_override(all_paths, overrides);
        }

        if is_mame(entry, matched_profiles) {
            let overrides = if key_is_save {
                readers::mame_save_path_overrides(&entry.path, &args_vec)
            } else {
                readers::mame_state_path_overrides(&entry.path, &args_vec)
            };
            all_paths = apply_override(all_paths, overrides);
        }

        if key_is_save && autoconfig::is_eden(entry, matched_profiles) {
            let overrides = readers::eden_save_path_overrides(&entry.path, &args_vec);
            all_paths = apply_override(all_paths, overrides);
        }

        if is_xenia(entry, matched_profiles) {
            let overrides = if key_is_save {
                readers::xenia_save_path_overrides(&entry.path, &args_vec)
            } else {
                readers::xenia_state_path_overrides(&entry.path, &args_vec)
            };
            all_paths = apply_override(all_paths, overrides);
        }

        if autoconfig::is_redream(entry, matched_profiles) {
            let overrides = if key_is_save {
                readers::redream_save_path_overrides(&entry.path, &args_vec)
            } else {
                readers::redream_state_path_overrides(&entry.path, &args_vec)
            };
            all_paths = apply_override(all_paths, overrides);
        }

        if key_is_save && autoconfig::is_xemu(entry, matched_profiles) {
            let overrides = readers::xemu_save_path_overrides(&entry.path, &args_vec);
            all_paths = apply_override(all_paths, overrides);
        }
    }

    if all_paths.is_empty() {
        return (Vec::new(), Vec::new());
    }

    let expand_ctx = ResolveContext {
        emulator_dir: ctx.emulator_dir,
        library_dir: ctx.library_dir,
        config_dir: ctx.config_dir,
        windows_documents: ctx.windows_documents,
        retroarch_portable_home: portable_home.as_deref().or(ctx.retroarch_portable_home),
    };
    let mut resolved: Vec<PathBuf> = Vec::new();
    for raw in &all_paths {
        if let Some(candidate) = expand_sync_path(raw, key_is_save, is_retroarch_flag, &expand_ctx)
        {
            resolved.push(candidate);
        }
    }

    let deduped = paths::dedupe_casefold(resolved);
    let explicit_file_roots: Vec<PathBuf> =
        deduped.iter().filter(|p| p.is_file()).cloned().collect();
    (deduped, explicit_file_roots)
}

/// `_resolved_screenshot_directories` (cloud_mixin.py:983-1027):
/// profile-only (no per-entry override), directories only, no
/// `%DOCUMENTS%` token.
pub fn resolved_screenshot_directories(
    _entry: &EmulatorEntry,
    profile: Option<&EmulatorProfile>,
    ctx: &ResolveContext,
) -> Vec<PathBuf> {
    let profile_paths: Vec<String> = profile
        .map(|p| trimmed_nonblank(&p.screenshot_directories))
        .unwrap_or_default();

    if profile_paths.is_empty() {
        return Vec::new();
    }

    let mut resolved: Vec<PathBuf> = Vec::new();
    for raw in &profile_paths {
        let expanded = expand_tokens(raw, ctx, false);
        let candidate = generic_candidate(&expanded, ctx);
        if candidate.is_dir() {
            resolved.push(candidate);
        }
    }
    paths::dedupe_casefold(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_env::EnvGuard;

    fn entry(name: &str, path: &str) -> EmulatorEntry {
        EmulatorEntry {
            name: name.to_string(),
            path: path.to_string(),
            ..Default::default()
        }
    }

    fn ctx<'a>(emulator_dir: Option<&'a Path>, config_dir: &'a Path) -> ResolveContext<'a> {
        ResolveContext {
            emulator_dir,
            library_dir: "",
            config_dir,
            windows_documents: None,
            retroarch_portable_home: None,
        }
    }

    fn isolated_env(dir: &Path) -> EnvGuard {
        let dir_str = dir.to_str().unwrap();
        EnvGuard::set(&[
            ("HOME", Some(dir_str)),
            ("XDG_CONFIG_HOME", None),
            ("XDG_DATA_HOME", None),
            ("USERPROFILE", None),
        ])
    }

    #[test]
    fn entry_paths_win_and_skip_all_probing() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());

        let emulator_dir = temp.path().join("RetroArch");
        std::fs::create_dir_all(&emulator_dir).unwrap();
        let exe = emulator_dir.join("retroarch.exe");
        std::fs::write(&exe, b"").unwrap();

        // A poisoned retroarch.cfg that WOULD add a save directory if the
        // retroarch reader were ever consulted.
        let poisoned = emulator_dir.join("poisoned_saves");
        std::fs::create_dir_all(&poisoned).unwrap();
        std::fs::write(
            emulator_dir.join("retroarch.cfg"),
            format!(
                "savefile_directory = \"{}\"\n",
                poisoned.to_string_lossy().replace('\\', "\\\\")
            ),
        )
        .unwrap();

        let configured = temp.path().join("configured_saves");
        std::fs::create_dir_all(&configured).unwrap();

        let mut e = entry("RetroArch", &exe.to_string_lossy());
        e.save_paths = configured.to_string_lossy().to_string();

        let c = ctx(Some(&emulator_dir), temp.path());
        let (resolved, _files) = resolved_sync_directory_paths(&e, None, PathKey::SavePaths, &c);

        assert_eq!(resolved, vec![paths::resolve_best_effort(&configured)]);
    }

    #[test]
    fn retroarch_prepends_config_dir_and_appends_literal_fallbacks() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());

        let emulator_dir = temp.path().join("RetroArch");
        std::fs::create_dir_all(&emulator_dir).unwrap();
        let exe = emulator_dir.join("retroarch.exe");
        std::fs::write(&exe, b"").unwrap();

        let configured_savefile_dir = emulator_dir.join("mysaves");
        std::fs::create_dir_all(&configured_savefile_dir).unwrap();
        std::fs::write(
            emulator_dir.join("retroarch.cfg"),
            format!(
                "savefile_directory = \"{}\"\n",
                configured_savefile_dir.to_string_lossy()
            ),
        )
        .unwrap();

        // The literal fallback "saves" also exists, so it should appear
        // too, AFTER the configured directory.
        let saves_fallback = emulator_dir.join("saves");
        std::fs::create_dir_all(&saves_fallback).unwrap();

        let e = entry("RetroArch", &exe.to_string_lossy());
        let c = ctx(Some(&emulator_dir), temp.path());
        let (resolved, _files) = resolved_sync_directory_paths(&e, None, PathKey::SavePaths, &c);

        assert_eq!(
            resolved,
            vec![
                paths::resolve_best_effort(&configured_savefile_dir),
                paths::resolve_best_effort(&saves_fallback),
            ]
        );
    }

    #[test]
    fn retroarch_default_sentinel_and_colon_slash_notation() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());

        let emulator_dir = temp.path().join("RetroArch");
        std::fs::create_dir_all(&emulator_dir).unwrap();
        let saves_dir = emulator_dir.join("saves");
        std::fs::create_dir_all(&saves_dir).unwrap();

        let c = ctx(Some(&emulator_dir), temp.path());

        let via_default = expand_sync_path("default", true, true, &c).unwrap();
        assert_eq!(via_default, paths::resolve_best_effort(&saves_dir));

        let via_colon = expand_sync_path(":/saves", true, true, &c).unwrap();
        assert_eq!(via_colon, paths::resolve_best_effort(&saves_dir));

        // Same raw values are NOT special-cased for a non-RetroArch entry.
        assert_eq!(expand_sync_path("default", true, false, &c), None);
    }

    #[test]
    fn tokens_expand_emulator_library_config_dirs() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());

        let emulator_dir = temp.path().join("Emu");
        let library_dir = temp.path().join("Library");
        let config_dir = temp.path().join("Config");
        let target = temp.path().join("target_dir");
        std::fs::create_dir_all(&emulator_dir).unwrap();
        std::fs::create_dir_all(&library_dir).unwrap();
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::create_dir_all(&target).unwrap();

        let library_dir_str = library_dir.to_string_lossy().to_string();
        let c = ResolveContext {
            emulator_dir: Some(&emulator_dir),
            library_dir: &library_dir_str,
            config_dir: &config_dir,
            windows_documents: None,
            retroarch_portable_home: None,
        };

        let via_emulator =
            expand_sync_path("%EMULATOR_DIR%/../target_dir", true, false, &c).unwrap();
        assert_eq!(via_emulator, paths::resolve_best_effort(&target));

        let via_library = expand_sync_path("%LIBRARY_DIR%", true, false, &c).unwrap();
        assert_eq!(via_library, paths::resolve_best_effort(&library_dir));

        let via_config = expand_sync_path("%CONFIG_DIR%", true, false, &c).unwrap();
        assert_eq!(via_config, paths::resolve_best_effort(&config_dir));
    }

    #[test]
    fn relative_paths_resolve_against_the_emulator_dir() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());

        let emulator_dir = temp.path().join("Emu");
        let sub = emulator_dir.join("saves").join("slot1");
        std::fs::create_dir_all(&sub).unwrap();
        let config_dir = temp.path().join("Config");
        std::fs::create_dir_all(&config_dir).unwrap();

        let c = ctx(Some(&emulator_dir), &config_dir);
        let resolved = expand_sync_path("saves/slot1", true, false, &c).unwrap();
        assert_eq!(resolved, paths::resolve_best_effort(&sub));
    }

    #[test]
    fn existing_files_are_kept_as_explicit_file_roots() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());

        let emulator_dir = temp.path().join("Emu");
        std::fs::create_dir_all(&emulator_dir).unwrap();
        let save_file = emulator_dir.join("card1.mcr");
        std::fs::write(&save_file, b"x").unwrap();

        let mut e = entry("Some Emu", "");
        e.save_paths = save_file.to_string_lossy().to_string();
        let c = ctx(Some(&emulator_dir), temp.path());

        let (resolved, explicit_file_roots) =
            resolved_sync_directory_paths(&e, None, PathKey::SavePaths, &c);

        assert_eq!(resolved, vec![paths::resolve_best_effort(&save_file)]);
        assert_eq!(
            explicit_file_roots,
            vec![paths::resolve_best_effort(&save_file)]
        );
    }

    #[test]
    fn results_dedupe_case_insensitively() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());

        let emulator_dir = temp.path().join("Emu");
        std::fs::create_dir_all(&emulator_dir).unwrap();
        let saves = emulator_dir.join("Saves");
        std::fs::create_dir_all(&saves).unwrap();

        let mut e = entry("Some Emu", "");
        e.save_paths = format!(
            "{};{}",
            saves.to_string_lossy(),
            saves.to_string_lossy().to_uppercase()
        );
        let c = ctx(Some(&emulator_dir), temp.path());

        let (resolved, _files) = resolved_sync_directory_paths(&e, None, PathKey::SavePaths, &c);

        assert_eq!(resolved.len(), 1);
    }

    #[test]
    fn screenshot_dirs_are_profile_only_and_must_be_directories() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());

        let emulator_dir = temp.path().join("Emu");
        std::fs::create_dir_all(&emulator_dir).unwrap();
        let shots_dir = emulator_dir.join("shots");
        std::fs::create_dir_all(&shots_dir).unwrap();
        let shots_file = emulator_dir.join("shots_file");
        std::fs::write(&shots_file, b"x").unwrap();

        let profile = EmulatorProfile {
            screenshot_directories: vec![
                "shots".to_string(),
                "shots_file".to_string(),
                "%DOCUMENTS%".to_string(),
            ],
            ..Default::default()
        };

        let e = entry("Some Emu", "");
        let c = ctx(Some(&emulator_dir), temp.path());

        let resolved = resolved_screenshot_directories(&e, Some(&profile), &c);

        // "shots_file" is a file, not a directory, and is dropped.
        // "%DOCUMENTS%" is not expanded (no %DOCUMENTS% token here), so it
        // resolves to a nonexistent "<emulator_dir>/%DOCUMENTS%" and is
        // dropped too.
        assert_eq!(resolved, vec![paths::resolve_best_effort(&shots_dir)]);

        // No profile at all -> no screenshot directories, regardless of
        // the entry.
        assert_eq!(
            resolved_screenshot_directories(&e, None, &c),
            Vec::<PathBuf>::new()
        );
    }

    #[test]
    fn pcsx2_override_wiring_lands_ahead_of_profile_paths() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());

        let emulator_dir = temp.path().join("PCSX2");
        std::fs::create_dir_all(&emulator_dir).unwrap();
        let exe = emulator_dir.join("pcsx2-qt.exe");
        std::fs::write(&exe, b"").unwrap();
        // Portable marker: makes the emulator dir itself PCSX2's data
        // root, so its default memory-card directory is predictable.
        std::fs::write(emulator_dir.join("portable.ini"), "").unwrap();
        let memcards = emulator_dir.join("memcards");
        std::fs::create_dir_all(&memcards).unwrap();

        let profile_only_dir = temp.path().join("profile_only");
        std::fs::create_dir_all(&profile_only_dir).unwrap();

        let profile = EmulatorProfile {
            match_tokens: vec!["pcsx2".to_string()],
            save_directories: vec![profile_only_dir.to_string_lossy().to_string()],
            ..Default::default()
        };

        let e = entry("PCSX2", &exe.to_string_lossy());
        let c = ctx(Some(&emulator_dir), temp.path());

        let (resolved, _files) =
            resolved_sync_directory_paths(&e, Some(&profile), PathKey::SavePaths, &c);

        // With no PCSX2.ini, the reader's default memory-card directory is
        // `<data_root>/memcards`; it must land AHEAD of the profile path.
        assert!(!resolved.is_empty());
        assert_eq!(resolved[0], paths::resolve_best_effort(&memcards));
        assert!(resolved.contains(&paths::resolve_best_effort(&profile_only_dir)));
    }

    #[test]
    fn dolphin_override_wiring_lands_ahead_of_profile_paths() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());

        let emulator_dir = temp.path().join("Dolphin");
        std::fs::create_dir_all(&emulator_dir).unwrap();
        let exe = emulator_dir.join("Dolphin.exe");
        std::fs::write(&exe, b"").unwrap();
        // Portable mode: a "portable.txt" marker makes Dolphin's user
        // root `<emulator_dir>/User`, so its GC save root is predictable
        // even with no Dolphin.ini on disk (dolphin_user_root_candidates,
        // readers.rs:671-698).
        std::fs::write(emulator_dir.join("portable.txt"), b"").unwrap();
        let gc_dir = emulator_dir
            .join("User")
            .join("GC")
            .join("USA")
            .join("Card A");
        std::fs::create_dir_all(&gc_dir).unwrap();

        let profile_only_dir = temp.path().join("profile_only");
        std::fs::create_dir_all(&profile_only_dir).unwrap();

        let profile = EmulatorProfile {
            match_tokens: vec!["dolphin".to_string()],
            save_directories: vec![profile_only_dir.to_string_lossy().to_string()],
            ..Default::default()
        };

        let e = entry("Dolphin", &exe.to_string_lossy());
        let c = ctx(Some(&emulator_dir), temp.path());

        let (resolved, _files) =
            resolved_sync_directory_paths(&e, Some(&profile), PathKey::SavePaths, &c);

        assert!(!resolved.is_empty());
        assert!(resolved.contains(&paths::resolve_best_effort(&profile_only_dir)));
        let override_position = resolved
            .iter()
            .position(|p| p.starts_with(paths::resolve_best_effort(&emulator_dir)))
            .expect("the Dolphin override's Card A folder must be present");
        let profile_position = resolved
            .iter()
            .position(|p| *p == paths::resolve_best_effort(&profile_only_dir))
            .expect("the profile path must be present");
        assert!(
            override_position < profile_position,
            "override at {override_position}, profile at {profile_position}: {resolved:?}"
        );
    }

    #[test]
    fn xemu_save_override_wiring_lands_ahead_of_profile_paths() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());

        let emulator_dir = temp.path().join("xemu");
        std::fs::create_dir_all(&emulator_dir).unwrap();
        let exe = emulator_dir.join("xemu.exe");
        std::fs::write(&exe, b"").unwrap();
        // A marker file makes the emulator dir itself xemu's base path
        // (xemu_base_path_candidates, readers.rs:2304-2335); with no
        // xemu.toml to override it, `hdd_path` defaults to exactly this
        // file, so it is both the marker and the override candidate.
        let hdd_path = emulator_dir.join("xbox_hdd.qcow2");
        std::fs::write(&hdd_path, b"").unwrap();

        let profile_only_dir = temp.path().join("profile_only");
        std::fs::create_dir_all(&profile_only_dir).unwrap();

        let profile = EmulatorProfile {
            match_tokens: vec!["xemu".to_string()],
            save_directories: vec![profile_only_dir.to_string_lossy().to_string()],
            ..Default::default()
        };

        let e = entry("xemu", &exe.to_string_lossy());
        let c = ctx(Some(&emulator_dir), temp.path());

        let (resolved, _files) =
            resolved_sync_directory_paths(&e, Some(&profile), PathKey::SavePaths, &c);

        assert_eq!(
            resolved,
            vec![
                paths::resolve_best_effort(&hdd_path),
                paths::resolve_best_effort(&profile_only_dir),
            ]
        );
    }

    /// A RetroArch AppImage laid out the way the AppImage runtime expects:
    /// `<name>.AppImage` next to `<name>.AppImage.home/.config/retroarch`,
    /// which becomes `$HOME` for the emulator at runtime. Returns
    /// `(executable, portable home)`.
    fn retroarch_appimage(root: &Path) -> (PathBuf, PathBuf) {
        let exe = root.join("RetroArch.AppImage");
        std::fs::write(&exe, b"").unwrap();
        let portable_home = root
            .join("RetroArch.AppImage.home")
            .join(".config")
            .join("retroarch");
        std::fs::create_dir_all(&portable_home).unwrap();
        (exe, portable_home)
    }

    #[test]
    fn retroarch_appimage_tilde_overrides_resolve_against_the_portable_home() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());

        let (exe, portable_home) = retroarch_appimage(temp.path());
        std::fs::write(
            portable_home.join("retroarch.cfg"),
            "savestate_directory = \"~/.config/retroarch/states\"\n\
             savefile_directory = \"~/.config/retroarch/saves\"\n",
        )
        .unwrap();
        // `sort_savestates_enable` puts files in a per-core subfolder; the
        // candidate walk is recursive (cloud::candidates' `walk_files`), so
        // only the parent directory has to resolve.
        let states = portable_home.join("states");
        let saves = portable_home.join("saves");
        std::fs::create_dir_all(states.join("bsnes")).unwrap();
        std::fs::create_dir_all(saves.join("bsnes")).unwrap();

        let e = entry("RetroArch", &exe.to_string_lossy());
        let c = ctx(Some(temp.path()), temp.path());

        let (resolved_states, _) = resolved_sync_directory_paths(&e, None, PathKey::StatePaths, &c);
        assert_eq!(resolved_states, vec![paths::resolve_best_effort(&states)]);

        let (resolved_saves, _) = resolved_sync_directory_paths(&e, None, PathKey::SavePaths, &c);
        assert_eq!(resolved_saves, vec![paths::resolve_best_effort(&saves)]);
    }

    #[test]
    fn retroarch_appimage_portable_fallbacks_apply_without_cfg_keys() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());

        let (exe, portable_home) = retroarch_appimage(temp.path());
        // A cfg with no savefile/savestate keys at all.
        std::fs::write(
            portable_home.join("retroarch.cfg"),
            "sort_savestates_enable = \"true\"\n",
        )
        .unwrap();
        let states = portable_home.join("states");
        let saves = portable_home.join("saves");
        std::fs::create_dir_all(states.join("bsnes")).unwrap();
        std::fs::create_dir_all(saves.join("bsnes")).unwrap();

        let e = entry("RetroArch", &exe.to_string_lossy());
        let c = ctx(Some(temp.path()), temp.path());

        let (resolved_states, _) = resolved_sync_directory_paths(&e, None, PathKey::StatePaths, &c);
        assert_eq!(resolved_states, vec![paths::resolve_best_effort(&states)]);

        let (resolved_saves, _) = resolved_sync_directory_paths(&e, None, PathKey::SavePaths, &c);
        assert_eq!(resolved_saves, vec![paths::resolve_best_effort(&saves)]);
    }

    #[test]
    fn retroarch_appimage_default_and_colon_notation_use_the_portable_home() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());

        let (_exe, portable_home) = retroarch_appimage(temp.path());
        let saves = portable_home.join("saves");
        std::fs::create_dir_all(&saves).unwrap();

        let c = ResolveContext {
            emulator_dir: Some(temp.path()),
            library_dir: "",
            config_dir: temp.path(),
            windows_documents: None,
            retroarch_portable_home: Some(&portable_home),
        };

        assert_eq!(
            expand_sync_path("default", true, true, &c),
            Some(paths::resolve_best_effort(&saves))
        );
        assert_eq!(
            expand_sync_path(":/saves", true, true, &c),
            Some(paths::resolve_best_effort(&saves))
        );
    }

    #[test]
    fn non_appimage_retroarch_still_expands_tilde_against_the_user_home() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        std::fs::create_dir_all(&home).unwrap();
        let _guard = isolated_env(&home);

        let emulator_dir = temp.path().join("RetroArch");
        std::fs::create_dir_all(&emulator_dir).unwrap();
        let exe = emulator_dir.join("retroarch.exe");
        std::fs::write(&exe, b"").unwrap();
        std::fs::write(
            emulator_dir.join("retroarch.cfg"),
            "savefile_directory = \"~/.config/retroarch/saves\"\n",
        )
        .unwrap();
        let user_saves = home.join(".config").join("retroarch").join("saves");
        std::fs::create_dir_all(&user_saves).unwrap();

        let e = entry("RetroArch", &exe.to_string_lossy());
        let c = ctx(Some(&emulator_dir), temp.path());

        let (resolved, _) = resolved_sync_directory_paths(&e, None, PathKey::SavePaths, &c);
        assert_eq!(resolved, vec![paths::resolve_best_effort(&user_saves)]);
    }
}
