//! PS3 archive classification and RPCS3 VFS routing.
//!
//! Ports `grid_launcher/library/ps3_install.py` function-for-function; see
//! `docs/porting/03-library-install.md` §11 for the behavior contract. The
//! ISO-only short circuit (moving a lone ISO next to the archive so RPCS3
//! can boot it directly) is the install service's job — this module only
//! reports [`iso_only_file`], and the ISO short circuit inside [`route`]
//! itself is the ordinary `iso_file` classification, extracted and routed
//! recursively.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::bytes::Regex as BytesRegex;
use regex::Regex;

use super::copy_tree_merge;

/// A PS3 game id: four upper-case letters followed by five digits
/// (`BLUS30336`). Matches `ps3_install.py:9`.
static GAME_ID_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Z]{4}[0-9]{5}$").unwrap());
/// A trophy set id: `NPWR` followed by five digits. Matches
/// `ps3_install.py:10`.
static NPWR_TROPHY_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^NPWR[0-9]{5}$").unwrap());
/// Searches anywhere in a string for a game-id-shaped substring. Matches
/// `ps3_install.py:20`.
static GAME_ID_SEARCH_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[A-Z]{4}[0-9]{5}").unwrap());
/// The byte-oriented twin of [`GAME_ID_SEARCH_RE`], run over raw
/// `PARAM.SFO` bytes. Matches `ps3_install.py:300`.
static SFO_GAME_ID_RE: LazyLock<BytesRegex> =
    LazyLock::new(|| BytesRegex::new(r"[A-Z]{4}[0-9]{5}").unwrap());

/// An ISO extraction function: given an ISO path and an empty destination
/// directory, extracts the ISO's contents into that directory or reports
/// an error message. [`crate::library::extract::extract_iso_with_system_7z`]
/// is the production implementation the install service wires in.
pub type IsoExtract<'a> = &'a dyn Fn(&Path, &Path) -> Result<(), String>;

/// The classification assigned to one top-level entry of a PS3 archive's
/// extracted contents. See `ps3_classify_extracted_contents`
/// (`ps3_install.py:71`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ps3Class {
    /// `<ID>/` containing both `PS3_GAME/` and `PS3_DISC.SFB`.
    DiscGameIdDir,
    /// `<ID>/` containing `PS3_GAME/` (or, as a fallback, matching the id
    /// pattern with neither marker present).
    GameIdDir,
    /// `NPWR#####/`.
    TrophyDir,
    /// `PS3_GAME/` at the top level, with no id wrapper.
    BareDiscDir,
    /// A `*.iso` file.
    IsoFile,
    /// `dev_hdd0/` containing `game/` and/or `home/`.
    NestedHdd0Game,
    /// `config/` at the top level.
    ConfigDir,
    /// Anything else.
    Unknown,
}

/// The RPCS3 VFS roots a PS3 install routes into. `games_root` and
/// `data_root` are optional; see [`route`] for the fallbacks used when
/// they are absent.
#[derive(Debug, Clone)]
pub struct Ps3Roots {
    pub dev_hdd0: PathBuf,
    pub games_root: Option<PathBuf>,
    pub data_root: Option<PathBuf>,
}

/// The result of routing a PS3 archive's extracted contents into the VFS.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ps3Outcome {
    pub game_id: String,
    pub installed_paths: Vec<PathBuf>,
    pub trophy_paths_json: String,
    pub extracted_path: String,
    pub extracted_dir: String,
}

// ---------------------------------------------------------------------------
// Shared id helpers
// ---------------------------------------------------------------------------

/// Finds the first game-id-shaped substring of `value` (case-insensitive),
/// or an empty string. Matches `ps3_game_id_from_text` (`ps3_install.py:17`).
pub fn game_id_from_text(value: &str) -> String {
    let upper = value.to_uppercase();
    GAME_ID_SEARCH_RE
        .find(&upper)
        .map(|m| m.as_str().to_string())
        .unwrap_or_default()
}

