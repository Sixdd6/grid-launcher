<script lang="ts">
  // Library › RomM › Emulators (RommStorefront README "RomM › Emulators";
  // prototype romm_emu.js): the server's Emulators platform as rows, not
  // covers. Emulators named for GRID install through the server-emulator
  // path and register; Other files install as plain files in the library's
  // Emulators folder; the builds GRID doesn't install here sit in a closed
  // disclosure. The platform header minus what doesn't apply (no firmware
  // or emulator chip, Backup all, Select, Size, Sort).
  import { untrack } from 'svelte';
  import type { RommEmulator, RommOtherFile } from '../../api';
  import FoldingHeader from '../../FoldingHeader.svelte';
  import Icon from '../../Icon.svelte';
  import { downloads } from '../../stores/downloads.svelte';
  import { navigation } from '../../stores/navigation.svelte';
  import { sizeText } from '../../backups/dialog';
  import RommMark from '../RommMark.svelte';
  import {
    BUILDS_CHIP_TITLE,
    EMULATORS_NOTE,
    EMULATORS_SEARCH,
    EMULATORS_SEARCH_FOLDED,
    OTHER_FILES_NOTE,
    cantLoadText,
    catalogCopyLine,
    downloadingLine,
    emulatorMatches,
    emulatorMetaLine,
    emulatorRowState,
    emulatorUninstallConfirm,
    emulatorsCount,
    emulatorsCountsLine,
    loadingText,
    noMatchText,
    nothingHereText,
    otherBuildsNote,
    otherBuildsSummary,
    otherFileLine,
    otherFileMatches,
    otherFileRowState,
    otherFileSourceId,
    otherFileUninstallConfirm,
    otherReasonText,
    versionText,
    wrongMachineLine,
    type EmuRowState,
  } from '../emulators';
  import {
    cancelRow,
    installEmulator,
    installOtherFile,
    loadEmulators,
    openEmulatorsFolder,
    openPackagingGuide,
    rommEmulators,
    uninstallEmulator,
    uninstallOtherFile,
  } from '../emulators.svelte';

  let {
    host,
    server,
  }: {
    /** The server's host, for the panels ("romm.home.lan"). */
    host: string;
    /** The connected server's URL: the list's key. */
    server: string;
  } = $props();

  let search = $state('');
  let folded = $state(false);
  let scrollEl = $state<HTMLElement | null>(null);
  /** The row whose inline Uninstall confirm is open, by key. */
  let confirming = $state<string | null>(null);

  // A fresh read each time the view opens: Settings › Systems may have
  // removed or updated an emulator since.
  $effect(() => {
    const s = server;
    untrack(() => void loadEmulators(s));
  });

  let load = $derived(rommEmulators.load ?? { phase: 'loading' as const });
  let view = $derived(load.phase === 'ready' ? load.data.view : null);
  let hostLabel = $derived(view?.host ?? '');
  let nothing = $derived(load.phase === 'ready' && emulatorsCount(view) === 0);
  let emus = $derived((view?.emulators ?? []).filter((e) => emulatorMatches(e, search)));
  let others = $derived((view?.other_files ?? []).filter((f) => otherFileMatches(f, search)));
  let searching = $derived(search.trim() !== '');
  let entries = $derived(downloads.entries);

  function openSystems(e: RommEmulator | null) {
    navigation.goSettings('systems', e !== null && !e.many_systems && e.system_slug !== null ? { system: e.systems } : {});
  }

  function emuKey(e: RommEmulator): string {
    return `emu:${e.source_id}`;
  }

  function fileKey(f: RommOtherFile): string {
    return `file:${f.rom_id}/${f.file_id}`;
  }

  /** `romm-emu-pcsx2`, from the emulator's name. */
  function emuTestId(e: RommEmulator): string {
    return `romm-emu-${e.name.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '')}`;
  }

  function progressText(s: EmuRowState, total: number): string {
    return s.kind === 'downloading' ? downloadingLine(s.pct, s.done, s.total > 0 ? s.total : total) : '';
  }
</script>

