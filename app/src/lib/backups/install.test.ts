import { beforeEach, describe, expect, it, vi } from 'vitest';

const api = vi.hoisted(() => ({ installFromDrive: vi.fn(), removeFromDrive: vi.fn() }));
const notifier = vi.hoisted(() => ({ notify: vi.fn(), quietQueued: vi.fn(), unquiet: vi.fn() }));
vi.mock('../api', () => ({ api }));
vi.mock('../notifications/notifier', () => notifier);
vi.mock('../stores/drives.svelte', () => ({ refresh: vi.fn(async () => {}) }));

import { batchQueuedNotice, installAllFromDrive, installFromDrive } from './install';

const g = (rom_id: number, name: string) => ({ rom_id, name, server: 'https://romm.lan' });

beforeEach(() => {
  vi.clearAllMocks();
});

describe('install from a drive', () => {
  it('turns the answer into cards: queued lines open Downloads, "has no emulator" opens nothing (item 428)', async () => {
    api.installFromDrive.mockResolvedValue({
      notices: ['DuckStation from Couch kit added to download queue', 'Couch kit has no emulator for Sony PlayStation 2'],
    });
    expect(await installFromDrive('/m/t7', g(1, 'A'))).toBe(true);
    expect(notifier.notify.mock.calls.map(([n]) => n)).toEqual([
      { group: 'drive-queued', text: 'DuckStation from Couch kit added to download queue', level: 'success', action: 'downloads' },
      { group: 'drive-note', text: 'Couch kit has no emulator for Sony PlayStation 2', level: 'success' },
    ]);
  });

  it('select mode: one card for the games that queued, however slow the calls (item 450)', async () => {
    api.installFromDrive.mockImplementation(async (_key: string, romId: number) => {
      await new Promise((r) => setTimeout(r, 5));
      if (romId === 3) throw new Error('No space left on the library drive.');
      return { notices: [] };
    });
    await installAllFromDrive('/m/t7', [g(1, 'A'), g(2, 'B'), g(3, 'C')], { label: 'SAMSUNG T7' });
    expect(notifier.quietQueued).toHaveBeenCalledWith([1, 2, 3]);
    expect(notifier.unquiet).toHaveBeenCalledWith(3);
    const cards = notifier.notify.mock.calls.map(([n]) => n);
    expect(cards).toHaveLength(2);
    expect(cards[0].text).toContain("C");
    expect(cards[1]).toEqual({
      group: 'game-queued',
      text: '2 games added to download queue',
      level: 'success',
      action: 'downloads',
      sub: 'Copying from SAMSUNG T7',
    });
  });

  it('the desktop keeps each game\'s own card from the queue', async () => {
    api.installFromDrive.mockResolvedValue({ notices: [] });
    await installAllFromDrive('/m/t7', [g(1, 'A'), g(2, 'B')]);
    expect(notifier.quietQueued).not.toHaveBeenCalled();
    expect(notifier.notify).not.toHaveBeenCalled();
  });

  it('one game is named', () => {
    expect(batchQueuedNotice(['A'], 'SAMSUNG T7')).toMatchObject({ text: 'A added to download queue', sub: 'Copying from SAMSUNG T7' });
  });
});
