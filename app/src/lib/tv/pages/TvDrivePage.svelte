<script lang="ts">
  // A set-up Backup drive's page (items 499–509; tvmode.js `drivePage`): For
  // Linux and Windows with Change…, the space bar, the pack's facts, Show in
  // Library, Open folder (in desktop mode), Rename… and Remove GRID files…,
  // which asks twice. The drive comes from the shared drives store, so the
  // page follows every change; when the drive goes away the page closes.
  import { gridFilesSize, openDriveFolder, removeGridFiles, renameDrive } from '../../backups/driveActions';
  import { drives } from '../../stores/drives.svelte';
  import { pushToast } from '../../stores/toasts.svelte';
  import { leaveTv } from '../../stores/uiMode.svelte';
  import { nav, popPage, pushPage } from '../nav.svelte';
  import { confirmBox, openKeyboard } from '../overlays/overlays.svelte';
  import { showDriveInLibrary } from '../tabs/library/library.svelte';
  import { drivePageRows, drivePageSub, removeFilesWords, RENAME_TITLE, renamedText } from './drivePages';
  import TvDriveOsPage from './TvDriveOsPage.svelte';
  import TvRowsPage from './TvRowsPage.svelte';

  let { driveKey }: { driveKey: string } = $props();

  let drive = $derived(drives.byKey(driveKey));
  // svelte-ignore state_referenced_locally
  let lastName = $state(drive?.pack?.name ?? '');
  $effect(() => {
    if (drive?.pack) lastName = drive.pack.name;
  });

  // The drive went away, or its GRID files did: nothing here acts on it now.
  let closed = false;
  $effect(() => {
    if (!closed && (drive === null || drive.pack === null)) {
      closed = true;
      void popPage();
    }
  });

  const fail = (err: unknown) => pushToast(err instanceof Error ? err.message : String(err), 'error');

  function rename() {
    const d = drive;
    if (!d?.pack) return;
    void openKeyboard({
      title: RENAME_TITLE,
      value: d.pack.name,
      onDone: (v) => {
        const name = v.trim();
        if (name === '') return;
        renameDrive(d, name).then((changed) => {
          if (changed) pushToast(renamedText(name));
        }, fail);
      },
    });
  }

  async function removeFiles() {
    const d = drive;
    if (!d?.pack) return;
    const bytes = await gridFilesSize(d);
    const w = removeFilesWords(d.pack.name, d.label, bytes);
    void confirmBox({
      ...w.first,
      danger: true,
      onOk: () =>
        void confirmBox({
          ...w.second,
          danger: true,
          onOk: () => {
            closed = true;
            removeGridFiles(d, w.freed).then(() => void popPage(), fail);
          },
        }),
    });
  }

  let rows = $derived(
    drive
      ? drivePageRows(drive, {
          changeOs: () => void pushPage({ title: 'Computers', component: TvDriveOsPage, props: { driveKey } }),
          showInLibrary: () => showDriveInLibrary(driveKey),
          openFolder: () => {
            const d = drive;
            if (!d) return;
            // The file manager needs the desktop (item 506).
            void leaveTv(nav.tab, nav.pane).then(() => openDriveFolder(d));
          },
          rename,
          removeFiles: () => void removeFiles(),
        })
      : [],
  );
</script>

<TvRowsPage title={drive?.pack?.name ?? lastName} sub={drive ? drivePageSub(drive) : undefined} {rows} testId="tv-drive-page" />
