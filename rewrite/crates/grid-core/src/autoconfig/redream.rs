//! Redream's flat `redream.cfg` overwrite writer.
//!
//! Ports `grid_launcher/emulator/redream.py`'s `ensure_redream_settings`
//! (redream.py:154-204). See `docs/porting/05-emulator-autoconfig.md`
//! ("Redream") for the behavior contract.
//!
//! Redream's own directory-settings reader lands in a later milestone task
//! (Task 9); until then this module implements the small slice of
//! `redream_data_root_candidates` (redream.py:57-77) that
//! [`ensure_settings`] needs — the portable-marker probe and the
//! platform-default root — so Task 9 can re-export
//! [`data_root_candidates`] rather than duplicating this logic.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use super::{paths, EnsureResult};

/// The bare filenames [`has_portable_marker`] looks for directly
/// (redream.py:33-35).
const PORTABLE_MARKER_FILES: [&str; 6] = [
    "redream.cfg",
    "flash.bin",
    "vmu0.bin",
    "vmu1.bin",
    "vmu2.bin",
    "vmu3.bin",
];

/// Whether `root` looks like a Redream portable data directory: it holds
/// any of [`PORTABLE_MARKER_FILES`], or any `*.sav`/`*.png` file
/// (redream.py:30-42). `false` for an empty path (redream.py:31's
/// `if not str(root)`) — checked by the caller, since `Path::new("")`
/// already reads as the current directory.
fn has_portable_marker(root: &Path) -> bool {
    if root.as_os_str().is_empty() {
        return false;
    }
    if PORTABLE_MARKER_FILES
        .iter()
        .any(|name| root.join(name).exists())
    {
        return true;
    }
    let Ok(entries) = std::fs::read_dir(root) else {
        return false;
    };
    entries.flatten().any(|entry| {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        name.ends_with(".sav") || name.ends_with(".png")
    })
}

/// `_default_user_root` (redream.py:45-53): `~/Library/Application
/// Support/redream` on macOS, else `$XDG_DATA_HOME/redream` or
/// `~/.local/share/redream`.
fn default_user_root() -> PathBuf {
    if cfg!(target_os = "macos") {
        return paths::home_dir()
            .unwrap_or_default()
            .join("Library")
            .join("Application Support")
            .join("redream");
    }
    paths::xdg_data_home().join("redream")
}

/// `path.trim()` expanded, dir-or-parent — `None` for a blank path
/// (redream.py:20-26's `_emulator_dir`, which returns `Path()` for a blank
/// path; that empty path can never hold a portable marker or a config file,
/// so `None` here is behaviorally equivalent).
fn resolve_emulator_dir(emulator_path: &str) -> Option<PathBuf> {
    let trimmed = emulator_path.trim();
    if trimmed.is_empty() {
        return None;
    }
    let expanded = paths::expand_user(trimmed);
    paths::emulator_dir(&expanded)
}

/// `redream_data_root_candidates` (redream.py:57-77), deduped
/// case-insensitively: the emulator directory FIRST when it holds a
/// portable marker; then the platform default root, but only when it
/// already exists on disk or the host is macOS; then the emulator
/// directory again as a guaranteed fallback tail entry.
pub fn data_root_candidates(emulator_path: &str) -> Vec<PathBuf> {
    let emulator_dir = resolve_emulator_dir(emulator_path);
    let mut candidates = Vec::new();

    if let Some(dir) = &emulator_dir {
        if has_portable_marker(dir) {
            candidates.push(dir.clone());
        }
    }

    let default_root = default_user_root();
    if default_root.exists() || cfg!(target_os = "macos") {
        candidates.push(default_root);
    }

    if let Some(dir) = &emulator_dir {
        candidates.push(dir.clone());
    }

    paths::dedupe_casefold(candidates)
}

/// `redream_directory_settings`'s candidate-selection rule
/// (redream.py:81-107), pulled out as a pure function over an
/// already-built candidate list: the first candidate whose `redream.cfg`
/// already exists, or whose directory itself already exists; when NONE of
/// them exist on disk, the very FIRST candidate in the list (not merely
/// "some" fallback — redream.py:105-106's `if not defaults["data_root"]:
/// defaults = settings` only ever fires on the loop's first iteration, so
/// it always captures candidate zero specifically). `None` for an empty
/// list.
fn select_data_root(candidates: &[PathBuf]) -> Option<PathBuf> {
    for candidate in candidates {
        if candidate.exists() || candidate.join("redream.cfg").exists() {
            return Some(candidate.clone());
        }
    }
    candidates.first().cloned()
}

