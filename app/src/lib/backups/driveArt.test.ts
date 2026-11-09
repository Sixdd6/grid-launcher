import { describe, expect, it } from 'vitest';
import { driveAssetUrl, isDriveAssetUrl, parseDriveAssetUrl } from './driveArt';

describe('drive asset URLs', () => {
  it('round-trips a mount path and a pack path with spaces', () => {
    const url = driveAssetUrl('/run/media/me/SAMSUNG T7', 'assets/psx/Cinder Peaks/cover.jpg')!;
    expect(isDriveAssetUrl(url)).toBe(true);
    expect(parseDriveAssetUrl(url)).toEqual({ key: '/run/media/me/SAMSUNG T7', rel: 'assets/psx/Cinder Peaks/cover.jpg' });
  });

  it('round-trips a Windows drive letter', () => {
    expect(parseDriveAssetUrl(driveAssetUrl('E:', 'assets/a/b/cover.png')!)).toEqual({ key: 'E:', rel: 'assets/a/b/cover.png' });
  });

  it('has no URL for an asset the pack does not name', () => {
    expect(driveAssetUrl('E:', '')).toBeNull();
  });

  it('leaves every other URL alone', () => {
    expect(parseDriveAssetUrl('https://romm/cover.jpg')).toBeNull();
    expect(isDriveAssetUrl('/assets/romm/resources/cover.png')).toBe(false);
    expect(parseDriveAssetUrl('gridpack-asset:E%3A/')).toBeNull();
  });
});
