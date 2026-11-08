import { describe, expect, it } from 'vitest';
import type { DownloadEntry, GameSummary } from '../api';
import {
  addedAgo,
  clampIndex,
  coverPlace,
  discoverAction,
  isWide,
  liveInstallStatus,
  logoColours,
  logoStyle,
  releasedLine,
  rgbToHsl,
  shotSource,
  wrapIndex,
} from './feed';

function game(over: Partial<GameSummary> = {}): GameSummary {
  return {
    id: 1,
    name: 'Game',
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
    ...over,
  };
}

function entry(over: Partial<DownloadEntry>): DownloadEntry {
  return {
    id: 1, job: 'game', kind: 'base', rom_id: 7, source_id: '', title: '', platform: '',
    status: 'downloading', downloaded_bytes: 0, total_bytes: 0, speed_bps: 0,
    install_processed_bytes: 0, install_total_bytes: 0, error: '', detail: '',
    ...over,
  };
}

describe('discoverAction', () => {
  it('plays an installed game and installs the rest', () => {
    expect(discoverAction(true, null)).toBe('Play');
    expect(discoverAction(false, null)).toBe('Install');
  });
  it('says Queued or Installing… while an install is live', () => {
    expect(discoverAction(false, 'queued')).toBe('Queued');
    expect(discoverAction(false, 'downloading')).toBe('Installing…');
    expect(discoverAction(false, 'installing')).toBe('Installing…');
  });
});

describe('liveInstallStatus', () => {
  it('finds only a live game install of that rom', () => {
    expect(liveInstallStatus([entry({})], 7)).toBe('downloading');
    expect(liveInstallStatus([entry({ status: 'completed' })], 7)).toBeNull();
    expect(liveInstallStatus([entry({ job: 'emulator' })], 7)).toBeNull();
    expect(liveInstallStatus([entry({ rom_id: 8 })], 7)).toBeNull();
  });
});

describe('dates', () => {
  it('formats a release date as a UTC day', () => {
    expect(releasedLine(Date.UTC(2026, 10, 7) / 1000)).toBe('Released 7 Nov 2026');
    expect(releasedLine(null)).toBe('');
  });
  it('says how long ago a game was added', () => {
    const now = Date.UTC(2026, 10, 8, 12);
    expect(addedAgo('2026-11-08T01:00:00+00:00', now)).toBe('Added today');
    expect(addedAgo('2026-11-07T23:00:00+00:00', now)).toBe('Added yesterday');
    expect(addedAgo('2026-11-05T07:42:09+00:00', now)).toBe('Added 3 days ago');
    expect(addedAgo('2026-10-18T00:00:00+00:00', now)).toBe('Added 3 weeks ago');
    expect(addedAgo('2026-01-02T00:00:00+00:00', now)).toBe('Added 2 Jan 2026');
    expect(addedAgo('', now)).toBe('');
    expect(addedAgo('nonsense', now)).toBe('');
  });
});

describe('indices', () => {
  it('clamps the coverflow and wraps Recently added', () => {
    expect(clampIndex(-1, 5)).toBe(0);
    expect(clampIndex(9, 5)).toBe(4);
    expect(clampIndex(3, 0)).toBe(0);
    expect(wrapIndex(5, 5)).toBe(0);
    expect(wrapIndex(-1, 5)).toBe(4);
  });
});

describe('coverPlace', () => {
  it('keeps the centre flat and tilts the sides 52° back', () => {
    expect(coverPlace(0)).toMatchObject({ center: true, hidden: false, zIndex: 50 });
    expect(coverPlace(0).transform).toContain('rotateY(0deg)');
    expect(coverPlace(1).transform).toBe('translateX(150px) translateZ(-140px) rotateY(-52deg)');
    expect(coverPlace(-2).transform).toBe('translateX(-216px) translateZ(-180px) rotateY(52deg)');
    expect(coverPlace(-2).zIndex).toBe(48);
  });
  it('hides covers beyond four from the centre', () => {
    expect(coverPlace(4).hidden).toBe(false);
    expect(coverPlace(-5).hidden).toBe(true);
  });
});

describe('shotSource', () => {
  it('prefers a screenshot, then fanart, then the cover', () => {
    expect(shotSource(game({ screenshot_urls: ['s'], fanart_urls: ['f'] }))).toEqual({ url: 's', kind: 'screenshot' });
    expect(shotSource(game({ fanart_urls: ['f'], path_cover_large: 'c' }))).toEqual({ url: 'f', kind: 'fanart' });
    expect(shotSource(game({ path_cover_small: 'cs' }))).toEqual({ url: 'cs', kind: 'cover' });
    expect(shotSource(game({ path_cover_large: 'cl', path_cover_small: 'cs' }))).toEqual({ url: 'cl', kind: 'cover' });
    expect(shotSource(game())).toBeNull();
  });
  it('calls 16:9 and wider wide', () => {
    expect(isWide(1920, 1080)).toBe(true);
    expect(isWide(2560, 1080)).toBe(true);
    expect(isWide(640, 480)).toBe(false);
    expect(isWide(256, 224)).toBe(false);
  });
});

describe('logos', () => {
  it('picks one of three styles by id', () => {
    expect([logoStyle(3), logoStyle(4), logoStyle(5)]).toEqual(['a', 'b', 'c']);
  });
  it('converts rgb to hsl', () => {
    expect(rgbToHsl([255, 0, 0])).toEqual([0, 100, 50]);
    expect(rgbToHsl([0, 0, 255])).toEqual([240, 100, 50]);
    expect(rgbToHsl([128, 128, 128])).toEqual([0, 0, 50]);
  });
  it('tints the letters at 88% and shades the outline at 16%', () => {
    expect(logoColours([255, 0, 0])).toEqual({ letters: 'hsl(0 70% 88%)', shadow: 'hsl(0 60% 16%)' });
    expect(logoColours([40, 90, 160])).toEqual({ letters: 'hsl(215 60% 88%)', shadow: 'hsl(215 60% 16%)' });
  });
});
