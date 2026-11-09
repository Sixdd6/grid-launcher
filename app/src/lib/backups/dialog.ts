// The Backup dialog's view model (GameBackups README, "The backup dialog";
// plan Task 19). Pure: `BackupDialog.svelte` passes in the drives and the
// `plan_backup` answers and renders what comes back. Nothing here reads a
// store or talks to the backend.

import type { BackupPlan, BackupRequest, DriveInfo, PackOs, PlanEmulator } from '../api';
import { formatBytes } from '../settings/library';
import { osNames, usageSegments } from './drives';

function plural(n: number, one: string, many = `${one}s`): string {
  return `${n} ${n === 1 ? one : many}`;
}

/** A size for the dialog: "11.1 GB", "76 GB", "120 MB" (a whole number drops its ".0"). */
export function sizeText(bytes: number): string {
  return formatBytes(Math.max(0, Math.round(bytes))).replace(/\.0 /, ' ');
}

// ── What is being backed up ──────────────────────────────────────────────

/** The three ways a backup starts, each with its own title. */
export type BackupSubject =
  | { kind: 'game'; name: string }
  | { kind: 'games'; count: number }
  | { kind: 'platform'; platform: string; count: number };

/** "Backup Cinder Peaks", "Backup 3 games" or "Backup PlayStation · 4 games". */
export function backupTitle(subject: BackupSubject): string {
  switch (subject.kind) {
    case 'game':
      return `Backup ${subject.name}`;
    case 'games':
      return `Backup ${plural(subject.count, 'game')}`;
    case 'platform':
      return `Backup ${subject.platform} · ${plural(subject.count, 'game')}`;
  }
}

/**
 * The `BackupRequest` for a subject. `platformIds` go along for a whole
 * platform (or a selection inside one), so the plan reads one game list per
 * platform instead of one detail per game.
 */
export function backupRequest(subject: BackupSubject, romIds: readonly number[], platformIds: readonly number[] = []): BackupRequest {
  const request: BackupRequest = { rom_ids: [...romIds], title: backupTitle(subject) };
  if (platformIds.length > 0) request.platform_ids = [...platformIds];
  return request;
}

/** The lede under the title. */
export function dialogLede(host: string): string {
  const from = host.trim() === '' ? 'the server' : host.trim();
  return `GRID downloads each game from ${from} and writes it, with its art and details, to the drive’s gridpack folder.`;
}

// ── Drives ───────────────────────────────────────────────────────────────

/** One part of a drive row's usage bar. `adding` is this backup's (hatched) share. */
export type DialogUsageSegment = { kind: 'other' | 'grid' | 'adding' | 'free'; pct: number; over: boolean };

export type DriveRowModel = {
  key: string;
  /** The pack name. */
  name: string;
  /** "SAMSUNG T7 · exFAT". */
  sub: string;
  /** "61.4 GB free", or "Needs 76 GB · 61.4 GB free" when the backup does not fit. */
  freeText: string;
  /** The backup does not fit: the free text takes the danger colour. */
  over: boolean;
  segments: DialogUsageSegment[];
  /** "61.4 GB free of 931.5 GB": the bar's accessible name. */
  usageLabel: string;
};

/**
 * One radio row of the Drive list. `plan` is that drive's plan, or `null`
 * while it is being made (the row then shows only the free space). Only the
 * chosen row hatches this backup's share into its usage bar.
 */
export function driveRow(drive: DriveInfo, plan: BackupPlan | null, chosen: boolean): DriveRowModel {
  const free = Math.max(0, drive.free_bytes);
  const over = plan !== null && !plan.fits;
  const base = usageSegments(drive.total_bytes, drive.free_bytes, drive.pack?.gridpack_bytes ?? 0);
  const total = Math.max(0, drive.total_bytes);
  const pct = (v: number) => (total > 0 ? (v / total) * 100 : 0);
  const freeSeg = base.find((s) => s.kind === 'free')!;
  const adding = chosen && plan !== null ? Math.min(plan.bytes_to_write, freeSeg.bytes) : 0;
  const segments: DialogUsageSegment[] = [
    ...base.filter((s) => s.kind !== 'free').map((s) => ({ kind: s.kind, pct: s.pct, over: false })),
    { kind: 'adding', pct: pct(adding), over },
    { kind: 'free', pct: pct(freeSeg.bytes - adding), over: false },
  ];
  return {
    key: drive.key,
    name: drive.pack?.name ?? drive.label,
    sub: `${drive.label} · ${drive.fs}`,
    freeText: over ? `Needs ${sizeText(plan!.bytes_to_write)} · ${sizeText(free)} free` : `${sizeText(free)} free`,
    over,
    segments: segments.filter((s) => s.kind === 'free' || s.pct > 0),
    usageLabel: `${sizeText(free)} free of ${sizeText(total)}`,
  };
}

/**
 * The drive the dialog starts on, or keeps: the one already chosen while it
 * is still a backup target, else the first.
 */
export function chooseDrive(ready: readonly DriveInfo[], current: string | null): string | null {
  if (current !== null && ready.some((d) => d.key === current)) return current;
  return ready[0]?.key ?? null;
}

/** With no set-up drive connected: what the Drive section says instead of the list. */
export function noDrivesText(connected: readonly DriveInfo[]): { title: string; body: string } {
  const fresh = connected.filter((d) => d.status === 'not_set_up' && !d.read_only).map((d) => d.label || d.mount);
  const also = fresh.length > 0 ? `, or set up ${fresh.join(' or ')} (connected now)` : '';
  return {
    title: 'No backup drive connected',
    body: `Plug in a drive you set up for GRID${also}. Set up a drive once in Settings › Backup drives.`,
  };
}

