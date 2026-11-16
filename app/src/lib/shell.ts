import type { OpenOn } from './api';
import type { RailKey } from './library/rail';
import type { RommRailKey } from './romm/rail';

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
  rail: RailKey | RommRailKey;
  /** Kept for the contract's shape; always `null` (decision 16: GRID opens
   *  on All games, never on the first Backup drive). */
  drive: string | null;
};

/**
 * Where GRID opens (plan 0.5, decision 16), the same in desktop and TV
 * mode. `rommConnected`: RomM is connected at startup — the config's
 * `stores.romm.connected`, which the status shows as connected or
 * connecting (`romm/rail.ts` `rommLive`) before the restore has answered.
 * - `automatic` and `discover`: RomM › Discover when connected, else All games
 *   (Discover does not exist while RomM is off);
 * - `library`: All games.
 * If the restore then fails, the rail's selection falls back to RomM ›
 * Installed (`rommSelectionFallback`).
 */
export function initialPlace(rommConnected: boolean, openOn: OpenOn = 'automatic'): InitialPlace {
  if (openOn !== 'library' && rommConnected) return { view: 'library', rail: 'romm:discover', drive: null };
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
