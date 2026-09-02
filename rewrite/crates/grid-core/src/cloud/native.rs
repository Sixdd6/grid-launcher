//! Native (non-cloud-archive) save path resolution, Wine-prefix path
//! translation, and `native_multi_dir` manifest-driven restore.
//!
//! Ported from `grid_launcher/library/cloud_transfer.py`:
//! `resolve_native_save_dir` (:484-541), `normalize_manual_save_path`
//! (:544-587); `grid_launcher/emulator/wine.py`'s
//! `translate_windows_path_to_wine_prefix` (the whole file); and
//! `grid_launcher/ui/mixins/cloud_mixin.py`'s inline `native_multi_dir`
//! restore branch (:2176-2246) plus the manual/PCGW path merge at :2689.
//!
//! **Signature shape differs from Python, both pinned by the task brief:**
//! - `resolve_native_save_dir` drops Python's `sys.platform == "win32"`
//!   gate around the Windows-Documents-redirection branch. Real Python only
//!   ever takes that branch when the *host* OS is Windows; this port takes
//!   it purely off whether `windows_documents` is `Some`, so the branch is
//!   exercisable (and its own unit tests can drive it) on any host,
//!   including the Linux machine this crate is developed and tested on.
//! - Both `resolve_native_save_dir`'s plain-expansion fallback and
//!   `normalize_manual_save_path`'s reverse-prefix lookups use `%VAR%`
//!   (Windows-style) substitution against the process environment
//!   unconditionally, rather than Python's real `os.path.expandvars`, whose
//!   `%VAR%` handling is itself Windows-only (on POSIX it expands only
//!   `$VAR`/`${VAR}`, leaving `%VAR%` text untouched — see
//!   [`expand_percent_vars`]'s doc comment for the full reasoning). Every
//!   Python unit test this module ports drives `resolve_native_save_dir`/
//!   `normalize_manual_save_path` through a *mocked* `os.path.expandvars`
//!   standing in for real Windows substitution, not real (unmocked) POSIX
//!   `expandvars` — so a faithful port of what those tests actually pin
//!   requires `%VAR%` substitution to work on every host, driven by real
//!   process env vars set under `test_env`'s lock. This is the intentional
//!   resolution of the tension between the brief's own "OS-conditional
//!   expandvars" note and its pinned oracle test tables: the tables win.

use std::collections::HashMap;
use std::io::{self, Cursor, Read};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;
use zip::ZipArchive;

use super::archive::{is_safe_member_name, payload_is_zip, resolve_under_root};
use crate::autoconfig::paths::resolve_best_effort;

// ---------------------------------------------------------------------
// Env expansion
// ---------------------------------------------------------------------

/// `%NAME%` — the Windows-style token `resolve_native_save_dir` and
/// `normalize_manual_save_path`'s hardcoded prefixes are always written in
/// (`%APPDATA%`, `%LOCALAPPDATA%`, `%USERPROFILE%`, ...).
static PERCENT_VAR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"%([A-Za-z_][A-Za-z0-9_]*)%").unwrap());

/// Replace every `%NAME%` token in `raw` with `std::env::var(NAME)`'s
/// value, leaving an unset variable's literal `%NAME%` text untouched —
/// matching Python's `os.path.expandvars` semantics (never raises, never
/// blanks an unresolved reference), applied to the `%VAR%` syntax these two
/// functions' hardcoded prefixes always use. See this module's doc comment
/// for why this substitution is unconditional rather than gated to a
/// Windows compile target.
fn expand_percent_vars(raw: &str) -> String {
    PERCENT_VAR_RE
        .replace_all(raw, |caps: &regex::Captures| {
            let name = &caps[1];
            std::env::var(name).unwrap_or_else(|_| caps[0].to_string())
        })
        .into_owned()
}

/// `str(PureWindowsPath(text))`'s separator normalization: forward slashes
/// become backslashes. No further normalization (no redundant-separator
/// collapse, no `.`/`..` handling) — the callers' fixtures never need it.
fn to_windows_display(text: &str) -> String {
    text.replace('/', "\\")
}

/// `text.rstrip("/\\")` — strip trailing path separators of either flavor.
fn trim_trailing_seps(text: &str) -> &str {
    text.trim_end_matches(['/', '\\'])
}

// ---------------------------------------------------------------------
// Username lookup (Wine prefix user directory)
// ---------------------------------------------------------------------

/// The current OS username, matching `getpass.getuser()`'s real POSIX
/// behavior (`emulator/wine.py:17` calls it directly): check `LOGNAME`,
/// `USER`, `LNAME`, then `USERNAME` in order, first non-empty wins; failing
/// all four, fall back to the password-database entry for the real
/// effective user id.
///
/// Python's tests mock `getpass.getuser` directly; Rust has no
/// monkeypatching, so this module's tests instead set `LOGNAME` (checked
/// first) under the crate's `test_env` lock, which deterministically wins
/// over whatever the ambient test-runner environment happens to have set.
fn current_username() -> String {
    for var in ["LOGNAME", "USER", "LNAME", "USERNAME"] {
        if let Ok(value) = std::env::var(var) {
            if !value.is_empty() {
                return value;
            }
        }
    }
    passwd_fallback_username().unwrap_or_default()
}

