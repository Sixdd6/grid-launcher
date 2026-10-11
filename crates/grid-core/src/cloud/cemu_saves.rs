//! Cemu (Wii U) cloud saves: where a record restores, the backup of the
//! files it replaces, and the title id a new upload records.
//!
//! Cemu keeps a title's saves at `<mlc>/usr/save/<high>/<low>/user/<child>`
//! (`child` is an account folder such as `80000001`, or `common`). Cemu
//! builds the path as `usr/save/%08x/%08x/user/…` (lowercase hex) and masks
//! the high half with `& ~0xC` (`nn_save.cpp` `SAVEInit`), so a DLC
//! (`0005000C…`) id shares the base game's `00050000` folder. Cemu runs the
//! base game, never an update, so an update (`0005000E…`) id also names
//! the base game's folder.
//!
//! An upload archives the `user` children with each child as the archive's
//! top folder (`80000001/…`). The archive does not say which title it came
//! from, so a restore places it into the CURRENT game's own `user` folder,
//! found from the game's Wii U title id
//! ([`super::tokens::cemu_game_title_id_tokens`]). This applies to every
//! record, old ones included. Released builds before this fix extracted the
//! record into `<mlc>/usr/save/<child>`, outside every title folder.
//!
//! **Backup.** Before a restore overwrites a local file, the file is copied
//! to `<low>/user.grid-backup-<timestamp>/<same relative path>` beside the
//! `user` folder. Only replaced files are copied; a restore that replaces
//! nothing makes no backup folder.
//!
//! **New uploads** put every child folder of one title into ONE archive and
//! name it after the 16-hex title id (`0005000010145D00-<time>.zip`). The
//! archive layout is unchanged (`<child>/…` at the top), so older builds
//! still read it. A restore prefers the newest record whose file name holds
//! the game's id, and falls back to the newest record otherwise.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use super::archive::{
    archive_top_level_entries, extract_payload_zip_only, payload_zip_file_members,
};
use super::tokens::cemu_game_title_id_tokens;
use super::CloudGame;
use super::IgnoreSets;

/// A Wii U title's save folder: the `<high>/<low>` pair Cemu stores its
/// saves under.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CemuTitle {
    high: u32,
    low: u32,
}

/// `nn_save.cpp` `SAVEInit`'s `& ~0xC` on the high half: a DLC id
/// (`0005000C`) shares the base game's `00050000` folder.
const HIGH_MASK: u32 = !0xC;
/// The high half of a Wii U update title. Cemu runs the base game, never
/// the update, so an update id names the base game's save folder.
const UPDATE_HIGH: u32 = 0x0005_000E;
const GAME_HIGH: u32 = 0x0005_0000;

fn parse_hex8(value: &str) -> Option<u32> {
    if value.len() != 8 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    u32::from_str_radix(value, 16).ok()
}

impl CemuTitle {
    /// The save folder of a 16-hex-digit title id (any case): DLC and
    /// update ids map to the base game's folder. `None` for anything else.
    pub fn from_hex16(id: &str) -> Option<Self> {
        if id.len() != 16 || !id.is_ascii() {
            return None;
        }
        let high = parse_hex8(&id[..8])?;
        let high = if high == UPDATE_HIGH {
            GAME_HIGH
        } else {
            high & HIGH_MASK
        };
        Some(Self {
            high,
            low: parse_hex8(&id[8..])?,
        })
    }

    /// The title of an on-disk `<high>/<low>` save folder pair, taken as
    /// it is. `None` when either name is not 8 hex digits.
    pub fn from_dir_names(high: &str, low: &str) -> Option<Self> {
        Some(Self {
            high: parse_hex8(high)?,
            low: parse_hex8(low)?,
        })
    }

    /// The 16-hex id, uppercase (`0005000010145D00`): the marker a record's
    /// file name carries.
    pub fn id(&self) -> String {
        format!("{:08X}{:08X}", self.high, self.low)
    }

    /// The `<high>` folder name, as Cemu writes it (`00050000`).
    pub fn high_dir(&self) -> String {
        format!("{:08x}", self.high)
    }

    /// The `<low>` folder name, as Cemu writes it (`10145d00`).
    pub fn low_dir(&self) -> String {
        format!("{:08x}", self.low)
    }
}

/// What [`cemu_title_for_game`] found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CemuTitleLookup {
    Found(CemuTitle),
    /// The game has no Wii U title id GRID can read.
    Unknown,
    /// The game names several titles and none (or more than one) has a
    /// save folder on disk.
    Ambiguous(Vec<CemuTitle>),
}

