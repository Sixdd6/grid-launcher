// A store view in the Library (GameStores README, "Header" and "States in
// place of the grid"; prototype `stRenderView`). Pure: what the header says,
// which chips and actions it has, and the panel that replaces the grid.
import type { StoreGame, StoreSource, StoreStatus } from '../api';
import { availBlock, type StorePart } from './rail';
import { hasStoreCloud, storeName, storeTool } from './marks';
import { agoText, plural, sizeGb } from './text';

/** The view's title parts: "GOG" and "· Installed". */
export function storeViewTitle(source: StoreSource, part: StorePart): { title: string; sub: string } {
  return { title: storeName(source), sub: part === 'installed' ? 'Installed' : 'Available' };
}

/**
 * The count line: "3 games · 67.0 GB" (Installed), "5 games you own and
 * haven’t installed · updated 5 min ago" (Available), or the plain line while
 * Available can't list games ("… · loading" while it loads).
 */
export function storeCountLine(
  status: StoreStatus,
  part: StorePart,
  games: readonly StoreGame[],
  nowSeconds: number,
): string {
  if (part === 'installed') {
    const bytes = games.reduce((t, g) => t + g.disk_bytes, 0);
    return `${plural(games.length, 'game')} · ${sizeGb(bytes)}`;
  }
  const block = availBlock(status);
  if (block === 'loading') return 'Games you own and haven’t installed · loading';
  if (block !== '') return 'Games you own and haven’t installed';
  return `${plural(games.length, 'game')} you own and haven’t installed · updated ${agoText(status.refreshed_at, nowSeconds)}`;
}

export type ChipAct = 'settings' | 'sign_in' | null;
export type StoreChip = { text: string; act: ChipAct; warn: boolean; testId: string };

/**
 * The header chips, in the prototype's order: the account (opens Settings ›
 * Stores), Login expired · Sign in again, Not connected · Connect…, Cloud
 * saves: on, and "gogdl not installed".
 */
export function storeChips(status: StoreStatus): StoreChip[] {
  const out: StoreChip[] = [];
  if (status.connection === 'connected') {
    out.push({ text: `Signed in as ${status.username}`, act: 'settings', warn: false, testId: 'store-chip-account' });
  } else if (status.connection === 'expired') {
    out.push({ text: 'Login expired · Sign in again', act: 'sign_in', warn: true, testId: 'store-chip-expired' });
  } else {
    out.push({ text: 'Not connected · Connect…', act: 'settings', warn: false, testId: 'store-chip-off' });
  }
  if (hasStoreCloud(status.source) && status.cloud_saves && status.connection !== 'not_connected') {
    out.push({ text: 'Cloud saves: on', act: null, warn: false, testId: 'store-chip-cloud' });
  }
  const tool = storeTool(status.source);
  if (tool !== null && status.tool.state !== 'installed' && status.connection === 'connected') {
    out.push({ text: `${tool.name} not installed`, act: 'settings', warn: true, testId: 'store-chip-tool' });
  }
  return out;
}

/** The header's actions on the right: Backup all N… (GOG › Installed), Install from installer… (GOG), Refresh (Available). */
export function storeHeaderActions(
  status: StoreStatus,
  part: StorePart,
  installedCount: number,
): { backupAll: number; installer: boolean; refresh: boolean; select: boolean } {
  const gog = status.source === 'gog';
  return {
    backupAll: gog && part === 'installed' ? installedCount : 0,
    installer: gog,
    refresh: part === 'available' && availBlock(status) === '',
    // Select mode (backups) is offered in GOG › Installed only.
    select: gog && part === 'installed' && installedCount > 0,
  };
}

export type PanelAct =
  | 'open_available'
  | 'installer'
  | 'refresh'
  | 'connect'
  | 'sign_in'
  | 'install_tool'
  | 'open_settings';

export type PanelAction = { label: string; act: PanelAct; primary: boolean; disabled?: boolean };

export type StorePanel = {
  /** `loading` is the skeleton grid with its line, not a panel. */
  kind: 'panel' | 'loading';
  tone: '' | 'warn' | 'err';
  title: string;
  body: string;
  actions: PanelAction[];
  testId: string;
};

/**
 * What replaces the grid, or `null` when the grid shows: nothing installed,
 * everything installed, not connected, login expired, store tool missing /
 * installing / failed, loading, an owned list that failed. A search with no
 * match is the plain "No … match" line (`noMatchText`), not a panel.
 */
