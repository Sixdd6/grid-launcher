<script lang="ts">
  // Settings › Backup drives in TV mode (items 495–498, 499, 510, 514;
  // tvmode.js `drivesPane`, `drivePage`): Connected drives, then the
  // remembered ones, then Every backup also copies › Firmware. A set-up drive
  // opens its page, one not set up opens Set up for GRID, a remembered one
  // offers Forget. Drives come and go with the shared drives store, and only
  // this pane redraws (a row that leaves gives focus to the one in its place:
  // the rows sit in `#dr-list`).
  import { api } from '../../../api';
  import { forgetDrive } from '../../../backups/driveActions';
  import { drives } from '../../../stores/drives.svelte';
  import { pushToast } from '../../../stores/toasts.svelte';
  import { tvf } from '../../focus/actions';
  import { pushPage } from '../../nav.svelte';
  import { openList } from '../../overlays/overlays.svelte';
  import TvDrivePage from '../../pages/TvDrivePage.svelte';
  import TvDriveSetupPage from '../../pages/TvDriveSetupPage.svelte';
  import TvIcon from '../../ui/TvIcon.svelte';
  import TvSwitch from '../../ui/TvSwitch.svelte';
  import {
    CONNECTED_HEADING,
    DEFAULTS_HEADING,
    drivesPane,
    FIRMWARE_LABEL,
    FIRMWARE_SUB,
    forgetList,
    NO_DRIVES,
    REMEMBERED_HEADING,
    type DriveRowModel,
  } from './panes/drives';

  let firmware = $state(true);
  $effect(() => {
    api
      .getBackupSettings()
      .then((s) => (firmware = s.include_firmware))
      .catch(() => {});
  });

  let model = $derived(drivesPane(drives.connected, drives.remembered, new Date()));

  function setFirmware() {
    const next = !firmware;
    firmware = next;
    api.setBackupIncludeFirmware(next).catch((err: unknown) => {
      firmware = !next;
      pushToast(err instanceof Error ? err.message : String(err), 'error');
    });
  }

  function open(row: DriveRowModel) {
    if (row.opens === 'remembered') {
      const r = drives.remembered.find((x) => `dr-${x.mount}` === row.key);
      if (!r) return;
      const words = forgetList(r, new Date());
      void openList({
        title: words.title,
        sub: words.sub,
        items: [
          {
            label: words.forget,
            danger: true,
            testId: 'tv-drive-forget',
            onPick: () =>
              void forgetDrive(r).then(
                () => pushToast(words.done),
                (err: unknown) => pushToast(err instanceof Error ? err.message : String(err), 'error'),
              ),
          },
        ],
      });
      return;
    }
    const d = drives.connected.find((x) => `dr-${x.mount}` === row.key);
    if (!d) return;
    if (row.opens === 'page') void pushPage({ title: row.title, component: TvDrivePage, props: { driveKey: d.key } });
    else if (row.opens === 'setup') void pushPage({ title: `Set up ${d.label}`, component: TvDriveSetupPage, props: { driveKey: d.key } });
  }
</script>

{#snippet driveRow(r: DriveRowModel)}
  <div
    class="srow"
    data-k={r.key}
    data-al="Open"
    data-why={r.opens === 'none' ? r.why : undefined}
    aria-disabled={r.opens === 'none' ? 'true' : undefined}
    data-testid="tv-drive-row"
    data-drive-key={r.key.slice(3)}
    use:tvf={{ a: () => open(r) }}
  >
    <div class="lab"><div class="t">{r.title}</div><div class="s" data-testid="tv-drive-sub">{r.sub}</div></div>
    <div class="val">
      <span class="st" class:ok={r.pill.tone === 'ok'} class:warn={r.pill.tone === 'warn'} data-testid="tv-drive-status">{r.pill.text}</span>
      <span class="chev"><TvIcon name="chevronRight" px={18} /></span>
    </div>
  </div>
{/snippet}

<div class="pane-h"><h1 data-testid="tv-pane-title">Backup drives</h1></div>
<div id="dr-list" data-nav="v">
  <div class="sgroup">{CONNECTED_HEADING}</div>
  {#each model.connected as r (r.key)}
    {@render driveRow(r)}
  {:else}
    <p class="note" style="margin-left:1.2rem" data-testid="tv-drives-none">{NO_DRIVES}</p>
  {/each}
  {#if model.remembered.length}
    <div class="sgroup">{REMEMBERED_HEADING}</div>
    {#each model.remembered as r (r.key)}
      {@render driveRow(r)}
    {/each}
  {/if}
</div>
<div class="sgroup">{DEFAULTS_HEADING}</div>
<div class="srow" data-k="dr-fw" data-al="Switch" data-testid="tv-drives-firmware" use:tvf={{ a: setFirmware }}>
  <div class="lab"><div class="t">{FIRMWARE_LABEL}</div><div class="s">{FIRMWARE_SUB}</div></div>
  <div class="val"><TvSwitch on={firmware} /></div>
</div>
