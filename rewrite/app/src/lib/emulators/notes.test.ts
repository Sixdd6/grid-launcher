import { describe, expect, it } from 'vitest';
import { dynamicEmulatorNotes, emulatorNotes } from './notes';

describe('emulatorNotes', () => {
  it('returns the Azahar note verbatim', () => {
    expect(emulatorNotes('Azahar')).toEqual([
      {
        key: 'azahar',
        text: 'Controller setup: Settings → Controls → Auto Map  ·  Press Esc to close emulator',
      },
    ]);
  });

  it('returns the Eden note verbatim', () => {
    expect(emulatorNotes('Eden')).toEqual([
      { key: 'eden', text: 'Controller setup: Controls → Configure → Map Controller' },
    ]);
  });

  it('returns the xemu note verbatim', () => {
    expect(emulatorNotes('xemu')).toEqual([
      {
        key: 'xemu',
        text: 'Controller setup: required to connect a controller first — layout is auto-detected',
      },
    ]);
  });

  it('returns the DuckStation note verbatim', () => {
    expect(emulatorNotes('DuckStation')).toEqual([
      {
        key: 'duckstation',
        text: 'RetroAchievements: Configure login via Emulator Settings → Achievements (tokens are machine-encrypted)',
      },
    ]);
  });

  it('returns the RPCS3 note verbatim', () => {
    expect(emulatorNotes('RPCS3')).toEqual([
      { key: 'rpcs3', text: 'Controller setup: Configure controllers via Config → Pads' },
    ]);
  });

  it('matches case-insensitively anywhere in the name, like the reference token test', () => {
    expect(emulatorNotes('My DuckStation build').map((n) => n.key)).toEqual(['duckstation']);
    expect(emulatorNotes('  rpcs3-nightly  ').map((n) => n.key)).toEqual(['rpcs3']);
  });

  it('returns nothing for an emulator with no note', () => {
    expect(emulatorNotes('RetroArch (Multi-System)')).toEqual([]);
    expect(emulatorNotes('')).toEqual([]);
  });

  it('keeps the reference order when a name matches more than one token', () => {
    expect(emulatorNotes('Eden and xemu combo').map((n) => n.key)).toEqual(['eden', 'xemu']);
  });
});

describe('dynamicEmulatorNotes', () => {
  const KEYS_NOTE = {
    key: 'eden-keys',
    text: 'Switch keys (prod.keys) must be placed in user/keys/ before playing games.',
  };
  const FIRMWARE_NOTE = {
    key: 'eden-firmware',
    text: 'Switch firmware must be installed via Emulation → Install Firmware before playing games.',
  };

  it('returns both notes verbatim, keys first, when both facts are false', () => {
    expect(
      dynamicEmulatorNotes('Eden', { eden_keys_present: false, eden_firmware_present: false }),
    ).toEqual([KEYS_NOTE, FIRMWARE_NOTE]);
  });

  it('returns only the missing half', () => {
    expect(
      dynamicEmulatorNotes('Eden', { eden_keys_present: true, eden_firmware_present: false }),
    ).toEqual([FIRMWARE_NOTE]);
    expect(
      dynamicEmulatorNotes('Eden', { eden_keys_present: false, eden_firmware_present: true }),
    ).toEqual([KEYS_NOTE]);
  });

  it('returns nothing when both facts are true', () => {
    expect(
      dynamicEmulatorNotes('Eden', { eden_keys_present: true, eden_firmware_present: true }),
    ).toEqual([]);
  });

  it('returns nothing for a non-Eden emulator, whatever the facts say', () => {
    expect(
      dynamicEmulatorNotes('RPCS3', { eden_keys_present: false, eden_firmware_present: false }),
    ).toEqual([]);
    expect(
      dynamicEmulatorNotes('', { eden_keys_present: false, eden_firmware_present: false }),
    ).toEqual([]);
  });

  it('returns nothing when the facts have not arrived yet', () => {
    expect(dynamicEmulatorNotes('Eden', undefined)).toEqual([]);
  });

  it('matches the Eden token case-insensitively anywhere in the name', () => {
    expect(
      dynamicEmulatorNotes('  My eden nightly  ', {
        eden_keys_present: false,
        eden_firmware_present: true,
      }).map((n) => n.key),
    ).toEqual(['eden-keys']);
  });
});
