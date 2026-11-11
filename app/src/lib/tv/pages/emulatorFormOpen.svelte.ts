// Opens Add an emulator / <emulator> settings (checklist item
// 337). The form's fields live in a `$state` object handed to the page as a
// prop: the folder browser (Program) is a page over the form, and coming
// back remounts it with the page's props.
import type { EmulatorEntry } from '../../api';
import { emulatorFormValues, SOMETHING_ELSE } from '../../emulators/form';
import { pushPage } from '../nav.svelte';
import { ADD_CRUMB, DEFAULT_ARGS, editTitle, type EmuFormState } from './emulatorForm';
import TvEmulatorForm from './TvEmulatorForm.svelte';

export function openEmulatorForm(mode: 'add' | 'edit', entry: EmulatorEntry | null = null): void {
  const seed = emulatorFormValues(mode === 'edit' ? entry : null);
  const add = mode === 'add';
  const form = $state<EmuFormState>({
    name: seed.name,
    path: seed.path,
    args: add ? DEFAULT_ARGS : seed.args,
    argsDefault: add,
    saveStrategy: seed.saveStrategy,
    savePaths: seed.savePaths,
    statePaths: seed.statePaths,
    ignoreFiles: seed.ignoreFiles,
    ignoreExtensions: seed.ignoreExtensions,
    which: SOMETHING_ELSE,
    whichFilled: null,
  });
  void pushPage({
    title: add || !entry ? ADD_CRUMB : editTitle(entry.name),
    component: TvEmulatorForm,
    props: { mode: add ? 'add' : 'edit', entry: add ? null : entry, form },
  });
}
