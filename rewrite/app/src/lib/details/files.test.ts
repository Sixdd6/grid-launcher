import { describe, expect, it } from 'vitest';
import { contentRows, fileRows, fileSizeText, gameRows } from './files';
import type { RomFile } from '../api';

function file(overrides: Partial<RomFile> & { id: number; file_name: string }): RomFile {
  return {
    file_size_bytes: 0,
    is_top_level: true,
    category: '',
    last_modified: '',
    ...overrides,
  };
}

describe('fileSizeText', () => {
  it('reports plain bytes below a kibibyte', () => {
    expect(fileSizeText(512)).toBe('512 B');
  });

  it('reports one decimal from a kibibyte up', () => {
    expect(fileSizeText(1536)).toBe('1.5 KB');
    expect(fileSizeText(5 * 1024 * 1024)).toBe('5.0 MB');
    expect(fileSizeText(3 * 1024 * 1024 * 1024)).toBe('3.0 GB');
  });

  it('reports an unknown size as an em dash rather than "0 B"', () => {
    expect(fileSizeText(0)).toBe('—');
  });

  it('never reports a negative size', () => {
    expect(fileSizeText(-1)).toBe('—');
  });
});

describe('fileRows', () => {
  it('carries the name, size and D-UI-10 version of every file', () => {
    expect(
      fileRows([
        file({ id: 1, file_name: 'mygame (v1.1.0).zip', file_size_bytes: 2048 }),
        file({ id: 2, file_name: 'game.json', last_modified: '2026-02-03T11:22:33' }),
      ])
    ).toEqual([
      { id: 1, name: 'mygame (v1.1.0).zip', sizeText: '2.0 KB', version: 'v1.1.0', category: '' },
      { id: 2, name: 'game.json', sizeText: '—', version: '2026-02-03', category: '' },
    ]);
  });

  it('is empty for a rom with no listed files', () => {
    expect(fileRows([])).toEqual([]);
  });
});

describe('contentRows', () => {
  it('picks out the update and dlc category files', () => {
    const rows = contentRows([
      file({ id: 1, file_name: 'ps4-base.zip', category: 'game' }),
      file({ id: 2, file_name: 'ps4-update.zip', category: 'update' }),
      file({ id: 3, file_name: 'ps4-dlc.zip', category: 'dlc' }),
    ]);
    expect(rows.map((r) => r.name)).toEqual(['ps4-update.zip', 'ps4-dlc.zip']);
    expect(rows.map((r) => r.category)).toEqual(['update', 'dlc']);
  });

  it('folds the category case, which the server does not guarantee', () => {
    const rows = contentRows([file({ id: 1, file_name: 'u.zip', category: 'UPDATE' })]);
    expect(rows).toHaveLength(1);
  });

  it('is empty when every file is an ordinary game file', () => {
    expect(contentRows([file({ id: 1, file_name: 'g.zip', category: 'game' })])).toEqual([]);
  });
});

describe('gameRows', () => {
  it('leaves the update and dlc files to contentRows, so no file is listed twice', () => {
    const files = [
      file({ id: 1, file_name: 'ps4-base.zip', category: 'game' }),
      file({ id: 2, file_name: 'ps4-update.zip', category: 'update' }),
      file({ id: 3, file_name: 'ps4-dlc.zip', category: 'dlc' }),
    ];
    expect(gameRows(files).map((r) => r.name)).toEqual(['ps4-base.zip']);
    // Together the two lists still account for every file the server sent.
    expect(gameRows(files).length + contentRows(files).length).toBe(files.length);
  });

  it('folds the category case, which the server does not guarantee', () => {
    expect(gameRows([file({ id: 1, file_name: 'u.zip', category: 'DLC' })])).toEqual([]);
  });

  it('keeps every ordinary file, including the blank category', () => {
    const rows = gameRows([
      file({ id: 1, file_name: 'a.zip', category: '' }),
      file({ id: 2, file_name: 'b.zip', category: 'game' }),
    ]);
    expect(rows.map((r) => r.name)).toEqual(['a.zip', 'b.zip']);
  });
});
