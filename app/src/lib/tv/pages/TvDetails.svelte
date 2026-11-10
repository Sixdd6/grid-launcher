<script lang="ts">
  // Details (checklist items 270–318, 430–437; tvmode.js `details`,
  // `detBody`, `detShowTab`): the cover and status lines on the left; the
  // title, header line, chips, one row of buttons and the tabs on the right.
  // LB / RB switch tabs (wrapping), focusing the new tab's first item at the
  // top of the tab, or the first button when it has nothing to focus. The
  // right stick scrolls the tab body (`data-read-area`) without moving
  // focus; Up into the buttons scrolls it back to its top
  // (`data-read-reset`). Every fact and action is the shared DetailsStore's.
  import { tick } from 'svelte';
  import Image from '../../Image.svelte';
  import { DetailsStore } from '../../details/detailsStore.svelte';
  import { keptFolder } from '../../details/unlinked';
  import { verificationLabel } from '../../details/header';
  import { fileSizeText } from '../../details/files';
  import { sizeFact } from '../../details/overview';
  import { shownTab, type DetailsTab } from '../../details/tabs';
  import type { DetailsSubject } from '../../details/subject';
  import * as run from '../../games/run';
  import { downloads } from '../../stores/downloads.svelte';
  import { session } from '../../stores/session.svelte';
  import { pushToast, TOAST_DURATION_MS } from '../../stores/toasts.svelte';
  import { focusables } from '../focus/dom';
  import { openFolderBrowser, playGame, promptFor } from '../gameActions';
  import { liveInstall } from '../games';
  import Glyph from '../glyphs/Glyph.svelte';
  import { nav, popPage, pushPage, setFocus } from '../nav.svelte';
  import { confirmBox } from '../overlays/overlays.svelte';
  import { useScreen } from '../screen.svelte';
  import TvButton from '../ui/TvButton.svelte';
  import TvIcon from '../ui/TvIcon.svelte';
  import TvAchievementsTab from './details/TvAchievementsTab.svelte';
  import TvMediaTab from './details/TvMediaTab.svelte';
  import TvOverviewTab from './details/TvOverviewTab.svelte';
  import TvSavesTab from './details/TvSavesTab.svelte';
  import {
    cancelConfirm,
    detButtons,
    leftLines,
    locatedNotice,
    locateFailedNotice,
    locateTitle,
    removeDriveConfirm,
    removeRowConfirm,
    stopConfirm,
    tabLabel,
    tabStep,
    tvHeaderLine,
    uninstallConfirm,
    updateConfirm,
    type DetButton,
  } from './details';
  import { det } from './detailsState.svelte';
  import TvGameSettings from './TvGameSettings.svelte';
  import TvLinkPage from './TvLinkPage.svelte';

  let { subject, sizeBytes = 0 }: { subject: DetailsSubject; sizeBytes?: number } = $props();

  const pageId = nav.topPage?.id ?? null;
  const d = new DetailsStore(() => subject);

  let bodyEl = $state<HTMLElement | null>(null);
  let btnsEl = $state<HTMLElement | null>(null);

  // The background shows the game's art when Details opens (item 11).
  $effect(() => {
    d.reportViewed();
  });
  $effect(() => {
    d.loadDetail();
  });
  $effect(() => {
    d.loadLaunchDefaults();
  });
  $effect(() => d.loadServerTitles());
  $effect(() => {
    d.refreshInstallBlocked();
  });
  $effect(() => {
    d.refreshContentAvailability();
  });
  $effect(() => {
    d.loadCloudPanelInfo();
  });

  let live = $derived(liveInstall(downloads.entries, subject.romId));
  let installed = $derived(d.installedNow);
  let row = $derived(d.installedRow);
  let missing = $derived(d.missing.showMissing);
  let unlinked = $derived(d.unlinkedRow !== null);
  let noId = $derived(subject.romId === null && !unlinked);
  let drive = $derived(subject.drive?.name ?? null);
  let size = $derived(sizeFact(d.detail?.files ?? []) || (sizeBytes > 0 ? fileSizeText(sizeBytes) : ''));

  let buttons = $derived(
    detButtons({
      title: subject.name,
      native: d.isNative,
      installed,
      missing,
      unlinked,
      noId,
      live: live ? { ...live, cancellable: d.liveEntry?.status !== 'installing' } : null,
      playing: d.liveSession !== undefined,
      updateLabel: d.updateLabel,
      content: d.buttons,
      drive,
      connected: session.connected,
    }),
  );
  let lines = $derived(
    leftLines({
      installed,
      missing,
      unlinked,
      noId,
      lastPlayedAt: row?.last_played_at ?? 0,
      launchTarget: d.launchTarget,
      installBlocked: d.installBlocked,
      size,
      onDrives: subject.drive === undefined ? d.onDrives : '',
      drive,
      driveCopy: d.driveCopy,
    }),
  );
  let header = $derived(tvHeaderLine(d.header, drive, d.ratingNumber));

  let tabs = $derived(d.tabs);
  let tab = $derived(shownTab(det.tab, tabs));

  /** LB / RB or a click on a tab (items 292–293): the new tab's first item
   *  at the top of the tab, else the first button. */
  async function showTab(t: DetailsTab) {
    det.tab = t;
    await tick();
    if (bodyEl) bodyEl.scrollTop = 0;
    const first = focusables(bodyEl)[0] ?? focusables(btnsEl)[0] ?? null;
    if (first) setFocus(first, { instant: true });
  }

  useScreen({
    // RS "Scroll", or D-pad "Read" while the summary is focused (item 294).
    hints: (el) => (el?.hasAttribute('data-read') ? [{ button: 'DPAD', label: 'Read' }] : [{ button: 'RS', label: 'Scroll' }]),
    lb: () => void showTab(tabStep(tabs, tab, -1)),
    rb: () => void showTab(tabStep(tabs, tab, 1)),
  });

  /** A refusal an action answers inline on the desktop: a card here. */
  function sayError() {
    if (d.error) pushToast(d.error, 'error');
  }

  function ask(words: { title: string; body: string; ok: string }, danger: boolean, onOk: () => void) {
    void confirmBox({ ...words, danger, onOk });
  }

  function press(b: DetButton) {
    const title = subject.name;
    switch (b.kind) {
      case 'cancel':
        ask(cancelConfirm(title), true, () => void d.cancel().then(sayError));
        return;
      case 'play':
        if (subject.romId !== null) void playGame({ romId: subject.romId, title, platform: subject.platformName });
        return;
      case 'stop':
        ask(stopConfirm(title), true, () => void d.stop().then(sayError));
        return;
      case 'install':
        void d.install().then((outcome) => outcome && promptFor(outcome, title));
        return;
      case 'install-drive':
        void d.installFromDrive();
        return;
      case 'remove-drive':
        if (subject.drive)
          ask(removeDriveConfirm(title, subject.drive.name, installed), true, async () => {
            if (await d.removeFromDrive()) void popPage();
            else sayError();
          });
        return;
      case 'link':
        if (d.unlinkedRow)
          void pushPage({
            title: 'Link to server game',
            component: TvLinkPage,
            props: { row: d.unlinkedRow, held: d.heldRomIds, detailsPage: pageId },
          });
        return;
      case 'remove-row': {
        const target = d.removeTarget;
        if (!target) return;
        ask(removeRowConfirm(title, keptFolder(target)), true, async () => {
          try {
            if (await d.removeFromLibrary()) void popPage();
          } catch (err) {
            pushToast(err instanceof Error ? err.message : String(err), 'error');
          }
        });
        return;
      }
      case 'locate': {
        const target = row;
        if (!target) return;
        const folder = keptFolder(target);
        openFolderBrowser({
          title: locateTitle(title),
          start: folder.replace(/[\\/][^\\/]*$/, ''),
          onPick: async (path) => {
            const outcome = await run.locateGame(target.title, target.platform, path);
            const n = outcome.kind === 'ok' ? locatedNotice(title, path) : locateFailedNotice(title, path);
            pushToast(n.text, outcome.kind === 'ok' ? 'success' : 'error', TOAST_DURATION_MS, { sub: n.sub });
          },
        });
        return;
      }
      case 'update':
        ask(updateConfirm(title), false, () => void d.update().then(sayError));
        return;
      case 'install-update':
        void d.installContent('update').then(sayError);
        return;
      case 'install-dlc':
        void d.installContent('dlc').then(sayError);
        return;
      case 'game-settings':
        if (subject.romId !== null)
          void pushPage({
            title: 'Game settings',
            component: TvGameSettings,
            props: { romId: subject.romId, title, platform: subject.platformName, platformSlug: d.platformSlug },
          });
        return;
      case 'uninstall':
        ask(uninstallConfirm(title), true, () =>
          void d.uninstall().then((ok) => {
            if (!ok) sayError();
          }),
        );
        return;
      case 'backup':
        // TODO(P13): the Backup page titled "Backup <title>" (item 435).
        return;
    }
  }
