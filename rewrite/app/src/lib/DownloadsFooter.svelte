<script lang="ts">
  import { downloads } from './stores/downloads.svelte';
  import { footerLine } from './downloads/format';

  let { onOpen }: { onOpen: () => void } = $props();

  let line = $derived(footerLine(downloads.entries));
</script>

<!-- Always mounted, hidden when nothing is live (design §3). Clicking
     anywhere on the strip opens the Downloads view. -->
<footer
  data-testid="downloads-footer"
  class="strip"
  hidden={line === null}
  role="button"
  tabindex="0"
  onclick={onOpen}
  onkeydown={(e) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      onOpen();
    }
  }}
>
  <span data-testid="downloads-aggregate" class="line">{line ?? ''}</span>
  <!-- Plan 4 puts the 60-sample sparkline here; the slot reserves its
       120×18 footprint now so the strip's height never changes later. -->
  <span class="sparkline-slot" aria-hidden="true"></span>
  <span class="open-link">Open Downloads</span>
</footer>

<style>
  .strip {
    position: fixed;
    left: 0;
    right: 0;
    bottom: 0;
    height: var(--footer-h);
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 0 16px;
    background: var(--surface-2);
    border-top: 1px solid var(--border);
    color: var(--text-muted);
    font-size: 12px;
    cursor: pointer;
    z-index: 10;
  }

  .strip[hidden] {
    display: none;
  }

  .strip:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: -2px;
  }

  .line {
    flex: 1 1 auto;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-h);
  }

  .sparkline-slot {
    flex: none;
    width: 120px;
    height: 18px;
  }

  .open-link {
    flex: none;
    color: var(--primary);
    text-decoration: underline;
    white-space: nowrap;
  }
</style>
