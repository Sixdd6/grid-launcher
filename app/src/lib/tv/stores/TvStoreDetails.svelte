<script lang="ts">
  // A store game's page (TVStores README § The game page; prototype
  // `sdetails`, `sBody`): the cover and its lines on the left; the title,
  // header line, chips, one row of buttons (Play · Install… · Cancel install
  // · 45% · Not supported · Update to 2.2 · Game settings · Verify and
  // repair · Uninstall · Backup to drive… / Install from drive · Remove from
  // drive…) and the tabs (LB / RB): Overview with DLC you own, Media, and
  // Cloud saves for an installed GOG game. From a drive's grid the header
  // line starts "From Couch kit".
  import { tick } from 'svelte';
  import { api, type DriveStoreGame, type StoreCloudState, type StoreGame } from '../../api';
  import { descriptionParagraphs } from '../../gamestores/details';
  import { liveJobFor } from '../../gamestores/jobs';
  import { canBackup, hasStoreCloud, storeName } from '../../gamestores/marks';
  import { sizeGb } from '../../gamestores/text';
  import { downloads } from '../../stores/downloads.svelte';
  import { drives } from '../../stores/drives.svelte';
  import { gameStores, refreshLibrary } from '../../stores/gameStores.svelte';
  import { installed } from '../../stores/installed.svelte';
  import { tvf } from '../focus/actions';
  import { focusables } from '../focus/dom';
  import Glyph from '../glyphs/Glyph.svelte';
  import { openLayer, popPage, pushPage, setFocus } from '../nav.svelte';
  import { confirmBox } from '../overlays/overlays.svelte';
  import { useScreen } from '../screen.svelte';
  import TvButton from '../ui/TvButton.svelte';
  import TvIcon from '../ui/TvIcon.svelte';
  import { storeRowOf, storeGameFromDrive } from './libraryStores';
  import TvStoreArt from './TvStoreArt.svelte';
  import TvStoreCover from './TvStoreCover.svelte';
  import TvStoreGameSettings from './TvStoreGameSettings.svelte';
  import TvStoreMark from './TvStoreMark.svelte';
  import TvStoreViewer from './TvStoreViewer.svelte';
  import { bumpDrives, driveStoreCache, setSyncing, tvStores } from './storeState.svelte';
  import { card, defaultCompat, fail, openInstall, openStoreBackup, playStore, resolveCloud, restoreFromDrive, toDownloads } from './storeActions';
  import { uninstalledNotice } from '../../gamestores/notices';
  import {
    cancelJobWords,
    dlcAction,
    keepConfirm,
    removeDlcWords,
    removeFromDriveWords,
    ROMM_CLOUD_OFF,
    tvCloudTab,
    tvDetailChips,
    tvFacts,
    tvHeaderLine,
    tvLeftLines,
    tvStoreButtons,
    tvTabs,
    tvTabStep,
    uninstallWords,
    updateWords,
    type TvStoreButton,
    type TvTab,
  } from './storeWords';
  import { storeBackupTitle } from '../../gamestores/backup';

  let { gameKey, drive = null }: { gameKey: string; drive?: { key: string; name: string } | null } = $props();

  // The drive's record of the game, when the page came from its grid.
  let driveGame = $state<DriveStoreGame | null>(null);
  $effect(() => {
    const d = drive;
    void tvStores.driveVersion;
    if (!d) return;
    driveGame = driveStoreCache.get(d.key)?.find((g) => g.key === gameKey) ?? driveGame;
    api
      .driveStoreGames(d.key)
      .then((list) => {
        driveStoreCache.set(d.key, list);
        driveGame = list.find((g) => g.key === gameKey) ?? null;
      })
      .catch(() => {});
  });

  let game = $derived<StoreGame | null>(gameStores.game(gameKey) ?? (driveGame ? storeGameFromDrive(driveGame) : null));
  let status = $derived(game ? gameStores.status(game.source) : null);
  let job = $derived(liveJobFor(downloads.entries, gameKey));
  let row = $derived(game ? storeRowOf(installed.list, game) : null);
  /** Still on the drive (it may have been removed while the page was open). */
  let fromDrive = $derived(drive !== null && driveGame !== null);

  // The ready drives that hold this game ("On Couch kit").
  let onDrives = $state<string[]>([]);
  $effect(() => {
    void tvStores.driveVersion;
    if (drive || !game || !canBackup(game.source)) return;
    const ready = drives.withPack;
    let cancelled = false;
    Promise.all(
      ready.map((d) =>
        (driveStoreCache.has(d.key) ? Promise.resolve(driveStoreCache.get(d.key)!) : api.driveStoreGames(d.key))
          .then((list) => {
            driveStoreCache.set(d.key, list);
            return list.some((g) => g.key === gameKey) ? (d.pack?.name ?? d.label) : null;
          })
          .catch(() => null),
      ),
    ).then((names) => {
      if (!cancelled) onDrives = names.filter((n): n is string => n !== null);
    });
    return () => {
      cancelled = true;
    };
  });

  // The cloud saves' state, for the Cloud saves tab.
  let cloud = $state<StoreCloudState | null>(null);
  function loadCloud() {
    if (!game || !game.installed || !hasStoreCloud(game.source)) return;
    api
      .storeCloudState(gameKey)
      .then((c) => (cloud = c))
      .catch(() => {});
  }
  $effect(() => {
    void game?.cloud;
    void game?.last_sync;
    void game?.installed;
    loadCloud();
  });

  let nowSeconds = $derived.by(() => {
    void game;
    void cloud;
    return Math.floor(Date.now() / 1000);
  });

  let tabs = $derived(game ? tvTabs(game) : (['Overview', 'Media'] as TvTab[]));
  let tab = $state<TvTab>('Overview');
  let shownTab = $derived(tabs.includes(tab) ? tab : 'Overview');

  let bodyEl = $state<HTMLElement | null>(null);
  let btnsEl = $state<HTMLElement | null>(null);

  async function showTab(t: TvTab) {
    tab = t;
    await tick();
    if (bodyEl) bodyEl.scrollTop = 0;
    const first = focusables(bodyEl)[0] ?? focusables(btnsEl)[0] ?? null;
    if (first) setFocus(first, { instant: true });
  }

  useScreen({
    hints: (el) => (el?.hasAttribute('data-read') ? [{ button: 'DPAD', label: 'Read' }] : [{ button: 'RS', label: 'Scroll' }]),
    lb: () => void showTab(tvTabStep(tabs, shownTab, -1)),
    rb: () => void showTab(tvTabStep(tabs, shownTab, 1)),
  });

  let buttons = $derived(game ? tvStoreButtons({ game, status, job, fromDrive }) : []);
  let lines = $derived(
    game
      ? tvLeftLines({
          game,
          nowSeconds,
          lastPlayedAt: row?.last_played_at ?? 0,
          compat: defaultCompat(),
          drive: fromDrive && drive && driveGame ? { name: drive.name, game: driveGame } : null,
          onDrives,
        })
      : [],
  );

  function ask(w: { title: string; body: string; ok: string }, danger: boolean, onOk: () => void) {
    void confirmBox({ ...w, danger, onOk });
  }

  function press(b: TvStoreButton) {
    const g = game;
    if (!g) return;
    switch (b.act) {
      case 'play':
        void playStore(g);
        return;
      case 'install':
        openInstall(g);
        return;
      case 'cancel': {
        const j = job;
        if (j) ask(cancelJobWords(g.title, j), true, () => void api.cancelInstall(j.id).catch(fail));
        return;
      }
      case 'restore':
        if (drive && driveGame) void restoreFromDrive(drive.key, gameKey);
        return;
      case 'update':
        // The queued card is this page's (a store row earns none from the
        // shared notifier, and the backend sends none for a command).
        ask(updateWords(g), false, () =>
          void api.storeUpdate(g.key).then(() => card(`Update for ${g.title} added to download queue`, 'success', undefined, toDownloads), fail),
        );
        return;
      case 'settings':
        void pushPage({ title: 'Game settings', component: TvStoreGameSettings, props: { gameKey } });
        return;
      case 'verify':
        void api.storeRepair(g.key).then(() => card(`Verifying ${g.title}’s files…`), fail);
        return;
      case 'uninstall':
        ask(uninstallWords(g), true, () =>
          void api
            .storeUninstall(g.key)
            .then(() => card(uninstalledNotice(g.source, g.title, (gameStores.status(g.source)?.cloud_saves ?? false) && g.cloud !== 'none').text), fail),
        );
        return;
      case 'remove_drive':
        if (drive)
          ask(removeFromDriveWords(g.title, drive.name, g.installed), true, async () => {
            try {
              await api.removeStoreFromDrive(drive.key, [gameKey]);
            } catch (err) {
              fail(err);
              return;
            }
            card(`${g.title} removed from ${drive.name}`);
            bumpDrives();
            void popPage();
          });
        return;
      case 'backup':
        openStoreBackup([g.key], storeBackupTitle({ kind: 'game', name: g.title }));
        return;
    }
  }

  function dlc(i: number) {
    const g = game;
    if (!g || !g.installed) return;
    const d = g.dlc[i];
    const short = d.title.startsWith(`${g.title} — `) ? d.title.slice(g.title.length + 3) : d.title;
    if (d.installed) {
      ask(removeDlcWords(short, g.title), true, () =>
        void api.storeRemoveDlc(g.key, d.id).then(() => {
          card(`${short} removed`);
          void refreshLibrary(g.source);
        }, fail),
      );
      return;
    }
    void api.storeInstallDlc(g.key, d.id).then(() => card(`${short} added to download queue`, 'success', undefined, toDownloads), fail);
  }

  function syncNow() {
    const g = game;
    if (!g || tvStores.syncing(g.key)) return;
    setSyncing(g.key, true);
    api
      .storeCloudSync(g.key)
      .then((c) => {
        cloud = c;
        card(`Cloud saves synced for ${g.title}`);
        void refreshLibrary(g.source);
      }, fail)
      .finally(() => setSyncing(g.key, false));
  }

  function keep(which: 'local' | 'cloud') {
    const g = game;
    if (!g) return;
    ask(keepConfirm(which, g.source), false, () => void resolveCloud(g, which).then(() => loadCloud()));
  }

  function view(i: number) {
    const g = game;
    if (!g) return;
    const model = $state({ urls: g.screenshots, index: i, title: g.title });
    const step = (d: number) => (model.index = (model.index + d + model.urls.length) % model.urls.length);
    void openLayer({
      component: TvStoreViewer,
      props: { model },
      cls: 'viewer-l',
      hints: [
        { button: 'LB', label: 'Previous' },
        { button: 'RB', label: 'Next' },
        { button: 'B', label: 'Close' },
      ],
      binds: { LB: () => step(-1), RB: () => step(1), LEFT: () => step(-1), RIGHT: () => step(1) },
    });
  }

  let cloudTab = $derived(game ? tvCloudTab(game, cloud, nowSeconds, tvStores.syncing(gameKey)) : null);
