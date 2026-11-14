//! External game stores (GOG, Epic, Amazon, Steam): the shared identity
//! model. See `docs/external-stores-plan.md` and SPEC.md "Game stores".
//!
//! Phase 0 holds only the identity types every layer agrees on:
//!
//! - [`StoreKind`]: which store a row or a job belongs to;
//! - [`GameSource`]: where an installed row came from (`romm` or a store),
//!   the registry's `source` column (v11);
//! - [`GameKey`]: one game's identity across launch, sessions and the
//!   queue — `Rom(rom_id)` for a RomM game, `Store { source, store_id }`
//!   for a store product — with a stable string form (`romm:123`,
//!   `gog:1207664643`) that is also its serde form.
//!
//! Phases 1 to 3 add, under this folder: `tools` (the pinned heroic-gogdl,
//! legendary and nile downloads), `runner` (the only place a tool
//! process is spawned), `progress` (tool progress and legendary's and nile's known
//! lines), `paths` (`<data_dir>/stores/<tool>/`, 0700/0600), `art`, the
//! cloud save types every store shares ([`CloudState`], [`SyncMode`],
//! [`SaveConflict`]), and one folder per source: `gog`, `offline` (GOG
//! offline installers), `epic` (Phase 2), `amazon` (Phase 3) and `steam`
//! (Phase 4: no tool; Steam's own files, the Web API and `steam://` links).
//! [`StoreError`] is every store operation's error.
//!
//! Secrets: the tools' own token files live under `<data_dir>/stores/` (the
//! narrow, documented exception in AGENTS.md). Nothing in this module tree
//! may log or forward a tool's stdout or stderr line;
//! `scripts/check_secret_hygiene.sh` checks for it.

use std::fmt;
use std::str::FromStr;

use crate::library::LibraryError;

pub mod amazon;
pub mod art;
pub mod epic;
pub mod gog;
pub mod offline;
pub mod paths;
pub mod progress;
pub mod runner;
pub mod steam;
pub mod tools;

/// Why a store operation failed. The Display text is GRID's own words,
/// never a store tool's output line (AGENTS.md "Secrets").
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("not signed in")]
    NotSignedIn,
    #[error("login expired")]
    LoginExpired,
    #[error("{0} isn't installed")]
    ToolMissing(&'static str),
    #[error("{tool} stopped: {reason}")]
    ToolFailed { tool: &'static str, reason: String },
    #[error("couldn't reach {0}")]
    Network(String),
    #[error("{0}")]
    Refused(String),
    #[error(transparent)]
    Library(#[from] LibraryError),
}

/// The text every Step 0 stub fails with until its work package fills it
/// in (`docs/stores-phase1-plan.md`).
pub const NOT_IMPLEMENTED: &str = "not implemented";

impl StoreError {
    /// The error a Step 0 stub returns.
    pub fn not_implemented() -> StoreError {
        StoreError::Refused(NOT_IMPLEMENTED.to_string())
    }
}

/// One external store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum StoreKind {
    Gog,
    Epic,
    Amazon,
    Steam,
}

impl StoreKind {
    /// Every store, in the order the plan ships them.
    pub const ALL: [StoreKind; 4] = [
        StoreKind::Gog,
        StoreKind::Epic,
        StoreKind::Amazon,
        StoreKind::Steam,
    ];

    /// The stable lowercase id: the registry's `source` value and the
    /// prefix of a [`GameKey`]'s string form.
    pub fn as_str(self) -> &'static str {
        match self {
            StoreKind::Gog => "gog",
            StoreKind::Epic => "epic",
            StoreKind::Amazon => "amazon",
            StoreKind::Steam => "steam",
        }
    }

    /// The platform name a store row carries in the registry (and that the
    /// Library shows): "GOG", "Epic Games", "Amazon Games", "Steam".
    pub fn platform_name(self) -> &'static str {
        match self {
            StoreKind::Gog => "GOG",
            StoreKind::Epic => "Epic Games",
            StoreKind::Amazon => "Amazon Games",
            StoreKind::Steam => "Steam",
        }
    }

    /// Whether GRID launches this store's rows itself, from the native
    /// launch columns (`launch::native::build_native_command`). Steam games
    /// launch through the Steam client instead.
    pub fn launches_natively(self) -> bool {
        !matches!(self, StoreKind::Steam)
    }

    /// The store for a stable id, `None` for anything else (including
    /// `romm`).
    pub fn parse(value: &str) -> Option<StoreKind> {
        StoreKind::ALL
            .into_iter()
            .find(|kind| kind.as_str() == value)
    }
}

