import { describe, expect, it } from 'vitest';
import type { GameSummary, InstalledGame } from '../api';
import { fromInstalled, fromSummary, summaryOf } from './subject';

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
