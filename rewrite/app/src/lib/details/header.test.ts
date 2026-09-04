import { describe, expect, it } from 'vitest';
import {
  developerOf,
  flagList,
  headerLine,
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
