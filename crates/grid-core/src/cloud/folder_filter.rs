//! G1-Q: which top-level folders of an RPCS3 cloud save record restore for
//! a game.
//!
//! An RPCS3 save record is a zip of savedata folders (`BLUS30443DATA/…`)
//! that restore extracts into the savedata root. Released builds before G1
//! could upload other PS3 games' savedata folders under one game's ROM, so
//! an old record can name a folder that belongs to another game. Restoring
//! that folder would overwrite the other game's local save. This module
//! decides, from data GRID has about the game, which folders of a record
//! are this game's:
//!
//! - **Known PS3 id** (`ps3_game_id`): a folder is kept when its name holds
//!   the id. This is the rule the RPCS3 save scanner uses for upload (G1),
//!   so a record written after G1 keeps every folder.
//! - **Unknown id**: a folder is kept when its name holds a PS3 serial
//!   found in the game's file names and install paths (or the install's
//!   `PARAM.SFO`), or when the folder's own `PARAM.SFO` `TITLE` equals the
//!   game title (compared `[a-z0-9]`-only). Title matches that point at
//!   more than one serial are ambiguous and are skipped.
//! - A folder whose serial is another installed game's known id (and not
//!   one of this game's serials) is skipped.
//! - Nothing else is kept. There is no "a one-folder record is this game's"
//!   rule: every RPCS3 upload writes one record per folder, so an old
//!   polluted record holds exactly one folder, and that folder is another
//!   game's.
//!
//! Cemu records are not filtered here: their top-level folders are Cemu
//! user folders (`80000001`, `common`) with no title id, and restore puts
//! them at `<mlc>/usr/save/<folder>`, outside every title folder, so a
//! Cemu restore cannot overwrite another game's save.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use crate::library::paths::expand_home;

use super::archive::{archive_member_bytes, archive_top_level_entries};
use super::restore::latest_server_record;
use super::tokens::compact_alnum;
use super::CloudGame;

/// A PS3 serial anywhere in a text, separators allowed (`BLUS-30443`).
static SERIAL_SEARCH_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[A-Z]{4}[-_ ]?[0-9]{5}").unwrap());
/// The serial a savedata folder name starts with (`BLUS30443DATA`).
static LEADING_SERIAL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Z]{4}[0-9]{5}").unwrap());
static NON_ALNUM_UPPER_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^A-Z0-9]+").unwrap());

/// How many skipped folder names a notice lists before "and N more".
const NOTICE_NAME_LIMIT: usize = 5;

/// What restore does with folders the filter cannot match.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FolderPolicy {
    /// Restore only the folders [`folders_to_restore`] keeps (auto-restore
    /// before launch, and the first try of a manual restore).
    #[default]
    MatchedOnly,
    /// Restore every folder in the record. Only a manual restore the user
    /// confirmed after seeing the skipped folders uses this.
    RestoreAll,
}

/// One top-level entry of a save record's archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordFolder {
    /// The entry name as the archive spells it.
    pub name: String,
    /// `false` for a loose file at the archive root.
    pub is_dir: bool,
    /// The `TITLE` of `<name>/PARAM.SFO`, when the archive has one.
    pub sfo_title: Option<String>,
}

impl RecordFolder {
    pub fn dir(name: &str, sfo_title: Option<&str>) -> Self {
        Self {
            name: name.to_string(),
            is_dir: true,
            sfo_title: sfo_title.map(str::to_string),
        }
    }
}

/// The data a game is matched by. Every id is `[A-Z0-9]`-only uppercase.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FolderMatchKeys {
    /// The game's known PS3 id (`ps3_game_id`), when it has one.
    pub known_ids: Vec<String>,
    /// Serials found in the game's file names, install paths, and install
    /// `PARAM.SFO`. Used only when `known_ids` is empty.
    pub derived_ids: Vec<String>,
    /// `compact_alnum(title)`.
    pub title_key: String,
    /// Other installed games' known ids, mapped to their titles.
    pub foreign_ids: BTreeMap<String, String>,
}

