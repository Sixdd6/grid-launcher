//! ROM path resolution: picks the on-disk file to hand an emulator for an
//! installed game. See `docs/porting/04-emulator-launch.md` §6
//! (`resolve_rom_path_for_game`, grid_launcher/emulator/launch.py:181).

use std::path::Path;

use crate::library::extract::keeps_romset_archive;
use crate::library::launch_select::select_launch_file;
use crate::library::paths::{archive_name, candidate_archives, candidate_extracted_dirs};
use crate::library::platforms::is_dos_platform;
use crate::library::registry::InstalledGame;

/// Whether `game` launches its archive rather than an extracted file: a
/// ROM-set platform ([`keeps_romset_archive`]) or DOS.
fn launches_archive(game: &InstalledGame) -> bool {
    keeps_romset_archive(&game.platform_slug, &game.platform) || is_dos_platform(&game.platform)
}

/// The launch refusal for an archive-launch game that an older GRID
/// extracted on install: only the unpacked folder is left, and no emulator
/// for the platform can run its loose files. GRID never re-zips the folder.
pub fn reinstall_message(platform: &str) -> String {
    let platform = platform.trim();
    let platform = if platform.is_empty() {
        "these"
    } else {
        platform
    };
    format!(
        "This game's zip was unpacked when it was installed, but {platform} games \
         launch from the zip itself. Uninstall the game and install it again."
    )
}

