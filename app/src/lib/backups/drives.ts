// Pure reads of a `DrivesSnapshot` (plan §1.3) for the drives store and the
// backup UI. Nothing here talks to the backend; `stores/drives.svelte.ts`
// holds the live snapshot and passes it in.

import type { DriveInfo, DrivesSnapshot, OsChangeOutcome, PackOs, PackSummaryEmulator, RememberedDrive } from '../api';
import { formatBytes } from '../settings/library';

/** The snapshot before the first `list_drives` answer: no drives at all. */
export const EMPTY_SNAPSHOT: DrivesSnapshot = { connected: [], remembered: [] };

/**
 * A server URL reduced to what identifies the server, mirroring grid-core's
 * `backup::pack::server_key`: no userinfo, the host lowercased, the port made
 * explicit, no trailing slash. A pack records the server its games came
 * from, and two URLs with the same key name the same server.
 */
export function serverKey(url: string): string {
  const trimmed = url.trim();
  try {
    const parsed = new URL(trimmed);
    const port = parsed.port !== '' ? parsed.port : defaultPort(parsed.protocol);
    return `${parsed.hostname.toLowerCase()}:${port}${parsed.pathname.replace(/\/+$/, '')}`;
  } catch {
    return trimmed.replace(/\/+$/, '').toLowerCase();
  }
}

function defaultPort(protocol: string): string {
  if (protocol === 'https:') return '443';
  if (protocol === 'http:') return '80';
  return '0';
}

/** Connected drives that are set up and writable: the drives a backup can go to. */
export function readyDrives(snapshot: DrivesSnapshot): DriveInfo[] {
  return snapshot.connected.filter((d) => d.status === 'ready' && d.pack !== null);
}

/**
 * Connected drives whose pack GRID can read: the ready ones and read-only
 * ones that hold a pack. Games install from any of them (Library › Backup
 * drives, install from a drive); only `readyDrives` take backups.
 */
export function packDrives(snapshot: DrivesSnapshot): DriveInfo[] {
  return snapshot.connected.filter((d) => d.pack !== null);
}

/** The connected drive with `key`, or `null` when it is not connected. */
export function driveByKey(snapshot: DrivesSnapshot, key: string): DriveInfo | null {
  return snapshot.connected.find((d) => d.key === key) ?? null;
}

/**
 * The key of the drive the offline shell opens Library on (`initialView`):
 * the first ready drive, else the first read-only one with a pack, since
 * games install from either.
 */
export function firstLibraryDriveKey(snapshot: DrivesSnapshot): string | null {
  return (readyDrives(snapshot)[0] ?? packDrives(snapshot)[0])?.key ?? null;
}

/**
 * Connected drives whose pack holds `romId` backed up from `serverUrl`
 * (Details' "On <pack>" line). Servers compare by `serverKey`, so userinfo,
 * host case, an explicit default port and a trailing slash do not matter.
 * An empty `serverUrl` matches nothing: a rom id means nothing without its
 * server.
 */
export function drivesHoldingRom(snapshot: DrivesSnapshot, serverUrl: string, romId: number): DriveInfo[] {
  if (serverUrl.trim() === '') return [];
  const wanted = serverKey(serverUrl);
  return packDrives(snapshot).filter((d) =>
    d.pack!.rom_ids.some((ref) => ref.rom_id === romId && serverKey(ref.server) === wanted),
  );
}

// ── Settings › Backup drives (plan Task 16) ──────────────────────────────
// The text and numbers each drive card shows. `DriveCard.svelte` renders
// them; nothing below reads a store or the clock (callers pass `now`).

/** The computers a pack can be for, in the order the UI lists them. */
export const PACK_OSES: readonly { os: PackOs; label: string }[] = [
  { os: 'linux', label: 'Linux' },
  { os: 'windows', label: 'Windows' },
];

function plural(n: number, one: string, many = `${one}s`): string {
  return `${n} ${n === 1 ? one : many}`;
}

