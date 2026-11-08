// First-run setup's Choose your systems (`ChooseSystems.svelte`): the
// selection model, kept pure so it can be tested without mounting the
// screen. The rows are Settings › Systems' rows (`systems/SystemRow.svelte`
// in tick mode); the Windows row's choices are `systems/proton.ts`'s.
import type { PlatformSetup, SetupChoice, SetupCompatChoice, SetupOption } from '../api';
import type { PickerOption } from '../systems/rows';
import { protonInstallSource, type ProtonPicker } from '../systems/proton';

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

/** Everything the main button installs: the distinct emulators plus a
 *  Proton download (`proton` is the Windows row's value, `null` unticked). */
export function installCount(rows: readonly PlatformSetup[], selection: Selection, proton: string | null): number {
  const protonDownload = proton !== null && protonInstallSource(proton) !== null ? 1 : 0;
  return toInstall(rows, selection).length + protonDownload;
}

export type ButtonInput = {
  /** [`installCount`]. */
  installs: number;
  /** Ticked systems, Windows not included. */
  ticked: number;
  /** The Windows row's value, `null` while unticked. */
  proton: string | null;
};

/** Whether the main button does anything: a system or Windows is ticked. */
export function canStart(input: ButtonInput): boolean {
  return input.ticked > 0 || (input.proton !== null && input.proton !== '');
}

/** The main button: "Install 3 emulators" (a Proton download counts as
 *  one), "Continue" when only an installed Proton version was picked, "Use
 *  selected emulators" when everything ticked is already installed, and
 *  "Install emulators" (disabled) while nothing is ticked. */
export function primaryLabel(input: ButtonInput): string {
  if (input.installs > 0) return `Install ${input.installs} emulator${input.installs === 1 ? '' : 's'}`;
  if (!canStart(input)) return 'Install emulators';
  if (input.ticked === 0) return 'Continue';
  return 'Use selected emulators';
}

const REC = ' (recommended)';

/** A ticked system's picker, grouped like Settings › Systems': Installed
 *  (configured already) and Download, each recommended first. Values are
 *  the options' `source_id`s, what [`pick`] takes. */
export function choosePicker(row: PlatformSetup): { installed: PickerOption[]; download: PickerOption[] } {
  const ordered = [...row.options].sort(
    (a, b) => Number(b.source_id === row.recommended) - Number(a.source_id === row.recommended),
  );
  const line = (o: SetupOption): PickerOption => {
    const recommended = o.source_id === row.recommended;
    const label = optionLabel({ ...o, installed: false }, row.core);
    return { value: o.source_id, label: recommended ? label + REC : label, recommended };
  };
  return {
    installed: ordered.filter((o) => o.installed).map(line),
    download: ordered.filter((o) => !o.installed).map(line),
  };
}

/** The Windows row's value once ticked: the recommended download (GE-Proton)
 *  when GRID can fetch it, else the current default, else the first
 *  installed tool, else the first download. `null` when nothing is offered:
 *  the row can't be ticked. */
export function protonPreselect(picker: ProtonPicker): string | null {
  const recommended = picker.download.find((o) => o.recommended);
  if (recommended) return recommended.value;
  if (!picker.unset) return picker.selected;
  return picker.installed[0]?.value ?? picker.download[0]?.value ?? null;
}

/** What `startPlatformSetup` is sent for the Windows row. */
export function compatChoice(proton: string | null): SetupCompatChoice | null {
  if (proton === null || proton === '') return null;
  const source = protonInstallSource(proton);
  return source === null ? { kind: 'installed', path: proton } : { kind: 'download', source_id: source };
}

/** An option's line in the select. RetroArch names the core it will use; a
 *  server build gives its version and says where it comes from. */
export function optionLabel(option: SetupOption, core: string | null): string {
  const name = option.name.replace(/\s*\(.*\)\s*$/, '');
  const versioned = option.server ? `${name} ${option.server.version}` : name;
  const withCore = option.needs_core && core ? `${versioned} · ${core} core` : versioned;
  const base = option.server ? `${withCore} · from your server` : withCore;
  return option.installed ? `${base} (installed)` : base;
}