/// `pwd.getpwuid(os.getuid()).pw_name` — the real-user password-database
/// fallback `getpass.getuser()` uses when no env var names a user.
///
/// `getuid`, not `geteuid`: Python reads the REAL uid, so a GRID started
/// under `sudo`/setuid resolves the invoking user's name, not root's.
#[cfg(unix)]
fn passwd_fallback_username() -> Option<String> {
    // SAFETY: `getpwuid` returns either null or a pointer into a
    // libc-owned static buffer; the name is copied out into an owned
    // `String` immediately and the pointer is never retained or reused
    // across calls.
    unsafe {
        let passwd = libc::getpwuid(libc::getuid());
        if passwd.is_null() {
            return None;
        }
        let name = (*passwd).pw_name;
        if name.is_null() {
            return None;
        }
        Some(
            std::ffi::CStr::from_ptr(name)
                .to_string_lossy()
                .into_owned(),
        )
    }
}

#[cfg(not(unix))]
fn passwd_fallback_username() -> Option<String> {
    None
}

// ---------------------------------------------------------------------
// Wine prefix path translation
// ---------------------------------------------------------------------

/// Translates a Windows env-var save path to its Linux path inside a Wine
/// `prefix`. Ports `translate_windows_path_to_wine_prefix`
/// (`grid_launcher/emulator/wine.py:7-42`) exactly: `%USERPROFILE%\AppData\
/// LocalLow` and `%USERPROFILE%\Documents` are matched (and stripped)
/// *before* the bare `%USERPROFILE%` prefix — that precedence is load
/// bearing, since `%USERPROFILE%` is itself a prefix of both — followed by
/// `%APPDATA%`, `%LOCALAPPDATA%`, then the bare `%USERPROFILE%`, then
/// `%PROGRAMDATA%`, `%PUBLIC%`, `%WINDIR%`. Matching is case-insensitive;
/// `None` when no known token prefixes `raw`.
pub fn translate_windows_path_to_wine_prefix(raw: &str, prefix: &Path) -> Option<PathBuf> {
    let username = current_username();
    let drive_c = prefix.join("drive_c");
    let user_home = drive_c.join("users").join(&username);

    let mappings: [(&str, PathBuf); 8] = [
        (
            "%USERPROFILE%\\AppData\\LocalLow",
            user_home.join("AppData").join("LocalLow"),
        ),
        ("%USERPROFILE%\\Documents", user_home.join("Documents")),
        ("%APPDATA%", user_home.join("AppData").join("Roaming")),
        ("%LOCALAPPDATA%", user_home.join("AppData").join("Local")),
        ("%USERPROFILE%", user_home.clone()),
        ("%PROGRAMDATA%", drive_c.join("ProgramData")),
        ("%PUBLIC%", drive_c.join("users").join("Public")),
        ("%WINDIR%", drive_c.join("windows")),
    ];

    let raw_cf = raw.to_lowercase();
    for (token, base) in mappings {
        let token_cf = token.to_lowercase();
        if let Some(rest) = raw_cf.strip_prefix(&token_cf) {
            let suffix_len = rest.len();
            let raw_suffix = &raw[raw.len() - suffix_len..];
            let normalized = raw_suffix.replace('\\', "/");
            let trimmed = normalized.trim_start_matches('/');
            if !trimmed.is_empty() {
                return Some(base.join(trimmed));
            }
            return Some(base);
        }
    }

    None
}

// ---------------------------------------------------------------------
// Native save directory resolution
// ---------------------------------------------------------------------

/// Expands a raw env-var save path, correcting for Windows Documents-folder
/// redirection. Ports `resolve_native_save_dir` (`cloud_transfer.py:484-541`)
/// — see this module's doc comment for the two pinned signature/behavior
/// deviations (no platform gate; unconditional `%VAR%` expansion).
///
/// When `wine_prefix` is `Some`, [`translate_windows_path_to_wine_prefix`]
/// is tried first; its `Some` result short-circuits everything below.
/// Otherwise `raw` is `%VAR%`-expanded. With `windows_documents` `None`,
/// that expansion is the final answer. Otherwise: if the Shell-resolved
/// Documents path already matches `%USERPROFILE%\Documents` (case-
/// insensitively, trailing separators ignored), there is no redirection —
/// return the plain expansion; if the expansion is exactly (or rooted
/// under) `%USERPROFILE%\Documents`, splice `windows_documents` in for that
/// prefix; otherwise the plain expansion stands untouched.
pub fn resolve_native_save_dir(
    raw: &str,
    windows_documents: Option<&Path>,
    wine_prefix: Option<&Path>,
) -> PathBuf {
    if let Some(prefix) = wine_prefix {
        if let Some(translated) = translate_windows_path_to_wine_prefix(raw, prefix) {
            return translated;
        }
    }

    let expanded_str = expand_percent_vars(raw);
    let expanded = PathBuf::from(&expanded_str);

    let Some(windows_documents) = windows_documents else {
        return expanded;
    };

    let userprofile = expand_percent_vars("%USERPROFILE%");
    let docs_via_env = format!("{}\\Documents", trim_trailing_seps(&userprofile));

    let windows_documents_display = windows_documents.to_string_lossy().into_owned();
    if trim_trailing_seps(&windows_documents_display).to_lowercase()
        == trim_trailing_seps(&docs_via_env).to_lowercase()
    {
        return expanded;
    }

    let expanded_display = to_windows_display(&expanded_str);
    let expanded_cf = expanded_display.to_lowercase();
    let docs_prefix = trim_trailing_seps(&docs_via_env).to_lowercase();

    if expanded_cf == docs_prefix {
        return windows_documents.to_path_buf();
    }

    if expanded_cf.starts_with(&format!("{docs_prefix}\\"))
        || expanded_cf.starts_with(&format!("{docs_prefix}/"))
    {
        let suffix = expanded_display[docs_via_env.len()..].trim_start_matches(['\\', '/']);
        return PathBuf::from(format!("{windows_documents_display}\\{suffix}"));
    }

    expanded
}

