<script lang="ts">
  // Library › RomM › Emulators in TV mode (TVRommStorefront README "RomM ›
  // Emulators"; prototype tvromm_emu.js): the header ("Emulators", the
  // counts, "Builds for Linux x64", the search chip, (Y) Search), then a
  // vertical list of rows: EMULATORS, OTHER FILES and NOT FOR THIS COMPUTER
  // (one Other builds row that opens a page). The rules and words are
  // desktop's (`romm/emulators.ts`); A and X act on the focused row.
  import { untrack } from 'svelte';
  import type { RommEmulator, RommOtherFile } from '../../../../api';
  import {
    EMULATORS_NOTE,
    OTHER_FILES_NOTE,
    TV_OTHER_BUILDS_ROW,
    cantLoadText,
    emulatorMatches,
    emulatorRowState,
    emulatorUninstallConfirm,
    emulatorsCount,
    emulatorsCountsLine,
    emulatorsMatchLine,
    loadingText,
    noMatchText,
    nothingHereText,
    otherFileLine,
    otherFileMatches,
    otherFileRowState,
    otherFileSourceId,
    otherFileUninstallConfirm,
    tvCancelConfirm,
    tvCatalogCopyLine,
    tvEmulatorLine,
    tvInstallingWhy,
    tvWrongMachineLine,
    versionText,
    type EmuRowState,
  } from '../../../../romm/emulators';
  import {
    cancelRow,
    installEmulator,
    installOtherFile,
    loadEmulators,
    rommEmulators,
    uninstallEmulator,
    uninstallOtherFile,
  } from '../../../../romm/emulators.svelte';
  import { rommHost } from '../../../../romm/words';
  import { downloads } from '../../../../stores/downloads.svelte';
  import { navigation } from '../../../../stores/navigation.svelte';
  import { romm } from '../../../../stores/romm.svelte';
  import { tvf } from '../../../focus/actions';
  import { openSystem } from '../../../gameActions';
  import Glyph from '../../../glyphs/Glyph.svelte';
  import { pushPage } from '../../../nav.svelte';
  import { confirmBox } from '../../../overlays/overlays.svelte';
  import { tvRomm } from '../../../romm/tvRomm.svelte';
  import TvButton from '../../../ui/TvButton.svelte';
  import TvChip from '../../../ui/TvChip.svelte';
  import TvIcon from '../../../ui/TvIcon.svelte';
  import TvRommOtherBuilds from './TvRommOtherBuilds.svelte';

  let server = $derived(romm.status.server_url);
  let host = $derived(rommHost(romm.status));

  // A fresh read each time the row opens (Settings › Systems may have
  // changed an emulator since).
  $effect(() => {
    const s = server;
    untrack(() => void loadEmulators(s));
  });

  let load = $derived(rommEmulators.load ?? { phase: 'loading' as const });
  let view = $derived(load.phase === 'ready' ? load.data.view : null);
  let hostLabel = $derived(view?.host ?? '');
  let query = $derived(tvRomm.query.trim());
  let emus = $derived((view?.emulators ?? []).filter((e) => emulatorMatches(e, query)));
  let others = $derived((view?.other_files ?? []).filter((f) => otherFileMatches(f, query)));
  let total = $derived(emulatorsCount(view));
  let nothing = $derived(load.phase === 'ready' && total === 0);
  let entries = $derived(downloads.entries);

  let counts = $derived(
    load.phase === 'ready' && query !== '' && total > 0 ? emulatorsMatchLine(emus.length + others.length, total) : emulatorsCountsLine(load, host),
  );

  function confirmCancel(sourceId: string, label: string) {
    const c = tvCancelConfirm(label);
    void confirmBox({ ...c, ok: 'Cancel install', danger: true, onOk: () => void cancelRow(sourceId, label, entries) });
  }

  function emuA(e: RommEmulator, s: EmuRowState) {
    switch (s.kind) {
      case 'none':
      case 'update':
      case 'failed':
        void installEmulator(e, entries);
        return;
      case 'queued':
      case 'downloading':
        confirmCancel(e.source_id, e.name);
        return;
      case 'installed':
        // The system's page; a many-systems emulator opens Settings › Systems.
        if (!e.many_systems && e.system_slug !== null) openSystem(e.systems);
        else navigation.goSettings('systems');
        return;
      default:
        return;
    }
  }

  function emuUninstall(e: RommEmulator) {
    const c = emulatorUninstallConfirm(e);
    void confirmBox({ ...c, ok: 'Uninstall', danger: true, onOk: () => void uninstallEmulator(e, server) });
  }

  function fileA(f: RommOtherFile, s: EmuRowState) {
    switch (s.kind) {
      case 'none':
      case 'failed':
        void installOtherFile(f, entries);
        return;
      case 'queued':
      case 'downloading':
        confirmCancel(otherFileSourceId(f), f.file_name);
        return;
      case 'installed': {
        const c = otherFileUninstallConfirm(f);
        void confirmBox({ ...c, ok: 'Uninstall', danger: true, onOk: () => void uninstallOtherFile(f, server) });
        return;
      }
      default:
        return;
    }
  }

  /** The A hint for a row in state `s`. */
  function emuAl(e: RommEmulator, s: EmuRowState): string | undefined {
    switch (s.kind) {
      case 'none':
        return 'Install';
      case 'update':
        return `Update to ${versionText(e.version)}`;
      case 'failed':
        return 'Retry';
      case 'queued':
      case 'downloading':
        return 'Cancel';
      case 'installed':
        return 'Open in Systems';
      default:
        return undefined;
    }
  }

  function fileAl(s: EmuRowState): string | undefined {
    switch (s.kind) {
      case 'none':
        return 'Install';
      case 'failed':
        return 'Retry';
      case 'queued':
      case 'downloading':
        return 'Cancel';
      case 'installed':
        return 'Uninstall';
      default:
        return undefined;
    }
  }

  function openBuilds() {
    void pushPage({ title: 'Other builds', component: TvRommOtherBuilds, props: {} });
  }

  const slug = (t: string) => t.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '');
  const emuK = (e: RommEmulator) => `re-${slug(e.folder)}-${slug(e.name)}`;
  const fileK = (f: RommOtherFile) => `re-f${f.rom_id}-${f.file_id}`;
  const emuTid = (e: RommEmulator) => `tv-romm-emu-${slug(e.name)}`;
