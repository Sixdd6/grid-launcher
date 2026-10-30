import { describe, expect, it } from 'vitest';
import type { RomFile } from '../api';
import { contentFact, descriptionParagraphs, sizeFact, versionFact } from './overview';

describe('descriptionParagraphs', () => {
  it('splits at line breaks and drops blank lines', () => {
    expect(descriptionParagraphs('First part.\n\nSecond part.\r\nThird.\n  \n')).toEqual([
      'First part.',
      'Second part.',
      'Third.',
    ]);
  });

  it('keeps a description with no breaks whole', () => {
    expect(descriptionParagraphs('One block. Two sentences.')).toEqual(['One block. Two sentences.']);
  });

  it('is empty for an empty or blank description', () => {
    expect(descriptionParagraphs('')).toEqual([]);
    expect(descriptionParagraphs(' \n ')).toEqual([]);
  });
});

function file(id: number, bytes: number, category = ''): RomFile {
  return { id, file_name: `f${id}`, file_size_bytes: bytes, is_top_level: id === 1, last_modified: '', category };
}

describe('sizeFact', () => {
  it('sums the game files, leaving out updates and DLC', () => {
    expect(sizeFact([file(1, 1024 * 1024), file(2, 1024 * 1024), file(3, 5 * 1024 * 1024, 'update')])).toBe('2.0 MB');
  });

  it('is empty when the server states no size', () => {
    expect(sizeFact([file(1, 0)])).toBe('');
    expect(sizeFact([])).toBe('');
  });
});

describe('versionFact', () => {
  it('shows the installed version, and the server one only when it differs', () => {
    expect(versionFact(true, 'v1.0.0', 'v1.0.0')).toBe('v1.0.0');
    expect(versionFact(true, 'v1.0.0', 'v1.1.0')).toBe('v1.0.0 · server v1.1.0');
    expect(versionFact(true, 'v1.0.0', '')).toBe('v1.0.0');
    expect(versionFact(true, '', 'v1.1.0')).toBe('unknown · server v1.1.0');
  });

  it('shows the server version for a game that is not installed', () => {
    expect(versionFact(false, '', 'v1.1.0')).toBe('v1.1.0');
    expect(versionFact(false, '', '')).toBe('');
  });
});

describe('contentFact', () => {
  it('counts updates and DLC', () => {
    expect(contentFact([file(1, 1), file(2, 1, 'update'), file(3, 1, 'DLC'), file(4, 1, 'dlc')])).toBe('1 update · 2 DLC');
    expect(contentFact([file(2, 1, 'update'), file(3, 1, 'update')])).toBe('2 updates');
  });

  it('is empty with no extra content', () => {
    expect(contentFact([file(1, 1)])).toBe('');
  });
});
