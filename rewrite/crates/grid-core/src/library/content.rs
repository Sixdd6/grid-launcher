//! Content categories: grouping a `RomDetail`'s files by their RomM
//! `category` (`server/catalog.py:246-262`), and the update/DLC content kind
//! that install specials key on.

use std::collections::BTreeMap;

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize)]
pub struct ContentAvailability {
    pub update: bool,
    pub dlc: bool,
}

/// Computes [`ContentAvailability`] from `files`.
pub fn content_availability(files: &[RomFile]) -> ContentAvailability {
    let map = file_ids_by_category(files);
    let has = |k: &str| map.get(k).is_some_and(|v| !v.is_empty());
    ContentAvailability {
        update: has("update"),
        dlc: has("dlc"),
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
            category: category.to_string(),
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
            }
        );
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