/// Replaces hardcoded user-profile path prefixes with env-var equivalents
/// so a manually-added save folder stays portable across reinstalls or
/// username changes. Ports `normalize_manual_save_path`
/// (`cloud_transfer.py:544-587`): forward slashes are normalized to
/// backslashes before matching; candidate prefixes are tried in precedence
/// order `%APPDATA%`, `%LOCALAPPDATA%`, `%USERPROFILE%\AppData\LocalLow`,
/// `%USERPROFILE%\Documents`, `%USERPROFILE%` (LocalLow and Documents
/// *before* the bare `%USERPROFILE%`, for the same reason as the Wine
/// mapping table — `%USERPROFILE%` is a prefix of both). A candidate whose
/// own expansion is unresolved (still starts with `%`, i.e. the env var is
/// unset) is skipped. A match requires the character right after the
/// prefix to be a separator (or end of string) — `AppData\Local` matching
/// inside `AppData\LocalLow` by pure string prefix, then rejected here, is
/// exactly why the LocalLow candidate must also be tried on its own. No
/// prefix matching: the ORIGINAL (pre-normalization) input string is
/// returned unchanged, matching Python's `return folder` (not
/// `folder_str`).
pub fn normalize_manual_save_path(path: &Path) -> String {
    let original = path.to_string_lossy().into_owned();
    let folder_str = to_windows_display(&original);
    let folder_lower = folder_str.to_lowercase();

    let userprofile = expand_percent_vars("%USERPROFILE%");
    let userprofile_trimmed = trim_trailing_seps(&userprofile).to_string();
    let appdata = expand_percent_vars("%APPDATA%");
    let localappdata = expand_percent_vars("%LOCALAPPDATA%");

    let candidates: [(&str, String); 5] = [
        ("%APPDATA%", appdata),
        ("%LOCALAPPDATA%", localappdata),
        (
            r"%USERPROFILE%\AppData\LocalLow",
            format!("{userprofile_trimmed}\\AppData\\LocalLow"),
        ),
        (
            r"%USERPROFILE%\Documents",
            format!("{userprofile_trimmed}\\Documents"),
        ),
        ("%USERPROFILE%", userprofile),
    ];

    for (env_var, expanded) in &candidates {
        if expanded.starts_with('%') {
            continue;
        }
        let prefix = trim_trailing_seps(expanded);
        if !folder_lower.starts_with(&prefix.to_lowercase()) {
            continue;
        }
        let after = &folder_str[prefix.len()..];
        if let Some(first) = after.chars().next() {
            if first != '\\' && first != '/' {
                continue;
            }
        }
        let remainder = after.trim_start_matches(['\\', '/']);
        return if remainder.is_empty() {
            (*env_var).to_string()
        } else {
            format!("{env_var}\\{remainder}")
        };
    }

    original
}

// ---------------------------------------------------------------------
// native_multi_dir manifest restore
// ---------------------------------------------------------------------

