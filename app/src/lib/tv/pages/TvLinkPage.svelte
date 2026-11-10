<script lang="ts">
  // Link to server game (items 346–348; tvmode.js `linkPage`, `ACT.doLink`):
  // "Link <title>", "Pick the <Platform> game on your server that this is."
  // (plus "Search: <text>" while searching), over the platform's server
  // games that no installed row holds. A links one and goes back; Y opens
  // the keyboard and the list narrows as you type (B there keeps the search
  // it had). The list and the pick are the shared LinkList's.
  import type { GameSummary, InstalledGame } from '../../api';
  import Image from '../../Image.svelte';
  import { LinkList } from '../../details/linkList.svelte';
  import { fromInstalled } from '../../details/subject';
  import * as run from '../../games/run';
  import { installed } from '../../stores/installed.svelte';
  import { pushToast } from '../../stores/toasts.svelte';
  import { tvf } from '../focus/actions';
  import { popPage, setPageProps } from '../nav.svelte';
  import { openKeyboard } from '../overlays/overlays.svelte';
  import { useScreen } from '../screen.svelte';
  import { LINK_NO_MATCH, linkedNotice, linkLine, linkSub } from './details';

  let { row, held, detailsPage = null }: { row: InstalledGame; held: ReadonlySet<number>; detailsPage?: number | null } = $props();

  const list = new LinkList();
  let q = $state('');
  $effect(() => list.load(row.platform));

  let games = $derived(list.shown(q.trim(), held));

  async function link(g: GameSummary) {
    await list.pick(g, async (rom) => {
      const outcome = await run.linkGame(row.title, row.platform, rom.id);
      if (outcome.kind !== 'ok') throw new Error(outcome.reason);
    });
    if (list.pickError) {
      pushToast(list.pickError, 'error');
      return;
    }
    pushToast(linkedNotice(row.title, g.name), 'success');
    // Details under this page follows the row to its server game.
    const linked = installed.list.find((r) => r.rom_id === g.id);
    if (detailsPage !== null && linked) setPageProps(detailsPage, { subject: fromInstalled(linked) });
    void popPage();
  }

  useScreen({
    hints: () => [{ button: 'Y', label: 'Search' }],
    y: () => {
      const before = q;
      void openKeyboard({
        title: `Search ${row.platform}`,
        value: q,
        onInput: (v) => (q = v),
        onDone: (v) => (q = v),
        onCancel: () => (q = before),
      });
    },
  });
</script>

<div class="page" data-scroll="link" data-testid="tv-link-page">
  <div class="page-in">
    <h1>Link {row.title}</h1>
    <p class="sub" data-testid="tv-link-sub">{linkSub(row.platform)}{#if q.trim()} Search: <b>{q.trim()}</b>{/if}</p>
    {#if list.loadError}<p class="note err">{list.loadError}</p>{/if}
    <div data-nav="v" id="link-list">
      {#if list.games !== null}
        {#each games as g (g.id)}
          <div class="lrow" data-k={`lk${g.id}`} data-al="Link" data-testid={`tv-link-${g.id}`} use:tvf={{ a: () => void link(g) }}>
            <span class="badge"><Image url={g.path_cover_small || g.path_cover_large} alt="" placeholder="" /></span>
            <div class="grow">
              <div class="t">{g.name}</div>
              <div class="s">{linkLine(row.platform, g.first_release_date)}</div>
            </div>
          </div>
        {:else}
          <p class="note" data-testid="tv-link-none">{LINK_NO_MATCH}</p>
        {/each}
      {/if}
    </div>
  </div>
</div>
