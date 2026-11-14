<script lang="ts">
  // Details for a store game (GameStores README, "Details for a store game";
  // prototype `stRenderDetails`, `stCloudHTML`). The RomM Details popup's
  // frame and layout, with none of its RomM sections: no server saves, no
  // "not linked", no Achievements, no Related row. Buttons, lines, chips,
  // tabs and facts come from `gamestores/details.ts`.
  import { untrack } from 'svelte';
  import { api, type DriveStoreGame, type StoreCloudState, type StoreGame, type StoreSource } from '../../api';
  import Icon from '../../Icon.svelte';
  import Image from '../../Image.svelte';
  import { downloads } from '../../stores/downloads.svelte';
  import { drives } from '../../stores/drives.svelte';
  import { gameStores } from '../../stores/gameStores.svelte';
  import { installed } from '../../stores/installed.svelte';
  import { sessions } from '../../stores/sessions.svelte';
  import { compatTools } from '../../stores/compatTools.svelte';
  import {
    cloudPanel,
    descriptionParagraphs,
    dlcRows,
    saveLocationsNote,
    STORE_TAB_LABELS,
    storeButtons,
    storeDetailChips,
    storeFacts,
    storeHeaderLine,
    storeLines,
    storeTabs,
    type PendingConfirm,
    type StoreButton,
    type StoreTab,
  } from '../details';
  import { liveJobFor } from '../jobs';
  import { canBackup, storeName } from '../marks';
  import { parseGameKey } from '../../gameKey';
  import { removedFromDriveNotice, uninstalledNotice } from '../notices';
  import StoreMark from './StoreMark.svelte';
  import ConflictSides from './ConflictSides.svelte';
  import {
    cancelStoreJob,
    installDlc,
    openConflict,
    openGameSettings,
    openInstall,
    openStoreBackup,
    playStore,
    removeDlc,
    repairStore,
    resolveCloud,
    showCard,
    signIn,
    stopStore,
    syncNow,
    uninstallStore,
    updateStore,
    type DetailsOpen,
  } from './ui.svelte';
  import { storeBackupTitle } from '../backup';

  let { open, onClose }: { open: DetailsOpen; onClose: () => void } = $props();

  /** The store a game key names (GOG for a key that names none). */
  function sourceOfKey(key: string): StoreSource {
    const k = parseGameKey(key);
    return k !== null && k.source !== 'romm' ? k.source : 'gog';
  }

  /** A drive game the store doesn't list (signed out): enough to show it. */
  function fromDrive(g: DriveStoreGame): StoreGame {
    return {
      key: g.key,
      source: sourceOfKey(g.key),
      store_id: g.key.slice(g.key.indexOf(':') + 1),
      title: g.name,
      installed: g.installed,
      cover: g.cover,
      screenshots: [],
      description: g.summary,
      developer: '',
      genres: [],
      year: '',
      platforms: [g.platform],
      installed_platform: g.installed ? g.platform : null,
      download_bytes: g.size,
      disk_bytes: g.size,
      version: g.version,
      update_version: '',
      origin: g.installed ? 'drive' : null,
      not_supported: '',
      dlc: [],
      cloud: 'unknown',
      last_sync: 0,
      install_dir: '',
      compat_tool: '',
    };
  }

  let game = $derived(gameStores.game(open.key) ?? (open.drive ? fromDrive(open.drive.game) : null));
  let status = $derived(game ? gameStores.status(game.source) : null);
  let job = $derived(game ? liveJobFor(downloads.entries, game.key) : null);
  let session = $derived(game ? sessions.sessionForKey(game.key) : undefined);
  let playing = $derived(session !== undefined);
  let confirm = $state<PendingConfirm>(null);
  let tab = $state<StoreTab>(untrack(() => open.tab));
  let cloud = $state<StoreCloudState | null>(null);
  let onDrives = $state<string[]>([]);
  let panelEl = $state<HTMLElement | null>(null);
  let now = $state(Math.floor(Date.now() / 1000));

  let tabs = $derived(game ? storeTabs(game) : (['overview', 'media'] as StoreTab[]));
  $effect(() => {
    if (!tabs.includes(tab)) tab = 'overview';
  });

  let lastPlayedAt = $derived.by(() => {
    if (!game) return 0;
    const row = installed.list.find((r) => r.source === game!.source && r.store_id === game!.store_id);
    return row?.last_played_at ?? 0;
  });

  let compat = $derived(compatTools.defaultTool || 'GE-Proton10-17');
  let driveCtx = $derived(open.drive ? { name: open.drive.name, game: open.drive.game } : null);
  let buttons = $derived(game ? storeButtons({ game, status, job, playing, confirm, drive: driveCtx }) : []);
  let lines = $derived(
    game
      ? storeLines(
          { game, status, job, playing, confirm, drive: driveCtx },
          { nowSeconds: now, lastPlayedAt, compat, onDrives, syncing: false },
        )
      : [],
  );
  let chips = $derived(game ? storeDetailChips(game, playing) : []);
  let facts = $derived(game ? storeFacts(game) : []);
  let dlc = $derived(game ? dlcRows(game, downloads.entries) : []);
  let panel = $derived(game ? cloudPanel(game, cloud, now) : null);

  // The cloud state, read when the tab opens (and again after a change).
  $effect(() => {
    if (!game || tab !== 'cloud') return;
    const key = game.key;
    void game.cloud;
    void game.last_sync;
    api
      .storeCloudState(key)
      .then((s) => (cloud = s))
      .catch(() => {});
  });

  // "On Couch kit": the connected drives that hold this GOG game.
  $effect(() => {
    if (!game || !canBackup(game.source) || open.drive) return;
    const key = game.key;
    const list = drives.withPack;
    let cancelled = false;
    Promise.all(
      list.map((d) =>
        api
          .driveStoreGames(d.key)
          .then((gs) => (gs.some((g) => g.key === key) ? [d.pack?.name ?? d.label] : []))
          .catch(() => [] as string[]),
      ),
    ).then((names) => {
      if (!cancelled) onDrives = names.flat();
    });
    return () => {
      cancelled = true;
    };
  });

  // A change to the game takes back a pending two-click button.
  $effect(() => {
    void game?.installed;
    void job;
    confirm = null;
  });

  $effect(() => {
    untrack(() => panelEl?.focus());
  });

  async function act(b: StoreButton) {
    if (!game) return;
    const g = game;
    switch (b.act) {
      case 'play':
        await playStore(g);
        break;
      case 'stop':
        await stopStore(g);
        break;
      case 'install':
        openInstall(g);
        break;
      case 'cancel':
        if (job) await cancelStoreJob(job, g.title);
        break;
      case 'update':
        await updateStore(g);
        break;
      case 'settings':
        openGameSettings(g);
        break;
      case 'backup':
        openStoreBackup([g.key], storeBackupTitle({ kind: 'game', name: g.title }));
        break;
      case 'restore':
        if (open.drive) {
          try {
            await api.installStoreFromDrive(open.drive.key, g.key);
          } catch (err) {
            showCard({ text: `Couldn’t install ${g.title}: ${err instanceof Error ? err.message : String(err)}`, error: true, action: null });
          }
        }
        break;
      case 'remove_drive':
        confirm = 'remove_drive';
        break;
      case 'remove_drive_confirm':
        if (open.drive) {
          try {
            await api.removeStoreFromDrive(open.drive.key, [g.key]);
            showCard(removedFromDriveNotice(g.title, open.drive.name));
            onClose();
          } catch (err) {
            confirm = null;
            showCard({ text: `Couldn’t remove ${g.title}: ${err instanceof Error ? err.message : String(err)}`, error: true, action: null });
          }
        }
        break;
      case 'verify':
        await repairStore(g);
        break;
      case 'uninstall':
        confirm = 'uninstall';
        break;
      case 'uninstall_confirm':
        confirm = null;
        if (await uninstallStore(g)) showCard(uninstalledNotice(g.source, g.title, (status?.cloud_saves ?? false) && g.cloud !== 'none'));
        break;
    }
  }

  async function sync() {
    if (!game) return;
    cloud = cloud ? { ...cloud, state: 'syncing' } : { state: 'syncing', synced_at: 0, locations: [], conflict: null };
    const s = await syncNow(game);
    if (s) {
      cloud = s;
      now = Math.floor(Date.now() / 1000);
      if (s.state === 'synced') showCard({ text: `Cloud saves synced for ${game.title}`, error: false, action: null });
      if (s.state === 'conflict') openConflict(game, s);
    }
  }

  async function keep(choice: 'upload' | 'download') {
    if (!game) return;
    const s = await resolveCloud(game, choice);
    if (s) cloud = s;
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      onClose();
    }
  }

  // A button that goes away after its click (Verify while the job runs,
  // Update once queued, Sync now while syncing) takes the focus with it, to
  // the page body: Escape must still close Details then, as long as no other
  // dialog is open over it.
  function onWindowKey(e: KeyboardEvent) {
    if (e.key !== 'Escape' || e.defaultPrevented || !panelEl) return;
    const active = document.activeElement;
    if (active && active !== document.body && active !== document.documentElement) return;
    const modals = document.querySelectorAll('[aria-modal="true"]');
    if (modals[modals.length - 1] !== panelEl) return;
    e.preventDefault();
    onClose();
  }

  function onBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) onClose();
  }
