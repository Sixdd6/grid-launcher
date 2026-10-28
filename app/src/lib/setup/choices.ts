// The first-run platform picker's selection model (`PlatformSetup.svelte`),
// kept pure so it can be tested without mounting the screen.
import type { PlatformSetup, SetupChoice, SetupOption } from '../api';

/** Ticked platforms: display label → the chosen option's `source_id`. */
export type Selection = Readonly<Record<string, string>>;

/** Alphabetical by the shown name, ignoring case — the order every platform
 *  list uses (the backend's `list_platforms` sorts the same way). */
export function sortRows(rows: readonly PlatformSetup[]): PlatformSetup[] {
  return [...rows].sort((a, b) => a.label.localeCompare(b.label, undefined, { sensitivity: 'base' }));
}

/**
 * - `unavailable`: nothing installable serves the platform; it cannot be ticked.
 * - `installed`: its recommended emulator is already configured.
 * - `available`: everything else.
 */
export function rowState(row: PlatformSetup): 'available' | 'installed' | 'unavailable' {
  if (row.recommended === null) return 'unavailable';
  const recommended = row.options.find((o) => o.source_id === row.recommended);
  return recommended?.installed ? 'installed' : 'available';
}

/** Ticks a platform with its recommended emulator, or unticks it. An
 *  unavailable platform cannot be ticked. */
export function toggle(selection: Selection, row: PlatformSetup): Selection {
  if (row.label in selection) {
    const { [row.label]: _removed, ...rest } = selection;
    return rest;
  }
  if (row.recommended === null) return selection;
  return { ...selection, [row.label]: row.recommended };
}

/** Changes a ticked platform's emulator. Only an option the row offers. */
export function pick(selection: Selection, row: PlatformSetup, sourceId: string): Selection {
  if (!(row.label in selection) || !row.options.some((o) => o.source_id === sourceId)) return selection;
  return { ...selection, [row.label]: sourceId };
}

/** What `startPlatformSetup` is sent. */
export function choices(selection: Selection): SetupChoice[] {
  return Object.entries(selection).map(([platform, source_id]) => ({ platform, source_id }));
}

/** The distinct emulators the selection would download: one per emulator,
 *  however many platforms share it, and none already configured. */
export function toInstall(rows: readonly PlatformSetup[], selection: Selection): SetupOption[] {
  const out = new Map<string, SetupOption>();
  for (const row of rows) {
    const id = selection[row.label];
    if (id === undefined) continue;
    const option = row.options.find((o) => o.source_id === id);
    if (option && !option.installed) out.set(option.source_id, option);
  }
  return [...out.values()];
}

/** The primary button: "Install 3 emulators", "Install 1 emulator", "Use
 *  selected emulators" when everything ticked is already installed, and
 *  "Install emulators" (disabled) while nothing is ticked. */
export function primaryLabel(installCount: number, tickedCount: number): string {
  if (tickedCount === 0) return 'Install emulators';
  if (installCount === 0) return 'Use selected emulators';
  return `Install ${installCount} emulator${installCount === 1 ? '' : 's'}`;
}

/** An option's line in the select. RetroArch names the core it will use. */
export function optionLabel(option: SetupOption, core: string | null): string {
  const name = option.name.replace(/\s*\(.*\)\s*$/, '');
  const base = option.kind === 'retroarch' && core ? `${name} · ${core} core` : name;
  return option.installed ? `${base} (installed)` : base;
}
