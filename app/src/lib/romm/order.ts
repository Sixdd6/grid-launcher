// The storefronts by name (decision 10): the Library rail's groups, the Game
// stores pane's cards and the welcome's rows, in desktop and TV mode.
// `gamestores/marks.ts` `STORE_ORDER` (stores only) is unchanged.
import type { StoreSource } from '../api';

export type Storefront = 'romm' | StoreSource;

/** Amazon, Epic, GOG, RomM, Steam. */
export const STOREFRONT_ORDER: readonly Storefront[] = ['amazon', 'epic', 'gog', 'romm', 'steam'];

/** A storefront's place in `STOREFRONT_ORDER`; an unknown one sorts last. */
export function storefrontIndex(source: string): number {
  const i = (STOREFRONT_ORDER as readonly string[]).indexOf(source);
  return i === -1 ? STOREFRONT_ORDER.length : i;
}

/** `items` in storefront order (stable for equal keys), by `sourceOf(item)`. */
export function inStorefrontOrder<T>(items: readonly T[], sourceOf: (item: T) => string): T[] {
  return items
    .map((item, i) => ({ item, i, at: storefrontIndex(sourceOf(item)) }))
    .sort((a, b) => a.at - b.at || a.i - b.i)
    .map((x) => x.item);
}
