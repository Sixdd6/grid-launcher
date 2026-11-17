//! Content categories: grouping a `RomDetail`'s files by their RomM
//! `category` (`server/catalog.py:246-262`), and the update/DLC content kind
//! that install specials key on.

use std::collections::BTreeMap;

use crate::library::registry::InstalledGame;
use crate::romm::RomFile;

/// The two non-game content kinds a RomM file can carry in its `category`
/// field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentKind {
    Update,
    Dlc,
}

impl ContentKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ContentKind::Update => "update",
            ContentKind::Dlc => "dlc",
        }
    }

    /// Parses `s` case-insensitively on the trimmed input. Returns `None`
    /// for anything else, including the "game"/blank category.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "update" => Some(ContentKind::Update),
            "dlc" => Some(ContentKind::Dlc),
            _ => None,
        }
    }
}

/// Groups `files`' ids by their normalized category (trimmed, lowercased;
/// blank becomes `"game"`). Preserves each file's relative order within its
/// category.
pub fn file_ids_by_category(files: &[RomFile]) -> BTreeMap<String, Vec<i64>> {
    let mut map: BTreeMap<String, Vec<i64>> = BTreeMap::new();
    for file in files {
        let category = file.category.trim().to_lowercase();
        let key = if category.is_empty() {
            "game".to_string()
        } else {
            category
        };
        map.entry(key).or_default().push(file.id);
    }
    map
}

/// The file ids in `files` whose category matches `kind`.
pub fn content_file_ids(files: &[RomFile], kind: ContentKind) -> Vec<i64> {
    file_ids_by_category(files)
        .remove(kind.as_str())
        .unwrap_or_default()
}

/// Which non-game content kinds `files` carries at least one file for.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize)]
pub struct ContentAvailability {
    pub update: bool,
    pub dlc: bool,
    /// For a game installed by running its GOG installer: the server's DLC
    /// files (by name) already installed into it. Empty for every other
    /// game.
    pub installed_dlc: Vec<String>,
}

/// Computes [`ContentAvailability`] from `files`.
pub fn content_availability(files: &[RomFile]) -> ContentAvailability {
    let map = file_ids_by_category(files);
    let has = |k: &str| map.get(k).is_some_and(|v| !v.is_empty());
    ContentAvailability {
        update: has("update"),
        dlc: has("dlc"),
        installed_dlc: Vec::new(),
    }
}

/// [`ContentAvailability`] for a game whose installed row is `row`.
///
/// A native game installed by running its GOG installer offers DLC when the
/// server holds `dlc` files whose installers have not run into it yet
/// ([`dlc_files_to_install`]), and no update. Any other native game offers
/// neither (it has no content flow). Every other row, and no row, reads
/// `files` alone ([`content_availability`]).
pub fn content_availability_for(
    files: &[RomFile],
    row: Option<&InstalledGame>,
) -> ContentAvailability {
    let Some(row) = row.filter(|row| row.is_native()) else {
        return content_availability(files);
    };
    if !row.installed_by_installer || row.is_store() {
        return ContentAvailability::default();
    }
    let installed = installed_dlc_names(&row.romm_dlc_files);
    ContentAvailability {
        update: false,
        dlc: !dlc_files_to_install(files, &installed).is_empty(),
        installed_dlc: installed_dlc_files(files, &installed)
            .into_iter()
            .map(|file| file.file_name.clone())
            .collect(),
    }
}

/// Whether `file` is a server DLC file GRID can download by its own name:
/// category `dlc`, a plain file name (no path separator).
fn is_dlc_download(file: &RomFile) -> bool {
    ContentKind::parse(&file.category) == Some(ContentKind::Dlc)
        && !file.file_name.trim().is_empty()
        && !file.file_name.contains('/')
        && !file.file_name.contains('\\')
}

/// The DLC files in `files` whose installers have not run into the game
/// yet (their names are not in `installed`), in server order.
pub fn dlc_files_to_install<'a>(files: &'a [RomFile], installed: &[String]) -> Vec<&'a RomFile> {
    files
        .iter()
        .filter(|file| is_dlc_download(file) && !installed.contains(&file.file_name))
        .collect()
}

