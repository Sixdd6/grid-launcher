<script lang="ts">
  // What replaces a store view's grid (GameStores README, "States in place
  // of the grid"; prototype `stState` and the loading skeleton): a panel
  // with the store's mark, a title, one line and its action, or "Loading
  // your GOG games…" over skeleton covers.
  import type { StoreSource } from '../../api';
  import type { PanelAct, StorePanel } from '../view';
  import StoreMark from './StoreMark.svelte';

  let { panel, source, onAct }: { panel: StorePanel; source: StoreSource; onAct: (act: PanelAct) => void } = $props();
</script>

{#if panel.kind === 'loading'}
  <div class="loading" data-testid={panel.testId}>
    <p class="loading-line"><span class="spin"></span>{panel.title}</p>
    <div class="skel-grid" aria-hidden="true">
      {#each Array.from({ length: 8 }) as _, i (i)}
        <div class="skel"><div class="skel-cover"></div><span class="skel-label"></span></div>
      {/each}
    </div>
  </div>
{:else}
  <div class="st-state {panel.tone}" data-testid={panel.testId}>
    <span class="st-ico"><StoreMark {source} size="lg" /></span>
    <h3>{panel.title}</h3>
    {#if panel.body}<p>{panel.body}</p>{/if}
    {#if panel.actions.length > 0}
      <div class="g-actions">
        {#each panel.actions as a (a.label)}
          <button
            class="g-btn"
            class:secondary={!a.primary}
            disabled={a.disabled}
            data-testid={`${panel.testId}-${a.act}`}
            onclick={() => onAct(a.act)}>{a.label}</button
          >
        {/each}
      </div>
    {/if}
  </div>
{/if}

<style>
  .st-state {
    align-self: stretch;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    /* The prototype's panel sits in the grid (padding --gutter 2px) with a --gutter margin. */
    margin: calc(var(--gutter) * 2) 2px 0;
    padding: 34px 24px;
    text-align: center;
    background: var(--panel);
    -webkit-backdrop-filter: var(--panel-blur);
    backdrop-filter: var(--panel-blur);
    border: 1px solid var(--line);
    border-radius: var(--r-panel);
  }

  .st-state h3 {
    margin: 0;
    font-size: 16px;
    color: var(--text-h);
  }

  .st-state p {
    margin: 0;
    max-width: 480px;
    color: var(--text-muted);
    font-size: 13px;
  }

  .st-state .g-actions {
    justify-content: center;
  }

  .st-state.warn {
    border-left: 3px solid var(--warning);
  }

  .st-state.err {
    border-left: 3px solid var(--danger);
  }

  .st-ico {
    display: inline-grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border-radius: var(--r-row);
    background: var(--field);
  }

  .loading-line {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: calc(var(--gutter) * 2) 2px 0;
    color: var(--text-muted);
    font-size: 13px;
  }

  .spin {
    display: inline-block;
    width: 10px;
    height: 10px;
    box-sizing: border-box;
    border-radius: 50%;
    border: 2px solid var(--line-strong);
    border-top-color: var(--primary);
    animation: spin 800ms linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .skel-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    gap: 22px 18px;
    margin: 18px 2px 0;
  }

  .skel {
    display: flex;
    flex-direction: column;
    gap: 8px;
    align-items: center;
  }

  .skel-cover {
    position: relative;
    width: 100%;
    aspect-ratio: 3 / 4;
    border-radius: var(--r-card);
    background: var(--field);
    overflow: hidden;
  }

  .skel-cover::after {
    content: '';
    position: absolute;
    inset: 0;
    background: linear-gradient(100deg, transparent 30%, color-mix(in srgb, var(--surface) 70%, transparent) 50%, transparent 70%);
    animation: shimmer 1.4s ease-in-out infinite;
  }

  .skel-label {
    display: block;
    height: 10px;
    width: 70%;
    border-radius: 3px;
    background: var(--field);
  }

  @keyframes shimmer {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(100%);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .spin,
    .skel-cover::after {
      animation: none;
    }
  }
</style>
