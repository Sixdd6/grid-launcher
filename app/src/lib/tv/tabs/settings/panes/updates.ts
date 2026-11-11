// Settings › Updates in TV mode (checklist items 268–269; tvmode.js
// `paneRows('updates')`): the version, when GRID last checked, the newer
// release when there is one (GRID only checks), and Debug logging.
import type { AppUpdateNotice } from '../../../../api';
import type { Row } from '../../../rows';

export type UpdatesState = {
  version: string;
  /** The backend's `checked_at` (RFC 3339), null while no check completed. */
  checkedAt: string | null;
  now: Date;
  /** The newer release, dismissed or not. */
  release: AppUpdateNotice | null;
  /** Debug logging, null until read. */
  debug: boolean | null;
};

export type UpdatesActions = { openRelease: () => void; setDebug: (on: boolean) => void };

export function versionText(version: string): string {
  const v = version.trim().replace(/^v/i, '');
  return v === '' ? 'GRID (version unknown)' : `GRID ${v}`;
}

const pad = (n: number) => String(n).padStart(2, '0');
const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];

/** "Today at 09:14", "Yesterday at 18:02", "3 Nov at 09:14", or "Not
 *  checked yet" (local time). */
export function lastCheckedText(checkedAt: string | null, now: Date): string {
  if (checkedAt === null) return 'Not checked yet';
  const at = new Date(checkedAt);
  if (Number.isNaN(at.getTime())) return 'Not checked yet';
  const time = `${pad(at.getHours())}:${pad(at.getMinutes())}`;
  const day = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
  const days = Math.round((day(now) - day(at)) / 86_400_000);
  if (days === 0) return `Today at ${time}`;
  if (days === 1) return `Yesterday at ${time}`;
  return `${at.getDate()} ${MONTHS[at.getMonth()]}${at.getFullYear() === now.getFullYear() ? '' : ` ${at.getFullYear()}`} at ${time}`;
}

/** "GRID 0.9.13" from a release tag ("v0.9.13"). */
export function releaseName(tag: string): string {
  return versionText(tag);
}

export const UPDATE_DISMISSED = 'Update notice dismissed';

export function updatesRows(s: UpdatesState, a: UpdatesActions): Row[] {
  const rows: Row[] = [
    { type: 'info', key: 'version', label: 'Version', value: versionText(s.version) },
    { type: 'info', key: 'checked', label: 'Last checked', value: lastCheckedText(s.checkedAt, s.now) },
  ];
  if (s.release) {
    rows.push({
      type: 'action',
      key: 'release',
      label: `${releaseName(s.release.tag)} is out`,
      sub: 'Update with the installer you used',
      run: a.openRelease,
    });
  }
  rows.push({ type: 'switch', key: 'debug', label: 'Debug logging', on: s.debug ?? false, set: a.setDebug });
  return rows;
}
