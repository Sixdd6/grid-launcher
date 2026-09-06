import { describe, expect, it } from 'vitest';
import {
  emulatorChipLabel,
  firmwareChipLabel,
  firmwareInstallable,
  platformCountsLine,
  showsEmulatorChip,
} from './header';
import { NO_EMULATOR_MARKER } from '../emulators/defaults';

describe('platformCountsLine', () => {
  it('reads as a sentence for the ordinary case', () => {
    expect(platformCountsLine(42, 7)).toBe('42 games · 7 installed');
  });
  it('singularises one game', () => {
    expect(platformCountsLine(1, 0)).toBe('1 game · 0 installed');
  });
  it('handles an empty platform without a stray dash', () => {
    expect(platformCountsLine(0, 0)).toBe('0 games · 0 installed');
  });
});

describe('emulatorChipLabel', () => {
  it('names the default emulator', () => {
    expect(emulatorChipLabel('RetroArch')).toBe('Emulator: RetroArch');
  });
  it('says so plainly when there is none, blank or whitespace', () => {
    expect(emulatorChipLabel('')).toBe('No default emulator');
    expect(emulatorChipLabel('   ')).toBe('No default emulator');
  });
  it('never prints the reserved <none> marker back at the user', () => {
    expect(emulatorChipLabel(NO_EMULATOR_MARKER)).toBe('No default emulator');
    expect(emulatorChipLabel(`  ${NO_EMULATOR_MARKER}  `)).toBe('No default emulator');
  });
});

describe('showsEmulatorChip', () => {
  it('is hidden for native launch platforms', () => {
    expect(showsEmulatorChip('Windows 9x')).toBe(false);
    expect(showsEmulatorChip('Windows')).toBe(false);
    expect(showsEmulatorChip('Linux')).toBe(false);
  });
  it('shows for anything else, including empty', () => {
    expect(showsEmulatorChip('Super Nintendo Entertainment System')).toBe(true);
    expect(showsEmulatorChip('')).toBe(true);
  });
});

describe('firmwareChipLabel', () => {
  it('says nothing is known while the status is still loading', () => {
    expect(firmwareChipLabel(null)).toBe('Firmware: checking…');
  });
  it('admits a refused status call rather than checking forever', () => {
    expect(firmwareChipLabel('unavailable')).toBe('Firmware: unavailable');
  });
  it('says plainly when the server offers none', () => {
    expect(firmwareChipLabel({ file_count: 0, has_default_emulator: true })).toBe(
      'No server firmware',
    );
  });
  it('counts the files, singularising one', () => {
    expect(firmwareChipLabel({ file_count: 1, has_default_emulator: true })).toBe(
      'Firmware: 1 file',
    );
    expect(firmwareChipLabel({ file_count: 4, has_default_emulator: true })).toBe(
      'Firmware: 4 files',
    );
  });
  it('names the blocker when there is nowhere to put the firmware', () => {
    expect(firmwareChipLabel({ file_count: 4, has_default_emulator: false })).toBe(
      'Firmware: 4 files — no default emulator',
    );
  });
});

describe('firmwareInstallable', () => {
  it('needs both files on the server and somewhere to put them', () => {
    expect(firmwareInstallable({ file_count: 4, has_default_emulator: true })).toBe(true);
    expect(firmwareInstallable({ file_count: 0, has_default_emulator: true })).toBe(false);
    expect(firmwareInstallable({ file_count: 4, has_default_emulator: false })).toBe(false);
    expect(firmwareInstallable(null)).toBe(false);
    expect(firmwareInstallable('unavailable')).toBe(false);
  });
});
