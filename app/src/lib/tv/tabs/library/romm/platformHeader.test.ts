import { describe, expect, it } from 'vitest';
import { backupAllLabel, headerChips, matchesTitle, noMatch, searching, searchTitle, platformCount } from './platformHeader';

describe('platformHeader (items 197–205; decision 14)', () => {
  it("counts RomM's games, then installed and only-here games; how many match while searching", () => {
    expect(platformCount({ onRomm: 3, installed: 1, onlyHere: 0 }, 3, false)).toBe('3 games on RomM · 1 installed');
    expect(platformCount({ onRomm: 2, installed: 1, onlyHere: 1 }, 3, false)).toBe('2 games on RomM · 1 installed · plus 1 only on this computer');
    expect(platformCount({ onRomm: 3, installed: 1, onlyHere: 0 }, 2, true)).toBe('2 of 3 games match');
    expect(platformCount({ onRomm: 1, installed: 0, onlyHere: 0 }, 1, false)).toBe('1 game on RomM · 0 installed');
  });

  it('the emulator chip names the default, or warns with none', () => {
    const fw = { file_count: 3, installed: true, available: true, has_default_emulator: true } as never;
    expect(headerChips('Super Nintendo Entertainment System', 'snes', 'Snes9x', fw)).toEqual({
      emu: { label: 'Emulator: Snes9x', name: 'Snes9x', warn: false },
      fw: { label: 'Firmware: 3 files · installed', focusable: true },
    });
    expect(headerChips('Super Nintendo Entertainment System', 'snes', '', null)?.emu).toEqual({ label: 'No default emulator', name: null, warn: true });
    expect(headerChips('Super Nintendo Entertainment System', 'snes', '<none>', null)?.emu.warn).toBe(true);
  });

  it('the firmware chip is plain when the server has none', () => {
    const none = { file_count: 0, installed: false, available: true, has_default_emulator: true } as never;
    expect(headerChips('Sony PlayStation', 'psx', 'DuckStation', none)?.fw).toEqual({ label: 'No server firmware', focusable: false });
    const needed = { file_count: 2, installed: false, available: true, has_default_emulator: true } as never;
    expect(headerChips('Sony PlayStation', 'psx', 'DuckStation', needed)?.fw).toEqual({ label: 'Firmware: 2 files', focusable: true });
  });

  it('native platforms have no chips (item 201)', () => {
    expect(headerChips('Windows', 'win', '', null)).toBeNull();
    expect(headerChips('Linux', 'linux', '', null)).toBeNull();
  });

  it('labels', () => {
    expect(backupAllLabel(12)).toBe('Backup all 12…');
    expect(searchTitle('PlayStation')).toBe('Search PlayStation');
    expect(noMatch('zz')).toBe('No games match “zz”');
    expect(matchesTitle('Neon Drift', 'neon')).toBe(true);
    expect(matchesTitle('Neon Drift', 'xyz')).toBe(false);
    expect(matchesTitle('Neon Drift', '')).toBe(true);
  });

  it('a blank search is no filter, as on the Library (consultant ruling)', () => {
    expect(searching('')).toBe(false);
    expect(searching('   ')).toBe(false);
    expect(searching(' ne ')).toBe(true);
    expect(matchesTitle('Neon Drift', '   ')).toBe(true);
    expect(noMatch(' zz ')).toBe('No games match “zz”');
  });
});
