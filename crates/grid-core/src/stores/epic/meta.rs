//! An Epic row's `store_meta` JSON (registry v12). Never a token: nothing
//! in here is a secret, and the column is never serialized to IPC.
//!
//! Every field defaults, so a row written by an older or newer build reads
//! back; unknown keys are dropped on the next write.

use serde::{Deserialize, Serialize};

/// What GRID keeps about one installed Epic game.
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq, Eq)]
#[serde(default)]
pub struct EpicMeta {
    /// legendary's app name (the row's `store_id` too).
    pub app_name: String,
    /// The installed build version, as legendary gives it
    /// (`list-installed` `version`); an update is found when the owned
    /// list's `build_version` differs.
    pub version: String,
    /// legendary's platform: always `"Windows"` (Epic has no Linux builds).
    pub platform: String,
    /// The installed DLC.
    pub dlc: Vec<EpicDlcMeta>,
    /// Whether Epic cloud saves are on for this game.
    pub cloud_saves: bool,
    /// Epic's `customAttributes.CloudSaveFolder` template (`{AppData}/…`),
    /// `""` when the game has no cloud saves.
    pub cloud_save_folder: String,
    /// The save folder GRID resolved from the template, passed to
    /// legendary as `--save-path`.
    pub save_path: String,
    /// Epoch seconds of the last cloud save sync, `0` for never.
    pub last_sync: i64,
    /// Whether the game may start with `launch --offline`.
    pub can_run_offline: bool,
    /// The user picked the executable in Game Settings; an update keeps it.
    pub exe_user_pinned: bool,
}

/// One installed DLC.
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq, Eq)]
#[serde(default)]
pub struct EpicDlcMeta {
    pub app_name: String,
    pub title: String,
}

impl EpicMeta {
    /// The meta stored in a row's `store_meta`; a blank or unreadable value
    /// reads as the default.
    pub fn from_json(text: &str) -> EpicMeta {
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
    fn meta_round_trips() {
        let meta = EpicMeta {
            app_name: "Starlight".into(),
            version: "++Fortnite+Release-28.10-CL-1234".into(),
            platform: "Windows".into(),
            dlc: vec![EpicDlcMeta {
                app_name: "StarlightDlc1".into(),
                title: "Courier Pack".into(),
            }],
            cloud_saves: true,
            cloud_save_folder: "{AppData}/Starlight/Saved".into(),
            save_path: "/g/prefix/drive_c/users/steamuser/AppData/Local/Starlight/Saved".into(),
            last_sync: 1_700_000_000,
            can_run_offline: true,
            exe_user_pinned: true,
        };
        let json = meta.to_json();
        assert!(json.contains(r#""app_name":"Starlight""#), "{json}");
        assert_eq!(EpicMeta::from_json(&json), meta);
    }

    #[test]
    fn missing_and_bad_meta_read_as_defaults() {
        let meta = EpicMeta::from_json(r#"{"version": "2", "future": 1}"#);
        assert_eq!(meta.version, "2");
        assert!(!meta.cloud_saves);
        assert!(meta.dlc.is_empty());
        assert_eq!(EpicMeta::from_json(""), EpicMeta::default());
        assert_eq!(EpicMeta::from_json("nope"), EpicMeta::default());
    }

    #[test]
    fn meta_never_holds_a_token_field() {
        let json = EpicMeta::default().to_json();
        for word in ["token", "code", "password", "secret"] {
            assert!(!json.contains(word), "{word} in {json}");
        }
    }
}
