import { describe, expect, it } from 'vitest';
import {
  emulatorChipLabel,
  firmwareChipLabel,
  firmwareChipLinks,
  platformCountsLine,
  showsChips,
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

describe('showsChips', () => {
  it('is hidden for native launch platforms', () => {
    expect(showsChips('Windows 9x', '')).toBe(false);
    expect(showsChips('Windows', '')).toBe(false);
    expect(showsChips('Linux', '')).toBe(false);
  });
  it('is hidden for the Emulators shelf', () => {
    expect(showsChips('Emulators', 'emulators')).toBe(false);
    expect(showsChips(' emulators ', '')).toBe(false);
  });
  it('shows for anything else, including empty', () => {
    expect(showsChips('Super Nintendo Entertainment System', '')).toBe(true);
    expect(showsChips('', '')).toBe(true);
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
    expect(firmwareChipLabel({ file_count: 0, has_default_emulator: true, installed: false, available: true })).toBe(
      'No server firmware',
    );
  });
  it('counts the files, singularising one', () => {
    expect(firmwareChipLabel({ file_count: 1, has_default_emulator: true, installed: false, available: true })).toBe(
      'Firmware: 1 file',
    );
    expect(firmwareChipLabel({ file_count: 4, has_default_emulator: true, installed: false, available: true })).toBe(
      'Firmware: 4 files',
    );
  });
  it('says when nothing on the server can be installed for the default emulator', () => {
    expect(firmwareChipLabel({ file_count: 2, has_default_emulator: true, installed: false, available: false })).toBe(
      'Firmware: 2 files · unavailable',
    );
  });
  it('says when the firmware is already installed', () => {
    expect(firmwareChipLabel({ file_count: 4, has_default_emulator: true, installed: true, available: true })).toBe(
      'Firmware: 4 files · installed',
    );
  });
  it('names the blocker when there is nowhere to put the firmware', () => {
    expect(firmwareChipLabel({ file_count: 4, has_default_emulator: false, installed: false, available: true })).toBe(
      'Firmware: 4 files — no default emulator',
    );
  });
});

describe('firmwareChipLinks', () => {
  it('links when the server has firmware, with or without an emulator to install it for', () => {
    expect(firmwareChipLinks({ file_count: 4, has_default_emulator: true, installed: false, available: true })).toBe(true);
    expect(firmwareChipLinks({ file_count: 4, has_default_emulator: false, installed: false, available: true })).toBe(true);
  });
  it('is a plain chip with no firmware, before the status, or when it is unavailable', () => {
    expect(firmwareChipLinks({ file_count: 0, has_default_emulator: true, installed: false, available: true })).toBe(false);
    expect(firmwareChipLinks(null)).toBe(false);
    expect(firmwareChipLinks('unavailable')).toBe(false);
  });
});