/// Scans every path component of every path for a game id, skipping any
/// component that is itself trophy-id-shaped (`NPWR#####`). Matches
/// `ps3_game_id_from_paths` (`ps3_install.py:26`).
pub fn game_id_from_paths(paths: &[PathBuf]) -> String {
    for path in paths {
        for part in path.components() {
            let std::path::Component::Normal(os_str) = part else {
                continue;
            };
            let candidate = game_id_from_text(&os_str.to_string_lossy());
            if !candidate.is_empty() && !NPWR_TROPHY_RE.is_match(&candidate) {
                return candidate;
            }
        }
    }
    String::new()
}

/// Tries to read a game id out of a disc dump's `PARAM.SFO`, checked at
/// `<parent>/PS3_GAME/PARAM.SFO` then `<parent>/PARAM.SFO`. Matches
/// `_detect_game_id_from_sfo` (`ps3_install.py:288`).
pub(crate) fn detect_game_id_from_sfo(parent: &Path) -> String {
    let candidates = [
        parent.join("PS3_GAME").join("PARAM.SFO"),
        parent.join("PARAM.SFO"),
    ];
    for sfo_path in candidates {
        if !sfo_path.is_file() {
            continue;
        }
        let Ok(data) = fs::read(&sfo_path) else {
            continue;
        };
        if let Some(m) = SFO_GAME_ID_RE.find(&data) {
            let candidate = String::from_utf8_lossy(m.as_bytes()).into_owned();
            if GAME_ID_RE.is_match(&candidate) {
                return candidate;
            }
        }
    }
    String::new()
}

// ---------------------------------------------------------------------------
// Classification
// ---------------------------------------------------------------------------

fn entry_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// `dev_hdd0.parent`, falling back to `dev_hdd0` itself when it has no
/// parent (matching Python, where `Path("/").parent == Path("/")`).
fn parent_or_self(path: &Path) -> PathBuf {
    path.parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| path.to_path_buf())
}

fn has_ps3_game_content(directory: &Path) -> bool {
    directory.join("PS3_GAME").is_dir()
}

fn is_disc_game_id_directory(directory: &Path) -> bool {
    directory.join("PS3_GAME").is_dir() && directory.join("PS3_DISC.SFB").is_file()
}

/// Directories sort before files; within each group, entries sort by
/// case-folded name. Matches the `sorted(..., key=...)` call in
/// `ps3_classify_extracted_contents` (`ps3_install.py:89`).
fn sort_key(path: &Path) -> (u8, String) {
    let rank = if path.is_dir() { 0 } else { 1 };
    (rank, entry_name(path).to_lowercase())
}

/// Scans `extracted_dir` and classifies each top-level entry. Matches
/// `ps3_classify_extracted_contents` (`ps3_install.py:71`).
pub fn classify(extracted_dir: &Path) -> Vec<(PathBuf, Ps3Class)> {
    let mut entries: Vec<PathBuf> = match fs::read_dir(extracted_dir) {
        Ok(read_dir) => read_dir.filter_map(|e| e.ok()).map(|e| e.path()).collect(),
        Err(_) => return Vec::new(),
    };
    entries.sort_by_key(|a| sort_key(a));

    let mut results = Vec::with_capacity(entries.len());
    for entry in entries {
        let name_upper = entry_name(&entry).to_uppercase();

        if entry.is_file() {
            let is_iso = entry
                .extension()
                .map(|ext| ext.to_string_lossy().to_lowercase() == "iso")
                .unwrap_or(false);
            results.push((
                entry,
                if is_iso {
                    Ps3Class::IsoFile
                } else {
                    Ps3Class::Unknown
                },
            ));
            continue;
        }

        if !entry.is_dir() {
            continue;
        }

        if NPWR_TROPHY_RE.is_match(&name_upper) {
            results.push((entry, Ps3Class::TrophyDir));
            continue;
        }

        if GAME_ID_RE.is_match(&name_upper) && is_disc_game_id_directory(&entry) {
            results.push((entry, Ps3Class::DiscGameIdDir));
            continue;
        }

        if GAME_ID_RE.is_match(&name_upper) && has_ps3_game_content(&entry) {
            results.push((entry, Ps3Class::GameIdDir));
            continue;
        }

        if name_upper == "PS3_GAME" {
            results.push((entry, Ps3Class::BareDiscDir));
            continue;
        }

        if name_upper == "DEV_HDD0" && (entry.join("game").is_dir() || entry.join("home").is_dir())
        {
            results.push((entry, Ps3Class::NestedHdd0Game));
            continue;
        }

        if name_upper == "CONFIG" {
            results.push((entry, Ps3Class::ConfigDir));
            continue;
        }

        if GAME_ID_RE.is_match(&name_upper) {
            results.push((entry, Ps3Class::GameIdDir));
            continue;
        }

        results.push((entry, Ps3Class::Unknown));
    }

    results
}

