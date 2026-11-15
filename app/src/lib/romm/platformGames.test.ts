import { describe, expect, it } from 'vitest';
import { fromLiveServer, installedOnPlatform, localTagLabel, platformGameCounts, platformGames } from './platformGames';
import { game, installed, platform } from './fixtures.test-util';

const LIVE = 'https://romm.home.lan';

describe('platformGames', () => {
  const serverGames = [game({ id: 1, name: 'A' }), game({ id: 2, name: 'B' }), game({ id: 3, name: 'C' })];

  it('matches installed rows to server games by rom id (Play / Install)', () => {
    const rowB = installed({ title: 'B', rom_id: 2 });
    const items = platformGames(serverGames, [rowB], LIVE);
    expect(items.map((i) => i.kind)).toEqual(['server', 'server', 'server']);
    expect(items[1]).toEqual({ kind: 'server', game: serverGames[1], installed: rowB });
    expect(items[0]).toMatchObject({ installed: null });
  });

  it('adds an unlinked row as NOT LINKED', () => {
    const unlinked = installed({ title: 'Hidden Kiln', rom_id: null });
    const items = platformGames(serverGames, [unlinked], LIVE);
    expect(items[3]).toEqual({ kind: 'local', row: unlinked, tag: 'not_linked' });
    expect(localTagLabel('not_linked')).toBe('NOT LINKED');
  });

  it('adds a row from another server, or gone from this one, as NOT ON ROMM', () => {
    // Same rom id as a server game, but installed from another server.
    const other = installed({ title: 'Sky Lantern', rom_id: 1, source_server: 'https://romm.old.lan' });
    const gone = installed({ title: 'Gone', rom_id: 99 });
    const items = platformGames(serverGames, [other, gone], LIVE);
    expect(items[0]).toMatchObject({ kind: 'server', installed: null });
    expect(items.slice(3)).toEqual([
      { kind: 'local', row: other, tag: 'not_on_romm' },
      { kind: 'local', row: gone, tag: 'not_on_romm' },
    ]);
    expect(localTagLabel('not_on_romm')).toBe('NOT ON ROMM');
  });

  it('treats a source_server equal to the live server as this server', () => {
    const row = installed({ rom_id: 3, source_server: 'https://ROMM.home.lan/' });
    expect(fromLiveServer(row, LIVE)).toBe(true);
    expect(platformGames(serverGames, [row], LIVE)[2]).toMatchObject({ installed: row });
  });

  it('counts games on RomM, installed and only here', () => {
    const items = platformGames(
      serverGames,
      [installed({ rom_id: 2 }), installed({ title: 'X', rom_id: null }), installed({ title: 'Y', rom_id: 77 })],
      LIVE,
    );
    expect(platformGameCounts(items)).toEqual({ onRomm: 3, installed: 1, onlyHere: 2 });
  });

  it('stays linear on a large platform', () => {
    const many = Array.from({ length: 5000 }, (_, i) => game({ id: i + 1 }));
    const rows = Array.from({ length: 2500 }, (_, i) => installed({ title: `G${i}`, rom_id: i * 2 + 1 }));
    const started = performance.now();
    const items = platformGames(many, rows, LIVE);
    expect(performance.now() - started).toBeLessThan(500);
    expect(platformGameCounts(items)).toEqual({ onRomm: 5000, installed: 2500, onlyHere: 0 });
  });
});

describe('installedOnPlatform', () => {
  it('keeps the RomM rows of one platform by the key rule', () => {
    const servers = [platform({ slug: 'snes', display_name: 'Super Famicom' })];
    const rows = [
      installed({ title: 'A', platform_slug: 'snes' }),
      installed({ title: 'B', platform: 'Super Famicom', platform_slug: '' }),
      installed({ title: 'C', platform: 'Nintendo 64', platform_slug: 'n64' }),
      installed({ title: 'D', platform_slug: 'snes', source: 'gog' }),
    ];
    expect(installedOnPlatform(rows, 'snes', servers).map((r) => r.title)).toEqual(['A', 'B']);
  });
});
