//! ROM path resolution: picks the on-disk file to hand an emulator for an
//! installed game. See `docs/porting/04-emulator-launch.md` §6
//! (`resolve_rom_path_for_game`, grid_launcher/emulator/launch.py:181).

use std::path::Path;

use crate::library::extract::is_arcade_platform;
use crate::library::launch_select::select_launch_file;
use crate::library::paths::{archive_name, candidate_archives, candidate_extracted_dirs};
use crate::library::registry::InstalledGame;

/// Resolves the ROM/disc-image path to launch `game` with, given the
/// library root.
///
/// Unless `game.platform` is arcade: tries every candidate extracted
/// directory that exists, in order — `select_launch_file(dir, archive_stem)`
/// — returning the first hit; then falls back to `game.extracted_path` when
/// it exists as a file. Either way (and always for arcade), falls back to
/// the first archive candidate that exists as a file. If nothing on disk
/// matched, returns `game.archive_path`, trimmed. `~` expansion of that
/// final fallback is left to the spawn step, not done here.
pub fn resolve_rom_path(game: &InstalledGame, library: &Path) -> String {
    let archive_name = archive_name(&game.rom_file_name, &game.title, &game.platform);
    let archive_candidates =
        candidate_archives(library, &game.platform, &game.archive_path, &archive_name);

    if !is_arcade_platform(&game.platform) {
        let archive_stem = Path::new(&archive_name)
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_else(|| archive_name.clone());

        let extracted_dirs = candidate_extracted_dirs(&archive_candidates, &game.extracted_dir);
        for dir in &extracted_dirs {
            if !dir.is_dir() {
                continue;
            }
            if let Some(selected) = select_launch_file(dir, &archive_stem) {
                return selected.to_string_lossy().into_owned();
            }
        }

        let extracted_path = Path::new(&game.extracted_path);
        if extracted_path.is_file() {
            return game.extracted_path.clone();
        }
    }

    for candidate in &archive_candidates {
        if candidate.is_file() {
            return candidate.to_string_lossy().into_owned();
        }
    }

    game.archive_path.trim().to_string()
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
        let platform_dir = library.path().join("SNES");
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

        let resolved = resolve_rom_path(&g, library.path());
        assert_eq!(resolved, rom_file.to_string_lossy());
    }

    // --- arcade always uses the archive, even when extracted exists --------

    #[test]
    fn arcade_returns_archive_even_when_extracted_exists() {
        let library = tempfile::tempdir().unwrap();
        let platform_dir = library.path().join("Arcade");
        fs::create_dir_all(&platform_dir).unwrap();

        let archive = platform_dir.join("Game.zip");
        fs::write(&archive, b"archive bytes").unwrap();

        let extracted_dir = platform_dir.join("Game");
        fs::create_dir_all(&extracted_dir).unwrap();
        fs::write(extracted_dir.join("Game.chd"), b"rom bytes").unwrap();

        let mut g = game("Some Game", "Arcade");
        g.rom_file_name = "Game.zip".to_string();

        let resolved = resolve_rom_path(&g, library.path());
        assert_eq!(resolved, archive.to_string_lossy());
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

        let resolved = resolve_rom_path(&g, library.path());
        assert_eq!(resolved, g.extracted_path);
    }

    // --- raw fallback when nothing exists on disk ---------------------------

    #[test]
    fn raw_archive_path_fallback_when_nothing_exists() {
        let library = tempfile::tempdir().unwrap();

        let mut g = game("Some Game", "SNES");
        g.rom_file_name = "Game.zip".to_string();
        g.archive_path = "  /nowhere/Game.zip  ".to_string();

        let resolved = resolve_rom_path(&g, library.path());
        assert_eq!(resolved, "/nowhere/Game.zip");
    }
}