/// The Cemu title a game's saves belong to, from
/// [`super::tokens::cemu_game_title_id_tokens`]. Several ids that mask to
/// one title (base, update, DLC) are one title. When several titles
/// remain, the one with a save folder under `save_roots` wins; otherwise
/// the result is [`CemuTitleLookup::Ambiguous`].
///
/// Only Wii U title ids count (high half `0005xxxx`). The token collector
/// also joins two separate 8-hex runs into a 16-hex token (`[0005000010145D00]
/// [0005000E10145D00]` yields `10145D000005000E`); such a token is not a
/// title id and is dropped here.
pub fn cemu_title_for_game(game: &CloudGame, save_roots: &[PathBuf]) -> CemuTitleLookup {
    let titles: BTreeSet<CemuTitle> = cemu_game_title_id_tokens(game)
        .iter()
        .filter_map(|token| CemuTitle::from_hex16(&token.to_uppercase()))
        .filter(|title| title.high >> 16 == WII_U_TITLE_TYPE)
        .collect();
    let titles: Vec<CemuTitle> = titles.into_iter().collect();
    match titles.as_slice() {
        [] => CemuTitleLookup::Unknown,
        [one] => CemuTitleLookup::Found(*one),
        _ => {
            let on_disk: Vec<CemuTitle> = titles
                .iter()
                .copied()
                .filter(|title| existing_title_dir(save_roots, title).is_some())
                .collect();
            match on_disk.as_slice() {
                [one] => CemuTitleLookup::Found(*one),
                _ => CemuTitleLookup::Ambiguous(titles),
            }
        }
    }
}

/// The high 16 bits every Wii U title id starts with (`0005`).
const WII_U_TITLE_TYPE: u32 = 0x0005;

/// The child of `parent` whose name equals `name` ignoring ASCII case.
fn child_ignoring_case(parent: &Path, name: &str) -> Option<PathBuf> {
    let exact = parent.join(name);
    if exact.is_dir() {
        return Some(exact);
    }
    std::fs::read_dir(parent)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .find(|path| {
            path.is_dir()
                && path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.eq_ignore_ascii_case(name))
        })
}

fn existing_title_dir(save_roots: &[PathBuf], title: &CemuTitle) -> Option<PathBuf> {
    save_roots.iter().find_map(|root| {
        let high = child_ignoring_case(root, &title.high_dir())?;
        child_ignoring_case(&high, &title.low_dir())
    })
}

/// The `<root>/<high>/<low>` folder a restore for `title` writes into: an
/// existing folder (names compared case-insensitively) under the first
/// save root that has one, else a new one under the first save root.
/// `None` when there is no save root.
pub fn cemu_title_dir(save_roots: &[PathBuf], title: &CemuTitle) -> Option<PathBuf> {
    if let Some(found) = existing_title_dir(save_roots, title) {
        return Some(found);
    }
    let root = save_roots.first()?;
    let high =
        child_ignoring_case(root, &title.high_dir()).unwrap_or_else(|| root.join(title.high_dir()));
    Some(high.join(title.low_dir()))
}

/// The title and `<high>/<low>` folder of an upload candidate folder
/// (`<low>/user/<child>`, or `<low>/user` when it has no child).
pub fn cemu_title_of_save_dir(dir: &Path) -> Option<(CemuTitle, PathBuf)> {
    let is_user = |path: &Path| {
        path.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.eq_ignore_ascii_case("user"))
    };
    let user = if is_user(dir) { dir } else { dir.parent()? };
    if !is_user(user) {
        return None;
    }
    let low = user.parent()?;
    let high = low.parent()?;
    let title = CemuTitle::from_dir_names(high.file_name()?.to_str()?, low.file_name()?.to_str()?)?;
    Some((title, low.to_path_buf()))
}

/// What [`place_cemu_payload`] wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CemuPlacement {
    /// Files written.
    pub extracted: usize,
    /// The backup folder, when the restore replaced at least one file.
    pub backup_dir: Option<PathBuf>,
    /// Files copied into `backup_dir`.
    pub backed_up: usize,
}

/// The backup folder name for `stamp` (`user.grid-backup-<stamp>`).
pub fn backup_dir_name(stamp: &str) -> String {
    format!("user.grid-backup-{stamp}")
}