// ── Also copy ────────────────────────────────────────────────────────────

function sameOs(a: readonly PackOs[], b: readonly PackOs[]): boolean {
  return a.length === b.length && a.every((o) => b.includes(o));
}

/**
 * The fixed Emulators line: "PCSX2 for Linux and Windows · 120 MB", or
 * "Already on the drive", or "None needed". `notes` are the emulators'
 * own remarks ("Cemu is only for Linux", a refusal's reason).
 */
export function emulatorLine(plan: BackupPlan): { text: string; notes: string[] } {
  const notes = plan.emulators.map((e) => e.note.trim()).filter((n) => n !== '');
  const writing = plan.emulators.filter((e) => !e.already && e.os.length > 0);
  if (writing.length === 0) {
    const anyAlready = plan.emulators.some((e) => e.already);
    return { text: anyAlready ? 'Already on the drive' : 'None needed', notes };
  }
  // Emulators that go for the same computers share one "for …".
  const groups: { os: PackOs[]; names: string[] }[] = [];
  for (const e of writing) {
    const group = groups.find((g) => sameOs(g.os, e.os));
    if (group) group.names.push(e.name);
    else groups.push({ os: e.os, names: [e.name] });
  }
  const what = groups.map((g) => `${g.names.join(', ')} for ${osNames(g.os)}`).join('; ');
  const size = emulatorBytes(writing);
  return { text: size === null ? what : `${what} · ${sizeText(size)}`, notes };
}

/** The bytes of the emulators being written, or `null` when any size is unknown. */
function emulatorBytes(writing: readonly PlanEmulator[]): number | null {
  let sum = 0;
  for (const e of writing) {
    if (e.bytes === null) return null;
    sum += e.bytes;
  }
  return sum;
}

/**
 * The Firmware checkbox: the systems and file counts it copies, or the
 * reason it is disabled (the drive has the firmware already, or the server
 * has none).
 */
export function firmwareLine(plan: BackupPlan): { text: string; disabled: boolean } {
  if (plan.firmware_reason.trim() !== '') return { text: plan.firmware_reason, disabled: true };
  const sets = plan.firmware.filter((f) => !f.already);
  return {
    text: sets.map((f) => `${f.platform_name} (${plural(f.files, 'file')})`).join(', '),
    disabled: sets.length === 0,
  };
}

/**
 * The optional "Make Windows games work offline" checkbox, or `null` when
 * the plan offers none (the pack is not for Linux, or no Windows game is
 * being written).
 */
export function runtimeLine(plan: BackupPlan): { label: string; text: string; disabled: boolean } | null {
  const rt = plan.runtime;
  if (rt === null) return null;
  const label = 'Make Windows games work offline';
  if (rt.already) return { label, text: `The ${rt.name} is already on the drive`, disabled: true };
  const size = rt.bytes === null ? '' : ` (${sizeText(rt.bytes)})`;
  return {
    label,
    text: `Adds the ${rt.name}${size} that umu-run otherwise downloads on its first launch`,
    disabled: false,
  };
}

// ── Notes and footer ─────────────────────────────────────────────────────

function names(list: readonly { name: string }[]): string {
  return list.map((g) => g.name).join(', ');
}

/**
 * The notes under Also copy: games already on the drive are skipped, and on
 * a FAT32 drive games with a file over 4 GB can't go. `warn` notes take the
 * warning colour.
 */
export function planNotes(plan: BackupPlan, drive: DriveInfo): { text: string; warn: boolean }[] {
  const notes: { text: string; warn: boolean }[] = [];
  const pack = drive.pack?.name ?? drive.label;
  const skipped = plan.skipped;
  if (skipped.length > 0) {
    const verb = skipped.length === 1 ? '1 game is' : `${skipped.length} games are`;
    const list = skipped.length <= 3 ? `: ${names(skipped)}` : '';
    notes.push({ text: `${verb} already on ${pack} and will be skipped${list}`, warn: false });
  }
  const big = plan.too_big;
  if (big.length > 0) {
    const verb = big.length === 1 ? '1 game has' : `${big.length} games have`;
    notes.push({
      text: `${verb} a file over 4 GB, which ${drive.label} (${drive.fs}) can’t hold: ${names(big)}`,
      warn: true,
    });
  }
  return notes;
}

export type FooterModel = {
  /** "11.1 GB to write", "Nothing new to write", or `''`. */
  note: string;
  /** The main button's label. */
  button: string;
  disabled: boolean;
};

/**
 * The footer for the chosen drive's plan (`null` while it is being made).
 * The button reads "Backup 3 games", "Nothing to backup" when every game
 * is on the drive or can't go, or "Not enough space" when it does not fit.
 */
export function footerModel(plan: BackupPlan | null): FooterModel {
  if (plan === null) return { note: '', button: 'Checking the drive…', disabled: true };
  const n = plan.games_to_write.length;
  if (n === 0) return { note: 'Nothing new to write', button: 'Nothing to backup', disabled: true };
  const note = `${sizeText(plan.bytes_to_write)} to write`;
  if (!plan.fits) return { note, button: 'Not enough space', disabled: true };
  return { note, button: `Backup ${plural(n, 'game')}`, disabled: false };
}
