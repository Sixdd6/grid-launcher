// Settings › Backup drives in TV mode (checklist items 495–498; tvmode.js
// `drivesPane`): Connected drives (set up or not), then the drives GRID
// remembers but that are not connected, then "Every backup also copies" with
// the Firmware switch. Pure, over the desktop's drive card rules
// (`backups/drives.ts`); `TvDrivesPane.svelte` draws it.
import type { DriveInfo, RememberedDrive } from '../../../../api';
import { driveCardModel, lastSeenText } from '../../../../backups/drives';
import { formatBytes } from '../../../../settings/library';

export type PillTone = 'ok' | 'warn' | 'plain';

export type DriveRowModel = {
  /** `data-k`: "dr-<mount>". */
  key: string;
  /** The pack name, or the volume label before setup. */
  title: string;
  sub: string;
  pill: { text: string; tone: PillTone };
  /** What A does: its page, the setup page, the remembered drive's list,
   *  or nothing (a drive GRID can't use, with the reason). */
  opens: 'page' | 'setup' | 'remembered' | 'none';
  why?: string;
};

export type DrivesPaneModel = {
  connected: DriveRowModel[];
  remembered: DriveRowModel[];
};

export const CONNECTED_HEADING = 'Connected';
export const REMEMBERED_HEADING = 'Not connected';
export const DEFAULTS_HEADING = 'Every backup also copies';
export const NO_DRIVES = 'No drives connected. Plug in a USB drive or SSD.';
export const FIRMWARE_LABEL = 'Firmware';
export const FIRMWARE_SUB = 'Firmware for the systems you backup';

const plural = (n: number, one: string) => `${n} ${one}${n === 1 ? '' : 's'}`;

/** A size for a drive row: "931.5 GB", "1.0 TB" (the desktop card's). */
export function driveSize(bytes: number): string {
  return formatBytes(Math.max(0, bytes));
}

/** The row's key: one per mount. */
export function driveRowKey(mount: string): string {
  return `dr-${mount}`;
}

/** A connected drive's row (item 496). */
export function connectedRow(d: DriveInfo): DriveRowModel {
  const card = driveCardModel(d);
  const ready = d.status === 'ready' && d.pack !== null;
  const games = d.pack ? ` · ${plural(d.pack.game_count, 'game')}` : '';
  const tone: PillTone = d.status === 'ready' ? 'ok' : d.status === 'read_only' ? 'plain' : 'warn';
  let opens: DriveRowModel['opens'];
  if (d.pack !== null) opens = 'page';
  else if (card.canSetUp) opens = 'setup';
  else opens = 'none';
  return {
    key: driveRowKey(d.mount),
    title: card.title,
    sub: `${d.label || card.title} · ${d.fs} · ${driveSize(d.total_bytes)}${ready || d.pack ? games : ''}`,
    pill: { text: card.status, tone },
    opens,
    why: opens === 'none' ? card.note || undefined : undefined,
  };
}

/** "Last seen <when> · <n> games · <size>" (items 497, 514). */
export function rememberedSub(r: RememberedDrive, now: Date): string {
  const seen = lastSeenText(r.last_seen, now);
  return [seen ? `Last seen ${seen}` : '', plural(r.game_count, 'game'), driveSize(r.total_bytes)].filter((s) => s !== '').join(' · ');
}

export function rememberedRow(r: RememberedDrive, now: Date): DriveRowModel {
  return {
    key: driveRowKey(r.mount),
    title: r.name || r.label,
    sub: rememberedSub(r, now),
    pill: { text: 'Not connected', tone: 'plain' },
    opens: 'remembered',
  };
}

/** The pane's drive rows (item 495). */
export function drivesPane(connected: readonly DriveInfo[], remembered: readonly RememberedDrive[], now: Date): DrivesPaneModel {
  return {
    connected: connected.map(connectedRow),
    remembered: remembered.map((r) => rememberedRow(r, now)),
  };
}

/** The remembered drive's list (item 514). */
export function forgetList(r: RememberedDrive, now: Date): { title: string; sub: string; forget: string; done: string } {
  const name = r.name || r.label;
  return { title: name, sub: rememberedSub(r, now), forget: `Forget ${name}`, done: `${name} forgotten` };
}
