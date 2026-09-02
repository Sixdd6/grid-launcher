//! PPSSPP's `PPSSPP.INI` overwrite writer, the `installed.txt` pre-sync
//! deletion, and the RetroAchievements token file.
//!
//! Ports `grid_launcher/emulator/ppsspp.py`'s `ensure_ppsspp_settings`
//! (ppsspp.py:75-166). See `docs/porting/05-emulator-autoconfig.md`
//! ("PPSSPP") for the behavior contract.
//!
//! Spec deviation D5 (binding): ppsspp.py:99 and ppsspp.py:156 each read a
//! file with no `try`/`except` around them at all — an unreadable existing
//! `PPSSPP.INI` or `.dat` file crashes the whole Python call. Both reads are
//! wrapped here exactly like every other writer's I/O: an unreadable file
//! is treated the same as any other write failure, `changed` stays
//! whatever it already was, and nothing propagates. See [`read_guarded`].
//!
//! Spec deviation D2 (RA-keys-only fan-out) — no direct Python counterpart:
//! [`ensure_ra_credentials`] is a narrow writer for just the `[Achievements]`
//! section plus the token file, mirroring `retroarch::ensure_ra_credentials`
//! and `pcsx2::ensure_ra_credentials`. It must never delete `installed.txt`
//! and must never touch `[General]`/`[Graphics]`/`[Sound]`/`[Theme]`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::{paths, writers, EnsureResult, RaCredentials};

/// `<emulator_dir>/memstick/PSP/SYSTEM/PPSSPP.INI` — exact casing, no
/// platform candidates (ppsspp.py:97).
fn ini_path(emulator_dir: &Path) -> PathBuf {
    emulator_dir
        .join("memstick")
        .join("PSP")
        .join("SYSTEM")
        .join("PPSSPP.INI")
}

/// `path.trim()` expanded, dir-or-parent (ppsspp.py:82-84) — `None` for a
/// blank path, with **no existence check on the path itself** (unlike most
/// sibling modules, this one never checks `expanded.is_file()`).
fn resolve_emulator_dir(emulator_path: &str) -> Option<PathBuf> {
    let trimmed = emulator_path.trim();
    if trimmed.is_empty() {
        return None;
    }
    let expanded = paths::expand_user(trimmed);
    Some(paths::emulator_dir(&expanded).unwrap_or_default())
}

/// Delete `<emulator_dir>/installed.txt` when present (ppsspp.py:87-92) —
/// this is what suppresses PPSSPP's first-run installer flow. Returns
/// whether the delete happened; any I/O error is swallowed and does NOT
/// count as a change, matching the reference's `except OSError: pass`.
fn delete_installed_txt(emulator_dir: &Path) -> bool {
    let marker = emulator_dir.join("installed.txt");
    marker.exists() && std::fs::remove_file(&marker).is_ok()
}

/// D5-guarded read: `Some(String::new())` when `path` does not exist,
/// `Some(content)` when it exists and is readable, `None` when it exists
/// but cannot be read — the case ppsspp.py:99 and ppsspp.py:156 crash on.
fn read_guarded(path: &Path) -> Option<String> {
    if !path.exists() {
        return Some(String::new());
    }
    std::fs::read_to_string(path).ok()
}

/// `create_dir_all` on the parent, then write `content` — ppsspp.py:149-152
/// and ppsspp.py:159-162 each wrap this pair in their own
/// `except OSError: pass`.
fn write_text_creating_parent(path: &Path, content: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)
}

/// The four unconditional base sections, in the pinned order
/// (ppsspp.py:100-124).
fn base_sections() -> Vec<(&'static str, writers::Desired)> {
    vec![
        (
            "General",
            crate::desired![("CheckForNewVersion", "False"), ("SaveStateSlotCount", "3")],
        ),
        (
            "Graphics",
            crate::desired![
                ("InternalResolution", "4"),
                ("MultiSampleLevel", "2"),
                ("Smart2DTexFiltering", "True"),
                ("TexScalingLevel", "4"),
                ("TexScalingType", "0"),
                ("TexDeposterize", "True"),
                ("TexHardwareScaling", "False"),
                ("TextureShader", "Off"),
                ("HardwareTessellation", "False"),
            ],
        ),
        (
            "Sound",
            crate::desired![("GameVolume", "25"), ("AchievementVolume", "40")],
        ),
        ("Theme", crate::desired![("ThemeName", "Slate Forest")]),
    ]
}

