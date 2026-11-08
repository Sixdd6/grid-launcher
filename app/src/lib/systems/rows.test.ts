import { describe, expect, it } from 'vitest';
import type { CompanionLink, DownloadEntry, EmulatorEntry, LaunchDefaults, Platform, PlatformSetup, SetupOption } from '../api';
import {
  alsoPlays,
  alsoPlaysText,
  buildSystemRows,
  canPlay,
  companionParent,
  companionsOf,
  installRecommendedLabel,
  isInstalling,
  liveEmulatorSources,
  missingRecommended,
  missingCompanionText,
  missingText,
  NONE_VALUE,
  otherEmulatorNote,
  otherEmulators,
  parsePick,
  pickerGroups,
  statusOf,
  systemFirmwareRequests,
  testKeyOf,
  updateAvailable,
  versionText,
} from './rows';

const opt = (source_id: string, name: string, extra: Partial<SetupOption> = {}): SetupOption => ({
  source_id,
  name,
  kind: 'standalone',
  installed: false,
  server: null,
  needs_core: false,
  ...extra,
});

const pcsx2 = opt('PCSX2/pcsx2', 'PCSX2 (Playstation 2)');
const pcsx2Server = opt('romm:ps2/pcsx2 (playstation 2)', 'PCSX2 (Playstation 2)', {
  kind: 'server',
  server: { rom_id: 1, file_id: 2, file_name: 'x', version: '2.0.2', folder: 'ps2' },
});
const redream = opt('redream/redream', 'Redream (Sega Dreamcast)');
const retroarch = opt('libretro/RetroArch', 'RetroArch (Multi-System)', { kind: 'retroarch', needs_core: true });

function setup(label: string, name: string, options: SetupOption[], core: string | null = null): PlatformSetup {
  return {
    label,
    name,
    slug: label.toLowerCase().replace(/\s+/g, ''),
    rom_count: 4,
    options,
    recommended: options[0]?.source_id ?? null,
    core,
  };
}

const platform = (id: number, name: string): Platform => ({
  id,
  name,
  slug: name.toLowerCase(),
  rom_count: 4,
  custom_name: null,
  display_name: name,
});

const defaults = (map: Record<string, string>): LaunchDefaults => ({
  default_emulators: map,
  retroarch_cores: {},
  launch_args: '',
});

const ps2 = setup('PlayStation 2', 'Sony PlayStation 2', [pcsx2, pcsx2Server]);
const dc = setup('Dreamcast', 'Sega Dreamcast', [redream]);
const snes = setup('Super Nintendo', 'Super Nintendo', [retroarch], 'snes9x');
const zeebo = setup('Zeebo', 'Zeebo', []);
const platforms = [platform(1, 'Sony PlayStation 2'), platform(2, 'Sega Dreamcast'), platform(3, 'Super Nintendo'), platform(4, 'Zeebo')];

describe('buildSystemRows', () => {
  it('puts systems with no emulator first, then alphabetical', () => {
    const rows = buildSystemRows([zeebo, ps2, snes, dc], platforms, defaults({}), {
      'Sega Dreamcast': ['Redream (Sega Dreamcast)'],
    });
    expect(rows.map((r) => r.label)).toEqual(['PlayStation 2', 'Super Nintendo', 'Zeebo', 'Dreamcast']);
    expect(rows[3].emulator).toBe('Redream (Sega Dreamcast)');
    expect(rows[3].platform?.id).toBe(2);
  });

  it('shows the saved default, else the first compatible one, and nothing after an explicit none', () => {
    const compatible = { 'Sony PlayStation 2': ['PCSX2 (Playstation 2)', 'Other'] };
    expect(buildSystemRows([ps2], platforms, defaults({ 'PlayStation 2': 'Other' }), compatible)[0].emulator).toBe('Other');
    expect(buildSystemRows([ps2], platforms, defaults({}), compatible)[0].emulator).toBe('PCSX2 (Playstation 2)');
    const none = buildSystemRows([ps2], platforms, defaults({ 'playstation 2': '<none>' }), compatible)[0];
    expect(none.emulator).toBe('');
    expect(none.explicitNone).toBe(true);
  });

  it('downloads hold only what is not installed, recommended first', () => {
    const installedRec = setup('PlayStation 2', 'Sony PlayStation 2', [{ ...pcsx2, installed: true }, pcsx2Server]);
    const row = buildSystemRows([installedRec], platforms, defaults({}), {})[0];
    expect(row.downloads).toEqual([pcsx2Server]);
    expect(row.recommended?.source_id).toBe('PCSX2/pcsx2');
  });
});

