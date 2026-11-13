import { describe, expect, it } from 'vitest';
import type { DriveGame, GameSummary, InstalledGame, RomDetail } from '../api';
import { parseDriveAssetUrl } from '../backups/driveArt';
import {
  detailApplies,
  driveCopyLines,
  fromDriveGame,
  fromDriveText,
  fromInstalled,
  fromSummary,
  mergeDetail,
  releaseSeconds,
  summaryOf,
  versionSource,
} from './subject';

function row(overrides: Partial<InstalledGame>): InstalledGame {
  return {
    title: 'Chrono Trigger',
    platform: 'SNES',
    platform_slug: '',
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
    fanart_urls: '',
    source: 'romm', store_id: '', native_executable_path: '',
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
    screenshot_urls: [],
    fanart_urls: [],
    platform_display_name: '',
    genres: [],
    created_at: '',
    first_release_date: null,
    logo_url: '',
    fs_size_bytes: 0,
    largest_file_bytes: 0,
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
      platform_slug: 'psx',
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
      platformSlug: 'psx',
      coverSmall: '/c/small.png',
      coverLarge: '/c/large.png',
      screenshotUrls: [],
      fanartUrls: [],
      description: 'desc',
      rating: '4.5',
      genres: 'RPG',
      source: 'installed',
    });
  });

  it('splits the registry row\'s newline-joined fanart column', () => {
    const subject = fromInstalled(row({ fanart_urls: 'https://romm/f1.jpg\n\n https://romm/f2.jpg ' }));
    expect(subject.fanartUrls).toEqual(['https://romm/f1.jpg', 'https://romm/f2.jpg']);
  });

  it('carries a null rom_id through as romId: null', () => {
    expect(fromInstalled(row({ rom_id: null })).romId).toBeNull();
  });
});

describe('fromSummary', () => {
  it('maps path_cover_small and path_cover_large onto the subject', () => {
    const g = game({ path_cover_small: '/s.png', path_cover_large: '/l.png' });
    const subject = fromSummary(g, 'SNES', 'snes');
    expect(subject.coverSmall).toBe('/s.png');
    expect(subject.coverLarge).toBe('/l.png');
  });

  it('carries id/name/platformName through, tagged as server', () => {
    const g = game({ id: 7, name: 'Chrono Trigger' });
    expect(fromSummary(g, 'SNES', 'snes')).toEqual({
      romId: 7,
      name: 'Chrono Trigger',
      platformName: 'SNES',
      platformSlug: 'snes',
      coverSmall: null,
      coverLarge: null,
      screenshotUrls: [],
      fanartUrls: [],
      description: '',
      rating: '',
      genres: '',
      source: 'server',
    });
  });

  it('carries the summary\'s own screenshots and fanart', () => {
    const subject = fromSummary(
      game({ screenshot_urls: ['https://romm/s1.png'], fanart_urls: ['https://romm/f1.jpg'] }),
      'SNES',
      'snes'
    );
    expect(subject.screenshotUrls).toEqual(['https://romm/s1.png']);
    expect(subject.fanartUrls).toEqual(['https://romm/f1.jpg']);
  });
});

describe('summaryOf', () => {
  it('gives id: 0 for a null rom id', () => {
    const subject = fromInstalled(row({ rom_id: null }));
    expect(summaryOf(subject).id).toBe(0);
  });

  it('round-trips the romId when present', () => {
    const subject = fromSummary(game({ id: 42 }), 'SNES', 'snes');
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
      screenshot_urls: [],
      fanart_urls: [],
      platform_display_name: '',
      genres: [],
      created_at: '',
      first_release_date: null,
      logo_url: '',
      fs_size_bytes: 0,
      largest_file_bytes: 0,
    });
  });
});

