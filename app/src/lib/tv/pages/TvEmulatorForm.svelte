<script lang="ts">
  // Add an emulator manually / <emulator> settings (checklist items 337–341;
  // tvmode.js `emuForm`): "Which emulator is it?" (add only: the emulators
  // GRID knows, or Something else), Name (keyboard), Program (the folder
  // browser in file mode; a known program fills in the name and launch
  // options while both are still blank), Launch options (keyboard, "%rom%"
  // to start), Cloud saves (advanced) with the save strategy list and the
  // desktop sheet's save and state folders and files not to upload, then
  // Save / Add emulator, which goes back. The save is the desktop's
  // (formSave.ts): an edit keeps the fields the form doesn't show, and a
  // manual add never sets a system's default.
  import type { EmulatorEntry } from '../../api';
  import { applyWhichEmulator, SOMETHING_ELSE } from '../../emulators/form';
  import { profileFill, saveEmulatorForm } from '../../emulators/formSave';
  import { pushToast } from '../../stores/toasts.svelte';
  import { openFolderBrowser } from '../gameActions';
  import { popPage } from '../nav.svelte';
  import { openKeyboard, openList } from '../overlays/overlays.svelte';
  import { useTvSystems } from '../systemsTv.svelte';
  import TvPage from '../ui/TvPage.svelte';
  import TvRows from '../ui/TvRows.svelte';
  import {
    ADD_SUB,
    ADD_TITLE,
    CLOUD_FIELDS,
    DEFAULT_ARGS,
    EDIT_SUB,
    editTitle,
    emulatorFormRows,
    PROGRAM_BROWSER_TITLE,
    programStart,
    savedNotice,
    STRATEGY_LIST_TITLE,
    STRATEGY_OPTIONS,
    typedArgs,
    type EmuFormState,
  } from './emulatorForm';

  let { mode, entry = null, form }: { mode: 'add' | 'edit'; entry?: EmulatorEntry | null; form: EmuFormState } = $props();

  const s = useTvSystems();
  let busy = $state(false);

  const errText = (err: unknown) => (err instanceof Error ? err.message : String(err));

  function chooseWhich() {
    const names = s.profiles.map((p) => p.name);
    void openList({
      title: 'Which emulator is it?',
      items: [...names, SOMETHING_ELSE].map((n, i) => ({
        label: n === SOMETHING_ELSE ? 'Something else' : n,
        checked: form.which === n,
        testId: `tv-which-${i}`,
        onPick: () => {
          form.which = n;
          const profile = s.profiles.find((p) => p.name === n) ?? null;
          const next = applyWhichEmulator(profile, { name: form.name, args: typedArgs(form) }, form.whichFilled);
          form.name = next.name;
          if (next.args !== typedArgs(form)) {
            // Cleared back to nothing: the add's "%rom%" again.
            form.argsDefault = next.args === '';
            form.args = next.args === '' ? DEFAULT_ARGS : next.args;
          }
          form.whichFilled = profile === null ? null : next;
        },
      })),
    });
  }

  function editName() {
    void openKeyboard({ title: 'Name', value: form.name, onDone: (v) => (form.name = v) });
  }

  // A picked program a known emulator profile matches fills in the name and
  // the launch options, while both are still blank (formSave.ts).
  async function autofill() {
    const fill = await profileFill(form.path, { name: form.name, args: typedArgs(form) });
    if (!fill) return;
    form.name = fill.name;
    form.args = fill.args;
    form.argsDefault = false;
  }

  function chooseProgram() {
    openFolderBrowser({
      title: PROGRAM_BROWSER_TITLE,
      start: programStart(form.path),
      files: true,
      onPick: (path) => {
        form.path = path;
        void autofill();
      },
    });
  }

  function editArgs() {
    void openKeyboard({
      title: 'Launch options',
      value: form.args,
      onDone: (v) => {
        form.args = v;
        form.argsDefault = false;
      },
    });
  }

  function chooseStrategy() {
    void openList({
      title: STRATEGY_LIST_TITLE,
      items: STRATEGY_OPTIONS.map(([v, label]) => ({
        label,
        checked: form.saveStrategy === v,
        testId: `tv-strategy-${v}`,
        onPick: () => (form.saveStrategy = v),
      })),
    });
  }

  function editText(field: (typeof CLOUD_FIELDS)[number][0]) {
    const label = CLOUD_FIELDS.find((c) => c[0] === field)?.[2] ?? '';
    void openKeyboard({ title: label, value: form[field], onDone: (v) => (form[field] = v) });
  }

  async function save() {
    if (busy || form.name.trim() === '') return;
    busy = true;
    try {
      await saveEmulatorForm(mode, entry, {
        name: form.name,
        path: form.path,
        args: form.args,
        saveStrategy: form.saveStrategy,
        ignoreFiles: form.ignoreFiles,
        ignoreExtensions: form.ignoreExtensions,
        savePaths: form.savePaths,
        statePaths: form.statePaths,
      });
      if (mode === 'edit') pushToast(savedNotice(form.name.trim()), 'success');
      void s.refreshAll();
      void popPage();
    } catch (err) {
      pushToast(errText(err), 'error');
    } finally {
      busy = false;
    }
  }

  let rows = $derived(
    emulatorFormRows(
      mode,
      form,
      { which: chooseWhich, name: editName, program: chooseProgram, args: editArgs, strategy: chooseStrategy, text: editText, save: () => void save() },
      busy,
    ),
  );
</script>

<div data-testid="tv-emu-form">
  <TvPage title={mode === 'edit' && entry ? editTitle(entry.name) : ADD_TITLE} sub={mode === 'edit' ? EDIT_SUB : ADD_SUB}>
    <TvRows {rows} />
  </TvPage>
</div>
