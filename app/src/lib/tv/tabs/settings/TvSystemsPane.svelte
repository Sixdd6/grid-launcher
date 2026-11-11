<script lang="ts">
  // Settings › Systems in TV mode (checklist items 252–260; tvmode.js
  // `systemsPane`): the heading with Install recommended (N) (primary; it
  // reads "Installing N emulators…" and can't be pressed again while those
  // installs run, then goes, and focus moves to the control now in its
  // place: the buttons sit in `#sy-head`), Check for updates and Add
  // manually; then one row per system, no emulator first, then the rest,
  // then Windows (Linux only); then Other emulators. A opens a system's
  // page; X is Change emulator, or Install <recommended> for a system with
  // none. The data and commands are the shared SystemsStore's
  // (systemsTv.svelte.ts); only this pane redraws.
  import { isWindowsHost } from '../../../emulators/compatTools';
  import { platformCoreSelect } from '../../../emulators/defaults';
  import { checkable, emulatorUpdates, recheck } from '../../../stores/emulatorUpdates.svelte';
  import { pushToast } from '../../../stores/toasts.svelte';
  import { updatesFoundText } from '../../../systems/systemsStore.svelte';
  import type { SystemRow } from '../../../systems/rows';
  import { tvf } from '../../focus/actions';
  import { pushPage } from '../../nav.svelte';
  import { installAllRecommended, openEmulatorList, openProtonList, tvRec, useTvSystems, windowsNow } from '../../systemsTv.svelte';
  import TvButton from '../../ui/TvButton.svelte';
  import TvIcon from '../../ui/TvIcon.svelte';
  import { openEmulatorForm } from '../../pages/emulatorFormOpen.svelte';
  import TvSystemPage from '../../pages/TvSystemPage.svelte';
  import {
    CHECK_RUNNING,
    checkingText,
    installingCount,
    OTHER_EMPTY,
    OTHER_HEADING,
    otherItems,
    orderSystems,
    REC_RUNNING,
    recButton,
    systemItem,
    updatesSub,
    windowsItem,
    type SystemItem,
  } from './panes/systems';

  const s = useTvSystems();
  const windowsHost = isWindowsHost(typeof navigator === 'undefined' ? '' : navigator.platform);

  let rec = $derived(
    recButton({
      queueing: s.recommendedInstalling,
      todo: s.missing.length,
      inFlight: installingCount(s.rows, tvRec.keys, (r) => s.installingRow(r) || s.busyRows[r.key] === true),
    }),
  );

  type Entry = { item: SystemItem; row: SystemRow | null };

  let entries = $derived.by((): Entry[] => {
    const list: Entry[] = orderSystems(s.rows).map((row) => {
      const st = s.rowStatus(row);
      const core = platformCoreSelect(s.defaults, row.key, row.emulator, s.coreOptions[row.name] ?? []);
      return { item: systemItem(row, st.status, st.text, core.visible && !core.disabled ? core.selected : null), row };
    });
    if (!windowsHost) {
      const w = windowsNow();
      list.push({ item: windowsItem(w.tool, w.status), row: null });
    }
    return list;
  });

  let others = $derived(otherItems(s.others, s.rows, s.companions));

  function openRow(e: Entry) {
    if (e.row === null) void pushPage({ title: 'Windows', component: TvSystemPage, props: { windows: true } });
    else void pushPage({ title: e.row.label, component: TvSystemPage, props: { system: e.row.key } });
  }

  function xRow(e: Entry) {
    if (e.item.x === null) return;
    if (e.row === null) openProtonList();
    else if (e.row.emulator === '' && e.row.recommended !== null) void s.installRecommended(e.row);
    else openEmulatorList(e.row);
  }

  // Check for updates (259): says how many it checks, then the result.
  async function checkUpdates() {
    if (s.checking) return;
    pushToast(checkingText(checkable(s.emulators).length), 'success');
    await recheck(s.emulators);
    const found = s.emulators
      .filter((e) => emulatorUpdates.hasUpdate(e.name))
      .map((e) => ({ name: e.name, version: emulatorUpdates.check(e.name)?.available_display ?? '' }));
    const sub = updatesSub(found);
    pushToast(updatesFoundText(found.length), 'success', undefined, sub ? { sub } : {});
  }
</script>

<!-- An area left by its edges (data-edge): Up from the header buttons stays
     in the pane, never the rail, which would switch the pane. -->
<div data-edge>
<div class="pane-h">
  <h1 data-testid="tv-pane-title">Systems</h1>
  <div class="btns" id="sy-head" data-nav="h">
    {#if rec}
      <TvButton k="sy-rec" primary disabled={rec.disabled} why={rec.disabled ? REC_RUNNING : undefined} testId="tv-sys-rec" onA={() => void installAllRecommended()}>
        {rec.label}
      </TvButton>
    {/if}
    <TvButton k="sy-chk" disabled={s.checking} why={s.checking ? CHECK_RUNNING : undefined} testId="tv-sys-check" onA={() => void checkUpdates()}>Check for updates</TvButton>
    <TvButton k="sy-add" testId="tv-sys-add" onA={() => openEmulatorForm('add')}>Add emulator</TvButton>
  </div>
</div>

{#if s.loadError}<p class="note err" data-testid="tv-sys-error">{s.loadError}</p>{/if}
{#if !s.loaded}
  <p class="note" data-testid="tv-sys-loading">Loading…</p>
{:else}
  <div id="sy-list" data-nav="v" data-testid="tv-sys-list">
    {#each entries as e (e.item.k)}
      <div
        class="srow"
        data-k={e.item.k}
        data-al="Open"
        data-xl={e.item.x ?? undefined}
        data-testid={`tv-sys-row-${e.item.testKey}`}
        use:tvf={{ a: () => openRow(e), x: () => xRow(e) }}
      >
        <div class="lab">
          <div class="t">{e.item.label}</div>
          <div class="s" data-testid={`tv-sys-sub-${e.item.testKey}`}>{e.item.sub}</div>
        </div>
        <div class="val">
          <span class="st" class:ok={e.item.ok} class:warn={!e.item.ok} data-status={e.item.status} data-testid={`tv-sys-status-${e.item.testKey}`}>{e.item.pill}</span>
          <span class="chev"><TvIcon name="chevronRight" px={18} /></span>
        </div>
      </div>
    {/each}
  </div>
  <div class="sgroup">{OTHER_HEADING}</div>
  {#if others.length === 0}
    <p class="note other-empty" data-testid="tv-sys-other-empty">{OTHER_EMPTY}</p>
  {:else}
    <div id="sy-others" data-nav="v" data-testid="tv-sys-others">
      {#each others as o (o.k)}
        <div
          class="srow"
          data-k={o.k}
          data-al="Open"
          data-testid={`tv-sys-other-${o.testKey}`}
          use:tvf={{ a: () => void pushPage({ title: o.name, component: TvSystemPage, props: { emulator: o.name } }) }}
        >
          <div class="lab">
            <div class="t">{o.name}</div>
            <div class="s">{o.note}</div>
          </div>
          <div class="val"><span class="chev"><TvIcon name="chevronRight" px={18} /></span></div>
        </div>
      {/each}
    </div>
  {/if}
{/if}

</div>

<style>
  .other-empty {
    margin-left: 1.2rem;
  }
</style>
