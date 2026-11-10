// Settings › Systems in TV mode (checklist items 252–260; tvmode.js
// `systemsPane`, `ACT.installRec`, `ACT.checkUpd`): the heading with
// Install recommended (N) / "Installing N emulators…", Check for updates
// and Add manually; one row per system, those with no emulator first, then
// the rest, then Windows (Proton); then Other emulators. Pure: the words,
// the order and what each row shows; TvSystemsPane reads the shared
// SystemsStore and runs the actions.
import type { EmulatorEntry, SetupOption } from '../../../../api';
import {
  canPlay,
  companionParent,
  installingRecommendedLabel,
  installRecommendedLabel,
  missingText,
  otherEmulatorNote,
  shortName,
  STATUS_LABELS,
  testKeyOf,
  type SystemRow,
  type SystemStatus,
} from '../../../../systems/rows';
import type { CompanionLink } from '../../../../api';

/** The header's first button: absent, Install recommended (N), or the
 *  disabled "Installing N emulators…" while those installs run (253). */
export type RecButton = { label: string; disabled: boolean } | null;

/**
 * Install recommended (252–254; tvmode.js `systemsPane`): while its own
 * press is being queued, "Installing N emulators…" for the N it asked for;
 * else Install recommended (N) for the systems that can still get one;
 * else, while no-emulator systems are still installing, "Installing N
 * emulators…" (it can't be pressed again); else nothing, and focus moves to
 * the control now in its place.
 */
export function recButton(o: { queueing: number; todo: number; inFlight: number }): RecButton {
  if (o.queueing > 0) return { label: installingRecommendedLabel(o.queueing), disabled: true };
  if (o.todo > 0) return { label: installRecommendedLabel(o.todo), disabled: false };
  if (o.inFlight > 0) return { label: installingRecommendedLabel(o.inFlight), disabled: true };
  return null;
}

/** A on the disabled Installing button says this. TODO(design): not in the design. */
export const REC_RUNNING = 'Those emulators are installing. Downloads shows how far they are.';
/** A on Check for updates while a check runs. TODO(design): not in the design. */
export const CHECK_RUNNING = 'Already checking for updates.';

/** One system row's look (256–257). */
export type SystemItem = {
  /** `data-k`: `sy-<slug>`, Windows `sy-windows`. */
  k: string;
  /** The test key (`tv-sys-row-<testKey>`). */
  testKey: string;
  label: string;
  sub: string;
  status: SystemStatus;
  /** The pill's text: "Ready" (green dot) or the status in the warning style. */
  pill: string;
  ok: boolean;
  /** X's label, or null when X does nothing now. */
  x: string | null;
};

/** The pill (256): "Ready" with a green dot, else the status's words (or
 *  the firmware-only "Installing firmware…"). */
export function pillOf(status: SystemStatus, text: string | null = null): { pill: string; ok: boolean } {
  return status === 'ready' ? { pill: 'Ready', ok: true } : { pill: text ?? STATUS_LABELS[status], ok: false };
}

/** The line under a system's name (256): its emulator and core, or why its
 *  games can't start yet. */
export function systemSub(row: Pick<SystemRow, 'emulator' | 'recommended'>, core: string | null): string {
  if (row.emulator === '') return missingText(row);
  return core ? `${row.emulator} · ${core}` : row.emulator;
}

/** X on a system row (257): Change emulator, or Install <recommended> for
 *  a system with none. Nothing while its emulator installs. */
export function systemX(row: Pick<SystemRow, 'emulator' | 'recommended'>, status: SystemStatus): string | null {
  if (status === 'installing') return null;
  if (row.emulator === '' && row.recommended !== null) return `Install ${shortName(row.recommended.name)}`;
  return 'Change emulator';
}

export function systemItem(row: SystemRow, status: SystemStatus, statusText: string | null, core: string | null): SystemItem {
  const key = testKeyOf(row.slug || row.label);
  return {
    k: `sy-${key}`,
    testKey: key,
    label: row.label,
    sub: systemSub(row, core),
    status,
    ...pillOf(status, statusText),
    x: systemX(row, status),
  };
}

/** The Windows (Proton) row, last (255; Linux only, as on the desktop). */
export function windowsItem(toolLabel: string | null, status: SystemStatus): SystemItem {
  return {
    k: 'sy-windows',
    testKey: 'windows',
    label: 'Windows',
    sub: toolLabel ?? 'Choose a Proton version',
    status,
    ...pillOf(status),
    x: status === 'installing' ? null : 'Change emulator',
  };
}

/** The pane's rows in order (255): no emulator first, then the rest (each
 *  group alphabetical, as `buildSystemRows` sorts them), then Windows. */
export function orderSystems<T extends { emulator: string }>(rows: readonly T[]): T[] {
  return [...rows.filter((r) => r.emulator === ''), ...rows.filter((r) => r.emulator !== '')];
}

/** Other emulators (258): installed emulators no system uses. */
export type OtherItem = { k: string; testKey: string; name: string; note: string };

export function otherItems(others: readonly EmulatorEntry[], rows: readonly SystemRow[], links: readonly CompanionLink[]): OtherItem[] {
  return others.map((e) => {
    const key = testKeyOf(e.name);
    return {
      k: `sy-other-${key}`,
      testKey: key,
      name: e.name,
      note: otherEmulatorNote(canPlay(e.name, rows), companionParent(e.name, links)),
    };
  });
}

export const OTHER_HEADING = 'Other emulators';
export const OTHER_EMPTY = 'Every installed emulator is used by a system.';

/** "Checking 5 emulators for updates…" (259). */
export function checkingText(count: number): string {
  return `Checking ${count} ${count === 1 ? 'emulator' : 'emulators'} for updates…`;
}

/** The result card's second line: each update found ("PCSX2 2.3.4"). */
export function updatesSub(found: readonly { name: string; version: string }[]): string {
  return found.map((f) => `${shortName(f.name)} ${f.version}`.trim()).join(', ');
}

/** How many systems are installing an emulator for Install recommended:
 *  those it was pressed for (`pressed`, by key: a system can show a
 *  fallback emulator while its own installs) and any with no emulator. */
export function installingCount<T extends { key: string; emulator: string; recommended: SetupOption | null }>(
  rows: readonly T[],
  pressed: readonly string[],
  installing: (row: T) => boolean,
): number {
  return rows.filter((r) => (r.emulator === '' || pressed.includes(r.key)) && installing(r)).length;
}
