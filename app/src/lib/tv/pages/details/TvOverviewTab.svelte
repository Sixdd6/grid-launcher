<script lang="ts">
  // Details › Overview (items 295–300; tvmode.js `detBody`): the summary,
  // which reads like a page (`data-read`: Down / Up scroll the tab by up to
  // 6rem while more of it is out of view, then move on), the Related row
  // (its covers go Up to the summary, item 298) and the facts panel. The
  // facts are the desktop's Overview facts that have a value.
  import type { DetailsStore } from '../../../details/detailsStore.svelte';
  import { developerOf } from '../../../details/header';
  import { contentFact, descriptionParagraphs, sizeFact, versionFact } from '../../../details/overview';
  import { normalizeTitle, relatedOnServer } from '../../../details/related';
  import { installed, isInstalled, matchesInstalled } from '../../../stores/installed.svelte';
  import { updates } from '../../../stores/updates.svelte';
  import { tvf } from '../../focus/actions';
  import { gameFromSummary, type TvGame } from '../../games';
  import TvGameCover from '../../ui/TvGameCover.svelte';
  import TvTrack from '../../ui/TvTrack.svelte';
  import { factsOf, RELATED_MAX, releaseFact } from '../details';

  let { d }: { d: DetailsStore } = $props();

  let paragraphs = $derived(descriptionParagraphs(d.description));
  let updateIds = $derived(new Set(updates.rows.map((r) => r.rom_id)));

  // Related: the desktop's "related, filtered to titles on the server", as
  // the server's own covers, up to six.
  let related = $derived.by((): TvGame[] => {
    const byTitle = new Map(d.serverGames.map((g) => [normalizeTitle(g.name), g]));
    const out: TvGame[] = [];
    for (const r of relatedOnServer(d.detail?.related ?? [], d.serverTitles)) {
      const g = byTitle.get(normalizeTitle(r.name));
      if (!g || g.id === d.subject.romId) continue;
      const label = d.subject.platformName;
      const row = isInstalled(g, label) ? (installed.list.find((x) => matchesInstalled(x, g, label)) ?? null) : null;
      out.push(gameFromSummary(g, label, d.platformSlug, row, updateIds));
      if (out.length === RELATED_MAX) break;
    }
    return out;
  });

  let facts = $derived.by(() => {
    const detail = d.detail;
    const companies = detail?.companies || d.subject.companies || '';
    return factsOf([
      ['developer', 'Developer', developerOf(companies)],
      ['companies', 'Companies', companies],
      ['release', 'Release', releaseFact(Number(detail?.first_release_date || d.subject.releaseDate || ''))],
      ['genres', 'Genres', d.genres],
      ['modes', 'Game modes', detail?.game_modes ?? ''],
      ['players', 'Players', detail?.player_count ?? ''],
      ['franchises', 'Franchises', detail?.franchises ?? ''],
      ['size', 'Size', sizeFact(detail?.files ?? [])],
      ['version', 'Version', versionFact(d.installedNow, d.installedVersion, d.serverVersion)],
      ['content', 'Content', contentFact(detail?.files ?? [])],
    ]);
  });
</script>

<div class="ov">
  <div>
    <div class="prose" data-k="prose" data-al="Read" data-read data-testid="tv-det-prose" use:tvf={{}}>
      {#each paragraphs as p, i (i)}
        <p>{p}</p>
      {:else}
        <p class="muted">No description available</p>
      {/each}
    </div>
    {#if related.length}
      <div class="sub-h">Related<span>{related.length}</span></div>
      <TvTrack scroll="det-rel" up="prose" enter="first">
        {#each related as g (g.id)}
          <TvGameCover k={`rel${g.id}`} game={g} label={false} testId={`tv-cover-rel${g.id}`} />
        {/each}
      </TvTrack>
    {/if}
  </div>
  {#if facts.length}
    <dl class="facts" data-testid="tv-det-facts">
      {#each facts as f (f.key)}
        <div><dt>{f.label}</dt><dd data-testid={`tv-det-fact-${f.key}`}>{f.value}</dd></div>
      {/each}
    </dl>
  {/if}
</div>
