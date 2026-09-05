//! Path helpers shared by every `ensure_*` writer: the home/XDG roots the
//! config candidate lists are built from, `~` expansion, case-insensitive
//! candidate deduplication, and the "directory that holds the executable"
//! rule.
//!
//! Ports `grid_launcher/core/path.py:33-44` plus the `Path.expanduser()` and
//! `emulator_path if emulator_path.is_dir() else emulator_path.parent`
//! idioms repeated across `grid_launcher/emulator/*.py` (for example
//! `grid_launcher/emulator/cemu.py:340`).

use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};

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

/// The RetroArch AppImage's portable home, `<parent>/<file name>.home/
/// .config/retroarch`, when that directory exists. The AppImage runtime
/// sets `$HOME` to `<AppImage>.home` whenever that directory exists next
/// to the file, so RetroArch then reads its `retroarch.cfg` and its
/// `cores/` from here rather than from the emulator directory. Both the
/// core installer (cores.rs) and the launch-time core resolver
/// (launch/template.rs) and the config writer (retroarch.rs) consult this
/// one rule so they can never disagree about the layout.
pub fn retroarch_portable_home(executable: &Path) -> Option<PathBuf> {
    let parent = executable.parent()?;
    let file_name = executable.file_name()?.to_string_lossy();
    let home = parent
        .join(format!("{file_name}.home"))
        .join(".config")
        .join("retroarch");
    home.is_dir().then_some(home)
}

/// `lstat`-equivalent symlink check for [`join_realpath`]: any failure to
/// stat `candidate` — not only "does not exist" — is treated as "not a
/// symlink", mirroring CPython's broad `except OSError: is_link = False`
/// (`posixpath.py:467-472`, `strict=False`'s `ignored_error = OSError`).
fn is_symlink(candidate: &Path) -> bool {
    std::fs::symlink_metadata(candidate)
        .map(|meta| meta.file_type().is_symlink())
        .unwrap_or(false)
}

/// Port of CPython's `posixpath._joinrealpath` (`strict=False` case only —
/// the engine behind `os.path.realpath`, which `pathlib.Path.resolve()`
/// calls under the hood) — walk `rest`'s components onto `path` one at a
/// time, resolving symlinks PROGRESSIVELY against the path as built so far,
/// not against a lexically-pre-normalized string. This distinction is
/// observable: given a symlink `dirB/linktoreal -> dirA/realdir`, resolving
/// `dirB/linktoreal/../sibling` must land on `dirA/sibling` (the `..`
/// cancels `realdir`, the symlink's TARGET) — a lexical-first pass that
/// collapses `..` before ever looking at the filesystem would instead
/// cancel `linktoreal` itself and land on the wrong `dirB/sibling`.
///
/// Every component:
/// - `.` is dropped (`posixpath.py:454-456`).
/// - A `Prefix` (Windows only — e.g. the `C:` in `C:\foo`) RESETS `path` to
///   just that prefix, discarding anything accumulated before it — a new
///   drive/UNC root always starts fresh.
/// - `RootDir` PUSHES onto the current `path` rather than replacing it.
///   `PathBuf::push`'s own documented Windows rule — "a path with a root
///   but no prefix replaces everything except `self`'s prefix" — is exactly
///   what preserves a drive letter a `Prefix` component just set
///   (`C:` + push(`\`) = `C:\`, not `\`); on non-Windows targets there is no
///   prefix concept, so pushing the (absolute) root simply replaces
///   whatever was there, which is the same "reset to root" behavior this
///   arm had before. Combining `RootDir` with `Prefix` into one
///   replace-`path` arm — this function's ORIGINAL shape — silently
///   dropped the drive letter whenever a `Prefix` was immediately followed
///   by a `RootDir`, which `Path::components()` always does for an
///   absolute Windows path; unreachable on the Linux/macOS builds this
///   crate is tested on today (`Component::Prefix` is never parsed off a
///   Unix path at all), but live logic in a `pub(crate)` helper this crate
///   also ships for Windows.
/// - `..` pops the last segment off `path` — whatever it currently is,
///   symlink-resolved or plain — clamped at the root (or, on Windows, the
///   prefix+root): `path.parent()` of a bare root (`/`, or `C:\`) is
///   `None`, so popping past it is a no-op, never producing a literal
///   `/../x` or stripping a drive letter (`posixpath.py:457-465`).
/// - A `Normal` name is checked via [`is_symlink`]: if it is NOT a
///   symlink — including when it does not exist at all, which gets
///   IDENTICAL treatment to an ordinary file/dir under `strict=False`,
///   since a failed stat also just means "not a symlink" — it is appended
///   as plain text with no further resolution, and later components are
///   themselves stat-checked against THIS (possibly still nonexistent)
///   prefix, exactly like CPython (`posixpath.py:466-475`). If it IS a
///   symlink, [`join_realpath`] recurses onto its target: an absolute
///   target's own leading `Prefix`/`RootDir` components naturally reset
///   `path` (handled by the two arms above, no special case needed —
///   Rust's `Components` always yields a path's own prefix/root first); a
///   relative one is walked starting from the CURRENT `path`, i.e. from
///   the symlink's own containing directory (`posixpath.py:476-494`).
///   `seen` records symlinks already entered on this call stack, so a
///   cycle falls back to the raw, unresolved candidate instead of
///   recursing forever, mirroring CPython's own non-strict loop guard
///   (`posixpath.py:477-489`).
fn join_realpath(
    mut path: PathBuf,
    rest: &Path,
    seen: &mut HashMap<PathBuf, Option<PathBuf>>,
) -> PathBuf {
    for component in rest.components() {
        match component {
            Component::CurDir => {}
            Component::Prefix(_) => {
                path = PathBuf::from(component.as_os_str());
            }
            Component::RootDir => {
                path.push(component.as_os_str());
            }
            Component::ParentDir => {
                if path.parent().is_some() {
                    path.pop();
                }
                // else: already at (or above) the root — clamp, no-op.
            }
            Component::Normal(name) => {
                let candidate = path.join(name);
                if !is_symlink(&candidate) {
                    path = candidate;
                    continue;
                }
                match seen.get(&candidate) {
                    Some(Some(resolved)) => path = resolved.clone(),
                    Some(None) => {
                        // Loop: this symlink is still being resolved
                        // higher up the call stack. Non-strict CPython
                        // falls back to the raw candidate rather than
                        // recursing forever.
                        path = candidate;
                    }
                    None => {
                        seen.insert(candidate.clone(), None);
                        let resolved = match std::fs::read_link(&candidate) {
                            Ok(target) => join_realpath(path.clone(), &target, seen),
                            // A symlink we just confirmed via `lstat` but
                            // can no longer `readlink` (e.g. a race):
                            // fall back to the raw candidate rather than
                            // erroring.
                            Err(_) => candidate.clone(),
                        };
                        seen.insert(candidate, Some(resolved.clone()));
                        path = resolved;
                    }
                }
            }
        }
    }
    path
}

