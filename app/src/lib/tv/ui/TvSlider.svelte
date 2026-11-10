<script lang="ts">
  // A slider (item 29): 12rem, with ◀ ▶ shown while its row is focused and
  // the formatted value. Drawn inside a focusable row (`data-adj`); Left /
  // Right on the row adjust it.
  let { percent, text }: { percent: number; text: string } = $props();
</script>

<span class="adj-arrows">◀</span><span class="sl"><b style={`width:${percent}%`}></b><i style={`left:${percent}%`}></i></span><span style="min-width:6rem">{text}</span><span class="adj-arrows">▶</span>
