<script lang="ts">
  // The first-run setup card while TV mode is up (items 151–156; tvmode.js
  // `setupCard`): a held notice pinned to the top of the stack, never
  // dropped, that follows setup's installs (the Downloads rows). One segment
  // per install; the step line names what runs. Once nothing is still
  // installing it says how many systems are ready and closes after 10 s,
  // which also ends the run. Draws nothing itself: TvToasts draws the card.
  import { onDestroy, untrack } from 'svelte';
  import { downloads } from '../../stores/downloads.svelte';
  import { dismissSetupRun, setupRun } from '../../stores/setupRun.svelte';
  import { closeNotice, releaseNotice, showNotice, tvNotices, updateNotice } from '../notices.svelte';
  import { SETUP_CARD_DONE_MS, setupCardView } from './setupCard';

  let id: number | null = null;
  let released = false;
  let timer: ReturnType<typeof setTimeout> | null = null;

  let view = $derived(
    setupRun.active
      ? setupCardView(
          { queued: setupRun.queued, errors: setupRun.errors, outcomes: setupRun.outcomes, names: setupRun.names, compat: setupRun.compat },
          downloads.entries,
        )
      : null,
  );

  $effect(() => {
    const v = view;
    untrack(() => {
      if (v === null) {
        if (id !== null) closeNotice(id);
        id = null;
        released = false;
        return;
      }
      const patch = { title: v.title, segs: v.segs, step: v.step, sub: v.sub };
      const shown = id !== null && tvNotices.cards.some((c) => c.id === id);
      if (shown) updateNotice(id!, patch);
      else if (!released) id = showNotice({ ...patch, step: v.step ?? undefined, sub: v.sub ?? undefined, hold: true, pinned: true });
      if (v.done && !released && id !== null) {
        released = true;
        releaseNotice(id, SETUP_CARD_DONE_MS);
        timer = setTimeout(() => {
          timer = null;
          dismissSetupRun();
        }, SETUP_CARD_DONE_MS);
      }
    });
  });

  // Leaving TV mode mid-countdown: the desktop's card takes over the run.
  onDestroy(() => {
    if (timer !== null) clearTimeout(timer);
  });
</script>
