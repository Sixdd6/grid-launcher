<script lang="ts">
  // An offline installer GRID refuses (TVStores README § Offline GOG
  // installer; prototype `sRefuse`): "Can’t install this file", the reason
  // in the desktop's words, then Choose another file… and OK (focused).
  import { closeLayer } from '../nav.svelte';
  import TvButton from '../ui/TvButton.svelte';

  let { heading, title, body, another }: { heading: string; title: string; body: string; another: () => void } = $props();
</script>

<div class="dialog" role="alertdialog" aria-label={heading} data-testid="tv-store-refusal">
  <h2>{heading}</h2>
  <p><b data-testid="tv-store-refusal-title">{title}</b><br />{body}</p>
  <div class="btns" data-nav="h">
    <TvButton k="rf-another" testId="tv-store-refusal-another" onA={() => void closeLayer().then(another)}>Choose another file…</TvButton>
    <TvButton k="rf-ok" primary autofocus testId="tv-store-refusal-ok" onA={() => void closeLayer()}>OK</TvButton>
  </div>
</div>
