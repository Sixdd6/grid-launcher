<script lang="ts">
  // A dialog with one button (OK, focused): the welcome's "settings filled
  // in" after a Python import (TVRommStorefront README § The welcome). B
  // closes it too.
  import { closeLayer } from '../nav.svelte';
  import TvButton from '../ui/TvButton.svelte';

  let { title, body, testId = 'tv-alert' }: { title: string; body: string; testId?: string } = $props();
</script>

<div class="dialog" role="alertdialog" aria-label={title} data-testid={testId}>
  <h2>{title}</h2>
  <p>{body}</p>
  <div class="btns" data-nav="h">
    <TvButton k="dlg-ok" primary autofocus testId={`${testId}-ok`} onA={() => void closeLayer()}>OK</TvButton>
  </div>
</div>
