//! Registering a server "Emulators"-platform package as an emulator entry.
//!
//! A ROM on the server's Emulators platform is an emulator build. It is
//! downloaded and extracted like any other game (under
//! `games/Emulators/<stem>`, with its hidden registry row), and then
//! registered as an emulator so it shows under Emulators › Installed and is
//! configured like a catalog install. Ports `_auto_configure_installed_emulator`
//! (grid-launcher.py:3622-3664) and the parts of
//! `auto_configure_emulator_settings` (emulator/autoconfig.py:472-575) that
//! decide the entry; the platform defaults and config writers run through
//! [`crate::autoconfig::sync_new_emulator`], as for a catalog install.
//!
//! DEVIATION (user ruling Q3/U4, 2026-10-10): the reference overwrote any
//! entry with the same name. Here a name that an entry the package does not
//! own already holds gets the suffix [`SERVER_SUFFIX`], so a catalog entry
//! (and its update source) is never replaced.

use std::path::{Path, PathBuf};

use crate::config::{Config, EmulatorEntry};
use crate::launch::profiles::{profile_for_package, EmulatorProfile};

/// Appended to the entry name when the plain name is taken.
pub const SERVER_SUFFIX: &str = " (server)";

/// The reference's fallback name when the ROM title is blank
/// (profiles.py:305).
const FALLBACK_NAME: &str = "Emulator";

/// What [`plan_server_emulator`] decided for one package.
#[derive(Debug, Clone, PartialEq)]
pub struct ServerEmulatorPlan<'a> {
    /// The config entry name to create or update.
    pub name: String,
    /// The args a NEW entry gets: the profile's, or `%rom%`.
    pub args: String,
    /// The matched bundled profile, if any.
    pub profile: Option<&'a EmulatorProfile>,
}

/// Decides the entry for a package whose selected executable is `exe`.
///
/// The profile comes from [`profile_for_package`]; with no match the name is
/// the ROM title and the args are `%rom%` (the reference's fallback
/// profile). The name is then resolved against `existing`:
///
/// 1. An entry this package already OWNS (its path is `exe`, or lies inside
///    `package_dir`) named `<Name>` or `<Name> (server)` keeps its name —
///    this is a reinstall, and the entry is updated in place.
/// 2. Else, when no entry is named `<Name>`, the package takes `<Name>`.
/// 3. Else it takes `<Name> (server)`.
///
/// Names compare trimmed and case-insensitively. A catalog entry (non-blank
/// `source_id`) is never owned, so rule 1 cannot select it.
pub fn plan_server_emulator<'a>(
    title: &str,
    exe: &Path,
    package_dir: Option<&Path>,
    existing: &[EmulatorEntry],
    profiles: &'a [EmulatorProfile],
) -> ServerEmulatorPlan<'a> {
    let exe_text = path_text(exe);
    let profile = profile_for_package(title, &exe_text, profiles);
    let (base, args) = match profile {
        Some(profile) => (profile.name.trim().to_string(), profile.args.clone()),
        None => {
            let title = title.trim();
            let name = if title.is_empty() {
                FALLBACK_NAME
            } else {
                title
            };
            (name.to_string(), "%rom%".to_string())
        }
    };
    let suffixed = format!("{base}{SERVER_SUFFIX}");

    let exe_key = path_key(&exe_text);
    let dir_key = package_dir.map(|dir| path_key(&path_text(dir)));
    let owned = |entry: &EmulatorEntry| {
        if !entry.source_id.trim().is_empty() || entry.path.trim().is_empty() {
            return false;
        }
        let key = path_key(&entry.path);
        key == exe_key || dir_key.as_ref().is_some_and(|dir| key.starts_with(dir))
    };
    let named =
        |entry: &EmulatorEntry, name: &str| entry.name.trim().to_lowercase() == name.to_lowercase();

    let name = [base.as_str(), suffixed.as_str()]
        .iter()
        .find_map(|candidate| {
            existing
                .iter()
                .find(|entry| named(entry, candidate) && owned(entry))
                .map(|entry| entry.name.clone())
        })
        .or_else(|| {
            existing
                .iter()
                .all(|entry| !named(entry, &base))
                .then(|| base.clone())
        })
        .unwrap_or_else(|| {
            // An existing entry may spell the suffixed name in another case;
            // reuse its exact spelling so the write updates it.
            existing
                .iter()
                .find(|entry| named(entry, &suffixed))
                .map(|entry| entry.name.clone())
                .unwrap_or(suffixed.clone())
        });

    ServerEmulatorPlan {
        name,
        args,
        profile,
    }
}

