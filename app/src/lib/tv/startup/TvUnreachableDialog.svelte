<script lang="ts">
  // The unreachable-library dialog's box (items 409–411; tvmode.js
  // `unreachable`): a blocking layer (B and a click outside do nothing; the
  // hints show only A "Select"), with Relocate the library… (primary,
  // focused) and Close without changing anything.
  import TvButton from '../ui/TvButton.svelte';
  import { CLOSE_LABEL, RELOCATE_LABEL, UNREACHABLE_TITLE, unreachableBody } from './unreachable';

  let { root, onRelocate, onClose }: { root: string; onRelocate: () => void; onClose: () => void } = $props();
</script>

<div class="dialog" role="alertdialog" aria-label={UNREACHABLE_TITLE} data-testid="tv-unreachable">
  <h2>{UNREACHABLE_TITLE}</h2>
  <p data-testid="tv-unreachable-body">{unreachableBody(root)}</p>
  <div class="btns" data-nav="h">
    <TvButton k="ur-relocate" primary autofocus testId="tv-unreachable-relocate" onA={onRelocate}>{RELOCATE_LABEL}</TvButton>
    <TvButton k="ur-close" testId="tv-unreachable-close" onA={onClose}>{CLOSE_LABEL}</TvButton>
  </div>
</div>
