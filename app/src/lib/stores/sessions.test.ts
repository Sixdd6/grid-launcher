import { describe, expect, it } from 'vitest';
import type { GameSession } from '../api';
import { findSession, findSessionByKey } from './sessions.svelte';

function session(overrides: Partial<GameSession>): GameSession {
  return {
    id: 1,
    key: 'romm:42',
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

describe('findSessionByKey', () => {
  it('finds a store session by its key, never by a missing rom id', () => {
    const store = session({ id: 3, key: 'gog:1207664643', rom_id: null });
    const sessions = [session({ id: 1 }), store];
    expect(findSessionByKey(sessions, 'gog:1207664643')).toEqual(store);
    expect(findSessionByKey(sessions, 'romm:42')?.id).toBe(1);
    expect(findSessionByKey(sessions, 'epic:x')).toBeUndefined();
  });
});
