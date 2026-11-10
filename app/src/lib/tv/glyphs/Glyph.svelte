<script lang="ts">
  // One button prompt in the set the controller pressed last uses (items
  // 57–69): a Kenney glyph drawn as a mask in the text colour, with the
  // button's name as its accessible label (item 65).
  import { controllers } from '../input/router.svelte';
  import { glyph, type GlyphButton } from './glyphs';

  let { button }: { button: GlyphButton } = $props();

  let set = $derived(controllers.promptSet);
  let g = $derived(glyph(set, button));
</script>

<span class="gli" role="img" aria-label={g.label} data-glyph={button} data-set={set} style={`--g:url("${g.url}")`}></span>
