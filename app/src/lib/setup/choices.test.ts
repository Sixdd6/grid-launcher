import { describe, expect, it } from 'vitest';
import type { PlatformSetup, SetupOption } from '../api';
import { choices, optionLabel, pick, primaryLabel, rowState, sortRows, toggle, toInstall } from './choices';

const opt = (source_id: string, name: string, kind: SetupOption['kind'] = 'standalone', installed = false): SetupOption => ({
  source_id,
  name,
  kind,
  installed,
});

const pcsx2 = opt('PCSX2/pcsx2', 'PCSX2 (Playstation 2)');
const retroarch = opt('libretro/RetroArch', 'RetroArch (Multi-System)', 'retroarch');

function row(label: string, rom_count: number, options: SetupOption[], core: string | null = null): PlatformSetup {
  return {
    label,
    name: label,
    slug: label.toLowerCase(),
    rom_count,
    options,
    recommended: options[0]?.source_id ?? null,
    core,
  };
}

const ps2 = row('PlayStation 2', 12, [pcsx2, retroarch], 'pcsx2');
const snes = row('Super Nintendo', 400, [retroarch], 'snes9x');
const nes = row('NES', 400, [retroarch], 'mesen');
const ps5 = row('PlayStation 5', 3, []);

describe('sortRows', () => {
  it('sorts alphabetically, ignoring case', () => {
    const lower = row('atari 2600', 5, [retroarch], 'stella');
    expect(sortRows([ps2, snes, ps5, nes, lower]).map((r) => r.label)).toEqual([
      'atari 2600',
      'NES',
      'PlayStation 2',
      'PlayStation 5',
      'Super Nintendo',
    ]);
  });
});

describe('rowState', () => {
  it('is unavailable when nothing installable serves the platform', () => {
    expect(rowState(ps5)).toBe('unavailable');
  });

  it('is installed when the recommended emulator is already configured', () => {
    expect(rowState(row('PS2', 1, [opt('PCSX2/pcsx2', 'PCSX2', 'standalone', true)]))).toBe('installed');
    expect(rowState(ps2)).toBe('available');
  });
});

describe('toggle and pick', () => {
  it('ticks a platform with its recommendation and unticks it again', () => {
    const ticked = toggle({}, ps2);
    expect(ticked).toEqual({ 'PlayStation 2': 'PCSX2/pcsx2' });
    expect(toggle(ticked, ps2)).toEqual({});
  });

  it('never ticks an unavailable platform', () => {
    expect(toggle({}, ps5)).toEqual({});
  });

  it('changes a ticked platform to another offered emulator only', () => {
    const ticked = toggle({}, ps2);
    expect(pick(ticked, ps2, 'libretro/RetroArch')).toEqual({ 'PlayStation 2': 'libretro/RetroArch' });
    expect(pick(ticked, ps2, 'someone/else')).toBe(ticked);
    expect(pick({}, ps2, 'libretro/RetroArch')).toEqual({});
  });
});

describe('toInstall and the button', () => {
  it('counts each emulator once, however many platforms share it', () => {
    let s = toggle({}, snes);
    s = toggle(s, nes);
    s = toggle(s, ps2);
    expect(toInstall([ps2, snes, nes], s).map((o) => o.name)).toEqual(['PCSX2 (Playstation 2)', 'RetroArch (Multi-System)']);
    expect(primaryLabel(2, 3)).toBe('Install 2 emulators');
    expect(primaryLabel(1, 1)).toBe('Install 1 emulator');
    expect(primaryLabel(0, 0)).toBe('Install emulators');
  });

  it('downloads nothing for an emulator already configured', () => {
    const installedPs2 = row('PS2', 1, [opt('PCSX2/pcsx2', 'PCSX2', 'standalone', true)]);
    expect(toInstall([installedPs2], toggle({}, installedPs2))).toEqual([]);
    expect(primaryLabel(0, 1)).toBe('Use selected emulators');
  });

  it('sends one choice per ticked platform', () => {
    expect(choices(toggle(toggle({}, snes), ps2))).toEqual([
      { platform: 'Super Nintendo', source_id: 'libretro/RetroArch' },
      { platform: 'PlayStation 2', source_id: 'PCSX2/pcsx2' },
    ]);
  });
});

describe('optionLabel', () => {
  it('drops the platform suffix and names RetroArch’s core', () => {
    expect(optionLabel(pcsx2, null)).toBe('PCSX2');
    expect(optionLabel(retroarch, 'snes9x')).toBe('RetroArch · snes9x core');
    expect(optionLabel(opt('a/b', 'Dolphin (GameCube, Wii)', 'standalone', true), null)).toBe('Dolphin (installed)');
  });
});