/// Why a folder was not restored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkipReason {
    /// Nothing about the folder matches this game.
    NoMatch,
    /// The folder matches the title, but the title matches folders of more
    /// than one serial.
    Ambiguous,
    /// The folder's serial is this other installed game's id.
    OtherGame(String),
    /// A loose file at the archive root, not a save folder.
    NotAFolder,
}

impl SkipReason {
    pub fn describe(&self) -> String {
        match self {
            SkipReason::NoMatch => "does not match this game".to_string(),
            SkipReason::Ambiguous => "title matches more than one game id".to_string(),
            SkipReason::OtherGame(title) => format!("belongs to {title}"),
            SkipReason::NotAFolder => "not a save folder".to_string(),
        }
    }
}

/// The filter's answer: the top-level names to restore, and every other
/// entry with the reason it was skipped. Both keep the record's order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FolderSelection {
    pub keep: Vec<String>,
    pub skipped: Vec<(String, SkipReason)>,
}

fn upper_alnum(value: &str) -> String {
    NON_ALNUM_UPPER_RE
        .replace_all(&value.trim().to_uppercase(), "")
        .into_owned()
}

fn push_unique(list: &mut Vec<String>, value: String) {
    if !value.is_empty() && !list.contains(&value) {
        list.push(value);
    }
}

/// Every PS3 serial in `text`, separators dropped, trophy ids (`NPWR…`)
/// excluded.
fn serials_in(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    for m in SERIAL_SEARCH_RE.find_iter(&text.to_uppercase()) {
        let serial = upper_alnum(m.as_str());
        if !serial.starts_with("NPWR") {
            push_unique(&mut found, serial);
        }
    }
    found
}

fn leading_serial(normalized_name: &str) -> Option<String> {
    LEADING_SERIAL_RE
        .find(normalized_name)
        .map(|m| m.as_str().to_string())
}

impl FolderMatchKeys {
    /// The keys for an RPCS3 game. `all_games` is the registry (plus any
    /// server rows, which carry no PS3 id); rows with this game's ROM id
    /// are not "other" games. Reads at most a few `PARAM.SFO` files of the
    /// game's own install.
    pub fn for_rpcs3_game(game: &CloudGame, all_games: &[CloudGame]) -> Self {
        let mut keys = FolderMatchKeys {
            title_key: compact_alnum(&game.title),
            ..Default::default()
        };
        push_unique(&mut keys.known_ids, upper_alnum(&game.ps3_game_id));

        for value in [
            &game.rom_file_name,
            &game.extracted_path,
            &game.extracted_dir,
            &game.archive_path,
            &game.native_game_dir,
        ] {
            for serial in serials_in(value) {
                push_unique(&mut keys.derived_ids, serial);
            }
        }
        for dir in install_sfo_dirs(game) {
            let serial = upper_alnum(&crate::library::specials::ps3::detect_game_id_from_sfo(
                &dir,
            ));
            push_unique(&mut keys.derived_ids, serial);
        }

        let own_rom = game.rom_id.trim();
        for other in all_games {
            if !own_rom.is_empty() && other.rom_id.trim() == own_rom {
                continue;
            }
            let id = upper_alnum(&other.ps3_game_id);
            if id.is_empty() || keys.known_ids.contains(&id) {
                continue;
            }
            keys.foreign_ids
                .entry(id)
                .or_insert_with(|| other.title.trim().to_string());
        }
        keys
    }

    /// The ids a record's file name is checked against when choosing which
    /// record to restore: the known id, else the derived serials.
    fn record_ids(&self) -> &[String] {
        if self.known_ids.is_empty() {
            &self.derived_ids
        } else {
            &self.known_ids
        }
    }
}

/// The directories whose `PS3_GAME/PARAM.SFO` or `PARAM.SFO` can name the
/// game's serial: `extracted_dir` and up to three parents of
/// `extracted_path` (the EBOOT sits at `<game>/PS3_GAME/USRDIR/EBOOT.BIN`).
fn install_sfo_dirs(game: &CloudGame) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let extracted_dir = game.extracted_dir.trim();
    if !extracted_dir.is_empty() {
        dirs.push(expand_home(extracted_dir));
    }
    let extracted_path = game.extracted_path.trim();
    if !extracted_path.is_empty() {
        let path = expand_home(extracted_path);
        dirs.extend(path.ancestors().skip(1).take(3).map(Path::to_path_buf));
    }
    dirs.retain(|d| d.is_dir());
    dirs
}

