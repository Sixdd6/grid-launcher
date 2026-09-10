import { describe, expect, it } from 'vitest';
import type { GameSession, SessionsSnapshot } from '../api';
import { applySnapshot, findSession } from './sessions.svelte';

function session(overrides: Partial<GameSession>): GameSession {
  return {
    id: 1,
    rom_id: 42,
    title: 'Chrono Trigger',
    emulator_name: 'Snes9x',
    started_at: 1000,
    pid: 1234,
    ...overrides,
  };
}

function snapshot(overrides: Partial<SessionsSnapshot>): SessionsSnapshot {
  return { sessions: [], warning: null, ...overrides };
}

describe('applySnapshot', () => {
  it('replaces the session list wholesale rather than merging', () => {
    const current = { sessions: [session({ id: 1, rom_id: 1 })], lastWarning: null };
    const next = snapshot({ sessions: [session({ id: 2, rom_id: 2 })] });
    const result = applySnapshot(current, next);
    expect(result.sessions).toEqual([session({ id: 2, rom_id: 2 })]);
  });

  it('replaces with an empty list when the snapshot has no sessions', () => {
    const current = { sessions: [session({ id: 1, rom_id: 1 })], lastWarning: null };
    const result = applySnapshot(current, snapshot({ sessions: [] }));
    expect(result.sessions).toEqual([]);
  });

  it('captures a warning from the snapshot', () => {
    const current = { sessions: [], lastWarning: null };
    const result = applySnapshot(current, snapshot({ warning: 'Game exited immediately' }));
    expect(result.lastWarning).toBe('Game exited immediately');
  });

  it('does not clear a previously captured warning when the next snapshot has no warning', () => {
    const current = { sessions: [], lastWarning: 'Game exited immediately' };
    const result = applySnapshot(current, snapshot({ warning: null }));
    expect(result.lastWarning).toBe('Game exited immediately');
  });

  it('keeps the latest warning across back-to-back warning snapshots', () => {
    const current = { sessions: [], lastWarning: 'First warning' };
    const result = applySnapshot(current, snapshot({ warning: 'Second warning' }));
    expect(result.lastWarning).toBe('Second warning');
  });
});

describe('findSession', () => {
  it('returns the session matching the given rom_id', () => {
    const sessions = [session({ id: 1, rom_id: 42 }), session({ id: 2, rom_id: 99 })];
    expect(findSession(sessions, 42)).toEqual(session({ id: 1, rom_id: 42 }));
  });

  it('returns undefined when no session matches the given rom_id', () => {
    const sessions = [session({ id: 1, rom_id: 42 })];
    expect(findSession(sessions, 7)).toBeUndefined();
  });
});