impl fmt::Display for StoreKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl serde::Serialize for StoreKind {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for StoreKind {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        StoreKind::parse(&value)
            .ok_or_else(|| serde::de::Error::custom(format!("unknown store {value:?}")))
    }
}

// --- cloud saves (shared by every store with a cloud) ----------------------

/// How a cloud save sync decides its direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncMode {
    /// GRID decides from timestamps; a conflict stops the sync.
    Auto,
    PreferUpload,
    PreferDownload,
}

/// A game's cloud save state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloudState {
    /// Cloud saves are off for the game (or the store has none for it).
    Off,
    /// On, never synced.
    None,
    Synced {
        at: i64,
    },
    Conflict(SaveConflict),
}

/// Both sides of a save conflict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveConflict {
    pub local_at: i64,
    pub local_files: u32,
    pub local_bytes: u64,
    pub cloud_at: i64,
    /// The cloud files in the game's save locations (GOG's cloudstorage
    /// listing); `None` when the store gives no file list (Epic's
    /// `list-saves` names one manifest per save, not its files).
    pub cloud_files: Option<u32>,
    /// Their bytes, `None` when the listing leaves a file's size out.
    pub cloud_bytes: Option<u64>,
}

/// The registry `source` value for a RomM row.
pub const ROMM_SOURCE: &str = "romm";

/// Where an installed row came from: the RomM server, or one store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GameSource {
    #[default]
    Romm,
    Store(StoreKind),
}

impl GameSource {
    /// `"romm"` or the store's id.
    pub fn as_str(self) -> &'static str {
        match self {
            GameSource::Romm => ROMM_SOURCE,
            GameSource::Store(kind) => kind.as_str(),
        }
    }

    /// The source for a stored value, `None` for an unknown one.
    pub fn parse(value: &str) -> Option<GameSource> {
        if value == ROMM_SOURCE {
            return Some(GameSource::Romm);
        }
        StoreKind::parse(value).map(GameSource::Store)
    }

    /// The store, `None` for a RomM row.
    pub fn store(self) -> Option<StoreKind> {
        match self {
            GameSource::Romm => None,
            GameSource::Store(kind) => Some(kind),
        }
    }
}

impl fmt::Display for GameSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl serde::Serialize for GameSource {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for GameSource {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        GameSource::parse(&value)
            .ok_or_else(|| serde::de::Error::custom(format!("unknown game source {value:?}")))
    }
}

/// One game's identity. RomM paths keep their `rom_id`; a store product is
/// its store plus the store's own product id (GOG product id, Epic app
/// name, Amazon product id, Steam app id).
///
/// The string form is stable and is also the serde form: `romm:<rom_id>`
/// or `<store>:<store_id>`. A store id may itself contain `:`; only the
/// first `:` separates.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum GameKey {
    Rom(i64),
    Store { source: StoreKind, store_id: String },
}

impl Default for GameKey {
    /// `Rom(0)`: no real game. Exists so records that embed a key (the
    /// session record) can derive `Default` for tests.
    fn default() -> Self {
        GameKey::Rom(0)
    }
}

impl GameKey {
    /// A store key. `store_id` is trimmed.
    pub fn store(source: StoreKind, store_id: impl Into<String>) -> GameKey {
        GameKey::Store {
            source,
            store_id: store_id.into().trim().to_string(),
        }
    }

    /// The rom id of a RomM key, `None` for a store key.
    pub fn rom_id(&self) -> Option<i64> {
        match self {
            GameKey::Rom(id) => Some(*id),
            GameKey::Store { .. } => None,
        }
    }

    /// The store of a store key, `None` for a RomM key.
    pub fn store_kind(&self) -> Option<StoreKind> {
        match self {
            GameKey::Rom(_) => None,
            GameKey::Store { source, .. } => Some(*source),
        }
    }

