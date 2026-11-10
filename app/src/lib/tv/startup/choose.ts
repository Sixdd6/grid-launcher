// TV mode's Choose your systems (checklist items 400–408; README § Startup
// screens › Choose your systems; tvmode.js `choosePage`, `ACT.chEmu`,
// `emuItems`). Pure: each row's words, the X list, and the intro. The
// selection model and the calls are the desktop's (`setup/choices.ts`,
// `setup/chooseSystems.svelte.ts`).
import type { PlatformSetup } from '../../api';
import { choosePicker, optionLabel, rowState, type Selection } from '../../setup/choices';
import type { PickerOption } from '../../systems/rows';
import { gamesText } from '../../systems/rows';
import type { NoticePart } from '../notices';

export const CHOOSE_TITLE = 'Choose your systems';
/** The intro line, with the X and Menu prompts drawn inline (item 400). */
export const CHOOSE_INTRO: readonly NoticePart[] = [
  'Tick the systems you want to play. GRID installs the recommended emulator for each. ',
  { glyph: 'X' },
  ' changes the emulator; ',
  { glyph: 'MENU' },
  ' skips this step. You can change this later in Settings › Systems.',
];
export const SKIP_LABEL = 'Skip for now';
export const CHANGE_EMULATOR = 'Change emulator';
export const WINDOWS_LABEL = 'Windows';
export const WINDOWS_SUB = 'Proton for Windows games';
export const NO_EMULATOR = 'No emulator available';
export const NO_PROTON = 'No Proton version available';
export const LOADING = 'Loading your systems…';
export const NO_SYSTEMS = 'Your server has no systems that need an emulator.';

export function loadFailed(error: string): string {
  return `Your systems could not be loaded: ${error}`;
}

export type ChooseRow = {
  key: string;
  label: string;
  sub: string;
  /** The emulator it would install: the pick once ticked, else the recommended one. */
  value: string;
  on: boolean;
  /** Nothing installable plays it: it can't be ticked. */
  disabled: boolean;
};

/** A row's emulator, without the "(recommended)" mark. */
function valueOf(row: PlatformSetup, sourceId: string | null): string {
  const o = sourceId === null ? undefined : row.options.find((x) => x.source_id === sourceId);
  return o ? optionLabel({ ...o, installed: false }, row.core) : NO_EMULATOR;
}

/** One system's row (item 401): switch, name, "<n> games", its emulator. */
export function chooseRow(row: PlatformSetup, selection: Selection): ChooseRow {
  const on = row.label in selection;
  return {
    key: `ch-${row.slug || row.label}`,
    label: row.label,
    sub: gamesText(row.rom_count),
    value: valueOf(row, on ? selection[row.label] : row.recommended),
    on,
    disabled: rowState(row) === 'unavailable',
  };
}

export type EmulatorItem = { group: string } | { label: string; value: string | null; checked: boolean };

function groupItems(installed: readonly PickerOption[], download: readonly PickerOption[], current: string | null): EmulatorItem[] {
  const items: EmulatorItem[] = [];
  for (const [name, list] of [['Installed', installed], ['Download', download]] as const) {
    if (!list.length) continue;
    items.push({ group: name });
    for (const o of list) items.push({ label: o.label, value: o.value, checked: o.value === current });
  }
  items.push({ group: ' ' });
  items.push({ label: 'None', value: null, checked: current === null });
  return items;
}

/** X's list "Emulator for <system>" (item 403): Installed and Download
 *  groups, recommended first, then None. The row's current emulator is checked. */
export function emulatorItems(row: PlatformSetup, selection: Selection): EmulatorItem[] {
  const groups = choosePicker(row);
  const current = row.label in selection ? selection[row.label] : row.recommended;
  return groupItems(groups.installed, groups.download, current);
}

/** The Windows row's list: the Proton versions, the same groups. */
export function protonItems(installed: readonly PickerOption[], download: readonly PickerOption[], current: string | null): EmulatorItem[] {
  return groupItems(installed, download, current);
}

/** A list label without its recommended mark, for the row's value:
 *  "Install GE-Proton (latest, recommended)" → "Install GE-Proton (latest)". */
export function unmarked(label: string): string {
  return label.replace(/, recommended\)$/, ')').replace(/ \(recommended\)$/, '');
}

export function emulatorListTitle(label: string): string {
  return `Emulator for ${label}`;
}
