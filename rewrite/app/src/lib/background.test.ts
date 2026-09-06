import { describe, expect, it } from 'vitest';
import type { InstalledGame } from './api';
import {
  BACKGROUND_CYCLE_MS,
  backgroundUrls,
  cycleIndex,
  HOVER_DELAY_MS,
  shouldCycle,
  startupSubject,
  subjectFromInstalled,
  subjectFromSummary,
} from './background';

function row(overrides: Partial<InstalledGame>): InstalledGame {
  // Only the art fields and `installed_at` are meaningful; the rest are
  // filled from the registry's own "blank, never null" convention.
  return {
    title: 'Chrono Trigger', platform: 'SNES', rom_id: 42, rom_file_name: '', archive_path: '',
    extracted_path: '', extracted_dir: '', multi_file_game_dir: '', description: '', rating: '',
    genres: '', regions: '', languages: '', tags: '', revision: '', companies: '',
    first_release_date: '', filesize_bytes: 0, server_updated_at: '', installed_at: 0,
    cover_small_path: '', cover_large_path: '', screenshot_urls: '', fanart_urls: '', native_executable_path: '',
    native_launch_parameters: '', native_compat_tool: '', native_wineprefix: '',
    native_game_dir: '', included_dlc: '', ps3_trophy_paths: '', ps3_game_id: '',
    ps3_iso_path: '', ps4_game_id: '', ps4_content: '', ra_id: '', last_played_at: 0,
    ...overrides,
  };
}

describe('background timings', () => {
  it('holds the 500ms hover dwell from design section 3', () => {
    expect(HOVER_DELAY_MS).toBe(500);
  });

  it('holds the 5s cycle the TV shell already used', () => {
    expect(BACKGROUND_CYCLE_MS).toBe(5000);
  });
});

describe('backgroundUrls', () => {
  it('prefers fanart over everything else', () => {
    expect(
      backgroundUrls({
        fanart: ['https://romm/f1.jpg', 'https://romm/f2.jpg'],
        screenshots: ['https://romm/s1.png'],
        cover: 'https://romm/cover.png',
      })
    ).toEqual(['https://romm/f1.jpg', 'https://romm/f2.jpg']);
  });

  it('falls back to the screenshots when there is no fanart', () => {
    expect(
      backgroundUrls({ fanart: [], screenshots: ['https://romm/s1.png'], cover: 'https://romm/c.png' })
    ).toEqual(['https://romm/s1.png']);
  });

  it('uses the cover only as a last resort', () => {
    expect(backgroundUrls({ fanart: [], screenshots: [], cover: 'https://romm/c.png' })).toEqual([
      'https://romm/c.png',
    ]);
  });

  it('is empty when the game has no art at all', () => {
    expect(backgroundUrls({ fanart: [], screenshots: [], cover: null })).toEqual([]);
    expect(backgroundUrls({ fanart: ['  '], screenshots: [''], cover: '   ' })).toEqual([]);
  });

  it('trims and de-duplicates within the chosen tier', () => {
    expect(
      backgroundUrls({ fanart: [], screenshots: [' https://romm/s1.png ', 'https://romm/s1.png'], cover: null })
    ).toEqual(['https://romm/s1.png']);
  });

  // Task 4: a URL whose blurred variant could not be built is dropped BEFORE
  // the first-non-empty-tier rule, so a fanart the backend cannot decode
  // falls through to the screenshots instead of leaving the shell blank.
  it('drops failed URLs from a tier and falls through when the tier empties', () => {
    const subject = {
      fanart: ['https://romm/f1.jpg', 'https://romm/f2.jpg'],
      screenshots: ['https://romm/s1.png'],
      cover: 'https://romm/cover.png',
    };
    expect(backgroundUrls(subject, new Set(['https://romm/f1.jpg']))).toEqual(['https://romm/f2.jpg']);
    expect(backgroundUrls(subject, new Set(['https://romm/f1.jpg', 'https://romm/f2.jpg']))).toEqual([
      'https://romm/s1.png',
    ]);
    expect(
      backgroundUrls(subject, new Set(['https://romm/f1.jpg', 'https://romm/f2.jpg', 'https://romm/s1.png']))
    ).toEqual(['https://romm/cover.png']);
  });

  it('is empty when every tier has failed', () => {
    expect(
      backgroundUrls(
        { fanart: ['https://romm/f1.jpg'], screenshots: [], cover: 'https://romm/cover.png' },
        new Set(['https://romm/f1.jpg', 'https://romm/cover.png'])
      )
    ).toEqual([]);
  });

  it('is unchanged when nothing has failed', () => {
    const subject = {
      fanart: ['https://romm/f1.jpg'],
      screenshots: ['https://romm/s1.png'],
      cover: 'https://romm/cover.png',
    };
    expect(backgroundUrls(subject, new Set())).toEqual(backgroundUrls(subject));
    expect(backgroundUrls(subject, new Set(['https://romm/other.jpg']))).toEqual(['https://romm/f1.jpg']);
  });

  // The failed set is matched against the TRIMMED URL, the same form the
  // fetch effect reports back after a rejection.
  it('matches failures after trimming', () => {
    expect(
      backgroundUrls(
        { fanart: [' https://romm/f1.jpg '], screenshots: ['https://romm/s1.png'], cover: null },
        new Set(['https://romm/f1.jpg'])
      )
    ).toEqual(['https://romm/s1.png']);
  });
});

