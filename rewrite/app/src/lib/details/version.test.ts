import { describe, expect, it } from 'vitest';
import { formatVersionTag, parseVersionTag, romFileNamesFor, versionLabel } from './version';

describe('parseVersionTag', () => {
  it('matches a numeric 5-digit tag', () => {
    expect(parseVersionTag('Game (v00042).zip')).toEqual({ kind: 'numeric', value: 42 });
  });

  it('matches a semver tag', () => {
    expect(parseVersionTag('Game (v3.6.0).zip')).toEqual({ kind: 'semver', parts: [3, 6, 0] });
  });

  it('returns null for a numeric tag with the wrong digit count', () => {
    expect(parseVersionTag('Game (v1234).zip')).toBeNull();
  });

  it('returns null when there is no tag', () => {
    expect(parseVersionTag('My Game.zip')).toBeNull();
  });

  it('prefers the numeric tag when both patterns could match', () => {
    expect(parseVersionTag('Game (v00042) (v1.2.3).zip')).toEqual({ kind: 'numeric', value: 42 });
  });
});

describe('formatVersionTag', () => {
  it('formats a numeric tag zero-padded to 5 digits', () => {
    expect(formatVersionTag({ kind: 'numeric', value: 42 })).toBe('v00042');
  });

  it('formats a semver tag dot-joined', () => {
    expect(formatVersionTag({ kind: 'semver', parts: [3, 6, 0] })).toBe('v3.6.0');
  });
});

describe('versionLabel', () => {
  it('renders the first found tag as "Version: v…" for Windows', () => {
    expect(versionLabel('Windows', ['', 'g (v1.0.0).zip'], '')).toBe('Version: v1.0.0');
  });

  it('falls back to the trimmed revision verbatim for non-Windows/PC platforms', () => {
    expect(versionLabel('PS2', ['g (v1.0.0).zip'], ' r2 ')).toBe('r2');
  });

  it('returns an empty string when there is no tag and no revision', () => {
    expect(versionLabel('Windows', ['g.zip'], '')).toBe('');
  });
});

describe('romFileNamesFor', () => {
  it('reads the installed name first for a Library-opened game', () => {
    expect(romFileNamesFor('installed', 'g (v1.0.0).zip', 'g (v1.1.0).zip')).toEqual([
      'g (v1.0.0).zip',
      'g (v1.1.0).zip',
    ]);
  });

  it('reads the server name first for a Server-opened game', () => {
    expect(romFileNamesFor('server', 'g (v1.0.0).zip', 'g (v1.1.0).zip')).toEqual([
      'g (v1.1.0).zip',
      'g (v1.0.0).zip',
    ]);
  });
});
