<script lang="ts">
  // More systems (TVRommStorefront README, "Settings › Systems without
  // RomM"): the catalog systems with an installable emulator and nothing set
  // up, one page down from the Systems pane because the list is long on a
  // TV. A opens a system's page; X installs the recommended emulator, after
  // which the system moves to Your systems.
  import { tvf } from '../focus/actions';
  import { pushPage } from '../nav.svelte';
  import { useTvSystems } from '../systemsTv.svelte';
  import { MORE_SYSTEMS, moreItem } from '../tabs/settings/panes/systems';
  import TvIcon from '../ui/TvIcon.svelte';
  import TvPage from '../ui/TvPage.svelte';
  import TvSystemPage from './TvSystemPage.svelte';

  const s = useTvSystems();
  let items = $derived(s.groups.more.map((row) => ({ row, item: moreItem(row) })));
</script>

<div data-testid="tv-more-systems">
  <TvPage title={MORE_SYSTEMS.title} sub={MORE_SYSTEMS.sub}>
    {#if items.length === 0}
      <p class="note" data-testid="tv-more-empty">Every system GRID can set up is in Your systems.</p>
    {/if}
    {#each items as { row, item } (item.k)}
      <div
        class="srow"
        data-k={item.k}
        data-al="Open"
        data-xl={item.x ?? undefined}
        data-testid={`tv-more-row-${item.testKey}`}
        use:tvf={{
          a: () => void pushPage({ title: row.label, component: TvSystemPage, props: { system: row.key } }),
          x: () => {
            if (item.x !== null) void s.installRecommended(row);
          },
        }}
      >
        <div class="lab">
          <div class="t">{item.label}</div>
          <div class="s">{item.sub}</div>
        </div>
        <div class="val">
          <span class="st">Not set up</span>
          <span class="chev"><TvIcon name="chevronRight" px={18} /></span>
        </div>
      </div>
    {/each}
  </TvPage>
</div>