/// G1-Q: which of `folders` restore for the game `keys` describes. See the
/// module doc for the rules.
pub fn folders_to_restore(folders: &[RecordFolder], keys: &FolderMatchKeys) -> FolderSelection {
    enum Verdict {
        Keep,
        /// A title-only match; the key is the folder's serial, or its whole
        /// normalized name when it has none.
        ByTitle(String),
        Skip(SkipReason),
    }

    let mut verdicts: Vec<Verdict> = Vec::with_capacity(folders.len());
    let mut serial_matched: BTreeSet<String> = BTreeSet::new();

    for folder in folders {
        if !folder.is_dir {
            verdicts.push(Verdict::Skip(SkipReason::NotAFolder));
            continue;
        }
        let name = upper_alnum(&folder.name);
        let holds = |ids: &[String]| ids.iter().any(|id| name.contains(id.as_str()));

        if !keys.known_ids.is_empty() {
            verdicts.push(if holds(&keys.known_ids) {
                Verdict::Keep
            } else {
                Verdict::Skip(SkipReason::NoMatch)
            });
            continue;
        }

        let serial = leading_serial(&name);
        if let Some(serial) = &serial {
            if !keys.derived_ids.contains(serial) {
                if let Some(title) = keys.foreign_ids.get(serial) {
                    verdicts.push(Verdict::Skip(SkipReason::OtherGame(title.clone())));
                    continue;
                }
            }
        }
        if holds(&keys.derived_ids) {
            serial_matched.insert(serial.unwrap_or_else(|| name.clone()));
            verdicts.push(Verdict::Keep);
            continue;
        }
        let title_matches = !keys.title_key.is_empty()
            && folder
                .sfo_title
                .as_deref()
                .is_some_and(|t| compact_alnum(t) == keys.title_key);
        verdicts.push(if title_matches {
            Verdict::ByTitle(serial.unwrap_or(name))
        } else {
            Verdict::Skip(SkipReason::NoMatch)
        });
    }

    // Title-only matches are trusted only when they point at one serial,
    // and at the serial a serial match found (when there was one).
    let title_keys: BTreeSet<&String> = verdicts
        .iter()
        .filter_map(|v| match v {
            Verdict::ByTitle(key) => Some(key),
            _ => None,
        })
        .collect();
    let titles_unambiguous = title_keys.len() <= 1
        && title_keys
            .iter()
            .all(|key| serial_matched.is_empty() || serial_matched.contains(*key));

    let mut selection = FolderSelection::default();
    for (folder, verdict) in folders.iter().zip(verdicts) {
        match verdict {
            Verdict::Keep => selection.keep.push(folder.name.clone()),
            Verdict::ByTitle(_) if titles_unambiguous => selection.keep.push(folder.name.clone()),
            Verdict::ByTitle(_) => selection
                .skipped
                .push((folder.name.clone(), SkipReason::Ambiguous)),
            Verdict::Skip(reason) => selection.skipped.push((folder.name.clone(), reason)),
        }
    }
    selection
}

/// Reads the top-level entries of a save archive and, for each folder, the
/// `TITLE` of its `PARAM.SFO`.
pub fn record_folders(payload: &[u8]) -> Result<Vec<RecordFolder>, String> {
    let entries = archive_top_level_entries(payload)?;
    Ok(entries
        .into_iter()
        .map(|(name, is_dir)| {
            let sfo_title = if is_dir {
                archive_member_bytes(payload, &format!("{name}/PARAM.SFO"))
                    .and_then(|sfo| sfo_string(&sfo, "TITLE"))
            } else {
                None
            };
            RecordFolder {
                name,
                is_dir,
                sfo_title,
            }
        })
        .collect())
}

