import { describe, expect, it } from 'vitest';
import { descriptionParagraphs } from './overview';

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
