import { describe, expect, it } from 'vitest';
import type { PlatformSetup, SetupOption } from '../../api';
import { relocatedNotice, unreachableBody, UNREACHABLE_TITLE } from './unreachable';
import { CHOOSE_INTRO, chooseRow, emulatorItems, emulatorListTitle, NO_EMULATOR, noEmulatorWhy, unmarked } from './choose';

const opt = (source_id: string, name: string, installed = false): SetupOption => ({
  source_id, name, kind: 'standalone', installed, server: null, needs_core: false,
});

const ps2: PlatformSetup = {
  label: 'PlayStation 2', name: 'PlayStation 2', slug: 'ps2', rom_count: 3,
  options: [opt('PCSX2/pcsx2', 'PCSX2'), opt('other/play', 'Play!', true)], recommended: 'PCSX2/pcsx2', core: null,
};
const zeebo: PlatformSetup = { label: 'Zeebo', name: 'Zeebo', slug: 'zeebo', rom_count: 1, options: [], recommended: null, core: null };

describe('Choose your systems rows (items 400–403)', () => {
  it('reads the intro', () => {
    const text = CHOOSE_INTRO.map((p) => (typeof p === 'string' ? p : `(${p.glyph})`)).join('');
    expect(text).toBe(
      'Pick the systems you play. GRID installs an emulator for each. You can change this later in Settings › Systems.',
    );
  });

  it('shows each system off at first with its games and the recommended emulator', () => {
    expect(chooseRow(ps2, {})).toEqual({ key: 'ch-ps2', label: 'PlayStation 2', sub: '3 games', value: 'PCSX2', on: false, disabled: false });
    expect(chooseRow({ ...ps2, rom_count: 1 }, {}).sub).toBe('1 game');
    expect(chooseRow(ps2, { 'PlayStation 2': 'other/play' })).toMatchObject({ on: true, value: 'Play!' });
  });

  it("can't tick a system nothing installable plays", () => {
    expect(chooseRow(zeebo, {})).toMatchObject({ disabled: true, value: NO_EMULATOR });
    expect(NO_EMULATOR).toBe('No emulator available');
  });

  it("says why an unavailable system can't be ticked (consultant, after P12)", () => {
    expect(noEmulatorWhy('Zeebo')).toBe(
      'GRID has no emulator for Zeebo. Add your own later in Settings › Systems.',
    );
  });

  it('lists the emulators in Installed and Download groups, then None, the current one checked', () => {
    expect(emulatorListTitle('PlayStation 2')).toBe('Emulator for PlayStation 2');
    expect(emulatorItems(ps2, {})).toEqual([
      { group: 'Installed' },
      { label: 'Play!', value: 'other/play', checked: false },
      { group: 'Download' },
      { label: 'PCSX2 (recommended)', value: 'PCSX2/pcsx2', checked: true },
      { group: ' ' },
      { label: 'None', value: null, checked: false },
    ]);
    expect(emulatorItems(ps2, { 'PlayStation 2': 'other/play' })[1]).toMatchObject({ checked: true });
  });

  it('drops the recommended mark from a value', () => {
    expect(unmarked('Install GE-Proton (latest, recommended)')).toBe('Install GE-Proton (latest)');
    expect(unmarked('PCSX2 (recommended)')).toBe('PCSX2');
    expect(unmarked('Proton 9.0')).toBe('Proton 9.0');
  });
});

describe('Unreachable library (items 409, 412)', () => {
  it('says the library is missing and GRID cannot run without it', () => {
    expect(UNREACHABLE_TITLE).toBe("Your library can't be reached");
    expect(unreachableBody('/run/media/USB-1TB/GRID')).toBe(
      "Can't find /run/media/USB-1TB/GRID. Connect its drive or locate the library.",
    );
  });

  it('notifies "Library relocated to <path>" / "No files were moved"', () => {
    expect(relocatedNotice({ status: 'relocated', library_path: '/mnt/GRID', report: [] })).toEqual({
      title: 'Library relocated to /mnt/GRID',
      sub: 'No files were moved',
      error: false,
    });
    expect(relocatedNotice({ status: 'relocated', library_path: '/mnt/GRID', report: ['a', 'b'] })).toEqual({
      title: 'Library relocated to /mnt/GRID',
      sub: "No files were moved · 2 paths weren't updated",
      error: true,
    });
  });
});
