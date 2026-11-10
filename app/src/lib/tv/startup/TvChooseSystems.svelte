<script lang="ts">
  // Choose your systems (items 400–408; tvmode.js `choosePage`): the second
  // startup screen, after a first connect. Each system is a row with a
  // switch (off at first), its game count and the emulator it would get; A
  // ticks, X changes the emulator from the system's own list (picking one
  // ticks it). Then Install N emulators (each emulator once) or Skip for
  // now; Start skips from anywhere (ruling 6). Install hands the run to the
  // setup card (TvSetupCard) and GRID opens where it opens. The data and
  // the calls are the desktop's (`setup/chooseSystems.svelte.ts`).
  import { isWindowsHost } from '../../emulators/compatTools';
  import { canStart, primaryLabel } from '../../setup/choices';
  import { ChooseSystems } from '../../setup/chooseSystems.svelte';
  import { finishSetup } from '../../stores/session.svelte';
  import { startSetupRun } from '../../stores/setupRun.svelte';
  import { tick } from 'svelte';
  import { tvf } from '../focus/actions';
  import { focusables } from '../focus/dom';
  import { focused, scope, setFocus } from '../nav.svelte';
  import Glyph from '../glyphs/Glyph.svelte';
  import { openList, type ListItem } from '../overlays/overlays.svelte';
  import { useScreen } from '../screen.svelte';
  import TvButton from '../ui/TvButton.svelte';
  import type { PlatformSetup } from '../../api';
  import {
    CHANGE_EMULATOR,
    CHOOSE_INTRO,
    CHOOSE_TITLE,
    chooseRow,
    emulatorItems,
    emulatorListTitle,
    loadFailed,
    LOADING,
    NO_EMULATOR,
    NO_PROTON,
    NO_SYSTEMS,
    protonItems,
    SKIP_LABEL,
    WINDOWS_LABEL,
    WINDOWS_SUB,
    unmarked,
    type EmulatorItem,
  } from './choose';

  const windowsHost = isWindowsHost(navigator.platform);
  const cs = new ChooseSystems(windowsHost);
  void cs.load();
  void cs.loadWindows();

  let rows = $derived(cs.rows === null ? [] : cs.rows.map((r) => ({ setup: r, view: chooseRow(r, cs.selection) })));
  let installLabel = $derived(primaryLabel(cs.button));
  let installReady = $derived(canStart(cs.button) && !cs.busy);
  let windowsValue = $derived.by(() => {
    const v = cs.proton ?? cs.protonDefault;
    if (v === null) return NO_PROTON;
    const all = [...cs.protonOptions.installed, ...cs.protonOptions.download];
    return unmarked(all.find((o) => o.value === v)?.label ?? v);
  });

  // The systems arrive after the screen: focus starts on the first one
  // (it sat on the buttons, the only items there were), unless the player
  // has moved since.
  let rowsFocused = false;
  $effect(() => {
    if (cs.rows === null || rowsFocused) return;
    rowsFocused = true;
    void tick().then(() => {
      const cur = focused();
      if (cur && !cur.closest('#ch-btns')) return;
      const first = focusables(scope()?.querySelector('#ch-rows') ?? null)[0];
      if (first) setFocus(first, { instant: true });
    });
  });

  function toList(items: EmulatorItem[], pick: (value: string | null) => void): ListItem[] {
    return items.map((it) => ('group' in it ? it : { label: it.label, checked: it.checked, onPick: () => pick(it.value) }));
  }

  function changeEmulator(row: PlatformSetup) {
    void openList({
      title: emulatorListTitle(row.label),
      items: toList(emulatorItems(row, cs.selection), (value) => {
        if (value === null) {
          if (row.label in cs.selection) cs.tick(row);
          return;
        }
        if (!(row.label in cs.selection)) cs.tick(row);
        cs.pick(row, value);
      }),
    });
  }

  function changeProton() {
    void openList({
      title: emulatorListTitle(WINDOWS_LABEL),
      items: toList(protonItems(cs.protonOptions.installed, cs.protonOptions.download, cs.proton ?? cs.protonDefault), (value) => {
        if (value === null) {
          if (cs.proton !== null) cs.tickWindows();
          return;
        }
        cs.pickProton(value);
      }),
    });
  }

  async function install() {
    if (!installReady) return;
    const done = await cs.install();
    if (done === null) return;
    startSetupRun(done.started, done.names);
    finishSetup();
  }

  async function skip() {
    if (cs.busy) return;
    await cs.skip();
    finishSetup();
  }

  useScreen({
    hints: () => [{ button: 'MENU', label: SKIP_LABEL }],
    main: () => ({ disabled: false, press: () => void skip() }),
  });