/// Restores a `native_multi_dir` cloud-save archive: parses its
/// `_grid_launcher_dirs.json` manifest (`{"<index>": "<raw save path>"}`),
/// then for every non-manifest, non-directory member `"<index>/<relative>"`
/// resolves the destination root (manifest's raw path for that index,
/// resolved via [`resolve_native_save_dir`]; else `fallback_dirs[0]`; else
/// the member is skipped) and writes the file under it, overwriting
/// whatever is already there. Ports the inline restore branch in
/// `cloud_mixin.py:2176-2246`.
///
/// A missing or malformed manifest degrades to an empty map (every member
/// then falls back to `fallback_dirs[0]`, or is skipped with none
/// configured) rather than failing the whole restore.
///
/// Per-member zip-slip protection reuses [`is_safe_member_name`] and
/// [`resolve_under_root`] from `cloud::archive` — the same guard
/// `extract_payload_zip` uses — rather than re-implementing Python's
/// `Path.resolve()` + `relative_to()` check by hand: the destination root
/// is best-effort-resolved (mirroring `target_root.resolve()`, no
/// existence required) via `autoconfig::paths::resolve_best_effort`, then
/// the member's relative path is walked and symlink-resolved against it,
/// with any result that escapes the root skipped.
///
/// A genuine I/O failure (directory creation, file creation, or copy)
/// aborts the whole restore with an `Err`, matching Python's un-caught
/// exception propagating out of the restore loop to its caller's `except
/// Exception` — only the routing/zip-slip decisions above are per-member
/// skips.
pub fn restore_native_multi_dir_archive(
    payload: &[u8],
    fallback_dirs: &[PathBuf],
    windows_documents: Option<&Path>,
    wine_prefix: Option<&Path>,
) -> Result<usize, String> {
    if !payload_is_zip(payload) {
        return Err("Downloaded save is not a valid zip archive.".to_string());
    }
    let mut archive = ZipArchive::new(Cursor::new(payload))
        .map_err(|_| "Downloaded save is not a valid zip archive.".to_string())?;

    let manifest: HashMap<String, String> = match archive.by_name("_grid_launcher_dirs.json") {
        Ok(mut entry) => {
            let mut buf = Vec::new();
            match entry.read_to_end(&mut buf) {
                Ok(_) => serde_json::from_slice(&buf).unwrap_or_default(),
                Err(_) => HashMap::new(),
            }
        }
        Err(_) => HashMap::new(),
    };

    let mut written = 0usize;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("failed to read archive entry: {e}"))?;
        let member_name = entry.name().to_string();
        if member_name == "_grid_launcher_dirs.json" || member_name.ends_with('/') {
            continue;
        }
        let Some((dir_idx, relative_str)) = member_name.split_once('/') else {
            continue;
        };
        if relative_str.is_empty() {
            continue;
        }

        let target_root: PathBuf = if let Some(raw_path) = manifest.get(dir_idx) {
            resolve_native_save_dir(raw_path, windows_documents, wine_prefix)
        } else if let Some(first) = fallback_dirs.first() {
            first.clone()
        } else {
            continue;
        };

        let resolved_root = resolve_best_effort(&target_root);

        let normalized = relative_str.replace('\\', "/");
        if !is_safe_member_name(&normalized) {
            continue;
        }
        let relative = Path::new(&normalized);
        let destination = resolve_under_root(&resolved_root, relative)
            .map_err(|e| format!("failed to resolve extraction path: {e}"))?;
        if !destination.starts_with(&resolved_root) {
            continue;
        }

        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create directory: {e}"))?;
        }
        let mut out_file = std::fs::File::create(&destination)
            .map_err(|e| format!("failed to create file: {e}"))?;
        io::copy(&mut entry, &mut out_file).map_err(|e| format!("failed to extract file: {e}"))?;
        written += 1;
    }

    Ok(written)
}

// ---------------------------------------------------------------------
// Combined PCGW + manual save path list
// ---------------------------------------------------------------------

