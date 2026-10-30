<script lang="ts">
  import type { RomDetail } from '../api';
  import { epochDate } from './header';
  import { descriptionParagraphs } from './overview';
  import { relatedKindLabel, relatedOnServer } from './related';

  let {
    description,
    detail,
    serverTitles,
  }: {
    description: string;
    detail: RomDetail | null;
    /** Every title on this game's platform, for design §7's "filtered to
     *  titles present on the server". Empty until the list loads. */
    serverTitles: string[];
  } = $props();

  let paragraphs = $derived(descriptionParagraphs(description));

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
</script>

<!-- The description and the facts side by side, so the text keeps a
     readable line length and the facts use the width instead of trailing
     down one edge; they stack when the popup is narrow. Screenshots live on
     the Media tab only. -->
<div class="overview">
  <div class="main">
    <div data-testid="details-description" class="description">
      {#each paragraphs as paragraph, i (i)}
        <p>{paragraph}</p>
      {:else}
        <p class="empty">No description available</p>
      {/each}
    </div>

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

  {#if metaRows.length}
    <dl class="meta">
      {#each metaRows as [key, label, value] (key)}
        <div class="fact">
          <dt>{label}</dt>
          <dd data-testid={`details-meta-${key}`}>{value}</dd>
        </div>
      {/each}
    </dl>
  {/if}
</div>

<style>
  .overview {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    gap: 20px 28px;
  }

  .main {
    flex: 1 1 340px;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  /* About 70 characters a line at most, with room between lines and
     paragraphs: the server's descriptions are often one long block. */
  .description {
    max-width: 68ch;
    color: var(--text);
    font-size: 14px;
    line-height: 1.65;
  }

  .description p {
    margin: 0;
  }

  .description p + p {
    margin-top: 0.75em;
  }

  /* Each fact is its label over its value, on a quiet panel beside the
     text. */
  .meta {
    flex: 0 1 220px;
    min-width: 180px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    margin: 0;
    padding: 14px 16px;
    border-radius: var(--r-row);
    background: var(--surface);
    border: 1px solid var(--line);
  }

  .meta dt {
    margin-bottom: 2px;
    color: var(--text-muted);
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .meta dd {
    margin: 0;
    /* One unbreakable company or franchise token must not widen the panel. */
    min-width: 0;
    overflow-wrap: anywhere;
    color: var(--text);
    font-size: 13px;
    line-height: 1.4;
  }

  .empty {
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
