import { describe, expect, it } from 'vitest';
import type { DownloadEntry } from '../api';
import { emulatorTerminalSignature } from './installSignal';

function entry(overrides: Partial<DownloadEntry>): DownloadEntry {
  return {
    id: 1,
    job: 'emulator',
    kind: 'emulator',
    rom_id: 0,
    source_id: 'PCSX2/pcsx2',
    title: 'PCSX2',
    platform: 'Emulator',
    status: 'downloading',
    downloaded_bytes: 0,
    total_bytes: 0,
    speed_bps: 0,
    install_processed_bytes: 0,
    install_total_bytes: 0,
    error: '',
    ...overrides,
  };
}

describe('emulatorTerminalSignature', () => {
  it('lists terminal catalog emulator installs', () => {
    const entries = [
      entry({ id: 1, status: 'completed' }),
      entry({ id: 2, status: 'failed' }),
      entry({ id: 3, status: 'downloading' }),
    ];
    expect(emulatorTerminalSignature(entries)).toBe('1:completed,2:failed');
  });

  it('counts a server Emulators-platform package, a game job with the emulator kind', () => {
    const entries = [
      entry({ id: 4, job: 'game', kind: 'emulator', rom_id: 9, source_id: '', status: 'completed' }),
    ];
    expect(emulatorTerminalSignature(entries)).toBe('4:completed');
  });

  it('ignores ordinary games and firmware rows', () => {
    const entries = [
      entry({ id: 5, job: 'game', kind: 'base', status: 'completed' }),
      entry({ id: 6, job: 'firmware', kind: 'firmware', status: 'completed' }),
    ];
    expect(emulatorTerminalSignature(entries)).toBe('');
  });
});