/// `pcgw + [m for m in manual if m not in pcgw]` (`cloud_mixin.py:2689`):
/// every PCGW-sourced path, followed by every manually-added path not
/// already present among them (manual-internal duplicates are not
/// themselves deduplicated, matching the Python list comprehension).
pub fn native_save_paths(pcgw: &[String], manual: &[String]) -> Vec<String> {
    let mut combined = pcgw.to_vec();
    for m in manual {
        if !pcgw.contains(m) {
            combined.push(m.clone());
        }
    }
    combined
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_env::EnvGuard;
    use std::io::Write as _;
    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipWriter};

    fn build_zip_bytes(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        for &(name, content) in entries {
            writer.start_file(name, options).unwrap();
            writer.write_all(content).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    // -------------------------------------------------------------
    // resolve_native_save_dir — test_cloud_transfer.py:30,45,62,82
    // -------------------------------------------------------------

    /// Ports `test_resolve_native_save_dir_returns_expanded_when_no_windows_documents`
    /// (`test_cloud_transfer.py:30`).
    #[test]
    fn resolve_native_save_dir_returns_expanded_when_no_windows_documents() {
        let _lock = crate::test_env::lock();
        let _guard = EnvGuard::set(&[("USERPROFILE", Some("C:\\Users\\TestUser"))]);

        let result = resolve_native_save_dir("%USERPROFILE%\\Documents\\Game\\saves", None, None);

        assert_eq!(
            result,
            PathBuf::from("C:\\Users\\TestUser\\Documents\\Game\\saves")
        );
    }

    /// Ports `test_resolve_native_save_dir_no_redirection_returns_standard_expansion`
    /// (`test_cloud_transfer.py:45`).
    #[test]
    fn resolve_native_save_dir_no_redirection_returns_standard_expansion() {
        let _lock = crate::test_env::lock();
        let _guard = EnvGuard::set(&[("USERPROFILE", Some("C:\\Users\\TestUser"))]);
        let windows_documents = PathBuf::from("C:\\Users\\TestUser\\Documents");

        let result = resolve_native_save_dir(
            "%USERPROFILE%\\Documents\\Game\\saves",
            Some(&windows_documents),
            None,
        );

        assert_eq!(
            result,
            PathBuf::from("C:\\Users\\TestUser\\Documents\\Game\\saves")
        );
    }

    /// Ports `test_resolve_native_save_dir_redirected_documents_uses_shell_path`
    /// (`test_cloud_transfer.py:62`).
    #[test]
    fn resolve_native_save_dir_redirected_documents_uses_shell_path() {
        let _lock = crate::test_env::lock();
        let _guard = EnvGuard::set(&[("USERPROFILE", Some("C:\\Users\\TestUser"))]);
        let windows_documents = PathBuf::from("Y:\\Users\\TestUser\\Documents");

        let result = resolve_native_save_dir(
            "%USERPROFILE%\\Documents\\Square Enix\\Batman GOTY\\SaveData",
            Some(&windows_documents),
            None,
        );

        assert_eq!(
            result,
            PathBuf::from("Y:\\Users\\TestUser\\Documents\\Square Enix\\Batman GOTY\\SaveData")
        );
    }

    /// Ports `test_resolve_native_save_dir_non_documents_path_unaffected_by_redirection`
    /// (`test_cloud_transfer.py:82`).
    #[test]
    fn resolve_native_save_dir_non_documents_path_unaffected_by_redirection() {
        let _lock = crate::test_env::lock();
        let _guard = EnvGuard::set(&[
            ("USERPROFILE", Some("C:\\Users\\TestUser")),
            ("APPDATA", Some("C:\\Users\\TestUser\\AppData\\Roaming")),
        ]);
        let windows_documents = PathBuf::from("Y:\\Users\\TestUser\\Documents");

        let result =
            resolve_native_save_dir("%APPDATA%\\Game\\saves", Some(&windows_documents), None);

        assert_eq!(
            result,
            PathBuf::from("C:\\Users\\TestUser\\AppData\\Roaming\\Game\\saves")
        );
    }

    // -------------------------------------------------------------
    // normalize_manual_save_path — test_cloud_transfer.py:98-188
    // -------------------------------------------------------------

    fn normalize_env() -> EnvGuard {
        EnvGuard::set(&[
            ("APPDATA", Some("C:\\Users\\TestUser\\AppData\\Roaming")),
            ("LOCALAPPDATA", Some("C:\\Users\\TestUser\\AppData\\Local")),
            ("USERPROFILE", Some("C:\\Users\\TestUser")),
        ])
    }

    #[test]
    fn normalize_manual_save_path_appdata_roaming() {
        let _lock = crate::test_env::lock();
        let _guard = normalize_env();

        let result = normalize_manual_save_path(Path::new(
            "C:\\Users\\TestUser\\AppData\\Roaming\\SomeGame\\saves",
        ));

        assert_eq!(result, "%APPDATA%\\SomeGame\\saves");
    }

    #[test]
    fn normalize_manual_save_path_appdata_local() {
        let _lock = crate::test_env::lock();
        let _guard = normalize_env();

        let result = normalize_manual_save_path(Path::new(
            "C:\\Users\\TestUser\\AppData\\Local\\SomeGame\\saves",
        ));

        assert_eq!(result, "%LOCALAPPDATA%\\SomeGame\\saves");
    }

    #[test]
    fn normalize_manual_save_path_appdata_locallow() {
        let _lock = crate::test_env::lock();
        let _guard = normalize_env();

        let result = normalize_manual_save_path(Path::new(
            "C:\\Users\\TestUser\\AppData\\LocalLow\\Paralives\\MySaves.mod",
        ));

        assert_eq!(
            result,
            "%USERPROFILE%\\AppData\\LocalLow\\Paralives\\MySaves.mod"
        );
    }

    #[test]
    fn normalize_manual_save_path_documents() {
        let _lock = crate::test_env::lock();
        let _guard = normalize_env();

        let result =
            normalize_manual_save_path(Path::new("C:\\Users\\TestUser\\Documents\\MyGame\\saves"));

        assert_eq!(result, "%USERPROFILE%\\Documents\\MyGame\\saves");
    }

    #[test]
    fn normalize_manual_save_path_other_userprofile_subpath() {
        let _lock = crate::test_env::lock();
        let _guard = normalize_env();

        let result =
            normalize_manual_save_path(Path::new("C:\\Users\\TestUser\\Saved Games\\SomeGame"));

        assert_eq!(result, "%USERPROFILE%\\Saved Games\\SomeGame");
    }

    #[test]
    fn normalize_manual_save_path_unrecognized_path_unchanged() {
        let _lock = crate::test_env::lock();
        let _guard = normalize_env();

        let result = normalize_manual_save_path(Path::new("D:\\GameSaves\\SomeGame"));

        assert_eq!(result, "D:\\GameSaves\\SomeGame");
    }

    #[test]
    fn normalize_manual_save_path_forward_slashes_normalized() {
        let _lock = crate::test_env::lock();
        let _guard = normalize_env();

        let result = normalize_manual_save_path(Path::new(
            "C:/Users/TestUser/AppData/Roaming/SomeGame/saves",
        ));

        assert_eq!(result, "%APPDATA%\\SomeGame\\saves");
    }

    /// Self-review completeness case: the raw path expands to *exactly*
    /// `%USERPROFILE%\Documents` with no suffix — the redirection splice's
    /// "exact match" arm (`expanded_cf == docs_prefix`) returns the
    /// Shell-resolved `windows_documents` path unchanged, not a joined
    /// path with a trailing separator.
    #[test]
    fn resolve_native_save_dir_exact_documents_match_returns_shell_path() {
        let _lock = crate::test_env::lock();
        let _guard = EnvGuard::set(&[("USERPROFILE", Some("C:\\Users\\TestUser"))]);
        let windows_documents = PathBuf::from("Y:\\Users\\TestUser\\Documents");

        let result =
            resolve_native_save_dir("%USERPROFILE%\\Documents", Some(&windows_documents), None);

        assert_eq!(result, windows_documents);
    }

    // -------------------------------------------------------------
    // WinePrefixPathTranslationTests (test_cloud_transfer.py:734) —
    // resolve_native_save_dir(..., wine_prefix=...)
    // -------------------------------------------------------------

    fn wine_env() -> EnvGuard {
        EnvGuard::set(&[("LOGNAME", Some("testuser"))])
    }

    #[test]
    fn resolve_native_save_dir_appdata_roaming_via_wine_prefix() {
        let _lock = crate::test_env::lock();
        let _guard = wine_env();
        let prefix = Path::new("/home/testuser/.wine");
        let user_home = prefix.join("drive_c").join("users").join("testuser");

        let result = resolve_native_save_dir("%APPDATA%\\Game\\saves", None, Some(prefix));

        assert_eq!(
            result,
            user_home
                .join("AppData")
                .join("Roaming")
                .join("Game")
                .join("saves")
        );
    }

    #[test]
    fn resolve_native_save_dir_localappdata_via_wine_prefix() {
        let _lock = crate::test_env::lock();
        let _guard = wine_env();
        let prefix = Path::new("/home/testuser/.wine");
        let user_home = prefix.join("drive_c").join("users").join("testuser");

        let result = resolve_native_save_dir("%LOCALAPPDATA%\\Game", None, Some(prefix));

        assert_eq!(result, user_home.join("AppData").join("Local").join("Game"));
    }

    #[test]
    fn resolve_native_save_dir_userprofile_documents_via_wine_prefix() {
        let _lock = crate::test_env::lock();
        let _guard = wine_env();
        let prefix = Path::new("/home/testuser/.wine");
        let user_home = prefix.join("drive_c").join("users").join("testuser");

        let result = resolve_native_save_dir("%USERPROFILE%\\Documents\\Game", None, Some(prefix));

        assert_eq!(result, user_home.join("Documents").join("Game"));
    }

    #[test]
    fn resolve_native_save_dir_userprofile_locallow_via_wine_prefix() {
        let _lock = crate::test_env::lock();
        let _guard = wine_env();
        let prefix = Path::new("/home/testuser/.wine");
        let user_home = prefix.join("drive_c").join("users").join("testuser");

        let result =
            resolve_native_save_dir("%USERPROFILE%\\AppData\\LocalLow\\Game", None, Some(prefix));

        assert_eq!(
            result,
            user_home.join("AppData").join("LocalLow").join("Game")
        );
    }

    #[test]
    fn resolve_native_save_dir_programdata_via_wine_prefix() {
        let _lock = crate::test_env::lock();
        let _guard = wine_env();
        let prefix = Path::new("/home/testuser/.wine");

        let result = resolve_native_save_dir("%PROGRAMDATA%\\Game", None, Some(prefix));

        assert_eq!(
            result,
            prefix.join("drive_c").join("ProgramData").join("Game")
        );
    }

    #[test]
    fn resolve_native_save_dir_no_wine_prefix_uses_percent_expansion() {
        let _lock = crate::test_env::lock();
        let _guard = EnvGuard::set(&[("APPDATA", Some("/expanded/AppData/Roaming"))]);

        let result = resolve_native_save_dir("%APPDATA%\\Game\\saves", None, None);

        assert_eq!(
            result,
            PathBuf::from("/expanded/AppData/Roaming\\Game\\saves")
        );
    }

    #[test]
    fn resolve_native_save_dir_unrecognized_var_falls_back_to_percent_expansion() {
        let _lock = crate::test_env::lock();
        let _guard = crate::test_env::EnvGuard::set(&[]);

        let result = resolve_native_save_dir(
            "%GAME_DIR%\\saves",
            None,
            Some(Path::new("/home/testuser/.wine")),
        );

        assert_eq!(result, PathBuf::from("%GAME_DIR%\\saves"));
    }

    // -------------------------------------------------------------
    // TranslateWindowsPathToWinePrefixTests (test_cloud_transfer.py:824)
    // -------------------------------------------------------------

    #[test]
    fn translate_appdata() {
        let _lock = crate::test_env::lock();
        let _guard = wine_env();
        let prefix = Path::new("/home/testuser/.wine");
        let user_home = prefix.join("drive_c").join("users").join("testuser");

        let result = translate_windows_path_to_wine_prefix("%APPDATA%\\Game", prefix);

        assert_eq!(
            result,
            Some(user_home.join("AppData").join("Roaming").join("Game"))
        );
    }

    #[test]
    fn translate_localappdata() {
        let _lock = crate::test_env::lock();
        let _guard = wine_env();
        let prefix = Path::new("/home/testuser/.wine");
        let user_home = prefix.join("drive_c").join("users").join("testuser");

        let result = translate_windows_path_to_wine_prefix("%LOCALAPPDATA%\\Game", prefix);

        assert_eq!(
            result,
            Some(user_home.join("AppData").join("Local").join("Game"))
        );
    }

    #[test]
    fn translate_userprofile_documents_multi_level() {
        let _lock = crate::test_env::lock();
        let _guard = wine_env();
        let prefix = Path::new("/home/testuser/.wine");
        let user_home = prefix.join("drive_c").join("users").join("testuser");

        let result = translate_windows_path_to_wine_prefix(
            "%USERPROFILE%\\Documents\\Square Enix\\Game",
            prefix,
        );

        assert_eq!(
            result,
            Some(user_home.join("Documents").join("Square Enix").join("Game"))
        );
    }

    #[test]
    fn translate_userprofile_locallow() {
        let _lock = crate::test_env::lock();
        let _guard = wine_env();
        let prefix = Path::new("/home/testuser/.wine");
        let user_home = prefix.join("drive_c").join("users").join("testuser");

        let result =
            translate_windows_path_to_wine_prefix("%USERPROFILE%\\AppData\\LocalLow\\Game", prefix);

        assert_eq!(
            result,
            Some(user_home.join("AppData").join("LocalLow").join("Game"))
        );
    }

    #[test]
    fn translate_userprofile_bare() {
        let _lock = crate::test_env::lock();
        let _guard = wine_env();
        let prefix = Path::new("/home/testuser/.wine");
        let user_home = prefix.join("drive_c").join("users").join("testuser");

        let result = translate_windows_path_to_wine_prefix("%USERPROFILE%", prefix);

        assert_eq!(result, Some(user_home));
    }

    #[test]
    fn translate_programdata() {
        let _lock = crate::test_env::lock();
        let _guard = wine_env();
        let prefix = Path::new("/home/testuser/.wine");

        let result = translate_windows_path_to_wine_prefix("%PROGRAMDATA%\\Game", prefix);

        assert_eq!(
            result,
            Some(prefix.join("drive_c").join("ProgramData").join("Game"))
        );
    }

    #[test]
    fn translate_windir() {
        let _lock = crate::test_env::lock();
        let _guard = wine_env();
        let prefix = Path::new("/home/testuser/.wine");

        let result = translate_windows_path_to_wine_prefix("%WINDIR%\\Fonts", prefix);

        assert_eq!(
            result,
            Some(prefix.join("drive_c").join("windows").join("Fonts"))
        );
    }

    #[test]
    fn translate_public() {
        let _lock = crate::test_env::lock();
        let _guard = wine_env();
        let prefix = Path::new("/home/testuser/.wine");

        let result = translate_windows_path_to_wine_prefix("%PUBLIC%\\Documents", prefix);

        assert_eq!(
            result,
            Some(
                prefix
                    .join("drive_c")
                    .join("users")
                    .join("Public")
                    .join("Documents")
            )
        );
    }

    #[test]
    fn translate_unrecognized_var_returns_none() {
        let _lock = crate::test_env::lock();
        let _guard = wine_env();
        let prefix = Path::new("/home/testuser/.wine");

        let result = translate_windows_path_to_wine_prefix("%GAME_DIR%\\saves", prefix);

        assert_eq!(result, None);
    }

    #[test]
    fn translate_case_insensitive_match() {
        let _lock = crate::test_env::lock();
        let _guard = wine_env();
        let prefix = Path::new("/home/testuser/.wine");
        let user_home = prefix.join("drive_c").join("users").join("testuser");

        let result = translate_windows_path_to_wine_prefix("%appdata%\\Game", prefix);

        assert_eq!(
            result,
            Some(user_home.join("AppData").join("Roaming").join("Game"))
        );
    }

    // -------------------------------------------------------------
    // restore_native_multi_dir_archive
    // -------------------------------------------------------------

    #[test]
    fn manifest_restore_resolves_indices_and_blocks_zip_slip() {
        let _lock = crate::test_env::lock();
        let _guard = crate::test_env::EnvGuard::set(&[]);
        let temp = tempfile::tempdir().unwrap();
        let dir0 = temp.path().join("dir0");
        let dir1 = temp.path().join("dir1");
        std::fs::create_dir_all(&dir0).unwrap();
        std::fs::create_dir_all(&dir1).unwrap();

        let manifest = serde_json::json!({
            "0": dir0.to_string_lossy(),
            "1": dir1.to_string_lossy(),
        });
        let manifest_bytes = serde_json::to_vec(&manifest).unwrap();

        let payload = build_zip_bytes(&[
            ("_grid_launcher_dirs.json", manifest_bytes.as_slice()),
            ("0/save.dat", b"save-zero"),
            ("1/sub/state.dat", b"save-one"),
            ("0/../../escape.txt", b"should not escape"),
        ]);

        let written = restore_native_multi_dir_archive(&payload, &[], None, None).unwrap();

        assert_eq!(written, 2);
        assert_eq!(std::fs::read(dir0.join("save.dat")).unwrap(), b"save-zero");
        assert_eq!(
            std::fs::read(dir1.join("sub").join("state.dat")).unwrap(),
            b"save-one"
        );
        assert!(!temp.path().parent().unwrap().join("escape.txt").exists());
    }

    #[test]
    fn manifest_restore_degrades_to_empty_manifest() {
        let _lock = crate::test_env::lock();
        let _guard = crate::test_env::EnvGuard::set(&[]);
        let temp = tempfile::tempdir().unwrap();
        let fallback = temp.path().join("fallback");
        std::fs::create_dir_all(&fallback).unwrap();

        // Malformed manifest bytes (not valid JSON) -> degrades to {}.
        let payload = build_zip_bytes(&[
            ("_grid_launcher_dirs.json", b"not json"),
            ("0/save.dat", b"content"),
        ]);

        let written =
            restore_native_multi_dir_archive(&payload, std::slice::from_ref(&fallback), None, None)
                .unwrap();

        assert_eq!(written, 1);
        assert_eq!(
            std::fs::read(fallback.join("save.dat")).unwrap(),
            b"content"
        );
    }

    /// A *lexical* `dest_root.join(relative)` + `starts_with` check would
    /// not notice a pre-existing symlink under the manifest-resolved root
    /// pointing outside it. Mirrors archive.rs's
    /// `extract_rejects_zip_slip_through_a_pre_existing_symlink`, proving
    /// the reused `resolve_under_root` guard catches it here too.
    #[test]
    #[cfg(unix)]
    fn manifest_restore_rejects_zip_slip_through_a_pre_existing_symlink() {
        let _lock = crate::test_env::lock();
        let _guard = crate::test_env::EnvGuard::set(&[]);
        let temp = tempfile::tempdir().unwrap();
        let dir0 = temp.path().join("dir0");
        std::fs::create_dir_all(&dir0).unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), dir0.join("link")).unwrap();

        let manifest = serde_json::json!({ "0": dir0.to_string_lossy() });
        let payload = build_zip_bytes(&[
            (
                "_grid_launcher_dirs.json",
                serde_json::to_vec(&manifest).unwrap().as_slice(),
            ),
            ("0/link/escape.txt", b"pwned"),
        ]);

        let written = restore_native_multi_dir_archive(&payload, &[], None, None).unwrap();

        assert_eq!(written, 0);
        assert!(!outside.path().join("escape.txt").exists());
    }

    /// The manifest's raw path resolves to a directory that does not yet
    /// exist on disk; per Python's `Path.resolve(strict=False)`, that must
    /// not prevent restore — parents (including the root itself) are
    /// created as needed.
    #[test]
    fn manifest_restore_creates_nonexistent_target_root() {
        let _lock = crate::test_env::lock();
        let _guard = crate::test_env::EnvGuard::set(&[]);
        let temp = tempfile::tempdir().unwrap();
        let target_root = temp.path().join("does").join("not").join("exist");
        assert!(!target_root.exists());

        let manifest = serde_json::json!({ "0": target_root.to_string_lossy() });
        let payload = build_zip_bytes(&[
            (
                "_grid_launcher_dirs.json",
                serde_json::to_vec(&manifest).unwrap().as_slice(),
            ),
            ("0/nested/save.dat", b"content"),
        ]);

        let written = restore_native_multi_dir_archive(&payload, &[], None, None).unwrap();

        assert_eq!(written, 1);
        assert_eq!(
            std::fs::read(target_root.join("nested").join("save.dat")).unwrap(),
            b"content"
        );
    }

    #[test]
    fn manifest_restore_missing_manifest_and_no_fallback_skips_member() {
        let _lock = crate::test_env::lock();
        let _guard = crate::test_env::EnvGuard::set(&[]);
        let payload = build_zip_bytes(&[("0/save.dat", b"content")]);

        let written = restore_native_multi_dir_archive(&payload, &[], None, None).unwrap();

        assert_eq!(written, 0);
    }

    #[test]
    fn manifest_restore_overwrites_existing_file() {
        let _lock = crate::test_env::lock();
        let _guard = crate::test_env::EnvGuard::set(&[]);
        let temp = tempfile::tempdir().unwrap();
        let fallback = temp.path().join("fallback");
        std::fs::create_dir_all(&fallback).unwrap();
        std::fs::write(fallback.join("save.dat"), b"old-content").unwrap();

        let payload = build_zip_bytes(&[("0/save.dat", b"new-content")]);

        let written =
            restore_native_multi_dir_archive(&payload, std::slice::from_ref(&fallback), None, None)
                .unwrap();

        assert_eq!(written, 1);
        assert_eq!(
            std::fs::read(fallback.join("save.dat")).unwrap(),
            b"new-content"
        );
    }

    // -------------------------------------------------------------
    // native_save_paths
    // -------------------------------------------------------------

    #[test]
    fn native_save_paths_dedupes_manual_against_pcgw() {
        let pcgw = vec![
            "%APPDATA%\\Game\\saves".to_string(),
            "%DOCUMENTS%\\Game".to_string(),
        ];
        let manual = vec![
            "%APPDATA%\\Game\\saves".to_string(), // already in pcgw, dropped
            "D:\\Custom\\saves".to_string(),      // kept
        ];

        let result = native_save_paths(&pcgw, &manual);

        assert_eq!(
            result,
            vec![
                "%APPDATA%\\Game\\saves".to_string(),
                "%DOCUMENTS%\\Game".to_string(),
                "D:\\Custom\\saves".to_string(),
            ]
        );
    }
}
