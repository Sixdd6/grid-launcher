import { describe, expect, it } from 'vitest';
import { queueRefusedNotice } from '../notifications/notices';
import { needsLibraryFolder } from './installRefusal';

const NO_ROOM =
  "There isn't enough free space in the library folder for Chrono Trigger (needs 1.2 GB, 800.0 MB available)";

describe('needsLibraryFolder', () => {
  it('is the unset library folder refusal', () => {
    expect(needsLibraryFolder('Set a library folder in Settings › Library before installing games')).toBe(true);
  });

  it('is not a library folder that is too full', () => {
    expect(needsLibraryFolder(NO_ROOM)).toBe(false);
    expect(needsLibraryFolder('the server lists no downloadable file for this game')).toBe(false);
  });
});

describe('an install refused for space', () => {
  it('reads as one card naming the game, the need and what is left', () => {
    expect(queueRefusedNotice('Chrono Trigger', NO_ROOM).text).toBe(
      `Couldn't add Chrono Trigger to the download queue: ${NO_ROOM}`,
    );
  });
});
