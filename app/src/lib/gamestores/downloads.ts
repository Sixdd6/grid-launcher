// The Downloads words for store rows and for RomM installs that run a GOG
// installer from their archive (GameStores README, "Downloads and
// notifications", "Offline GOG installer › Progress", "RomM games with a GOG
// installer in their archive", "GOG games on backup drives"). Worded from the
// row's `kind`, `status`, `phase` and `failure`. Pure, and free of the
// downloads formatter so that module can import this one.
import type { DownloadEntry, DownloadKind, JobFailure, StoreSource } from '../api';
import { jobPercent } from './jobs';
import { isEnabledStore, readableVersions, STORE_ORDER, storeName, storeOfPlatform } from './marks';

/** The store a store row belongs to, from its game key (`gog:…`), tool source (`epic`) or platform ("Epic Games"). */
export function entrySource(e: DownloadEntry): StoreSource | null {
  const id = e.source_id.trim();
  const head = id.includes(':') ? id.slice(0, id.indexOf(':')) : id;
  if ((STORE_ORDER as readonly string[]).includes(head)) return head as StoreSource;
  const byName = storeOfPlatform(e.platform);
  return byName !== null && isEnabledStore(byName) ? byName : null;
}

/** Whether the row is a store game, store tool, or GOG backup row (it gets the store's badge). */
export function hasStoreBadge(e: DownloadEntry): boolean {
  if (e.job === 'store') return true;
  return e.kind === 'backup_store_game';
}

/** The small kind badge for store kinds: none for a game, Update, Repair, Content (DLC), Store tool, Installer, Backup. */
export function storeKindLabel(kind: DownloadKind): string | null {
  switch (kind) {
    case 'store_install':
    case 'store_restore':
      return '';
    case 'store_update':
      return 'Update';
    case 'store_repair':
      return 'Repair';
    case 'store_dlc':
      return 'Content';
    case 'store_tool':
      return 'Store tool';
    case 'store_installer':
      return 'Installer';
    case 'backup_store_game':
      return 'Backup';
    default:
      return null;
  }
}

/** "41.2 MB/s", the prototype's rate form (binary units, one decimal). */
export function rateText(bps: number): string {
  const units = ['B', 'KB', 'MB', 'GB'];
  let v = Math.max(0, bps);
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i += 1;
  }
  return `${i === 0 ? v.toFixed(0) : v.toFixed(1)} ${units[i]}/s`;
}

function sizeText(bytes: number): string {
  const gb = Math.max(0, bytes) / 1024 ** 3;
  if (gb >= 100) return `${Math.round(gb)} GB`;
  if (gb >= 1) return `${gb.toFixed(1)} GB`;
  if (gb >= 0.001) return `${Math.max(1, Math.round(gb * 1000))} MB`;
  return '<1 MB';
}

/** Extra facts a line can use that the row itself does not carry. */
export type LineContext = {
  /** An updated game's new version ("Updated to 2.2"). */
  version?: string;
  /** The Windows compatibility tool a GOG installer runs under; `''` on a Windows host (it runs natively). */
  compat?: string;
};

const WIZARD_LINE = 'Waiting for the installer’s setup window · finish it there';

/** The failed line's reason, worded from `failure` when it has the parts, else the backend's error. */
export function failureReason(e: DownloadEntry, failure: JobFailure | null = e.failure): string {
  const error = e.error.trim();
  if (failure !== null) {
    const offline = e.kind === 'store_installer';
    switch (failure.code) {
      case 'installer_dlc_base_missing': {
        const base = failure.base_title.trim() || 'its base game';
        return offline
          ? `This is DLC for ${base}, which isn’t installed. Install ${base} first.`
          : `Its archive holds a DLC installer for ${base}, which isn’t installed. Install ${base} first; the download was removed.`;
      }
      case 'installer_needs_wizard':
        return `This installer needs its setup window. Install it in desktop mode.${
          failure.kept ? (offline ? ' The installer was kept.' : ' The download was kept.') : ''
        }`;
      default:
        break;
    }
  }
  return error || 'Unknown error';
}

/**
 * A store row's status line ("Downloading 45% · 41.2 MB/s", "Installing 90%",
 * "Verifying files 45%", the installer and restore phases, "Waiting for a
 * slot", and the completed and failed lines).
 */
export function storeEntryLine(e: DownloadEntry, ctx: LineContext = {}): string {
  const p = jobPercent(e);
  const drive = e.drive_label.trim() || e.drive_name.trim() || 'the drive';
  switch (e.status) {
    case 'queued':
      return 'Waiting for a slot';
    case 'cancelling':
      return 'Cancelling…';
    case 'cancelled':
      return 'Cancelled';
    case 'failed':
      return `Failed · ${failureReason(e)}`;
    case 'completed':
      return completedLine(e, ctx, drive);
    default:
      break;
  }
  if (e.kind === 'store_tool' || e.phase === 'checksum') return `Downloading ${p}% · checking its SHA-256`;
  switch (e.phase) {
    case 'verifying':
      return `Verifying files ${p}%`;
    case 'unpacking':
      return `Unpacking the game files ${p}%`;
    case 'installer_silent':
      return `Running the installer silently · ${p}%`;
    case 'installer_wizard':
      return WIZARD_LINE;
    case 'running_installer':
      return `Running the installer · ${p}%`;
    case 'copying_installer':
      return `Copying the installer from ${drive} ${p}%`;
    case 'copying_from_drive':
      return `Copying from ${drive} ${p}%`;
    case 'installing':
      return `Installing ${p}%`;
    case 'downloading':
      return `Downloading ${p}% · ${rateText(e.speed_bps)}`;
    default:
      break;
  }
  if (e.kind === 'store_repair') return `Verifying files ${p}%`;
  if (e.kind === 'store_restore') return `Copying from ${drive} ${p}%`;
  if (e.status === 'installing') return `Installing ${p}%`;
  return `Downloading ${p}% · ${rateText(e.speed_bps)}`;
}

