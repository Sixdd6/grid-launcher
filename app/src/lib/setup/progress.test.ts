import { describe, expect, it } from 'vitest';
import type { DownloadEntry, SetupOutcome } from '../api';
import {
  assignedCount,
  cardCountsDown,
  headline,
  outcomeNotes,
  readyCount,
  readyText,
  segments,
  summarize,
} from './progress';

function row(
  id: number,
  source_id: string,
  status: DownloadEntry['status'],
  job: DownloadEntry['job'] = 'emulator',
  title = '',
): DownloadEntry {
  return { id, source_id, status, job, title } as DownloadEntry;
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

describe('summarize with server builds', () => {
  it('follows a server build by its source id, like a catalog emulator', () => {
    const p = summarize(
      ['PCSX2/pcsx2', 'romm:pico/pico-8'],
      [row(1, 'PCSX2/pcsx2', 'completed'), row(2, 'romm:pico/pico-8', 'installing')],
    );
    expect([p.total, p.done, p.running]).toEqual([2, 1, 1]);
  });
});

describe('headline', () => {
  it('reads as progress, then as the end state', () => {
    expect(headline({ total: 3, done: 1, running: 2, failed: [] })).toBe('Setting up 3 installs · 1 done');
    expect(headline({ total: 3, done: 3, running: 0, failed: [] })).toBe('3 installs done');
    expect(headline({ total: 1, done: 1, running: 0, failed: [] })).toBe('1 install done');
    expect(headline({ total: 2, done: 1, running: 0, failed: [row(1, 'a', 'failed')] })).toBe('1 of 2 installs done');
    expect(headline({ total: 0, done: 0, running: 0, failed: [] })).toBe('Setup done');
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

describe('cardCountsDown', () => {
  const failed = { id: 9 } as DownloadEntry;

  it('holds while anything is still installing', () => {
    expect(cardCountsDown({ total: 2, done: 1, running: 1, failed: [] })).toBe(false);
  });

  it('counts down once every install is done', () => {
    expect(cardCountsDown({ total: 2, done: 2, running: 0, failed: [] })).toBe(true);
  });

  it('counts down once nothing is running, even with a failure', () => {
    expect(cardCountsDown({ total: 2, done: 1, running: 0, failed: [failed] })).toBe(true);
  });
});

describe('segments', () => {
  const PROTON = 'GloriousEggroll/proton-ge-custom';
  const names = { 'PCSX2/pcsx2': 'PCSX2', [PROTON]: 'GE-Proton' };

  it('gives one segment per install, in queue order, with its state', () => {
    const segs = segments(
      ['PCSX2/pcsx2', 'redream/redream', PROTON, 'libretro/RetroArch'],
      [
        row(1, 'PCSX2/pcsx2', 'completed'),
        row(2, 'redream/redream', 'downloading', 'emulator', 'Redream'),
        row(3, PROTON, 'queued'),
        row(4, 'libretro/RetroArch', 'failed'),
      ],
      names,
    );
    expect(segs.map((s) => [s.sourceId, s.state])).toEqual([
      ['PCSX2/pcsx2', 'done'],
      ['redream/redream', 'running'],
      [PROTON, 'waiting'],
      ['libretro/RetroArch', 'failed'],
    ]);
    expect(segs.map((s) => s.title)).toEqual([
      'PCSX2 · Installed',
      'Redream · Installing',
      'GE-Proton · Waiting',
      'libretro/RetroArch · Didn’t install',
    ]);
  });

  it('is waiting before its Downloads row exists, and follows a retry', () => {
    expect(segments(['PCSX2/pcsx2'], [], names)[0].state).toBe('waiting');
    const retried = segments(['PCSX2/pcsx2'], [row(1, 'PCSX2/pcsx2', 'failed'), row(5, 'PCSX2/pcsx2', 'installing')], names);
    expect(retried[0].state).toBe('running');
  });
});

describe('readyCount', () => {
  const PROTON = 'GloriousEggroll/proton-ge-custom';
  const assigned: SetupOutcome[] = [
    { outcome: 'assigned', platform: 'PS2', emulator: 'PCSX2', core: null },
    { outcome: 'assigned', platform: 'Dreamcast', emulator: 'Redream', core: null },
  ];

  it('counts Windows once its Proton download is done', () => {
    const waiting = segments([PROTON], [row(1, PROTON, 'installing')], {});
    const done = segments([PROTON], [row(1, PROTON, 'completed')], {});
    expect(readyCount(assigned, { queued: PROTON, set: false }, waiting)).toBe(2);
    expect(readyCount(assigned, { queued: PROTON, set: false }, done)).toBe(3);
  });

  it('counts Windows at once when an installed version was picked', () => {
    expect(readyCount(assigned, { queued: null, set: true }, [])).toBe(3);
    expect(readyCount([], { queued: null, set: false }, [])).toBe(0);
  });

  it('reads as a sentence', () => {
    expect(readyText(3)).toBe('3 systems are ready to play.');
    expect(readyText(1)).toBe('1 system is ready to play.');
  });
});