/// A local-time stamp for a backup folder name (`20261010-153000`).
pub fn backup_stamp() -> String {
    chrono::Local::now().format("%Y%m%d-%H%M%S").to_string()
}

/// Extracts a Cemu save record into `title_dir` (`<root>/<high>/<low>`):
/// every top-level folder goes into `title_dir/user/`, except a top-level
/// `user` folder (an upload of a `user` folder with no child), which goes
/// into `title_dir/` itself. Local files the restore replaces are first
/// copied into `title_dir/<backup_dir_name(stamp)>/` (a numbered suffix is
/// added when that folder already exists).
pub fn place_cemu_payload(
    payload: &[u8],
    title_dir: &Path,
    ignore: &IgnoreSets,
    stamp: &str,
) -> Result<CemuPlacement, String> {
    let user_dir = title_dir.join("user");
    let mut into_title: BTreeSet<String> = BTreeSet::new();
    let mut into_user: BTreeSet<String> = BTreeSet::new();
    for (name, _) in archive_top_level_entries(payload)? {
        if name == "user" {
            into_title.insert(name);
        } else {
            into_user.insert(name);
        }
    }
    let groups = [(title_dir, into_title), (user_dir.as_path(), into_user)];

    // The local files the restore replaces, relative to `user`.
    let mut replaced: Vec<(PathBuf, PathBuf)> = Vec::new();
    for (dest, only) in &groups {
        if only.is_empty() {
            continue;
        }
        for member in payload_zip_file_members(payload, ignore, Some(only))? {
            let local = dest.join(&member);
            if !local.is_file() {
                continue;
            }
            let Ok(relative) = local.strip_prefix(&user_dir) else {
                continue;
            };
            replaced.push((local.clone(), relative.to_path_buf()));
        }
    }

    let mut backup_dir = None;
    if !replaced.is_empty() {
        let backup = free_backup_dir(title_dir, stamp);
        for (local, relative) in &replaced {
            let target = backup.join(relative);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("failed to create the backup folder: {e}"))?;
            }
            std::fs::copy(local, &target)
                .map_err(|e| format!("failed to back up an existing save file: {e}"))?;
        }
        backup_dir = Some(backup);
    }

    let mut extracted = 0usize;
    for (dest, only) in &groups {
        if only.is_empty() {
            continue;
        }
        extracted += extract_payload_zip_only(payload, dest, ignore, only)?;
    }

    Ok(CemuPlacement {
        extracted,
        backed_up: if backup_dir.is_some() {
            replaced.len()
        } else {
            0
        },
        backup_dir,
    })
}

/// `title_dir/<backup_dir_name(stamp)>`, or with `-2`, `-3`, … appended
/// when that folder already exists.
fn free_backup_dir(title_dir: &Path, stamp: &str) -> PathBuf {
    let base = backup_dir_name(stamp);
    let mut candidate = title_dir.join(&base);
    let mut n = 2;
    while candidate.exists() {
        candidate = title_dir.join(format!("{base}-{n}"));
        n += 1;
    }
    candidate
}

/// The notice when a Cemu game's title id is unknown.
pub fn unknown_title_notice(game_title: &str) -> String {
    format!(
        "Cloud save for {} was not restored: GRID cannot find the game's Wii U title id, so it does not know which Cemu save folder the save belongs to.",
        display_title(game_title)
    )
}

/// The notice when a Cemu game names several titles.
pub fn ambiguous_title_notice(game_title: &str, titles: &[CemuTitle]) -> String {
    let ids: Vec<String> = titles.iter().map(CemuTitle::id).collect();
    format!(
        "Cloud save for {} was not restored: the game names several Wii U title ids ({}), so GRID does not know which Cemu save folder the save belongs to.",
        display_title(game_title),
        ids.join(", ")
    )
}

/// The notice after a restore that replaced local files, `None` when it
/// replaced none. Names the backup folder relative to the save root.
pub fn backup_notice(
    game_title: &str,
    title: &CemuTitle,
    placement: &CemuPlacement,
) -> Option<String> {
    let backup = placement.backup_dir.as_ref()?;
    let folder = backup.file_name()?.to_string_lossy();
    let files = if placement.backed_up == 1 {
        "1 existing save file was".to_string()
    } else {
        format!("{} existing save files were", placement.backed_up)
    };
    Some(format!(
        "Cloud save for {}: {files} replaced. Copies are in {}/{}/{folder} in the Cemu save folder.",
        display_title(game_title),
        title.high_dir(),
        title.low_dir(),
    ))
}