/** "Linux", "Windows" or "Linux and Windows", in `PACK_OSES` order whatever the input order. */
export function osNames(os: readonly PackOs[]): string {
  return PACK_OSES.filter((o) => os.includes(o.os))
    .map((o) => o.label)
    .join(' and ');
}

/**
 * The pack OS of the computer GRID runs on, from `navigator.platform`: the
 * box ticked by default in the setup form. `null` on macOS, which no pack
 * carries emulators for, so the form starts with nothing ticked.
 */
export function hostPackOs(platform: string): PackOs | null {
  const p = platform.trim().toLowerCase();
  if (p.startsWith('win')) return 'windows';
  if (p.startsWith('linux') || p.includes('x11') || p.includes('freebsd')) return 'linux';
  return null;
}

/** The setup form's pack name before the user types: the volume label, else the mount's last folder. */
export function defaultPackName(drive: Pick<DriveInfo, 'label' | 'mount'>): string {
  const label = drive.label.trim();
  if (label !== '') return label;
  const parts = drive.mount.split(/[\\/]+/).filter((s) => s !== '');
  return parts[parts.length - 1] ?? 'Backup drive';
}

/**
 * Why the setup form can't be submitted, or `null`. The wording matches the
 * backend's own refusals in `setup_drive`.
 */
export function setupError(name: string, os: readonly PackOs[]): string | null {
  if (name.trim() === '') return 'Give the drive a name';
  if (os.length === 0) return 'Choose at least one computer';
  return null;
}

/** Why a rename can't be saved, or `null` (`rename_drive`'s wording). */
export function renameError(name: string): string | null {
  return name.trim() === '' ? 'Give the drive a name' : null;
}

/** OS-change validation: at least one computer stays ticked (`set_drive_os`'s wording). */
export function osChangeError(next: readonly PackOs[]): string | null {
  return next.length === 0 ? 'Choose at least one computer' : null;
}

/** Whether the ticked set differs from the pack's: Save does nothing otherwise. */
export function osChanged(current: readonly PackOs[], next: readonly PackOs[]): boolean {
  return current.length !== next.length || current.some((o) => !next.includes(o));
}

/** The distinct emulator names on a drive, in pack order (one build per OS is one emulator). */
export function emulatorNames(emulators: readonly PackSummaryEmulator[]): string[] {
  const names: string[] = [];
  for (const e of emulators) if (!names.includes(e.name)) names.push(e.name);
  return names;
}

/** The help under the inline "Computers this pack is for" checkboxes of a set-up drive. */
export function osChangeHelp(emulatorCount: number): string {
  if (emulatorCount === 0) {
    return 'No emulators are on this drive yet. Each backup adds a build for every computer you pick.';
  }
  return `Adding one writes its build of the ${plural(emulatorCount, 'emulator')} on this drive. Removing one deletes those builds; games and firmware stay.`;
}

/**
 * The toasts after `set_drive_os` answered: one line for the builds it
 * removed, one for the builds it queued, or a plain confirmation when
 * neither happened (no emulators on the drive yet).
 */
export function osChangeToasts(
  packName: string,
  before: readonly PackOs[],
  after: readonly PackOs[],
  outcome: OsChangeOutcome,
): string[] {
  const removed = before.filter((o) => !after.includes(o));
  const added = after.filter((o) => !before.includes(o));
  const lines: string[] = [];
  if (removed.length > 0 && outcome.removed > 0) {
    lines.push(`Removed the ${osNames(removed)} builds of ${plural(outcome.removed, 'emulator')} from ${packName}`);
  }
  if (added.length > 0 && outcome.queued > 0) {
    lines.push(
      `${osNames(added)} builds of ${plural(outcome.queued, 'emulator')} added to the backup queue for ${packName}`,
    );
  }
  if (lines.length === 0) lines.push(`${packName} is now for ${osNames(after)}`);
  return lines;
}

/** The setup form's warning on a FAT32 drive. GRID never formats a drive, so it only suggests exFAT. */
export const FAT32_WARNING = {
  title: "FAT32 can't hold files over 4 GB.",
  body: "Games with a larger file (many PlayStation 2, GameCube and Windows games) can't go on this drive. Reformat it as exFAT to back up everything.",
} as const;