describe('pickerGroups', () => {
  it('groups Installed and Download, the recommended one marked', () => {
    const row = buildSystemRows([ps2], platforms, defaults({}), {})[0];
    const groups = pickerGroups(row);
    expect(groups.selected).toBe(NONE_VALUE);
    expect(groups.installed).toEqual([]);
    expect(groups.download.map((o) => o.label)).toEqual([
      'Install PCSX2 (recommended)',
      'Install PCSX2 2.0.2 · from your server',
    ]);
    expect(groups.download[0]).toMatchObject({ value: 'install:PCSX2/pcsx2', recommended: true });
  });

  it('selects the emulator in use and marks an installed recommendation', () => {
    const installedRec = setup('PlayStation 2', 'Sony PlayStation 2', [{ ...pcsx2, installed: true }]);
    const row = buildSystemRows([installedRec], platforms, defaults({}), {
      'Sony PlayStation 2': ['PCSX2 (Playstation 2)', 'Mednafen'],
    })[0];
    const groups = pickerGroups(row);
    expect(groups.selected).toBe('use:PCSX2 (Playstation 2)');
    expect(groups.installed.map((o) => o.label)).toEqual(['PCSX2 (Playstation 2) (recommended)', 'Mednafen']);
    expect(groups.download).toEqual([]);
  });

  it('names the core RetroArch would use', () => {
    const row = buildSystemRows([snes], platforms, defaults({}), {})[0];
    expect(pickerGroups(row).download[0].label).toBe('Install RetroArch · snes9x core (recommended)');
  });
});

describe('parsePick', () => {
  it('reads the three kinds of value', () => {
    expect(parsePick('')).toEqual({ kind: 'none' });
    expect(parsePick('use:PCSX2 (Playstation 2)')).toEqual({ kind: 'use', name: 'PCSX2 (Playstation 2)' });
    expect(parsePick('install:romm:ps2/pcsx2')).toEqual({ kind: 'install', sourceId: 'romm:ps2/pcsx2' });
  });
});

describe('statusOf', () => {
  const base = { emulator: 'PCSX2', installing: false, firmware: null, updateAvailable: false } as const;
  it('ranks installing, none, firmware, update, ready', () => {
    expect(statusOf({ ...base, emulator: '', installing: true })).toBe('installing');
    expect(statusOf({ ...base, emulator: '' })).toBe('none');
    expect(statusOf({ ...base, firmware: 'install', updateAvailable: true })).toBe('firmware');
    expect(statusOf({ ...base, firmware: 'requested' })).toBe('installing');
    expect(statusOf({ ...base, updateAvailable: true })).toBe('update');
    expect(statusOf({ ...base, firmware: 'installed' })).toBe('ready');
    expect(statusOf({ ...base, firmware: 'unavailable' })).toBe('ready');
  });
});

const download = (id: number, source_id: string, status: DownloadEntry['status'], job: DownloadEntry['job'] = 'emulator') =>
  ({ id, job, kind: 'emulator', source_id, status }) as unknown as DownloadEntry;

describe('installing', () => {
  it('needs a pending default and a running install of one of its emulators', () => {
    const row = buildSystemRows([ps2], platforms, defaults({}), {})[0];
    const live = liveEmulatorSources([
      download(1, 'PCSX2/pcsx2', 'downloading'),
      download(2, 'redream/redream', 'failed'),
      download(3, 'x', 'downloading', 'game'),
    ]);
    expect([...live]).toEqual(['PCSX2/pcsx2']);
    expect(isInstalling(row, ['playstation 2'], live)).toBe(true);
    expect(isInstalling(row, [], live)).toBe(false);
    expect(isInstalling(row, ['PlayStation 2'], new Set())).toBe(false);
  });

  it('Install recommended covers every system with none it can install, skipping installing ones', () => {
    const rows = buildSystemRows([ps2, dc, snes, zeebo], platforms, defaults({}), {
      'Sega Dreamcast': ['Redream (Sega Dreamcast)'],
    });
    const choices = missingRecommended(rows, (r) => r.key === 'Super Nintendo');
    expect(choices).toEqual([{ platform: 'PlayStation 2', source_id: 'PCSX2/pcsx2' }]);
    expect(installRecommendedLabel(choices.length)).toBe('Install recommended (1)');
  });
});

describe('emulators across systems', () => {
  const compatible = {
    'Sony PlayStation 2': ['RetroArch'],
    'Sega Dreamcast': ['RetroArch'],
    'Super Nintendo': ['RetroArch', 'Snes9x'],
  };
  const rows = buildSystemRows([ps2, dc, snes], platforms, defaults({}), compatible);
  const entry = (name: string): EmulatorEntry => ({ name, path: `/emu/${name}`, args: '' });

  it('says which other systems an emulator plays', () => {
    expect(alsoPlays('RetroArch', rows, 'Dreamcast')).toEqual(['PlayStation 2', 'Super Nintendo']);
    expect(alsoPlaysText(['PlayStation 2'])).toBe('Also plays PlayStation 2');
    expect(alsoPlaysText([])).toBe('');
  });

  it('lists installed emulators no system uses', () => {
    const others = otherEmulators([entry('RetroArch'), entry('Snes9x'), entry('Mednafen')], rows);
    expect(others.map((e) => e.name)).toEqual(['Snes9x', 'Mednafen']);
    expect(otherEmulatorNote(canPlay('Snes9x', rows))).toBe('Not used by any system. It can play Super Nintendo.');
    expect(otherEmulatorNote(canPlay('Mednafen', rows))).toBe('Not used by any system on your server.');
  });

  it('asks for firmware only for each system and its own emulator', () => {
    expect(systemFirmwareRequests(rows).map((r) => r.key)).toEqual(['2:RetroArch', '1:RetroArch', '3:RetroArch']);
  });
});

