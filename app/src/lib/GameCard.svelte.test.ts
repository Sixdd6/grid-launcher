// @vitest-environment node
import { describe, expect, it } from 'vitest';
import { render } from 'svelte/server';

import GameCard from './GameCard.svelte';

function card(missing: boolean | undefined) {
  return render(GameCard, {
    props: {
      testId: 'library-card-7',
      badgeId: 7,
      title: 'Mario',
      platform: 'SNES',
      coverUrl: null,
      installed: true,
      ...(missing === undefined ? {} : { missing }),
      updateLabel: null,
      cloudPlatforms: new Set<string>(),
      focused: false,
      onOpen: () => {},
      onPrimary: () => {},
      onCloud: () => {},
      onHoverStart: () => {},
      onHoverEnd: () => {},
    },
  }).body;
}

describe('GameCard missing badge (9c)', () => {
  it('shows the MISSING badge and a Locate… primary for a missing game', () => {
    const body = card(true);
    expect(body).toContain('data-testid="library-card-missing-7"');
    expect(body).toContain('MISSING');
    expect(body).toContain('Locate…');
  });

  it('shows no badge and Play for a present game, and by default', () => {
    for (const missing of [false, undefined]) {
      const body = card(missing);
      expect(body).not.toContain('library-card-missing-7');
      expect(body).toContain('>Play<');
    }
  });
});