/**
 * The second confirmation of "Remove GRID files…", with the bytes it frees.
 * `gameCount` is `null` when the pack can't be read, so its games are unknown.
 */
export function wipeConfirmText(
  label: string,
  gameCount: number | null,
  bytes: number,
): { title: string; body: string } {
  const games = gameCount === null ? '' : `${plural(gameCount, 'game')} and `;
  return {
    title: `Delete the gridpack folder from ${label}?`,
    body: `This removes ${games}${formatBytes(bytes)} of GRID files from the drive. Nothing else on the drive is touched, and games installed on this computer stay.`,
  };
}

/** The pack folder on a drive: `<mount>/gridpack`, with the mount's own separator. */
export function gridpackPath(mount: string): string {
  if (mount.endsWith('/') || mount.endsWith('\\')) return `${mount}gridpack`;
  const sep = mount.includes('\\') || /^[A-Za-z]:$/.test(mount) ? '\\' : '/';
  return `${mount}${sep}gridpack`;
}

/** A day for people ("9 Nov 2026"), or `''` for a missing or bad timestamp. */
export function formatDay(iso: string, locale?: string): string {
  const t = Date.parse(iso);
  if (iso.trim() === '' || Number.isNaN(t)) return '';
  return new Date(t).toLocaleDateString(locale, { day: 'numeric', month: 'short', year: 'numeric' });
}

/** When a remembered drive was last seen: "today", "yesterday", "3 days ago", then the day. */
export function lastSeenText(iso: string, now: Date, locale?: string): string {
  const t = Date.parse(iso);
  if (iso.trim() === '' || Number.isNaN(t)) return '';
  // Calendar days in local time, so 23:50 yesterday is "yesterday", not "today".
  const startOf = (d: Date) => Date.UTC(d.getFullYear(), d.getMonth(), d.getDate());
  const days = Math.round((startOf(now) - startOf(new Date(t))) / 86_400_000);
  if (days <= 0) return 'today';
  if (days === 1) return 'yesterday';
  if (days < 30) return `${days} days ago`;
  return formatDay(iso, locale);
}

/** One segment of a drive's usage bar. `pct` is of the drive's size, 0–100. */
export type UsageSegment = { kind: 'other' | 'grid' | 'free'; bytes: number; pct: number; legend: string };

/**
 * A drive's usage bar: other files, the gridpack folder, free space. The
 * three always add up to the drive's size, so a stale `gridpack_bytes`
 * larger than what the drive reports used never pushes the bar past 100%.
 */
export function usageSegments(totalBytes: number, freeBytes: number, gridBytes: number): UsageSegment[] {
  const total = Math.max(0, totalBytes);
  const free = Math.min(Math.max(0, freeBytes), total);
  const grid = Math.min(Math.max(0, gridBytes), total - free);
  const other = Math.max(0, total - free - grid);
  const pct = (v: number) => (total > 0 ? (v / total) * 100 : 0);
  return [
    { kind: 'other', bytes: other, pct: pct(other), legend: `Other files ${formatBytes(other)}` },
    { kind: 'grid', bytes: grid, pct: pct(grid), legend: `GRID ${formatBytes(grid)}` },
    { kind: 'free', bytes: free, pct: pct(free), legend: `Free ${formatBytes(free)}` },
  ];
}

/** The usage bar's accessible name: "61.4 GB free of 931.5 GB". */
export function usageLabel(totalBytes: number, freeBytes: number): string {
  return `${formatBytes(Math.max(0, freeBytes))} free of ${formatBytes(Math.max(0, totalBytes))}`;
}

export type DriveTone = 'ok' | 'neutral' | 'warn';