describe('companions', () => {
  const SHAD = 'ShadPS4 (Playstation 4)';
  const QT = 'ShadPS4 Qt Launcher';
  const link = (parent: string | null, companion: string | null): CompanionLink => ({
    parent_profile: SHAD,
    parent_entry: parent,
    companion_profile: QT,
    companion_entry: companion,
    companion_source_id: 'shadps4-emu/shadps4-qtlauncher',
  });
  const ps4 = setup('PlayStation 4', 'Sony PlayStation 4', []);
  const entry = (name: string): EmulatorEntry => ({ name, path: `/emu/${name}`, args: '' });
  const withShad = buildSystemRows([ps4], platforms, defaults({}), { 'Sony PlayStation 4': [SHAD] });
  const withNone = buildSystemRows([ps4], platforms, defaults({}), {});

  it('lists a companion under its parent emulator, installed or not', () => {
    expect(companionsOf(SHAD, [link(SHAD, QT)])).toEqual([
      { profile: QT, entry: QT, sourceId: 'shadps4-emu/shadps4-qtlauncher' },
    ]);
    expect(companionsOf(SHAD, [link(SHAD, null)])).toEqual([
      { profile: QT, entry: null, sourceId: 'shadps4-emu/shadps4-qtlauncher' },
    ]);
    expect(companionsOf('PCSX2', [link(SHAD, QT)])).toEqual([]);
    expect(companionsOf('', [link(null, QT)])).toEqual([]);
  });

  it('keeps a companion of a system’s emulator out of Other emulators', () => {
    expect(withShad[0].emulator).toBe(SHAD);
    const others = otherEmulators([entry(SHAD), entry(QT), entry('Dolphin')], withShad, [link(SHAD, QT)]);
    expect(others.map((e) => e.name)).toEqual(['Dolphin']);
  });

  it('leaves the companion under Other emulators, saying whose it is, when no system uses its parent', () => {
    const lone = otherEmulators([entry(QT)], withNone, [link(null, QT)]);
    expect(lone.map((e) => e.name)).toEqual([QT]);
    expect(otherEmulatorNote(canPlay(QT, withNone), companionParent(QT, [link(null, QT)]))).toBe(
      `Companion to ${SHAD}.`,
    );
    const both = otherEmulators([entry(SHAD), entry(QT)], withNone, [link(SHAD, QT)]);
    expect(both.map((e) => e.name)).toEqual([SHAD, QT]);
    expect(companionParent(QT, [link('My ShadPS4', QT)])).toBe('My ShadPS4');
    expect(companionParent('Dolphin', [link(SHAD, QT)])).toBeNull();
  });

  it('counts a companion update as an update for the system', () => {
    const has = (names: string[]) => (n: string) => names.includes(n);
    expect(updateAvailable(SHAD, [link(SHAD, QT)], has([QT]))).toBe(true);
    expect(updateAvailable(SHAD, [link(SHAD, QT)], has([SHAD]))).toBe(true);
    expect(updateAvailable(SHAD, [link(SHAD, QT)], has([]))).toBe(false);
    expect(updateAvailable(SHAD, [link(SHAD, null)], has([QT]))).toBe(false);
    expect(updateAvailable('', [link(SHAD, QT)], has([QT]))).toBe(false);
  });

  it('explains a missing companion', () => {
    expect(missingCompanionText({ profile: QT }, SHAD)).toBe(
      `${QT} is not installed. Installing or updating ShadPS4 adds it too.`,
    );
  });
});

describe('texts', () => {
  it('explains a system with no emulator', () => {
    const [noOption] = buildSystemRows([zeebo], platforms, defaults({}), {});
    expect(missingText(noOption)).toMatch(/^No emulator GRID installs plays this system\. .*Add an emulator manually\.$/);
    const [installable] = buildSystemRows([ps2], platforms, defaults({}), {});
    expect(missingText(installable)).toBe("Games for this system can't start yet. PCSX2 is recommended.");
  });

  it('shows the version, and up to date once a check says so', () => {
    expect(versionText({ source_installed_tag: 'v2.0.2' }, undefined)).toBe('v2.0.2');
    expect(versionText(undefined, { installed_display: 'v2.0.2', up_to_date: true })).toBe('v2.0.2 · up to date');
    expect(versionText({}, undefined)).toBe('');
  });

  it('makes test ids', () => {
    expect(testKeyOf('PCSX2 (Playstation 2)')).toBe('pcsx2-playstation-2');
    expect(testKeyOf('other-Redream')).toBe('other-redream');
  });
});
