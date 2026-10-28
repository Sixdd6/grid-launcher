import { describe, expect, it } from 'vitest';
import type { DownloadEntry, SetupOutcome } from '../api';
import { assignedCount, headline, outcomeNotes, summarize } from './progress';

function row(id: number, source_id: string, status: DownloadEntry['status'], job: DownloadEntry['job'] = 'emulator'): DownloadEntry {
  return { id, source_id, status, job } as DownloadEntry;
}

const queued = ['PCSX2/pcsx2', 'libretro/RetroArch', 'redream/redream'];

describe('summarize', () => {
  it('counts done, running and failed from the emulators’ Downloads rows', () => {
    const p = summarize(queued, [
      row(1, 'PCSX2/pcsx2', 'completed'),
      row(2, 'libretro/RetroArch', 'downloading'),
      row(3, 'redream/redream', 'failed'),
    ]);
    expect([p.total, p.done, p.running, p.failed.map((f) => f.id)]).toEqual([3, 1, 1, [3]]);
  });

  it('counts an emulator with no row yet as running', () => {
    expect(summarize(queued, []).running).toBe(3);
  });

  it('lets the newest row decide after a retry', () => {
    const p = summarize(['redream/redream'], [row(3, 'redream/redream', 'failed'), row(9, 'redream/redream', 'completed')]);
    expect([p.done, p.failed.length]).toEqual([1, 0]);
  });

  it('ignores rows that are not setup’s emulators', () => {
    const p = summarize(['PCSX2/pcsx2'], [row(1, 'PCSX2/pcsx2', 'completed'), row(2, 'other/emu', 'failed'), row(3, 'PCSX2/pcsx2', 'failed', 'game')]);
    expect([p.done, p.failed.length]).toEqual([1, 0]);
  });
});

describe('summarize with server packages', () => {
  it('follows a server package by its ROM’s game row', () => {
    const p = summarize(
      ['PCSX2/pcsx2'],
      [row(1, 'PCSX2/pcsx2', 'completed'), { ...row(2, '', 'installing', 'game'), rom_id: 3432 } as DownloadEntry],
      [3432],
    );
    expect([p.total, p.done, p.running]).toEqual([2, 1, 1]);
  });

  it('ignores game rows for other ROMs', () => {
    const p = summarize([], [{ ...row(5, '', 'failed', 'game'), rom_id: 99 } as DownloadEntry], [3432]);
    expect([p.total, p.running, p.failed.length]).toEqual([1, 1, 0]);
  });
});

describe('headline', () => {
  it('reads as progress, then as the end state', () => {
    expect(headline({ total: 3, done: 1, running: 2, failed: [] })).toBe('Setting up 3 emulators · 1 done');
    expect(headline({ total: 3, done: 3, running: 0, failed: [] })).toBe('3 emulators installed');
    expect(headline({ total: 1, done: 0, running: 0, failed: [row(1, 'a', 'failed')] })).toBe('0 of 1 emulator installed');
  });
});

describe('outcomes', () => {
  const outcomes: SetupOutcome[] = [
    { outcome: 'assigned', platform: 'SNES', emulator: 'RetroArch (Multi-System)', core: 'snes9x' },
    { outcome: 'core_missing', platform: 'Arcade', emulator: 'RetroArch (Multi-System)' },
    { outcome: 'kept_none', platform: 'PS2' },
  ];

  it('explains a platform RetroArch had no core for', () => {
    expect(outcomeNotes(outcomes)).toEqual(['Arcade: RetroArch has no core for it, so it was left without an emulator.']);
  });

  it('counts the platforms that got their emulator', () => {
    expect(assignedCount(outcomes)).toBe(1);
  });
});
