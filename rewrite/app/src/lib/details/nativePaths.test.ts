import { describe, expect, it } from 'vitest';
import {
  nativePathsEmptyLabel,
  nativePathsStatusLine,
  nativeUploadEnabled,
  nativeUploadTooltip,
} from './nativePaths';

describe('nativePathsStatusLine', () => {
  it('names PCGamingWiki while the lookup is running', () => {
    expect(nativePathsStatusLine('loading', 0)).toBe('Looking up save locations on PCGamingWiki…');
  });

  it('names PCGamingWiki when the lookup found nothing', () => {
    expect(nativePathsStatusLine('loaded', 0)).toBe('No save locations found on PCGamingWiki.');
  });

  it('counts the configured locations, keeping the original "(s)" wording', () => {
    expect(nativePathsStatusLine('loaded', 1)).toBe('1 save location(s) configured.');
    expect(nativePathsStatusLine('loaded', 3)).toBe('3 save location(s) configured.');
  });
});

describe('nativePathsEmptyLabel', () => {
  it('says what is being fetched while loading', () => {
    expect(nativePathsEmptyLabel('loading')).toBe('Fetching save locations from PCGamingWiki…');
  });

  it('is blank once loaded, because the list itself is shown', () => {
    expect(nativePathsEmptyLabel('loaded')).toBe('');
  });
});

describe('nativeUploadTooltip', () => {
  it('explains the wait while the lookup runs', () => {
    expect(nativeUploadTooltip('loading', 0, true)).toBe('Waiting for save location lookup…');
  });

  it('asks for a location when there are none', () => {
    expect(nativeUploadTooltip('loaded', 0, true)).toBe('Add a save location to enable uploads.');
  });

  it('names the missing rom id ahead of the happy path', () => {
    expect(nativeUploadTooltip('loaded', 2, false)).toBe('Missing ROM id for this game.');
  });

  it('describes the upload when everything is in place', () => {
    expect(nativeUploadTooltip('loaded', 2, true)).toBe('Upload save files from the listed locations.');
  });
});

describe('nativeUploadEnabled', () => {
  it('is disabled while loading, while pending, with no paths, and with no rom id', () => {
    expect(nativeUploadEnabled('loading', 2, true, false)).toBe(false);
    expect(nativeUploadEnabled('loaded', 2, true, true)).toBe(false);
    expect(nativeUploadEnabled('loaded', 0, true, false)).toBe(false);
    expect(nativeUploadEnabled('loaded', 2, false, false)).toBe(false);
  });

  it('is enabled with at least one path, a rom id and nothing in flight', () => {
    expect(nativeUploadEnabled('loaded', 1, true, false)).toBe(true);
  });
});
