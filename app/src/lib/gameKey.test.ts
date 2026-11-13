import { describe, expect, it } from 'vitest';
import { gameKey, isStoreRow, parseGameKey } from './gameKey';

describe('gameKey', () => {
  it('keys a RomM row by its rom id', () => {
    expect(gameKey({ source: 'romm', store_id: '', rom_id: 123 })).toBe('romm:123');
  });

  it('treats a row with no source as a RomM row', () => {
    expect(gameKey({ rom_id: 7 })).toBe('romm:7');
    expect(isStoreRow({})).toBe(false);
  });

  it('has no key for an unlinked RomM row', () => {
    expect(gameKey({ source: 'romm', store_id: '', rom_id: null })).toBeNull();
  });

  it('keys a store row by its store and product id, ignoring any rom id', () => {
    expect(gameKey({ source: 'gog', store_id: '1207664643', rom_id: null })).toBe('gog:1207664643');
    expect(gameKey({ source: 'epic', store_id: ' Sugar ', rom_id: 5 })).toBe('epic:Sugar');
    expect(gameKey({ source: 'steam', store_id: '620', rom_id: null })).toBe('steam:620');
    expect(isStoreRow({ source: 'amazon' })).toBe(true);
  });

  it('has no key for a store row with a blank store id', () => {
    expect(gameKey({ source: 'gog', store_id: '  ', rom_id: null })).toBeNull();
  });
});

describe('parseGameKey', () => {
  it('round-trips both forms', () => {
    expect(parseGameKey('romm:123')).toEqual({ source: 'romm', romId: 123 });
    expect(parseGameKey('gog:1207664643')).toEqual({ source: 'gog', storeId: '1207664643' });
    expect(parseGameKey('amazon:amzn1.adg:x')).toEqual({ source: 'amazon', storeId: 'amzn1.adg:x' });
  });

  it('refuses anything else', () => {
    for (const bad of ['', 'romm', 'romm:', 'romm:abc', 'itch:5', ':5', 'gog:', '123']) {
      expect(parseGameKey(bad), bad).toBeNull();
    }
  });
});