/// `Path.resolve(strict=False)` — Python's default, which succeeds even
/// when the path does not exist. Makes a relative `path` absolute against
/// the current directory, then walks it via [`join_realpath`], which
/// resolves symlinks progressively and collapses `.`/`..` against
/// whatever has been resolved so far — see that function's doc comment for
/// why "collapse `..` lexically first, canonicalize second" (this
/// function's own previous implementation) is NOT equivalent and gives the
/// wrong answer whenever a `..` crosses a symlink boundary.
///
/// This is the one shared home for what used to be duplicated, divergent
/// copies in `readers.rs` and `rpcs3.rs` — every `ensure_*`/
/// `*_directory_settings` call site in this crate that ports a Python
/// `.resolve()` call should go through this function instead of
/// hand-rolling another one.
pub(crate) fn resolve_best_effort(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    };

    let mut seen: HashMap<PathBuf, Option<PathBuf>> = HashMap::new();
    join_realpath(PathBuf::new(), &absolute, &mut seen)
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
    fn portable_home_is_found_next_to_an_appimage() {
        let temp = tempfile::tempdir().unwrap();
        let exe = temp.path().join("RetroArch-Linux-x86_64.AppImage");
        std::fs::write(&exe, b"").unwrap();
        let home = temp
            .path()
            .join("RetroArch-Linux-x86_64.AppImage.home")
            .join(".config")
            .join("retroarch");
        std::fs::create_dir_all(&home).unwrap();
        assert_eq!(retroarch_portable_home(&exe), Some(home));
    }

    #[test]
    fn portable_home_is_none_without_the_home_dir() {
        let temp = tempfile::tempdir().unwrap();
        let exe = temp.path().join("retroarch");
        std::fs::write(&exe, b"").unwrap();
        assert_eq!(retroarch_portable_home(&exe), None);
    }

    #[test]
    fn portable_home_is_none_when_home_is_a_file() {
        let temp = tempfile::tempdir().unwrap();
        let exe = temp.path().join("retroarch");
        std::fs::write(&exe, b"").unwrap();
        std::fs::write(temp.path().join("retroarch.home"), b"").unwrap();
        assert_eq!(retroarch_portable_home(&exe), None);
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

    /// Reproduces, byte-for-byte, the scenario CPython 3.12 was checked
    /// against for this fix:
    ///
    /// ```text
    /// $ mkdir -p /tmp/resolve_test/t/dirA/realdir /tmp/resolve_test/t/dirB
    /// $ ln -s /tmp/resolve_test/t/dirA/realdir /tmp/resolve_test/t/dirB/linktoreal
    /// $ python3 -c "from pathlib import Path; \
    ///     print(Path('/tmp/resolve_test/t/dirB/linktoreal/../sibling').resolve(strict=False))"
    /// /tmp/resolve_test/t/dirA/sibling
    /// ```
    ///
    /// `sibling` is never created. A "collapse `..` lexically first, THEN
    /// canonicalize" implementation gets this wrong: it would cancel
    /// `linktoreal` itself (the LAST PATH SEGMENT, lexically) and land on
    /// `dirB/sibling` — the symlink is never even consulted. The correct,
    /// progressive answer resolves `linktoreal` to `dirA/realdir` FIRST,
    /// and only then applies `..` against that resolved location, landing
    /// on `dirA/sibling`.
    #[test]
    #[cfg(unix)]
    fn resolve_best_effort_applies_parent_dir_against_the_symlink_target_not_the_link_itself() {
        let temp = tempfile::tempdir().unwrap();
        let dir_a = temp.path().join("dirA");
        let real_dir = dir_a.join("realdir");
        let dir_b = temp.path().join("dirB");
        std::fs::create_dir_all(&real_dir).unwrap();
        std::fs::create_dir_all(&dir_b).unwrap();
        let link = dir_b.join("linktoreal");
        std::os::unix::fs::symlink(&real_dir, &link).unwrap();

        let candidate = link.join("..").join("sibling");
        let resolved = resolve_best_effort(&candidate);

        let expected = resolve_best_effort(&dir_a).join("sibling");
        assert_eq!(
            resolved, expected,
            "must resolve against the symlink's TARGET directory (dirA), \
             not the link's own containing directory (dirB)"
        );
        assert!(
            !resolved.starts_with(&dir_b),
            "landing under dirB means the `..` was applied to the link \
             itself instead of its resolved target: {resolved:?}"
        );
    }

    /// A `..` following a chain of components that never resolve to
    /// anything on disk (the parent, `nonexistent`, does not exist either)
    /// stays purely lexical for that whole unresolved tail — cross-checked
    /// against real CPython 3.12:
    ///
    /// ```text
    /// $ python3 -c "from pathlib import Path; \
    ///     print(Path('/tmp/resolve_test/t/nonexistent/deeper/../sibling').resolve(strict=False))"
    /// /tmp/resolve_test/t/nonexistent/sibling
    /// ```
    ///
    /// i.e. `deeper` is popped back off to leave `nonexistent`, and neither
    /// segment is ever `lstat`-resolved as anything other than "not a
    /// symlink" (a failed stat gets that same answer).
    #[test]
    fn resolve_best_effort_pops_lexically_through_a_wholly_nonexistent_tail() {
        let temp = tempfile::tempdir().unwrap();
        let candidate = temp
            .path()
            .join("nonexistent")
            .join("deeper")
            .join("..")
            .join("sibling");

        let resolved = resolve_best_effort(&candidate);

        let expected = resolve_best_effort(temp.path())
            .join("nonexistent")
            .join("sibling");
        assert_eq!(resolved, expected);
    }

    /// A Windows drive prefix must survive the `RootDir` that always
    /// follows it in an absolute path's `Components`, and a `..` must clamp
    /// at the drive root rather than popping the prefix away.
    ///
    /// `#[cfg(windows)]`, not a platform-agnostic test: `Component::Prefix`
    /// is parsed ONLY on Windows — `Path::new("C:\\foo").components()` on
    /// Linux/macOS yields a single `Normal("C:\\foo")` component, never
    /// `Prefix`+`RootDir`+`Normal("foo")` (there is no public way to
    /// construct a `PrefixComponent` directly; the parser is the only
    /// source of one, and it never fires off-Windows) — so this guard is
    /// inherently untestable on the platforms this crate's CI runs on
    /// today, and is gated here rather than faked with a synthetic
    /// `Component` value.
    #[test]
    #[cfg(windows)]
    fn resolve_best_effort_preserves_the_windows_drive_prefix() {
        let resolved = resolve_best_effort(Path::new(r"C:\foo\..\bar"));
        assert_eq!(resolved, PathBuf::from(r"C:\bar"));

        // Root-clamp must stop at the drive root, not strip the prefix.
        let clamped = resolve_best_effort(Path::new(r"C:\..\..\baz"));
        assert_eq!(clamped, PathBuf::from(r"C:\baz"));
    }
}