/// Resolves the ROM/disc-image path to launch `game` with, given the
/// library root.
///
/// Unless the game launches its archive (a ROM-set platform or DOS — see
/// [`launches_archive`]): tries every candidate extracted directory that
/// exists, in order — `select_launch_file(dir, archive_stem)` — returning
/// the first hit; then falls back to `game.extracted_path` when it exists
/// as a file. Either way falls back to the first archive candidate that
/// exists as a file. If nothing on disk matched, returns
/// `game.archive_path`, trimmed. `~` expansion of that final fallback is
/// left to the spawn step, not done here.
///
/// An archive-launch game whose archive is gone fails with
/// [`reinstall_message`] when an extracted folder from an earlier install
/// is still there (a single-file install from before the platform kept its
/// archive). A multi-file install falls back to its recorded launch file.
pub fn resolve_rom_path(game: &InstalledGame, library: &Path) -> Result<String, String> {
    let archive_name = archive_name(&game.rom_file_name, &game.title, &game.platform);
    let archive_candidates = candidate_archives(
        library,
        &game.platform,
        &game.archive_path,
        &archive_name,
        &game.native_game_dir,
    );
    let extracted_dirs = || {
        candidate_extracted_dirs(
            &archive_candidates,
            &game.extracted_dir,
            &game.native_game_dir,
        )
    };
    let launches_archive = launches_archive(game);

    if !launches_archive {
        let archive_stem = Path::new(&archive_name)
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_else(|| archive_name.clone());

        for dir in &extracted_dirs() {
            if !dir.is_dir() {
                continue;
            }
            if let Some(selected) = select_launch_file(dir, &archive_stem) {
                return Ok(selected.to_string_lossy().into_owned());
            }
        }

        let extracted_path = Path::new(&game.extracted_path);
        if extracted_path.is_file() {
            return Ok(game.extracted_path.clone());
        }
    }

    for candidate in &archive_candidates {
        if candidate.is_file() {
            return Ok(candidate.to_string_lossy().into_owned());
        }
    }

    if launches_archive {
        if !game.multi_file_game_dir.trim().is_empty() {
            if Path::new(&game.extracted_path).is_file() {
                return Ok(game.extracted_path.clone());
            }
        } else if extracted_dirs().iter().any(|dir| dir.is_dir()) {
            return Err(reinstall_message(&game.platform));
        }
    }

    Ok(game.archive_path.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn game(title: &str, platform: &str) -> InstalledGame {
        InstalledGame {
            title: title.to_string(),
            platform: platform.to_string(),
            ..Default::default()
        }
    }

    // --- extracted candidates win for a normal platform --------------------

    #[test]
    fn extracted_candidate_wins_over_archive_for_normal_platform() {
        let library = tempfile::tempdir().unwrap();
        let platform_dir = library.path().join("games").join("SNES");
        fs::create_dir_all(&platform_dir).unwrap();

        // Archive also exists on disk, to prove extracted is preferred over
        // it rather than merely being the only thing present.
        fs::write(platform_dir.join("Game.zip"), b"archive bytes").unwrap();

        let extracted_dir = platform_dir.join("Game");
        fs::create_dir_all(&extracted_dir).unwrap();
        let rom_file = extracted_dir.join("Game.chd");
        fs::write(&rom_file, b"rom bytes").unwrap();

        let mut g = game("Some Game", "SNES");
        g.rom_file_name = "Game.zip".to_string();

        let resolved = resolve_rom_path(&g, library.path()).unwrap();
        assert_eq!(resolved, rom_file.to_string_lossy());
    }

    // --- arcade always uses the archive, even when extracted exists --------

    #[test]
    fn arcade_returns_archive_even_when_extracted_exists() {
        let library = tempfile::tempdir().unwrap();
        let platform_dir = library.path().join("games").join("Arcade");
        fs::create_dir_all(&platform_dir).unwrap();

        let archive = platform_dir.join("Game.zip");
        fs::write(&archive, b"archive bytes").unwrap();

        let extracted_dir = platform_dir.join("Game");
        fs::create_dir_all(&extracted_dir).unwrap();
        fs::write(extracted_dir.join("Game.chd"), b"rom bytes").unwrap();

        let mut g = game("Some Game", "Arcade");
        g.rom_file_name = "Game.zip".to_string();

        let resolved = resolve_rom_path(&g, library.path()).unwrap();
        assert_eq!(resolved, archive.to_string_lossy());
    }

    // --- DOS launches the archive: the DOSBox cores mount the zip -----------

    #[test]
    fn dos_returns_archive_even_when_extracted_exists() {
        let library = tempfile::tempdir().unwrap();
        let platform_dir = library.path().join("games").join("DOS");
        fs::create_dir_all(&platform_dir).unwrap();

        let archive = platform_dir.join("Game.zip");
        fs::write(&archive, b"archive bytes").unwrap();

        let extracted_dir = platform_dir.join("Game");
        fs::create_dir_all(&extracted_dir).unwrap();
        fs::write(extracted_dir.join("GAME.EXE"), b"rom bytes").unwrap();

        let mut g = game("Some Game", "DOS");
        g.rom_file_name = "Game.zip".to_string();

        let resolved = resolve_rom_path(&g, library.path()).unwrap();
        assert_eq!(resolved, archive.to_string_lossy());
    }

    // --- dual-shape fallback: games/ vs the legacy platform directory ------

    #[test]
    fn resolve_rom_path_falls_back_to_the_legacy_platform_directory() {
        let library = tempfile::tempdir().unwrap();
        // Only the legacy (pre-v1) `<library>/<Platform>/` archive exists —
        // no `games/` directory at all.
        let legacy_dir = library.path().join("SNES");
        fs::create_dir_all(&legacy_dir).unwrap();
        let archive = legacy_dir.join("Game.zip");
        fs::write(&archive, b"archive bytes").unwrap();

        let mut g = game("Some Game", "SNES");
        g.rom_file_name = "Game.zip".to_string();

        let resolved = resolve_rom_path(&g, library.path()).unwrap();
        assert_eq!(resolved, archive.to_string_lossy());
    }

    #[test]
    fn resolve_rom_path_prefers_the_games_directory() {
        let library = tempfile::tempdir().unwrap();
        let games_dir = library.path().join("games").join("SNES");
        fs::create_dir_all(&games_dir).unwrap();
        let games_archive = games_dir.join("Game.zip");
        fs::write(&games_archive, b"games bytes").unwrap();

        let legacy_dir = library.path().join("SNES");
        fs::create_dir_all(&legacy_dir).unwrap();
        fs::write(legacy_dir.join("Game.zip"), b"legacy bytes").unwrap();

        let mut g = game("Some Game", "SNES");
        g.rom_file_name = "Game.zip".to_string();

        let resolved = resolve_rom_path(&g, library.path()).unwrap();
        assert_eq!(resolved, games_archive.to_string_lossy());
    }

    // --- multi-file row resolves extracted_path (the .m3u) -----------------

    #[test]
    fn multi_file_row_resolves_extracted_path() {
        let library = tempfile::tempdir().unwrap();
        // No platform directory / archive / extracted dir on disk at all —
        // only the recorded extracted_path (the .m3u the installer wrote)
        // exists.
        let m3u = library.path().join("Game.m3u");
        fs::write(&m3u, b"m3u contents").unwrap();

        let mut g = game("Some Game", "PS1");
        g.rom_file_name = "Game.zip".to_string();
        g.extracted_path = m3u.to_string_lossy().into_owned();

        let resolved = resolve_rom_path(&g, library.path()).unwrap();
        assert_eq!(resolved, g.extracted_path);
    }

    // --- raw fallback when nothing exists on disk ---------------------------

    #[test]
    fn raw_archive_path_fallback_when_nothing_exists() {
        let library = tempfile::tempdir().unwrap();

        let mut g = game("Some Game", "SNES");
        g.rom_file_name = "Game.zip".to_string();
        g.archive_path = "  /nowhere/Game.zip  ".to_string();

        let resolved = resolve_rom_path(&g, library.path()).unwrap();
        assert_eq!(resolved, "/nowhere/Game.zip");
    }

    // --- ROM-set platforms launch the zip -----------------------------------

    #[test]
    fn model_3_returns_the_zip_even_when_an_extracted_folder_exists() {
        let library = tempfile::tempdir().unwrap();
        let platform_dir = library.path().join("games").join("Sega Model 3");
        fs::create_dir_all(&platform_dir).unwrap();
        let archive = platform_dir.join("dayto2pe.zip");
        fs::write(&archive, b"archive bytes").unwrap();
        let extracted_dir = platform_dir.join("dayto2pe");
        fs::create_dir_all(&extracted_dir).unwrap();
        fs::write(extracted_dir.join("epr-20985.bin"), b"chip").unwrap();

        let mut g = game("Daytona USA 2: Power Edition", "Sega Model 3");
        g.rom_file_name = "dayto2pe.zip".to_string();
        g.extracted_dir = extracted_dir.to_string_lossy().into_owned();
        g.extracted_path = extracted_dir
            .join("epr-20985.bin")
            .to_string_lossy()
            .into_owned();

        let resolved = resolve_rom_path(&g, library.path()).unwrap();
        assert_eq!(resolved, archive.to_string_lossy());
    }

    #[test]
    fn a_rom_set_extracted_by_an_older_install_asks_for_a_reinstall() {
        let library = tempfile::tempdir().unwrap();
        let platform_dir = library.path().join("games").join("Sega Model 3");
        let extracted_dir = platform_dir.join("dayto2pe");
        fs::create_dir_all(&extracted_dir).unwrap();
        let chip = extracted_dir.join("epr-20985.bin");
        fs::write(&chip, b"chip").unwrap();

        let mut g = game("Daytona USA 2: Power Edition", "Sega Model 3");
        g.rom_file_name = "dayto2pe.zip".to_string();
        g.extracted_dir = extracted_dir.to_string_lossy().into_owned();
        g.extracted_path = chip.to_string_lossy().into_owned();

        assert_eq!(
            resolve_rom_path(&g, library.path()),
            Err(reinstall_message("Sega Model 3"))
        );
        assert!(reinstall_message("Sega Model 3").contains("install it again"));
    }

    #[test]
    fn the_rom_set_rule_also_reads_the_platform_slug() {
        let library = tempfile::tempdir().unwrap();
        let platform_dir = library.path().join("games").join("Model Three");
        let extracted_dir = platform_dir.join("game");
        fs::create_dir_all(&extracted_dir).unwrap();
        fs::write(extracted_dir.join("chip.bin"), b"chip").unwrap();

        let mut g = game("Game", "Model Three");
        g.platform_slug = "model3".to_string();
        g.rom_file_name = "game.zip".to_string();
        g.extracted_dir = extracted_dir.to_string_lossy().into_owned();

        assert!(resolve_rom_path(&g, library.path()).is_err());
    }

    #[test]
    fn a_multi_file_rom_set_install_launches_its_recorded_entry() {
        let library = tempfile::tempdir().unwrap();
        let game_dir = library.path().join("games").join("Arcade").join("Game");
        fs::create_dir_all(&game_dir).unwrap();
        let entry = game_dir.join("game.zip");
        fs::write(&entry, b"zip").unwrap();

        let mut g = game("Game", "Arcade");
        g.rom_file_name = "Game".to_string();
        g.multi_file_game_dir = game_dir.to_string_lossy().into_owned();
        g.extracted_path = entry.to_string_lossy().into_owned();

        let resolved = resolve_rom_path(&g, library.path()).unwrap();
        assert_eq!(resolved, entry.to_string_lossy());
    }
}
