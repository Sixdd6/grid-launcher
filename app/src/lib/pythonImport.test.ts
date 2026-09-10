import { describe, expect, it } from 'vitest';
import { importToastText } from './pythonImport';

describe('importToastText', () => {
  it('reports the counts and asks for the RomM token', () => {
    expect(
      importToastText({ emulators: 2, games: 14, skipped_games: 0, retroachievements: false }),
    ).toBe('Imported 2 emulators and 14 games from the previous version. Enter your RomM token to reconnect.');
  });

  it('appends the RetroAchievements sentence only when a username came across', () => {
    expect(
      importToastText({ emulators: 1, games: 1, skipped_games: 0, retroachievements: true }),
    ).toBe(
      'Imported 1 emulator and 1 game from the previous version. Enter your RomM token to reconnect. Enter your RetroAchievements token as well.',
    );
  });

  it('says none rather than 0', () => {
    expect(
      importToastText({ emulators: 0, games: 0, skipped_games: 3, retroachievements: false }),
    ).toBe('Imported no emulators and no games from the previous version. Enter your RomM token to reconnect.');
  });
});
