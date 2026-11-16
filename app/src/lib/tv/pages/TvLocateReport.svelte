<script lang="ts">
  // The Locate report page (TVRommStorefront README, "The Locate report"):
  // "Firmware check", what was checked, then the Installed, Installed · not
  // verified and Refused groups, each counted; the folder's unmatched count;
  // the nothing-installed note; Locate more files… and Done (Start), which
  // goes back to the System page with the files marked.
  import type { Row } from '../rows';
  import { useScreen } from '../screen.svelte';
  import TvButton from '../ui/TvButton.svelte';
  import TvPage from '../ui/TvPage.svelte';
  import TvRows from '../ui/TvRows.svelte';
  import type { TvReport } from './filePicker';

  let { report, onMore, onDone }: { report: TvReport; onMore: () => void; onDone: () => void } = $props();

  function rowsOf(group: TvReport['groups'][number]): Row[] {
    return group.items.map((item) => ({ type: 'info', key: `rep-${item.key}`, label: item.label, sub: item.sub, value: item.pill }));
  }

  useScreen({ main: () => ({ disabled: false, press: onDone }) });
</script>

<div data-testid="tv-locate-report">
  <TvPage title={report.title} sub={report.sub}>
    {#each report.groups as g (g.kind)}
      <div class="sgroup" data-testid={`tv-locate-group-${g.kind}`}>{g.title}</div>
      <TvRows rows={rowsOf(g)} />
    {/each}
    {#if report.unmatched}<p class="note" data-testid="tv-locate-unmatched">{report.unmatched}</p>{/if}
    {#if report.nothing}<p class="note warn" data-testid="tv-locate-nothing">{report.nothing}</p>{/if}
    <div class="btns" data-nav="h">
      <TvButton k="loc-more" testId="tv-locate-more" onA={onMore}>Locate more files…</TvButton>
      <TvButton k="loc-done" primary autofocus testId="tv-locate-done" onA={onDone}>Done</TvButton>
    </div>
  </TvPage>
</div>
