import { describe, expect, it } from 'vitest';
import {
  cloudStatusLabel,
  developerOf,
  epochDate,
  flagList,
  headerLine,
  lastPlayedText,
  launchTargetLine,
  ratingText,
  releaseYear,
  verificationLabel,
} from './header';

describe('releaseYear', () => {
  it('reads the year out of the epoch-seconds string the backend sends', () => {
    // 631152000 = 1990-01-01T00:00:00Z
    expect(releaseYear('631152000')).toBe('1990');
  });

  it('is blank for a blank date', () => {
    expect(releaseYear('')).toBe('');
  });

  it('is blank for a non-numeric date rather than rendering NaN', () => {
    expect(releaseYear('sometime in 1990')).toBe('');
  });
});

describe('developerOf', () => {
  it('takes the first company as the developer', () => {
    expect(developerOf('Nintendo, Nintendo EAD')).toBe('Nintendo');
  });

  it('trims', () => {
    expect(developerOf('  Konami  ')).toBe('Konami');
  });

  it('is blank when there are no companies', () => {
    expect(developerOf('')).toBe('');
  });
});

describe('ratingText', () => {
  it('stars a rating', () => {
    expect(ratingText('9.2')).toBe('★ 9.2');
  });

  it('is blank for no rating', () => {
    expect(ratingText('   ')).toBe('');
  });
});

describe('flagList', () => {
  it('splits the comma-joined backend string', () => {
    expect(flagList('USA, Europe')).toEqual(['USA', 'Europe']);
  });

  it('drops blanks', () => {
    expect(flagList('USA, , ')).toEqual(['USA']);
  });

  it('is empty for a blank field', () => {
    expect(flagList('')).toEqual([]);
  });
});

describe('verificationLabel', () => {
  it('names both states', () => {
    expect(verificationLabel(true)).toBe('Identified');
    expect(verificationLabel(false)).toBe('Unidentified');
  });
});

describe('headerLine', () => {
  it('joins platform, year, developer, genres and rating with the middot', () => {
    expect(
      headerLine({
        platformName: 'SNES',
        firstReleaseDate: '631152000',
        companies: 'Nintendo',
        genres: 'Platformer',
        rating: '9.2',
      })
    ).toBe('SNES · 1990 · Nintendo · Platformer · ★ 9.2');
  });

  it('drops every part the server has nothing for, with no dangling separator', () => {
    expect(
      headerLine({
        platformName: 'SNES',
        firstReleaseDate: '',
        companies: '',
        genres: '',
        rating: '',
      })
    ).toBe('SNES');
  });

  it('is blank when the server knows nothing at all', () => {
    expect(
      headerLine({ platformName: '', firstReleaseDate: '', companies: '', genres: '', rating: '' })
    ).toBe('');
  });
});

describe('epochDate', () => {
  it('formats an epoch as a UTC date', () => {
    expect(epochDate(1_800_000_000)).toBe('2027-01-15');
  });

  it('is blank for never', () => {
    expect(epochDate(0)).toBe('');
  });
});

describe('lastPlayedText', () => {
  it('names the date of the last launch', () => {
    expect(lastPlayedText(1_800_000_000)).toBe('Last played 2027-01-15');
  });

  it('says so when the game has never been launched through GRID', () => {
    expect(lastPlayedText(0)).toBe('Never played');
  });
});

describe('launchTargetLine', () => {
  const defaults = (
    emulators: Record<string, string>,
    cores: Record<string, string> = {}
  ) => ({ default_emulators: emulators, retroarch_cores: cores, launch_args: '' });

  it('names the platform default emulator', () => {
    expect(launchTargetLine(defaults({ snes: 'Snes9x' }), 'SNES')).toBe('Snes9x');
  });

  it('names the core too when the default is a RetroArch build', () => {
    expect(
      launchTargetLine(defaults({ snes: 'RetroArch' }, { snes: 'snes9x_libretro' }), 'SNES')
    ).toBe('RetroArch · snes9x_libretro');
  });

  it('says a RetroArch default has no core rather than naming half a target', () => {
    expect(launchTargetLine(defaults({ snes: 'RetroArch' }), 'SNES')).toBe('RetroArch · no core');
  });

  it('reads a remembered "(none)" the same as an absent default', () => {
    expect(launchTargetLine(defaults({ snes: '<none>' }), 'SNES')).toBe('No default emulator');
  });

  it('says so when nothing is configured at all', () => {
    expect(launchTargetLine(null, 'SNES')).toBe('No default emulator');
  });
});

describe('cloudStatusLabel', () => {
  it('offers the panel when either kind is supported', () => {
    expect(cloudStatusLabel(true, false)).toBe('Cloud saves');
    expect(cloudStatusLabel(false, true)).toBe('Cloud saves');
  });

  it('says so when neither is', () => {
    expect(cloudStatusLabel(false, false)).toBe('Not configured');
  });
});
