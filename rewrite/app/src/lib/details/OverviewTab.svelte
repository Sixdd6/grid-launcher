<script lang="ts">
  import type { RomDetail } from '../api';
  import Image from '../Image.svelte';
  import { epochDate } from './header';
  import { overviewStrip } from './media';
  import { relatedKindLabel, relatedOnServer } from './related';

  let {
    name,
    description,
    screenshotUrls,
    detail,
    serverTitles,
  }: {
    name: string;
    description: string;
    screenshotUrls: string[];
    detail: RomDetail | null;
    /** Every title on this game's platform, for design §7's "filtered to
     *  titles present on the server". Empty until the list loads. */
    serverTitles: string[];
  } = $props();

  let strip = $derived(overviewStrip(screenshotUrls));

  let related = $derived(relatedOnServer(detail?.related ?? [], serverTitles));

  // `details-meta-<key>` rows, built from whatever the server actually
  // knows. A row with no value is dropped rather than rendered blank: an
  // empty grid cell reads as a failure, an absent row reads as "the server
  // has nothing", which is the truth.
  let metaRows = $derived(
    (
      [
        ['developer', 'Developer', detail?.companies.split(',')[0]?.trim() ?? ''],
        ['companies', 'Companies', detail?.companies ?? ''],
        // The backend sends IGDB's epoch SECONDS as a string; the raw
        // number is not a date a reader can use. The full day, not just
        // the year, because the header line above already states the year.
        ['release', 'Release', epochDate(Number(detail?.first_release_date ?? ''))],
        ['genres', 'Genres', detail?.genres ?? ''],
        ['modes', 'Game modes', detail?.game_modes ?? ''],
        ['players', 'Players', detail?.player_count ?? ''],
        ['franchises', 'Franchises', detail?.franchises ?? ''],
      ] as const
    ).filter(([, , value]) => value.trim() !== '')
  );

  let failedScreenshots = $state<Record<string, true>>({});
  function markScreenshotFailed(url: string) {
    failedScreenshots = { ...failedScreenshots, [url]: true };
  }
</script>

<div class="overview">
  <p data-testid="details-description" class="description">{description}</p>

  {#if metaRows.length}
    <dl class="meta">
      {#each metaRows as [key, label, value] (key)}
        <dt>{label}</dt>
        <dd data-testid={`details-meta-${key}`}>{value}</dd>
      {/each}
    </dl>
  {/if}

  {#if strip.length}
    <div class="shots" data-testid="details-screenshots">
      {#each strip as url, i (url)}
        {#if !failedScreenshots[url]}
          <Image
            {url}
            alt={`${name} screenshot ${i + 1}`}
            data-testid={`details-screenshot-${i}`}
            onerror={() => markScreenshotFailed(url)}
          />
        {/if}
      {/each}
    </div>
  {:else}
    <p class="empty" data-testid="details-no-screenshots">No screenshots available</p>
  {/if}

  {#if related.length}
    <div class="related" data-testid="details-related">
      <h3>Related</h3>
      <div class="chips">
        {#each related as entry, i (entry.name)}
          <span class="related-chip" data-testid={`details-related-${i}`}>
            {entry.name}
            <em>{relatedKindLabel(entry.kind)}</em>
          </span>
        {/each}
      </div>
    </div>
  {/if}
</div>

<style>
  .overview {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .description {
    margin: 0;
    color: var(--text);
    font-size: 14px;
    line-height: 1.5;
  }

  .meta {
    display: grid;
    grid-template-columns: 140px 1fr;
    gap: 6px 12px;
    margin: 0;
  }

  .meta dt {
    color: var(--text-muted);
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .meta dd {
    margin: 0;
    color: var(--text);
    font-size: 13px;
  }

  .shots {
    display: flex;
    gap: 8px;
    overflow-x: auto;
    padding-bottom: 4px;
  }

  .shots :global(img) {
    height: 110px;
    width: auto;
    flex: none;
    border-radius: var(--r-chip);
    object-fit: cover;
  }

  .empty {
    margin: 0;
    color: var(--text-muted);
    font-size: 13px;
  }

  .related h3 {
    margin: 0 0 8px;
    font-size: 13px;
    font-weight: 600;
    color: var(--text-h);
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .related-chip {
    display: inline-flex;
    align-items: baseline;
    gap: 6px;
    font-size: 12px;
    padding: 4px 10px;
    border-radius: var(--r-pill);
    background: var(--surface);
    border: 1px solid var(--border);
    color: var(--text);
  }

  .related-chip em {
    font-style: normal;
    color: var(--text-muted);
    font-size: 11px;
  }
</style>
