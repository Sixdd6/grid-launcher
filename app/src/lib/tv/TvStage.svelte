<script lang="ts">
  // The stage between the bars: the tab on show, or the top page over it
  // (item 319). One screen is mounted at a time; coming back to one restores
  // its scroll positions and focus (nav.svelte.ts `showScreen`).
  import ScreenFrame from './ScreenFrame.svelte';
  import { nav } from './nav.svelte';
  import TvLibrary from './tabs/library/TvLibrary.svelte';
  import TvServer from './tabs/server/TvServer.svelte';
  import TvDownloads from './tabs/downloads/TvDownloads.svelte';
  import TvSettings from './tabs/settings/TvSettings.svelte';

  const TABS = { library: TvLibrary, server: TvServer, downloads: TvDownloads, settings: TvSettings };
  let page = $derived(nav.topPage);
  let Tab = $derived(TABS[nav.tab]);
</script>

{#if page}
  {#key page.id}
    <ScreenFrame screen={`page:${page.id}`}>
      <page.component {...page.props ?? {}} />
    </ScreenFrame>
  {/key}
{:else}
  {#key nav.tab}
    <ScreenFrame screen={`tab:${nav.tab}`}>
      <Tab />
    </ScreenFrame>
  {/key}
{/if}