export function storePanel(status: StoreStatus, part: StorePart, games: readonly StoreGame[]): StorePanel | null {
  const name = storeName(status.source);
  const gog = status.source === 'gog';
  const panel = (p: Omit<StorePanel, 'kind' | 'tone'> & { tone?: StorePanel['tone'] }): StorePanel => ({
    kind: 'panel',
    tone: p.tone ?? '',
    title: p.title,
    body: p.body,
    actions: p.actions,
    testId: p.testId,
  });
  if (part === 'installed') {
    if (games.length > 0) return null;
    return panel({
      title: `No ${name} games installed yet`,
      body: gog ? 'Install one from Available, or from a GOG offline installer you downloaded.' : 'Install one from Available.',
      actions: [
        { label: 'Open Available', act: 'open_available', primary: true },
        ...(gog ? [{ label: 'Install from installer…', act: 'installer' as const, primary: false }] : []),
      ],
      testId: 'store-state-empty-installed',
    });
  }
  const block = availBlock(status);
  if (block === 'off') {
    return panel({
      title: `${name} isn’t connected`,
      body: `Your installed ${name} games still play. Connect again to see the games you own and install them.`,
      actions: [{ label: `Connect ${name}…`, act: 'connect', primary: true }],
      testId: 'store-state-off',
    });
  }
  if (block === 'expired') {
    return panel({
      tone: 'warn',
      title: `Your ${name} login expired`,
      body: 'Sign in again to see the games you own and install them. Installed games still play.',
      actions: [{ label: 'Sign in again…', act: 'sign_in', primary: true }],
      testId: 'store-state-expired',
    });
  }
  if (block === 'tool') {
    const tool = storeTool(status.source) ?? { name: 'its tool', pkg: '' };
    const size = sizeGb(status.tool.size_bytes);
    const busy = status.tool.state === 'installing';
    const failed = status.tool.state === 'failed';
    return panel({
      tone: failed ? 'err' : '',
      title: failed ? `Couldn’t install ${tool.name}` : `${name} needs ${tool.name}`,
      body: failed
        ? toolFailureBody(status.tool.error)
        : `GRID installs and updates ${name} games with ${tool.name} (${tool.pkg}, ${status.tool.license || 'GPL-3.0'}). It’s ${size}.`,
      actions: [
        {
          label: busy ? `Installing ${tool.name}…` : failed ? 'Try again' : `Install ${tool.name} (${size})`,
          act: 'install_tool',
          primary: true,
          disabled: busy,
        },
      ],
      testId: failed ? 'store-state-tool-failed' : busy ? 'store-state-tool-installing' : 'store-state-tool',
    });
  }
  if (block === 'loading') {
    return { kind: 'loading', tone: '', title: `Loading your ${name} games…`, body: '', actions: [], testId: 'store-state-loading' };
  }
  if (block === 'error') {
    return panel({
      tone: 'err',
      title: `Couldn’t load your ${name} games`,
      body: status.owned_error.trim() || 'Try again in a moment.',
      actions: [{ label: 'Refresh', act: 'refresh', primary: true }],
      testId: 'store-state-error',
    });
  }
  if (games.length === 0) {
    return panel({
      title: `Every ${name} game you own is installed`,
      body: 'New games you buy show here after a refresh.',
      actions: [{ label: 'Refresh', act: 'refresh', primary: false }],
      testId: 'store-state-all-installed',
    });
  }
  return null;
}

/**
 * The failed tool install's body. The checksum case is the design's own words;
 * any other reason the backend gives follows "Couldn’t install …".
 */
export function toolFailureBody(error: string): string {
  const e = error.trim();
  if (e === '' || /checksum|sha-?256/i.test(e)) return 'The download didn’t match its checksum, so GRID deleted it. Nothing else changed.';
  return e;
}

/** "No installed GOG games match “x”" / "No GOG games match “x”". */
export function noMatchText(source: StoreSource, part: StorePart, search: string): string {
  const name = storeName(source);
  return part === 'installed' ? `No installed ${name} games match “${search.trim()}”` : `No ${name} games match “${search.trim()}”`;
}

/** Search filters by title (and the store's name, as in the prototype). */
export function storeGameMatches(game: StoreGame, search: string): boolean {
  const q = search.trim().toLowerCase();
  return q === '' || game.title.toLowerCase().includes(q) || storeName(game.source).toLowerCase().includes(q);
}

/** Titles A–Z, the store views' order. */
export function sortStoreGames(games: readonly StoreGame[]): StoreGame[] {
  return [...games].sort((a, b) => a.title.localeCompare(b.title, undefined, { sensitivity: 'base' }));
}
