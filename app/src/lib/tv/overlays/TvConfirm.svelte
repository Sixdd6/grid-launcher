<script lang="ts">
  // A confirmation (items 373–376; tvmode.js `confirmBox`): a centred
  // 40rem dialog, title, body, then Cancel and the action right-aligned.
  // Cancel has focus when it opens; the action is danger-styled for a
  // destructive action, primary otherwise. B cancels (the layer's default).
  import { closeLayer } from '../nav.svelte';
  import TvButton from '../ui/TvButton.svelte';
  import type { ConfirmSpec } from './overlays.svelte';

  let { spec }: { spec: ConfirmSpec } = $props();

  function ok() {
    const fn = spec.onOk;
    void closeLayer().then(fn);
  }
</script>

<div class="dialog" role="alertdialog" aria-label={spec.title} data-testid="tv-confirm">
  <h2>{spec.title}</h2>
  <p>{spec.body}</p>
  <div class="btns" data-nav="h">
    <TvButton k="dlg-cancel" autofocus testId="tv-confirm-cancel" onA={() => void closeLayer()}>Cancel</TvButton>
    <TvButton k="dlg-ok" primary={!spec.danger} danger={!!spec.danger} testId="tv-confirm-ok" onA={ok}>{spec.ok}</TvButton>
  </div>
</div>
