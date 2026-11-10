<script lang="ts">
  // Downloads (checklist items 209–224; tvmode.js `TABS.downloads`,
  // `ACT.dlrow`): the Active, Queued and Completed panels in a centred
  // column, each heading with its count. A row shows its title and kind
  // badge, "<Platform> · <status>", a bar and the network / disk sparkline
  // while active, and the speed. A opens the row's actions (Cancel, asked
  // first for every kind of row — ruling 8; Retry; Dismiss; Open <title>),
  // Y clears the completed rows. Progress redraws keep focus by row key, and
  // a row that leaves gives focus to the one now in its place (the list's
  // `id`). B does nothing here (item 224: a tab's top level).
  import type { DownloadEntry } from '../../../api';
  import { fromInstalled, type DetailsSubject } from '../../../details/subject';
  import { cancelEntry, dismissAll, dismissEntry, retryEntry } from '../../../downloads/actions';
  import { sparklinePaths } from '../../../downloads/sparkline';
  import { downloads } from '../../../stores/downloads.svelte';
  import { installed } from '../../../stores/installed.svelte';
  import { libraryMove } from '../../../stores/libraryMove.svelte';
  import { pushToast } from '../../../stores/toasts.svelte';
  import { tvf } from '../../focus/actions';
  import { openDetails } from '../../gameActions';
  import { confirmBox, openList } from '../../overlays/overlays.svelte';
  import { useScreen } from '../../screen.svelte';
  import {
    cancelWords,
    completedIds,
    hasCompleted,
    isFailed,
    kindBadge,
    listSub,
    progress,
    rowA,
    rowActions,
    rowWhy,
    speedText,
    statusLine,
    tvSegments,
    type RowAction,
  } from './downloadRows';

  let segments = $derived(tvSegments(downloads.entries));
  const SPARK = { width: 120, height: 38 };

  function fail(err: unknown) {
    pushToast(err instanceof Error ? err.message : String(err), 'error');
  }

  function subjectOf(e: DownloadEntry): DetailsSubject {
    const row = installed.list.find((r) => r.rom_id === e.rom_id);
    if (row) return fromInstalled(row);
    return {
      romId: e.rom_id,
      name: e.title,
      platformName: e.platform,
      platformSlug: '',
      coverSmall: null,
      coverLarge: null,
      screenshotUrls: [],
      fanartUrls: [],
      description: '',
      rating: '',
      genres: '',
      source: 'server',
    };
  }

  function run(e: DownloadEntry, a: RowAction) {
    switch (a.kind) {
      case 'cancel': {
        const w = cancelWords(e);
        void confirmBox({ ...w, danger: true, onOk: () => void cancelEntry(e).catch(fail) });
        return;
      }
      case 'retry':
        void retryEntry(e).catch(fail);
        return;
      case 'dismiss':
        void dismissEntry(e.id).catch(fail);
        return;
      case 'open':
        openDetails(subjectOf(e));
    }
  }

  function actions(e: DownloadEntry) {
    const acts = rowActions(e, libraryMove.phase);
    void openList({
      title: e.title,
      sub: listSub(e),
      items: acts.map((a) => ({ label: a.label, danger: a.danger, testId: `tv-dl-act-${a.kind}`, onPick: () => run(e, a) })),
    });
  }

  function clearCompleted() {
    const ids = completedIds(downloads.entries);
    if (ids.length) void dismissAll(ids).catch(fail);
  }

  useScreen({
    hints: () => (hasCompleted(downloads.entries) ? [{ button: 'Y', label: 'Clear completed' }] : []),
    y: () => {
      if (hasCompleted(downloads.entries)) clearCompleted();
    },
  });
</script>

<div class="list" data-scroll="list" data-testid="tv-downloads">
  <div class="list-in" id="dl-list">
    {#each segments as s (s.seg)}
      <section class="panel" data-testid={`tv-dl-seg-${s.seg}`}>
        <h2>{s.name}<span data-testid={`tv-dl-count-${s.seg}`}>{s.rows.length}</span></h2>
        <div data-nav="v">
          {#if s.rows.length === 0}
            <p class="note" data-testid={`tv-dl-empty-${s.seg}`}>{s.empty}</p>
          {:else}
            {#each s.rows as e (e.id)}
              {@const a = rowA(e, libraryMove.phase)}
              {@const pct = progress(e)}
              <div
                class="lrow dl"
                data-k={`dl-${e.id}`}
                data-al={a ?? undefined}
                data-why={a === null ? rowWhy(e) : undefined}
                aria-disabled={a === null ? 'true' : undefined}
                data-testid={`tv-dl-row-${e.id}`}
                use:tvf={{ a: () => actions(e) }}
              >
                <div class="grow">
                  <div class="t">{e.title}{#if kindBadge(e)}<span class="kind">{kindBadge(e)}</span>{/if}</div>
                  <div class="s" class:err={isFailed(e)} data-testid={`tv-dl-line-${e.id}`}>{statusLine(e)}</div>
                  {#if s.seg === 'active'}<div class="pbar"><b style={`width:${pct ?? 0}%`}></b></div>{/if}
                </div>
                {#if s.seg === 'active'}
                  {@const paths = sparklinePaths(downloads.samplesFor(e.id), SPARK)}
                  <div class="spark">
                    <svg viewBox="0 0 120 38" preserveAspectRatio="none" aria-hidden="true">
                      <path class="net" d={paths.net} />
                      <path class="disk" d={paths.disk} />
                    </svg>
                  </div>
                {:else}
                  <div></div>
                {/if}
                <div class="s">{speedText(e)}</div>
              </div>
            {/each}
          {/if}
        </div>
      </section>
    {/each}
  </div>
</div>
