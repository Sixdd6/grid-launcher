import { describe, expect, it } from 'vitest';
import { inStorefrontOrder, STOREFRONT_ORDER, storefrontIndex } from './order';

describe('STOREFRONT_ORDER', () => {
  it('is alphabetical by name, RomM between GOG and Steam (decision 10)', () => {
    expect(STOREFRONT_ORDER).toEqual(['amazon', 'epic', 'gog', 'romm', 'steam']);
  });
  it('sorts items by storefront, unknown ones last, stable otherwise', () => {
    const items = ['steam', 'romm', 'x', 'amazon', 'gog', 'epic'].map((source, n) => ({ source, n }));
    expect(inStorefrontOrder(items, (i) => i.source).map((i) => i.source)).toEqual(['amazon', 'epic', 'gog', 'romm', 'steam', 'x']);
    expect(storefrontIndex('nope')).toBe(STOREFRONT_ORDER.length);
  });
});
