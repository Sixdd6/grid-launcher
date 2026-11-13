<script lang="ts">
  // A store game's cover (TVStores README § Library › Covers; prototype
  // `scover`): the installed dot; one top-left tag (NOT SUPPORTED, SAVE
  // CONFLICT or UPDATE, in that order); while a job runs, its percentage
  // (or Queued) and a bar along the bottom edge; in mixed lists the store's
  // tag at the bottom left. A opens the game's page; X is the quick action
  // (Play, Install…, Install from drive), none while a job runs.
  import type { DownloadEntry, StoreGame } from '../../api';
  import { storeName } from '../../gamestores/marks';
  import { tvf } from '../focus/actions';
  import TvIcon from '../ui/TvIcon.svelte';
  import TvStoreArt from './TvStoreArt.svelte';
  import TvStoreMark from './TvStoreMark.svelte';
  import { tvCoverTag, tvJobTag } from './storeWords';

  let {
    k,
    game,
    job,
    quick,
    mixed = false,
    sel = undefined,
    picked = false,
    label = true,
    width = undefined,
    testId = undefined,
    onA = undefined,
    onX = undefined,
  }: {
    k: string;
    game: StoreGame;
    job: DownloadEntry | null;
    /** X's label, or null for none. */
    quick: string | null;
    /** All games, Recent, Updates and a drive's grid: the store's tag. */
    mixed?: boolean;
    /** The id select mode picks it by (GOG › Installed). */
    sel?: string;
    picked?: boolean;
    label?: boolean;
    width?: string;
    testId?: string;
    /** Not set: the cover isn't focusable (the game page's own cover). */
    onA?: () => void;
    onX?: () => void;
  } = $props();

  let tag = $derived(tvCoverTag(game, job));
  let jt = $derived(tvJobTag(job));
  let style = $derived(`--shape: .75${width ? `; --cw: ${width}` : ''}`);
</script>

{#snippet art()}
  <span class="cv-art">
    <span class="cv-img"><TvStoreArt url={game.cover} alt={game.title} placeholder={game.title} /></span>
    {#if picked}<span class="pick" aria-label="Selected"><TvIcon name="check" px={16} /></span>{/if}
    {#if tag}<span class={`tag ${tag.cls}`} data-testid="tv-store-cover-tag">{tag.text}</span>{/if}
    {#if game.installed}<i class="idot" aria-label="Installed"></i>{/if}
    {#if mixed}<span class="tag sst"><TvStoreMark source={game.source} />{storeName(game.source)}</span>{/if}
    {#if jt}<span class="tag q" data-testid="tv-store-cover-job">{jt.text}</span><span class="sprog"><b style={`width:${jt.pct}%`}></b></span>{/if}
  </span>
  {#if label}<span class="cv-l">{game.title}</span>{/if}
{/snippet}

{#if onA}
  <button
    class="cv"
    class:sns={game.not_supported !== ''}
    class:pk={picked}
    data-k={k}
    data-al="Details"
    data-xl={quick ?? undefined}
    data-cover
    data-sel={sel}
    data-testid={testId}
    {style}
    use:tvf={{ a: onA, x: quick ? onX : undefined }}
  >
    {@render art()}
  </button>
{:else}
  <div class="cv" class:sns={game.not_supported !== ''} {style} data-testid={testId}>
    {@render art()}
  </div>
{/if}