/** What a connected drive's card shows, apart from its forms. */
export type DriveCardModel = {
  /** The pack name, or the volume label before setup. */
  title: string;
  /** "SAMSUNG T7 · exFAT · 931.5 GB" (the label only when it differs from the title). */
  subtitle: string;
  /** The status pill. */
  status: string;
  tone: DriveTone;
  /** A sentence under the header for a drive without the full set-up body, else `''`. */
  note: string;
  /** "Linux and Windows" for a drive with a pack, else `''`. */
  forLine: string;
  /** The usage bar, the facts, Show in Library and Open folder. */
  showsPack: boolean;
  /** Change… and Rename…: the pack can be written. */
  canEdit: boolean;
  /** Set up for GRID…. */
  canSetUp: boolean;
  /** Remove GRID files…: a gridpack folder is there and the drive is writable. */
  canRemove: boolean;
};

const STATUS_TEXT: Record<DriveInfo['status'], { text: string; tone: DriveTone }> = {
  ready: { text: 'Ready for backups', tone: 'ok' },
  not_set_up: { text: 'Not set up', tone: 'neutral' },
  read_only: { text: 'Read-only', tone: 'neutral' },
  unreadable: { text: "Can't read", tone: 'warn' },
  newer_format: { text: 'Newer GRID needed', tone: 'warn' },
};

function statusNote(d: DriveInfo): string {
  switch (d.status) {
    case 'not_set_up':
      return `${formatBytes(d.free_bytes)} free. Set it up to back up games to it.`;
    case 'read_only':
      return d.pack
        ? `${d.label} is read-only. Games install from it, but backups can't go to it.`
        : `${d.label} is read-only, so GRID can't set it up.`;
    case 'unreadable':
      return d.problem.trim() !== ''
        ? `GRID can't read the gridpack folder on ${d.label}: ${d.problem}`
        : `GRID can't read the gridpack folder on ${d.label}.`;
    case 'newer_format':
      return `${d.label} was set up by a newer version of GRID. Update GRID to use it.`;
    default:
      return '';
  }
}

export function driveCardModel(d: DriveInfo): DriveCardModel {
  const pack = d.pack;
  const label = pack && d.label !== '' && d.label !== pack.name ? `${d.label} · ` : '';
  const { text, tone } = STATUS_TEXT[d.status];
  return {
    title: pack ? pack.name : defaultPackName(d),
    subtitle: `${label}${d.fs} · ${formatBytes(d.total_bytes)}`,
    status: text,
    tone,
    note: statusNote(d),
    forLine: pack ? osNames(pack.os) : '',
    showsPack: pack !== null,
    canEdit: d.status === 'ready' && pack !== null,
    canSetUp: d.status === 'not_set_up' && !d.read_only,
    canRemove: !d.read_only && (d.status === 'ready' || d.status === 'unreadable'),
  };
}

/** A set-up drive's facts: games, emulators, firmware, last backup, folder. */
export function packFacts(d: DriveInfo, locale?: string): { label: string; value: string; mono: boolean }[] {
  const pack = d.pack;
  if (!pack) return [];
  const emus = emulatorNames(pack.emulators);
  const empty = pack.game_count === 0 && emus.length === 0 && pack.firmware.length === 0;
  return [
    { label: 'Games', value: String(pack.game_count), mono: false },
    { label: 'Emulators', value: emus.length > 0 ? emus.join(', ') : 'None', mono: false },
    { label: 'Firmware', value: pack.firmware.length > 0 ? pack.firmware.join(', ') : 'None', mono: false },
    { label: 'Last backup', value: empty ? 'None yet' : formatDay(pack.updated, locale) || 'Unknown', mono: false },
    { label: 'Folder', value: gridpackPath(d.mount), mono: true },
  ];
}

/** A remembered, not-connected drive's card. */
export function rememberedCardModel(
  r: RememberedDrive,
  now: Date,
  locale?: string,
): { title: string; subtitle: string; note: string } {
  const seen = lastSeenText(r.last_seen, now, locale);
  const facts = [seen !== '' ? `Last seen ${seen}` : '', plural(r.game_count, 'game')].filter((s) => s !== '');
  const label = r.label !== '' && r.label !== r.name ? `${r.label} · ` : '';
  return {
    title: r.name || r.label,
    subtitle: `${label}${r.fs} · ${formatBytes(r.total_bytes)}`,
    note: `${facts.join(' · ')}. Plug it in to back up to it or install from it.`,
  };
}
