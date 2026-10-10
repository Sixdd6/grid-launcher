// @vitest-environment node
import { describe, expect, it, vi } from 'vitest';
import { render } from 'svelte/server';

// Server render never runs `$effect`, so the picker renders in its
// pre-fetch state; the API is stubbed only so the import resolves.
vi.mock('../api', () => ({ api: { listServerRomsForPlatform: vi.fn() } }));

import LinkPicker from './LinkPicker.svelte';
import RemoveConfirm from './RemoveConfirm.svelte';

describe('LinkPicker', () => {
  it('renders the search box and a loading line before the list arrives', () => {
    const body = render(LinkPicker, {
      props: {
        title: 'Mystery Game',
        platform: 'SNES',
        heldRomIds: new Set<number>(),
        onPick: async () => {},
        onClose: () => {},
      },
    }).body;
    expect(body).toContain('data-testid="link-picker"');
    expect(body).toContain('data-testid="link-picker-search"');
    expect(body).toContain('Loading server games…');
    expect(body).toContain('Mystery Game');
    expect(body).not.toContain('data-testid="link-picker-item"');
  });
});

describe('RemoveConfirm', () => {
  it('names the folder that stays on disk', () => {
    const body = render(RemoveConfirm, {
      props: {
        title: 'Old Demo',
        folder: '/kept/Old Demo',
        onConfirm: async () => {},
        onClose: () => {},
      },
    }).body;
    expect(body).toContain('data-testid="remove-confirm"');
    expect(body).toContain('Its files stay on disk in /kept/Old Demo.');
    expect(body).toContain('data-testid="remove-confirm-yes"');
  });
});
