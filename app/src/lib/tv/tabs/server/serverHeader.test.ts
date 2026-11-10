import { describe, expect, it } from 'vitest';
import { backupAllLabel, headerChips, matchesTitle, noMatch, searchTitle, srvCount } from './serverHeader';

describe('serverHeader (items 197–205)', () => {
  it('counts, and how many match while searching', () => {
    expect(srvCount(3, 3, 1, false)).toBe('3 games · 1 installed');
    expect(srvCount(3, 2, 1, true)).toBe('2 of 3 games match · 1 installed');
    expect(srvCount(1, 1, 0, false)).toBe('1 game · 0 installed');
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
});
