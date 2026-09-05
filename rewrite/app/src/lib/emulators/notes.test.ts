import { describe, expect, it } from 'vitest';
import { emulatorNotes } from './notes';

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