    /// The source this key belongs to.
    pub fn source(&self) -> GameSource {
        match self.store_kind() {
            Some(kind) => GameSource::Store(kind),
            None => GameSource::Romm,
        }
    }
}

impl fmt::Display for GameKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GameKey::Rom(id) => write!(f, "{ROMM_SOURCE}:{id}"),
            GameKey::Store { source, store_id } => write!(f, "{source}:{store_id}"),
        }
    }
}

/// Why a string is not a [`GameKey`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("not a game key: {0:?}")]
pub struct GameKeyParseError(pub String);

impl FromStr for GameKey {
    type Err = GameKeyParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let bad = || GameKeyParseError(value.to_string());
        let (source, id) = value.trim().split_once(':').ok_or_else(bad)?;
        let id = id.trim();
        if id.is_empty() {
            return Err(bad());
        }
        if source == ROMM_SOURCE {
            return id.parse::<i64>().map(GameKey::Rom).map_err(|_| bad());
        }
        let source = StoreKind::parse(source).ok_or_else(bad)?;
        Ok(GameKey::store(source, id))
    }
}

impl serde::Serialize for GameKey {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> serde::Deserialize<'de> for GameKey {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_string_form_round_trips() {
        let cases = [
            (GameKey::Rom(123), "romm:123"),
            (
                GameKey::store(StoreKind::Gog, "1207664643"),
                "gog:1207664643",
            ),
            (GameKey::store(StoreKind::Epic, "Fortnite"), "epic:Fortnite"),
            (
                GameKey::store(StoreKind::Amazon, "amzn1.adg:x"),
                "amazon:amzn1.adg:x",
            ),
            (GameKey::store(StoreKind::Steam, "620"), "steam:620"),
        ];
        for (key, text) in cases {
            assert_eq!(key.to_string(), text);
            assert_eq!(text.parse::<GameKey>().unwrap(), key);
            let json = serde_json::to_string(&key).unwrap();
            assert_eq!(json, format!("\"{text}\""));
            assert_eq!(serde_json::from_str::<GameKey>(&json).unwrap(), key);
        }
    }

    #[test]
    fn bad_strings_are_refused() {
        for bad in [
            "", "romm", "romm:", "romm:abc", "itch:5", "gog:", ":5", "123",
        ] {
            assert!(bad.parse::<GameKey>().is_err(), "{bad:?} parsed");
        }
        assert!(serde_json::from_str::<GameKey>("\"nope:1\"").is_err());
    }

    #[test]
    fn keys_answer_their_parts() {
        let rom = GameKey::Rom(7);
        assert_eq!(rom.rom_id(), Some(7));
        assert_eq!(rom.store_kind(), None);
        assert_eq!(rom.source(), GameSource::Romm);
        let gog = GameKey::store(StoreKind::Gog, " 42 ");
        assert_eq!(gog.rom_id(), None);
        assert_eq!(gog.store_kind(), Some(StoreKind::Gog));
        assert_eq!(gog.source(), GameSource::Store(StoreKind::Gog));
        assert_eq!(gog.to_string(), "gog:42", "the id is trimmed");
    }

    #[test]
    fn store_names_and_sources() {
        assert_eq!(StoreKind::Gog.platform_name(), "GOG");
        assert_eq!(StoreKind::Epic.platform_name(), "Epic Games");
        assert_eq!(StoreKind::Amazon.platform_name(), "Amazon Games");
        assert_eq!(StoreKind::Steam.platform_name(), "Steam");
        assert!(StoreKind::Gog.launches_natively());
        assert!(!StoreKind::Steam.launches_natively());
        assert_eq!(GameSource::parse("romm"), Some(GameSource::Romm));
        assert_eq!(
            GameSource::parse("epic"),
            Some(GameSource::Store(StoreKind::Epic))
        );
        assert_eq!(GameSource::parse("ROMM"), None);
        assert_eq!(StoreKind::parse("romm"), None);
        assert_eq!(
            serde_json::to_string(&GameSource::Romm).unwrap(),
            "\"romm\""
        );
        assert_eq!(
            serde_json::to_string(&GameSource::Store(StoreKind::Steam)).unwrap(),
            "\"steam\""
        );
    }
}
