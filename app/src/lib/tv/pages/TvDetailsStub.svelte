<script lang="ts">
  // A stand-in for Details until P9 builds TvDetails (TV mode plan § P9):
  // the game's title and platform, so A on a cover lands somewhere real and
  // B comes back to the cover. TODO(P9): replace with TvDetails.
  import type { DetailsSubject } from '../../details/subject';
  import TvPage from '../ui/TvPage.svelte';
  import TvButton from '../ui/TvButton.svelte';
  import { popPage } from '../nav.svelte';

  let { subject }: { subject: DetailsSubject } = $props();
</script>

<div data-testid="tv-details-stub">
  <TvPage title={subject.name} sub={subject.platformName}>
    <div class="btns" data-nav="h">
      <TvButton k="dt-back" autofocus onA={() => void popPage()}>Back</TvButton>
    </div>
  </TvPage>
</div>
