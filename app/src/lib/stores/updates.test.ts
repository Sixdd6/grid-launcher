import { describe, expect, it } from 'vitest';
import type { UpdateRow } from '../api';
import { labelFor } from './updates.svelte';

const rows: UpdateRow[] = [
  { rom_id: 1, label: 'Update to v2' },
  { rom_id: 2, label: 'Update to v3' },
];

describe('labelFor', () => {
  it('returns the label for a rom_id that has an update', () => {
    expect(labelFor(rows, 1)).toBe('Update to v2');
  });

  it('returns null for a rom_id with no update', () => {
    expect(labelFor(rows, 99)).toBeNull();
  });

  it('returns null for a null rom id', () => {
    expect(labelFor(rows, null)).toBeNull();
  });
});
