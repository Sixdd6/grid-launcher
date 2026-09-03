import { describe, expect, it } from 'vitest';
import type { GameSummary, InstalledGame } from '../api';
import { matchesInstalled } from './installed.svelte';

function row(overrides: Partial<InstalledGame>): InstalledGame {
  return {
    title: 'Chrono Trigger',
    platform: 'SNES',
    rom_id: 42,
    rom_file_name: 'chrono.sfc',
    archive_path: '',
    extracted_path: '',
    extracted_dir: '',
    multi_file_game_dir: '',
    description: '',
    rating: '',
    genres: '',
    regions: '',
    languages: '',
    tags: '',
    revision: '',
    companies: '',
    first_release_date: '',
    filesize_bytes: 0,
    server_updated_at: '',
    installed_at: 0,
    cover_small_path: '',
    cover_large_path: '',
    screenshot_urls: '',
    ...overrides,
  };
}

function game(overrides: Partial<GameSummary>): GameSummary {
  return {
    id: 42,
    name: 'Chrono Trigger',
    platform_id: 1,
    path_cover_small: null,
    path_cover_large: null,
    ...overrides,
  };
}

describe('matchesInstalled', () => {
  it('matches on rom_id when it is set, regardless of title/platform text', () => {
    const r = row({ rom_id: 42, title: 'Something Else', platform: 'Other' });
    expect(matchesInstalled(r, game({ id: 42 }), 'SNES')).toBe(true);
  });

  it('does not match a different rom_id when identity also differs', () => {
    const r = row({ rom_id: 42, title: 'Chrono Trigger', platform: 'SNES' });
    expect(matchesInstalled(r, game({ id: 99, name: 'Chrono Cross' }), 'SNES')).toBe(false);
  });

  it('falls back to identity match when rom_id is null', () => {
    const r = row({ rom_id: null, title: 'Chrono Trigger', platform: 'SNES' });
    expect(matchesInstalled(r, game({ id: 42 }), 'SNES')).toBe(true);
  });

  it('identity match is case-insensitive', () => {
    const r = row({ rom_id: null, title: 'CHRONO TRIGGER', platform: 'snes' });
    expect(matchesInstalled(r, game({ id: 42, name: 'chrono trigger' }), 'SNES')).toBe(true);
  });

  it('identity match trims surrounding whitespace', () => {
    const r = row({ rom_id: null, title: '  Chrono Trigger  ', platform: ' SNES ' });
    expect(matchesInstalled(r, game({ id: 42, name: 'Chrono Trigger' }), 'SNES')).toBe(true);
  });

  it('does not match when rom_id is null and title differs', () => {
    const r = row({ rom_id: null, title: 'Chrono Cross', platform: 'SNES' });
    expect(matchesInstalled(r, game({ id: 42, name: 'Chrono Trigger' }), 'SNES')).toBe(false);
  });

  it('does not match when rom_id is null and platform differs', () => {
    const r = row({ rom_id: null, title: 'Chrono Trigger', platform: 'Genesis' });
    expect(matchesInstalled(r, game({ id: 42, name: 'Chrono Trigger' }), 'SNES')).toBe(false);
  });

  it('does NOT fall back to identity when rom_id is set but mismatched, even if title/platform match', () => {
    // The rom_id comparison is authoritative once the row has one (docs/porting/
    // 03-library-install.md identity rules) — no identity rescue, otherwise a
    // duplicate-title library would badge/uninstall the wrong game.
    const r = row({ rom_id: 7, title: 'Chrono Trigger', platform: 'SNES' });
    expect(matchesInstalled(r, game({ id: 42, name: 'Chrono Trigger' }), 'SNES')).toBe(false);
  });
});
