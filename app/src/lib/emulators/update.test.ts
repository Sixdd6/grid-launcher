import { describe, expect, it } from 'vitest';
import { installedToastText, updateConfirmText, upToDateText } from './update';

const check = (installed: string, available: string, upToDate = false) => ({
  installed_display: installed,
  available_display: available,
  up_to_date: upToDate,
});

describe('updateConfirmText', () => {
  it('names the emulator and both versions, verbatim', () => {
    expect(updateConfirmText('PCSX2', check('v1.0', 'v2.0'))).toBe(
      'Update PCSX2?\n\nInstalled: v1.0\nAvailable: v2.0',
    );
  });

  it('carries an unknown installed version through as-is', () => {
    expect(updateConfirmText('Redream', check('unknown', 'Unknown (direct source)'))).toBe(
      'Update Redream?\n\nInstalled: unknown\nAvailable: Unknown (direct source)',
    );
  });
});

describe('upToDateText', () => {
  it('reports the available version', () => {
    expect(upToDateText(check('v2.0', 'v2.0', true))).toBe('Already up to date (v2.0).');
  });
});

describe('installedToastText', () => {
  it('says "Installed" for a first install and "Updated" for an update', () => {
    expect(installedToastText('PCSX2', true)).toBe("Installed emulator 'PCSX2' from source.");
    expect(installedToastText('PCSX2', false)).toBe("Updated emulator 'PCSX2' from source.");
  });
});
