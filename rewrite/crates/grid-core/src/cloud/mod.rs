//! Cloud save sync engine: shared types used across the `cloud` module.
//!
//! Ported from `grid_launcher/library/{identity,cloud_sync}.py` and the
//! other `grid_launcher/library/cloud_*.py` modules (see
//! `docs/porting/06-cloud-saves.md`).

pub mod state;

/// A kind of cloud-synced save data. `cloud_sync.py` and friends represent
/// this as the plain strings `"save"` / `"state"`; the port uses an enum so
/// call sites are exhaustively checked, with `as_str()` for the two spots
/// that still need the wire string (debug segments, RomM endpoints).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SaveType {
    Save,
    State,
}

impl SaveType {
    pub fn as_str(&self) -> &'static str {
        match self {
            SaveType::Save => "save",
            SaveType::State => "state",
        }
    }
}

/// Plain-data view of a game for cloud logic. Built from an InstalledGame
/// or a server GameSummary; fields the source lacks stay `""`.
///
/// The three id fields below (`title_id`, `base_title_id`, `ps3_game_id`)
/// are a recorded data-availability gap: Python fills them during PS3/Wii-U
/// archive preparation, which the rewrite has not ported as of this
/// milestone. With them blank, the RPCS3/Cemu scanners run with empty token
/// sets, which — per the reference — accept everything: degraded matching,
/// not a crash.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CloudGame {
    pub title: String,
    pub platform: String,
    pub rom_id: String, // string form, "" when absent (Python parity)
    pub rom_file_name: String,
    pub extracted_path: String,
    pub archive_path: String,
    pub description: String,
    pub title_id: String,      // data-availability gap: the rewrite's
    pub base_title_id: String, // registry does not carry these three yet;
    pub ps3_game_id: String,   // token logic ports fully, wiring passes ""
}