fn display_title(game_title: &str) -> &str {
    let trimmed = game_title.trim();
    if trimmed.is_empty() {
        "this game"
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    fn zip_of(members: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = ZipWriter::new(std::io::Cursor::new(Vec::new()));
        for (name, bytes) in members {
            writer
                .start_file(*name, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    fn write(path: &Path, bytes: &[u8]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    fn files_under(root: &Path) -> Vec<String> {
        let mut out = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            for entry in fs::read_dir(&dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else {
                    let rel = path.strip_prefix(root).unwrap();
                    out.push(rel.to_string_lossy().replace('\\', "/"));
                }
            }
        }
        out.sort();
        out
    }

    // -- title id -> folder names -----------------------------------------

    #[test]
    fn placement_table_maps_title_ids_to_cemu_folder_names() {
        // (title id as found, high folder, low folder, record marker)
        let table = [
            (
                "0005000010145D00",
                "00050000",
                "10145d00",
                "0005000010145D00",
            ),
            (
                "0005000010145d00",
                "00050000",
                "10145d00",
                "0005000010145D00",
            ),
            // a demo title keeps its own folder
            (
                "0005000210145D00",
                "00050002",
                "10145d00",
                "0005000210145D00",
            ),
            // update (the base game runs) and DLC (& ~0xC) ids share the
            // base game's folder
            (
                "0005000E10145D00",
                "00050000",
                "10145d00",
                "0005000010145D00",
            ),
            (
                "0005000C10145D00",
                "00050000",
                "10145d00",
                "0005000010145D00",
            ),
            (
                "000500001010EC00",
                "00050000",
                "1010ec00",
                "000500001010EC00",
            ),
        ];
        for (id, high, low, marker) in table {
            let title = CemuTitle::from_hex16(id).expect(id);
            assert_eq!(title.high_dir(), high, "{id}");
            assert_eq!(title.low_dir(), low, "{id}");
            assert_eq!(title.id(), marker, "{id}");
        }
        assert_eq!(CemuTitle::from_hex16("0005000010145D0"), None);
        assert_eq!(CemuTitle::from_hex16("0005000010145D0G"), None);
        assert_eq!(CemuTitle::from_hex16("0005000010145D000"), None);
        assert_eq!(
            CemuTitle::from_dir_names("00050000", "10145D00"),
            CemuTitle::from_hex16("0005000010145D00")
        );
        assert_eq!(CemuTitle::from_dir_names("0005000", "10145d00"), None);
    }

    #[test]
    fn title_for_game_reads_the_id_from_the_rom_file_name() {
        let game = CloudGame {
            title: "Some Wii U Game".to_string(),
            rom_file_name: "Some Wii U Game [0005000010145D00].wua".to_string(),
            ..Default::default()
        };
        assert_eq!(
            cemu_title_for_game(&game, &[]),
            CemuTitleLookup::Found(CemuTitle::from_hex16("0005000010145D00").unwrap())
        );
    }

    #[test]
    fn title_for_game_merges_base_and_update_ids() {
        let game = CloudGame {
            rom_file_name: "Game [0005000010145D00] [0005000E10145D00].wua".to_string(),
            ..Default::default()
        };
        assert_eq!(
            cemu_title_for_game(&game, &[]),
            CemuTitleLookup::Found(CemuTitle::from_hex16("0005000010145D00").unwrap())
        );
    }

    #[test]
    fn title_for_game_is_unknown_without_an_id() {
        let game = CloudGame {
            title: "Some Wii U Game".to_string(),
            rom_file_name: "Some Wii U Game.wua".to_string(),
            ..Default::default()
        };
        assert_eq!(cemu_title_for_game(&game, &[]), CemuTitleLookup::Unknown);
    }

    #[test]
    fn title_for_game_reads_the_ids_inside_a_wua() {
        let dir = tempfile::tempdir().unwrap();
        let wua = dir.path().join("Some Wii U Game (USA) (DLC) (v208).wua");
        let names = ["00050000101c9400_v0", "0005000c101c9400_v0", "content"];
        fs::write(&wua, crate::cloud::wua::test_wua(&names)).unwrap();
        let game = CloudGame {
            rom_file_name: "Some Wii U Game (USA) (DLC) (v208).wua".to_string(),
            archive_path: wua.to_string_lossy().into_owned(),
            ..Default::default()
        };
        assert_eq!(
            cemu_title_for_game(&game, &[]),
            CemuTitleLookup::Found(CemuTitle::from_hex16("00050000101C9400").unwrap())
        );
    }

    #[test]
    fn title_for_game_with_two_titles_takes_the_one_with_a_save_folder() {
        let root = tempfile::tempdir().unwrap();
        let game = CloudGame {
            rom_file_name: "Two [0005000010145D00] [000500001010EC00].wua".to_string(),
            ..Default::default()
        };
        let roots = vec![root.path().to_path_buf()];
        let both = cemu_title_for_game(&game, &roots);
        assert_eq!(
            both,
            CemuTitleLookup::Ambiguous(vec![
                CemuTitle::from_hex16("000500001010EC00").unwrap(),
                CemuTitle::from_hex16("0005000010145D00").unwrap(),
            ])
        );
        fs::create_dir_all(root.path().join("00050000/1010ec00/user")).unwrap();
        assert_eq!(
            cemu_title_for_game(&game, &roots),
            CemuTitleLookup::Found(CemuTitle::from_hex16("000500001010EC00").unwrap())
        );
    }

    #[test]
    fn title_dir_reuses_an_existing_folder_of_any_case() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        let title = CemuTitle::from_hex16("0005000010145D00").unwrap();
        let roots = vec![first.path().to_path_buf(), second.path().to_path_buf()];

        // nothing on disk: a new lowercase folder under the first root
        assert_eq!(
            cemu_title_dir(&roots, &title),
            Some(first.path().join("00050000").join("10145d00"))
        );
        // an existing upper-case folder under the second root wins
        fs::create_dir_all(second.path().join("00050000").join("10145D00")).unwrap();
        let found = cemu_title_dir(&roots, &title).unwrap();
        assert_eq!(
            found.parent().unwrap().parent().unwrap(),
            second.path(),
            "{found:?}"
        );
        assert!(found
            .file_name()
            .unwrap()
            .to_string_lossy()
            .eq_ignore_ascii_case("10145d00"));
        assert_eq!(cemu_title_dir(&[], &title), None);
    }

    #[test]
    fn title_of_save_dir_reads_child_and_childless_user_folders() {
        let root = Path::new("mlc/usr/save");
        let title = CemuTitle::from_hex16("0005000010145D00").unwrap();
        let low = root.join("00050000").join("10145d00");
        assert_eq!(
            cemu_title_of_save_dir(&low.join("user").join("80000001")),
            Some((title, low.clone()))
        );
        assert_eq!(
            cemu_title_of_save_dir(&low.join("user").join("common")),
            Some((title, low.clone()))
        );
        assert_eq!(
            cemu_title_of_save_dir(&low.join("user")),
            Some((title, low))
        );
        assert_eq!(cemu_title_of_save_dir(Path::new("x/y/user/80000001")), None);
    }

    // -- placement + backup -------------------------------------------------

    #[test]
    fn a_record_lands_in_the_title_user_folder_without_a_backup_when_nothing_is_replaced() {
        let root = tempfile::tempdir().unwrap();
        let title_dir = root.path().join("00050000").join("10145d00");
        let payload = zip_of(&[
            ("80000001/game.sav", b"cloud-save"),
            ("common/shared.dat", b"cloud-common"),
        ]);

        let placement = place_cemu_payload(
            &payload,
            &title_dir,
            &IgnoreSets::default(),
            "20261010-120000",
        )
        .unwrap();

        assert_eq!(placement.extracted, 2);
        assert_eq!(placement.backup_dir, None);
        assert_eq!(placement.backed_up, 0);
        assert_eq!(
            files_under(root.path()),
            vec![
                "00050000/10145d00/user/80000001/game.sav",
                "00050000/10145d00/user/common/shared.dat",
            ]
        );
        // nothing at <root>/<child>: the old wrong place
        assert!(!root.path().join("80000001").exists());
    }

    #[test]
    fn a_restore_backs_up_exactly_the_files_it_replaces() {
        let root = tempfile::tempdir().unwrap();
        let title_dir = root.path().join("00050000").join("10145d00");
        let user = title_dir.join("user");
        write(&user.join("80000001/game.sav"), b"local-save");
        write(&user.join("80000001/keep.dat"), b"local-only");
        write(&user.join("common/shared.dat"), b"local-common");
        let payload = zip_of(&[
            ("80000001/game.sav", b"cloud-save"),
            ("80000001/new.dat", b"cloud-new"),
            ("common/shared.dat", b"cloud-common"),
        ]);

        let placement = place_cemu_payload(
            &payload,
            &title_dir,
            &IgnoreSets::default(),
            "20261010-120000",
        )
        .unwrap();

        let backup = title_dir.join("user.grid-backup-20261010-120000");
        assert_eq!(placement.extracted, 3);
        assert_eq!(placement.backup_dir.as_deref(), Some(backup.as_path()));
        assert_eq!(placement.backed_up, 2);
        assert_eq!(
            files_under(&backup),
            vec!["80000001/game.sav", "common/shared.dat"]
        );
        assert_eq!(
            fs::read(backup.join("80000001/game.sav")).unwrap(),
            b"local-save"
        );
        assert_eq!(
            fs::read(backup.join("common/shared.dat")).unwrap(),
            b"local-common"
        );
        assert_eq!(
            fs::read(user.join("80000001/game.sav")).unwrap(),
            b"cloud-save"
        );
        assert_eq!(
            fs::read(user.join("80000001/new.dat")).unwrap(),
            b"cloud-new"
        );
        assert_eq!(
            fs::read(user.join("80000001/keep.dat")).unwrap(),
            b"local-only"
        );
    }

    #[test]
    fn a_second_backup_in_the_same_second_gets_its_own_folder() {
        let root = tempfile::tempdir().unwrap();
        let title_dir = root.path().join("00050000").join("10145d00");
        write(&title_dir.join("user/80000001/game.sav"), b"first");
        let payload = zip_of(&[("80000001/game.sav", b"cloud")]);
        let ignore = IgnoreSets::default();

        let one = place_cemu_payload(&payload, &title_dir, &ignore, "S").unwrap();
        let two = place_cemu_payload(&payload, &title_dir, &ignore, "S").unwrap();

        let one = one.backup_dir.unwrap();
        let two = two.backup_dir.unwrap();
        assert_ne!(one, two);
        assert_eq!(fs::read(one.join("80000001/game.sav")).unwrap(), b"first");
        assert_eq!(fs::read(two.join("80000001/game.sav")).unwrap(), b"cloud");
    }

    #[test]
    fn a_childless_user_record_lands_in_the_user_folder_itself() {
        let root = tempfile::tempdir().unwrap();
        let title_dir = root.path().join("00050000").join("10145d00");
        write(&title_dir.join("user/game.sav"), b"local");
        let payload = zip_of(&[("user/game.sav", b"cloud")]);

        let placement =
            place_cemu_payload(&payload, &title_dir, &IgnoreSets::default(), "S").unwrap();

        assert_eq!(fs::read(title_dir.join("user/game.sav")).unwrap(), b"cloud");
        assert!(!title_dir.join("user/user").exists());
        let backup = placement.backup_dir.unwrap();
        assert_eq!(files_under(&backup), vec!["game.sav"]);
    }

    #[test]
    fn ignored_members_are_neither_restored_nor_backed_up() {
        let root = tempfile::tempdir().unwrap();
        let title_dir = root.path().join("00050000").join("10145d00");
        write(&title_dir.join("user/80000001/Thumbs.db"), b"local");
        let payload = zip_of(&[
            ("80000001/game.sav", b"cloud"),
            ("80000001/Thumbs.db", b"cloud-junk"),
        ]);
        let ignore = IgnoreSets {
            basenames: ["thumbs.db".to_string()].into_iter().collect(),
            ..Default::default()
        };

        let placement = place_cemu_payload(&payload, &title_dir, &ignore, "S").unwrap();

        assert_eq!(placement.extracted, 1);
        assert_eq!(placement.backup_dir, None);
        assert_eq!(
            fs::read(title_dir.join("user/80000001/Thumbs.db")).unwrap(),
            b"local"
        );
    }

    #[test]
    fn backup_notice_names_the_folder_relative_to_the_save_root() {
        let title = CemuTitle::from_hex16("0005000010145D00").unwrap();
        let none = CemuPlacement {
            extracted: 1,
            backup_dir: None,
            backed_up: 0,
        };
        assert_eq!(backup_notice("Game", &title, &none), None);
        let some = CemuPlacement {
            extracted: 2,
            backup_dir: Some(PathBuf::from("/x/00050000/10145d00/user.grid-backup-S")),
            backed_up: 2,
        };
        assert_eq!(
            backup_notice("Game", &title, &some).unwrap(),
            "Cloud save for Game: 2 existing save files were replaced. Copies are in 00050000/10145d00/user.grid-backup-S in the Cemu save folder."
        );
    }
}
