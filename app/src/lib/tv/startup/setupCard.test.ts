import { describe, expect, it } from 'vitest';
import type { SetupOutcome } from '../../api';
import { entry } from '../fixtures.test-util';
import { segFill, setupCardView, stepLine, SETUP_CARD_DONE_MS, type SetupRunView } from './setupCard';

const emu = (id: number, source_id: string, o: Parameters<typeof entry>[0] = {}) =>
  entry({ id, job: 'emulator', kind: 'emulator', rom_id: 0, source_id, ...o });

const assigned = (platform: string): SetupOutcome => ({ outcome: 'assigned', platform, emulator: 'X', core: null });

function run(o: Partial<SetupRunView> = {}): SetupRunView {
  return {
    queued: ['PCSX2/pcsx2', 'inolen/redream'],
    errors: [],
    outcomes: [],
    names: { 'PCSX2/pcsx2': 'PCSX2', 'inolen/redream': 'Redream' },
    compat: { queued: null, set: false },
    ...o,
  };
}

describe('setup card segments (items 152–153)', () => {
  it('has one segment per install, each emulator once, empty while it waits', () => {
    const v = setupCardView(run({ queued: ['PCSX2/pcsx2', 'inolen/redream', 'PCSX2/pcsx2'] }), []);
    expect(v.segs).toEqual([
      { pct: 0, state: 'wait' },
      { pct: 0, state: 'wait' },
    ]);
  });

  it('fills the first half while downloading and the second while installing', () => {
    expect(segFill(emu(1, 'a', { status: 'downloading', downloaded_bytes: 50, total_bytes: 100 }))).toEqual({ pct: 25, state: 'run' });
    expect(segFill(emu(1, 'a', { status: 'downloading', downloaded_bytes: 0, total_bytes: 0 }))).toEqual({ pct: 0, state: 'run' });
    expect(segFill(emu(1, 'a', { status: 'installing', install_processed_bytes: 1, install_total_bytes: 2 }))).toEqual({ pct: 75, state: 'run' });
    expect(segFill(emu(1, 'a', { status: 'installing' }))).toEqual({ pct: 50, state: 'run' });
  });

  it('is full when done and red (full) when it failed', () => {
    expect(segFill(emu(1, 'a', { status: 'completed' }))).toEqual({ pct: 100, state: 'done' });
    expect(segFill(emu(1, 'a', { status: 'failed' }))).toEqual({ pct: 100, state: 'fail' });
    expect(segFill(emu(1, 'a', { status: 'cancelled' }))).toEqual({ pct: 100, state: 'fail' });
    expect(segFill(emu(1, 'a', { status: 'queued' }))).toEqual({ pct: 0, state: 'wait' });
  });
});

describe('setup card title, step and countdown (items 154–156)', () => {
  it('reads "Setting up N installs · K done", then "N installs done"', () => {
    const a = setupCardView(run(), [emu(1, 'PCSX2/pcsx2', { status: 'completed' })]);
    expect(a.title).toBe('Setting up 2 installs · 1 done');
    const one = setupCardView(run({ queued: ['PCSX2/pcsx2'] }), []);
    expect(one.title).toBe('Setting up 1 install · 0 done');
    const done = setupCardView(run(), [emu(1, 'PCSX2/pcsx2', { status: 'completed' }), emu(2, 'inolen/redream', { status: 'completed' })]);
    expect(done.title).toBe('2 installs done');
  });

  it('names the step: "<Emulator> · downloading 62%", then "<Emulator> · installing…"', () => {
    expect(stepLine('DuckStation', emu(1, 'a', { status: 'downloading', downloaded_bytes: 62, total_bytes: 100 }))).toBe('DuckStation · downloading 62%');
    expect(stepLine('DuckStation', emu(1, 'a', { status: 'installing' }))).toBe('DuckStation · installing…');
    const v = setupCardView(run(), [emu(1, 'PCSX2/pcsx2', { status: 'completed' }), emu(2, 'inolen/redream', { status: 'downloading', downloaded_bytes: 7, total_bytes: 10 })]);
    expect(v.step).toBe('Redream · downloading 70%');
  });

  it('has no countdown while anything still installs or waits', () => {
    expect(setupCardView(run(), []).done).toBe(false);
    expect(setupCardView(run(), [emu(1, 'PCSX2/pcsx2', { status: 'completed' })]).done).toBe(false);
  });

  it('once the last install is done, says how many systems are ready and counts down 10 s', () => {
    const entries = [emu(1, 'PCSX2/pcsx2', { status: 'completed' }), emu(2, 'inolen/redream', { status: 'completed' })];
    const v = setupCardView(run({ outcomes: [assigned('PlayStation 2'), assigned('Dreamcast'), assigned('PSP')] }), entries);
    expect(v.done).toBe(true);
    expect(v.step).toBe('3 systems are ready to play.');
    expect(setupCardView(run({ outcomes: [assigned('PlayStation 2')] }), entries).step).toBe('1 system is ready to play.');
    expect(SETUP_CARD_DONE_MS).toBe(10_000);
  });

  it('counts a failed install as finished, red, with the desktop words', () => {
    const v = setupCardView(run(), [emu(1, 'PCSX2/pcsx2', { status: 'completed' }), emu(2, 'inolen/redream', { status: 'failed' })]);
    expect(v.done).toBe(true);
    expect(v.segs[1].state).toBe('fail');
    expect(v.title).toBe('1 of 2 installs done');
    expect(v.sub).toBe("Redream didn't install.");
  });
});
