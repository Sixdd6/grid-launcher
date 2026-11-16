import { describe, expect, it, vi } from 'vitest';

vi.mock('../stores/toasts.svelte', () => ({ pushToast: () => 1 }));

import { nav } from './nav.svelte';
import { stepTab, TAB_ORDER } from './input/press';

describe('TV navigation (TVRommStorefront README § The shell)', () => {
  it('opens on the Library: three tabs, no Server', () => {
    expect(nav.tab).toBe('library');
    expect(TAB_ORDER).toEqual(['library', 'downloads', 'settings']);
    expect(stepTab('settings', 1)).toBe('library');
    expect(stepTab('library', -1)).toBe('settings');
  });
});
