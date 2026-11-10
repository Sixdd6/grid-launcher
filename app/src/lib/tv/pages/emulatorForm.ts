// Add an emulator manually / <emulator> settings in TV mode (checklist
// items 337–341; tvmode.js `emuForm`; README § Pages): the desktop's
// settings sheet (emulators/form.ts, formSave.ts) as rows. Pure: the form's
// fields and its rows; TvEmulatorForm opens the lists, the keyboard and the
// folder browser, and saves.
import { SAVE_STRATEGIES, SOMETHING_ELSE, type SaveStrategy } from '../../emulators/form';
import type { Row } from '../rows';

/** The form's fields. Kept in the page's props, so the folder browser
 *  (a page over it, which remounts it) doesn't lose what was typed. */
export type EmuFormState = {
  name: string;
  path: string;
  args: string;
  /** The launch options are still the "%rom%" an add starts with. */
  argsDefault: boolean;
  saveStrategy: SaveStrategy;
  savePaths: string;
  statePaths: string;
  ignoreFiles: string;
  ignoreExtensions: string;
  /** "Which emulator is it?": a profile name, or SOMETHING_ELSE. */
  which: string;
  /** What the last "Which emulator" pick filled in (form.ts `applyWhichEmulator`). */
  whichFilled: { name: string; args: string } | null;
};

export const DEFAULT_ARGS = '%rom%';
export const ADD_TITLE = 'Add an emulator manually';
/** The breadcrumb's words for the add page. */
export const ADD_CRUMB = 'Add an emulator';
export const ADD_SUB = "For an emulator GRID doesn't install. It never becomes a system's default by itself.";
export const EDIT_SUB = 'Program, launch options, cloud save folders';
export const PROGRAM_BROWSER_TITLE = 'Choose the program';
export const STRATEGY_LIST_TITLE = 'How does it store saves?';
/** A on Save with no name. TODO(design): not in the design. */
export const NAME_NEEDED = 'Give the emulator a name first.';

export function editTitle(name: string): string {
  return `${name} settings`;
}

/** The edit form's card (341). An add gets the desktop's own card
 *  ("Added emulator '<name>'."), from formSave.ts. */
export function savedNotice(name: string): string {
  return `${name} settings saved`;
}

/** The TV list's words for the three strategies (340). */
export const STRATEGY_WORDS: Record<SaveStrategy, string> = {
  auto: 'Automatic',
  single_file: 'One file per game',
  folder: 'A folder per game',
};

export const STRATEGY_OPTIONS: readonly (readonly [SaveStrategy, string])[] = SAVE_STRATEGIES.map((s) => [s, STRATEGY_WORDS[s]] as const);

/** The program's file name (339). */
export function fileName(path: string): string {
  const trimmed = path.trim().replace(/[\\/]+$/, '');
  const cut = Math.max(trimmed.lastIndexOf('/'), trimmed.lastIndexOf('\\'));
  return cut >= 0 ? trimmed.slice(cut + 1) : trimmed;
}

/** Where the program browser opens: the program's folder, else home. */
export function programStart(path: string): string {
  const trimmed = path.trim();
  const cut = Math.max(trimmed.lastIndexOf('/'), trimmed.lastIndexOf('\\'));
  return cut > 0 ? trimmed.slice(0, cut) : '';
}

/** The launch options the profile autofill and "Which emulator" see:
 *  blank while they are still the untouched default. */
export function typedArgs(f: Pick<EmuFormState, 'args' | 'argsDefault'>): string {
  return f.argsDefault && f.args === DEFAULT_ARGS ? '' : f.args;
}

export function whichLabel(which: string): string {
  return which === SOMETHING_ELSE ? 'Something else' : which;
}

export type EmuFormActions = {
  which: () => void;
  name: () => void;
  program: () => void;
  args: () => void;
  strategy: () => void;
  text: (field: 'savePaths' | 'statePaths' | 'ignoreFiles' | 'ignoreExtensions') => void;
  save: () => void;
};

/** The extra cloud fields the desktop sheet keeps under "Cloud saves
 *  (advanced)" (SPEC.md › Settings › Systems), each a keyboard row. */
export const CLOUD_FIELDS = [
  ['savePaths', 'save-paths', 'Save folders', 'Separate folders with ;'],
  ['statePaths', 'state-paths', 'Save state folders', ''],
  ['ignoreFiles', 'ignore-files', "Don't upload these files", ''],
  ['ignoreExtensions', 'ignore-ext', "Don't upload these file types", ''],
] as const;

/** The form's rows (338–341): which emulator (add only), Name, Program,
 *  Launch options, Cloud saves (advanced) and its fields, then Save or Add
 *  emulator. */
export function emulatorFormRows(mode: 'add' | 'edit', f: EmuFormState, a: EmuFormActions, busy = false): Row[] {
  const rows: Row[] = [];
  if (mode === 'add') rows.push({ type: 'action', key: 'which', label: 'Which emulator is it?', value: whichLabel(f.which), run: a.which });
  rows.push({ type: 'action', key: 'name', label: 'Name', value: f.name.trim() || 'Not set', run: a.name });
  rows.push({
    type: 'action',
    key: 'prog',
    label: 'Program',
    value: fileName(f.path) || 'Not set',
    sub: f.path.trim() || "Browse for the emulator's program",
    run: a.program,
  });
  rows.push({ type: 'action', key: 'args', label: 'Launch options', value: f.args, sub: '%rom% is replaced by the game file', run: a.args });
  rows.push({
    type: 'action',
    key: 'strategy',
    label: 'Cloud saves (advanced)',
    value: `Save strategy: ${STRATEGY_WORDS[f.saveStrategy]}`,
    sub: 'Save and state folders, files not to upload',
    run: a.strategy,
  });
  for (const [field, key, label, sub] of CLOUD_FIELDS) {
    rows.push({ type: 'text', key, label, ...(sub ? { sub } : {}), value: f[field], edit: () => a.text(field) });
  }
  const unnamed = f.name.trim() === '';
  rows.push({
    type: 'action',
    key: 'save',
    label: mode === 'edit' ? 'Save' : 'Add emulator',
    ...(unnamed ? { disabled: true, why: NAME_NEEDED } : busy ? { disabled: true, why: 'Saving…' } : {}),
    run: a.save,
  });
  return rows;
}
