<script lang="ts">
  // The top bar (items 119–126; tvmode.js `paintTop`): the GRID icon,
  // wordmark and TV tag; the tabs between their LB / RB prompts, or on a page
  // the breadcrumb, or on a startup screen nothing; then the live download,
  // the connection and the clock.
  import TvIcon from './ui/TvIcon.svelte';
  import { downloads } from '../stores/downloads.svelte';
  import { session } from '../stores/session.svelte';
  import { clock } from './clock.svelte';
  import Glyph from './glyphs/Glyph.svelte';
  import { TAB_ORDER } from './input/press';
  import { nav } from './nav.svelte';
  import { activeDownload, connection, TAB_NAMES } from './topbar';

  let dl = $derived(nav.startup ? null : activeDownload(downloads.entries));
  let conn = $derived(connection(session));
</script>

<header class="top" data-testid="tv-topbar">
  <div class="brand">
    <span class="g-icon"><TvIcon name="grid" px={22} /></span>GRID <small>TV</small>
  </div>
  {#if nav.startup}
    <div class="crumb"></div>
  {:else if nav.topPage}
    <div class="crumb" data-testid="tv-crumb">
      <Glyph button="B" />
      {TAB_NAMES[nav.tab]} › <b>{nav.topPage.title}</b>
    </div>
  {:else}
    <nav class="tabs" data-testid="tv-tabs">
      <Glyph button="LB" />
      {#each TAB_ORDER as t (t)}
        <button class="tab" class:on={t === nav.tab} data-tab={t} data-testid={`tv-tab-${t}`} aria-current={t === nav.tab ? 'page' : undefined}>{TAB_NAMES[t]}</button>
      {/each}
      <Glyph button="RB" />
    </nav>
  {/if}
  <div class="status">
    {#if dl}
      <span class="dlmini" data-testid="tv-topbar-download">
        <TvIcon name="download" px={16} />{dl.title} <i><b style={`width:${dl.pct}%`}></b></i> {dl.pct}%
      </span>
    {/if}
    {#if !nav.startup}
      <span data-testid="tv-connection"><i class="dot" class:offline={conn.dot === 'offline'} class:none={conn.dot === 'none'}></i>{conn.text}</span>
    {/if}
    <span class="clock" data-testid="tv-clock">{clock.text}</span>
  </div>
</header>
