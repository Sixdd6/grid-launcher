<script lang="ts">
  // Set up for GRID (items 510–513; tvmode.js `setupPage`; ruling 14): the
  // pack name (the keyboard; the volume label by default), the two computers
  // with this computer's OS ticked, the FAT32 warning and the note that only
  // a gridpack folder is made. Start presses Set up for GRID; the page goes
  // back once the drive is ready.
  import type { PackOs } from '../../api';
  import { defaultPackName, hostPackOs } from '../../backups/drives';
  import { setUpDrive } from '../../backups/driveActions';
  import { drives } from '../../stores/drives.svelte';
  import { pushToast } from '../../stores/toasts.svelte';
  import { popPage } from '../nav.svelte';
  import { openKeyboard } from '../overlays/overlays.svelte';
  import { AT_LEAST_ONE, flipOs, RENAME_TITLE, setupMain, setupRows, setupStart, setupSub, setupTitle, type SetupForm } from './drivePages';
  import TvRowsPage from './TvRowsPage.svelte';

  let { driveKey }: { driveKey: string } = $props();

  let drive = $derived(drives.byKey(driveKey));
  // svelte-ignore state_referenced_locally
  const first = drive;
  let form = $state<SetupForm>(
    first ? setupStart(first, hostPackOs(navigator.platform), defaultPackName(first)) : { name: '', os: ['linux'] },
  );
  let busy = $state(false);

  let closed = false;
  $effect(() => {
    if (!closed && drive === null) {
      closed = true;
      void popPage();
    }
  });

  function editName() {
    void openKeyboard({
      title: RENAME_TITLE,
      value: form.name,
      onDone: (v) => {
        if (v.trim() !== '') form.name = v.trim();
      },
    });
  }

  function flip(os: PackOs) {
    const r = flipOs(form.os, os);
    if ('refused' in r) {
      pushToast(AT_LEAST_ONE, 'error');
      return;
    }
    form.os = r.next;
  }

  async function setUp() {
    const d = drive;
    if (!d || busy) return;
    busy = true;
    try {
      await setUpDrive(d, form.name, [...form.os]);
      closed = true;
      void popPage();
    } catch (err) {
      pushToast(err instanceof Error ? err.message : String(err), 'error');
    } finally {
      busy = false;
    }
  }

  let rows = $derived(drive ? setupRows(drive, form, { editName, flip }) : []);
  let main = $derived(setupMain(form, busy, () => void setUp()));
</script>

<TvRowsPage title={setupTitle(drive?.label ?? first?.label ?? '')} sub={drive ? setupSub(drive) : undefined} {rows} {main} testId="tv-drive-setup-page" />
