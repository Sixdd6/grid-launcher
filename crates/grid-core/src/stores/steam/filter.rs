//! Which Steam apps are games, and how far an app manifest's install has
//! come (`docs/stores-phase4-plan.md` "Telling games from tools" and
//! "`StateFlags` is a bit set"). Pure: no I/O.

use super::{AppPhase, ManifestState};

/// Tool, runtime and redistributable app ids Steam installs beside games.
/// They never show in the Library (rule 1).
const DENYLIST: &[(u32, &str)] = &[
    (228980, "Steamworks Common Redistributables"),
    (1070560, "Steam Linux Runtime 1.0 (scout)"),
    (1391110, "Steam Linux Runtime 2.0 (soldier)"),
    (1628350, "Steam Linux Runtime 3.0 (sniper)"),
    (4183110, "Steam Linux Runtime 4.0"),
    (1493710, "Proton Experimental"),
    (2180100, "Proton Hotfix"),
    (1826330, "Proton EasyAntiCheat Runtime"),
    (1161040, "Proton BattlEye Runtime"),
    (858280, "Proton, a numbered release"),
    (930400, "Proton, a numbered release"),
    (961940, "Proton, a numbered release"),
    (996510, "Proton, a numbered release"),
    (1054830, "Proton, a numbered release"),
    (1113280, "Proton, a numbered release"),
    (1245040, "Proton, a numbered release"),
    (1420170, "Proton, a numbered release"),
    (1580130, "Proton, a numbered release"),
    (1887720, "Proton, a numbered release"),
    (2348590, "Proton, a numbered release"),
    (2805730, "Proton 9.0"),
    (3658110, "Proton 10.0"),
    (4628710, "Proton 11.0"),
    (221410, "Steam for Linux"),
    (250820, "SteamVR"),
    (323910, "SteamVR Performance Test"),
];

/// Store types (`IStoreBrowseService/GetItems` `type`) that are games:
/// 0 game, 1 demo. Every other type (software, tools, videos, …) is not.
const GAME_TYPES: [u8; 2] = [0, 1];

/// Whether a Steam app is a game GRID shows, by the plan's rules:
/// 1. not on the tool denylist;
/// 2. not named `Proton …` / `Steam Linux Runtime…`, and not installed in a
///    `SteamLinuxRuntime…` / `Proton …` folder;
/// 3. when the store type is known, a game or a demo. Unknown (offline
///    before the first fetch, or no store page) leaves rules 1–2 alone.
pub fn is_game(appid: u32, name: &str, install_dir: &str, kind: Option<u8>) -> bool {
    if DENYLIST.iter().any(|(id, _)| *id == appid) {
        return false;
    }
    let name = name.trim();
    let install_dir = install_dir.trim();
    if name.starts_with("Proton ")
        || name.starts_with("Steam Linux Runtime")
        || install_dir.starts_with("SteamLinuxRuntime")
        || install_dir.starts_with("Proton ")
    {
        return false;
    }
    match kind {
        Some(kind) => GAME_TYPES.contains(&kind),
        None => true,
    }
}

/// `StateFlags` bits GRID reads.
pub mod flags {
    pub const UNINSTALLED: u32 = 1;
    pub const UPDATE_REQUIRED: u32 = 2;
    pub const FULLY_INSTALLED: u32 = 4;
    pub const UPDATE_RUNNING: u32 = 256;
    pub const UPDATE_PAUSED: u32 = 512;
    pub const UPDATE_STARTED: u32 = 1024;
    pub const UNINSTALLING: u32 = 2048;
    pub const PREALLOCATING: u32 = 524_288;
    pub const DOWNLOADING: u32 = 1_048_576;
    pub const STAGING: u32 = 2_097_152;
    pub const COMMITTING: u32 = 4_194_304;
}

/// The bits that mean a first install is under way (or paused).
const INSTALLING: u32 = flags::UPDATE_REQUIRED
    | flags::UPDATE_RUNNING
    | flags::UPDATE_PAUSED
    | flags::UPDATE_STARTED
    | flags::PREALLOCATING
    | flags::DOWNLOADING
    | flags::STAGING
    | flags::COMMITTING;

