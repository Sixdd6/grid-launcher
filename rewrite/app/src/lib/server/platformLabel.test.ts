// @vitest-environment node
import { describe, expect, it } from 'vitest';
import { platformLabel } from './platformLabel';

describe('platformLabel', () => {
  it('prefers display_name over name', () => {
    expect(
      platformLabel({ display_name: 'Windows 9x', name: 'Windows', slug: 'win' })
    ).toBe('Windows 9x');
  });

  it('falls back to name when display_name is empty (older server)', () => {
    expect(
      platformLabel({ display_name: '', name: 'Nintendo Switch', slug: 'switch' })
    ).toBe('Nintendo Switch');
  });

  it('falls back to slug when both display_name and name are empty', () => {
    expect(platformLabel({ display_name: '', name: '', slug: 'unknown-slug' })).toBe(
      'unknown-slug'
    );
  });
});
