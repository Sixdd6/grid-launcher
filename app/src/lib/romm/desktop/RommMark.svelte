<script lang="ts" module>
  /** RomM's mark (RommStorefront README): the monogram "R" in RomM's colour,
   *  never a logo. The prototype's `RM_MARK`. */
  export const ROMM_MARK = { name: 'RomM', mono: 'R', hue: 'hsl(258 50% 46%)' } as const;
</script>

<script lang="ts">
  let { size = 'sm' }: { size?: 'sm' | 'lg' } = $props();
</script>

<span class="mono {size}" style:--st-hue={ROMM_MARK.hue} aria-hidden="true" data-mono={ROMM_MARK.mono}></span>

<style>
  .mono {
    display: inline-grid;
    place-items: center;
    flex: none;
    width: 16px;
    height: 16px;
    border-radius: 4px;
    background: var(--st-hue);
    color: #fff;
    font: 700 10px/1 system-ui, sans-serif;
    letter-spacing: 0;
  }

  /* The letter is drawn, not text: a label reads as the name alone. */
  .mono::before {
    content: attr(data-mono);
  }

  .mono.lg {
    width: 34px;
    height: 34px;
    border-radius: var(--r-control);
    font-size: 16px;
  }
</style>
