<script lang="ts">
  // Details › Achievements (item 315; tvmode.js `detBody`): the summary line
  // ("<k> of <n> unlocked · <p> of <total> points · <h> hardcore"), then a
  // row per achievement: the badge (locked ones grey and faded), the name,
  // "Unlocked <when>" (with " · Hardcore") or the description, and
  // "<n> pts". The data and wording are the desktop tab's.
  import { api, type AchievementsView } from '../../../api';
  import Image from '../../../Image.svelte';
  import { PROGRESS_HINT, showsProgressHint, summaryLine, unlockDate } from '../../../details/achievements';
  import { tvf } from '../../focus/actions';

  let { romId }: { romId: number } = $props();

  let view = $state<AchievementsView | null>(null);
  let error = $state<string | null>(null);

  $effect(() => {
    const id = romId;
    let cancelled = false;
    view = null;
    error = null;
    api.getAchievements(id).then(
      (v) => {
        if (!cancelled) view = v;
      },
      (err: unknown) => {
        if (!cancelled) error = err instanceof Error ? err.message : String(err);
      },
    );
    return () => {
      cancelled = true;
    };
  });
</script>

{#if error}
  <p class="note err" data-testid="tv-ach-error">{error}</p>
{:else if view === null}
  <p class="note">Loading achievements…</p>
{:else}
  {@const known = view.progress_known}
  <p class="note" data-testid="tv-ach-summary">{summaryLine(view)}</p>
  {#if showsProgressHint(view)}<p class="note">{PROGRESS_HINT}</p>{/if}
  <div data-nav="v" id="ach-list">
    {#each view.rows as r, i (r.ra_id ?? `row-${i}`)}
      <div class="lrow" data-k={`a${i}`} use:tvf={{}}>
        <span class="badge" class:locked={known && !r.earned}><Image url={r.badge_url || null} alt={r.title} placeholder="" /></span>
        <div class="grow">
          <div class="t">{r.title}</div>
          <div class="s">{r.earned ? `Unlocked ${unlockDate(r.unlocked_at)}${r.hardcore ? ' · Hardcore' : ''}` : r.description}</div>
        </div>
        <span class="muted">{r.points} pts</span>
      </div>
    {/each}
  </div>
{/if}
