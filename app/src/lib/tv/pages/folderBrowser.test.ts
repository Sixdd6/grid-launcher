import { describe, expect, it } from 'vitest';
import { browserHints, browserRows, childPath, rowAction } from './folderBrowser';

const listing = (path: string, parent: string | null) => ({
  path,
  parent,
  entries: [
    { name: 'Games', is_dir: true },
    { name: 'bsnes.AppImage', is_dir: false },
  ],
});

describe('the folder browser (items 349–352)', () => {
  it('Up one folder comes first, and is absent at the root', () => {
    expect(browserRows(listing('/home/sam', '/home'), false).map((r) => r.kind)).toEqual(['up', 'dir']);
    expect(browserRows(listing('/', null), false).map((r) => r.kind)).toEqual(['dir']);
  });

  it('file mode lists files too, and has no Y', () => {
    const rows = browserRows(listing('/home/sam', '/home'), true);
    expect(rows.map((r) => [r.kind, r.path])).toEqual([
      ['up', '/home'],
      ['dir', '/home/sam/Games'],
      ['file', '/home/sam/bsnes.AppImage'],
    ]);
    expect(rows.map(rowAction)).toEqual(['Up', 'Open', 'Choose']);
    expect(browserHints(true)).toEqual([]);
    expect(browserHints(false)).toEqual([{ button: 'Y', label: 'Use this folder' }]);
  });

  it('joins paths with the folder own separator', () => {
    expect(childPath('/', 'home')).toBe('/home');
    expect(childPath('C:\\Games', 'GRID')).toBe('C:\\Games\\GRID');
  });
});
