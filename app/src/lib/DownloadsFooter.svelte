<script lang="ts">
  import { downloads } from './stores/downloads.svelte';
  import { currentTransfer, footerLine } from './downloads/format';
  import Sparkline from './downloads/Sparkline.svelte';
  import Icon from './Icon.svelte';

  let { onOpen }: { onOpen: () => void } = $props();

  let current = $derived(currentTransfer(downloads.entries));
  let line = $derived(footerLine(downloads.entries));
</script>

<!-- Always mounted, hidden when nothing is live (design §3). Clicking
     anywhere on the strip opens the Downloads view. The sparkline is the
     current transfer's 60 samples at 120×18 — the same component and the
     same ring the Downloads view draws at 120×38. -->
<footer
  data-testid="downloads-footer"
  class="strip"
  hidden={current === null}
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
  {#if line !== null}
    <span class="lead"><Icon name="download" size={14} /></span>
  {/if}
  <span data-testid="downloads-aggregate" class="line">{line ?? ''}</span>
  {#if current !== null}
    <Sparkline
      samples={downloads.samplesFor(current.id)}
      width={120}
      height={18}
      label={`Transfer rate for ${current.title}`}
      testId="downloads-footer-graph"
    />
  {/if}
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

  /* The download mark, outside `.line` so the ellipsis box stays a single
     text run, and in `.line`'s colour rather than the strip's muted one so
     the two read as one unit. */
  .lead {
    display: flex;
    flex: none;
    color: var(--text-h);
  }

  .line {
    flex: 1 1 auto;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-h);
  }

  .open-link {
    flex: none;
    color: var(--primary);
    text-decoration: underline;
    white-space: nowrap;
  }
</style>