function detail(overrides: Partial<RomDetail>): RomDetail {
  return {
    id: 42,
    name: 'Chrono Trigger',
    platform_id: 1,
    platform_name: 'SNES',
    platform_slug: '',
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
    franchises: '',
    game_modes: '',
    player_count: '',
    filesize_bytes: 0,
    server_updated_at: '',
    files: [],
    cover_small_path: '',
    cover_large_path: '',
    screenshot_urls: [],
    fanart_urls: [],
    youtube_video_id: '',
    video_path: '',
    is_identified: false,
    related: [],
    achievement_count: 0,
    logo_url: '',
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

  it('keeps the subject\'s fanart when the detail has none', () => {
    const merged = mergeDetail(
      { ...stored, fanartUrls: ['https://romm/f1.jpg'] },
      detail({ fanart_urls: [] })
    );
    expect(merged.fanartUrls).toEqual(['https://romm/f1.jpg']);
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

function driveGame(overrides: Partial<DriveGame> = {}): DriveGame {
  return {
    rom_id: 7, server: 'https://romm.home.lan', name: 'Cinder Peaks', platform: 'psx', platform_name: 'PlayStation',
    size: 700, files_max: 700, cover: 'assets/psx/Cinder Peaks/cover.jpg', installed: false, summary: 'A climb.',
    region: 'USA', languages: '', tags: '', revision: '', rating: '82', companies: ['Ember', 'Soot'],
    release_date: '1998-03-01', genres: ['Platform', 'Puzzle'], game_modes: [], franchises: [], players: '1',
    version: '', fanart: 'assets/psx/Cinder Peaks/fanart.jpg', logo: '',
    screenshots: ['assets/psx/Cinder Peaks/screenshots/screenshot-01.jpg'], added: '2026-11-07T09:01:00Z',
    installed_copy: false, installed_version: '', outdated: false, saves_copied: false,
    ...overrides,
  };
}

const t7 = { key: '/run/media/me/T7', label: 'SAMSUNG T7', mount: '/run/media/me/T7', pack: { name: 'Couch kit' } } as const;

describe('fromDriveGame (GameBackups README, "Details from a drive")', () => {
  // A cast: only the pack's name is read.
  const drive = t7 as unknown as Parameters<typeof fromDriveGame>[1];

  it('names the drive it came from: "From Couch kit"', () => {
    const subject = fromDriveGame(driveGame(), drive);
    expect(subject.source).toBe('drive');
    expect(subject.drive).toEqual({ key: t7.key, name: 'Couch kit', label: 'SAMSUNG T7', server: 'https://romm.home.lan' });
    expect(fromDriveText(subject)).toBe('From Couch kit');
    expect(fromDriveText(fromInstalled(row({})))).toBe('');
  });

  it('takes the art from the drive, through drive_asset', () => {
    const subject = fromDriveGame(driveGame(), drive);
    expect(parseDriveAssetUrl(subject.coverLarge!)).toEqual({ key: t7.key, rel: 'assets/psx/Cinder Peaks/cover.jpg' });
    expect(subject.coverSmall).toBe(subject.coverLarge);
    expect(subject.fanartUrls.map((u) => parseDriveAssetUrl(u)?.rel)).toEqual(['assets/psx/Cinder Peaks/fanart.jpg']);
    expect(subject.screenshotUrls).toHaveLength(1);
  });

  it('has no cover and no fanart where the pack names none', () => {
    const subject = fromDriveGame(driveGame({ cover: '', fanart: '', screenshots: [] }), drive);
    expect(subject.coverLarge).toBeNull();
    expect(subject.fanartUrls).toEqual([]);
    expect(subject.screenshotUrls).toEqual([]);
  });

  it('carries the pack\'s details: summary, genres, platform, release date, companies', () => {
    const subject = fromDriveGame(driveGame(), drive);
    expect(subject.romId).toBe(7);
    expect(subject.platformName).toBe('PlayStation');
    expect(subject.platformSlug).toBe('psx');
    expect(subject.description).toBe('A climb.');
    expect(subject.genres).toBe('Platform, Puzzle');
    expect(subject.releaseDate).toBe('888710400');
    expect(subject.companies).toBe('Ember, Soot');
  });

  it('merges a server detail only from the server the game was backed up from', () => {
    const subject = fromDriveGame(driveGame(), drive);
    expect(detailApplies(subject, 'https://user:pw@ROMM.home.lan:443/')).toBe(true);
    expect(detailApplies(subject, 'https://elsewhere.lan')).toBe(false);
    expect(detailApplies(subject, '')).toBe(false);
    expect(detailApplies(fromInstalled(row({})), 'https://elsewhere.lan')).toBe(true);
  });

  it('says a copy of an installed game is one, with its version', () => {
    expect(driveCopyLines(fromDriveGame(driveGame(), drive))).toEqual([]);
    expect(driveCopyLines(fromDriveGame(driveGame({ installed_copy: true, installed_version: 'v1.1' }), drive))).toEqual([
      'A copy of an installed game · v1.1',
    ]);
    expect(
      driveCopyLines(fromDriveGame(driveGame({ installed_copy: true, installed_version: 'v1.1', outdated: true }), drive)),
    ).toEqual(['A copy of an installed game · v1.1 (the server had newer files)']);
    expect(driveCopyLines(fromDriveGame(driveGame({ installed_copy: true }), drive))).toEqual(['A copy of an installed game']);
    expect(driveCopyLines(fromInstalled(row({})))).toEqual([]);
  });

  it('adds the Wine-prefix line for a Windows copy', () => {
    const win = driveGame({ installed_copy: true, installed_version: 'v2.0', platform: 'win', platform_name: 'Windows' });
    expect(driveCopyLines(fromDriveGame(win, drive))).toEqual([
      'A copy of an installed game · v2.0',
      'Its Wine prefix wasn’t copied, so saves kept there aren’t on the drive',
    ]);
  });

  it('says when a Windows game\'s saves came along from its Wine prefix', () => {
    const win = { platform: 'win', platform_name: 'Windows', saves_copied: true };
    expect(driveCopyLines(fromDriveGame(driveGame({ ...win, installed_copy: true, installed_version: 'v2.0' }), drive))).toEqual([
      'A copy of an installed game · v2.0',
      'Saves from its Wine prefix are included',
    ]);
    expect(driveCopyLines(fromDriveGame(driveGame(win), drive))).toEqual(['Saves from its Wine prefix are included']);
    expect(driveCopyLines(fromInstalled(row({})))).toEqual([]);
  });

  it('reads its version like a server game', () => {
    expect(versionSource(fromDriveGame(driveGame(), drive))).toBe('server');
    expect(versionSource(fromInstalled(row({})))).toBe('installed');
  });
});

describe('releaseSeconds', () => {
  it('turns a pack date into the seconds the header reads, like grid-core', () => {
    expect(releaseSeconds('1993-11-11')).toBe('752976000');
    expect(releaseSeconds('752976000')).toBe('752976000');
    expect(releaseSeconds('soon')).toBe('');
    expect(releaseSeconds('')).toBe('');
  });
});
