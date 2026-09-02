//! Path helpers shared by every `ensure_*` writer: the home/XDG roots the
//! config candidate lists are built from, `~` expansion, case-insensitive
//! candidate deduplication, and the "directory that holds the executable"
//! rule.
//!
//! Ports `grid_launcher/core/path.py:33-44` plus the `Path.expanduser()` and
//! `emulator_path if emulator_path.is_dir() else emulator_path.parent`
//! idioms repeated across `grid_launcher/emulator/*.py` (for example
//! `grid_launcher/emulator/cemu.py:340`).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// The user's home directory, or `None` when it cannot be determined.
///
/// `directories::UserDirs` first (which is `$HOME` on unix and the Windows
/// known-folder API on Windows), then a direct `$HOME` read as a fallback so
/// a test can point the whole helper family at a temporary directory.
pub fn home_dir() -> Option<PathBuf> {
    if let Some(user_dirs) = directories::UserDirs::new() {
        return Some(user_dirs.home_dir().to_path_buf());
    }
    let raw = std::env::var("HOME").ok()?;
    let trimmed = raw.trim();
    (!trimmed.is_empty()).then(|| PathBuf::from(trimmed))
}

/// `$XDG_CONFIG_HOME`, else `~/.config` (`core/path.py:33`).
///
/// With no home directory at all, the fallback degrades to the relative
/// `.config` — Python would raise `RuntimeError` there; a candidate path
/// that simply never exists is the kinder failure for a candidate list.
pub fn xdg_config_home() -> PathBuf {
    env_dir("XDG_CONFIG_HOME").unwrap_or_else(|| {
        home_dir()
            .unwrap_or_else(|| PathBuf::from(""))
            .join(".config")
    })
}

/// `$XDG_DATA_HOME`, else `~/.local/share` (`core/path.py:40`).
pub fn xdg_data_home() -> PathBuf {
    env_dir("XDG_DATA_HOME").unwrap_or_else(|| {
        home_dir()
            .unwrap_or_else(|| PathBuf::from(""))
            .join(".local")
            .join("share")
    })
}

/// The directory named by environment variable `var`: `Some` only when the
/// variable is set and non-blank once trimmed, with a leading `~` expanded.
///
/// Python's call sites are a mix of `if value:` (`core/path.py:34`) and
/// `.strip()` (`core/path.py:54`); trimming everywhere is the stricter of
/// the two and only differs for a whitespace-only value, which can never
/// name a real directory.
pub fn env_dir(var: &str) -> Option<PathBuf> {
    let raw = std::env::var(var).ok()?;
    let trimmed = raw.trim();
    (!trimmed.is_empty()).then(|| expand_user(trimmed))
}

/// Expand a leading `~` — `~` alone or `~/rest` — to the user's home
/// directory, like `Path.expanduser()`.
///
/// `~user/...` is NOT expanded (Python resolves it through the password
/// database); with no home directory, the text is returned untouched, which
/// is also what `Path.expanduser()` does before it raises.
pub fn expand_user(text: &str) -> PathBuf {
    if text == "~" {
        if let Some(home) = home_dir() {
            return home;
        }
    } else if let Some(rest) = text.strip_prefix("~/") {
        if let Some(home) = home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(text)
}

/// Deduplicate a candidate list case-insensitively, keeping the first
/// occurrence of each path and the overall order.
///
/// The key is `to_string_lossy().to_lowercase()` — no `resolve()` — so this
/// only collapses paths that are literally the same text modulo case, the
/// way the emulator modules build their candidate lists.
pub fn dedupe_casefold(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen: HashSet<String> = HashSet::new();
    paths
        .into_iter()
        .filter(|path| seen.insert(path.to_string_lossy().to_lowercase()))
        .collect()
}

/// The directory an emulator's config lives beside: `path` itself when it is
/// an existing directory, else `path`'s parent.
///
/// Shared by duckstation/dolphin/azahar/eden/cemu/ppsspp/xemu/redream. A
/// path that does not exist counts as a file, so a not-yet-installed
/// emulator still resolves to its install directory. `None` when there is no
/// parent at all (a filesystem root, or an empty path).
pub fn emulator_dir(path: &Path) -> Option<PathBuf> {
    if path.is_dir() {
        return Some(path.to_path_buf());
    }
    path.parent().map(Path::to_path_buf)
}

/// Collapse `.`/`..` components lexically — no filesystem access — the way
/// `os.path.normpath` does: a `ParentDir` cancels the preceding `Normal`
/// component when there is one; at the root (or with no root yet collected),
/// a `ParentDir` is dropped rather than climbing above the root, matching
/// Python's own clamping (`Path("/../../foo").resolve()` is `/foo`, not an
/// error and not `/../foo`). `CurDir` is dropped outright. This never
/// touches disk, so it collapses `..` through path segments that do not
/// exist just as readily as ones that do — the property
/// [`resolve_best_effort`] needs it for.
fn lexically_normalize(path: &Path) -> PathBuf {
    use std::path::Component;

    let mut stack: Vec<Component> = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => match stack.last() {
                Some(Component::Normal(_)) => {
                    stack.pop();
                }
                Some(Component::RootDir) | Some(Component::Prefix(_)) => {
                    // Clamp at root: already there, so ".." is a no-op.
                }
                None | Some(Component::ParentDir) => {
                    // A relative path with no root collected yet (or one
                    // that already starts with unresolved ".." segments):
                    // keep the ".." rather than lose information — this
                    // never happens for the absolute paths
                    // `resolve_best_effort` normalizes, since those always
                    // collect a `RootDir`/`Prefix` first.
                    stack.push(component);
                }
                Some(Component::CurDir) => unreachable!("CurDir is never pushed onto the stack"),
            },
            other => stack.push(other),
        }
    }
    stack.into_iter().collect()
}

