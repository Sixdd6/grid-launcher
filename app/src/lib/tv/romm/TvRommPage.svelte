<script lang="ts">
  // Settings › Game stores › RomM (TVRommStorefront README § The RomM page;
  // prototype `rCardPage`): a page of rows, title "RomM" with the mark, the
  // sub line naming the state, a warning or a note, Server, User, Sign-in,
  // Installed RomM games, Edit sign-in…, Show in Library, Open in browser and
  // Disconnect…. The main button (Start) is Reconnect (Can't reach), Sign in
  // again… (expired) or Connect RomM… (not connected); none while connected.
  // Every row follows the RomM status as it changes, focus staying put.
  import { installed } from '../../stores/installed.svelte';
  import { romm } from '../../stores/romm.svelte';
  import { rommInstalledRows } from '../../romm/rail';
  import { useScreen } from '../screen.svelte';
  import TvButton from '../ui/TvButton.svelte';
  import TvPageRows from '../ui/TvPageRows.svelte';
  import { disconnectBox, openRommInBrowser, openSignIn, retryRomm, showRommInLibrary } from './rommActions';
  import { rommPageMain, rommPageRows, rommPageSub } from './rommTv';
  import TvRommMark from './TvRommMark.svelte';

  let status = $derived(romm.status);
  let count = $derived(rommInstalledRows(installed.list).length);
  let sub = $derived(rommPageSub(status));
  let rows = $derived(
    rommPageRows(status, count, {
      edit: () => openSignIn('edit'),
      showInLibrary: showRommInLibrary,
      openInBrowser: openRommInBrowser,
      disconnect: disconnectBox,
    }),
  );
  let main = $derived(rommPageMain(status));

  function press() {
    const m = main;
    if (!m || m.disabled) return;
    if (m.action === 'reconnect') void retryRomm();
    else if (m.action === 'signin') openSignIn('again');
    else if (m.action === 'connect') openSignIn('connect');
  }

  useScreen({
    hints: () => (main && !main.disabled ? [{ button: 'MENU', label: main.hint }] : []),
    main: () => (main ? { disabled: main.disabled, why: main.why, press } : null),
  });
</script>

<div class="page" data-scroll="page" data-testid="tv-romm-page">
  <div class="page-in" data-nav="v" id="rows-page">
    <h1 data-testid="tv-rows-title"><TvRommMark lg /> RomM</h1>
    <p class="sub" data-testid="tv-rows-sub">{sub.before}{#if sub.user}<b>{sub.user}</b>{/if}{sub.after}</p>
    <TvPageRows {rows} />
    {#if main}
      <div class="bk-foot">
        <span class="muted"></span>
        <TvButton k="main" primary glyph={main.disabled ? undefined : 'MENU'} disabled={main.disabled} why={main.why} testId="tv-rows-main" onA={press}>
          {#if main.disabled}<span class="sspin"></span>{/if}{main.label}
        </TvButton>
      </div>
    {/if}
  </div>
</div>
