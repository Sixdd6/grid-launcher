//! The gridpack format: `gridpack/gridpack.json` at a drive's root, the index
//! of every game, emulator and firmware set the drive holds. Every path in it
//! is relative to `gridpack/`. Reading, writing, validation and the layout
//! helpers land in Task 1; this file holds the schema.
//!
//! A drive's `gridpack.json` is untrusted input: unknown `os` values and the
//! old prototype shape (games keyed by "0", "1"…) fail to parse.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The folder GRID owns at a drive's root. GRID touches nothing outside it.
pub const PACK_DIR: &str = "gridpack";
/// The index file inside [`PACK_DIR`].
pub const PACK_FILE: &str = "gridpack.json";
/// The format this build writes. A pack with a greater format is read-only.
pub const FORMAT: u32 = 1;
/// Every in-flight file is `<name>.gridtmp` until it is complete.
pub const TMP_SUFFIX: &str = ".gridtmp";
/// The largest file FAT32 can hold.
pub const FAT32_MAX_FILE: u64 = 4_294_967_295;

/// An operating system a pack carries emulators for.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[serde(rename_all = "lowercase")]
pub enum PackOs {
    Linux,
    Windows,
}

impl PackOs {
    /// The OS this build runs on, if a pack can target it.
    pub fn host() -> Option<Self> {
        if cfg!(target_os = "linux") {
            Some(Self::Linux)
        } else if cfg!(windows) {
            Some(Self::Windows)
        } else {
            None
        }
    }

    /// The forge host name: `linux` or `win32`.
    pub fn forge_host(self) -> &'static str {
        match self {
            Self::Linux => "linux",
            Self::Windows => "win32",
        }
    }

    /// The emulator folder tag: `linux-x86_64` or `windows-x86_64`.
    pub fn arch_tag(self) -> &'static str {
        match self {
            Self::Linux => "linux-x86_64",
            Self::Windows => "windows-x86_64",
        }
    }
}

/// What reading a drive's `gridpack.json` found.
#[derive(Clone, PartialEq, Debug)]
pub enum PackState {
    /// No `gridpack/gridpack.json`: the drive is not set up.
    Absent,
    Ready(Pack),
    /// Written by a newer GRID. Shown, never rewritten.
    NewerFormat(u32),
    /// Present but not a pack this build can read.
    Unreadable(String),
}

/// The whole `gridpack.json`.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Pack {
    pub format: u32,
    pub name: String,
    pub os: Vec<PackOs>,
    pub created: String,
    pub updated: String,
    #[serde(default)]
    pub games: Vec<PackGame>,
    #[serde(default)]
    pub emulators: Vec<PackEmulator>,
    #[serde(default)]
    pub firmware: Vec<PackFirmware>,
    /// Keys this build does not know, kept so a rewrite does not drop them.
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// Where a game came from: enough to rebuild its detail and image cache keys.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct PackSource {
    /// The server URL, always without userinfo.
    pub server: String,
    pub rom_id: i64,
    #[serde(default)]
    pub igdb_id: Option<i64>,
    #[serde(default)]
    pub fs_name: String,
    #[serde(default)]
    pub server_updated_at: String,
    #[serde(default)]
    pub cover_small_path: String,
    #[serde(default)]
    pub cover_large_path: String,
    #[serde(default)]
    pub screenshot_urls: Vec<String>,
    #[serde(default)]
    pub fanart_urls: Vec<String>,
    #[serde(default)]
    pub logo_url: String,
}

/// One backed-up game.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct PackGame {
    /// The RomM platform slug.
    pub platform: String,
    pub platform_name: String,
    pub source: PackSource,
    pub name: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub region: String,
    #[serde(default)]
    pub languages: String,
    #[serde(default)]
    pub tags: String,
    #[serde(default)]
    pub revision: String,
    #[serde(default)]
    pub rating: String,
    #[serde(default)]
    pub companies: Vec<String>,
    #[serde(default)]
    pub release_date: String,
    #[serde(default)]
    pub genres: Vec<String>,
    #[serde(default)]
    pub game_modes: Vec<String>,
    #[serde(default)]
    pub franchises: Vec<String>,
    #[serde(default)]
    pub players: String,
    #[serde(default)]
    pub version: String,
    pub size: u64,
    #[serde(default)]
    pub sha1: String,
    pub files: Vec<PackFile>,
    #[serde(default)]
    pub assets: PackAssets,
    pub added: String,
}

/// One file of a game, as the server lists it.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct PackFile {
    pub path: String,
    pub size: u64,
    pub server_file_id: i64,
    pub name: String,
    #[serde(default)]
    pub category: String,
    #[serde(default = "yes")]
    pub top_level: bool,
    #[serde(default)]
    pub md5: String,
    #[serde(default)]
    pub sha1: String,
}

fn yes() -> bool {
    true
}

/// A game's art on the drive.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug, Default)]
pub struct PackAssets {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover_small: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fanart: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo: Option<String>,
    #[serde(default)]
    pub screenshots: Vec<String>,
}

/// One emulator release for one OS.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct PackEmulator {
    /// The emulator profile name.
    pub name: String,
    /// `emulator` or `compat_tool`.
    pub kind: String,
    /// A catalog `owner/repo`, `romm:<folder>/<emu>`, or `local:<name>` for an
    /// archived manual emulator.
    pub source_id: String,
    pub version: String,
    /// `local:` only: the program's path inside the archive.
    #[serde(default)]
    pub program: String,
    /// `local:` only: the launch options.
    #[serde(default)]
    pub args: String,
    /// [`PackOs::arch_tag`].
    pub os: String,
    pub platforms: Vec<String>,
    pub path: String,
    #[serde(default)]
    pub supplementals: Vec<String>,
    pub asset_name: String,
    pub size: u64,
}

