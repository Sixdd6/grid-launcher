// A store cover's tags, progress bar and hover action (GameStores README,
// "Store covers"; prototype `stCard`, `stPrimary`, `stProgress`). Pure.
import type { DownloadEntry, StoreGame, StoreStatus } from '../api';
import { isManagedRow, jobAct, jobPercent } from './jobs';
import { hasStoreCloud, managedByClient } from './marks';
import { INSTALL_IN_STEAM, INSTALLING_IN_STEAM, MANAGED_BY_STEAM } from './steam';

export type CoverTagTone = 'queue' | 'update' | 'conflict' | 'unsupported';
export type CoverTag = { text: string; tone: CoverTagTone };

/**
 * Top-left tags, in the prototype's order: the job's percentage ("38%",
 * "QUEUED"), UPDATE, SAVE CONFLICT (red), NOT SUPPORTED.
 */
export function coverTags(game: StoreGame, job: DownloadEntry | null): CoverTag[] {
  const tags: CoverTag[] = [];
  if (job !== null) tags.push({ text: job.status === 'queued' ? 'QUEUED' : `${jobPercent(job)}%`, tone: 'queue' });
  if (game.installed && game.update_version !== '' && job === null) tags.push({ text: 'UPDATE', tone: 'update' });
  if (game.installed && hasStoreCloud(game.source) && game.cloud === 'conflict') tags.push({ text: 'SAVE CONFLICT', tone: 'conflict' });
  if (game.not_supported !== '') tags.push({ text: 'NOT SUPPORTED', tone: 'unsupported' });
  return tags;
}

/** The bar along the cover's bottom edge: the job's percentage, 0 while queued; `null` with no job. */
export function coverProgress(job: DownloadEntry | null): number | null {
  if (job === null) return null;
  return job.status === 'queued' ? 0 : jobPercent(job);
}

/** The bar's colour: teal (the disk graph's) for an install the store's client runs (Steam), else the primary. */
export function coverProgressTone(job: DownloadEntry | null): 'primary' | 'managed' {
  return job !== null && isManagedRow(job) ? 'managed' : 'primary';
}

export type CoverAct = 'play' | 'install' | 'details';
/** `disabled` with its `title`: Steam's "Installing in Steam" ("Managed by Steam"). */
export type CoverPrimary = { label: string; act: CoverAct; disabled?: boolean; title?: string };

/**
 * The hover bar's main button: Installing… / Queued / Updating… / Verifying…
 * (opens Details) while a job runs, Play when installed, Details for a game
 * GRID can't install or a store that isn't signed in, else Install (the
 * install dialog).
 */
export function coverPrimary(game: StoreGame, status: StoreStatus | null, job: DownloadEntry | null): CoverPrimary {
  if (job !== null && isManagedRow(job)) return { label: INSTALLING_IN_STEAM, act: 'details', disabled: true, title: MANAGED_BY_STEAM };
  if (job !== null) {
    if (job.status === 'queued') return { label: 'Queued', act: 'details' };
    const act = jobAct(job);
    const label = act === 'update' ? 'Updating…' : act === 'repair' ? 'Verifying…' : 'Installing…';
    return { label, act: 'details' };
  }
  if (game.installed) return { label: 'Play', act: 'play' };
  if (game.not_supported !== '') return { label: 'Details', act: 'details' };
  // Steam installs through its client: no dialog (GameStores README, "Steam").
  if (managedByClient(game.source)) return { label: INSTALL_IN_STEAM, act: 'install' };
  if (status === null || status.connection !== 'connected') return { label: 'Details', act: 'details' };
  return { label: 'Install', act: 'install' };
}

/** The cloud button shows on an installed GOG (or Epic) game; it opens Details on Cloud saves. */
export function showCoverCloud(game: StoreGame): boolean {
  return game.installed && hasStoreCloud(game.source);
}

/** A game GRID can't install has faded art. */
export function coverFaded(game: StoreGame): boolean {
  return game.not_supported !== '';
}
