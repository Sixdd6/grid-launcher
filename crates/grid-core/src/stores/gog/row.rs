//! The registry row of an installed GOG game, built from its
//! `goggame-*.info`, its [`GogMeta`] and (when GRID has it) the product's
//! store details. Every install path (gogdl, offline installer, drive
//! restore) writes its row through [`gog_row`], so they all launch alike.
//!
//! The row takes the native launch branch (`launch::native`): it sets
//! `extracted_dir` (the resolver's install folder), `native_game_dir`, the
//! primary play task's executable, arguments and working folder, and a
//! `linux`/`win` platform slug.

use std::path::{Component, Path, PathBuf};

use super::goggame::GogGameInfo;
use super::meta::GogMeta;
use super::GogProduct;
use crate::library::registry::InstalledGame;
use crate::library::specials::native::{LAUNCH_SOURCE_SIDECAR, LAUNCH_SOURCE_USER};
use crate::stores::{GameSource, StoreKind};

/// The row for the GOG game `info` describes, installed in `install_dir`.
///
/// `info` is the base game's file (its `game_id` is the row's store id).
/// The title is the store's when `product` is given, else the file's name.
/// The executable is the primary play task's, as an absolute path under
/// `install_dir`; a task path that climbs out of it (`..`) or is absolute
/// is not used, and Game Settings asks the user for one.
///
/// `meta.exe_user_pinned` marks the launch source `user`; the caller then
/// carries the user's executable over from the old row. `installed_at` is
/// now; `filesize_bytes` is left `0` for the caller to fill.
pub fn gog_row(
    info: &GogGameInfo,
    install_dir: &Path,
    meta: &GogMeta,
    product: Option<&GogProduct>,
    wineprefix: Option<&Path>,
) -> InstalledGame {
    let dir = install_dir.to_string_lossy().into_owned();
    let task = info.primary_task();
    let executable = task
        .and_then(|task| relative_path(&task.path))
        .map(|rel| install_dir.join(rel).to_string_lossy().into_owned())
        .unwrap_or_default();
    let working_dir = task
        .and_then(|task| relative_path(&task.working_dir))
        .map(|rel| {
            rel.components()
                .map(|part| part.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/")
        })
        .unwrap_or_default();
    let title = product
        .map(|product| product.title.trim())
        .filter(|title| !title.is_empty())
        .unwrap_or(info.name.trim())
        .to_string();
    let launch_source = if meta.exe_user_pinned {
        LAUNCH_SOURCE_USER
    } else {
        LAUNCH_SOURCE_SIDECAR
    };

    let mut row = InstalledGame {
        title,
        platform: StoreKind::Gog.platform_name().to_string(),
        rom_id: None,
        extracted_dir: dir.clone(),
        native_game_dir: dir,
        native_executable_path: executable,
        native_launch_parameters: task.map(|task| task.arguments.clone()).unwrap_or_default(),
        native_working_dir: working_dir,
        native_launch_source: launch_source.to_string(),
        native_wineprefix: wineprefix
            .map(|prefix| prefix.to_string_lossy().into_owned())
            .unwrap_or_default(),
        platform_slug: meta.platform.slug().to_string(),
        revision: meta.version.clone(),
        languages: meta.language.clone(),
        installed_at: chrono::Utc::now().timestamp(),
        source: GameSource::Store(StoreKind::Gog),
        store_id: info.game_id.trim().to_string(),
        store_meta: meta.to_json(),
        ..Default::default()
    };
    if let Some(product) = product {
        row.description = product.description.clone();
        row.genres = product.genres.join(", ");
        row.companies = [&product.developer, &product.publisher]
            .into_iter()
            .map(|name| name.trim())
            .filter(|name| !name.is_empty())
            .fold(Vec::<&str>::new(), |mut names, name| {
                if !names.contains(&name) {
                    names.push(name);
                }
                names
            })
            .join(", ");
        row.first_release_date = product.release_date.clone();
        row.screenshot_urls = product.screenshots.join("\n");
    }
    row
}

/// A play-task path as a relative path, either separator; `None` when it
/// is blank, absolute, or climbs out with `..`.
fn relative_path(text: &str) -> Option<PathBuf> {
    let mut rel = PathBuf::new();
    for part in text.trim().split(['/', '\\']) {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." || part.contains(':') {
            return None;
        }
        rel.push(part);
    }
    let plain = rel
        .components()
        .all(|component| matches!(component, Component::Normal(_)));
    (plain && !text.trim().starts_with(['/', '\\']) && rel.components().next().is_some())
        .then_some(rel)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stores::gog::goggame::{parse_info, PlayTask};
    use crate::stores::gog::meta::{GogOrigin, GogPlatform};

    fn info() -> GogGameInfo {
        parse_info(
            br#"{"gameId": "1207658924", "rootGameId": "1207658924", "name": "The Witcher",
                 "buildId": "56", "playTasks": [
                   {"category": "game", "isPrimary": true, "path": "System\\witcher.exe",
                    "arguments": "-nointro", "workingDir": "System\\Data", "type": "FileTask"}]}"#,
        )
        .unwrap()
    }

    #[test]
    fn a_windows_row_launches_its_primary_task() {
        let dir = Path::new("/games/GOG/The Witcher");
        let meta = GogMeta {
            origin: GogOrigin::Gogdl,
            platform: GogPlatform::Windows,
            version: "1.5".into(),
            language: "en-US".into(),
            ..Default::default()
        };
        let prefix = Path::new("/prefixes/witcher");
        let row = gog_row(&info(), dir, &meta, None, Some(prefix));
        assert_eq!(row.title, "The Witcher");
        assert_eq!(row.platform, "GOG");
        assert_eq!(row.platform_slug, "win");
        assert_eq!(row.source, GameSource::Store(StoreKind::Gog));
        assert_eq!(row.store_id, "1207658924");
        assert_eq!(row.rom_id, None);
        assert_eq!(row.extracted_dir, dir.to_string_lossy());
        assert_eq!(row.native_game_dir, dir.to_string_lossy());
        assert_eq!(
            PathBuf::from(&row.native_executable_path),
            dir.join("System").join("witcher.exe")
        );
        assert_eq!(row.native_launch_parameters, "-nointro");
        assert_eq!(row.native_working_dir, "System/Data");
        assert_eq!(row.native_launch_source, LAUNCH_SOURCE_SIDECAR);
        assert_eq!(row.native_wineprefix, prefix.to_string_lossy());
        assert_eq!(row.revision, "1.5");
        assert_eq!(GogMeta::from_json(&row.store_meta), meta);
        assert!(row.is_native());
        assert!(row.is_windows());
        assert!(row.installed_at > 0);
    }

    #[test]
    fn a_linux_row_uses_the_product_details() {
        let product = GogProduct {
            id: "1207658924".into(),
            title: "The Witcher: Enhanced Edition".into(),
            description: "Geralt.".into(),
            developer: "CD PROJEKT RED".into(),
            publisher: "CD PROJEKT RED".into(),
            genres: vec!["Role-playing".into(), "Fantasy".into()],
            release_date: "2008-09-16".into(),
            screenshots: vec!["https://img/1.jpg".into(), "https://img/2.jpg".into()],
            ..Default::default()
        };
        let meta = GogMeta {
            platform: GogPlatform::Linux,
            exe_user_pinned: true,
            ..Default::default()
        };
        let row = gog_row(&info(), Path::new("/g"), &meta, Some(&product), None);
        assert_eq!(row.title, "The Witcher: Enhanced Edition");
        assert_eq!(row.platform_slug, "linux");
        assert!(!row.is_windows());
        assert_eq!(row.description, "Geralt.");
        assert_eq!(row.genres, "Role-playing, Fantasy");
        assert_eq!(row.companies, "CD PROJEKT RED", "one name once");
        assert_eq!(row.first_release_date, "2008-09-16");
        assert_eq!(row.screenshot_urls, "https://img/1.jpg\nhttps://img/2.jpg");
        assert_eq!(row.native_launch_source, LAUNCH_SOURCE_USER);
        assert_eq!(row.native_wineprefix, "");
    }

    #[test]
    fn an_unsafe_or_missing_task_leaves_the_executable_blank() {
        let mut info = info();
        for bad in [
            "..\\..\\evil.exe",
            "/bin/sh",
            "\\\\server\\x.exe",
            "C:\\x.exe",
            "  ",
        ] {
            info.play_tasks = vec![PlayTask {
                is_primary: true,
                path: bad.into(),
                ..Default::default()
            }];
            let row = gog_row(&info, Path::new("/g"), &GogMeta::default(), None, None);
            assert_eq!(row.native_executable_path, "", "{bad:?}");
            assert_eq!(row.native_working_dir, "", "{bad:?}");
        }
        info.play_tasks.clear();
        let row = gog_row(&info, Path::new("/g"), &GogMeta::default(), None, None);
        assert_eq!(row.native_executable_path, "");
        assert_eq!(row.native_launch_parameters, "");
    }

    #[test]
    fn relative_paths_normalise_both_separators() {
        assert_eq!(
            relative_path("./bin\\x64//game.exe"),
            Some(PathBuf::from("bin").join("x64").join("game.exe"))
        );
        assert_eq!(relative_path(""), None);
        assert_eq!(relative_path("."), None);
    }
}