describe('shouldCycle', () => {
  it('cycles only with more than one image', () => {
    expect(shouldCycle(['a', 'b'], 25)).toBe(true);
    expect(shouldCycle(['a'], 25)).toBe(false);
    expect(shouldCycle([], 25)).toBe(false);
  });

  // User ruling 2026-09-05: fade 0 means the art is invisible, so a timer
  // swapping invisible images is pure cost.
  it('does not cycle while the fade slider is at 0', () => {
    expect(shouldCycle(['a', 'b'], 0)).toBe(false);
  });
});

describe('cycleIndex', () => {
  it('advances and wraps', () => {
    expect(cycleIndex(0, 3)).toBe(1);
    expect(cycleIndex(2, 3)).toBe(0);
  });

  it('is 0 for an empty list, never NaN', () => {
    expect(cycleIndex(4, 0)).toBe(0);
  });

  it('recovers from an index past the end (the list shrank mid-cycle)', () => {
    expect(cycleIndex(9, 2)).toBe(0);
  });

  // JS `%` keeps the sign of its left operand, so a bare `(current + 1) % count`
  // would hand a negative index straight back to the caller.
  it('never returns a negative index', () => {
    expect(cycleIndex(-5, 3)).toBe(2);
    expect(cycleIndex(-1, 3)).toBe(0);
  });
});

describe('startupSubject', () => {
  it('is null when nothing is installed', () => {
    expect(startupSubject([])).toBeNull();
  });

  it('picks the newest row that has any art', () => {
    const subject = startupSubject([
      row({ installed_at: 100, cover_large_path: 'https://romm/old.png' }),
      row({ installed_at: 300, cover_large_path: 'https://romm/newest.png' }),
      row({ installed_at: 200, cover_large_path: '' }),
    ]);
    expect(subject).toEqual({ fanart: [], screenshots: [], cover: 'https://romm/newest.png' });
  });

  it('accepts a row with screenshots but no cover', () => {
    expect(
      startupSubject([row({ installed_at: 1, cover_large_path: '', screenshot_urls: 'https://romm/s1.png' })])
    ).toEqual({ fanart: [], screenshots: ['https://romm/s1.png'], cover: null });
  });

  it('skips rows with no art at all', () => {
    expect(startupSubject([row({ installed_at: 9, cover_large_path: '' })])).toBeNull();
  });
});

describe('subjectFromInstalled / subjectFromSummary', () => {
  it("splits the registry row's newline-joined columns", () => {
    expect(
      subjectFromInstalled(
        row({
          fanart_urls: 'https://romm/f1.jpg',
          screenshot_urls: 'https://romm/s1.png\nhttps://romm/s2.png',
          cover_large_path: 'https://romm/c.png',
        })
      )
    ).toEqual({
      fanart: ['https://romm/f1.jpg'],
      screenshots: ['https://romm/s1.png', 'https://romm/s2.png'],
      cover: 'https://romm/c.png',
    });
  });

  it("reads the server summary's own arrays", () => {
    expect(
      subjectFromSummary({
        id: 1,
        name: 'x',
        platform_id: 1,
        path_cover_small: 'https://romm/s.png',
        path_cover_large: 'https://romm/l.png',
        screenshot_urls: ['https://romm/s1.png'],
        fanart_urls: [],
      })
    ).toEqual({ fanart: [], screenshots: ['https://romm/s1.png'], cover: 'https://romm/l.png' });
  });
});
