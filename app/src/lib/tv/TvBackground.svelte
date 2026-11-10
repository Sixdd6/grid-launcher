<script lang="ts">
  // The background art (items 9–12): the focused game's art behind the
  // panels (TvCover reports it; Details and Playing do in P9), faded and
  // blurred by Settings › Appearance, hidden when Background art is off. The
  // desktop's BackgroundArt draws it: the art arrives already blurred from
  // the backend (`ensure_background_variant`), so the prototype's CSS blur
  // and its 6rem overscan, which hid that blur's soft edge, have nothing to
  // do. It fades over 360 ms in TV mode.
  import BackgroundArt from '../BackgroundArt.svelte';
</script>

<div class="tv-art" aria-hidden="true">
  <BackgroundArt fadeMs={360} />
</div>