/// The string value of `key` in a `PARAM.SFO`, NUL padding removed, or
/// `None` when the bytes are not a well-formed SFO or have no such key.
pub fn sfo_string(bytes: &[u8], key: &str) -> Option<String> {
    fn u16_at(bytes: &[u8], at: usize) -> Option<usize> {
        let raw = bytes.get(at..at.checked_add(2)?)?;
        Some(u16::from_le_bytes([raw[0], raw[1]]) as usize)
    }
    fn u32_at(bytes: &[u8], at: usize) -> Option<usize> {
        let raw = bytes.get(at..at.checked_add(4)?)?;
        Some(u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]) as usize)
    }

    if bytes.get(..4)? != b"\0PSF" {
        return None;
    }
    let key_table = u32_at(bytes, 8)?;
    let data_table = u32_at(bytes, 12)?;
    let entries = u32_at(bytes, 16)?;
    for index in 0..entries {
        let entry = 20usize.checked_add(index.checked_mul(16)?)?;
        let key_start = key_table.checked_add(u16_at(bytes, entry)?)?;
        let data_len = u32_at(bytes, entry + 4)?;
        let data_start = data_table.checked_add(u32_at(bytes, entry + 12)?)?;

        let key_bytes = bytes.get(key_start..)?;
        let key_end = key_bytes.iter().position(|&b| b == 0)?;
        if &key_bytes[..key_end] != key.as_bytes() {
            continue;
        }
        let value = bytes.get(data_start..data_start.checked_add(data_len)?)?;
        let text = String::from_utf8_lossy(value);
        return Some(text.trim_end_matches('\0').trim().to_string());
    }
    None
}

/// The record auto-restore picks for an RPCS3 game: the newest record of
/// this emulator whose file name holds one of the game's ids (an upload
/// names its archive after the folder: `BLUS30443DATA-<time>.zip`), else
/// [`latest_server_record`]. Old polluted uploads put several games'
/// folders under one ROM at the same time; this keeps the game's own record
/// from losing to another game's record by a second.
pub fn preferred_record<'a>(
    records: &'a [Value],
    emulator_name: &str,
    keys: &FolderMatchKeys,
) -> Option<&'a Value> {
    let ids = keys.record_ids();
    if !ids.is_empty() {
        let named: Vec<Value> = records
            .iter()
            .filter(|record| {
                let name = upper_alnum(
                    record
                        .get("file_name")
                        .and_then(Value::as_str)
                        .unwrap_or(""),
                );
                ids.iter().any(|id| name.contains(id.as_str()))
            })
            .cloned()
            .collect();
        if let Some(best) = latest_server_record(&named, emulator_name) {
            let best_id = best.get("id");
            return records.iter().find(|r| r.get("id") == best_id);
        }
    }
    latest_server_record(records, emulator_name)
}

fn listed_names(names: &[&str]) -> String {
    if names.len() <= NOTICE_NAME_LIMIT {
        return names.join(", ");
    }
    format!(
        "{} and {} more",
        names[..NOTICE_NAME_LIMIT].join(", "),
        names.len() - NOTICE_NAME_LIMIT
    )
}

/// The notice when no folder of a record could be matched (nothing was
/// restored).
pub fn nothing_matched_notice(title: &str, selection: &FolderSelection) -> String {
    let title = title_or_default(title);
    if selection.skipped.len() == 1 {
        return format!(
            "Cloud save for {title} holds a save folder ({}) that could not be matched to this \
             game — restore it manually in Manage Saves.",
            selection.skipped[0].0
        );
    }
    format!(
        "Cloud save for {title} holds saves for several games; none could be matched to this \
         game — restore it manually in Manage Saves."
    )
}

/// The notice when some folders were restored and others skipped.
pub fn skipped_notice(title: &str, selection: &FolderSelection) -> String {
    let names: Vec<&str> = selection.skipped.iter().map(|(n, _)| n.as_str()).collect();
    format!(
        "Cloud save for {}: skipped save folders that could not be matched to this game: {}.",
        title_or_default(title),
        listed_names(&names)
    )
}

fn title_or_default(title: &str) -> &str {
    let title = title.trim();
    if title.is_empty() {
        "this game"
    } else {
        title
    }
}