/// Returns the ISO path when `extracted_dir` contains nothing but a single
/// ISO file — RPCS3 can boot such an ISO directly, so the install service
/// can skip decompressing it into the `dev_hdd0` layout entirely. Matches
/// `ps3_iso_only_extracted_file` (`ps3_install.py:146`).
pub fn iso_only_file(extracted_dir: &Path) -> Option<PathBuf> {
    let classified = classify(extracted_dir);
    if classified.len() != 1 {
        return None;
    }
    let (path, class) = &classified[0];
    if *class != Ps3Class::IsoFile {
        return None;
    }
    Some(path.clone())
}

// ---------------------------------------------------------------------------
// Routing
// ---------------------------------------------------------------------------

/// An error raised while routing, carrying either an I/O failure or the
/// message an [`IsoExtract`] closure returned. Both render the same way —
/// [`route`] wraps whichever occurred in the same
/// `"Failed to install PS3 game <title>: <error>"` message.
enum RouteError {
    Io(io::Error),
    Extract(String),
}

impl fmt::Display for RouteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RouteError::Io(error) => write!(f, "{error}"),
            RouteError::Extract(message) => write!(f, "{message}"),
        }
    }
}

impl From<io::Error> for RouteError {
    fn from(error: io::Error) -> Self {
        RouteError::Io(error)
    }
}

/// Routes `staging`'s classified contents into `roots` and deletes
/// `staging` afterwards. Errors are the verbatim strings documented in
/// doc 03 §11: an empty game id yields
/// `"No PS3 game ID found in archive for <title>"`, and any I/O failure
/// (including one an [`IsoExtract`] closure reports) yields
/// `"Failed to install PS3 game <title>: <error>"`. Matches the
/// post-routing steps in `archive_preparation.py:1197-1229`.
pub fn route(
    staging: &Path,
    roots: &Ps3Roots,
    title: &str,
    iso_extract: IsoExtract,
) -> Result<Ps3Outcome, String> {
    let (game_id, installed_paths) = route_inner(staging, roots, iso_extract)
        .map_err(|error| format!("Failed to install PS3 game {title}: {error}"))?;

    if game_id.is_empty() {
        return Err(format!("No PS3 game ID found in archive for {title}"));
    }

    let extracted = installed_paths
        .iter()
        .find(|path| entry_name(path).to_uppercase() == game_id)
        .cloned()
        .unwrap_or_else(|| roots.dev_hdd0.join("game").join(&game_id));
    let extracted_path = path_string(&extracted);

    let trophy_paths: Vec<String> = installed_paths
        .iter()
        .filter(|path| path_string(path).to_lowercase().contains("trophy"))
        .map(|path| path_string(path))
        .collect();
    let trophy_paths_json =
        serde_json::to_string(&trophy_paths).expect("Vec<String> always serializes");

    // Staging is only ever removed once every other step above has
    // succeeded, mirroring `archive_preparation.py:1197-1229`: the
    // empty-id return and the `Err` mapping above both leave it in place.
    let _ = fs::remove_dir_all(staging);

    Ok(Ps3Outcome {
        game_id,
        installed_paths,
        trophy_paths_json,
        extracted_path: extracted_path.clone(),
        extracted_dir: extracted_path,
    })
}

