// The external stores GRID knows, their marks and their tools (GameStores
// README, designer decision 3: neutral monogram tiles in a store colour,
// never a store's logo). Pure and UI-agnostic: the desktop and TV mode both
// read it. Phase 1 shows GOG only: Epic, Amazon and Steam have no rail group,
// card or row until their phase (docs/stores-phase1-plan.md).
import type { StoreSource } from '../api';

export type StoreMark = {
  source: StoreSource;
  /** The store's name as the UI writes it ("GOG", "Epic Games"). */
  name: string;
  /** The monogram on the tile. */
  mono: string;
  /** The tile's colour (the prototype's `--st-hue`). */
  hue: string;
};

export const STORE_MARKS: Record<StoreSource, StoreMark> = {
  amazon: { source: 'amazon', name: 'Amazon Games', mono: 'A', hue: 'hsl(32 80% 42%)' },
  epic: { source: 'epic', name: 'Epic Games', mono: 'E', hue: 'hsl(230 10% 26%)' },
  gog: { source: 'gog', name: 'GOG', mono: 'G', hue: 'hsl(278 45% 42%)' },
  steam: { source: 'steam', name: 'Steam', mono: 'S', hue: 'hsl(210 45% 30%)' },
};

/** Every store, alphabetical by name (designer decision 2: the rail's order). */
export const STORE_ORDER: readonly StoreSource[] = ['amazon', 'epic', 'gog', 'steam'];

/** The stores this build shows. Phase 1: GOG only (the others are hidden, not "coming soon"). */
export const ENABLED_STORES: readonly StoreSource[] = ['gog'];

export function isEnabledStore(source: string): source is StoreSource {
  return (ENABLED_STORES as readonly string[]).includes(source);
}

export function storeMark(source: StoreSource): StoreMark {
  return STORE_MARKS[source];
}

export function storeName(source: StoreSource): string {
  return STORE_MARKS[source].name;
}

/** The command-line tool a store installs with, as the UI names it. */
export type StoreToolInfo = { name: string; pkg: string };

const TOOLS: Partial<Record<StoreSource, StoreToolInfo>> = {
  gog: { name: 'gogdl', pkg: 'heroic-gogdl' },
  epic: { name: 'legendary', pkg: 'legendary' },
  amazon: { name: 'nile', pkg: 'nile' },
};

/** The store's tool, or `null` for Steam (it installs through the Steam client). */
export function storeTool(source: StoreSource): StoreToolInfo | null {
  return TOOLS[source] ?? null;
}

/** Whether the store keeps its own cloud saves GRID syncs (GOG and Epic). */
export function hasStoreCloud(source: StoreSource): boolean {
  return source === 'gog' || source === 'epic';
}

/** Why a store's games can't go on a backup drive; `''` for GOG, which can. */
export function noBackupReason(source: StoreSource): string {
  switch (source) {
    case 'epic':
      return 'Epic Games games can’t be backed up: legendary installs only from Epic’s servers.';
    case 'amazon':
      return 'Amazon Games games can’t be backed up: nile installs only from Amazon’s servers.';
    case 'steam':
      return 'Steam games can’t be backed up here. Steam has its own Backup and restore.';
    default:
      return '';
  }
}