</script>

<div class="det" data-testid="tv-details">
  <div class="det-l">
    <div class="cv" style="--cw:17rem">
      <span class="cv-art"><span class="cv-img"><Image url={d.coverLarge ?? d.coverSmall} alt={subject.name} placeholder={subject.name} /></span></span>
    </div>
    <div class="lp" data-testid="tv-det-left" id="tv-det-left-lines">
      {#each lines as line, i (i)}
        <div>{#each line as seg, j (j)}{#if seg.b}<b>{seg.text}</b>{:else}{seg.text}{/if}{/each}</div>
      {/each}
    </div>
  </div>
  <div class="det-r">
    <h1 data-testid="tv-det-title">{subject.name}</h1>
    <div class="hl" data-testid="tv-det-header">{header}</div>
    <div class="chips">
      {#if installed}<span class="chip"><b>Installed</b></span>{/if}
      {#if d.updateLabel !== null}<span class="chip warn">Update available</span>{/if}
      {#if d.liveSession}<span class="chip"><b>Playing</b></span>{/if}
      {#if d.detail}<span class="chip">{verificationLabel(d.detail.is_identified)}</span>{/if}
      {#if d.flags.length}<span class="chip">{d.flags.join(' · ')}</span>{/if}
      {#if d.version}<span class="chip">{d.version}</span>{/if}
    </div>
    <div class="btns" data-nav="h" data-read-reset id="det-btns" bind:this={btnsEl}>
      {#each buttons as b (b.kind)}
        <TvButton
          k={`db-${b.kind}`}
          primary={b.primary}
          danger={b.danger}
          disabled={b.disabled}
          why={b.why}
          testId={`tv-det-${b.kind}`}
          onA={() => press(b)}
        >
          {#if b.icon}<TvIcon name={b.icon} px={18} />{/if}{b.label}
        </TvButton>
      {/each}
    </div>
    <div class="ttabs" data-testid="tv-det-tabs">
      <Glyph button="LB" />
      {#each tabs as t (t)}
        <button class="ttab" class:on={t === tab} data-testid={`tv-det-tab-${t}`} onclick={() => void showTab(t)}>{tabLabel(t)}</button>
      {/each}
      <Glyph button="RB" />
    </div>
    <div class="tbody" data-read-area data-scroll="tbody" id="det-body" bind:this={bodyEl} data-testid="tv-det-body">
      {#if tab === 'overview'}
        <TvOverviewTab {d} />
      {:else if tab === 'media'}
        <TvMediaTab {d} />
      {:else if tab === 'saves'}
        <TvSavesTab {d} {installed} {unlinked} {noId} lastPlayedAt={row?.last_played_at ?? 0} />
      {:else if tab === 'achievements' && subject.romId !== null}
        <TvAchievementsTab romId={subject.romId} />
      {/if}
    </div>
  </div>
</div>
