// The external stores GRID knows, their marks and their tools (GameStores
// README, designer decision 3: neutral monogram tiles in a store colour,
// never a store's logo). Pure and UI-agnostic: the desktop and TV mode both
// read it. Phase 3 shows GOG, Epic Games and Amazon Games: Steam has no rail
// group, card or row until its phase (docs/stores-phase3-plan.md).
//
// What a store can do is a capability here (`storeCaps`), never a
// `source === 'gog'` check in a view: GOG, Epic and Amazon all have a tool,
// but only GOG and Epic have cloud saves and DLC, only GOG has offline
// installers, drive backups, a Linux build to pick and install languages, and
// Amazon's versions are opaque ids GRID never shows.
import type { StoreGame, StoreSource } from '../api';

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

/** The stores this build shows. Phase 3: GOG, Epic Games and Amazon Games (Steam is hidden, not "coming soon"). */
export const ENABLED_STORES: readonly StoreSource[] = ['amazon', 'epic', 'gog'];

export function isEnabledStore(source: string): source is StoreSource {
  return (ENABLED_STORES as readonly string[]).includes(source);
}

/** The store a game key names (`amazon:amzn1.adg.product.…`: only the first `:` splits), else GOG. */
export function storeOfKey(key: string): StoreSource {
  const head = key.includes(':') ? key.slice(0, key.indexOf(':')) : key;
  return (STORE_ORDER as readonly string[]).includes(head) ? (head as StoreSource) : 'gog';
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

/** What a store can do in GRID. */
export type StoreCaps = {
  /** GRID installs its games with a command-line tool it downloads (gogdl, legendary, nile). */
  hasTool: boolean;
  /** The store keeps cloud saves GRID syncs (GOG and Epic). */
  hasCloud: boolean;
  /** Its installed games can go on a backup drive (GOG only; `noBackupReason` says why not). */
  canBackup: boolean;
  /** It has offline installers: Install from installer… (GOG only). */
  hasInstaller: boolean;
  /** A game can have a Linux and a Windows build: the install dialog's Version (GOG only). */
  hasVersions: boolean;
  /** A game can install in one of several languages: the install dialog's Language (GOG only). */
  hasLanguages: boolean;
  /** A game can have DLC GRID installs and removes: "DLC you own" (GOG and Epic; nile has no DLC model). */
  hasDlc: boolean;
  /**
   * Its versions are numbers a person reads ("2.2"): Update to 2.2, the v2.1
   * chip and Overview's Version. Amazon's are opaque version ids, so its
   * button reads "Update" and no version shows (user decision 2026-11-13).
   */
  readableVersions: boolean;
  /**
   * The store's own client installs, updates and starts its games, and owns
   * pause and cancel (Steam). No caller reads it until the Steam UI lands.
   */
  managedByClient: boolean;
};

const CAPS: Record<StoreSource, StoreCaps> = {
  gog: { hasTool: true, hasCloud: true, canBackup: true, hasInstaller: true, hasVersions: true, hasLanguages: true, hasDlc: true, readableVersions: true, managedByClient: false },
  epic: { hasTool: true, hasCloud: true, canBackup: false, hasInstaller: false, hasVersions: false, hasLanguages: false, hasDlc: true, readableVersions: true, managedByClient: false },
  amazon: { hasTool: true, hasCloud: false, canBackup: false, hasInstaller: false, hasVersions: false, hasLanguages: false, hasDlc: false, readableVersions: false, managedByClient: false },
  steam: { hasTool: false, hasCloud: false, canBackup: false, hasInstaller: false, hasVersions: false, hasLanguages: false, hasDlc: false, readableVersions: false, managedByClient: true },
};

export function storeCaps(source: StoreSource): StoreCaps {
  return CAPS[source];
}

export function hasTool(source: StoreSource): boolean {
  return CAPS[source].hasTool;
}

export function hasCloud(source: StoreSource): boolean {
  return CAPS[source].hasCloud;
}

export function canBackup(source: StoreSource): boolean {
  return CAPS[source].canBackup;
}

export function hasInstaller(source: StoreSource): boolean {
  return CAPS[source].hasInstaller;
}

export function hasDlc(source: StoreSource): boolean {
  return CAPS[source].hasDlc;
}

export function readableVersions(source: StoreSource): boolean {
  return CAPS[source].readableVersions;
}

/** The installed version a person can read ("2.1"), or `''` (none, or an opaque id: Amazon). */
export function shownVersion(game: Pick<StoreGame, 'source' | 'version'>): string {
  return readableVersions(game.source) ? game.version.trim() : '';
}

/** The update button: "Update to 2.2", or "Update" for a store whose versions are opaque (Amazon). */
export function updateLabel(game: Pick<StoreGame, 'source' | 'update_version'>): string {
  const v = game.update_version.trim();
  return readableVersions(game.source) && v !== '' ? `Update to ${v}` : 'Update';
}

/** Whether the store keeps its own cloud saves GRID syncs (GOG and Epic). Same as `hasCloud`. */
export function hasStoreCloud(source: StoreSource): boolean {
  return hasCloud(source);
}

/**
 * Whether GRID installed the game with its store's tool (`origin` is
 * `gogdl` / `legendary` / `nile`), so the tool can verify and repair it. An offline
 * install or a drive copy can't be verified.
 */
export function installedByTool(game: Pick<StoreGame, 'source' | 'origin'>): boolean {
  const tool = storeTool(game.source);
  return tool !== null && game.origin === tool.name;
}

/** The store's own launcher, named where GRID sends a game it can't install ("the Epic Games Launcher"). */
export function storeLauncher(source: StoreSource): string {
  switch (source) {
    case 'epic':
      return 'Epic Games Launcher';
    default:
      return `${storeName(source)} app`;
  }
}

/**
 * The store a Downloads row's `platform` names ("GOG", "Epic Games"), or
 * `null` for a platform that is no store this build shows.
 */
export function storeOfPlatform(platform: string): StoreSource | null {
  const p = platform.trim().toLowerCase();
  if (p === '') return null;
  return STORE_ORDER.find((s) => s === p || STORE_MARKS[s].name.toLowerCase() === p) ?? null;
}

function listWords(words: readonly string[]): string {
  if (words.length <= 1) return words.join('');
  return `${words.slice(0, -1).join(', ')} and ${words[words.length - 1]}`;
}

/** The tool stores as the lede names them, in its order (the prototype: "GOG, Epic and Amazon"). */
const LEDE_NAMES: readonly [StoreSource, string][] = [
  ['gog', 'GOG'],
  ['epic', 'Epic'],
  ['amazon', 'Amazon'],
];

/**
 * Settings › Stores' lede, naming the stores this build shows that install
 * through a tool (the prototype's line names GOG, Epic and Amazon; a store
 * that isn't shown yet isn't named).
 */
export function storesLede(enabled: readonly StoreSource[] = ENABLED_STORES): string {
  const tools = LEDE_NAMES.filter(([s]) => enabled.includes(s)).map(([, n]) => n);
  const start = 'Connect the stores you buy games from. Each connected store gets a group in the Library with its Installed and Available games.';
  const how =
    tools.length === 1
      ? ` GRID installs ${tools[0]} games with the open-source tool Heroic Games Launcher uses; its login stays in that tool’s files in GRID’s data folder.`
      : tools.length > 1
        ? ` GRID installs ${listWords(tools)} games with the open-source tools Heroic Games Launcher uses; their logins stay in those tools’ files in GRID’s data folder.`
        : '';
  const steam = enabled.includes('steam') ? ' Steam games install and start through the Steam client.' : '';
  return start + how + steam;
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
