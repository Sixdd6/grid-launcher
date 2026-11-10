<script lang="ts">
  // The on-screen keyboard (items 377–388; tvmode.js `openOSK`): full width
  // from the bottom, on `overlay`, its title muted over the field and a
  // blinking caret, four rows of keys, then Shift (LT), the symbols key,
  // Space (Y), Delete (X) and Done (Menu). Keys keep their `data-k` when
  // Shift redraws them, so focus stays on the same key (item 388).
  import { tvf } from '../focus/actions';
  import Glyph from '../glyphs/Glyph.svelte';
  import { BACK, DONE, KEY_ROWS, keyFace, oskShown, SHIFT, SPACE, SYMBOLS_KEY } from './osk';
  import type { KeyboardModel } from './overlays.svelte';

  let { model }: { model: KeyboardModel } = $props();

  let shown = $derived(oskShown(model.osk, model.mask));
</script>

<div class="osk" data-testid="tv-osk" role="dialog" aria-label={model.title}>
  <div class="osk-head">
    <div class="t">{model.title}</div>
    <div class="osk-val" data-testid="tv-osk-value">{shown.before}<span class="caret"></span>{shown.after}</div>
  </div>
  <div class="keys">
    {#each KEY_ROWS as row, ri (ri)}
      <div class="krow" data-nav="h">
        {#each row as k (k)}
          {@const face = keyFace(k, model.osk.shift)}
          <button class="key" data-k={`k${k}`} data-key={face} data-testid={`tv-key-${k}`} use:tvf={{ a: () => model.type(face) }}>{face}</button>
        {/each}
      </div>
    {/each}
    <div class="krow" data-nav="h">
      <button class="key w2" class:on={model.osk.shift} data-k="kshift" data-key={SHIFT} data-testid="tv-key-shift" use:tvf={{ a: () => model.type(SHIFT) }}><Glyph button="LT" />Shift</button>
      <button class="key w2" data-k="ksym" data-key={SYMBOLS_KEY} data-testid="tv-key-sym" use:tvf={{ a: () => model.type(SYMBOLS_KEY) }}>@ _ #</button>
      <button class="key w5" data-k="kspace" data-key={SPACE} data-testid="tv-key-space" use:tvf={{ a: () => model.type(SPACE) }}><Glyph button="Y" />Space</button>
      <button class="key w2" data-k="kback" data-key={BACK} data-testid="tv-key-back" aria-label="Delete" use:tvf={{ a: () => model.type(BACK) }}><Glyph button="X" />⌫</button>
      <button class="key w2 pri" data-k="kdone" data-key={DONE} data-testid="tv-key-done" use:tvf={{ a: () => model.type(DONE) }}><Glyph button="MENU" />Done</button>
    </div>
  </div>
</div>
