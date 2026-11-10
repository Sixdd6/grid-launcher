<script lang="ts">
  // "Computers this pack is for" (items 502–504; tvmode.js `osPage`; ruling
  // 9): Linux and Windows switches, at least one stays on. Adding a computer
  // queues that OS's build of every emulator on the drive (the batch's own
  // card words it); turning one off deletes those builds, so it asks first.
  import type { PackOs } from '../../api';
  import { changeDriveOs } from '../../backups/driveActions';
  import { drives } from '../../stores/drives.svelte';
  import { pushToast } from '../../stores/toasts.svelte';
  import { popPage } from '../nav.svelte';
  import { confirmBox } from '../overlays/overlays.svelte';
  import { flipOs, OS_PAGE_SUB, OS_PAGE_TITLE, osOffConfirm, osPageRows } from './drivePages';
  import TvRowsPage from './TvRowsPage.svelte';

  let { driveKey }: { driveKey: string } = $props();

  let drive = $derived(drives.byKey(driveKey));
  let busy = $state(false);

  let closed = false;
  $effect(() => {
    if (!closed && (drive === null || drive.pack === null)) {
      closed = true;
      void popPage();
    }
  });

  const fail = (err: unknown) => pushToast(err instanceof Error ? err.message : String(err), 'error');

  function apply(next: PackOs[]) {
    const d = drive;
    if (!d) return;
    busy = true;
    changeDriveOs(d, next)
      .catch(fail)
      .finally(() => (busy = false));
  }

  function flip(os: PackOs) {
    const d = drive;
    if (!d?.pack || busy) return;
    const r = flipOs(d.pack.os, os);
    if ('refused' in r) {
      pushToast(r.refused, 'error');
      return;
    }
    if (d.pack.os.includes(os)) void confirmBox({ ...osOffConfirm(os, d.pack.name), danger: true, onOk: () => apply(r.next) });
    else apply(r.next);
  }

  let rows = $derived(drive?.pack ? osPageRows(drive.pack.os, flip) : []);
</script>

<TvRowsPage title={OS_PAGE_TITLE} sub={OS_PAGE_SUB} {rows} testId="tv-drive-os-page" />
