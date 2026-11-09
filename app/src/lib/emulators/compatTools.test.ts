import { describe, expect, it } from 'vitest';
import type { DownloadEntry, UmuRunStatus } from '../api';
import {
  compatToolTerminalSignature,
  isWindowsHost,
  UMU_SOURCE_ID,
  umuActionLabel,
  umuInstallBusy,
  umuStatusLabel,
} from './compatTools';

function entry(overrides: Partial<DownloadEntry>): DownloadEntry {
  return {
    id: 1,
    job: 'emulator',
    kind: 'compat_tool',
    rom_id: 0,
    source_id: 'ge-proton/GE-Proton8-25',
    title: 'GE-Proton8-25',
    platform: '',
    status: 'downloading',
    downloaded_bytes: 0,
    total_bytes: 0,
    speed_bps: 0,
    install_processed_bytes: 0,
    install_total_bytes: 0,
    error: '',
    detail: '',
    drive_id: '',
    drive_name: '',
    drive_label: '',
    batch: 0,
    ...overrides,
  };
}

describe('isWindowsHost (re-exported from details/actions, not redefined)', () => {
  it('is true for Win32', () => {
    expect(isWindowsHost('Win32')).toBe(true);
  });

  it('is false for a non-Windows platform', () => {
    expect(isWindowsHost('Linux x86_64')).toBe(false);
  });
});

describe('compatToolTerminalSignature', () => {
  it('is empty with no entries', () => {
    expect(compatToolTerminalSignature([])).toBe('');
  });

  it('is empty when the only compat_tool entries are still live', () => {
    const entries = [entry({ id: 1, status: 'downloading' }), entry({ id: 2, status: 'queued' })];
    expect(compatToolTerminalSignature(entries)).toBe('');
  });

  it('ignores terminal entries of a different kind', () => {
    const entries = [entry({ id: 1, kind: 'emulator', status: 'completed' })];
    expect(compatToolTerminalSignature(entries)).toBe('');
  });

  it('includes a completed compat_tool entry', () => {
    const entries = [entry({ id: 1, status: 'completed' })];
    expect(compatToolTerminalSignature(entries)).toBe('1:completed');
  });

  it('includes failed and cancelled compat_tool entries', () => {
    const entries = [entry({ id: 1, status: 'failed' }), entry({ id: 2, status: 'cancelled' })];
    expect(compatToolTerminalSignature(entries)).toBe('1:failed,2:cancelled');
  });

  it('changes when a live entry transitions to a terminal status', () => {
    const before = compatToolTerminalSignature([entry({ id: 1, status: 'installing' })]);
    const after = compatToolTerminalSignature([entry({ id: 1, status: 'completed' })]);
    expect(before).toBe('');
    expect(after).toBe('1:completed');
    expect(before).not.toBe(after);
  });

  it('mixes terminal compat_tool entries with live and other-kind entries, keeping only terminal compat_tool ones', () => {
    const entries = [
      entry({ id: 1, status: 'completed' }),
      entry({ id: 2, status: 'downloading' }),
      entry({ id: 3, kind: 'emulator', status: 'failed' }),
    ];
    expect(compatToolTerminalSignature(entries)).toBe('1:completed');
  });
});

function umu(overrides: Partial<UmuRunStatus>): UmuRunStatus {
  return { source: 'missing', path: '', release_tag: '', source_id: UMU_SOURCE_ID, ...overrides };
}

describe('umuStatusLabel', () => {
  it('names the system copy with its path', () => {
    expect(umuStatusLabel(umu({ source: 'system', path: '/usr/bin/umu-run' }))).toBe(
      'umu-run: system — /usr/bin/umu-run'
    );
  });

  it('names the GRID-managed copy with its release', () => {
    expect(umuStatusLabel(umu({ source: 'managed', path: '/d/umu-run', release_tag: '1.4.4' }))).toBe(
      'umu-run: GRID-managed 1.4.4'
    );
    expect(umuStatusLabel(umu({ source: 'managed', path: '/d/umu-run' }))).toBe('umu-run: GRID-managed');
  });

  it('says when nothing is installed', () => {
    expect(umuStatusLabel(umu({}))).toBe('umu-run: not installed');
  });
});

describe('umuActionLabel', () => {
  it('offers Install, Reinstall, or nothing for the system copy', () => {
    expect(umuActionLabel(umu({}))).toBe('Install');
    expect(umuActionLabel(umu({ source: 'managed' }))).toBe('Reinstall');
    expect(umuActionLabel(umu({ source: 'system' }))).toBeNull();
  });
});

describe('umuInstallBusy', () => {
  it('is true only while a umu-launcher row is live', () => {
    const live = entry({ source_id: UMU_SOURCE_ID, status: 'queued' });
    expect(umuInstallBusy([live])).toBe(true);
    expect(umuInstallBusy([{ ...live, status: 'installing' }])).toBe(true);
    expect(umuInstallBusy([{ ...live, status: 'failed' }])).toBe(false);
    expect(umuInstallBusy([{ ...live, status: 'completed' }])).toBe(false);
    expect(umuInstallBusy([entry({ status: 'downloading' })])).toBe(false);
    expect(umuInstallBusy([])).toBe(false);
  });
});