/// What a manifest's `StateFlags` say: installed (fully installed and not
/// uninstalling; an update queued or running still plays), installing (not
/// fully installed, with an install bit set; a paused first install stays
/// installing), or gone (uninstalling, uninstalled, or nothing else).
pub fn phase(flags: u32) -> AppPhase {
    if flags & flags::UNINSTALLING != 0 {
        AppPhase::Gone
    } else if flags & flags::FULLY_INSTALLED != 0 {
        AppPhase::Installed
    } else if flags & INSTALLING != 0 {
        AppPhase::Installing
    } else {
        AppPhase::Gone
    }
}

/// The install's `(done, total)` bytes: downloaded of to-download, else
/// staged of to-stage, else `(0, 0)`. `done` never passes `total`.
pub fn progress(m: &ManifestState) -> (u64, u64) {
    let (done, total) = if m.bytes_to_download > 0 {
        (m.bytes_downloaded, m.bytes_to_download)
    } else if m.bytes_to_stage > 0 {
        (m.bytes_staged, m.bytes_to_stage)
    } else {
        (0, 0)
    };
    (done.min(total), total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn denylisted_tools_are_never_games() {
        for (id, name) in DENYLIST {
            assert!(!is_game(*id, name, "", None), "{id} {name}");
            assert!(!is_game(*id, "Anything", "anything", Some(0)), "{id}");
        }
    }

    #[test]
    fn proton_and_runtime_patterns_catch_new_releases() {
        assert!(!is_game(9_000_001, "Proton 12.0", "Proton 12.0", None));
        assert!(!is_game(9_000_002, "Steam Linux Runtime 5.0", "x", None));
        assert!(!is_game(9_000_003, "", "SteamLinuxRuntime_5", None));
        assert!(!is_game(9_000_005, "Mystery", "Proton 12.0", Some(0)));
        // A game whose name merely contains the word is kept.
        assert!(is_game(9_000_006, "Protonic Rush", "Protonic Rush", None));
        assert!(is_game(
            9_000_007,
            "The Steam Linux Runtime Story",
            "x",
            None
        ));
    }

    #[test]
    fn the_store_type_decides_when_known() {
        assert!(is_game(620, "Portal 2", "Portal 2", Some(0)));
        assert!(is_game(620, "Portal 2 Demo", "Portal 2 Demo", Some(1)));
        // Wallpaper Engine is software (type 6).
        assert!(!is_game(
            431960,
            "Wallpaper Engine",
            "wallpaper_engine",
            Some(6)
        ));
        assert!(!is_game(1, "A video", "v", Some(13)));
        // Unknown: rules 1–2 alone.
        assert!(is_game(620, "Portal 2", "Portal 2", None));
        assert!(is_game(
            431960,
            "Wallpaper Engine",
            "wallpaper_engine",
            None
        ));
    }

    #[test]
    fn state_flags_read_as_the_plan_says() {
        assert_eq!(phase(4), AppPhase::Installed);
        assert_eq!(phase(6), AppPhase::Installed, "update queued");
        assert_eq!(phase(4 | 2 | 256 | 1024), AppPhase::Installed, "updating");
        assert_eq!(phase(1542), AppPhase::Installed, "update paused");
        assert_eq!(phase(1026), AppPhase::Installing, "first install");
        assert_eq!(phase(1024 | 512 | 2), AppPhase::Installing, "paused");
        assert_eq!(phase(1_048_576), AppPhase::Installing);
        assert_eq!(phase(2048), AppPhase::Gone);
        assert_eq!(phase(4 | 2048), AppPhase::Gone, "uninstalling");
        assert_eq!(phase(1), AppPhase::Gone);
        assert_eq!(phase(0), AppPhase::Gone);
    }

    #[test]
    fn progress_prefers_download_bytes_then_staging() {
        let mut m = ManifestState {
            bytes_to_download: 200,
            bytes_downloaded: 50,
            bytes_to_stage: 400,
            bytes_staged: 300,
            ..Default::default()
        };
        assert_eq!(progress(&m), (50, 200));
        m.bytes_to_download = 0;
        assert_eq!(progress(&m), (300, 400));
        m.bytes_to_stage = 0;
        assert_eq!(progress(&m), (0, 0));
        m.bytes_to_download = 10;
        m.bytes_downloaded = 20;
        assert_eq!(progress(&m), (10, 10), "never past the total");
    }
}
