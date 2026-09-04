import { describe, expect, it } from 'vitest';
import type { GameSummary, InstalledGame, RomDetail } from '../api';
import { fromInstalled, fromSummary, mergeDetail, summaryOf } from './subject';

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
    native_executable_path: '',
    native_launch_parameters: '',
    native_compat_tool: '',
    native_wineprefix: '',
    native_game_dir: '',
    included_dlc: '',
    ps3_trophy_paths: '',
    ps3_game_id: '',
    ps3_iso_path: '',
    ps4_game_id: '',
    ps4_content: '',
    ra_id: '',
    last_played_at: 0,
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

describe('fromInstalled', () => {
  it('splits screenshot_urls on newlines, trimming and dropping blank lines', () => {
    const r = row({ screenshot_urls: '  /a/one.png  \n\n/a/two.png\n   \n/a/three.png' });
    expect(fromInstalled(r).screenshotUrls).toEqual(['/a/one.png', '/a/two.png', '/a/three.png']);
  });

  it('produces an empty array for an empty screenshot_urls string', () => {
    expect(fromInstalled(row({ screenshot_urls: '' })).screenshotUrls).toEqual([]);
  });

  it('carries the rest of the row through, tagged as installed', () => {
    const r = row({
      title: 'Chrono Cross',
      platform: 'PS1',
      rom_id: 99,
      cover_small_path: '/c/small.png',
      cover_large_path: '/c/large.png',
      description: 'desc',
      rating: '4.5',
      genres: 'RPG',
    });
    expect(fromInstalled(r)).toEqual({
      romId: 99,
      name: 'Chrono Cross',
      platformName: 'PS1',
      coverSmall: '/c/small.png',
      coverLarge: '/c/large.png',
      screenshotUrls: [],
      description: 'desc',
      rating: '4.5',
      genres: 'RPG',
      source: 'installed',
    });
  });

  it('carries a null rom_id through as romId: null', () => {
    expect(fromInstalled(row({ rom_id: null })).romId).toBeNull();
  });
});

describe('fromSummary', () => {
  it('maps path_cover_small and path_cover_large onto the subject', () => {
    const g = game({ path_cover_small: '/s.png', path_cover_large: '/l.png' });
    const subject = fromSummary(g, 'SNES');
    expect(subject.coverSmall).toBe('/s.png');
    expect(subject.coverLarge).toBe('/l.png');
  });

  it('carries id/name/platformName through, tagged as server', () => {
    const g = game({ id: 7, name: 'Chrono Trigger' });
    expect(fromSummary(g, 'SNES')).toEqual({
      romId: 7,
      name: 'Chrono Trigger',
      platformName: 'SNES',
      coverSmall: null,
      coverLarge: null,
      screenshotUrls: [],
      description: '',
      rating: '',
      genres: '',
      source: 'server',
    });
  });
});

describe('summaryOf', () => {
  it('gives id: 0 for a null rom id', () => {
    const subject = fromInstalled(row({ rom_id: null }));
    expect(summaryOf(subject).id).toBe(0);
  });

  it('round-trips the romId when present', () => {
    const subject = fromSummary(game({ id: 42 }), 'SNES');
    expect(summaryOf(subject).id).toBe(42);
  });

  it('maps name, cover paths, and a fixed platform_id: 0', () => {
    const subject = fromInstalled(row({ title: 'Chrono Trigger', cover_small_path: '/s.png', cover_large_path: '/l.png' }));
    expect(summaryOf(subject)).toEqual({
      id: 42,
      name: 'Chrono Trigger',
      platform_id: 0,
      path_cover_small: '/s.png',
      path_cover_large: '/l.png',
    });
  });
});

function detail(overrides: Partial<RomDetail>): RomDetail {
  return {
    id: 42,
    name: 'Chrono Trigger',
    platform_id: 1,
    platform_name: 'SNES',
    fs_name: 'chrono.sfc',
    description: '',
    regions: '',
    languages: '',
    tags: '',
    revision: '',
    rating: '',
    genres: '',
    companies: '',
    first_release_date: '',
    filesize_bytes: 0,
    server_updated_at: '',
    files: [],
    cover_small_path: '',
    cover_large_path: '',
    screenshot_urls: [],
    ...overrides,
  };
}

describe('mergeDetail', () => {
  const stored = fromInstalled(
    row({
      cover_small_path: '/stored/small.png',
      cover_large_path: '/stored/large.png',
      screenshot_urls: '/stored/one.png\n/stored/two.png',
      description: 'stored description',
      rating: '4.0',
      genres: 'RPG',
    })
  );

  it('keeps the subject covers when the detail has no large cover', () => {
    const merged = mergeDetail(stored, detail({ cover_large_path: '' }));
    expect(merged.coverLarge).toBe('/stored/large.png');
    expect(merged.coverSmall).toBe('/stored/small.png');
  });

  it('overrides the covers when the detail carries them', () => {
    const merged = mergeDetail(
      stored,
      detail({ cover_small_path: '/new/small.png', cover_large_path: '/new/large.png' })
    );
    expect(merged.coverLarge).toBe('/new/large.png');
    expect(merged.coverSmall).toBe('/new/small.png');
  });

  it('normalizes a cover missing on both sides to null, not an empty string', () => {
    const bare = fromInstalled(row({ cover_small_path: '', cover_large_path: '' }));
    const merged = mergeDetail(bare, detail({}));
    expect(merged.coverLarge).toBeNull();
    expect(merged.coverSmall).toBeNull();
  });

  it('keeps the stored screenshots when the detail list is empty', () => {
    const merged = mergeDetail(stored, detail({ screenshot_urls: [] }));
    expect(merged.screenshotUrls).toEqual(['/stored/one.png', '/stored/two.png']);
  });

  it('replaces the screenshots when the detail list is non-empty', () => {
    const merged = mergeDetail(stored, detail({ screenshot_urls: ['/new/a.png'] }));
    expect(merged.screenshotUrls).toEqual(['/new/a.png']);
  });

  it('keeps the stored text fields when the detail sends empty strings', () => {
    const merged = mergeDetail(stored, detail({}));
    expect(merged.description).toBe('stored description');
    expect(merged.rating).toBe('4.0');
    expect(merged.genres).toBe('RPG');
  });

  it('overrides the text fields when the detail carries them', () => {
    const merged = mergeDetail(stored, detail({ description: 'new', rating: '5.0', genres: 'Action' }));
    expect(merged.description).toBe('new');
    expect(merged.rating).toBe('5.0');
    expect(merged.genres).toBe('Action');
  });

  it('carries the subject identity fields through untouched', () => {
    const merged = mergeDetail(stored, detail({ name: 'Renamed', platform_name: 'Elsewhere' }));
    expect(merged.name).toBe(stored.name);
    expect(merged.platformName).toBe(stored.platformName);
    expect(merged.romId).toBe(stored.romId);
    expect(merged.source).toBe('installed');
  });
});
