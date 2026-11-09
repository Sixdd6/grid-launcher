// Pure reads of a `DrivesSnapshot` (plan §1.3) for the drives store and the
// backup UI. Nothing here talks to the backend; `stores/drives.svelte.ts`
// holds the live snapshot and passes it in.

import type { DriveInfo, DrivesSnapshot } from '../api';

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