/// The DLC files in `files` that are installed (their names are in
/// `installed`), in server order. A recorded name the server no longer
/// lists is left out.
pub fn installed_dlc_files<'a>(files: &'a [RomFile], installed: &[String]) -> Vec<&'a RomFile> {
    files
        .iter()
        .filter(|file| is_dlc_download(file) && installed.contains(&file.file_name))
        .collect()
}

/// The DLC file names recorded on a row's `romm_dlc_files` (a JSON array of
/// names). Blank or unreadable is none.
pub fn installed_dlc_names(json: &str) -> Vec<String> {
    if json.trim().is_empty() {
        return Vec::new();
    }
    serde_json::from_str::<Vec<String>>(json).unwrap_or_default()
}

/// `json` (a row's `romm_dlc_files`) with `name` recorded, once.
pub fn with_installed_dlc(json: &str, name: &str) -> String {
    let mut names = installed_dlc_names(json);
    if !names.iter().any(|known| known == name) {
        names.push(name.to_string());
    }
    serde_json::to_string(&names).unwrap_or_default()
}

/// The label of one DLC file's Downloads row: the file name without its
/// extension (`San Rafaela.7z` → `San Rafaela`).
pub fn dlc_label(file_name: &str) -> String {
    let stem = std::path::Path::new(file_name)
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    if stem.trim().is_empty() {
        file_name.to_string()
    } else {
        stem
    }
}