/// The firmware of one platform.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct PackFirmware {
    pub platform: String,
    pub platform_name: String,
    pub files: Vec<PackFirmwareFile>,
}

/// One firmware file, as the server serves it.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct PackFirmwareFile {
    pub name: String,
    pub path: String,
    pub size: u64,
    #[serde(default)]
    pub md5: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The spec's draft, with the fields the schema requires filled in, plus an
    /// unknown top-level key.
    const DRAFT: &str = r#"{
      "format": 1,
      "name": "Couch kit",
      "os": ["linux", "windows"],
      "created": "2026-10-12T18:20:00Z",
      "updated": "2026-11-07T09:02:00Z",
      "games": [
        {
          "platform": "genesis", "platform_name": "Sega Genesis",
          "source": { "server": "https://romm.home.lan", "rom_id": 1234, "igdb_id": 1991 },
          "name": "Aladdin", "summary": "A street rat.", "region": "USA",
          "companies": ["Virgin Games"], "release_date": "1993-11-11",
          "genres": ["Platform"], "game_modes": ["Single player"], "players": "1",
          "version": "1.0", "size": 2097152, "sha1": "abc",
          "files": [ { "path": "games/genesis/Aladdin (USA).zip", "size": 2097152, "server_file_id": 5678, "name": "Aladdin (USA).zip", "category": "game" } ],
          "assets": { "cover": "assets/genesis/Aladdin (USA)/cover.jpg", "fanart": "assets/genesis/Aladdin (USA)/fanart.jpg", "logo": "assets/genesis/Aladdin (USA)/logo.png", "screenshots": ["assets/genesis/Aladdin (USA)/screenshots/1.jpg"] },
          "added": "2026-11-07T09:01:00Z"
        }
      ],
      "emulators": [ { "name": "DuckStation", "kind": "emulator", "source_id": "stenzek/duckstation", "version": "v0.1-9226", "os": "linux-x86_64", "platforms": ["psx"], "path": "emulators/duckstation/v0.1-9226/linux-x86_64/DuckStation.AppImage", "asset_name": "DuckStation.AppImage", "size": 40000000 } ],
      "firmware": [ { "platform": "psx", "platform_name": "Sony PlayStation", "files": [ { "name": "scph5501.bin", "path": "firmware/psx/scph5501.bin", "size": 524288, "md5": "490f666e1afb15b7362b406ed1cea246" } ] } ],
      "future_key": { "kept": [1, 2, 3] }
    }"#;

    /// The ClasPlatf example drive's prototype shape: games keyed by "0", a
    /// nested "platform" object, no format.
    const CLASPLATF: &str = r#"{
      "gridpack": { "name": "Classic Platformers", "desc": "A bundle of classic platformers for several platforms." },
      "games": {
        "platform": {
          "name": "genesis", "num-games": "33",
          "games": {
            "0": {
              "name": "Aladdin", "region": "USA", "release-date": "1999-01-01", "players": "1",
              "assets": { "rom": "games/genesis/Aladdin (USA).zip", "cover": "assets/genesis/Aladdin-cover.jpg" }
            }
          }
        }
      }
    }"#;

    #[test]
    fn spec_draft_round_trips_with_unknown_keys() {
        let pack: Pack = serde_json::from_str(DRAFT).expect("draft parses");
        assert_eq!(pack.format, FORMAT);
        assert_eq!(pack.os, vec![PackOs::Linux, PackOs::Windows]);
        let game = &pack.games[0];
        assert_eq!(game.source.rom_id, 1234);
        assert_eq!(game.source.igdb_id, Some(1991));
        assert_eq!(game.files[0].server_file_id, 5678);
        assert!(game.files[0].top_level, "top_level defaults to true");
        assert_eq!(game.assets.cover_small, None);
        assert_eq!(pack.emulators[0].program, "");
        assert_eq!(pack.firmware[0].files[0].size, 524288);
        assert_eq!(
            pack.extra.get("future_key"),
            Some(&serde_json::json!({ "kept": [1, 2, 3] }))
        );

        let written = serde_json::to_value(&pack).expect("serializes");
        assert_eq!(
            written["future_key"],
            serde_json::json!({ "kept": [1, 2, 3] })
        );
        assert!(written.get("id").is_none(), "a pack has no id");
        let again: Pack = serde_json::from_value(written).expect("reparses");
        assert_eq!(again, pack);
    }

    #[test]
    fn clasplatf_prototype_shape_does_not_parse() {
        assert!(serde_json::from_str::<Pack>(CLASPLATF).is_err());
    }

    #[test]
    fn unknown_os_does_not_parse() {
        let bad = DRAFT.replace(r#"["linux", "windows"]"#, r#"["linux", "macos"]"#);
        assert!(serde_json::from_str::<Pack>(&bad).is_err());
    }

    #[test]
    fn os_tags() {
        assert_eq!(PackOs::Linux.forge_host(), "linux");
        assert_eq!(PackOs::Windows.forge_host(), "win32");
        assert_eq!(PackOs::Linux.arch_tag(), "linux-x86_64");
        assert_eq!(PackOs::Windows.arch_tag(), "windows-x86_64");
        assert_eq!(
            serde_json::to_string(&PackOs::Windows).unwrap(),
            "\"windows\""
        );
    }
}
