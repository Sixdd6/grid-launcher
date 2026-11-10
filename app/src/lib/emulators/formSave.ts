// The emulator settings sheet's effects: the profile autofill from a picked
// program and the add / save of the entry. The desktop sheet
// (EmulatorForm.svelte) and the TV emulator form both call these and keep
// their own field state.
import { api, type EmulatorEntry } from '../api';
import { pushToast } from '../stores/toasts.svelte';
import { addedEmulatorToast, entryPatch, normalizeSaveStrategy, type EmulatorFormValues } from './form';

/**
 * The name and launch options a known emulator profile gives `path`, when
 * the form has neither typed yet. `null` when the form already has one,
 * the path is blank, no profile matches, or the lookup fails (autofill is
 * best effort only).
 */
export async function profileFill(
  path: string,
  current: { name: string; args: string },
): Promise<{ name: string; args: string } | null> {
  if (current.name.trim() !== '' || current.args.trim() !== '') return null;
  const trimmed = path.trim();
  if (!trimmed) return null;
  try {
    const profile = await api.matchProfile(trimmed);
    return profile ? { name: profile.name, args: profile.args } : null;
  } catch {
    return null;
  }
}

/** The sheet's fields as typed; the save strategy is normalised on save. */
export type FormFields = Omit<EmulatorFormValues, 'saveStrategy'> & { saveStrategy: string };

/**
 * Adds a new entry (`mode` add) or saves `entry` under the form's values.
 * The fields the form does not show (install provenance, autoconfig paths)
 * are spread back from `entry` untouched. Only a new manual entry gets a
 * card; an edit stays silent. Throws the backend's refusal for the form.
 */
export async function saveEmulatorForm(
  mode: 'add' | 'edit',
  entry: EmulatorEntry | null,
  values: FormFields,
): Promise<void> {
  // `originalName` is what `save_emulator` uses to find-and-replace a
  // renamed entry; blank means "insert".
  const originalName = mode === 'add' ? '' : (entry?.name ?? '');
  const patch = entryPatch({ ...values, saveStrategy: normalizeSaveStrategy(values.saveStrategy) });
  const next: EmulatorEntry = {
    ...(mode === 'edit' && entry ? entry : {}),
    ...patch,
  };
  await api.saveEmulator(originalName, next);
  if (mode === 'add') pushToast(addedEmulatorToast(patch.name));
}