<div data-testid="romm-emulators-view" class="body">
  <FoldingHeader testId="server-head" scroller={scrollEl} resetKey="emulators" bind:folded>
    {#snippet title()}
      <header data-testid="romm-emulators-header" class="emu-header">
        <h2>Emulators</h2>
        <p data-testid="romm-emulators-counts" class="counts">{emulatorsCountsLine(load, host)}</p>
        <div class="chips">
          {#if hostLabel !== ''}
            <span data-testid="romm-emulators-builds-chip" class="chip" title={BUILDS_CHIP_TITLE}>Builds for {hostLabel}</span>
          {/if}
          <button data-testid="romm-emulators-systems-chip" class="chip link" onclick={() => openSystems(null)}>Settings › Systems</button>
        </div>
      </header>
    {/snippet}
    {#snippet toolbar()}
      <div class="toolbar">
        <input
          data-testid="romm-emulators-search"
          class="search"
          type="search"
          placeholder={folded ? EMULATORS_SEARCH_FOLDED : EMULATORS_SEARCH}
          aria-label="Search emulators or files"
          bind:value={search}
        />
      </div>
    {/snippet}
  </FoldingHeader>

  <div class="scroller" bind:this={scrollEl}>
    <div class="wrap">
      {#if load.phase === 'loading'}
        <div data-testid="romm-emulators-loading" class="panel sec">
          <h3><span class="spin" aria-hidden="true"></span> {loadingText(host)}</h3>
          <div class="skel" aria-hidden="true"><i></i><i></i><i></i><i></i></div>
        </div>
      {:else if load.phase === 'error'}
        <div data-testid="romm-emulators-error" class="state warn">
          <span class="ico"><RommMark size="lg" /></span>
          <h3>Can’t load the Emulators platform</h3>
          <p>{cantLoadText(host)}</p>
          <button data-testid="romm-emulators-retry" class="g-btn secondary" onclick={() => loadEmulators(server, true)}>Retry</button>
        </div>
      {:else if nothing}
        <div data-testid="romm-emulators-nothing" class="state">
          <span class="ico"><RommMark size="lg" /></span>
          <h3>Nothing here for this computer</h3>
          <p>{nothingHereText(view?.other_builds.length ?? 0, hostLabel)}</p>
          <button data-testid="romm-emulators-guide" class="g-btn secondary" onclick={openPackagingGuide}>How to name builds</button>
        </div>
        {@render builds()}
      {:else if searching && emus.length === 0 && others.length === 0}
        <p data-testid="romm-emulators-no-match" class="panel empty">{noMatchText(search)}</p>
      {:else}
        {#if emus.length > 0}
          <section data-testid="romm-emulators-emulators" class="panel sec">
            <h3>Emulators</h3>
            <p class="note">{EMULATORS_NOTE}</p>
            <ul class="list">
              {#each emus as e (e.source_id)}
                {@const s = emulatorRowState(e, entries)}
                {@const confirm = emulatorUninstallConfirm(e)}
                <li data-testid={emuTestId(e)} data-state={s.kind} class="row" class:bad={s.kind === 'failed'}>
                  <span class="tile" aria-hidden="true">{e.name.charAt(0)}</span>
                  <div class="main">
                    <div class="t"><b>{e.name}</b><span class="sys">{e.systems}</span></div>
                    <p class="line">{emulatorMetaLine(e)}</p>
                    <p class="line"><code>{e.file_name}</code></p>
                    {#if s.kind === 'none' && e.catalog_copy}
                      <p data-testid="romm-emu-catalog-copy" class="line">{catalogCopyLine(e)}</p>
                    {:else if s.kind === 'installed'}
                      <p class="line">Set up in Settings › Systems · <button class="linkish" onclick={() => openSystems(e)}>Open in Systems</button></p>
                    {:else if s.kind === 'update'}
                      <p class="line">
                        <b class="upd">Installed: {versionText(e.installed_version ?? '')}</b> · {versionText(e.version)} is on RomM ·
                        <button class="linkish" onclick={() => openSystems(e)}>Open in Systems</button>
                      </p>
                    {:else if s.kind === 'failed'}
                      <p data-testid="romm-emu-error" class="err" role="alert">
                        {s.builtFor !== null ? wrongMachineLine(s.builtFor, hostLabel) : s.message || 'Couldn’t install.'}
                      </p>
                    {:else if s.kind === 'queued'}
                      <p class="line">Queued · waits for the download before it</p>
                    {:else if s.kind === 'downloading'}
                      <div class="prog">
                        <span>{progressText(s, e.size_bytes)}</span>
                        <div class="bar"><b style:width={`${s.pct}%`}></b></div>
                      </div>
                    {:else if s.kind === 'installing'}
                      <div class="prog">
                        <span>Installing · checking the program</span>
                        <div class="bar"><b class="indet"></b></div>
                      </div>
                    {/if}
                    {#if confirming === emuKey(e)}
                      <div data-testid="romm-emu-confirm" class="confirm" role="alertdialog" aria-label={confirm.title}>
                        <b>{confirm.title}</b>
                        <p>{confirm.body}</p>
                        <div class="g-actions">
                          <button
                            data-testid="romm-emu-confirm-uninstall"
                            class="small-btn danger"
                            onclick={() => {
                              confirming = null;
                              void uninstallEmulator(e, server);
                            }}>Uninstall</button
                          >
                          <button class="small-btn" onclick={() => (confirming = null)}>Cancel</button>
                        </div>
                      </div>
                    {/if}
                  </div>
                  <div class="acts">
                    {#if s.kind === 'none'}
                      <button data-testid="romm-emu-install" class="small-btn primary" onclick={() => installEmulator(e, entries)}>Install</button>
                    {:else if s.kind === 'installed'}
                      <span class="pill ok"><Icon name="check" size={12} />Installed</span>
                      <button data-testid="romm-emu-uninstall" class="small-btn" onclick={() => (confirming = emuKey(e))}>Uninstall…</button>
                    {:else if s.kind === 'update'}
                      <button data-testid="romm-emu-update" class="small-btn primary" onclick={() => installEmulator(e, entries)}>
                        Update to {versionText(e.version)}
                      </button>
                      <button data-testid="romm-emu-uninstall" class="small-btn" onclick={() => (confirming = emuKey(e))}>Uninstall…</button>
                    {:else if s.kind === 'failed'}
                      <button data-testid="romm-emu-retry" class="small-btn" onclick={() => installEmulator(e, entries)}>Retry</button>
                    {:else if s.kind === 'queued' || s.kind === 'downloading'}
                      <button data-testid="romm-emu-cancel" class="small-btn" onclick={() => cancelRow(e.source_id, e.name, entries)}>Cancel</button>
                    {/if}
                  </div>
                </li>
              {/each}
            </ul>
          </section>
        {/if}
        {#if others.length > 0}
          <section data-testid="romm-emulators-other-files" class="panel sec">
            <h3>Other files</h3>
            <p class="note">
              {OTHER_FILES_NOTE}
              <button data-testid="romm-emulators-guide" class="linkish" onclick={openPackagingGuide}>How to name builds</button>
            </p>
            <ul class="list">
              {#each others as f (fileKey(f))}
                {@const s = otherFileRowState(f, entries)}
                {@const confirm = otherFileUninstallConfirm(f)}
                <li data-testid={`romm-emu-file-${f.file_id}`} data-state={s.kind} class="row" class:bad={s.kind === 'failed'}>
                  <span class="tile file" aria-hidden="true">·</span>
                  <div class="main">
                    <div class="t"><b class="file-name">{f.file_name}</b></div>
                    <p class="line">{otherFileLine(f)}</p>
                    {#if s.kind === 'installed'}
                      <p class="line">
                        In your library’s Emulators folder ·
                        <button data-testid="romm-emu-show-folder" class="linkish" onclick={openEmulatorsFolder}>Show folder</button>
                      </p>
                    {:else if s.kind === 'failed'}
                      <p data-testid="romm-emu-error" class="err" role="alert">{s.message || 'Couldn’t install.'}</p>
                    {:else if s.kind === 'queued'}
                      <p class="line">Queued · waits for the download before it</p>
                    {:else if s.kind === 'downloading'}
                      <div class="prog">
                        <span>{progressText(s, f.size_bytes)}</span>
                        <div class="bar"><b style:width={`${s.pct}%`}></b></div>
                      </div>
                    {:else if s.kind === 'installing'}
                      <div class="prog">
                        <span>Installing · checking the program</span>
                        <div class="bar"><b class="indet"></b></div>
                      </div>
                    {/if}
                    {#if confirming === fileKey(f)}
                      <div data-testid="romm-emu-confirm" class="confirm" role="alertdialog" aria-label={confirm.title}>
                        <b>{confirm.title}</b>
                        <p>{confirm.body}</p>
                        <div class="g-actions">
                          <button
                            data-testid="romm-emu-confirm-uninstall"
                            class="small-btn danger"
                            onclick={() => {
                              confirming = null;
                              void uninstallOtherFile(f, server);
                            }}>Uninstall</button
                          >
                          <button class="small-btn" onclick={() => (confirming = null)}>Cancel</button>
                        </div>
                      </div>
                    {/if}
                  </div>
                  <div class="acts">
                    {#if s.kind === 'none'}
                      <button data-testid="romm-emu-install" class="small-btn" onclick={() => installOtherFile(f, entries)}>Install</button>
                    {:else if s.kind === 'installed' || s.kind === 'update'}
                      <span class="pill ok"><Icon name="check" size={12} />Installed</span>
                      <button data-testid="romm-emu-uninstall" class="small-btn" onclick={() => (confirming = fileKey(f))}>Uninstall…</button>
                    {:else if s.kind === 'failed'}
                      <button data-testid="romm-emu-retry" class="small-btn" onclick={() => installOtherFile(f, entries)}>Retry</button>
                    {:else if s.kind === 'queued' || s.kind === 'downloading'}
                      <button data-testid="romm-emu-cancel" class="small-btn" onclick={() => cancelRow(otherFileSourceId(f), f.file_name, entries)}>
                        Cancel
                      </button>
                    {/if}
                  </div>
                </li>
              {/each}
            </ul>
          </section>
        {/if}
        {#if !searching}{@render builds()}{/if}
      {/if}
    </div>
  </div>
</div>

{#snippet builds()}
  {#if view !== null && view.other_builds.length > 0}
    <section class="panel sec">
      <details data-testid="romm-emulators-builds" class="more">
        <summary>{otherBuildsSummary(view.other_builds.length)}</summary>
        <p class="note">{otherBuildsNote(hostLabel)}</p>
        <ul class="list">
          {#each view.other_builds as b (`${b.folder}/${b.file_name}`)}
            <li class="row dim">
              <span class="tile file" aria-hidden="true">·</span>
              <div class="main">
                <div class="t"><b class="file-name">{b.file_name}</b></div>
                <p class="line">{b.folder} folder · {sizeText(b.size_bytes)} · {otherReasonText(b.reason)}</p>
              </div>
            </li>
          {/each}
        </ul>
      </details>
    </section>
  {/if}
{/snippet}

<style>
  .body {
    position: relative;
    flex: 1 1 auto;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .scroller {
    flex: 1 1 auto;
    min-height: 0;
    overflow-y: auto;
    box-sizing: border-box;
    padding-top: var(--head-h, 116px);
  }

  .wrap {
    display: flex;
    flex-direction: column;
    gap: var(--gutter);
    padding: var(--gutter) 0;
  }

  /* The platform header's title row (RommPlatformHeader). */
  .emu-header {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 8px 14px;
  }

  .emu-header h2 {
    margin: 0;
    font-size: 20px;
    font-weight: 600;
    color: var(--text-h);
  }

  .counts {
    margin: 0;
    font-size: 12px;
    color: var(--text-muted);
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin-left: auto;
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font: inherit;
    font-size: 11px;
    padding: 4px 10px;
    border-radius: var(--r-pill);
    border: 1px solid var(--line-strong);
    background: transparent;
    color: var(--text-muted);
  }

  .chip.link {
    cursor: pointer;
  }

  .chip.link:hover {
    color: var(--text-h);
    border-color: var(--primary);
  }

  .toolbar {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .search {
    flex: 1 1 240px;
    min-width: 120px;
    font: inherit;
    height: var(--control-h);
    box-sizing: border-box;
    padding: 0 10px;
    border-radius: var(--r-control);
    border: 1px solid var(--line-strong);
    background: var(--field);
    color: var(--text-h);
  }

  /* Prototype romm_emu.css. */
  .sec {
    padding: 14px 16px;
  }

  .sec h3 {
    margin: 0 0 2px;
    font-size: 15px;
    color: var(--text-h);
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .sec > .note,
  .more > .note {
    margin: 0 0 10px;
    max-width: 720px;
    font-size: 13px;
    color: var(--text-muted);
  }

  .more > .note {
    margin: 6px 0 8px;
  }

  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .row {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    padding: 10px 12px;
    border-radius: var(--r-row);
    background: var(--surface);
  }

  .row.dim {
    opacity: 0.72;
  }

  .row.bad {
    box-shadow: inset 3px 0 0 var(--danger);
  }

  .tile {
    flex: none;
    width: 34px;
    height: 34px;
    border-radius: 8px;
    display: grid;
    place-items: center;
    font-weight: 700;
    font-size: 15px;
    color: var(--text-h);
    background: color-mix(in srgb, var(--primary) 16%, var(--panel));
    border: 1px solid var(--line);
  }

  .tile.file {
    background: var(--panel);
    color: var(--text-muted);
    font-size: 20px;
  }

  .main {
    flex: 1 1 auto;
    min-width: 0;
  }

  .t {
    display: flex;
    align-items: baseline;
    gap: 10px;
    flex-wrap: wrap;
  }

  .t b {
    color: var(--text-h);
    font-weight: 600;
    font-size: 14px;
  }

  .t b.file-name {
    font-family: ui-monospace, monospace;
    font-size: 13px;
    font-weight: 500;
    word-break: break-all;
  }

  .sys {
    font-size: 12px;
    color: var(--text-muted);
  }

  .line {
    margin: 3px 0 0;
    font-size: 12px;
    color: var(--text-muted);
  }

  .line code {
    font-size: 11.5px;
    word-break: break-all;
  }

  .upd {
    color: var(--text-h);
    font-weight: 600;
  }

  .err {
    margin: 6px 0 0;
    font-size: 12.5px;
    color: var(--danger);
    max-width: 640px;
  }

  .acts {
    flex: none;
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .prog {
    margin-top: 6px;
    max-width: 420px;
    font-size: 12px;
    color: var(--text-muted);
  }

  .bar {
    margin-top: 4px;
    height: 4px;
    border-radius: var(--r-pill);
    background: var(--field);
    overflow: hidden;
  }

  .bar b {
    display: block;
    height: 100%;
    background: var(--primary);
    transition: width 0.3s;
  }

  .bar b.indet {
    width: 35%;
    animation: indet 1.1s ease-in-out infinite alternate;
  }

  @keyframes indet {
    from {
      margin-left: 0;
    }
    to {
      margin-left: 65%;
    }
  }

  .confirm {
    margin-top: 8px;
    padding: 10px 12px;
    border-radius: var(--r-row);
    border: 1px solid var(--line);
    border-left: 3px solid var(--danger);
    background: var(--panel);
    max-width: 640px;
  }

  .confirm b {
    color: var(--text-h);
  }

  .confirm p {
    margin: 2px 0 8px;
    font-size: 12.5px;
    color: var(--text-muted);
  }

  .confirm .g-actions {
    gap: 6px;
  }

  .small-btn {
    font: inherit;
    font-size: 12px;
    padding: 5px 10px;
    border-radius: var(--r-chip);
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text-h);
    cursor: pointer;
    white-space: nowrap;
  }

  .small-btn:hover {
    border-color: var(--primary);
  }

  .small-btn.primary {
    background: var(--primary);
    border-color: var(--primary);
    color: var(--on-primary);
  }

  .small-btn.danger {
    background: var(--danger);
    border-color: var(--danger);
    color: #fff;
  }

  .pill {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 12px;
    font-weight: 600;
    padding: 2px 10px;
    border-radius: var(--r-pill);
    color: var(--text-muted);
    border: 1px solid var(--line);
    white-space: nowrap;
  }

  .pill.ok {
    color: var(--text-h);
  }

  .pill.ok :global(svg) {
    color: var(--primary);
  }

  .linkish {
    font: inherit;
    padding: 0;
    border: 0;
    background: none;
    color: var(--primary);
    cursor: pointer;
    text-decoration: underline;
    text-underline-offset: 2px;
  }

  .more summary {
    cursor: pointer;
    font-size: 13px;
    font-weight: 600;
    color: var(--text-h);
  }

  .skel {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-top: 10px;
  }

  .skel i {
    display: block;
    height: 52px;
    border-radius: var(--r-row);
    background: var(--surface);
    animation: pulse 1.2s ease-in-out infinite alternate;
  }

  @keyframes pulse {
    from {
      opacity: 0.5;
    }
    to {
      opacity: 1;
    }
  }

  .empty {
    align-self: flex-start;
    margin: 0;
    padding: 10px 14px;
    color: var(--text-muted);
    font-size: 14px;
  }

  /* Prototype `stState`, as RommServerView's connecting panel. */
  .state {
    align-self: stretch;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    padding: 34px 24px;
    text-align: center;
    background: var(--panel);
    -webkit-backdrop-filter: var(--panel-blur);
    backdrop-filter: var(--panel-blur);
    border: 1px solid var(--line);
    border-radius: var(--r-panel);
  }

  .state.warn {
    border-color: var(--warning);
  }

  .state h3 {
    margin: 0;
    font-size: 16px;
    color: var(--text-h);
  }

  .state p {
    margin: 0;
    max-width: 480px;
    color: var(--text-muted);
    font-size: 13px;
  }

  .ico {
    display: inline-grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border-radius: var(--r-row);
    background: var(--field);
  }

  .spin {
    display: inline-block;
    width: 10px;
    height: 10px;
    box-sizing: border-box;
    border-radius: 50%;
    border: 2px solid var(--line-strong);
    border-top-color: var(--primary);
    animation: spin 800ms linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .spin,
    .bar b.indet,
    .skel i {
      animation: none;
    }
  }
</style>
