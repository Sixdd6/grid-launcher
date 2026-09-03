// Pure helpers for the Library section's installed grid. Ported from
// game_views.py:297-311 (`is_hidden_library_platform` / `visible_library_games`)
// — the synthetic "Emulators" platform (native-executable games with no real
// platform) never shows up as a library entry of its own, and the grid sorts
// by title then platform, both case- and whitespace-insensitively.
import type { InstalledGame } from './api';

const HIDDEN_PLATFORMS = new Set(['emulator', 'emulators']);

export function isHiddenLibraryPlatform(platform: string): boolean {
  return HIDDEN_PLATFORMS.has(platform.trim().toLowerCase());
}

export function visibleLibraryGames(rows: InstalledGame[]): InstalledGame[] {
  return rows
    .filter((row) => !isHiddenLibraryPlatform(row.platform))
    .slice()
    .sort((a, b) => {
      const titleA = a.title.trim().toLowerCase();
      const titleB = b.title.trim().toLowerCase();
      if (titleA !== titleB) return titleA < titleB ? -1 : 1;
      const platformA = a.platform.trim().toLowerCase();
      const platformB = b.platform.trim().toLowerCase();
      if (platformA !== platformB) return platformA < platformB ? -1 : 1;
      return 0;
    });
}
