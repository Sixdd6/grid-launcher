// Pure helpers for the manual add/edit emulator form: the five per-emulator
// cloud fields the reference dialog has (parity gap 1) and the Arguments
// label (parity gap 15). No store or API imports, so this stays trivially
// unit-testable — the rule `catalog.ts` and `retroachievements.ts` follow.
import type { EmulatorEntry } from '../api';

/** `EmulatorConfigDialog._save_strategy_values` (dialogs.py:314). */
export const SAVE_STRATEGIES = ['auto', 'single_file', 'folder'] as const;

export type SaveStrategy = (typeof SAVE_STRATEGIES)[number];

/** The Arguments row's label, verbatim from dialogs.py:362. */
export const ARGS_LABEL = 'Arguments (%rom%, %core%, %ps3_launch_target%)';

export type EmulatorFormValues = {
  name: string;
  path: string;
  args: string;
  saveStrategy: SaveStrategy;
  ignoreFiles: string;
  ignoreExtensions: string;
  savePaths: string;
  statePaths: string;
};

/**
 * The frontend half of `normalize_save_strategy`
 * (crates/grid-core/src/autoconfig/entry.rs:87-95): the alias table collapsed
 * to the three values the select offers, so an entry whose stored strategy
 * is an alias (`"single-file"`, written by an older config or by a profile)
 * still selects the right option instead of silently falling back to `auto`.
 */
export function normalizeSaveStrategy(raw: string | null | undefined): SaveStrategy {
  switch ((raw ?? '').trim().toLowerCase()) {
    case 'singlefile':
    case 'single_file':
    case 'single-file':
    case 'single file':
    case 'file':
      return 'single_file';
    case 'folder':
    case 'directory':
    case 'folder_per_game':
    case 'folder-per-game':
      return 'folder';
    default:
      return 'auto';
  }
}

/**
 * `_apply_emulator_values` (dialogs.py:488-519), minus its `%rom%` default
 * for a blank Arguments field: the rewrite opens the add form with an empty
 * Arguments box, which is what both auto-fills gate on and what
 * `e2e/specs/emulators.spec.ts` asserts.
 */
export function emulatorFormValues(entry: EmulatorEntry | null): EmulatorFormValues {
  return {
    name: entry?.name ?? '',
    path: entry?.path ?? '',
    args: entry?.args ?? '',
    saveStrategy: normalizeSaveStrategy(entry?.save_strategy),
    ignoreFiles: entry?.ignore_files ?? '',
    ignoreExtensions: entry?.ignore_extensions ?? '',
    savePaths: entry?.save_paths ?? '',
    statePaths: entry?.state_paths ?? '',
  };
}

/**
 * `entry_payload` (dialogs.py:527-539): the name and the four
 * semicolon-separated lists are trimmed; the strategy is always written
 * (`... or "auto"`, dialogs.py:537) rather than left blank, which is
 * behaviourally identical because `normalize_save_strategy` maps `""` and
 * `"auto"` to the same result. `path` and `args` are passed through exactly
 * as typed — the form has always done that, and the E2E specs set them
 * literally.
 */
export function entryPatch(
  values: EmulatorFormValues,
): Pick<
  EmulatorEntry,
  | 'name'
  | 'path'
  | 'args'
  | 'save_strategy'
  | 'ignore_files'
  | 'ignore_extensions'
  | 'save_paths'
  | 'state_paths'
> {
  return {
    name: values.name.trim(),
    path: values.path,
    args: values.args,
    save_strategy: values.saveStrategy,
    ignore_files: values.ignoreFiles.trim(),
    ignore_extensions: values.ignoreExtensions.trim(),
    save_paths: values.savePaths.trim(),
    state_paths: values.statePaths.trim(),
  };
}

/** `_show_toast` on a new manual entry (emulator_ui_mixin.py:1591), verbatim. */
export function addedEmulatorToast(name: string): string {
  return `Added emulator '${name.trim()}'.`;
}
