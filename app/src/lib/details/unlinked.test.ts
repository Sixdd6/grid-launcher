import { describe, expect, it } from 'vitest';
import type { GameSummary, InstalledGame } from '../api';
import { keptFolder, linkCandidates, removeConfirmMessage, showsUnlinkedActions } from './unlinked';

function game(id: number, name: string, extra: Partial<GameSummary> = {}): GameSummary {
  return {
    id,
    name,
    platform_id: 1,
    path_cover_small: null,
    path_cover_large: null,
    screenshot_urls: [],
    fanart_urls: [],
    platform_display_name: 'Super Nintendo Entertainment System',
    genres: [],
    ...extra,
  } as GameSummary;
}

function row(extra: Partial<InstalledGame> = {}): InstalledGame {
  return {
    title: 'Old Demo',
    platform: 'SNES',
    rom_id: null,
    rom_file_name: '',
    archive_path: '',
    extracted_path: '',
    extracted_dir: '',
    multi_file_game_dir: '',
    native_executable_path: '',
    native_game_dir: '',
    ps3_iso_path: '',
    ...extra,
  } as InstalledGame;
}

describe('showsUnlinkedActions', () => {
  it('is true only for an installed row with no rom id', () => {
    expect(showsUnlinkedActions(null, row())).toBe(true);
    expect(showsUnlinkedActions(7, row({ rom_id: 7 }))).toBe(false);
    // A server subject with no registry row has nothing to link or remove.
    expect(showsUnlinkedActions(null, null)).toBe(false);
  });
});

describe('linkCandidates', () => {
  const games = [
    game(101, 'Super Mario World', { genres: ['Platformer'] }),
    game(102, 'Chrono Trigger (USA)', { genres: ['RPG'] }),
    game(103, 'Secret of Mana', { genres: ['RPG'] }),
  ];

  it('returns every game for a blank query', () => {
    expect(linkCandidates(games, '  ', new Set()).map((g) => g.id)).toEqual([101, 102, 103]);
  });

  it('filters by title, platform or genre like the Server search box', () => {
    expect(linkCandidates(games, 'chrono', new Set()).map((g) => g.id)).toEqual([102]);
    expect(linkCandidates(games, 'rpg', new Set()).map((g) => g.id)).toEqual([102, 103]);
    expect(linkCandidates(games, 'nintendo', new Set()).map((g) => g.id)).toEqual([101, 102, 103]);
  });

  it('hides a rom another installed row already holds', () => {
    expect(linkCandidates(games, '', new Set([102])).map((g) => g.id)).toEqual([101, 103]);
  });
});

describe('keptFolder', () => {
  it('prefers the extracted folder', () => {
    expect(keptFolder(row({ extracted_dir: '/lib/games/SNES/Old Demo', archive_path: '/x/a.zip' }))).toBe(
      '/lib/games/SNES/Old Demo',
    );
  });

  it('falls back to the multi-file and native folders', () => {
    expect(keptFolder(row({ multi_file_game_dir: '/lib/multi' }))).toBe('/lib/multi');
    expect(keptFolder(row({ native_game_dir: 'C:\\Games\\Demo' }))).toBe('C:\\Games\\Demo');
  });

  it('uses the folder of a single file when no folder is stored', () => {
    expect(keptFolder(row({ archive_path: '/lib/games/SNES/Old Demo.zip' }))).toBe('/lib/games/SNES');
    expect(keptFolder(row({ extracted_path: 'C:\\lib\\Old Demo\\demo.sfc' }))).toBe('C:\\lib\\Old Demo');
  });

  it('is empty when the row stores no path', () => {
    expect(keptFolder(row())).toBe('');
  });
});

describe('removeConfirmMessage', () => {
  it('names the folder that stays on disk', () => {
    expect(removeConfirmMessage('Old Demo', '/lib/games/SNES/Old Demo')).toBe(
      'Remove “Old Demo” from your library? Its files stay on disk in /lib/games/SNES/Old Demo.',
    );
  });

  it('still says nothing is deleted when no folder is known', () => {
    expect(removeConfirmMessage('Old Demo', '')).toBe(
      'Remove “Old Demo” from your library? No files are deleted.',
    );
  });
});