</script>

<div class="start" data-testid="tv-choose">
  <div class="start-box wide" data-nav="v">
    <h1>{CHOOSE_TITLE}</h1>
    <p class="sub" data-testid="tv-choose-intro">{#each CHOOSE_INTRO as part, i (i)}{#if typeof part === 'string'}{part}{:else}<Glyph button={part.glyph} />{/if}{/each}</p>
    {#if cs.startError}<p class="errline" role="alert" data-testid="tv-choose-error">{cs.startError}</p>{/if}
    <div class="start-body" data-nav="v" id="ch-rows" data-scroll="ch">
      {#if cs.loadError !== null}
        <p class="errline" role="alert">{loadFailed(cs.loadError)}</p>
        <div class="btns" data-nav="h"><TvButton k="ch-retry" onA={() => void cs.load()}>Try again</TvButton></div>
      {:else if cs.rows === null}
        <p class="note">{LOADING}</p>
      {:else}
        {#if rows.length === 0}<p class="note">{NO_SYSTEMS}</p>{/if}
        {#each rows as r (r.view.key)}
          <div
            class="srow"
            data-k={r.view.key}
            data-al={r.view.on ? 'Untick' : 'Tick'}
            data-xl={r.view.disabled ? undefined : CHANGE_EMULATOR}
            data-why={r.view.disabled ? NO_EMULATOR : undefined}
            aria-disabled={r.view.disabled ? 'true' : undefined}
            data-testid={`tv-ch-row-${r.setup.slug || r.setup.label}`}
            use:tvf={{ a: () => cs.tick(r.setup), x: r.view.disabled ? undefined : () => changeEmulator(r.setup) }}
          >
            <span class="sw" class:on={r.view.on}></span>
            <div class="lab"><div class="t">{r.view.label}</div><div class="s">{r.view.sub}</div></div>
            <div class="val" data-testid={`tv-ch-val-${r.setup.slug || r.setup.label}`}>{r.view.value}</div>
          </div>
        {/each}
        {#if !windowsHost}
          {@const blocked = cs.protonDefault === null && cs.proton === null}
          <div
            class="srow"
            data-k="ch-windows"
            data-al={cs.proton !== null ? 'Untick' : 'Tick'}
            data-xl={blocked ? undefined : CHANGE_EMULATOR}
            data-why={blocked ? NO_PROTON : undefined}
            aria-disabled={blocked ? 'true' : undefined}
            data-testid="tv-ch-row-windows"
            use:tvf={{ a: () => cs.tickWindows(), x: blocked ? undefined : changeProton }}
          >
            <span class="sw" class:on={cs.proton !== null}></span>
            <div class="lab"><div class="t">{WINDOWS_LABEL}</div><div class="s">{WINDOWS_SUB}</div></div>
            <div class="val" data-testid="tv-ch-val-windows">{windowsValue}</div>
          </div>
        {/if}
      {/if}
    </div>
    <div class="btns start-foot" data-nav="h" id="ch-btns">
      <TvButton k="ch-go" primary disabled={!installReady} testId="tv-choose-install" onA={() => void install()}>
        {cs.busy ? 'Starting…' : installLabel}
      </TvButton>
      <TvButton k="ch-skip" glyph="MENU" testId="tv-choose-skip" onA={() => void skip()}>{SKIP_LABEL}</TvButton>
    </div>
  </div>
</div>