/// `Path.resolve(strict=False)` — Python's default, which succeeds even
/// when the path does not exist. Unlike `std::fs::canonicalize` (which
/// requires the full path to exist), this: makes a relative `path` absolute
/// against the current directory; lexically collapses `.`/`..` components
/// via [`lexically_normalize`] — through path segments that do not exist,
/// same as Python — clamping at the root rather than climbing above it;
/// then resolves symlinks for whatever longest ancestor of the normalized
/// path actually exists, rejoining the (already-normalized, so no further
/// `..` to collapse) non-existent remainder untouched.
///
/// This is the one shared home for what used to be duplicated,
/// trailing-slash-preserving, `..`-blind copies in `readers.rs` and
/// `rpcs3.rs` — every `ensure_*`/`*_directory_settings` call site in this
/// crate that ports a Python `.resolve()` call should go through this
/// function instead of hand-rolling another one.
pub(crate) fn resolve_best_effort(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    };

    let normalized = lexically_normalize(&absolute);

    if let Ok(canonical) = std::fs::canonicalize(&normalized) {
        return canonical;
    }

    let mut existing = normalized.clone();
    let mut remainder: Vec<std::ffi::OsString> = Vec::new();
    while !existing.exists() {
        let Some(name) = existing.file_name().map(|n| n.to_os_string()) else {
            break;
        };
        remainder.push(name);
        if !existing.pop() {
            break;
        }
    }

    let mut resolved = std::fs::canonicalize(&existing).unwrap_or(existing);
    for part in remainder.into_iter().rev() {
        resolved.push(part);
    }
    resolved
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dedupe_casefold_keeps_first_occurrence() {
        let deduped = dedupe_casefold(vec![
            PathBuf::from("/Games/RetroArch"),
            PathBuf::from("/games/retroarch"),
            PathBuf::from("/games/Dolphin"),
            PathBuf::from("/Games/RetroArch"),
        ]);
        assert_eq!(
            deduped,
            vec![
                PathBuf::from("/Games/RetroArch"),
                PathBuf::from("/games/Dolphin"),
            ]
        );
    }

    #[test]
    fn emulator_dir_uses_parent_for_a_file_and_self_for_a_directory() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("RetroArch");
        std::fs::create_dir(&dir).unwrap();
        let file = dir.join("retroarch");
        std::fs::write(&file, b"").unwrap();

        assert_eq!(emulator_dir(&dir), Some(dir.clone()));
        assert_eq!(emulator_dir(&file), Some(dir.clone()));
        assert_eq!(
            emulator_dir(&dir.join("missing").join("emulator")),
            Some(dir.join("missing")),
            "a path that does not exist is treated as a file"
        );
    }

    #[test]
    fn expand_user_only_touches_a_leading_tilde() {
        // `home_dir()` reads `$HOME`, which races with any test elsewhere in
        // the crate that mutates it — see `crate::test_env`.
        let _lock = crate::test_env::lock();
        let home = home_dir().expect("this test needs a home directory");
        assert_eq!(expand_user("~"), home);
        assert_eq!(expand_user("~/.config/eden"), home.join(".config/eden"));
        assert_eq!(
            expand_user("/opt/~/games"),
            PathBuf::from("/opt/~/games"),
            "a tilde that is not leading is literal"
        );
        assert_eq!(
            expand_user("~root/games"),
            PathBuf::from("~root/games"),
            "another user's home is not resolved"
        );
    }

    #[test]
    fn env_dir_rejects_unset_and_blank_values() {
        // `env_dir()` reads a process env var; see `crate::test_env`.
        let _lock = crate::test_env::lock();
        assert_eq!(env_dir("GRID_AUTOCONFIG_TEST_UNSET_VAR"), None);
    }

    #[test]
    fn resolve_best_effort_collapses_parent_dir_through_a_nonexistent_directory() {
        let temp = tempfile::tempdir().unwrap();
        // "sub" is never created: a naive `Components`-collecting fallback
        // (no lexical ".." collapse) would leave the literal ".." in place.
        let candidate = temp.path().join("sub").join("..").join("other");

        let resolved = resolve_best_effort(&candidate);

        assert_eq!(resolved, resolve_best_effort(temp.path()).join("other"));
        assert!(!resolved.to_string_lossy().contains(".."), "{resolved:?}");
    }

    #[test]
    fn resolve_best_effort_clamps_a_leading_parent_dir_at_root() {
        let resolved = resolve_best_effort(Path::new("/../../some/nonexistent/dir"));
        assert_eq!(resolved, PathBuf::from("/some/nonexistent/dir"));
    }

    #[test]
    fn resolve_best_effort_drops_a_trailing_slash_on_a_nonexistent_path() {
        let temp = tempfile::tempdir().unwrap();
        let with_trailing_slash = PathBuf::from(format!(
            "{}/",
            temp.path().join("missing").to_string_lossy()
        ));

        let resolved = resolve_best_effort(&with_trailing_slash);

        assert_eq!(resolved, resolve_best_effort(temp.path()).join("missing"));
    }

    #[test]
    fn resolve_best_effort_makes_a_relative_path_absolute() {
        let cwd = std::env::current_dir().unwrap();
        let resolved = resolve_best_effort(Path::new("grid-launcher-test-relative/nonexistent"));
        assert_eq!(
            resolved,
            resolve_best_effort(&cwd)
                .join("grid-launcher-test-relative")
                .join("nonexistent")
        );
    }

    #[test]
    fn resolve_best_effort_canonicalizes_an_existing_path() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("real");
        std::fs::create_dir_all(&dir).unwrap();

        assert_eq!(
            resolve_best_effort(&dir),
            std::fs::canonicalize(&dir).unwrap()
        );
    }
}
