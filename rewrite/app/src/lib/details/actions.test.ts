import { describe, expect, it } from 'vitest';
import {
  candidateLabel,
  contentButtons,
  installDirOf,
  installLabel,
  isContentPlatform,
  isNativePlatform,
  isWindowsHost,
} from './actions';

describe('isNativePlatform', () => {
  it('matches Windows case- and whitespace-insensitively', () => {
    expect(isNativePlatform('Windows')).toBe(true);
    expect(isNativePlatform(' windows 10')).toBe(true);
  });

  it('does not match other platforms', () => {
    expect(isNativePlatform('PlayStation 4')).toBe(false);
    expect(isNativePlatform('Xbox 360')).toBe(false);
    expect(isNativePlatform('SNES')).toBe(false);
    expect(isNativePlatform('')).toBe(false);
  });
});

describe('isContentPlatform', () => {
  it('matches PS4 spellings', () => {
    expect(isContentPlatform('PlayStation 4')).toBe(true);
    expect(isContentPlatform('Sony PS4')).toBe(true);
    expect(isContentPlatform('PlayStation4')).toBe(true);
  });

  it('matches Xbox 360 spellings', () => {
    expect(isContentPlatform('Xbox 360')).toBe(true);
    expect(isContentPlatform('Microsoft Xbox360')).toBe(true);
  });

  it('does not match Windows, plain Xbox, or unrelated platforms', () => {
    expect(isContentPlatform('Windows')).toBe(false);
    expect(isContentPlatform('Xbox')).toBe(false);
    expect(isContentPlatform('Xbox One')).toBe(false);
    expect(isContentPlatform('SNES')).toBe(false);
    expect(isContentPlatform('')).toBe(false);
  });
});

describe('installLabel', () => {
  it('reads "Install App" for native platforms', () => {
    expect(installLabel('Windows')).toBe('Install App');
    expect(installLabel(' windows 10')).toBe('Install App');
  });

  it('reads "Install" for every other platform', () => {
    expect(installLabel('PlayStation 4')).toBe('Install');
    expect(installLabel('Xbox 360')).toBe('Install');
    expect(installLabel('SNES')).toBe('Install');
  });
});

describe('contentButtons', () => {
  it('hides both buttons when availability has not resolved', () => {
    expect(contentButtons(null, true, false)).toEqual({ update: false, dlc: false });
  });

  it('hides both buttons while a live download-drawer entry exists', () => {
    expect(contentButtons({ update: true, dlc: true }, true, true)).toEqual({
      update: false,
      dlc: false,
    });
  });

  it('hides both buttons when the game is not installed', () => {
    expect(contentButtons({ update: true, dlc: true }, false, false)).toEqual({
      update: false,
      dlc: false,
    });
  });

  it('shows exactly what availability reports when installed and idle', () => {
    expect(contentButtons({ update: true, dlc: true }, true, false)).toEqual({
      update: true,
      dlc: true,
    });
    expect(contentButtons({ update: true, dlc: false }, true, false)).toEqual({
      update: true,
      dlc: false,
    });
    expect(contentButtons({ update: false, dlc: false }, true, false)).toEqual({
      update: false,
      dlc: false,
    });
  });
});

describe('installDirOf', () => {
  it('is empty for no candidates', () => {
    expect(installDirOf([])).toBe('');
  });

  it('is the shallowest (first) candidate\'s own directory', () => {
    expect(
      installDirOf(['/games/Foo/game.exe', '/games/Foo/bin/helper.exe'])
    ).toBe('/games/Foo');
  });

  it('normalizes Windows separators', () => {
    expect(installDirOf(['C:\\Games\\Foo\\game.exe'])).toBe('C:/Games/Foo');
  });
});

describe('candidateLabel', () => {
  it('is the path relative to the install dir when inside it', () => {
    expect(candidateLabel('/games/Foo/bin/helper.exe', '/games/Foo')).toBe('bin/helper.exe');
    expect(candidateLabel('/games/Foo/game.exe', '/games/Foo')).toBe('game.exe');
  });

  it('normalizes Windows separators on both sides', () => {
    expect(candidateLabel('C:\\Games\\Foo\\bin\\helper.exe', 'C:\\Games\\Foo')).toBe(
      'bin/helper.exe'
    );
  });

  it('falls back to the full path when not inside the install dir', () => {
    expect(candidateLabel('/other/game.exe', '/games/Foo')).toBe('/other/game.exe');
  });

  it('falls back to the full path when there is no install dir', () => {
    expect(candidateLabel('/games/Foo/game.exe', '')).toBe('/games/Foo/game.exe');
  });
});

describe('isWindowsHost', () => {
  it('matches Tauri\'s navigator.platform strings for Windows', () => {
    expect(isWindowsHost('Win32')).toBe(true);
    expect(isWindowsHost('win')).toBe(true);
  });

  it('does not match non-Windows hosts', () => {
    expect(isWindowsHost('Linux x86_64')).toBe(false);
    expect(isWindowsHost('MacIntel')).toBe(false);
    expect(isWindowsHost('')).toBe(false);
  });
});
