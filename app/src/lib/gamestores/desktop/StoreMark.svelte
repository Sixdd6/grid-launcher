<script lang="ts">
  // The store mark (GameStores README, designer decision 3): a monogram tile
  // in the store's colour, never the store's logo. Prototype `.st-mono`.
  import type { StoreSource } from '../../api';
  import { storeMark } from '../marks';

  let { source, size = 'sm' }: { source: StoreSource; size?: 'xs' | 'sm' | 'lg' | 'xl' } = $props();
  let mark = $derived(storeMark(source));
</script>

<span class="st-mono {size}" style:--st-hue={mark.hue} aria-hidden="true">{mark.mono}</span>

<style>
  .st-mono {
    display: inline-grid;
    place-items: center;
    flex: none;
    width: 16px;
    height: 16px;
    border-radius: 4px;
    background: var(--st-hue, var(--field));
    color: #fff;
    font: 700 10px/1 system-ui, sans-serif;
    letter-spacing: 0;
  }

  .st-mono.xs {
    width: 11px;
    height: 11px;
    border-radius: 2px;
    font-size: 7px;
  }

  .st-mono.lg {
    width: 34px;
    height: 34px;
    border-radius: var(--r-control);
    font-size: 16px;
  }

  .st-mono.xl {
    width: 48px;
    height: 48px;
    border-radius: var(--r-row);
    font-size: 22px;
  }
</style>
