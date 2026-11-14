// Settings › Stores (GameStores README, "Settings › Stores"; prototype
// `stStoreCard`, `stToolLine`, `renderStoresPane`). Pure: the card's pill,
// lines, rows and the Disconnect confirm, for every store with a tool (GOG,
// Epic Games).
import type { StoreStatus } from '../api';
import { hasCloud, hasInstaller, storeName, storesLede, storeTool } from './marks';
import { agoText, plural, sizeGb } from './text';

/** The pane's lede, naming the stores this build shows (`storesLede`). */
export const STORES_LEDE = storesLede();

export type Pill = { text: string; tone: 'ok' | 'warn' | 'off' };

export function storePill(status: StoreStatus): Pill {
  if (status.connection === 'connected') return { text: 'Connected', tone: 'ok' };
  if (status.connection === 'expired') return { text: 'Login expired', tone: 'warn' };
  return { text: 'Not connected', tone: 'off' };
}

/** The line under the store's name. */
export function accountLine(status: StoreStatus): string {
  if (status.connection !== 'not_connected') return `Signed in as ${status.username}`;
  // A store with offline installers can have games here with no login (the prototype's GOG card).
  if (hasInstaller(status.source) && status.installed_count > 0) {
    return `Not signed in · ${plural(status.installed_count, 'game')} installed here`;
  }
  return 'Not signed in';
}

/** "8 · 3 installed · updated 5 min ago"; `null` while loading (a spinner and "Loading…"). */
export function ownedLine(status: StoreStatus, nowSeconds: number): string | null {
  if (status.owned === 'loading' || status.owned === 'idle') return null;
  if (status.owned === 'error') return status.owned_error.trim() || 'Couldn’t load your games';
  return `${status.owned_count} · ${status.installed_count} installed · updated ${agoText(status.refreshed_at, nowSeconds)}`;
}

/** The Store tool row: its state and the button it offers. */
export type ToolLine =
  | { kind: 'installed'; text: string }
  | { kind: 'installing'; text: string; pct: number }
  | { kind: 'missing'; text: string; button: string }
  | { kind: 'failed'; text: string; button: string }
  | { kind: 'python'; text: string }
  | { kind: 'pending'; text: string };

/** The failure the tool row shows when the host's Python is too old for the tool (gogdl's zipapp, legendary). */
export function needsPython(error: string): boolean {
  return /python/i.test(error);
}

export function toolLine(status: StoreStatus): ToolLine {
  const tool = storeTool(status.source) ?? { name: 'tool', pkg: '' };
  const t = status.tool;
  if (status.connection === 'not_connected' && t.state !== 'installed' && t.state !== 'installing') {
    return { kind: 'pending', text: `Installs when you connect · ${sizeGb(t.size_bytes)}` };
  }
  switch (t.state) {
    case 'installed':
      return { kind: 'installed', text: `${tool.name} ${t.version} · installed` };
    case 'installing':
      return { kind: 'installing', text: `Installing ${tool.name}…`, pct: t.total > 0 ? Math.min(100, Math.round((t.done * 100) / t.total)) : 0 };
    case 'failed':
      if (needsPython(t.error)) return { kind: 'python', text: `${tool.name} needs Python 3.10 or newer. Install it from your distribution’s software centre, then try again.` };
      return {
        kind: 'failed',
        text: `Couldn’t install ${tool.name}: ${
          t.error.trim() === '' || /checksum|sha-?256/i.test(t.error) ? 'the download didn’t match its checksum, so GRID deleted it.' : t.error.trim()
        }`,
        button: 'Try again',
      };
    default:
      return { kind: 'missing', text: `Not installed · ${sizeGb(t.size_bytes)}`, button: 'Install' };
  }
}

/** "heroic-gogdl · GPL-3.0" / "legendary · GPL-3.0" and the source link (shown without its scheme). */
export function licenseLine(status: StoreStatus): { text: string; url: string; label: string } {
  const tool = storeTool(status.source) ?? { name: '', pkg: '' };
  const url = status.tool.source_url;
  return { text: `${tool.pkg} · ${status.tool.license || 'GPL-3.0'}`, url, label: url.replace(/^https?:\/\//, '') };
}

/** What a not-connected card says connecting does. */
export function notConnectedLede(status: StoreStatus): string {
  const name = storeName(status.source);
  const tool = storeTool(status.source) ?? { name: 'its tool', pkg: '' };
  return `Sign in to ${name} to see the games you own in the Library and install them. ${
    hasInstaller(status.source) ? `You can install ${name} offline installers without signing in. ` : ''
  }GRID installs with ${tool.name} (${tool.pkg}, ${status.tool.license || 'GPL-3.0'}, ${sizeGb(status.tool.size_bytes)}), which it downloads when you connect.`;
}

export function expiredWarning(status: StoreStatus): { bold: string; rest: string } {
  const name = storeName(status.source);
  return { bold: `Your ${name} login expired.`, rest: 'Sign in again to see new games, install and update. Installed games still play.' };
}

/** The inline Disconnect confirm. */
export function disconnectConfirm(status: StoreStatus): { bold: string; rest: string } {
  const name = storeName(status.source);
  const tool = storeTool(status.source)?.name ?? 'its tool';
  const n = status.installed_count;
  const stay =
    n > 0
      ? ` The ${plural(n, 'installed game')} stay and still play; ${
          hasCloud(status.source) ? 'they can’t install, update or sync cloud saves' : 'they can’t install or update'
        } until you connect again.`
      : '';
  return { bold: `Disconnect ${name}?`, rest: `GRID deletes your ${name} login and ${tool} from this computer.${stay}` };
}

export const CLOUD_SWITCH_TEXT = 'Sync before a game starts and after it closes';
