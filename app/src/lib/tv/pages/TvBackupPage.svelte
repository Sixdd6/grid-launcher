<script lang="ts">
  // The Backup page (items 457–482; tvmode.js `openBackup`): the desktop's
  // backup dialog as a pushed page. The plans, picks and Start are the
  // shared `BackupPlanStore`'s (the desktop dialog runs the same one); the
  // rows are `backupPage.ts`'s. Start presses Backup N games from anywhere
  // on the page; a queued backup goes back and ends select mode, and the
  // batch notifications announce it ("3 games added to the backup queue for
  // Couch kit", with Open Downloads).
  import { BackupPlanStore } from '../../backups/backupPlan.svelte';
  import type { BackupDialogOpen } from '../../backups/dialogState.svelte';
  import { drives } from '../../stores/drives.svelte';
  import { popPage, setPane, showTab } from '../nav.svelte';
  import { openList } from '../overlays/overlays.svelte';
  import { selEnd } from '../select.svelte';
  import { backupLede, backupMain, backupPageRows, outdatedWords, type BackupPageModel } from './backupPage';
  import TvRowsPage from './TvRowsPage.svelte';

  let { open }: { open: BackupDialogOpen } = $props();

  // svelte-ignore state_referenced_locally
  const b = new BackupPlanStore(() => open);
  let each = $state(false);

  $effect(() => {
    b.loadSettings();
  });
  $effect(() => {
    b.keepChosen();
  });
  $effect(() => {
    b.planAll();
  });

  let model = $derived<BackupPageModel>({
    title: open.request.title,
    host: b.host,
    ready: b.ready,
    connected: drives.connected,
    plans: b.plans,
    chosen: b.chosen,
    busy: b.busy,
    useLocal: b.useLocal,
    choices: b.choices,
    lookup: (id) => b.localInfo.get(id),
    includeFirmware: b.includeFirmware,
    includeRuntime: b.includeRuntime,
    each,
    error: b.error,
    starting: b.starting,
  });

  async function start() {
    if (await b.start()) {
      selEnd();
      void popPage();
    }
  }

  function choiceFor(romId: number) {
    const o = b.plan?.outdated.find((x) => x.rom_id === romId);
    if (!o) return;
    const w = outdatedWords(o);
    const c = b.choices[romId] ?? o.choice;
    void openList({
      title: o.name,
      sub: w.listSub,
      items: [
        { label: w.local, checked: c === 'local', testId: 'tv-bk-choice-local', onPick: () => b.pick(romId, 'local') },
        { label: w.download, checked: c === 'download', testId: 'tv-bk-choice-download', onPick: () => b.pick(romId, 'download') },
      ],
    });
  }

  let rows = $derived(
    backupPageRows(model, {
      choose: (key) => b.choose(key),
      setUp: () => {
        setPane('backup-drives');
        void showTab('settings', { again: true });
      },
      toggleLocal: () => (b.useLocal = !b.useLocal),
      choiceFor,
      pickAll: (source) => b.pickAll(source),
      toggleEach: () => (each = !each),
      toggleFirmware: () => (b.includeFirmware = !b.includeFirmware),
      toggleRuntime: () => (b.includeRuntime = !b.includeRuntime),
      start: () => void start(),
    }),
  );
  let main = $derived(backupMain(model, () => void start()));
</script>

<TvRowsPage title={open.request.title} sub={backupLede(b.host, b.plan)} {rows} {main} testId="tv-backup-page" />
