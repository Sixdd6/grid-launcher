//! A GOG row's `store_meta` JSON (registry v11). Never a token: nothing in
//! here is a secret, and the column is never serialized to IPC.
//!
//! Every field defaults, so a row written by an older or newer build reads
//! back; unknown keys are dropped on the next write.

use serde::{Deserialize, Serialize};

/// What GRID keeps about one installed GOG game.
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq, Eq)]
#[serde(default)]
pub struct GogMeta {
    /// How the game got onto disk.
    pub origin: GogOrigin,
    /// Which version is installed.
    pub platform: GogPlatform,
    /// The version text GOG (or the installer) gives, `"1.4.2"`.
    pub version: String,
    /// The installed build (content system), for the update check.
    pub build_id: String,
    pub language: String,
    /// The offline installer's files (absolute paths: the `.sh`, or the
    /// `setup_*.exe` and its `.bin` parts). Empty unless `origin` is
    /// offline (or an installer restore).
    pub installer: Vec<String>,
    /// The installer files' total size when they were recorded.
    pub installer_bytes: u64,
    /// The installed DLC.
    pub dlc: Vec<GogDlcMeta>,
    /// Whether GOG cloud saves are on for this game.
    pub cloud_saves: bool,
    /// The save folders GOG reports for the game.
    pub save_locations: Vec<SaveLocation>,
    /// Epoch seconds of the last cloud save sync, `0` for never.
    pub last_sync: i64,
    /// The user picked the executable in Game Settings; an update keeps it.
    pub exe_user_pinned: bool,
}

/// How a GOG game was installed.
#[derive(Serialize, Deserialize, Default, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum GogOrigin {
    /// Downloaded with gogdl.
    #[default]
    Gogdl,
    /// From an offline installer file.
    Offline,
    /// Copied back from a backup drive.
    Drive,
}

/// Which version of a GOG game.
#[derive(Serialize, Deserialize, Default, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum GogPlatform {
    #[default]
    Linux,
    Windows,
}

impl GogPlatform {
    /// The registry `platform_slug` a row of this version carries: `linux`
    /// or `win` (the native launch rules key on it).
    pub fn slug(self) -> &'static str {
        match self {
            GogPlatform::Linux => "linux",
            GogPlatform::Windows => "win",
        }
    }
}

/// One installed DLC.
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq, Eq)]
#[serde(default)]
pub struct GogDlcMeta {
    pub id: String,
    pub title: String,
    /// The DLC's offline installer files, like [`GogMeta::installer`].
    pub installer: Vec<String>,
}

/// One save folder GOG reports.
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq, Eq)]
#[serde(default)]
pub struct SaveLocation {
    pub name: String,
    pub path: String,
}

impl GogMeta {
    /// The meta stored in a row's `store_meta`; a blank or unreadable value
    /// reads as the default.
    pub fn from_json(text: &str) -> GogMeta {
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
        let meta = GogMeta {
            origin: GogOrigin::Offline,
            platform: GogPlatform::Windows,
            version: "1.4.2".into(),
            build_id: "5639".into(),
            language: "en-US".into(),
            installer: vec!["/dl/setup_x.exe".into(), "/dl/setup_x-1.bin".into()],
            installer_bytes: 42,
            dlc: vec![GogDlcMeta {
                id: "9".into(),
                title: "Bonus".into(),
                installer: Vec::new(),
            }],
            cloud_saves: true,
            save_locations: vec![SaveLocation {
                name: "saves".into(),
                path: "<?DOCUMENTS?>/x".into(),
            }],
            last_sync: 1_700_000_000,
            exe_user_pinned: true,
        };
        let json = meta.to_json();
        assert!(json.contains(r#""origin":"offline""#), "{json}");
        assert!(json.contains(r#""platform":"windows""#), "{json}");
        assert_eq!(GogMeta::from_json(&json), meta);
    }

    #[test]
    fn missing_and_bad_meta_read_as_defaults() {
        let meta = GogMeta::from_json(r#"{"version": "2", "future": 1}"#);
        assert_eq!(meta.version, "2");
        assert_eq!(meta.origin, GogOrigin::Gogdl);
        assert_eq!(meta.platform, GogPlatform::Linux);
        assert_eq!(GogMeta::from_json(""), GogMeta::default());
        assert_eq!(GogMeta::from_json("nope"), GogMeta::default());
    }

    #[test]
    fn platform_slugs() {
        assert_eq!(GogPlatform::Linux.slug(), "linux");
        assert_eq!(GogPlatform::Windows.slug(), "win");
    }
}
