import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { GameSummary } from '../api';

function game(id: number, name: string): GameSummary {
  return { id, name } as GameSummary;
}

describe('LinkList', () => {
  let listServerRomsForPlatform: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    vi.resetModules();
    listServerRomsForPlatform = vi.fn(async () => [game(1, 'Alpha'), game(2, 'Beta')]);
    vi.doMock('../api', () => ({ api: { listServerRomsForPlatform } }));
  });

  afterEach(() => vi.doUnmock('../api'));

  async function list() {
    const { LinkList } = await import('./linkList.svelte');
    return new LinkList();
  }

  it('loads the platform list and leaves out held roms', async () => {
    const l = await list();
    expect(l.shown('', new Set())).toEqual([]);
    l.load('SNES');
    await vi.waitFor(() => expect(l.games).not.toBeNull());
    expect(listServerRomsForPlatform).toHaveBeenCalledWith('SNES');
    expect(l.shown('', new Set([1])).map((g) => g.id)).toEqual([2]);
  });

  it('keeps a load error, and drops an answer after the stop', async () => {
    listServerRomsForPlatform.mockRejectedValueOnce(new Error('offline'));
    const l = await list();
    l.load('SNES');
    await vi.waitFor(() => expect(l.loadError).toBe('offline'));
    const late = await list();
    late.load('SNES')();
    await Promise.resolve();
    await Promise.resolve();
    expect(late.games).toBeNull();
  });

  it('picks one at a time and keeps a refusal', async () => {
    const l = await list();
    let release!: () => void;
    const link = vi.fn(() => new Promise<void>((res) => (release = res)));
    const first = l.pick(game(1, 'Alpha'), link);
    expect(l.pickingId).toBe(1);
    await l.pick(game(2, 'Beta'), link);
    expect(link).toHaveBeenCalledTimes(1);
    release();
    await first;
    expect(l.pickingId).toBeNull();
    await l.pick(game(2, 'Beta'), async () => {
      throw new Error('taken');
    });
    expect(l.pickError).toBe('taken');
  });
});
