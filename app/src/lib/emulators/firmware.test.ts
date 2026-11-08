import { describe, expect, it } from 'vitest';
import type { LaunchDefaults, Platform } from '../api';
import {
  emulatorFirmwareRows,
  firmwareAction,
  firmwareFinishedToast,
  firmwareKey,
  firmwareStartedToast,
  type EmulatorFirmwareRow,
} from './firmware';

function platform(id: number, name: string, slug = name.toLowerCase()): Platform {
  return { id, name, slug, rom_count: 1, custom_name: null, display_name: name };
}

const platforms = [
  platform(1, 'PlayStation'),
  platform(2, 'PlayStation 2'),
  platform(3, 'Xbox'),
  platform(4, 'Emulators'),
  platform(5, 'Windows', 'win'),
];
// `compatible_emulators`, keyed by raw platform name. Whether an emulator
// is a platform's default does not enter into it.
const compatible: Record<string, string[]> = {
  PlayStation: ['DuckStation', 'RetroArch'],
  'PlayStation 2': ['PCSX2'],
  Xbox: ['Xemu (Xbox)'],
  Emulators: ['Xemu (Xbox)'],
  Windows: ['Xemu (Xbox)'],
};

const defaults: LaunchDefaults = {
  default_emulators: { PlayStation: 'DuckStation', 'PlayStation 2': 'PCSX2', Xbox: '<none>' },
  retroarch_cores: {},
  launch_args: '',
};

describe('emulatorFirmwareRows', () => {
  const status = (file_count: number, installed = false, available = true) => ({
    file_count,
    has_default_emulator: true,
    installed,
    available,
  });
  const statuses = {
    [firmwareKey('DuckStation', 1)]: status(3),
    [firmwareKey('PCSX2', 2)]: status(0),
    [firmwareKey('Xemu (Xbox)', 3)]: status(4),
  };

  it('lists every supported platform with server firmware, default or not', () => {
    expect(emulatorFirmwareRows('Xemu (Xbox)', platforms, compatible, defaults, statuses)).toEqual([
      {
        key: firmwareKey('Xemu (Xbox)', 3),
        platformId: 3,
        platformName: 'Xbox',
        label: 'Xbox firmware · 4 files',
        installed: false,
        available: true,
      },
    ]);
  });

  it('tells only the default emulator that nothing fits', () => {
    const none = {
      [firmwareKey('DuckStation', 1)]: status(3, false, false),
      [firmwareKey('RetroArch', 1)]: status(3, false, false),
      [firmwareKey('Xemu (Xbox)', 3)]: status(4, false, false),
    };
    expect(emulatorFirmwareRows('DuckStation', platforms, compatible, defaults, none)[0].available).toBe(false);
    expect(emulatorFirmwareRows('RetroArch', platforms, compatible, defaults, none)).toEqual([]);
    expect(emulatorFirmwareRows('Xemu (Xbox)', platforms, compatible, defaults, none)).toEqual([]);
  });

  it('keeps each emulator apart on a shared platform', () => {
    expect(emulatorFirmwareRows('DuckStation', platforms, compatible, defaults, statuses)).toHaveLength(1);
    expect(emulatorFirmwareRows('RetroArch', platforms, compatible, defaults, statuses)).toEqual([]);
  });

  it('leaves out a platform with no firmware files or no status yet', () => {
    expect(emulatorFirmwareRows('PCSX2', platforms, compatible, defaults, statuses)).toEqual([]);
    expect(emulatorFirmwareRows('DuckStation', platforms, compatible, defaults, {})).toEqual([]);
  });

  it('says when the firmware is already installed', () => {
    const done = { [firmwareKey('DuckStation', 1)]: status(3, true) };
    expect(emulatorFirmwareRows('DuckStation', platforms, compatible, defaults, done)[0].installed).toBe(true);
  });

  it('says one file in the singular', () => {
    const one = { [firmwareKey('DuckStation', 1)]: status(1) };
    expect(emulatorFirmwareRows('DuckStation', platforms, compatible, defaults, one)[0].label).toBe(
      'PlayStation firmware · 1 file'
    );
  });
});

describe('firmwareAction', () => {
  const row = (installed: boolean, available: boolean): EmulatorFirmwareRow => ({
    key: '1:DuckStation',
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

  it('says Not needed when nothing on the server can land for this emulator', () => {
    expect(firmwareAction(row(false, false), false, false)).toBe('unavailable');
  });

  it('shows a running install or recheck over everything else', () => {
    expect(firmwareAction(row(true, true), true, false)).toBe('requested');
    expect(firmwareAction(row(false, false), false, true)).toBe('checking');
  });
});

describe('firmware toasts', () => {
  it('says the install has started', () => {
    expect(firmwareStartedToast('Xbox', 'Xemu (Xbox)')).toBe('Installing Xbox firmware for Xemu (Xbox)…');
  });

  it('says it finished, or how many files did not make it', () => {
    expect(firmwareFinishedToast('Xbox', 'Xemu (Xbox)', true, 0)).toEqual({
      text: 'Xbox firmware installed for Xemu (Xbox)',
      level: 'success',
    });
    expect(firmwareFinishedToast('Xbox', 'Xemu (Xbox)', false, 1)).toEqual({
      text: 'Xbox firmware for Xemu (Xbox): 1 file could not be installed',
      level: 'error',
    });
    expect(firmwareFinishedToast('Xbox', 'Xemu (Xbox)', false, 2).text).toContain('2 files');
    expect(firmwareFinishedToast('Xbox', 'Xemu (Xbox)', false, 0)).toEqual({
      text: 'Could not install Xbox firmware for Xemu (Xbox)',
      level: 'error',
    });
  });
});
