import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { InstalledGame } from '../api';
import type { BackgroundSubject } from '../background';

// The store is module-scoped state, so each case re-imports it to start from
// a clean "nothing viewed yet" module.
async function freshStore() {
  vi.resetModules();
  return await import('./lastViewed.svelte');
}

function subject(overrides: Partial<BackgroundSubject> = {}): BackgroundSubject {
  return { fanart: [], screenshots: [], cover: 'https://romm/c.png', ...overrides };
}

function row(overrides: Partial<InstalledGame>): InstalledGame {
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

beforeEach(() => {
  vi.resetModules();
});

describe('noteViewed', () => {
  it('starts with no art at all', async () => {
    const { lastViewed } = await freshStore();
    expect(lastViewed.urls).toEqual([]);
  });

  it('a subject with no art leaves the previous art alone', async () => {
    const { lastViewed, noteViewed } = await freshStore();
    noteViewed(subject({ cover: 'https://romm/first.png' }));
    noteViewed({ fanart: [], screenshots: [], cover: null });
    noteViewed({ fanart: ['  '], screenshots: [''], cover: '   ' });
    expect(lastViewed.urls).toEqual(['https://romm/first.png']);
  });

  it('a re-report of the same art keeps the subject identity', async () => {
    const { lastViewed, noteViewed } = await freshStore();
    noteViewed(subject({ screenshots: ['https://romm/s1.png', 'https://romm/s2.png'] }));
    const before = lastViewed.subject;
    // The details overlay re-reports its merged subject whenever ANY field
    // changes; the art is the same object graph, so the cycle must not reset.
    noteViewed(subject({ screenshots: ['https://romm/s1.png', 'https://romm/s2.png'] }));
    expect(lastViewed.subject).toBe(before);
  });

  it('a different subject replaces the art', async () => {
    const { lastViewed, noteViewed } = await freshStore();
    noteViewed(subject({ screenshots: ['https://romm/s1.png'] }));
    const before = lastViewed.subject;
    noteViewed(subject({ screenshots: ['https://romm/s2.png'] }));
    expect(lastViewed.subject).not.toBe(before);
    expect(lastViewed.urls).toEqual(['https://romm/s2.png']);
  });

  it('reports the chosen tier only, fanart first', async () => {
    const { lastViewed, noteViewed } = await freshStore();
    noteViewed({
      fanart: ['https://romm/f1.jpg'],
      screenshots: ['https://romm/s1.png'],
      cover: 'https://romm/c.png',
    });
    expect(lastViewed.urls).toEqual(['https://romm/f1.jpg']);
  });
});

describe('seedLastViewed', () => {
  it('seeds the newest installed row when nothing has been viewed', async () => {
    const { lastViewed, seedLastViewed } = await freshStore();
    seedLastViewed([
      row({ installed_at: 10, cover_large_path: 'https://romm/old.png' }),
      row({ installed_at: 20, cover_large_path: 'https://romm/new.png' }),
    ]);
    expect(lastViewed.urls).toEqual(['https://romm/new.png']);
  });

  it('never overwrites a real view', async () => {
    const { lastViewed, noteViewed, seedLastViewed } = await freshStore();
    noteViewed(subject({ cover: 'https://romm/viewed.png' }));
    seedLastViewed([row({ installed_at: 99, cover_large_path: 'https://romm/newest.png' })]);
    expect(lastViewed.urls).toEqual(['https://romm/viewed.png']);
  });

  // The seed is what a `noteViewed` that found no art must not undo: a blank
  // report marks nothing as seeded, so a later seed would still overwrite it.
  it('a rejected art-less report does not mark the store as seeded', async () => {
    const { lastViewed, noteViewed, seedLastViewed } = await freshStore();
    noteViewed({ fanart: [], screenshots: [], cover: null });
    seedLastViewed([row({ installed_at: 1, cover_large_path: 'https://romm/seeded.png' })]);
    expect(lastViewed.urls).toEqual(['https://romm/seeded.png']);
  });

  it('runs only once', async () => {
    const { lastViewed, seedLastViewed } = await freshStore();
    seedLastViewed([row({ installed_at: 1, cover_large_path: 'https://romm/first.png' })]);
    seedLastViewed([row({ installed_at: 2, cover_large_path: 'https://romm/second.png' })]);
    expect(lastViewed.urls).toEqual(['https://romm/first.png']);
  });
});