</script>

{#if game}
  <div class="det" data-testid="tv-store-details">
    <div class="det-l">
      <TvStoreCover k="dc" {game} {job} quick={null} label={false} width="17rem" testId="tv-store-det-cover" />
      <div class="lp" data-testid="tv-det-left">
        {#each lines as line (line.key)}
          <div data-testid={`tv-store-line-${line.key}`}>{#each line.segs as s, j (j)}{#if s.b}<b class:warn={s.warn}>{s.text}</b>{:else}{s.text}{/if}{/each}</div>
        {/each}
      </div>
    </div>
    <div class="det-r">
      <h1 data-testid="tv-det-title">{game.title}</h1>
      <div class="hl" data-testid="tv-det-header">{tvHeaderLine(game, fromDrive && drive ? drive.name : null)}</div>
      <div class="chips">
        <span class="chip"><TvStoreMark source={game.source} /><b>{storeName(game.source)}</b></span>
        {#each tvDetailChips(game) as c (c.text)}
          <span class="chip" class:warn={c.warn}>{#if c.bold}<b>{c.text}</b>{:else}{c.text}{/if}</span>
        {/each}
      </div>
      <div class="btns" data-nav="h" data-read-reset id="det-btns" bind:this={btnsEl}>
        {#each buttons as b (b.act)}
          <TvButton k={`sd-${b.act}`} primary={b.primary} danger={b.danger} disabled={b.disabled} why={b.why} testId={`tv-store-det-${b.act}`} onA={() => press(b)}>
            {#if b.icon}<TvIcon name={b.icon} px={18} />{/if}{b.label}
          </TvButton>
        {/each}
      </div>
      <div class="ttabs" data-testid="tv-det-tabs">
        <Glyph button="LB" />
        {#each tabs as t (t)}
          <button class="ttab" class:on={t === shownTab} data-testid={`tv-store-tab-${t}`} onclick={() => void showTab(t)}>{t}</button>
        {/each}
        <Glyph button="RB" />
      </div>
      <div class="tbody" data-read-area data-scroll="tbody" id="det-body" bind:this={bodyEl} data-testid="tv-det-body">
        {#if shownTab === 'Overview'}
          <div class="ov">
            <div>
              <div class="prose" data-k="prose" data-al="Read" data-read data-testid="tv-det-prose" use:tvf={{}}>
                {#each descriptionParagraphs(game.description) as p, i (i)}
                  <p>{p}</p>
                {:else}
                  <p class="muted">No description available</p>
                {/each}
              </div>
              {#if game.dlc.length}
                <div class="sub-h">DLC you own<span>{game.dlc.length}</span></div>
                <div data-nav="v" id="store-dlc">
                  {#each game.dlc as x, i (x.id)}
                    {@const short = x.title.startsWith(`${game.title} — `) ? x.title.slice(game.title.length + 3) : x.title}
                    <div class="lrow" data-k={`dlc${i}`} data-al={dlcAction(game.installed, x.installed)} data-testid={`tv-store-dlc-${x.id}`} use:tvf={{ a: () => dlc(i) }}>
                      <div class="grow"><div class="t">{short}</div><div class="s">{sizeGb(x.download_bytes)}{x.installed ? ' · Installed' : ''}</div></div>
                    </div>
                  {/each}
                </div>
              {/if}
            </div>
            <dl class="facts" data-testid="tv-det-facts">
              {#each tvFacts(game) as f (f.term)}
                <div><dt>{f.term}</dt><dd>{f.value}</dd></div>
              {/each}
            </dl>
          </div>
        {:else if shownTab === 'Media'}
          {#if game.screenshots.length}
            <div class="sub-h">Screenshots<span>{game.screenshots.length}</span></div>
            <div class="mtrack" data-nav="h" data-enter="first" data-track data-scroll="det-shots" data-testid="tv-store-shots">
              {#each game.screenshots as url, i (i)}
                <button class="mtile" data-k={`sm${i}`} data-al="View" style="width:14.2rem" use:tvf={{ a: () => view(i) }}>
                  <TvStoreArt {url} alt={`${game.title} ${i + 1}`} placeholder="Screenshot" />
                </button>
              {/each}
            </div>
            <p class="note">From {storeName(game.source)}’s store page.</p>
          {:else}
            <p class="note" data-testid="tv-det-no-media">No media available</p>
          {/if}
        {:else if cloudTab}
          <div data-nav="v" id="store-cloud" data-testid="tv-store-cloud">
            {#if cloudTab.kind === 'conflict'}
              <p class="note warn">{cloudTab.note}</p>
              <div class="lrow warnrow" data-k="ck-l" data-al="Keep it" data-testid="tv-store-cloud-local" use:tvf={{ a: () => keep('local') }}>
                <div class="grow"><div class="t">{cloudTab.local.title}</div><div class="s">{cloudTab.local.sub}</div></div>
              </div>
              <div class="lrow" data-k="ck-c" data-al="Use it" data-testid="tv-store-cloud-cloud" use:tvf={{ a: () => keep('cloud') }}>
                <div class="grow"><div class="t">{cloudTab.cloud.title}</div><div class="s">{cloudTab.cloud.sub}</div></div>
              </div>
            {:else if cloudTab.kind === 'off'}
              <div class="lrow" data-k="ck-o" data-al="Open" data-testid="tv-store-cloud-off" use:tvf={{ a: () => press({ act: 'settings', label: '', primary: false }) }}>
                <div class="grow"><div class="t">{cloudTab.title}</div><div class="s">{cloudTab.sub}</div></div>
              </div>
            {:else if cloudTab.kind === 'none'}
              <p class="note" data-testid="tv-store-cloud-none">{cloudTab.note}</p>
            {:else}
              <div class="lrow" data-k="ck-s" data-al="Sync now" data-testid="tv-store-cloud-sync" use:tvf={{ a: syncNow }}>
                <div class="grow"><div class="t">{cloudTab.title}</div><div class="s">{cloudTab.sub}</div></div>
                <span class="muted">Sync now</span>
              </div>
            {/if}
          </div>
          <div class="sub-h">Save location</div>
          {#each cloud?.locations ?? [] as loc (loc)}
            <p class="note spath">{loc}</p>
          {/each}
          <p class="note">{ROMM_CLOUD_OFF}</p>
        {/if}
      </div>
    </div>
  </div>
{/if}
