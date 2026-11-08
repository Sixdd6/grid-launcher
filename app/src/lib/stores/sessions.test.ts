import { describe, expect, it } from 'vitest';
import type { GameSession } from '../api';
import { findSession } from './sessions.svelte';

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
