import { describe, expect, it } from 'vitest';
import type { LaunchDefaults, Platform } from '../api';
import { emulatorFirmwareRows, firmwareAction, firmwarePlatforms, type EmulatorFirmwareRow } from './firmware';

function platform(id: number, name: string): Platform {
  return { id, name, slug: name.toLowerCase(), rom_count: 1, custom_name: null, display_name: name };
}

const platforms = [platform(1, 'PlayStation'), platform(2, 'PlayStation 2'), platform(3, 'Dreamcast')];
const defaults: LaunchDefaults = {
  default_emulators: { PlayStation: 'DuckStation', 'PlayStation 2': 'PCSX2', Dreamcast: 'Gone' },
  retroarch_cores: {},
  launch_args: '',
};

describe('firmwarePlatforms', () => {
  it('is the platforms whose default emulator is configured', () => {
    expect(firmwarePlatforms(platforms, defaults, ['DuckStation', 'PCSX2']).map((p) => p.id)).toEqual([1, 2]);
  });

  it('is empty before the defaults load', () => {
    expect(firmwarePlatforms(platforms, null, ['DuckStation'])).toEqual([]);
  });
});

describe('emulatorFirmwareRows', () => {
  const statuses = {
    1: { file_count: 3, has_default_emulator: true, installed: false, available: true },
    2: { file_count: 0, has_default_emulator: true, installed: false, available: true },
  };

  it('lists the platforms the emulator is the default for that have server firmware', () => {
    expect(emulatorFirmwareRows('DuckStation', platforms, defaults, statuses)).toEqual([
      { platformId: 1, platformName: 'PlayStation', label: 'PlayStation firmware · 3 files', installed: false, available: true },
    ]);
  });

  it('leaves out a platform with no firmware files or no status yet', () => {
    expect(emulatorFirmwareRows('PCSX2', platforms, defaults, statuses)).toEqual([]);
    expect(emulatorFirmwareRows('DuckStation', platforms, defaults, {})).toEqual([]);
  });

  it('says when the firmware is already installed', () => {
    const done = { 1: { file_count: 3, has_default_emulator: true, installed: true, available: true } };
    expect(emulatorFirmwareRows('DuckStation', platforms, defaults, done)[0].installed).toBe(true);
  });

  it('says one file in the singular', () => {
    const one = { 1: { file_count: 1, has_default_emulator: true, installed: false, available: true } };
    expect(emulatorFirmwareRows('DuckStation', platforms, defaults, one)[0].label).toBe('PlayStation firmware · 1 file');
  });
});

describe('firmwareAction', () => {
  const row = (installed: boolean, available: boolean): EmulatorFirmwareRow => ({
    platformId: 1,
    platformName: 'PlayStation',
    label: 'PlayStation firmware · 1 file',
    installed,
    available,
  });

  it('offers Install for available firmware not yet in place', () => {
    expect(firmwareAction(row(false, true), false, false)).toBe('install');
  });

  it('says Installed once every file is in place', () => {
    expect(firmwareAction(row(true, true), false, false)).toBe('installed');
  });

  it('says Unavailable when nothing on the server can land for this emulator', () => {
    expect(firmwareAction(row(false, false), false, false)).toBe('unavailable');
  });

  it('shows a running install or recheck over everything else', () => {
    expect(firmwareAction(row(true, true), true, false)).toBe('requested');
    expect(firmwareAction(row(false, false), false, true)).toBe('checking');
  });
});
