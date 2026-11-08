import { describe, expect, it } from 'vitest';
import type { CatalogEntry, CompatTool, PlatformSetup, SetupOption } from '../api';
import { protonPicker } from '../systems/proton';
import {
  canStart,
  choices,
  choosePicker,
  compatChoice,
  installCount,
  optionLabel,
  pick,
  primaryLabel,
  protonPreselect,
  rowState,
  sortRows,
  toggle,
  toInstall,
} from './choices';

const opt = (source_id: string, name: string, kind: SetupOption['kind'] = 'standalone', installed = false): SetupOption => ({
  source_id,
  name,
  kind,
  installed,
  server: kind === 'server' ? { rom_id: 1, file_id: 2, file_name: `${name} (v1.0) (arch-linux-x64).zip`, version: '1.0', folder: 'x' } : null,
  needs_core: kind === 'retroarch',
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
    expect(installCount([ps2, snes, nes], s, null)).toBe(2);
    expect(primaryLabel({ installs: 2, ticked: 3, proton: null })).toBe('Install 2 emulators');
    expect(primaryLabel({ installs: 1, ticked: 1, proton: null })).toBe('Install 1 emulator');
  });

  it('is disabled while nothing is ticked', () => {
    const none = { installs: 0, ticked: 0, proton: null };
    expect(canStart(none)).toBe(false);
    expect(primaryLabel(none)).toBe('Install emulators');
  });

  it('downloads nothing for an emulator already configured', () => {
    const installedPs2 = row('PS2', 1, [opt('PCSX2/pcsx2', 'PCSX2', 'standalone', true)]);
    expect(toInstall([installedPs2], toggle({}, installedPs2))).toEqual([]);
    expect(primaryLabel({ installs: 0, ticked: 1, proton: null })).toBe('Use selected emulators');
    expect(primaryLabel({ installs: 0, ticked: 1, proton: 'wine' })).toBe('Use selected emulators');
  });

  it('counts a Proton download with the emulators', () => {
    const s = toggle(toggle({}, snes), ps2);
    const proton = 'install:GloriousEggroll/proton-ge-custom';
    expect(installCount([ps2, snes], s, proton)).toBe(3);
    expect(primaryLabel({ installs: 3, ticked: 2, proton })).toBe('Install 3 emulators');
    expect(installCount([ps2], {}, proton)).toBe(1);
    expect(primaryLabel({ installs: 1, ticked: 0, proton })).toBe('Install 1 emulator');
  });

  it('reads Continue when only an installed Proton version was picked', () => {
    expect(installCount([ps2], {}, '/usr/bin/wine')).toBe(0);
    const input = { installs: 0, ticked: 0, proton: '/usr/bin/wine' };
    expect(canStart(input)).toBe(true);
    expect(primaryLabel(input)).toBe('Continue');
  });

  it('sends one choice per ticked platform', () => {
    expect(choices(toggle(toggle({}, snes), ps2))).toEqual([
      { platform: 'Super Nintendo', source_id: 'libretro/RetroArch' },
      { platform: 'PlayStation 2', source_id: 'PCSX2/pcsx2' },
    ]);
  });
});

describe('optionLabel', () => {
  it('gives a server build’s version and says where it comes from', () => {
    expect(optionLabel(opt('romm:pico/pico-8', 'Pico-8', 'server'), null)).toBe('Pico-8 1.0 · from your server');
    const retroarch = { ...opt('romm:multi/retroarch (multi-system)', 'RetroArch (Multi-System)', 'server'), needs_core: true };
    expect(optionLabel(retroarch, 'snes9x')).toBe('RetroArch 1.0 · snes9x core · from your server');
  });

  it('drops the platform suffix and names RetroArch’s core', () => {
    expect(optionLabel(pcsx2, null)).toBe('PCSX2');
    expect(optionLabel(retroarch, 'snes9x')).toBe('RetroArch · snes9x core');
    expect(optionLabel(opt('a/b', 'Dolphin (GameCube, Wii)', 'standalone', true), null)).toBe('Dolphin (installed)');
  });
});

describe('choosePicker', () => {
  it('groups Installed and Download, the recommended one first and marked', () => {
    const server = opt('romm:ps2/pcsx2', 'PCSX2 (Playstation 2)', 'server');
    const dolphin = opt('a/dolphin', 'Dolphin', 'standalone', true);
    const r = { ...row('PlayStation 2', 3, [dolphin, server, pcsx2], 'pcsx2'), recommended: 'PCSX2/pcsx2' };
    expect(choosePicker(r)).toEqual({
      installed: [{ value: 'a/dolphin', label: 'Dolphin', recommended: false }],
      download: [
        { value: 'PCSX2/pcsx2', label: 'PCSX2 (recommended)', recommended: true },
        { value: 'romm:ps2/pcsx2', label: 'PCSX2 1.0 · from your server', recommended: false },
      ],
    });
  });
});

describe('the Windows row', () => {
  const wine: CompatTool = { name: 'Wine 10.0', kind: 'wine', path: '/usr/bin/wine', source: 'system' };
  const ge: CatalogEntry = {
    name: 'GE-Proton',
    source_id: 'GloriousEggroll/proton-ge-custom',
    provider: 'github-release',
    tag: 'latest',
    installed: false,
  } as CatalogEntry;
  const cachy = { ...ge, name: 'Proton-CachyOS', source_id: 'CachyOS/proton-cachyos' } as CatalogEntry;

  it('pre-selects the GE-Proton download', () => {
    expect(protonPreselect(protonPicker([wine], [cachy, ge], ''))).toBe('install:GloriousEggroll/proton-ge-custom');
  });

  it('falls back to the current default, then the first installed tool', () => {
    expect(protonPreselect(protonPicker([wine], [{ ...ge, installed: true }], '/usr/bin/wine'))).toBe('/usr/bin/wine');
    expect(protonPreselect(protonPicker([wine], [], ''))).toBe('/usr/bin/wine');
    expect(protonPreselect(protonPicker([], [cachy], ''))).toBe('install:CachyOS/proton-cachyos');
  });

  it('cannot be ticked when nothing is offered', () => {
    expect(protonPreselect(protonPicker([], [], ''))).toBeNull();
  });

  it('sends an installed tool by path, a download by source id, and nothing unticked', () => {
    expect(compatChoice('/usr/bin/wine')).toEqual({ kind: 'installed', path: '/usr/bin/wine' });
    expect(compatChoice('install:GloriousEggroll/proton-ge-custom')).toEqual({
      kind: 'download',
      source_id: 'GloriousEggroll/proton-ge-custom',
    });
    expect(compatChoice(null)).toBeNull();
  });
});
