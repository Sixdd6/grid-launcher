<script lang="ts">
  // RomM's mark (RommStorefront README: "the RomM mark (monogram "R", never
  // a logo)"): the store marks' monogram tile (`StoreMark.svelte`) in RomM's
  // colour. Prototype `RM_MARK`.
  let { size = 'sm' }: { size?: 'xs' | 'sm' | 'lg' | 'xl' } = $props();
</script>

<span class="st-mono {size}" aria-hidden="true" data-mono="R"></span>

<style>
  .st-mono {
    display: inline-grid;
    place-items: center;
    flex: none;
    width: 16px;
    height: 16px;
    border-radius: 4px;
    background: hsl(258 50% 46%);
    color: #fff;
    font: 700 10px/1 system-ui, sans-serif;
    letter-spacing: 0;
  }

  /* The letter is drawn, not text: a label reads as the name alone. */
  .st-mono::before {
    content: attr(data-mono);
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