</script>

<div class="vh two" id="lib-h" data-testid="tv-romm-emulators-header">
  <div class="vh-top">
    <h1 data-testid="tv-lib-name">Emulators</h1>
    <span class="cnt" data-testid="tv-lib-count">{counts}</span>
  </div>
  <div class="vh-row">
    {#if hostLabel !== ''}<TvChip testId="tv-romm-emu-host">Builds for <b>{hostLabel}</b></TvChip>{/if}
    {#if query !== ''}<TvChip testId="tv-lib-search">Search: <b>{query}</b></TvChip>{/if}
    <span class="sp"></span>
    {#if load.phase === 'ready'}<TvChip glyph="Y">Search</TvChip>{/if}
  </div>
</div>
<div class="scroll" data-scroll="lib-grid">
  <div class="tre-list" id="lib-grid" data-nav="v" data-enter="first" data-testid="tv-romm-emulators">
    {#if load.phase === 'loading'}
      <div class="panel rconnecting" data-testid="tv-romm-emu-loading"><h2><span class="sspin"></span>{loadingText(host)}</h2></div>
      <div class="tre-skel" aria-hidden="true"><i></i><i></i><i></i><i></i></div>
    {:else if load.phase === 'error'}
      <div class="panel tre-panel" data-testid="tv-romm-emu-error">
        <h2>Can’t load the Emulators platform</h2>
        <p class="note">{cantLoadText(host)}</p>
        <div class="btns" data-nav="h">
          <TvButton k="re-retry" al="Retry" testId="tv-romm-emu-retry" onA={() => void loadEmulators(server, true)}>Retry</TvButton>
        </div>
      </div>
    {:else if nothing}
      <div class="panel tre-panel" data-testid="tv-romm-emu-nothing">
        <h2>Nothing here for this computer</h2>
        <p class="note">{nothingHereText(view?.other_builds.length ?? 0, hostLabel)}</p>
      </div>
      {@render buildsRow()}
    {:else if query !== '' && emus.length === 0 && others.length === 0}
      <div class="empty" data-testid="tv-lib-empty">
        <h2>{noMatchText(query)}</h2>
        Press <Glyph button="Y" /> to change the search.
      </div>
    {:else}
      {#if emus.length > 0}
        <div class="sgroup">Emulators</div>
        <p class="note tre-note">{EMULATORS_NOTE}</p>
        {#each emus as e (e.source_id)}
          {@const s = emulatorRowState(e, entries)}
          {@const busy = s.kind === 'installing'}
          <div
            class="srow"
            class:warnrow={s.kind === 'failed'}
            data-k={emuK(e)}
            data-al={emuAl(e, s)}
            data-xl={s.kind === 'installed' || s.kind === 'update' ? 'Uninstall' : undefined}
            aria-disabled={busy ? 'true' : undefined}
            data-why={busy ? tvInstallingWhy(e.name) : undefined}
            data-state={s.kind}
            data-testid={emuTid(e)}
            use:tvf={{ a: () => emuA(e, s), x: s.kind === 'installed' || s.kind === 'update' ? () => emuUninstall(e) : undefined }}
          >
            <span class="tre-tile">{e.name.charAt(0)}</span>
            <div class="lab">
              <div class="t">{e.name}</div>
              <div class="s">
                {tvEmulatorLine(e, s)}
                {#if s.kind === 'none' && e.catalog_copy}<br />{tvCatalogCopyLine(e)}{/if}
                {#if s.kind === 'failed'}<br /><span class="tre-err" data-testid="tv-romm-emu-error-line"
                    >{s.builtFor !== null ? tvWrongMachineLine(s.builtFor, hostLabel) : s.message}</span
                  >{/if}
                {#if s.kind === 'downloading'}<span class="tre-bar"><b style:width={`${s.pct}%`}></b></span>{/if}
              </div>
            </div>
            <div class="val">
              {#if s.kind === 'installed'}<span class="st ok">Installed</span>
              {:else if s.kind === 'update'}<span class="st warn">Update available</span>
              {:else if s.kind === 'failed'}<span class="st warn">Didn’t install</span>
              {:else if s.kind === 'queued'}<span class="st">Queued</span>
              {:else if s.kind === 'downloading'}<span class="st">{s.pct}%</span>
              {:else if s.kind === 'installing'}<span class="st"><span class="sspin"></span> Installing…</span>{/if}
            </div>
          </div>
        {/each}
      {/if}
      {#if others.length > 0}
        <div class="sgroup">Other files</div>
        <p class="note tre-note">{OTHER_FILES_NOTE}</p>
        {#each others as f (fileK(f))}
          {@const s = otherFileRowState(f, entries)}
          {@const busy = s.kind === 'installing'}
          <div
            class="srow"
            class:warnrow={s.kind === 'failed'}
            data-k={fileK(f)}
            data-al={fileAl(s)}
            aria-disabled={busy ? 'true' : undefined}
            data-why={busy ? tvInstallingWhy(f.file_name) : undefined}
            data-state={s.kind}
            data-testid={`tv-romm-emu-file-${f.file_id}`}
            use:tvf={{ a: () => fileA(f, s) }}
          >
            <span class="tre-tile">·</span>
            <div class="lab">
              <div class="t"><span class="tre-file">{f.file_name}</span></div>
              <div class="s">
                {otherFileLine(f)}
                {#if s.kind === 'installed'}<br />In your library’s Emulators folder{/if}
                {#if s.kind === 'failed'}<br /><span class="tre-err">{s.message}</span>{/if}
                {#if s.kind === 'downloading'}<span class="tre-bar"><b style:width={`${s.pct}%`}></b></span>{/if}
              </div>
            </div>
            <div class="val">
              {#if s.kind === 'installed'}<span class="st ok">Installed</span>
              {:else if s.kind === 'failed'}<span class="st warn">Didn’t install</span>
              {:else if s.kind === 'queued'}<span class="st">Queued</span>
              {:else if s.kind === 'downloading'}<span class="st">{s.pct}%</span>
              {:else if s.kind === 'installing'}<span class="st"><span class="sspin"></span> Installing…</span>{/if}
            </div>
          </div>
        {/each}
      {/if}
      {#if query === '' && (view?.other_builds.length ?? 0) > 0}
        <div class="sgroup">Not for this computer</div>
        {@render buildsRow()}
      {/if}
    {/if}
  </div>
</div>

{#snippet buildsRow()}
  {#if view !== null && view.other_builds.length > 0}
    <div class="srow" data-k="re-builds" data-al="Open" data-testid="tv-romm-emu-builds" use:tvf={{ a: openBuilds }}>
      <span class="tre-tile">·</span>
      <div class="lab">
        <div class="t">{TV_OTHER_BUILDS_ROW.label}</div>
        <div class="s">{TV_OTHER_BUILDS_ROW.sub}</div>
      </div>
      <div class="val">{view.other_builds.length}<span class="chev"><TvIcon name="chevronRight" px={18} /></span></div>
    </div>
  {/if}
{/snippet}
