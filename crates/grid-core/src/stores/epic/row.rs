//! The registry row of an installed Epic game, built from its legendary
//! `list-installed` entry, its [`EpicMeta`] and (when GRID has it) the
//! owned list's details. Every Epic install path writes its row through
//! [`epic_row`], so they all launch alike.
//!
//! The row takes the native launch branch (`launch::native`): it sets
//! `extracted_dir` and `native_game_dir` (the install folder), the
//! executable legendary names as an absolute path under it, the launch
//! parameters legendary keeps, a working folder (the executable's own, as
//! legendary launches it) and the `win` platform slug: Epic has no Linux
//! builds, so every Epic game runs through the compat tool off Windows.

use std::path::Path;

use super::meta::EpicMeta;
use super::{EpicGame, EpicInstalled};
use crate::library::registry::InstalledGame;
use crate::library::specials::native::{LAUNCH_SOURCE_SIDECAR, LAUNCH_SOURCE_USER};
use crate::stores::gog::row::relative_path;
use crate::stores::{GameSource, StoreKind};

/// The row for the Epic game `inst` describes.
///
/// The title is the owned list's when `game` is given, else legendary's.
/// The executable is `install_path` + legendary's `executable` (either
/// separator); one that is absolute, has a drive letter or climbs out
/// (`..`) is not used, and Game Settings asks the user for one.
/// `native_working_dir` is the executable's folder, relative with `/`.
/// `wineprefix` is `<install dir>/prefix` off Windows, `None` on Windows.
///
/// `meta.exe_user_pinned` marks the launch source `user`; the caller then
/// carries the user's executable over from the old row. `installed_at` is
/// now; `filesize_bytes` is legendary's `install_size`.
pub fn epic_row(
    inst: &EpicInstalled,
    game: Option<&EpicGame>,
    meta: &EpicMeta,
    wineprefix: Option<&Path>,
) -> InstalledGame {
    let install_dir = Path::new(inst.install_path.trim());
    let dir = install_dir.to_string_lossy().into_owned();
    let rel = relative_path(&inst.executable);
    let executable = rel
        .as_ref()
        .map(|rel| install_dir.join(rel).to_string_lossy().into_owned())
        .unwrap_or_default();
    let working_dir = rel
        .as_ref()
        .and_then(|rel| rel.parent())
        .map(|parent| {
            parent
                .components()
                .map(|part| part.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/")
        })
        .unwrap_or_default();
    let title = game
        .map(|game| game.title.trim())
        .filter(|title| !title.is_empty())
        .unwrap_or(inst.title.trim())
        .to_string();
    let launch_source = if meta.exe_user_pinned {
        LAUNCH_SOURCE_USER
    } else {
        LAUNCH_SOURCE_SIDECAR
    };

    let mut row = InstalledGame {
        title,
        platform: StoreKind::Epic.platform_name().to_string(),
        rom_id: None,
        extracted_dir: dir.clone(),
        native_game_dir: dir,
        native_executable_path: executable,
        native_launch_parameters: inst.launch_parameters.trim().to_string(),
        native_working_dir: working_dir,
        native_launch_source: launch_source.to_string(),
        native_wineprefix: wineprefix
            .map(|prefix| prefix.to_string_lossy().into_owned())
            .unwrap_or_default(),
        platform_slug: "win".to_string(),
        revision: meta.version.clone(),
        filesize_bytes: i64::try_from(inst.install_size).unwrap_or(i64::MAX),
        installed_at: chrono::Utc::now().timestamp(),
        source: GameSource::Store(StoreKind::Epic),
        store_id: inst.app_name.trim().to_string(),
        store_meta: meta.to_json(),
        ..Default::default()
    };
    if let Some(game) = game {
        row.description = game.description.clone();
        row.companies = game.developer.trim().to_string();
        row.first_release_date = game.year.clone();
        row.screenshot_urls = game.screenshots.join("\n");
    }
    row
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn installed() -> EpicInstalled {
        EpicInstalled {
            app_name: "Starlight".into(),
            title: "starlight courier".into(),
            version: "1.0.2".into(),
            install_path: "/games/Epic Games/Starlight Courier".into(),
            install_size: 24_000_000_000,
            executable: "Binaries\\Win64/Starlight.exe".into(),
            launch_parameters: " -nolauncher ".into(),
            is_dlc: false,
            can_run_offline: true,
            save_path: String::new(),
            prereq_info: None,
        }
    }

    #[test]
    fn a_row_launches_legendarys_executable_through_the_compat_tool() {
        let meta = EpicMeta {
            app_name: "Starlight".into(),
            version: "1.0.2".into(),
            platform: "Windows".into(),
            ..Default::default()
        };
        let game = EpicGame {
            app_name: "Starlight".into(),
            title: "Starlight Courier".into(),
            description: "Deliver.".into(),
            developer: "Lumen Works".into(),
            year: "2024".into(),
            screenshots: vec!["https://cdn1.epicgames.com/a.jpg".into()],
            ..Default::default()
        };
        let prefix = Path::new("/games/Epic Games/Starlight Courier/prefix");
        let row = epic_row(&installed(), Some(&game), &meta, Some(prefix));
        let dir = Path::new("/games/Epic Games/Starlight Courier");
        assert_eq!(row.title, "Starlight Courier");
        assert_eq!(row.platform, "Epic Games");
        assert_eq!(row.platform_slug, "win");
        assert_eq!(row.source, GameSource::Store(StoreKind::Epic));
        assert_eq!(row.store_id, "Starlight");
        assert_eq!(row.rom_id, None);
        assert_eq!(row.extracted_dir, dir.to_string_lossy());
        assert_eq!(row.native_game_dir, dir.to_string_lossy());
        assert_eq!(
            PathBuf::from(&row.native_executable_path),
            dir.join("Binaries").join("Win64").join("Starlight.exe")
        );
        assert_eq!(row.native_working_dir, "Binaries/Win64");
        assert_eq!(row.native_launch_parameters, "-nolauncher");
        assert_eq!(row.native_launch_source, LAUNCH_SOURCE_SIDECAR);
        assert_eq!(row.native_wineprefix, prefix.to_string_lossy());
        assert_eq!(row.revision, "1.0.2");
        assert_eq!(row.filesize_bytes, 24_000_000_000);
        assert_eq!(row.companies, "Lumen Works");
        assert_eq!(row.description, "Deliver.");
        assert_eq!(row.first_release_date, "2024");
        assert_eq!(row.screenshot_urls, "https://cdn1.epicgames.com/a.jpg");
        assert_eq!(EpicMeta::from_json(&row.store_meta), meta);
        assert!(row.is_native());
        assert!(row.is_windows());
        assert!(row.installed_at > 0);
    }

    #[test]
    fn without_the_owned_list_the_row_uses_legendarys_title() {
        let meta = EpicMeta {
            exe_user_pinned: true,
            ..Default::default()
        };
        let row = epic_row(&installed(), None, &meta, None);
        assert_eq!(row.title, "starlight courier");
        assert_eq!(row.native_launch_source, LAUNCH_SOURCE_USER);
        assert_eq!(row.native_wineprefix, "");
        assert_eq!(row.companies, "");
    }

    #[test]
    fn an_unsafe_or_missing_executable_is_left_blank() {
        for bad in ["..\\..\\evil.exe", "/bin/sh", "C:\\x.exe", "", "  "] {
            let inst = EpicInstalled {
                executable: bad.into(),
                ..installed()
            };
            let row = epic_row(&inst, None, &EpicMeta::default(), None);
            assert_eq!(row.native_executable_path, "", "{bad:?}");
            assert_eq!(row.native_working_dir, "", "{bad:?}");
        }
        let top = EpicInstalled {
            executable: "Game.exe".into(),
            ..installed()
        };
        let row = epic_row(&top, None, &EpicMeta::default(), None);
        assert_eq!(row.native_working_dir, "", "the install folder itself");
        assert!(row.native_executable_path.ends_with("Game.exe"));
    }
}