/// The recursive worker behind [`route`]. Matches
/// `ps3_route_extracted_contents` (`ps3_install.py:165`).
fn route_inner(
    extracted_dir: &Path,
    roots: &Ps3Roots,
    iso_extract: IsoExtract,
) -> Result<(String, Vec<PathBuf>), RouteError> {
    let classified = classify(extracted_dir);
    let mut installed_paths: Vec<PathBuf> = Vec::new();
    let mut game_id = String::new();

    for (item_path, class) in classified {
        match class {
            Ps3Class::DiscGameIdDir => {
                let id = entry_name(&item_path).to_uppercase();
                let destination_root = roots
                    .games_root
                    .clone()
                    .unwrap_or_else(|| roots.dev_hdd0.join("game"));
                let dest = destination_root.join(&id);
                copy_tree_merge(&item_path, &dest)?;
                installed_paths.push(dest);
                if game_id.is_empty() {
                    game_id = id;
                }
            }
            Ps3Class::GameIdDir => {
                let id = entry_name(&item_path).to_uppercase();
                let dest = roots.dev_hdd0.join("game").join(&id);
                copy_tree_merge(&item_path, &dest)?;
                installed_paths.push(dest);
                if game_id.is_empty() {
                    game_id = id;
                }
            }
            Ps3Class::TrophyDir => {
                let dest = roots
                    .dev_hdd0
                    .join("home")
                    .join("00000001")
                    .join("trophy")
                    .join(entry_name(&item_path));
                copy_tree_merge(&item_path, &dest)?;
                installed_paths.push(dest);
            }
            Ps3Class::BareDiscDir => {
                let parent = item_path.parent().unwrap_or(Path::new(""));
                let sfo_id = detect_game_id_from_sfo(parent);
                let synthetic_id = if sfo_id.is_empty() {
                    "PS3_GAME_DISC".to_string()
                } else {
                    sfo_id
                };
                let dest = roots.dev_hdd0.join("game").join(&synthetic_id);
                let inner_dest = dest.join("PS3_GAME");
                copy_tree_merge(&item_path, &inner_dest)?;
                installed_paths.push(dest);
                if game_id.is_empty() {
                    game_id = synthetic_id;
                }
            }
            Ps3Class::IsoFile => {
                let tmp = tempfile::tempdir().map_err(RouteError::Io)?;
                iso_extract(&item_path, tmp.path()).map_err(RouteError::Extract)?;
                let (iso_game_id, iso_paths) = route_inner(tmp.path(), roots, iso_extract)?;
                installed_paths.extend(iso_paths);
                if game_id.is_empty() && !iso_game_id.is_empty() {
                    game_id = iso_game_id;
                }
            }
            Ps3Class::NestedHdd0Game => {
                let game_dir = item_path.join("game");
                if game_dir.is_dir() {
                    // The Python source wraps this whole loop, the copy
                    // included, in `try/except OSError: pass`
                    // (ps3_install.py:238-249) — a failure here aborts the
                    // loop silently instead of failing the install, so
                    // mirror that instead of propagating with `?`.
                    if let Ok(read_dir) = fs::read_dir(&game_dir) {
                        for entry in read_dir {
                            let Ok(entry) = entry else { break };
                            let child = entry.path();
                            if !child.is_dir() {
                                continue;
                            }
                            let id = entry_name(&child).to_uppercase();
                            let dest = roots.dev_hdd0.join("game").join(&id);
                            if copy_tree_merge(&child, &dest).is_err() {
                                break;
                            }
                            installed_paths.push(dest);
                            if game_id.is_empty() && GAME_ID_RE.is_match(&id) {
                                game_id = id;
                            }
                        }
                    }
                }

                let home_dir = item_path.join("home");
                if home_dir.is_dir() {
                    copy_tree_merge(&home_dir, &roots.dev_hdd0.join("home"))?;

                    let trophy_base = home_dir.join("00000001").join("trophy");
                    if trophy_base.is_dir() {
                        if let Ok(read_dir) = fs::read_dir(&trophy_base) {
                            for entry in read_dir {
                                let Ok(entry) = entry else { break };
                                let child = entry.path();
                                if !child.is_dir() {
                                    continue;
                                }
                                let name_upper = entry_name(&child).to_uppercase();
                                if NPWR_TROPHY_RE.is_match(&name_upper) {
                                    let dest = roots
                                        .dev_hdd0
                                        .join("home")
                                        .join("00000001")
                                        .join("trophy")
                                        .join(entry_name(&child));
                                    installed_paths.push(dest);
                                }
                            }
                        }
                    }
                }
            }
            Ps3Class::ConfigDir => {
                let effective_data_root = roots
                    .data_root
                    .clone()
                    .unwrap_or_else(|| parent_or_self(&roots.dev_hdd0));
                let dest = effective_data_root.join("config");
                copy_tree_merge(&item_path, &dest)?;
                installed_paths.push(dest);
            }
            Ps3Class::Unknown => {}
        }
    }

    if game_id.is_empty() {
        game_id = game_id_from_paths(&installed_paths);
    }

    Ok((game_id, installed_paths))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roots(dev_hdd0: &Path) -> Ps3Roots {
        Ps3Roots {
            dev_hdd0: dev_hdd0.to_path_buf(),
            games_root: None,
            data_root: None,
        }
    }

    fn no_iso(_iso: &Path, _dest: &Path) -> Result<(), String> {
        Err("no ISO extraction configured for this test".to_string())
    }

    fn trophy_paths(outcome: &Ps3Outcome) -> Vec<String> {
        serde_json::from_str(&outcome.trophy_paths_json).unwrap()
    }

    // -- classify -------------------------------------------------------

    #[test]
    fn classify_every_class() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        // BCUS99999/ — empty, falls back to GameIdDir.
        fs::create_dir_all(root.join("BCUS99999")).unwrap();
        // BLES01234/ with PS3_GAME/ only — GameIdDir.
        fs::create_dir_all(root.join("BLES01234/PS3_GAME")).unwrap();
        // BLUS30336/ with PS3_GAME/ + PS3_DISC.SFB — DiscGameIdDir.
        fs::create_dir_all(root.join("BLUS30336/PS3_GAME")).unwrap();
        fs::write(root.join("BLUS30336/PS3_DISC.SFB"), b"disc").unwrap();
        // config/ — ConfigDir.
        fs::create_dir_all(root.join("config")).unwrap();
        // dev_hdd0/game/ — NestedHdd0Game.
        fs::create_dir_all(root.join("dev_hdd0/game")).unwrap();
        // misc/ — Unknown directory.
        fs::create_dir_all(root.join("misc")).unwrap();
        // NPWR12345/ — TrophyDir.
        fs::create_dir_all(root.join("NPWR12345")).unwrap();
        // PS3_GAME/ at top level — BareDiscDir.
        fs::create_dir_all(root.join("PS3_GAME")).unwrap();
        // game.iso — IsoFile.
        fs::write(root.join("game.iso"), b"iso").unwrap();
        // readme.txt — Unknown file.
        fs::write(root.join("readme.txt"), b"read me").unwrap();

        let classified = classify(root);
        let names: Vec<(String, Ps3Class)> = classified
            .iter()
            .map(|(p, c)| (entry_name(p), *c))
            .collect();

        assert_eq!(
            names,
            vec![
                ("BCUS99999".to_string(), Ps3Class::GameIdDir),
                ("BLES01234".to_string(), Ps3Class::GameIdDir),
                ("BLUS30336".to_string(), Ps3Class::DiscGameIdDir),
                ("config".to_string(), Ps3Class::ConfigDir),
                ("dev_hdd0".to_string(), Ps3Class::NestedHdd0Game),
                ("misc".to_string(), Ps3Class::Unknown),
                ("NPWR12345".to_string(), Ps3Class::TrophyDir),
                ("PS3_GAME".to_string(), Ps3Class::BareDiscDir),
                ("game.iso".to_string(), Ps3Class::IsoFile),
                ("readme.txt".to_string(), Ps3Class::Unknown),
            ]
        );
    }

    // -- iso_only_file ----------------------------------------------------

    #[test]
    fn iso_only_file_requires_exactly_one_iso_entry() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("game.iso"), b"iso").unwrap();

        assert_eq!(iso_only_file(root), Some(root.join("game.iso")));

        fs::write(root.join("readme.txt"), b"extra").unwrap();
        assert_eq!(iso_only_file(root), None);
    }

    #[test]
    fn iso_only_file_rejects_a_single_non_iso_entry() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("readme.txt"), b"only file").unwrap();

        assert_eq!(iso_only_file(root), None);
    }

    // -- id helpers ---------------------------------------------------------

    #[test]
    fn game_id_from_text_and_paths() {
        assert_eq!(game_id_from_text("blus30336"), "BLUS30336");
        assert_eq!(game_id_from_text("prefix-BLUS30336-suffix"), "BLUS30336");
        assert_eq!(game_id_from_text("no id here"), "");

        let paths = vec![
            PathBuf::from("/staging/NPWR12345/data.bin"),
            PathBuf::from("/staging/BLUS30336/PS3_GAME"),
        ];
        assert_eq!(game_id_from_paths(&paths), "BLUS30336");

        let only_trophy = vec![PathBuf::from("/staging/NPWR12345")];
        assert_eq!(game_id_from_paths(&only_trophy), "");

        assert_eq!(game_id_from_paths(&[]), "");
    }

    // -- route --------------------------------------------------------------

    #[test]
    fn route_disc_and_game_id_dirs_into_roots() {
        let dir = tempfile::tempdir().unwrap();
        let staging = dir.path().join("staging");
        let games_root = dir.path().join("games");
        let dev_hdd0 = dir.path().join("dev_hdd0");

        // Named so the disc dir sorts (and so routes, and sets the game
        // id) before the plain game-id dir.
        fs::create_dir_all(staging.join("BLUS30336/PS3_GAME")).unwrap();
        fs::write(staging.join("BLUS30336/PS3_DISC.SFB"), b"disc").unwrap();
        fs::create_dir_all(staging.join("BLUS99999/PS3_GAME")).unwrap();

        let roots = Ps3Roots {
            dev_hdd0: dev_hdd0.clone(),
            games_root: Some(games_root.clone()),
            data_root: None,
        };

        let outcome = route(&staging, &roots, "Foo", &no_iso).unwrap();

        assert_eq!(outcome.game_id, "BLUS30336");
        assert!(games_root.join("BLUS30336/PS3_GAME").is_dir());
        assert!(dev_hdd0.join("game/BLUS99999/PS3_GAME").is_dir());
        assert_eq!(
            outcome.extracted_dir,
            path_string(&games_root.join("BLUS30336"))
        );
        assert_eq!(outcome.extracted_path, outcome.extracted_dir);
        assert!(!staging.exists());
    }

    #[test]
    fn route_trophy_and_nested_hdd0() {
        let dir = tempfile::tempdir().unwrap();
        let staging = dir.path().join("staging");
        let dev_hdd0 = dir.path().join("dev_hdd0");

        fs::create_dir_all(staging.join("NPWR12345")).unwrap();
        fs::write(staging.join("NPWR12345/TROPUSR.DAT"), b"trophy").unwrap();

        fs::create_dir_all(staging.join("dev_hdd0/game/BLUS00001")).unwrap();
        fs::write(staging.join("dev_hdd0/game/BLUS00001/x"), b"game data").unwrap();
        fs::create_dir_all(staging.join("dev_hdd0/home/00000001/trophy/NPWR00002")).unwrap();
        fs::write(
            staging.join("dev_hdd0/home/00000001/trophy/NPWR00002/y"),
            b"nested trophy",
        )
        .unwrap();

        let roots = roots(&dev_hdd0);
        let outcome = route(&staging, &roots, "Foo", &no_iso).unwrap();

        assert_eq!(outcome.game_id, "BLUS00001");
        assert!(dev_hdd0.join("game/BLUS00001/x").is_file());
        assert!(dev_hdd0.join("home/00000001/trophy/NPWR00002/y").is_file());
        assert!(dev_hdd0
            .join("home/00000001/trophy/NPWR12345/TROPUSR.DAT")
            .is_file());

        let nested_dest = path_string(&dev_hdd0.join("home/00000001/trophy/NPWR00002"));
        let top_dest = path_string(&dev_hdd0.join("home/00000001/trophy/NPWR12345"));
        assert_eq!(trophy_paths(&outcome), vec![nested_dest, top_dest]);
        assert!(outcome
            .installed_paths
            .contains(&dev_hdd0.join("home/00000001/trophy/NPWR12345")));
        assert!(outcome
            .installed_paths
            .contains(&dev_hdd0.join("home/00000001/trophy/NPWR00002")));
    }

    #[test]
    fn route_bare_disc_synthesizes_id_from_sfo() {
        let dir = tempfile::tempdir().unwrap();
        let staging = dir.path().join("staging");
        let dev_hdd0 = dir.path().join("dev_hdd0");

        fs::create_dir_all(staging.join("PS3_GAME")).unwrap();
        fs::write(staging.join("PS3_GAME/EBOOT.BIN"), b"eboot").unwrap();
        let mut sfo = b"\x00\x00PARAM\x00\x00".to_vec();
        sfo.extend_from_slice(b"BLUS30336");
        sfo.extend_from_slice(b"\x00\x00tail\x00\x00");
        fs::write(staging.join("PS3_GAME/PARAM.SFO"), &sfo).unwrap();

        let roots = roots(&dev_hdd0);
        let outcome = route(&staging, &roots, "Foo", &no_iso).unwrap();

        assert_eq!(outcome.game_id, "BLUS30336");
        assert!(dev_hdd0.join("game/BLUS30336/PS3_GAME/EBOOT.BIN").is_file());
    }

    #[test]
    fn route_bare_disc_without_sfo_uses_placeholder() {
        let dir = tempfile::tempdir().unwrap();
        let staging = dir.path().join("staging");
        let dev_hdd0 = dir.path().join("dev_hdd0");

        fs::create_dir_all(staging.join("PS3_GAME")).unwrap();
        fs::write(staging.join("PS3_GAME/EBOOT.BIN"), b"eboot").unwrap();

        let roots = roots(&dev_hdd0);
        let outcome = route(&staging, &roots, "Foo", &no_iso).unwrap();

        assert_eq!(outcome.game_id, "PS3_GAME_DISC");
        assert!(dev_hdd0
            .join("game/PS3_GAME_DISC/PS3_GAME/EBOOT.BIN")
            .is_file());
    }

    #[test]
    fn route_config_dir_uses_data_root_when_set() {
        let dir = tempfile::tempdir().unwrap();
        let staging = dir.path().join("staging");
        let dev_hdd0 = dir.path().join("dev_hdd0");
        let data_root = dir.path().join("rpcs3_data");

        fs::create_dir_all(staging.join("BLUS30336/PS3_GAME")).unwrap();
        fs::create_dir_all(staging.join("config")).unwrap();
        fs::write(staging.join("config/curr_conf.yml"), b"cfg").unwrap();

        let roots = Ps3Roots {
            dev_hdd0: dev_hdd0.clone(),
            games_root: None,
            data_root: Some(data_root.clone()),
        };
        let outcome = route(&staging, &roots, "Foo", &no_iso).unwrap();

        assert!(data_root.join("config/curr_conf.yml").is_file());
        assert!(outcome.installed_paths.contains(&data_root.join("config")));
    }

    #[test]
    fn route_config_dir_falls_back_to_dev_hdd0_parent() {
        let dir = tempfile::tempdir().unwrap();
        let staging = dir.path().join("staging");
        let dev_hdd0 = dir.path().join("vfs/dev_hdd0");

        fs::create_dir_all(staging.join("BLUS30336/PS3_GAME")).unwrap();
        fs::create_dir_all(staging.join("config")).unwrap();
        fs::write(staging.join("config/curr_conf.yml"), b"cfg").unwrap();

        let roots = roots(&dev_hdd0);
        let outcome = route(&staging, &roots, "Foo", &no_iso).unwrap();

        let expected_dest = dev_hdd0.parent().unwrap().join("config");
        assert!(expected_dest.join("curr_conf.yml").is_file());
        assert!(outcome.installed_paths.contains(&expected_dest));
    }

    #[test]
    fn route_iso_entry_uses_the_extractor() {
        let dir = tempfile::tempdir().unwrap();
        let staging = dir.path().join("staging");
        let dev_hdd0 = dir.path().join("dev_hdd0");
        fs::create_dir_all(&staging).unwrap();
        fs::write(staging.join("game.iso"), b"iso bytes").unwrap();

        let extractor = |_iso: &Path, dest: &Path| -> Result<(), String> {
            fs::create_dir_all(dest.join("BLUS30336/PS3_GAME/USRDIR")).unwrap();
            fs::write(dest.join("BLUS30336/PS3_GAME/USRDIR/EBOOT.BIN"), b"eboot").unwrap();
            Ok(())
        };

        let roots = roots(&dev_hdd0);
        let outcome = route(&staging, &roots, "Foo", &extractor).unwrap();

        assert_eq!(outcome.game_id, "BLUS30336");
        assert!(dev_hdd0
            .join("game/BLUS30336/PS3_GAME/USRDIR/EBOOT.BIN")
            .is_file());
    }

    #[test]
    fn route_iso_entry_propagates_extractor_error() {
        let dir = tempfile::tempdir().unwrap();
        let staging = dir.path().join("staging");
        let dev_hdd0 = dir.path().join("dev_hdd0");
        fs::create_dir_all(&staging).unwrap();
        fs::write(staging.join("game.iso"), b"iso bytes").unwrap();

        let failing = |_iso: &Path, _dest: &Path| -> Result<(), String> {
            Err("7z: bad archive".to_string())
        };

        let roots = roots(&dev_hdd0);
        let error = route(&staging, &roots, "Foo", &failing).unwrap_err();

        assert_eq!(error, "Failed to install PS3 game Foo: 7z: bad archive");
    }

    #[test]
    fn route_without_game_id_fails() {
        let dir = tempfile::tempdir().unwrap();
        let staging = dir.path().join("staging");
        let dev_hdd0 = dir.path().join("dev_hdd0");
        fs::create_dir_all(&staging).unwrap();
        fs::create_dir_all(staging.join("misc")).unwrap();
        fs::write(staging.join("readme.txt"), b"nothing useful").unwrap();

        let roots = roots(&dev_hdd0);
        let error = route(&staging, &roots, "Foo", &no_iso).unwrap_err();

        assert_eq!(error, "No PS3 game ID found in archive for Foo");
        // The empty-id path returns before staging is deleted.
        assert!(staging.exists());
    }

    #[test]
    fn route_scans_installed_paths_for_id_skipping_npwr() {
        let dir = tempfile::tempdir().unwrap();
        let staging = dir.path().join("staging");
        let dev_hdd0 = dir.path().join("dev_hdd0");

        // Only a trophy dir: the sole installed path is
        // dev_hdd0/home/00000001/trophy/NPWR12345, which is
        // game-id-shaped but must be skipped as a trophy id.
        fs::create_dir_all(staging.join("NPWR12345")).unwrap();
        fs::write(staging.join("NPWR12345/TROPUSR.DAT"), b"trophy").unwrap();

        let roots = roots(&dev_hdd0);
        let error = route(&staging, &roots, "Foo", &no_iso).unwrap_err();

        assert_eq!(error, "No PS3 game ID found in archive for Foo");
        assert!(dev_hdd0
            .join("home/00000001/trophy/NPWR12345/TROPUSR.DAT")
            .is_file());
    }
}
