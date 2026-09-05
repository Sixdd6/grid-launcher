<script lang="ts">
  import { FILLED_ICONS, ICONS, type IconName } from './icons';

  // The app's one icon. Modelled on `downloads/Sparkline.svelte`, which is
  // the only other SVG in the app and already gets this right: an explicit
  // `viewBox` plus explicit `width`/`height`, `display: block` so the mark
  // is a block box that cannot drift on a text baseline the way a glyph
  // does, and `currentColor` so the colour is always the caller's token.
  //
  // `label` decides the ARIA shape. Absent (the icon sits beside visible
  // text, or its button already has an `aria-label`): the SVG is hidden from
  // the accessibility tree so it cannot be announced twice. Present (the
  // icon IS the label): `role="img"` plus the name.
  let {
    name,
    size = 16,
    label = undefined,
  }: {
    name: IconName;
    size?: number;
    label?: string;
  } = $props();

  let filled = $derived(FILLED_ICONS.includes(name));
</script>

<svg
  class="icon"
  viewBox="0 0 24 24"
  width={size}
  height={size}
  fill="none"
  stroke="currentColor"
  stroke-width="1.5"
  stroke-linecap="round"
  stroke-linejoin="round"
  role={label === undefined ? undefined : 'img'}
  aria-label={label}
  aria-hidden={label === undefined ? 'true' : undefined}
  focusable="false"
>
  <path
    d={ICONS[name]}
    fill={filled ? 'currentColor' : 'none'}
    stroke={filled ? 'none' : 'currentColor'}
  />
</svg>

<style>
  .icon {
    display: block;
    flex: none;
    /* The icon never becomes the event target. Every icon in the app sits
       inside a button whose id the E2E suite clicks, and letting the click
       land on the button itself keeps `elementFromPoint`-style hit tests
       (and any future tooltip) pointing at the control, not the artwork. */
    pointer-events: none;
  }
</style>
