import { describe, expect, it } from 'vitest';
import { CHECK_ONLY_NOTE, relativeCheckTime, updateStatusLine, versionLine } from './updates';

const MINUTE = 60_000;
// 2023-11-14T22:13:20Z, the stamp the Rust tests use too.
const CHECKED_AT = '2023-11-14T22:13:20Z';
const CHECKED_MS = Date.parse(CHECKED_AT);
const NOTICE = { tag: 'v9.9.9-e2e', url: 'https://github.com/Sixdd6/grid-launcher/releases/tag/v9.9.9-e2e' };

describe('versionLine', () => {
  it('names the running build', () => {
    expect(versionLine('0.9.0')).toBe('GRID Launcher 0.9.0');
    expect(versionLine('0.9.0-dev')).toBe('GRID Launcher 0.9.0-dev');
  });
  it('says so when the version has not loaded', () => {
    expect(versionLine('')).toBe('GRID Launcher (version unknown)');
  });
});

describe('relativeCheckTime', () => {
  it('rounds to minutes, then hours', () => {
    expect(relativeCheckTime(CHECKED_AT, CHECKED_MS + 20_000)).toBe('just now');
    expect(relativeCheckTime(CHECKED_AT, CHECKED_MS + MINUTE)).toBe('1 min ago');
    expect(relativeCheckTime(CHECKED_AT, CHECKED_MS + 5 * MINUTE)).toBe('5 min ago');
    expect(relativeCheckTime(CHECKED_AT, CHECKED_MS + 60 * MINUTE)).toBe('1 h ago');
    expect(relativeCheckTime(CHECKED_AT, CHECKED_MS + 150 * MINUTE)).toBe('2 h ago');
  });
  it('never goes negative when the clock moves', () => {
    expect(relativeCheckTime(CHECKED_AT, CHECKED_MS - MINUTE)).toBe('just now');
  });
  it('is empty for a stamp it cannot parse', () => {
    expect(relativeCheckTime('yesterday', CHECKED_MS)).toBe('');
  });
});

describe('updateStatusLine (the three backend states, Task 3)', () => {
  it('never claims up to date when no check completed', () => {
    expect(updateStatusLine(null, null, CHECKED_MS)).toBe('Not checked yet');
  });
  it('reports up to date with the relative check time', () => {
    expect(updateStatusLine(null, CHECKED_AT, CHECKED_MS + 5 * MINUTE)).toBe('Up to date · checked 5 min ago');
  });
  it('drops the time when the stamp is unparseable', () => {
    expect(updateStatusLine(null, 'garbage', CHECKED_MS)).toBe('Up to date');
  });
  it('names the release when a notice is stored, verbatim to the badge title', () => {
    expect(updateStatusLine(NOTICE, CHECKED_AT, CHECKED_MS)).toBe('GRID Launcher v9.9.9-e2e is available');
  });
});

describe('CHECK_ONLY_NOTE', () => {
  it('states the check-only rule (doc 10 D-10-h)', () => {
    expect(CHECK_ONLY_NOTE).toBe(
      'GRID Launcher checks GitHub for a newer release once at startup. It never downloads or installs an update — open the release page to get it.',
    );
  });
});
