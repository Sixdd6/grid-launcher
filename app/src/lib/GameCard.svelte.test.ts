// @vitest-environment node
import { describe, expect, it } from 'vitest';
import { render } from 'svelte/server';

import GameCard from './GameCard.svelte';

function card(missing: boolean | undefined) {
  return render(GameCard, {
    props: {
      testId: 'library-card-7',
      index: 3,
      badgeId: 7,
      title: 'Mario',
      platform: 'SNES',
      cover: { small: null, large: null },
      installed: true,
      ...(missing === undefined ? {} : { missing }),
      updateLabel: null,
      cloudPlatforms: new Set<string>(),
      labels: 'always' as const,
      rect: { x: 10, y: 20, w: 160, h: 213 },
      focused: false,
      onOpen: () => {},
      onPrimary: () => {},
      onCloud: () => {},
      onHoverStart: () => {},
      onHoverEnd: () => {},
    },
  }).body;
}

describe('GameCard placement', () => {
  it('is placed and sized by its rectangle, label row included', () => {
    const body = card(false);
    expect(body).toContain('--x: 10px; --y: 20px; --w: 160px; --cover-h: 213px; --card-h: 245px');
  });
});

describe('GameCard index', () => {
  it('renders its position in the whole list as data-index, for the warmer', () => {
    expect(card(false)).toContain('data-index="3"');
  });
});

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

describe('GameCard select mode (GameBackups)', () => {
  function selectCard(selected: boolean) {
    return render(GameCard, {
      props: {
        testId: 'game-card-7',
        index: 0,
        badgeId: 7,
        title: 'Mario',
        platform: 'SNES',
        cover: { small: null, large: null },
        installed: false,
        updateLabel: null,
        cloudPlatforms: new Set<string>(),
        labels: 'always' as const,
        rect: { x: 0, y: 0, w: 160, h: 213 },
        focused: false,
        selecting: true,
        selected,
        onSelect: () => {},
        onOpen: () => {},
        onPrimary: () => {},
        onCloud: () => {},
        onHoverStart: () => {},
        onHoverEnd: () => {},
      },
    }).body;
  }

  it('marks a picked card selected, with a filled check and no hover bar', () => {
    const body = selectCard(true);
    expect(body).toMatch(/class="card[^"]*\bselecting\b[^"]*\bselected\b/);
    expect(body).toContain('aria-selected="true"');
    expect(body).toContain('data-testid="card-check-7"');
    expect(body).toContain('<svg');
    expect(body).not.toContain('card-primary-7');
  });

  it('shows an empty check on an unpicked card', () => {
    const body = selectCard(false);
    expect(body).toContain('aria-selected="false"');
    expect(body).not.toMatch(/class="card[^"]*\bselected\b/);
    expect(body).toContain('data-testid="card-check-7"');
  });

  it('keeps the hover bar and no check outside select mode', () => {
    const body = card(false);
    expect(body).not.toContain('card-check-7');
    expect(body).toContain('card-primary-7');
  });
});