/// Whether `category`, trimmed and lowercased, is blank or `"game"` — i.e.
/// not update/DLC/other bonus content.
pub fn is_game_category(category: &str) -> bool {
    let c = category.trim().to_lowercase();
    c.is_empty() || c == "game"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(id: i64, category: &str) -> RomFile {
        RomFile {
            id,
            file_name: format!("file{id}.zip"),
            file_size_bytes: 0,
            is_top_level: true,
            last_modified: String::new(),
            category: category.to_string(),
            md5_hash: String::new(),
            sha1_hash: String::new(),
        }
    }

    #[test]
    fn blank_category_groups_as_game() {
        let files = [file(1, "")];
        let map = file_ids_by_category(&files);
        assert_eq!(map.get("game"), Some(&vec![1]));
    }

    #[test]
    fn category_is_trimmed_and_lowercased() {
        let files = [file(1, " Update ")];
        let map = file_ids_by_category(&files);
        assert_eq!(map.get("update"), Some(&vec![1]));
    }

    #[test]
    fn two_updates_keep_order() {
        let files = [file(1, "update"), file(2, "update")];
        let map = file_ids_by_category(&files);
        assert_eq!(map.get("update"), Some(&vec![1, 2]));
    }

    #[test]
    fn content_file_ids_reads_from_the_grouped_map() {
        let files = [file(1, "game"), file(2, "update"), file(3, "dlc")];
        assert_eq!(content_file_ids(&files, ContentKind::Update), vec![2]);
        assert_eq!(content_file_ids(&files, ContentKind::Dlc), vec![3]);
    }

    #[test]
    fn availability_from_a_mixed_list() {
        let files = [file(1, "game"), file(2, "update"), file(3, "")];
        let availability = content_availability(&files);
        assert_eq!(
            availability,
            ContentAvailability {
                update: true,
                dlc: false,
                installed_dlc: Vec::new(),
            }
        );
    }

    fn named(id: i64, name: &str, category: &str) -> RomFile {
        RomFile {
            file_name: name.to_string(),
            is_top_level: false,
            ..file(id, category)
        }
    }

    fn cartel_files() -> Vec<RomFile> {
        vec![
            named(1, "Cartel Tycoon.7z", ""),
            named(2, "San Rafaela.7z", "dlc"),
            named(3, "Lieutenants Pack - Guerilla.7z", "dlc"),
            named(4, "Lieutenants Pack - La Familia.7z", "DLC"),
            named(5, "Soundtrack.zip", "soundtrack"),
            named(6, "nested/Other.7z", "dlc"),
        ]
    }

    fn installer_row(dlc: &str) -> InstalledGame {
        InstalledGame {
            title: "Cartel Tycoon".into(),
            platform: "Windows".into(),
            platform_slug: "win".into(),
            rom_id: Some(7),
            installed_by_installer: true,
            romm_dlc_files: dlc.into(),
            ..Default::default()
        }
    }

    #[test]
    fn dlc_files_to_install_are_the_dlc_files_not_yet_installed() {
        let files = cartel_files();
        let names = |picked: Vec<&RomFile>| -> Vec<i64> { picked.iter().map(|f| f.id).collect() };
        assert_eq!(names(dlc_files_to_install(&files, &[])), vec![2, 3, 4]);
        let installed = vec!["San Rafaela.7z".to_string(), "Gone.7z".to_string()];
        assert_eq!(names(dlc_files_to_install(&files, &installed)), vec![3, 4]);
        assert_eq!(names(installed_dlc_files(&files, &installed)), vec![2]);
    }

    #[test]
    fn installed_dlc_bookkeeping_records_each_name_once() {
        assert!(installed_dlc_names("").is_empty());
        assert!(installed_dlc_names("not json").is_empty());
        let one = with_installed_dlc("", "San Rafaela.7z");
        assert_eq!(one, r#"["San Rafaela.7z"]"#);
        let two = with_installed_dlc(&one, "Lieutenants Pack - Guerilla.7z");
        assert_eq!(with_installed_dlc(&two, "San Rafaela.7z"), two);
        assert_eq!(
            installed_dlc_names(&two),
            vec!["San Rafaela.7z", "Lieutenants Pack - Guerilla.7z"]
        );
    }

    #[test]
    fn a_gog_installer_game_offers_the_dlc_it_lacks() {
        let files = cartel_files();
        let none = content_availability_for(&files, Some(&installer_row("")));
        assert!(none.dlc && !none.update);
        assert!(none.installed_dlc.is_empty());

        let some = content_availability_for(&files, Some(&installer_row(r#"["San Rafaela.7z"]"#)));
        assert!(some.dlc);
        assert_eq!(some.installed_dlc, vec!["San Rafaela.7z"]);

        let all = content_availability_for(
            &files,
            Some(&installer_row(
                r#"["San Rafaela.7z","Lieutenants Pack - Guerilla.7z","Lieutenants Pack - La Familia.7z"]"#,
            )),
        );
        assert!(!all.dlc);
        assert_eq!(all.installed_dlc.len(), 3);
    }

    #[test]
    fn other_native_games_offer_no_content_and_consoles_read_the_files() {
        let files = cartel_files();
        let mut plain = installer_row("");
        plain.installed_by_installer = false;
        assert_eq!(
            content_availability_for(&files, Some(&plain)),
            ContentAvailability::default()
        );
        let ps4 = InstalledGame {
            platform: "PlayStation 4".into(),
            platform_slug: "ps4".into(),
            ..Default::default()
        };
        assert!(content_availability_for(&files, Some(&ps4)).dlc);
        assert!(content_availability_for(&files, None).dlc);
    }

    #[test]
    fn dlc_labels_drop_the_extension() {
        assert_eq!(dlc_label("San Rafaela.7z"), "San Rafaela");
        assert_eq!(
            dlc_label("Lieutenants Pack - La Familia.7z"),
            "Lieutenants Pack - La Familia"
        );
        assert_eq!(dlc_label("setup_dlc.sh"), "setup_dlc");
    }

    #[test]
    fn availability_is_all_false_with_no_content_files() {
        let files = [file(1, "game")];
        assert_eq!(content_availability(&files), ContentAvailability::default());
    }

    #[test]
    fn content_kind_parse_is_case_insensitive_on_trimmed_input() {
        assert_eq!(ContentKind::parse("DLC"), Some(ContentKind::Dlc));
        assert_eq!(ContentKind::parse(" update "), Some(ContentKind::Update));
        assert_eq!(ContentKind::parse("x"), None);
        assert_eq!(ContentKind::parse("game"), None);
    }

    #[test]
    fn content_kind_serde_round_trip_uses_snake_case() {
        let json = serde_json::to_string(&ContentKind::Update).unwrap();
        assert_eq!(json, "\"update\"");
        let back: ContentKind = serde_json::from_str(&json).unwrap();
        assert_eq!(back, ContentKind::Update);
    }

    #[test]
    fn is_game_category_treats_blank_and_game_as_game() {
        assert!(is_game_category(""));
        assert!(is_game_category("  "));
        assert!(is_game_category(" Game "));
        assert!(!is_game_category("update"));
        assert!(!is_game_category("dlc"));
    }
}
