<script lang="ts">
  // Menu › Notifications (items 356–359; tvmode.js `notesPage`): the
  // session's notices, newest first, each with its line and time; Y clears
  // them. The history keeps the whole session (ruling 16).
  import { tvf } from '../focus/actions';
  import Glyph from '../glyphs/Glyph.svelte';
  import { restoreFocus } from '../nav.svelte';
  import { clearHistory, tvNotices } from '../notices.svelte';
  import { timeOf } from '../notices';
  import { useScreen } from '../screen.svelte';

  useScreen({
    hints: () => [{ button: 'Y', label: 'Clear all' }],
    y: () => {
      clearHistory();
      restoreFocus();
    },
  });
</script>

<div class="list" data-scroll="list" data-testid="tv-notifications">
  <div class="list-in">
    <div class="panel">
      <h2>Notifications<span data-testid="tv-notifications-count">{tvNotices.history.length}</span></h2>
      <div data-nav="v">
        {#each tvNotices.history as n (n.id)}
          <div class="lrow" data-k={`n${n.id}`} data-testid="tv-notification" use:tvf>
            <div class="grow">
              <div class="t" class:err={n.error}>{n.title}</div>
              <div class="s">{#if n.sub !== null}{#if typeof n.sub === 'string'}{n.sub}{:else}{#each n.sub as part, i (i)}{#if typeof part === 'string'}{part}{:else}<Glyph button={part.glyph} />{/if}{/each}{/if} · {/if}{timeOf(n.at)}</div>
            </div>
          </div>
        {:else}
          <p class="note" data-testid="tv-notifications-empty">No notifications yet</p>
        {/each}
      </div>
    </div>
  </div>
</div>