/// Writes `plan`'s entry into `config` with `exe` as its path. Returns
/// `true` when the entry is NEW.
///
/// An existing entry with exactly `plan.name` keeps everything the user
/// owns — args, save strategy, save and state paths, ignore lists — and only
/// its path changes. No `source_*` field is ever written: a server package
/// has no forge update source.
pub fn write_server_emulator_entry(
    config: &mut Config,
    plan: &ServerEmulatorPlan<'_>,
    exe: &Path,
) -> bool {
    let path = path_text(exe);
    match config
        .emulators
        .iter_mut()
        .find(|existing| existing.name == plan.name)
    {
        Some(existing) => {
            existing.path = path;
            false
        }
        None => {
            config.emulators.push(EmulatorEntry {
                name: plan.name.clone(),
                path,
                args: plan.args.clone(),
                ..Default::default()
            });
            true
        }
    }
}

/// `path` as the string a config entry stores.
fn path_text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// A comparable form of a path string: `~` expanded, and on Windows
/// case-folded with `/` read as `\`.
pub(crate) fn path_key(raw: &str) -> PathBuf {
    let expanded = crate::library::paths::expand_home(raw.trim());
    if cfg!(windows) {
        PathBuf::from(expanded.to_string_lossy().replace('/', "\\").to_lowercase())
    } else {
        expanded
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(name: &str, tokens: &[&str], args: &str) -> EmulatorProfile {
        EmulatorProfile {
            name: name.to_string(),
            match_tokens: tokens.iter().map(|t| t.to_lowercase()).collect(),
            args: args.to_string(),
            ..Default::default()
        }
    }

    fn entry(name: &str, path: &str) -> EmulatorEntry {
        EmulatorEntry {
            name: name.to_string(),
            path: path.to_string(),
            args: "%rom%".to_string(),
            ..Default::default()
        }
    }

    fn catalog_entry(name: &str, path: &str) -> EmulatorEntry {
        EmulatorEntry {
            source_id: "owner/repo".to_string(),
            source_release_tag: "latest".to_string(),
            ..entry(name, path)
        }
    }

    fn pkg() -> (PathBuf, PathBuf) {
        let dir = PathBuf::from("/lib/games/Emulators/pcsx2-pkg");
        let exe = dir.join("pcsx2-qt.exe");
        (dir, exe)
    }

    fn profiles() -> Vec<EmulatorProfile> {
        vec![profile(
            "PCSX2 (Playstation 2)",
            &["pcsx2-qt.exe"],
            "-batch \"%rom%\"",
        )]
    }

    #[test]
    fn a_matched_profile_names_the_entry_and_gives_its_args() {
        let (dir, exe) = pkg();
        let profiles = profiles();
        let plan = plan_server_emulator("PCSX2 Nightly", &exe, Some(&dir), &[], &profiles);
        assert_eq!(plan.name, "PCSX2 (Playstation 2)");
        assert_eq!(plan.args, "-batch \"%rom%\"");
        assert_eq!(
            plan.profile.map(|p| p.name.as_str()),
            Some("PCSX2 (Playstation 2)")
        );
    }

    #[test]
    fn no_profile_falls_back_to_the_rom_title_and_rom_args() {
        let dir = PathBuf::from("/lib/games/Emulators/odd");
        let exe = dir.join("odd-emu.exe");
        let profiles = profiles();
        let plan = plan_server_emulator("  Odd Emu  ", &exe, Some(&dir), &[], &profiles);
        assert_eq!(plan.name, "Odd Emu");
        assert_eq!(plan.args, "%rom%");
        assert!(plan.profile.is_none());

        let blank = plan_server_emulator("   ", &exe, Some(&dir), &[], &profiles);
        assert_eq!(blank.name, "Emulator");
    }

    #[test]
    fn a_name_held_by_another_entry_gets_the_server_suffix() {
        let (dir, exe) = pkg();
        let profiles = profiles();
        let existing = vec![catalog_entry(
            "PCSX2 (Playstation 2)",
            "/lib/emulators/PCSX2 (Playstation 2)/pcsx2.AppImage",
        )];
        let plan = plan_server_emulator("PCSX2", &exe, Some(&dir), &existing, &profiles);
        assert_eq!(plan.name, "PCSX2 (Playstation 2) (server)");

        // A hand-added entry with the name (any case) clashes too.
        let existing = vec![entry("pcsx2 (playstation 2)", "/usr/bin/pcsx2")];
        let plan = plan_server_emulator("PCSX2", &exe, Some(&dir), &existing, &profiles);
        assert_eq!(plan.name, "PCSX2 (Playstation 2) (server)");
    }

    #[test]
    fn a_reinstall_keeps_the_entry_the_package_already_owns() {
        let (dir, exe) = pkg();
        let profiles = profiles();
        // The package registered under the plain name before; its new
        // executable sits in the same package directory.
        let existing = vec![entry(
            "PCSX2 (Playstation 2)",
            &path_text(&dir.join("old").join("pcsx2-qt.exe")),
        )];
        let plan = plan_server_emulator("PCSX2", &exe, Some(&dir), &existing, &profiles);
        assert_eq!(plan.name, "PCSX2 (Playstation 2)");

        // Registered with the suffix before (the catalog entry still exists).
        let existing = vec![
            catalog_entry(
                "PCSX2 (Playstation 2)",
                "/lib/emulators/PCSX2 (Playstation 2)/pcsx2.AppImage",
            ),
            entry("PCSX2 (Playstation 2) (server)", &path_text(&exe)),
        ];
        let plan = plan_server_emulator("PCSX2", &exe, Some(&dir), &existing, &profiles);
        assert_eq!(plan.name, "PCSX2 (Playstation 2) (server)");

        // The catalog entry was deleted since: the owned suffixed entry is
        // still the one updated, not a second plain-named one.
        let existing = vec![entry("PCSX2 (Playstation 2) (server)", &path_text(&exe))];
        let plan = plan_server_emulator("PCSX2", &exe, Some(&dir), &existing, &profiles);
        assert_eq!(plan.name, "PCSX2 (Playstation 2) (server)");
    }

    #[test]
    fn a_catalog_entry_is_never_owned_even_inside_the_package_dir() {
        let (dir, exe) = pkg();
        let profiles = profiles();
        let existing = vec![catalog_entry("PCSX2 (Playstation 2)", &path_text(&exe))];
        let plan = plan_server_emulator("PCSX2", &exe, Some(&dir), &existing, &profiles);
        assert_eq!(plan.name, "PCSX2 (Playstation 2) (server)");
    }

    #[test]
    fn without_a_package_dir_only_the_exact_path_is_owned() {
        let dir = PathBuf::from("/lib/games/Emulators");
        let exe = dir.join("pcsx2-qt.exe");
        let profiles = profiles();
        let sibling = vec![entry(
            "PCSX2 (Playstation 2)",
            &path_text(&dir.join("other.exe")),
        )];
        let plan = plan_server_emulator("PCSX2", &exe, None, &sibling, &profiles);
        assert_eq!(plan.name, "PCSX2 (Playstation 2) (server)");

        let same = vec![entry("PCSX2 (Playstation 2)", &path_text(&exe))];
        let plan = plan_server_emulator("PCSX2", &exe, None, &same, &profiles);
        assert_eq!(plan.name, "PCSX2 (Playstation 2)");
    }

    #[test]
    fn writing_a_new_entry_uses_the_plan_args_and_no_source() {
        let (_dir, exe) = pkg();
        let profiles = profiles();
        let mut config = Config::default();
        let plan = ServerEmulatorPlan {
            name: "PCSX2 (Playstation 2) (server)".to_string(),
            args: "-batch \"%rom%\"".to_string(),
            profile: profiles.first(),
        };
        assert!(write_server_emulator_entry(&mut config, &plan, &exe));
        assert_eq!(config.emulators.len(), 1);
        let written = &config.emulators[0];
        assert_eq!(written.name, plan.name);
        assert_eq!(written.path, path_text(&exe));
        assert_eq!(written.args, "-batch \"%rom%\"");
        assert_eq!(written.source_id, "");
        assert_eq!(written.source_release_tag, "");
    }

    #[test]
    fn updating_an_entry_keeps_user_fields_and_moves_only_the_path() {
        let (dir, exe) = pkg();
        let mut config = Config::default();
        let catalog = catalog_entry("Other", "/x/other.exe");
        let mine = EmulatorEntry {
            name: "Odd Emu".to_string(),
            path: path_text(&dir.join("old.exe")),
            args: "--my-flag %rom%".to_string(),
            save_strategy: "folder".to_string(),
            save_paths: "saves".to_string(),
            ..Default::default()
        };
        config.emulators = vec![catalog.clone(), mine.clone()];
        let plan = ServerEmulatorPlan {
            name: "Odd Emu".to_string(),
            args: "%rom%".to_string(),
            profile: None,
        };
        assert!(!write_server_emulator_entry(&mut config, &plan, &exe));
        assert_eq!(config.emulators.len(), 2);
        assert_eq!(config.emulators[0], catalog, "other entries are untouched");
        let updated = &config.emulators[1];
        assert_eq!(updated.path, path_text(&exe));
        assert_eq!(updated.args, "--my-flag %rom%");
        assert_eq!(updated.save_strategy, "folder");
        assert_eq!(updated.save_paths, "saves");
    }
}
