<script lang="ts">
  // One button prompt in the set the controller pressed last uses (items
  // 57–69): a Kenney glyph drawn as a mask in the text colour, with the
  // button's name as its accessible label (item 65).
  //
  // `button` is what the button does (A selects, B goes back); the glyph is
  // the physical button the controller's assignment binds to it, as the set
  // prints it (`faceGlyph`). `data-glyph` keeps the role, `data-face` the
  // glyph drawn.
  import { controllers } from '../input/router.svelte';
  import { faceGlyph } from '../input/prompts';
  import { glyph, type GlyphButton } from './glyphs';

  let { button }: { button: GlyphButton } = $props();

  let set = $derived(controllers.promptSet);
  let face = $derived(faceGlyph(button, controllers.assignment, set));
  let g = $derived(glyph(set, face));
</script>

<span class="gli" role="img" aria-label={g.label} data-glyph={button} data-face={face} data-set={set} style={`--g:url("${g.url}")`}></span>
