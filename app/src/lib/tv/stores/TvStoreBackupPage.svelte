<script lang="ts">
  // The Backup page for GOG games (TVStores README § GOG games on backup
  // drives; prototype `sBackupPage`): the lede, Drive (radio rows with the
  // free space), What goes on the drive (a row per game with its Installer
  // files or Installed copy tag), the Windows prefix note, skipped and
  // refused games as notes, and the main button (Start): Backup N games, or,
  // disabled, Not enough space / Nothing to backup. No GOG login is needed.
  import { api, type StoreBackupPlan } from '../../api';
  import { STORE_BACKUP_LEDE, storeBackupView, WINDOWS_PREFIX_NOTE } from '../../gamestores/backup';
  import { plural, sizeGb } from '../../gamestores/text';
  import { drives } from '../../stores/drives.svelte';
  import { popPage } from '../nav.svelte';
  import TvRowsPage from '../pages/TvRowsPage.svelte';
  import type { PageMain, PageRow } from '../pages/pageRows';
  import { selEnd } from '../select.svelte';
  import { bumpDrives } from './storeState.svelte';
  import { card, fail, toDownloads } from './storeActions';
  import { driveRadioSub, NO_BACKUP_DRIVE } from './storeWords';

  let { keys, title }: { keys: string[]; title: string } = $props();

  let ready = $derived(drives.ready);
  let chosen = $state<string | null>(null);
  let drive = $derived(ready.find((d) => d.key === chosen) ?? ready[0] ?? null);
  let plan = $state<StoreBackupPlan | null>(null);
  let planError = $state<string | null>(null);
  let starting = $state(false);

  $effect(() => {
    const d = drive;
    const k = keys;
    if (!d) {
      plan = null;
      return;
    }
    let cancelled = false;
    planError = null;
    api
      .planStoreBackup(d.key, k)
      .then((p) => {
        if (!cancelled) plan = p;
      })
      .catch((err) => {
        if (!cancelled) planError = err instanceof Error ? err.message : String(err);
      });
    return () => {
      cancelled = true;
    };
  });

  let pack = $derived(drive?.pack?.name ?? drive?.label ?? '');
  let view = $derived(plan && drive && plan.drive === drive.key ? storeBackupView(plan, pack) : null);

  let rows = $derived.by((): PageRow[] => {
    const R: PageRow[] = [{ kind: 'heading', key: 'h-drive', text: 'Drive' }];
    if (ready.length === 0) R.push({ kind: 'note', key: 'no-drive', text: NO_BACKUP_DRIVE, warn: true });
    for (const d of ready) {
      R.push({
        kind: 'row',
        key: `drv-${d.key}`,
        label: d.pack?.name ?? d.label,
        sub: driveRadioSub(d.label, d.fs, plan && plan.drive === d.key ? plan.free_bytes : d.free_bytes),
        radio: drive?.key === d.key,
        run: () => (chosen = d.key),
      });
    }
    if (planError) {
      R.push({ kind: 'note', key: 'plan-err', text: planError, err: true });
      return R;
    }
    if (!view) return R;
    R.push({ kind: 'heading', key: 'h-what', text: 'What goes on the drive' });
    for (const r of view.rows) R.push({ kind: 'info', key: `g-${r.key}`, label: r.name, sub: r.note, tag: r.tag });
    if (view.windowsNote) R.push({ kind: 'note', key: 'wine', text: WINDOWS_PREFIX_NOTE });
    if (view.skipped) R.push({ kind: 'note', key: 'skipped', text: view.skipped });
    view.refusals.forEach((t, i) => R.push({ kind: 'note', key: `refused-${i}`, text: t, warn: true }));
    return R;
  });

  async function start() {
    if (!drive || !view || !view.canStart || starting || !plan) return;
    starting = true;
    const d = drive;
    const n = view.rows.length;
    try {
      await api.startStoreBackup(d.key, view.rows.map((r) => r.key));
    } catch (err) {
      starting = false;
      fail(err);
      return;
    }
    selEnd();
    bumpDrives();
    await popPage();
    card(`${plural(n, 'game')} added to the backup queue for ${pack}`, 'success', undefined, toDownloads);
  }

  let main = $derived.by((): PageMain | null => {
    if (!drive) return { label: 'Nothing to backup', summary: '', disabled: true, why: NO_BACKUP_DRIVE, hint: 'Backup', press: () => {} };
    if (!view || !plan) return null;
    return {
      label: view.button,
      summary: view.rows.length ? view.footNote : '',
      disabled: !view.canStart || starting,
      why: view.over ? `Needs ${sizeGb(plan.bytes_to_write)} · ${sizeGb(plan.free_bytes)} free` : 'Nothing new to write',
      hint: 'Backup',
      press: () => void start(),
    };
  });
</script>

<TvRowsPage {title} sub={STORE_BACKUP_LEDE} {rows} {main} testId="tv-store-backup" />
