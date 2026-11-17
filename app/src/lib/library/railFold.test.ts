import { describe, expect, it } from 'vitest';
import {
  foldRailEntries,
  parseRailClosed,
  RAIL_GROUP_IDS,
  railClosedCount,
  railGroupClosed,
  railGroupName,
  railGroupOf,
  railHoldsTitle,
  withRailGroup,
} from './railFold';

describe('railGroupOf', () => {
  it('puts each rail key in its group', () => {
    expect(railGroupOf('drive:/run/media/USB')).toBe('drives');
    expect(railGroupOf('romm:installed')).toBe('romm');
    expect(railGroupOf('romm:discover')).toBe('romm');
    expect(railGroupOf('platform:snes')).toBe('romm');
    expect(railGroupOf('store:gog:installed')).toBe('gog');
    expect(railGroupOf('store:steam:available')).toBe('steam');
    expect(railGroupOf('store:amazon:installed')).toBe('amazon');
    expect(railGroupOf('store:epic:available')).toBe('epic');
  });

  it('All games, Recent, Updates and unknown stores fold nowhere', () => {
    for (const key of ['all', 'recent', 'updates', 'store:zzz:installed', '']) expect(railGroupOf(key)).toBeNull();
  });
});

describe('the remembered list', () => {
  it('reads a list of strings, dropping blanks and repeats, keeping unknown ids', () => {
    expect(parseRailClosed(['gog', ' drives ', '', 'gog', 'future'])).toEqual(['gog', 'drives', 'future']);
  });

  it('reads anything else as empty, every group open', () => {
    for (const raw of [undefined, null, 'gog', 3, {}, [1], ['gog', 2], true]) expect(parseRailClosed(raw)).toEqual([]);
  });

  it('a group not listed is open', () => {
    expect(railGroupClosed([], 'romm')).toBe(false);
    expect(railGroupClosed(['romm'], 'romm')).toBe(true);
    expect(railGroupClosed(['romm'], 'gog')).toBe(false);
  });

  it('closing and opening touch only that id, unknown ids kept', () => {
    expect(withRailGroup(['future'], 'gog', true)).toEqual(['future', 'gog']);
    expect(withRailGroup(['gog', 'future'], 'gog', true)).toEqual(['future', 'gog']);
    expect(withRailGroup(['gog', 'future', 'drives'], 'gog', false)).toEqual(['future', 'drives']);
    expect(withRailGroup([], 'gog', false)).toEqual([]);
  });

  it('knows the six groups by name, in rail order', () => {
    expect(RAIL_GROUP_IDS.map(railGroupName)).toEqual(['Backup drives', 'Amazon Games', 'Epic Games', 'GOG', 'RomM', 'Steam']);
  });
});

describe('a closed heading', () => {
  it('counts Installed for a store or RomM, and drives for Backup drives', () => {
    expect(railClosedCount('gog', 3)).toEqual({ n: 3, label: '3 installed' });
    expect(railClosedCount('romm', 0)).toEqual({ n: 0, label: '0 installed' });
    expect(railClosedCount('drives', 1)).toEqual({ n: 1, label: '1 drive' });
    expect(railClosedCount('drives', 2)).toEqual({ n: 2, label: '2 drives' });
  });

  it('has no count when Installed shows no number or no drive is connected', () => {
    expect(railClosedCount('steam', null)).toBeNull();
    expect(railClosedCount('drives', 0)).toBeNull();
  });

  it('says which view it holds', () => {
    expect(railHoldsTitle('gog', 'Installed')).toBe('Showing GOG › Installed');
    expect(railHoldsTitle('romm', 'Discover')).toBe('Showing RomM › Discover');
  });
});

describe('foldRailEntries', () => {
  it('gathers each group’s rows under it, plain rows apart', () => {
    const e = (key: string) => ({ key });
    const out = foldRailEntries([
      e('all'),
      e('recent'),
      e('updates'),
      e('drive:a'),
      e('drive:b'),
      e('store:gog:installed'),
      e('store:gog:available'),
      e('romm:installed'),
      e('romm:discover'),
      e('platform:snes'),
      e('store:steam:installed'),
    ]);
    expect(out.map((i) => (i.kind === 'row' ? i.entry.key : `${i.id}:${i.rows.length}`))).toEqual([
      'all',
      'recent',
      'updates',
      'drives:2',
      'gog:2',
      'romm:3',
      'steam:1',
    ]);
  });
});
