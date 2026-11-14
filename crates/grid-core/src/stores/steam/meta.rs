//! A Steam row's `store_meta` JSON. Never the Web API key: nothing in here
//! is a secret, and the column is never serialized to IPC.
//!
//! Every field defaults, so a row written by an older or newer build reads
//! back; unknown keys are dropped on the next write.

use serde::{Deserialize, Serialize};

/// What GRID keeps about one installed Steam game.
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq, Eq)]
#[serde(default)]
pub struct SteamMeta {
    /// The Steam library folder the game is in (the folder holding
    /// `steamapps/`).
    pub library: String,
    /// The manifest's `installdir` (under `steamapps/common/`).
    pub install_dir: String,
    /// The manifest's `buildid`.
    pub build_id: u64,
    /// Not installed through GRID's Install in Steam (found in Steam's
    /// files): Details says "Installed in Steam, outside GRID".
    pub outside: bool,
}

impl SteamMeta {
    /// The meta stored in a row's `store_meta`; a blank or unreadable value
    /// reads as the default.
    pub fn from_json(text: &str) -> SteamMeta {
        serde_json::from_str(text).unwrap_or_default()
    }

    /// The JSON for a row's `store_meta`.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meta_round_trips_and_tolerates_old_or_blank_values() {
        let meta = SteamMeta {
            library: "/home/sam/.local/share/Steam".into(),
            install_dir: "Portal 2".into(),
            build_id: 12_345,
            outside: true,
        };
        assert_eq!(SteamMeta::from_json(&meta.to_json()), meta);
        assert_eq!(SteamMeta::from_json(""), SteamMeta::default());
        assert_eq!(SteamMeta::from_json("not json"), SteamMeta::default());
        let partial = SteamMeta::from_json(r#"{"build_id": 9, "future": 1}"#);
        assert_eq!(partial.build_id, 9);
        assert!(!partial.outside);
        assert!(!meta.to_json().to_lowercase().contains("key"));
    }
}