function completedLine(e: DownloadEntry, ctx: LineContext, drive: string): string {
  switch (e.kind) {
    case 'store_tool':
      return e.detail.trim() ? `Installed · ${e.detail.trim()}` : 'Installed';
    case 'store_update': {
      // Amazon's versions are opaque ids: its row reads "Updated".
      const src = entrySource(e);
      return ctx.version && (src === null || readableVersions(src)) ? `Updated to ${ctx.version}` : 'Updated';
    }
    case 'store_repair':
      return e.detail.trim() ? `Verified · ${e.detail.trim()}` : 'Verified';
    case 'store_dlc':
      return 'Installed';
    case 'store_installer':
      return 'Installed from its installer';
    case 'store_restore':
      // The queue never resets `phase`, so the last phase says how it restored.
      return e.phase === 'running_installer' || e.phase === 'copying_installer'
        ? `Installed from ${drive} · ran its installer · saves restored`
        : `Copied from ${drive} · saves restored`;
    default: {
      const bytes = e.install_total_bytes > 0 ? e.install_total_bytes : e.total_bytes;
      return bytes > 0 ? `Installed · ${sizeText(bytes)}` : 'Installed';
    }
  }
}

/** Whether a RomM game row is in, or ended in, a GOG installer phase. */
export function isRommInstallerRow(e: DownloadEntry): boolean {
  if (e.job !== 'game') return false;
  return (
    e.phase === 'installer_silent' ||
    e.phase === 'installer_wizard' ||
    e.phase === 'unpacking' ||
    e.phase === 'running_installer' ||
    e.failure !== null
  );
}

/**
 * A RomM install's line while it runs its archive's GOG installer, or `null`
 * for a row with no installer phase (the ordinary game line applies):
 * "Windows · Running the GOG installer silently under GE-Proton10-17 · 40%",
 * "Linux · Unpacking the GOG installer 40%", the wizard wait, "Windows ·
 * Installed · ran its GOG installer", or a "Failed ·" line.
 */
export function rommInstallerLine(e: DownloadEntry, ctx: LineContext = {}): string | null {
  if (!isRommInstallerRow(e)) return null;
  const plat = e.platform.trim();
  const lead = plat === '' ? '' : `${plat} · `;
  const p = jobPercent(e);
  if (e.status === 'failed') return `${lead}Failed · ${failureReason(e)}`;
  if (e.status === 'completed') return `${lead}Installed · ran its GOG installer`;
  if (e.status === 'cancelled') return `${lead}Cancelled`;
  switch (e.phase) {
    case 'installer_wizard':
      return `${lead}${WIZARD_LINE}`;
    case 'unpacking':
      return `${lead}Unpacking the GOG installer ${p}%`;
    case 'installer_silent':
    case 'running_installer': {
      const under = ctx.compat && ctx.compat.trim() !== '' ? ` under ${ctx.compat.trim()}` : '';
      return `${lead}Running the GOG installer silently${under} · ${p}%`;
    }
    default:
      return null;
  }
}

/** A RomM install's line while it downloads, with its platform first ("Windows · Downloading 45% · 38.2 MB/s"). */
export function rommDownloadLine(e: DownloadEntry): string {
  const plat = e.platform.trim();
  return `${plat === '' ? '' : `${plat} · `}Downloading ${jobPercent(e)}% · ${rateText(e.speed_bps)}`;
}

/**
 * A GOG backup row's line: "GOG · Installer files and saves · Reading its GOG
 * details", "… · Copying from this computer to SAMSUNG T7 45%", done "… ·
 * Copied from this computer · Written to Couch kit". `detail` carries what
 * goes ("Installer files and saves" / "Installed copy and saves") when the
 * backend fills it.
 */
export function storeBackupLine(e: DownloadEntry): string {
  const what = e.detail.trim();
  const lead = what === '' ? 'GOG · ' : `GOG · ${what} · `;
  const volume = e.drive_label.trim() || e.drive_name.trim() || 'the drive';
  const pack = e.drive_name.trim() || e.drive_label.trim() || 'the drive';
  switch (e.status) {
    case 'queued':
      return `${lead}Waiting · to ${pack}`;
    case 'downloading':
      return `${lead}Reading its GOG details`;
    case 'installing':
      return `${lead}Copying from this computer to ${volume} ${jobPercent(e)}%`;
    case 'cancelling':
      return 'Cancelling…';
    case 'completed':
      return `${lead}Copied from this computer · Written to ${pack}`;
    case 'failed':
      return `Failed — ${e.error.trim() || 'Unknown error'}`;
    case 'cancelled':
      return 'Cancelled';
    default:
      return '';
  }
}

/** The strip's line for a store row: "Copper Saints · GOG · Downloading 63% · 41.2 MB/s". */
export function storeStripLine(e: DownloadEntry, ctx: LineContext = {}): string {
  const source = entrySource(e);
  const name = source !== null && isEnabledStore(source) ? storeName(source) : e.platform.trim() || 'GOG';
  return `${e.title} · ${name} · ${storeEntryLine(e, ctx)}`;
}

/** Whether the row's line is an error (red, with Retry and Dismiss). */
export function isErrorLine(e: DownloadEntry): boolean {
  return e.status === 'failed';
}
