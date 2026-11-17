import type { RailKey } from './library/rail';

/** The views with RomM as a storefront (plan 0.5): Library, Downloads,
 *  Settings, in pill order. The index in this array is also the
 *  `Ctrl+<n>` accelerator. */
export const STOREFRONT_VIEWS = ['library', 'downloads', 'settings'] as const;
export type View = (typeof STOREFRONT_VIEWS)[number];

/** The storefront view a `Ctrl+<key>` accelerator selects, or `null`. */
export function storefrontViewForDigit(key: string): View | null {
  const index = STOREFRONT_VIEWS.findIndex((_, i) => String(i + 1) === key);
  return index === -1 ? null : STOREFRONT_VIEWS[index];
}

/**
 * The startup phases (plan 0.5):
 * - `welcome`: the first-run welcome, only while `setup.welcomed` is false
 *   (`welcome_status`); never because of RomM's state;
 * - `setup`: Choose your systems, after a successful RomM connect when setup
 *   is needed; `session.setupReturn` says where Skip/Done go;
 * - `shell`.
 */
export type SessionPhase = 'loading' | 'welcome' | 'setup' | 'shell';
/** Where Choose your systems returns: on to the shell (from the welcome), or
 *  back to Settings › Game stores (from the RomM card). */
export type SetupReturn = 'welcome' | 'settings';
/** Where GRID opens with RomM as a storefront: always the Library, on a rail row. */
export type InitialPlace = {
  view: 'library';
  rail: RailKey;
  /** Kept for the contract's shape; always `null` (decision 16: GRID opens
   *  on All games, never on the first Backup drive). */
  drive: string | null;
};

/**
 * Where GRID opens (plan 0.5, decision 16), the same in desktop and TV
 * mode: Library › All games, always. Discover and Settings › Appearance ›
 * Open GRID on are removed for now (2026-11-17, pending the Discover
 * rework); the stored `[ui] open_on` is still read and written, but ignored.
 */
export function initialPlace(): InitialPlace {
  return { view: 'library', rail: 'all', drive: null };
}

export function viewLabel(view: View): string {
  return view.charAt(0).toUpperCase() + view.slice(1);
}

export function hostOf(serverUrl: string): string {
  try {
    return new URL(serverUrl).host || serverUrl;
  } catch {
    return serverUrl;
  }
}
