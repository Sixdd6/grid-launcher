//! A Steam game's registry row, built from its app manifest and the
//! `GetItems` metadata. A Steam row has no native launch columns:
//! [`InstalledGame::is_native`] is `false` for it, so `LaunchService` never
//! builds a command for it (the Steam client starts it).

use std::path::{Path, PathBuf};

use super::meta::SteamMeta;
use super::{AppInfo, ManifestState};
use crate::library::registry::InstalledGame;
use crate::stores::{GameSource, StoreKind};

/// The platform slug a Steam row carries.
pub const STEAM_SLUG: &str = "steam";

/// The row for one installed Steam game: source `steam`, `store_id` the
/// app id, platform "Steam", slug `steam`; the title is the store's name,
/// else the manifest's, else the app id; the size is `SizeOnDisk`;
/// companies are the developers and the release date is the year.
///
/// The game's folder ([`install_path`]) lives in `store_meta` only: no path
/// column is set, so no GRID path that removes, moves or backs up a row's
/// files (uninstall, the library move, drive backups) can ever reach a
/// folder Steam owns.
pub fn steam_row(m: &ManifestState, info: Option<&AppInfo>, meta: &SteamMeta) -> InstalledGame {
    let store_id = m.appid.to_string();
    let title = info
        .map(|info| info.name.trim())
        .filter(|name| !name.is_empty())
        .or_else(|| Some(m.name.trim()).filter(|name| !name.is_empty()))
        .unwrap_or(&store_id)
        .to_string();
    let mut row = InstalledGame {
        title,
        platform: StoreKind::Steam.platform_name().to_string(),
        rom_id: None,
        platform_slug: STEAM_SLUG.to_string(),
        revision: if m.build_id == 0 {
            String::new()
        } else {
            m.build_id.to_string()
        },
        filesize_bytes: i64::try_from(m.size_on_disk).unwrap_or(i64::MAX),
        installed_at: if m.last_updated > 0 {
            m.last_updated
        } else {
            chrono::Utc::now().timestamp()
        },
        source: GameSource::Store(StoreKind::Steam),
        store_id,
        store_meta: meta.to_json(),
        ..Default::default()
    };
    if let Some(info) = info {
        row.description = info.description.clone();
        row.companies = info.developers.join(", ");
        row.first_release_date = info.year.clone();
        row.screenshot_urls = info.screenshots.join("\n");
    }
    row
}

/// A Steam game's folder: `<library>/steamapps/common/<installdir>`, `None`
/// when the manifest names no folder.
pub fn install_path(library: &str, install_dir: &str) -> Option<PathBuf> {
    let install_dir = install_dir.trim();
    if install_dir.is_empty() {
        return None;
    }
    Some(
        Path::new(library)
            .join("steamapps")
            .join("common")
            .join(install_dir),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> ManifestState {
        ManifestState {
            appid: 620,
            name: "Portal 2 (manifest)".into(),
            library: "/games/SteamLibrary".into(),
            install_dir: "Portal 2".into(),
            flags: 4,
            size_on_disk: 12_000_000_000,
            build_id: 777,
            last_updated: 1_700_000_000,
            last_played: 1_700_000_100,
            ..Default::default()
        }
    }

    fn info() -> AppInfo {
        AppInfo {
            appid: 620,
            kind: Some(0),
            name: "Portal 2".into(),
            description: "Puzzles.".into(),
            developers: vec!["Valve".into(), "Friends".into()],
            year: "2011".into(),
            cover: "https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/620/library_600x900.jpg".into(),
            screenshots: vec!["https://a/1.jpg".into(), "https://a/2.jpg".into()],
        }
    }

    #[test]
    fn a_row_from_the_manifest_and_the_store_metadata() {
        let meta = SteamMeta {
            library: "/games/SteamLibrary".into(),
            install_dir: "Portal 2".into(),
            build_id: 777,
            outside: true,
        };
        let row = steam_row(&manifest(), Some(&info()), &meta);
        assert_eq!(row.source, GameSource::Store(StoreKind::Steam));
        assert_eq!(row.store_id, "620");
        assert_eq!(row.title, "Portal 2");
        assert_eq!(row.platform, "Steam");
        assert_eq!(row.platform_slug, "steam");
        assert_eq!(
            install_path(&meta.library, &meta.install_dir),
            Some(PathBuf::from(
                "/games/SteamLibrary/steamapps/common/Portal 2"
            ))
        );
        assert_eq!(install_path("/x", " "), None);
        assert!(row.extracted_dir.is_empty(), "no path column, ever");
        assert_eq!(row.filesize_bytes, 12_000_000_000);
        assert_eq!(row.companies, "Valve, Friends");
        assert_eq!(row.first_release_date, "2011");
        assert_eq!(row.description, "Puzzles.");
        assert_eq!(row.screenshot_urls, "https://a/1.jpg\nhttps://a/2.jpg");
        assert_eq!(row.revision, "777");
        assert_eq!(row.installed_at, 1_700_000_000);
        assert_eq!(SteamMeta::from_json(&row.store_meta), meta);
        assert!(row.native_executable_path.is_empty());
        assert!(row.native_game_dir.is_empty());
        assert!(!row.is_native(), "the Steam client launches it");
        assert_eq!(row.last_played_at, 0, "only touch_last_played writes it");
    }

    #[test]
    fn the_title_falls_back_to_the_manifest_then_the_app_id() {
        let meta = SteamMeta::default();
        let row = steam_row(&manifest(), None, &meta);
        assert_eq!(row.title, "Portal 2 (manifest)");
        assert!(row.companies.is_empty());
        let mut blank = info();
        blank.name = "  ".into();
        assert_eq!(
            steam_row(&manifest(), Some(&blank), &meta).title,
            "Portal 2 (manifest)"
        );
        let mut m = manifest();
        m.name = String::new();
        assert_eq!(steam_row(&m, None, &meta).title, "620");
    }
}