/// `[Achievements]`, in the pinned order (ppsspp.py:127-141) — five
/// notification-position keys at `3`, the last (`AchievementsUnlockedPos`)
/// at `4`. The caller has already confirmed both `ra_user` and `ra_token`
/// are non-blank.
fn achievements_section(ra_user: &str, ra_token: &str) -> writers::Desired {
    crate::desired![
        ("AchievementsEnable", "True"),
        ("AchievementsUserName", ra_user),
        ("AchievementsToken", ra_token),
        ("AchievementsChallengeMode", "False"),
        ("AchievementsLeaderboardTrackerPos", "3"),
        ("AchievementsLeaderboardStartedOrFailedPos", "3"),
        ("AchievementsLeaderboardSubmittedPos", "3"),
        ("AchievementsProgressPos", "3"),
        ("AchievementsChallengePos", "3"),
        ("AchievementsUnlockedPos", "4"),
    ]
}

/// Runs every `(section, desired)` pair through
/// [`writers::ini_overwrite_section`] in order, writing the INI back only
/// when something changed. `Some(changed)` when the pre-existing INI could
/// be read (`changed` may still be `false` — a write failure is swallowed,
/// ppsspp.py:149-152); `None` when the pre-existing INI could not be read
/// at all (D5).
fn write_ini_sections(ini_path: &Path, sections: &[(&str, writers::Desired)]) -> Option<bool> {
    let mut content = read_guarded(ini_path)?;
    let mut any_changed = false;
    for (section, desired) in sections {
        let (new_content, section_changed) =
            writers::ini_overwrite_section(&content, section, desired);
        content = new_content;
        any_changed = any_changed || section_changed;
    }
    if any_changed && write_text_creating_parent(ini_path, &content).is_ok() {
        return Some(true);
    }
    Some(false)
}

/// Writes the RA token `.dat` file when its trimmed contents differ from
/// `token` (ppsspp.py:155-162), D5-guarded. Content is the bare trimmed
/// token with **no trailing newline**. Returns whether the write happened;
/// a pre-existing unreadable `.dat` or a write failure is swallowed.
fn write_ra_token_dat(dat_path: &Path, token: &str) -> bool {
    let Some(existing) = read_guarded(dat_path) else {
        return false;
    };
    if existing.trim() == token {
        return false;
    }
    write_text_creating_parent(dat_path, token).is_ok()
}

/// `ensure_ppsspp_settings` (ppsspp.py:75-166). Blank path (after `.trim()`)
/// returns [`EnsureResult::unchanged`].
///
/// Order of operations: delete `installed.txt` (ppsspp.py:87), then
/// overwrite the four base sections plus, only when BOTH `ra` fields are
/// non-blank after trimming, an `[Achievements]` section, then — again only
/// with both RA fields non-blank — the `ppsspp_retroachievements.dat` token
/// file. `config_path` is always the INI path;
/// `extras["ra_token_path"]` is set whenever both RA fields are present,
/// independent of whether either write actually happened.
pub fn ensure_settings(emulator_path: &str, ra: Option<&RaCredentials>) -> EnsureResult {
    let Some(emulator_dir) = resolve_emulator_dir(emulator_path) else {
        return EnsureResult::unchanged();
    };

    let mut changed = delete_installed_txt(&emulator_dir);
    let ini_path = ini_path(&emulator_dir);

    let (ra_user, ra_token) = ra
        .map(|creds| {
            (
                creds.username().trim().to_string(),
                creds.token().trim().to_string(),
            )
        })
        .unwrap_or_default();
    let has_ra = !ra_user.is_empty() && !ra_token.is_empty();

    let mut sections = base_sections();
    if has_ra {
        sections.push(("Achievements", achievements_section(&ra_user, &ra_token)));
    }

    if let Some(ini_changed) = write_ini_sections(&ini_path, &sections) {
        changed |= ini_changed;
    }

    let mut extras = BTreeMap::new();
    if has_ra {
        let dat_path = ini_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| emulator_dir.clone())
            .join("ppsspp_retroachievements.dat");
        changed |= write_ra_token_dat(&dat_path, &ra_token);
        extras.insert("ra_token_path".to_string(), dat_path);
    }

    EnsureResult {
        changed,
        config_path: Some(ini_path),
        extras,
    }
}

