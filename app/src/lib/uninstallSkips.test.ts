import { describe, expect, it } from 'vitest';
import { skippedRemovalText } from './uninstallSkips';

describe('skippedRemovalText', () => {
  it('is blank when nothing was skipped', () => {
    expect(skippedRemovalText([])).toBe('');
    expect(skippedRemovalText(undefined)).toBe('');
    expect(skippedRemovalText(null)).toBe('');
  });

  it('names one folder and its reason', () => {
    expect(skippedRemovalText([{ path: '/lib/games/SNES', reason: 'a platform folder' }])).toBe(
      '1 folder was kept on disk for safety: /lib/games/SNES (a platform folder).',
    );
  });

  it('names every folder with its own reason', () => {
    expect(
      skippedRemovalText([
        { path: 'C:\\Games\\Library', reason: 'a library folder or a folder that holds one' },
        { path: 'C:\\Users\\me', reason: 'the home folder or a folder that holds it' },
      ]),
    ).toBe(
      '2 folders were kept on disk for safety: C:\\Games\\Library (a library folder or a folder that holds one); C:\\Users\\me (the home folder or a folder that holds it).',
    );
  });
});