/// `ensure_redream_settings` (redream.py:154-204). Resolves `config_path`
/// as `<selected data root>/redream.cfg`, where the data root is chosen by
/// [`select_data_root`] over [`data_root_candidates`] — the first EXISTING
/// candidate, falling back to the first candidate only when none exist. No
/// candidates at all (a blank `emulator_path`) yields
/// [`EnsureResult::unchanged`].
///
/// Parses `key=value` lines with no comment or section handling, splitting
/// each on the FIRST `=` and trimming both halves. Managed keys:
/// `mode`=`fullscreen`, `volume`=`40`. When both already hold those exact
/// values, returns `changed = false` with **no write at all** — the file
/// stays byte-identical, mtime included. Otherwise every line whose
/// pre-`=` trimmed key is managed is rewritten `{key}={value}` (no spaces,
/// original spacing dropped; every duplicate occurrence is rewritten — no
/// dedupe, unlike the INI families); every other line, comments included,
/// is preserved verbatim; unwritten managed keys are appended in the
/// pinned order. Output is `lines.join("\n") + "\n"` with **no
/// `trim_end()`** — pre-existing trailing blank lines survive. Any I/O
/// error (reading the existing file, creating the parent, or writing)
/// yields [`EnsureResult::unchanged`].
pub fn ensure_settings(emulator_path: &str) -> EnsureResult {
    let candidates = data_root_candidates(emulator_path);
    let Some(data_root) = select_data_root(&candidates) else {
        return EnsureResult::unchanged();
    };
    let config_path = data_root.join("redream.cfg");

    let existing_lines: Vec<String> = if config_path.exists() {
        match std::fs::read_to_string(&config_path) {
            Ok(text) => text.lines().map(str::to_string).collect(),
            Err(_) => return EnsureResult::unchanged(),
        }
    } else {
        Vec::new()
    };

    // A later duplicate key's value wins — plain `HashMap::insert` already
    // gives this "last write wins" behavior, matching the reference's dict
    // assignment in the same loop (redream.py:174-177).
    let mut parsed: HashMap<&str, String> = HashMap::new();
    for line in &existing_lines {
        if let Some((key, value)) = line.split_once('=') {
            parsed.insert(key.trim(), value.trim().to_string());
        }
    }

    let desired: [(&str, &str); 2] = [("mode", "fullscreen"), ("volume", "40")];
    let already_correct = desired
        .iter()
        .all(|(key, value)| parsed.get(key).map(String::as_str) == Some(*value));

    if already_correct {
        return EnsureResult::at(config_path, false);
    }

    let mut written_keys: HashSet<&str> = HashSet::new();
    let mut output_lines: Vec<String> = Vec::new();
    for line in &existing_lines {
        if let Some((raw_key, _)) = line.split_once('=') {
            let key = raw_key.trim();
            if let Some((managed_key, value)) = desired.iter().find(|(k, _)| *k == key) {
                output_lines.push(format!("{managed_key}={value}"));
                written_keys.insert(managed_key);
                continue;
            }
        }
        output_lines.push(line.clone());
    }
    for (key, value) in desired {
        if !written_keys.contains(key) {
            output_lines.push(format!("{key}={value}"));
        }
    }

    let output = format!("{}\n", output_lines.join("\n"));

    if let Some(parent) = config_path.parent() {
        if std::fs::create_dir_all(parent).is_err() {
            return EnsureResult::unchanged();
        }
    }
    if std::fs::write(&config_path, &output).is_err() {
        return EnsureResult::unchanged();
    }

    EnsureResult::at(config_path, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_exe(temp: &Path) -> (PathBuf, PathBuf) {
        let dir = temp.join("Redream");
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("redream.exe");
        std::fs::write(&exe, b"").unwrap();
        (exe, dir)
    }

    #[test]
    fn redream_writes_mode_and_volume() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, dir) = make_exe(temp.path());
        std::fs::write(dir.join("redream.cfg"), "mode=windowed\nvolume=100\n").unwrap();

        let result = ensure_settings(exe.to_str().unwrap());

        assert!(result.changed);
        let text = std::fs::read_to_string(dir.join("redream.cfg")).unwrap();
        assert!(text.contains("mode=fullscreen"));
        assert!(text.contains("volume=40"));
        assert!(!text.contains("mode=windowed"));
        assert!(!text.contains("volume=100"));
    }

    #[test]
    fn redream_is_idempotent_with_no_write() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, dir) = make_exe(temp.path());
        let cfg_path = dir.join("redream.cfg");
        std::fs::write(&cfg_path, "mode=fullscreen\nvolume=40\n").unwrap();
        let before_bytes = std::fs::read(&cfg_path).unwrap();
        let before_mtime = std::fs::metadata(&cfg_path).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));

        let result = ensure_settings(exe.to_str().unwrap());

        assert!(!result.changed);
        let after_bytes = std::fs::read(&cfg_path).unwrap();
        let after_mtime = std::fs::metadata(&cfg_path).unwrap().modified().unwrap();
        assert_eq!(before_bytes, after_bytes, "bytes must be untouched");
        assert_eq!(before_mtime, after_mtime, "mtime must be untouched");
    }

    #[test]
    fn redream_preserves_comments_and_trailing_blank_lines() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, dir) = make_exe(temp.path());
        // Both managed keys already present (with the wrong values) so
        // nothing needs to be appended after the trailing blank lines —
        // isolating the "no trim_end()" behavior from the separate
        // "missing keys are appended at the end" behavior.
        std::fs::write(
            dir.join("redream.cfg"),
            "# a comment\nmode=windowed\nvolume=10\n\n\n",
        )
        .unwrap();

        ensure_settings(exe.to_str().unwrap());

        let text = std::fs::read_to_string(dir.join("redream.cfg")).unwrap();
        assert!(text.starts_with("# a comment\n"));
        assert!(text.contains("mode=fullscreen"));
        assert!(text.contains("volume=40"));
        assert!(
            text.ends_with("\n\n\n"),
            "pre-existing trailing blank lines must survive: {text:?}"
        );
    }

    #[test]
    fn redream_rewrites_every_duplicate_managed_key() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, dir) = make_exe(temp.path());
        std::fs::write(
            dir.join("redream.cfg"),
            "mode=windowed\nmode=windowed\nvolume=10\n",
        )
        .unwrap();

        ensure_settings(exe.to_str().unwrap());

        let text = std::fs::read_to_string(dir.join("redream.cfg")).unwrap();
        assert_eq!(text.matches("mode=fullscreen").count(), 2);
        assert_eq!(text.matches("volume=40").count(), 1);
    }

    #[test]
    fn redream_blank_path_is_unchanged() {
        // A blank path still routes through `default_user_root()`, which
        // reads `$HOME`/`$XDG_DATA_HOME` — isolate them like every sibling
        // module's env-touching test (crate::test_env's crate-wide lock).
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = crate::test_env::EnvGuard::set(&[
            ("HOME", Some(temp.path().to_str().unwrap())),
            ("XDG_DATA_HOME", None),
        ]);

        let result = ensure_settings("");

        assert!(!result.changed);
        assert_eq!(result.config_path, None);
    }

    // --- select_data_root ---------------------------------------------------

    #[test]
    fn redream_targets_the_first_existing_candidate_not_just_the_first() {
        let temp = tempfile::tempdir().unwrap();
        let candidate1 = temp.path().join("candidate1"); // never created
        let candidate2 = temp.path().join("candidate2");
        std::fs::create_dir_all(&candidate2).unwrap();
        let candidates = vec![candidate1.clone(), candidate2.clone()];

        assert_eq!(
            select_data_root(&candidates),
            Some(candidate2),
            "candidate 1 is absent, candidate 2 is an existing dir — candidate 2 must win"
        );

        // Neither candidate exists on disk any more: falls back to the
        // very first candidate in the list.
        std::fs::remove_dir_all(temp.path().join("candidate2")).unwrap();
        assert_eq!(select_data_root(&candidates), Some(candidate1));
    }

    #[test]
    fn redream_select_data_root_is_none_for_an_empty_list() {
        assert_eq!(select_data_root(&[]), None);
    }
}
