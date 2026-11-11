import { describe, expect, it } from 'vitest';
import type { EmulatorEntry, LaunchDefaults, Platform, PlatformSetup, SetupOption } from '../../../../api';
import { buildSystemRows } from '../../../../systems/rows';
import {
  CHECK_RUNNING,
  checkingText,
  installingCount,
  OTHER_EMPTY,
  otherItems,
  orderSystems,
  pillOf,
  recButton,
  REC_RUNNING,
  systemItem,
  systemSub,
  systemX,
  updatesSub,
  windowsItem,
} from './systems';

const opt = (source_id: string, name: string, extra: Partial<SetupOption> = {}): SetupOption => ({
  source_id,
  name,
  kind: 'standalone',
  installed: false,
  server: null,
  needs_core: false,
  ...extra,
});

function setup(label: string, name: string, options: SetupOption[], core: string | null = null): PlatformSetup {
  return { label, name, slug: label.toLowerCase().replace(/\s+/g, ''), rom_count: 4, options, recommended: options[0]?.source_id ?? null, core };
}

const platform = (id: number, name: string): Platform => ({ id, name, slug: name.toLowerCase(), rom_count: 4, custom_name: null, display_name: name });

const entry = (name: string): EmulatorEntry => ({ name, path: `/emu/${name}`, args: '%rom%' }) as EmulatorEntry;

const ymir = opt('ymir/ymir', 'Ymir (Sega Saturn)');
const pcsx2 = opt('PCSX2/pcsx2', 'PCSX2 (Playstation 2)', { installed: true, entry: 'PCSX2 (Playstation 2)' });
const rows = buildSystemRows(
  [setup('PlayStation 2', 'Sony PlayStation 2', [pcsx2]), setup('Saturn', 'Sega Saturn', [ymir]), setup('Zeebo', 'Zeebo', [])],
  [platform(1, 'Sony PlayStation 2'), platform(2, 'Sega Saturn'), platform(3, 'Zeebo')],
  { default_emulators: { 'PlayStation 2': 'PCSX2 (Playstation 2)' }, retroarch_cores: {}, launch_args: '' } as LaunchDefaults,
  { 'Sony PlayStation 2': ['PCSX2 (Playstation 2)'] },
);

describe('Settings › Systems in TV mode', () => {
  it('orders systems with no emulator first, then the rest; Windows goes last (item 255)', () => {
    expect(orderSystems(rows).map((r) => r.label)).toEqual(['Saturn', 'Zeebo', 'PlayStation 2']);
    expect(orderSystems([{ emulator: 'A' }, { emulator: '' }, { emulator: 'B' }, { emulator: '' }])).toEqual([
      { emulator: '' },
      { emulator: '' },
      { emulator: 'A' },
      { emulator: 'B' },
    ]);
  });

  it('a row shows its emulator (and core) or why its games cannot start (item 256)', () => {
    const [saturn, zeebo, ps2] = orderSystems(rows);
    expect(systemSub(ps2, null)).toBe('PCSX2 (Playstation 2)');
    expect(systemSub({ emulator: 'RetroArch', recommended: null }, 'snes9x')).toBe('RetroArch · snes9x');
    expect(systemSub(saturn, null)).toBe("Games for this system can't start yet. Ymir is recommended.");
    expect(systemSub(zeebo, null)).toBe('GRID has no emulator for this system. You can add your own.');
  });

  it('the status pill: Ready with the green dot, else the status in the warning style (item 256)', () => {
    expect(pillOf('ready')).toEqual({ pill: 'Ready', ok: true });
    expect(pillOf('none')).toEqual({ pill: 'No emulator', ok: false });
    expect(pillOf('update')).toEqual({ pill: 'Update available', ok: false });
    expect(pillOf('firmware')).toEqual({ pill: 'Firmware needed', ok: false });
    expect(pillOf('installing', 'Installing firmware…')).toEqual({ pill: 'Installing firmware…', ok: false });
  });

  it('X is Change emulator, or Install <recommended> for a system with none (item 257)', () => {
    const [saturn, zeebo, ps2] = orderSystems(rows);
    expect(systemX(ps2, 'ready')).toBe('Change emulator');
    expect(systemX(saturn, 'none')).toBe('Install Ymir');
    expect(systemX(zeebo, 'none')).toBe('Change emulator');
    expect(systemX(saturn, 'installing')).toBeNull();
    const item = systemItem(saturn, 'none', null, null);
    expect(item).toMatchObject({ k: 'sy-saturn', testKey: 'saturn', label: 'Saturn', pill: 'No emulator', ok: false, x: 'Install Ymir' });
    expect(windowsItem('GE-Proton9-20 (downloaded by GRID)', 'ready')).toMatchObject({
      k: 'sy-windows',
      label: 'Windows',
      sub: 'GE-Proton9-20 (downloaded by GRID)',
      pill: 'Ready',
      x: 'Change emulator',
    });
    expect(windowsItem(null, 'none').sub).toBe('Choose a Proton version');
  });

  it('Install recommended (N), then "Installing N emulators…" that can’t be pressed, then nothing (items 252–254)', () => {
    expect(recButton({ queueing: 0, todo: 3, inFlight: 0 })).toEqual({ label: 'Install recommended (3)', disabled: false });
    expect(recButton({ queueing: 3, todo: 0, inFlight: 0 })).toEqual({ label: 'Installing 3 emulators…', disabled: true });
    expect(recButton({ queueing: 0, todo: 0, inFlight: 1 })).toEqual({ label: 'Installing 1 emulator…', disabled: true });
    expect(recButton({ queueing: 0, todo: 1, inFlight: 2 })).toEqual({ label: 'Install recommended (1)', disabled: false });
    expect(recButton({ queueing: 0, todo: 0, inFlight: 0 })).toBeNull();
    // Only no-emulator systems installing count; a system nothing installs never does.
    expect(installingCount(rows, [], (r) => r.label !== 'PlayStation 2')).toBe(2);
    expect(installingCount(rows, [], () => true)).toBe(2);
    // A system Install recommended was pressed for counts even with a fallback emulator shown.
    expect(installingCount(rows, ['PlayStation 2'], () => true)).toBe(3);
    expect(installingCount(rows, ['PlayStation 2'], () => false)).toBe(0);
  });

  it('Other emulators: each with what it could play, or the note (item 258)', () => {
    expect(OTHER_EMPTY).toBe('No other emulators');
    expect(otherItems([entry('Mednafen')], rows, [])).toEqual([
      { k: 'sy-other-mednafen', testKey: 'mednafen', name: 'Mednafen', note: 'Not used by any system on your server.' },
    ]);
    expect(
      otherItems([entry('Qt Launcher')], rows, [
        { parent_profile: 'ShadPS4', parent_entry: 'ShadPS4', companion_profile: 'Qt Launcher', companion_entry: 'Qt Launcher', companion_source_id: null },
      ])[0].note,
    ).toBe('Companion to ShadPS4.');
  });

  it('Check for updates: the count, then each update found (item 259)', () => {
    expect(checkingText(5)).toBe('Checking 5 emulators for updates…');
    expect(checkingText(1)).toBe('Checking 1 emulator for updates…');
    expect(updatesSub([{ name: 'PCSX2 (Playstation 2)', version: '2.3.4' }])).toBe('PCSX2 2.3.4');
    expect(updatesSub([])).toBe('');
  });
});

describe('disabled reasons (consultant decisions)', () => {
  it('words the running Install recommended and Check for updates', () => {
    expect(REC_RUNNING).toBe('Installing… see Downloads');
    expect(CHECK_RUNNING).toBe('Already checking for updates.');
  });
});