/// Spec deviation D2: the `[Achievements]` block plus the `.dat` file only —
/// never `installed.txt`, never `[General]`/`[Graphics]`/`[Sound]`/`[Theme]`.
/// A no-op ([`EnsureResult::unchanged`]) when either RA field is blank after
/// trimming, checked before any path resolution, and again for a blank
/// `emulator_path`.
pub fn ensure_ra_credentials(emulator_path: &str, ra: &RaCredentials) -> EnsureResult {
    let ra_user = ra.username().trim().to_string();
    let ra_token = ra.token().trim().to_string();
    if ra_user.is_empty() || ra_token.is_empty() {
        return EnsureResult::unchanged();
    }

    let Some(emulator_dir) = resolve_emulator_dir(emulator_path) else {
        return EnsureResult::unchanged();
    };

    let ini_path = ini_path(&emulator_dir);
    let mut changed = false;

    let sections = vec![("Achievements", achievements_section(&ra_user, &ra_token))];
    if let Some(ini_changed) = write_ini_sections(&ini_path, &sections) {
        changed |= ini_changed;
    }

    let dat_path = ini_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| emulator_dir.clone())
        .join("ppsspp_retroachievements.dat");
    changed |= write_ra_token_dat(&dat_path, &ra_token);

    EnsureResult::at(ini_path, changed).with_extra("ra_token_path", dat_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_exe(temp: &Path) -> (PathBuf, PathBuf) {
        let dir = temp.join("PPSSPP");
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("PPSSPPWindows64.exe");
        std::fs::write(&exe, b"").unwrap();
        (exe, dir)
    }

    fn ini_path_for(dir: &Path) -> PathBuf {
        dir.join("memstick")
            .join("PSP")
            .join("SYSTEM")
            .join("PPSSPP.INI")
    }

    fn dat_path_for(dir: &Path) -> PathBuf {
        dir.join("memstick")
            .join("PSP")
            .join("SYSTEM")
            .join("ppsspp_retroachievements.dat")
    }

    #[test]
    fn ppsspp_deletes_installed_txt_and_reports_changed() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, dir) = make_exe(temp.path());
        std::fs::write(dir.join("installed.txt"), "marker").unwrap();

        let result = ensure_settings(exe.to_str().unwrap(), None);

        assert!(result.changed);
        assert!(!dir.join("installed.txt").exists());
    }

    #[test]
    fn ppsspp_writes_the_ini_at_the_memstick_path_with_exact_casing() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, dir) = make_exe(temp.path());

        let result = ensure_settings(exe.to_str().unwrap(), None);

        assert_eq!(result.config_path, Some(ini_path_for(&dir)));
        assert!(ini_path_for(&dir).is_file());
    }

    #[test]
    fn ppsspp_writes_all_four_base_sections() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, dir) = make_exe(temp.path());

        ensure_settings(exe.to_str().unwrap(), None);

        let text = std::fs::read_to_string(ini_path_for(&dir)).unwrap();
        assert!(text.contains("[General]"));
        assert!(text.contains("CheckForNewVersion = False"));
        assert!(text.contains("SaveStateSlotCount = 3"));
        assert!(text.contains("[Graphics]"));
        assert!(text.contains("InternalResolution = 4"));
        assert!(text.contains("MultiSampleLevel = 2"));
        assert!(text.contains("Smart2DTexFiltering = True"));
        assert!(text.contains("TexScalingLevel = 4"));
        assert!(text.contains("TexScalingType = 0"));
        assert!(text.contains("TexDeposterize = True"));
        assert!(text.contains("TexHardwareScaling = False"));
        assert!(text.contains("TextureShader = Off"));
        assert!(text.contains("HardwareTessellation = False"));
        assert!(text.contains("[Sound]"));
        assert!(text.contains("GameVolume = 25"));
        assert!(text.contains("AchievementVolume = 40"));
        assert!(text.contains("[Theme]"));
        assert!(text.contains("ThemeName = Slate Forest"));
        assert!(!text.contains("[Achievements]"));
    }

    #[test]
    fn ppsspp_achievements_block_requires_both_fields() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, dir) = make_exe(temp.path());
        let username_only = RaCredentials::new("psp_user", "");
        ensure_settings(exe.to_str().unwrap(), Some(&username_only));
        let text = std::fs::read_to_string(ini_path_for(&dir)).unwrap();
        assert!(!text.contains("[Achievements]"));

        std::fs::remove_dir_all(&dir).unwrap();
        let (exe, dir) = make_exe(temp.path());
        let token_only = RaCredentials::new("", "psp_tok");
        ensure_settings(exe.to_str().unwrap(), Some(&token_only));
        let text = std::fs::read_to_string(ini_path_for(&dir)).unwrap();
        assert!(!text.contains("[Achievements]"));
    }

    #[test]
    fn ppsspp_writes_the_ra_token_dat_without_a_trailing_newline() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, dir) = make_exe(temp.path());
        let ra = RaCredentials::new("psp_user", "psp_tok");

        let result = ensure_settings(exe.to_str().unwrap(), Some(&ra));

        assert!(result.changed);
        let text = std::fs::read_to_string(ini_path_for(&dir)).unwrap();
        assert!(text.contains("[Achievements]"));
        assert!(text.contains("AchievementsEnable = True"));
        assert!(text.contains("AchievementsUserName = psp_user"));
        assert!(text.contains("AchievementsToken = psp_tok"));
        assert!(text.contains("AchievementsChallengeMode = False"));
        assert!(text.contains("AchievementsLeaderboardTrackerPos = 3"));
        assert!(text.contains("AchievementsLeaderboardStartedOrFailedPos = 3"));
        assert!(text.contains("AchievementsLeaderboardSubmittedPos = 3"));
        assert!(text.contains("AchievementsProgressPos = 3"));
        assert!(text.contains("AchievementsChallengePos = 3"));
        assert!(text.contains("AchievementsUnlockedPos = 4"));

        let dat_path = result.extras.get("ra_token_path").cloned().unwrap();
        assert_eq!(dat_path, dat_path_for(&dir));
        let dat_bytes = std::fs::read(&dat_path).unwrap();
        assert_eq!(dat_bytes, b"psp_tok");
    }

    #[test]
    fn ppsspp_skips_the_dat_write_when_the_token_is_unchanged() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, dir) = make_exe(temp.path());
        let ra = RaCredentials::new("psp_user", "psp_tok");

        // First call establishes the INI and the dat file.
        ensure_settings(exe.to_str().unwrap(), Some(&ra));
        let dat_path = dat_path_for(&dir);
        let before = std::fs::metadata(&dat_path).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));

        let result = ensure_settings(exe.to_str().unwrap(), Some(&ra));

        assert!(!result.changed, "a second identical call is a no-op");
        let after = std::fs::metadata(&dat_path).unwrap().modified().unwrap();
        assert_eq!(before, after, "the dat file must not be rewritten");
    }

    #[cfg(unix)]
    #[test]
    fn ppsspp_unreadable_ini_yields_changed_false() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().unwrap();
        let (exe, dir) = make_exe(temp.path());
        let ini = ini_path_for(&dir);
        std::fs::create_dir_all(ini.parent().unwrap()).unwrap();
        std::fs::write(&ini, "[General]\nCheckForNewVersion = True\n").unwrap();
        std::fs::set_permissions(&ini, std::fs::Permissions::from_mode(0o000)).unwrap();

        let result = std::panic::catch_unwind(|| ensure_settings(exe.to_str().unwrap(), None));

        // Always restore permissions before the tempdir is dropped, panic or not.
        std::fs::set_permissions(&ini, std::fs::Permissions::from_mode(0o644)).unwrap();

        let result = result.expect("must not panic on an unreadable ini");
        assert!(!result.changed);
    }

    #[test]
    fn ppsspp_is_idempotent() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, _dir) = make_exe(temp.path());
        let ra = RaCredentials::new("psp_user", "psp_tok");

        ensure_settings(exe.to_str().unwrap(), Some(&ra));
        let second = ensure_settings(exe.to_str().unwrap(), Some(&ra));

        assert!(!second.changed, "a second identical call must be a no-op");
    }

    #[test]
    fn ppsspp_ensure_ra_credentials_does_not_delete_installed_txt() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, dir) = make_exe(temp.path());
        std::fs::write(dir.join("installed.txt"), "marker").unwrap();
        let ra = RaCredentials::new("psp_user", "psp_tok");

        let result = ensure_ra_credentials(exe.to_str().unwrap(), &ra);

        assert!(result.changed);
        assert!(
            dir.join("installed.txt").exists(),
            "ensure_ra_credentials must never delete installed.txt"
        );
        let text = std::fs::read_to_string(ini_path_for(&dir)).unwrap();
        assert!(text.contains("[Achievements]"));
        assert!(!text.contains("[General]"));
        assert!(!text.contains("[Graphics]"));
        assert!(!text.contains("[Sound]"));
        assert!(!text.contains("[Theme]"));
    }

    #[test]
    fn ppsspp_no_change_for_empty_path() {
        let result = ensure_settings("", None);
        assert!(!result.changed);
    }
}
