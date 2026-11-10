<script lang="ts">
  // A stand-in for a system's page until P11 builds TvSystem (TV mode plan
  // § P11), reached from a Server platform's emulator and firmware chips.
  // TODO(P11): replace with TvSystem.
  import TvPage from '../ui/TvPage.svelte';
  import TvButton from '../ui/TvButton.svelte';
  import { popPage } from '../nav.svelte';

  let { platform }: { platform: string } = $props();
</script>

<div data-testid="tv-system-stub">
  <TvPage title={platform}>
    <div class="btns" data-nav="h">
      <TvButton k="sy-back" autofocus onA={() => void popPage()}>Back</TvButton>
    </div>
  </TvPage>
</div>