</script>

<svelte:window onkeydown={onWindowKey} />

<div class="backdrop" onclick={onBackdropClick} role="presentation">
  <div
    data-testid="store-details"
    class="panel"
    bind:this={panelEl}
    role="dialog"
    aria-modal="true"
    aria-label={game?.title ?? 'Details'}
    tabindex="-1"
    onkeydown={onKey}
  >
    <button data-testid="store-details-close" class="close icon-btn" onclick={onClose} aria-label="Close">
      <Icon name="close" size={20} />
    </button>

    {#if game}
      <div class="layout">
        <aside class="left">
          <div class="cover" class:faded={game.not_supported !== ''}>
            <Image url={game.cover || null} alt={game.title} placeholder="No cover" />
          </div>

          <div class="action">
            <!-- Keyed without `_confirm`: the second click's button is the
                 same element, so it keeps focus (and Escape still closes). -->
            {#each buttons as b (b.act.replace(/_confirm$/, ''))}
              <button
                data-testid={`store-details-${b.act.replace(/_/g, '-')}`}
                class:secondary={!b.primary}
                class:destructive={b.destructive && !b.confirm}
                class:confirm={b.confirm}
                disabled={b.disabled}
                title={b.title}
                onclick={() => act(b)}
              >
                {b.label}
              </button>
            {/each}
          </div>

          {#each lines as l (l.testId)}
            <p
              class="meta-line"
              class:why={l.kind === 'why'}
              class:cloud={l.kind === 'cloud' || l.kind === 'cloud-err'}
              class:err={l.kind === 'cloud-err'}
              class:drive={l.kind === 'drive'}
              data-testid={l.testId}
            >
              {#if l.kind === 'cloud' || l.kind === 'cloud-err'}<Icon name="cloud" size={12} />{/if}
              {#if l.kind === 'drive'}<Icon name="drive" size={12} />{/if}
              {l.text}{#if l.link}{l.text.endsWith(' ') ? '' : ' '}<button
                  class="linkish"
                  onclick={() => (l.link?.act === 'sign_in' ? signIn(game!.source) : (tab = 'cloud'))}>{l.link.label}</button
                >{/if}
            </p>
          {/each}
        </aside>

        <section class="right">
          <header class="head">
            <h2>{game.title}</h2>
            <p class="header-line" data-testid="store-details-header-line">
              {#if open.drive}<span class="from-drive">From {open.drive.name}</span>{' · '}{/if}{storeHeaderLine(game)}
            </p>
            <div class="chips">
              {#each chips as c (c.testId)}
                <span class="chip" class:playing={c.kind === 'playing'} class:store={c.kind === 'store'} data-testid={c.testId}>
                  {#if c.kind === 'store'}<StoreMark source={game.source} />{/if}{c.text}
                </span>
              {/each}
            </div>
          </header>

          <div class="tabs" role="tablist">
            {#each tabs as t (t)}
              <button
                role="tab"
                data-testid={`store-details-tab-${t}`}
                class:active={tab === t}
                aria-selected={tab === t}
                onclick={() => (tab = t)}
                onmousedown={(e) => e.preventDefault()}>{STORE_TAB_LABELS[t]}</button
              >
            {/each}
          </div>

          <div class="tabpanel" role="tabpanel">
            {#if tab === 'overview'}
              <div class="overview">
                <div class="desc">
                  {#each descriptionParagraphs(game.description) as p (p)}<p>{p}</p>{/each}
                  {#if dlc.length > 0}
                    <p class="sec-title">DLC you own</p>
                    <ul class="dlc" data-testid="store-details-dlc">
                      {#each dlc as d (d.id)}
                        <li>
                          <span class="t">{d.title}</span>
                          <span class="s">{d.size}{d.installed ? ' · Installed' : ''}</span>
                          {#if d.canAct}
                            {#if d.installed}
                              <button class="small-btn" onclick={() => removeDlc(game!, d.id, d.title)}>Remove</button>
                            {:else}
                              <button class="small-btn primary" disabled={d.busy} onclick={() => installDlc(game!, d.id, d.title)}
                                >{d.busy ? 'Installing…' : 'Install'}</button
                              >
                            {/if}
                          {/if}
                        </li>
                      {/each}
                    </ul>
                  {/if}
                </div>
                <dl class="facts">
                  {#each facts as f (f.term)}
                    <div><dt>{f.term}</dt><dd>{#if f.code}<code>{f.value}</code>{:else}{f.value}{/if}</dd></div>
                  {/each}
                </dl>
              </div>
            {:else if tab === 'media'}
              <p class="sec-title">Screenshots</p>
              {#if game.screenshots.length === 0}
                <p class="note">{storeName(game.source)} has no screenshots for this game.</p>
              {:else}
                <div class="media-row">
                  {#each game.screenshots as s (s)}<span class="shot"><Image url={s} alt="" placeholder="" /></span>{/each}
                </div>
                <p class="note">From {storeName(game.source)}’s store page.</p>
              {/if}
            {:else if tab === 'cloud' && panel}
              <div class="st-cloud" data-testid="store-details-cloud-tab">
                {#if panel.kind === 'conflict'}
                  <div class="cloud-state warn">
                    <span class="cloud-ico"><Icon name="cloud" size={18} /></span>
                    <span><b>{panel.title}</b><span class="note">{panel.note}</span></span>
                  </div>
                  <ConflictSides sides={panel.sides} onChoose={keep} />
                {:else}
                  <div class="cloud-state">
                    <span class="cloud-ico"><Icon name="cloud" size={18} /></span>
                    <span><b>{panel.title}</b><span class="note">{panel.note}</span></span>
                    {#if panel.kind === 'synced'}
                      <button class="small-btn" data-testid="store-details-sync" disabled={panel.busy} onclick={sync}>{panel.button}</button>
                    {:else if panel.kind === 'off'}
                      <button class="small-btn" onclick={() => openGameSettings(game!)}>Game Settings</button>
                    {/if}
                  </div>
                {/if}
                <div>
                  <p class="sec-title">Save locations</p>
                  <ul class="paths">
                    {#each cloud?.locations ?? [] as loc (loc)}<li><code>{loc}</code></li>{/each}
                  </ul>
                  <p class="note">{saveLocationsNote(game.source)}</p>
                </div>
              </div>
            {/if}
          </div>
        </section>
      </div>
    {:else}
      <p class="note">This game isn’t in your {storeName(sourceOfKey(open.key))} library any more.</p>
    {/if}
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: var(--scrim);
    backdrop-filter: blur(6px);
    display: grid;
    place-items: center;
    z-index: 20;
  }

  .panel {
    position: relative;
    width: min(1040px, calc(100vw - 48px));
    height: min(680px, calc(100vh - 48px));
    box-sizing: border-box;
    padding: 24px;
    border-radius: var(--r-dialog);
    background: var(--overlay);
    border: 1px solid var(--line);
    box-shadow: var(--shadow-overlay);
  }

  .panel:focus-visible {
    outline: none;
  }

  .layout {
    display: grid;
    grid-template-columns: 240px 1fr;
    gap: 24px;
    height: 100%;
    min-height: 0;
  }

  .left {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-height: 0;
    overflow-y: auto;
  }

  .right {
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-height: 0;
    min-width: 0;
  }

  .close {
    position: absolute;
    top: 8px;
    right: 8px;
    color: var(--text);
  }

  .close:hover,
  .close:focus-visible {
    color: var(--primary);
  }

  .cover {
    display: flex;
    justify-content: center;
    flex: none;
  }

  .cover :global(img) {
    display: block;
    max-width: 100%;
    max-height: 320px;
    border-radius: var(--r-card);
    box-shadow: var(--shadow-lift);
  }

  .cover.faded :global(img) {
    filter: saturate(0.35) brightness(0.8);
  }

  .cover :global(.skeleton),
  .cover :global(.placeholder) {
    width: 100%;
    aspect-ratio: 3 / 4;
    height: auto;
    border-radius: var(--r-card);
  }

  h2 {
    margin: 0;
    color: var(--text-h);
    font-size: 20px;
  }

  .header-line {
    margin: 0;
    color: var(--text-muted);
    font-size: 13px;
  }

  .from-drive {
    color: var(--primary);
  }

  .head {
    display: flex;
    flex-direction: column;
    gap: 6px;
    flex: none;
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 11px;
    font-weight: 600;
    padding: 2px 10px;
    border-radius: var(--r-pill);
    background: var(--surface);
    color: var(--text);
    border: 1px solid var(--border);
  }

  .chip.playing {
    background: var(--primary);
    border-color: var(--primary);
    color: var(--on-primary);
    text-transform: uppercase;
    letter-spacing: 0.03em;
  }

  .tabs {
    display: flex;
    gap: 4px;
    border-bottom: 1px solid var(--border);
    flex: none;
  }

  .tabs button {
    font: inherit;
    padding: 8px 14px;
    border: none;
    border-bottom: 2px solid transparent;
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
  }

  .tabs button.active {
    color: var(--text-h);
    border-bottom-color: var(--primary);
  }

  .tabpanel {
    flex: 1;
    min-height: 0;
    min-width: 0;
    overflow-y: auto;
    padding-right: 4px;
  }

  .action {
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .action button {
    width: 100%;
    font: inherit;
    padding: 10px 16px;
    border-radius: var(--r-control);
    border: none;
    background: var(--primary);
    color: var(--on-primary);
    cursor: pointer;
  }

  .action button:hover:not(:disabled) {
    background: var(--primary-hover);
  }

  .action button.secondary {
    padding: 8px 16px;
    background: transparent;
    color: var(--text);
    border: 1px solid var(--border);
  }

  .action button.secondary:hover:not(:disabled) {
    background: var(--surface);
  }

  .action button.secondary.destructive {
    color: var(--danger);
  }

  .action button.confirm {
    background: var(--danger);
    color: var(--on-danger);
    border: none;
  }

  .action button:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .meta-line {
    margin: 0;
    color: var(--text-muted);
    font-size: 12px;
  }

  .meta-line.cloud,
  .meta-line.drive {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 5px;
  }

  .meta-line.err {
    color: var(--danger);
  }

  .meta-line.why {
    margin-top: -2px;
    text-align: center;
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

  .overview {
    display: flex;
    flex-wrap: wrap;
    gap: 24px;
  }

  .desc {
    flex: 1 1 340px;
    min-width: 0;
    color: var(--text);
    font-size: 14px;
    line-height: 1.65;
  }

  .desc p {
    margin: 0;
  }

  .desc p + p {
    margin-top: 0.75em;
  }

  /* The prototype's `.sec-title` (and the RomM Details' section headings). */
  .sec-title {
    margin: 4px 0 8px;
    font-size: 13px;
    font-weight: 600;
    color: var(--text-h);
  }

  .desc .sec-title {
    margin-top: 18px;
    line-height: inherit;
  }

  .tabpanel > .sec-title:first-child {
    margin-top: 0;
  }

  .dlc {
    list-style: none;
    margin: 8px 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .dlc li {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 12px;
    border-radius: var(--r-row);
    background: var(--surface);
  }

  .dlc .t {
    color: var(--text-h);
  }

  .dlc .s {
    margin-left: auto;
    color: var(--text-muted);
    font-size: 12px;
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

  .small-btn.primary {
    background: var(--primary);
    border-color: var(--primary);
    color: var(--on-primary);
  }

  .small-btn:disabled {
    opacity: 0.6;
    cursor: default;
  }

  /* The RomM Overview's facts panel (details/OverviewTab.svelte `.meta`). */
  .facts {
    flex: 0 1 220px;
    min-width: 180px;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 14px 16px;
    border-radius: var(--r-row);
    background: var(--surface);
    border: 1px solid var(--line);
  }

  .facts dt {
    margin-bottom: 2px;
    color: var(--text-muted);
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .facts dd {
    margin: 0;
    min-width: 0;
    overflow-wrap: anywhere;
    color: var(--text);
    font-size: 13px;
    line-height: 1.4;
  }

  .facts code {
    font-size: 11px;
    overflow-wrap: anywhere;
  }

  .note {
    margin: 10px 0 0;
    font-size: 12px;
    color: var(--text-muted);
  }

  .media-row {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: 10px;
    margin-top: 8px;
  }

  .shot {
    display: block;
    width: 100%;
    aspect-ratio: 16 / 9;
    overflow: hidden;
    border-radius: var(--r-row);
    background: var(--field);
  }

  .shot :global(img) {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .st-cloud {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .cloud-state {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 14px;
    border-radius: var(--r-row);
    background: var(--surface);
    border: 1px solid var(--line);
  }

  .cloud-state b {
    color: var(--text-h);
    display: block;
  }

  .cloud-state .note {
    display: block;
    margin: 0;
  }

  .cloud-state > :last-child:not(:nth-child(2)) {
    margin-left: auto;
  }

  .cloud-ico {
    display: inline-grid;
    place-items: center;
    width: 34px;
    height: 34px;
    border-radius: var(--r-control);
    background: var(--field);
    color: var(--primary);
    flex: none;
  }

  .cloud-state.warn {
    border-color: var(--danger);
  }

  .cloud-state.warn .cloud-ico {
    color: var(--danger);
  }

  .paths {
    margin: 8px 0 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .paths code {
    display: block;
    padding: 6px 10px;
    border-radius: var(--r-control);
    background: var(--field);
    border: 1px solid var(--line);
    color: var(--text-h);
    font: 12px ui-monospace, monospace;
    overflow-wrap: anywhere;
  }
</style>