#[cfg(test)]
/// A minimal `PARAM.SFO`: the header, one index entry per key, the key
/// table, then the data table (UTF-8 strings, NUL padded).
pub(crate) fn test_sfo(pairs: &[(&str, &str)]) -> Vec<u8> {
    let mut keys = Vec::new();
    let mut data = Vec::new();
    let mut index = Vec::new();
    for (key, value) in pairs {
        let key_offset = keys.len() as u16;
        keys.extend_from_slice(key.as_bytes());
        keys.push(0);
        let mut value_bytes = value.as_bytes().to_vec();
        value_bytes.push(0);
        let len = value_bytes.len() as u32;
        let max_len = len.div_ceil(4) * 4 + 4;
        let data_offset = data.len() as u32;
        data.extend_from_slice(&value_bytes);
        data.resize(data.len() + (max_len - len) as usize, 0);
        index.extend_from_slice(&key_offset.to_le_bytes());
        index.extend_from_slice(&0x0204u16.to_le_bytes());
        index.extend_from_slice(&len.to_le_bytes());
        index.extend_from_slice(&max_len.to_le_bytes());
        index.extend_from_slice(&data_offset.to_le_bytes());
    }
    while keys.len() % 4 != 0 {
        keys.push(0);
    }
    let key_table_start = 20 + index.len() as u32;
    let data_table_start = key_table_start + keys.len() as u32;
    let mut out = b"\0PSF".to_vec();
    out.extend_from_slice(&0x0101u32.to_le_bytes());
    out.extend_from_slice(&key_table_start.to_le_bytes());
    out.extend_from_slice(&data_table_start.to_le_bytes());
    out.extend_from_slice(&(pairs.len() as u32).to_le_bytes());
    out.extend_from_slice(&index);
    out.extend_from_slice(&keys);
    out.extend_from_slice(&data);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn keys_known(id: &str) -> FolderMatchKeys {
        FolderMatchKeys {
            known_ids: vec![id.to_string()],
            title_key: "demonssouls".to_string(),
            ..Default::default()
        }
    }

    fn keys_unknown(title: &str) -> FolderMatchKeys {
        FolderMatchKeys {
            title_key: compact_alnum(title),
            ..Default::default()
        }
    }

    fn kept(selection: &FolderSelection) -> Vec<&str> {
        selection.keep.iter().map(String::as_str).collect()
    }

    fn skipped(selection: &FolderSelection) -> Vec<(&str, SkipReason)> {
        selection
            .skipped
            .iter()
            .map(|(n, r)| (n.as_str(), r.clone()))
            .collect()
    }

    use super::test_sfo as sfo;

    // --- sfo_string ------------------------------------------------------

    #[test]
    fn sfo_string_reads_a_key_and_drops_nul_padding() {
        let bytes = sfo(&[("CATEGORY", "SD"), ("TITLE", "Demon's Souls™")]);
        assert_eq!(
            sfo_string(&bytes, "TITLE").as_deref(),
            Some("Demon's Souls™")
        );
        assert_eq!(sfo_string(&bytes, "CATEGORY").as_deref(), Some("SD"));
        assert_eq!(sfo_string(&bytes, "SUB_TITLE"), None);
    }

    #[test]
    fn sfo_string_rejects_bad_magic_and_truncated_tables() {
        let bytes = sfo(&[("TITLE", "Demon's Souls")]);
        let mut bad_magic = bytes.clone();
        bad_magic[1] = b'X';
        assert_eq!(sfo_string(&bad_magic, "TITLE"), None);
        // The value is 14 bytes plus 6 of padding: cut into the value.
        assert_eq!(sfo_string(&bytes[..bytes.len() - 12], "TITLE"), None);
        assert_eq!(sfo_string(&bytes[..10], "TITLE"), None);
        assert_eq!(sfo_string(b"", "TITLE"), None);
    }

    // --- folders_to_restore ---------------------------------------------

    /// Known id: the G1 rule — a folder is kept when its name holds the id.
    #[test]
    fn known_id_keeps_only_folders_that_hold_the_id() {
        let folders = [
            RecordFolder::dir("BLUS30443DATA", None),
            RecordFolder::dir("BLUS30443-AUTOSAVE", None),
            RecordFolder::dir("BLES00001SAVE", Some("Demon's Souls")),
        ];
        let selection = folders_to_restore(&folders, &keys_known("BLUS30443"));
        assert_eq!(
            kept(&selection),
            vec!["BLUS30443DATA", "BLUS30443-AUTOSAVE"]
        );
        assert_eq!(
            skipped(&selection),
            vec![("BLES00001SAVE", SkipReason::NoMatch)],
            "with a known id, a title match does not rescue another serial"
        );
    }

    /// Unknown id, serial in the ROM file name: that serial matches.
    #[test]
    fn unknown_id_matches_a_serial_from_the_rom_name() {
        let game = CloudGame {
            title: "Demon's Souls".to_string(),
            rom_id: "7".to_string(),
            rom_file_name: "Demon's Souls [BLUS-30443].iso".to_string(),
            ..Default::default()
        };
        let keys = FolderMatchKeys::for_rpcs3_game(&game, &[]);
        assert!(keys.known_ids.is_empty());
        assert_eq!(keys.derived_ids, vec!["BLUS30443".to_string()]);

        let folders = [
            RecordFolder::dir("BLUS30443DATA", None),
            RecordFolder::dir("BCUS98174AUTO", None),
        ];
        let selection = folders_to_restore(&folders, &keys);
        assert_eq!(kept(&selection), vec!["BLUS30443DATA"]);
        assert_eq!(
            skipped(&selection),
            vec![("BCUS98174AUTO", SkipReason::NoMatch)]
        );
    }

    /// Unknown id, serial only in the install's `PARAM.SFO`.
    #[test]
    fn unknown_id_matches_a_serial_from_the_install_param_sfo() {
        let install = tempfile::tempdir().unwrap();
        let game_dir = install.path().join("Demon's Souls");
        std::fs::create_dir_all(game_dir.join("PS3_GAME")).unwrap();
        std::fs::write(
            game_dir.join("PS3_GAME/PARAM.SFO"),
            sfo(&[("TITLE", "Demon's Souls"), ("TITLE_ID", "BLUS30443")]),
        )
        .unwrap();
        let game = CloudGame {
            title: "Demon's Souls".to_string(),
            rom_id: "7".to_string(),
            extracted_dir: game_dir.to_string_lossy().into_owned(),
            ..Default::default()
        };
        let keys = FolderMatchKeys::for_rpcs3_game(&game, &[]);
        assert_eq!(keys.derived_ids, vec!["BLUS30443".to_string()]);
    }

    /// Unknown id, no serial anywhere: the folder's own `PARAM.SFO` title
    /// equal to the game title (`[a-z0-9]`-only) matches.
    #[test]
    fn unknown_id_matches_the_folder_param_sfo_title() {
        let folders = [
            RecordFolder::dir("BLUS30443DATA", Some("Demon's Souls™")),
            RecordFolder::dir("BLUS30443-AUTOSAVE", Some("DEMON'S SOULS")),
            RecordFolder::dir("BCUS98174AUTO", Some("Uncharted 2: Among Thieves")),
            RecordFolder::dir("NPUB30024SAVE", None),
        ];
        let selection = folders_to_restore(&folders, &keys_unknown("Demon's Souls"));
        assert_eq!(
            kept(&selection),
            vec!["BLUS30443DATA", "BLUS30443-AUTOSAVE"]
        );
        assert_eq!(
            skipped(&selection),
            vec![
                ("BCUS98174AUTO", SkipReason::NoMatch),
                ("NPUB30024SAVE", SkipReason::NoMatch),
            ]
        );
    }

    /// A title that matches folders of two serials is ambiguous: neither
    /// restores.
    #[test]
    fn title_matches_across_two_serials_are_ambiguous() {
        let folders = [
            RecordFolder::dir("BLUS30443DATA", Some("Demon's Souls")),
            RecordFolder::dir("BLES00932DATA", Some("Demon's Souls")),
        ];
        let selection = folders_to_restore(&folders, &keys_unknown("Demon's Souls"));
        assert!(selection.keep.is_empty());
        assert_eq!(
            skipped(&selection),
            vec![
                ("BLUS30443DATA", SkipReason::Ambiguous),
                ("BLES00932DATA", SkipReason::Ambiguous),
            ]
        );
    }

    /// A title match on another serial than the one a serial match found is
    /// ambiguous; the serial match still restores.
    #[test]
    fn a_title_match_off_the_serial_match_is_ambiguous() {
        let keys = FolderMatchKeys {
            derived_ids: vec!["BLUS30443".to_string()],
            title_key: "demonssouls".to_string(),
            ..Default::default()
        };
        let folders = [
            RecordFolder::dir("BLUS30443DATA", None),
            RecordFolder::dir("BLES00932DATA", Some("Demon's Souls")),
        ];
        let selection = folders_to_restore(&folders, &keys);
        assert_eq!(kept(&selection), vec!["BLUS30443DATA"]);
        assert_eq!(
            skipped(&selection),
            vec![("BLES00932DATA", SkipReason::Ambiguous)]
        );
    }

    /// One folder is not "this game's by construction": every RPCS3 upload
    /// writes one record per folder, so an old polluted record holds
    /// exactly one folder of another game.
    #[test]
    fn a_single_unmatched_folder_is_not_restored() {
        let folders = [RecordFolder::dir("BCUS98174AUTO", None)];
        let selection = folders_to_restore(&folders, &keys_unknown("Demon's Souls"));
        assert!(selection.keep.is_empty());
        assert_eq!(
            skipped(&selection),
            vec![("BCUS98174AUTO", SkipReason::NoMatch)]
        );
        assert_eq!(
            nothing_matched_notice("Demon's Souls", &selection),
            "Cloud save for Demon's Souls holds a save folder (BCUS98174AUTO) that could not be \
             matched to this game — restore it manually in Manage Saves."
        );
    }

    /// Several folders, none matched: nothing restores, and the notice says
    /// so.
    #[test]
    fn several_unmatched_folders_restore_nothing() {
        let folders = [
            RecordFolder::dir("BCUS98174AUTO", None),
            RecordFolder::dir("NPUB30024SAVE", Some("Flower")),
        ];
        let selection = folders_to_restore(&folders, &keys_unknown("Demon's Souls"));
        assert!(selection.keep.is_empty());
        assert_eq!(selection.skipped.len(), 2);
        assert_eq!(
            nothing_matched_notice("Demon's Souls", &selection),
            "Cloud save for Demon's Souls holds saves for several games; none could be matched \
             to this game — restore it manually in Manage Saves."
        );
    }

    /// A folder whose serial is another installed game's known id is
    /// skipped, even when its title would match.
    #[test]
    fn a_folder_with_another_installed_games_id_is_skipped() {
        let this = CloudGame {
            title: "Demon's Souls".to_string(),
            rom_id: "7".to_string(),
            ..Default::default()
        };
        let other = CloudGame {
            title: "Demon's Souls (Asia)".to_string(),
            rom_id: "8".to_string(),
            ps3_game_id: "BCAS20071".to_string(),
            ..Default::default()
        };
        // A registry row of this same ROM never counts as "another game".
        let same_rom = CloudGame {
            rom_id: "7".to_string(),
            ps3_game_id: "BLUS30443".to_string(),
            ..Default::default()
        };
        let keys = FolderMatchKeys::for_rpcs3_game(&this, &[other, same_rom]);
        assert_eq!(
            keys.foreign_ids,
            BTreeMap::from([("BCAS20071".to_string(), "Demon's Souls (Asia)".to_string())])
        );

        let folders = [
            RecordFolder::dir("BCAS20071DATA", Some("Demon's Souls")),
            RecordFolder::dir("BLUS30443DATA", Some("Demon's Souls")),
        ];
        let selection = folders_to_restore(&folders, &keys);
        assert_eq!(kept(&selection), vec!["BLUS30443DATA"]);
        assert_eq!(
            skipped(&selection),
            vec![(
                "BCAS20071DATA",
                SkipReason::OtherGame("Demon's Souls (Asia)".to_string())
            )]
        );
    }

    /// A loose file at the archive root is never a save folder.
    #[test]
    fn a_loose_root_file_is_skipped() {
        let folders = [
            RecordFolder {
                name: "BLUS30443.txt".to_string(),
                is_dir: false,
                sfo_title: None,
            },
            RecordFolder::dir("BLUS30443DATA", None),
        ];
        let selection = folders_to_restore(&folders, &keys_known("BLUS30443"));
        assert_eq!(kept(&selection), vec!["BLUS30443DATA"]);
        assert_eq!(
            skipped(&selection),
            vec![("BLUS30443.txt", SkipReason::NotAFolder)]
        );
    }

    /// No title and no ids: nothing can match.
    #[test]
    fn empty_keys_match_nothing() {
        let folders = [RecordFolder::dir("BLUS30443DATA", Some(""))];
        let selection = folders_to_restore(&folders, &FolderMatchKeys::default());
        assert!(selection.keep.is_empty());
    }

    // --- record_folders --------------------------------------------------

    #[test]
    fn record_folders_lists_root_entries_with_their_sfo_titles() {
        use std::io::{Cursor, Write as _};
        use zip::write::SimpleFileOptions;
        use zip::ZipWriter;
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default();
        for (name, body) in [
            (
                "BLUS30443DATA/PARAM.SFO",
                sfo(&[("TITLE", "Demon's Souls")]),
            ),
            ("BLUS30443DATA/SAVE.BIN", b"save".to_vec()),
            ("BCUS98174AUTO/SAVE.BIN", b"other".to_vec()),
            ("loose.txt", b"x".to_vec()),
        ] {
            writer.start_file(name, options).unwrap();
            writer.write_all(&body).unwrap();
        }
        let bytes = writer.finish().unwrap().into_inner();

        assert_eq!(
            record_folders(&bytes).unwrap(),
            vec![
                RecordFolder::dir("BLUS30443DATA", Some("Demon's Souls")),
                RecordFolder::dir("BCUS98174AUTO", None),
                RecordFolder {
                    name: "loose.txt".to_string(),
                    is_dir: false,
                    sfo_title: None,
                },
            ]
        );
    }

    // --- preferred_record ------------------------------------------------

    /// Old polluted uploads: the newest record is another game's folder,
    /// but the game's own record is picked by its file name.
    #[test]
    fn preferred_record_picks_the_newest_record_named_after_the_games_id() {
        let records = vec![
            json!({"id": 1, "emulator": "RPCS3", "file_name": "BLUS30443DATA-2026-01-01T10-00-00.zip",
                   "updated_at": "2026-01-01T10:00:00Z"}),
            json!({"id": 2, "emulator": "RPCS3", "file_name": "BCUS98174AUTO-2026-01-01T10-00-01.zip",
                   "updated_at": "2026-01-01T10:00:01Z"}),
            json!({"id": 3, "emulator": "RPCS3", "file_name": "BLUS30443DATA-2025-12-01T10-00-00.zip",
                   "updated_at": "2025-12-01T10:00:00Z"}),
        ];
        let picked = preferred_record(&records, "RPCS3", &keys_known("BLUS30443")).unwrap();
        assert_eq!(picked["id"], 1);

        // No id, or no record named after it: the plain latest record.
        let picked = preferred_record(&records, "RPCS3", &keys_unknown("Demon's Souls")).unwrap();
        assert_eq!(picked["id"], 2);
        let picked = preferred_record(&records, "RPCS3", &keys_known("NPUB30024")).unwrap();
        assert_eq!(picked["id"], 2);
    }

    // --- notices ---------------------------------------------------------

    #[test]
    fn skipped_notice_lists_at_most_five_names() {
        let selection = FolderSelection {
            keep: vec!["BLUS30443DATA".to_string()],
            skipped: (1..=7)
                .map(|i| (format!("BCUS9817{i}AUTO"), SkipReason::NoMatch))
                .collect(),
        };
        assert_eq!(
            skipped_notice("Flower", &selection),
            "Cloud save for Flower: skipped save folders that could not be matched to this \
             game: BCUS98171AUTO, BCUS98172AUTO, BCUS98173AUTO, BCUS98174AUTO, BCUS98175AUTO and 2 more."
        );
    }
}
