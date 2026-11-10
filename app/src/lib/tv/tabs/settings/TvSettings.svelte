<script lang="ts">
  // Settings: the panes' rail and the pane beside it. Moving through the
  // rail shows each pane at once (item 105), and only the pane redraws
  // (item 116). P5 builds the TV mode pane's controller line and Switch to
  // desktop mode (item 523); P10 and P11 build the other panes.
  import { leaveTv } from '../../../stores/uiMode.svelte';
  import { TV_PANES, TV_PANE_LABELS, type TvPane } from '../../../uiMode';
  import { controllers } from '../../input/router.svelte';
  import { nav, setPane } from '../../nav.svelte';
  import type { Row } from '../../rows';
  import TvRail from '../../ui/TvRail.svelte';
  import TvRailRow from '../../ui/TvRailRow.svelte';
  import TvRows from '../../ui/TvRows.svelte';

  function rowsFor(pane: TvPane): Row[] {
    if (pane === 'tv') {
      // TODO(P10): Button prompts, Button assignments and Quit GRID (items
      // 261–267), which need P6's lists and confirmation.
      return [
        { type: 'info', key: 'tv-controller', label: 'Controller', value: controllers.name ?? 'None found · the keyboard works too' },
        {
          type: 'action',
          key: 'tv-desktop',
          label: 'Switch to desktop mode',
          sub: 'Also in the Menu. To come back: the TV button in the desktop top bar, or start GRID with --tv',
          run: () => void leaveTv(nav.tab, nav.pane),
        },
      ];
    }
    // TODO(P10/P11/P13): this pane's rows.
    return [];
  }

  let rows = $derived(rowsFor(nav.pane));
</script>

<div class="split" data-testid="tv-settings">
  <TvRail label="Settings">
    {#each TV_PANES as p (p)}
      <TvRailRow k={`r-${p}`} selected={nav.pane === p} testId={`tv-set-rail-${p}`} onSelect={() => setPane(p)}>{TV_PANE_LABELS[p]}</TvRailRow>
    {/each}
  </TvRail>
  <div class="pane" id="pane" data-nav="v" data-enter="first" data-rail-target data-scroll="pane" data-testid="tv-pane">
    <div class="pane-h"><h1>{TV_PANE_LABELS[nav.pane]}</h1></div>
    <TvRows {rows} />
  </div>
</div>
