<script lang="ts">
  // Settings (checklist items 225–251, 261–269; tvmode.js `TABS.settings`,
  // `paneRows`, `srowAct`, `srowAdj`): the panes' rail and the pane beside
  // it. Moving through the rail shows each pane at once (item 226); A or
  // Right goes into the pane. Every change applies at once and redraws only
  // the pane, keeping focus on the row (233: rows keep their keys). Sliders
  // adjust with Left / Right while focused, with the D-pad "Adjust" hint
  // (230). The rows are the pane modules' (`panes/*.ts`); this screen holds
  // what they read and runs what they ask for. Systems is its own pane
  // (TvSystemsPane); P13 builds Backup drives.
  import { api, type CloudSettings, type LibraryPathCheck, type RaStatus } from '../../../api';
  import { appUpdate, dismiss as dismissUpdate, refreshAppUpdateStatus } from '../../../stores/appUpdate.svelte';
  import { libraryMove } from '../../../stores/libraryMove.svelte';
  import { connectToServer, disconnect, session } from '../../../stores/session.svelte';
  import { pushToast } from '../../../stores/toasts.svelte';
  import {
    commitBackgroundBlur,
    commitBackgroundFade,
    setBackgroundEnabled,
    setHardwareAcceleration,
    setOpenOn,
    setTheme,
    setTvAssignment,
    setTvPrompts,
    uiSettings,
  } from '../../../stores/uiSettings.svelte';
  import { leaveTv } from '../../../stores/uiMode.svelte';
  import { LibraryChange } from '../../../settings/libraryChange.svelte';
  import { clearRaCredentials, fetchRaStatus, raLogin, saveRaToken } from '../../../settings/raActions';
  import { SKIPPED_TOAST_MS, skippedRemovalText } from '../../../uninstallSkips';
  import { TV_PANES, TV_PANE_LABELS, type TvPane } from '../../../uiMode';
  import { openFolderBrowser } from '../../gameActions';
  import { controllers } from '../../input/router.svelte';
  import { nav, setPane } from '../../nav.svelte';
  import { confirmBox, openKeyboard, openList, quitBox } from '../../overlays/overlays.svelte';
  import { reconnect } from '../../reconnect';
  import type { Row } from '../../rows';
  import { useScreen } from '../../screen.svelte';
  import { settingsState } from './settingsState.svelte';
  import TvRail from '../../ui/TvRail.svelte';
  import TvRailRow from '../../ui/TvRailRow.svelte';
  import TvRows from '../../ui/TvRows.svelte';
  import TvSystemsPane from './TvSystemsPane.svelte';
  import { appearanceRows } from './panes/appearance';
  import type { ChoiceSpec } from './panes/choice';
  import { cloudRows } from './panes/cloud';
  import { connectionRows } from './panes/connection';
  import {
    browserStart,
    CHANGE_LIBRARY_TITLE,
    deleteLabel,
    deleteWords,
    FRESH_TITLE,
    freshNotice,
    libraryRows,
    moveListSub,
    moveListTitle,
    moveStartedNotice,
    refusalNotice,
  } from './panes/library';
  import { RA_SAVED, raCanLogin, raCanSave, raRows } from './panes/ra';
  import { tvRows } from './panes/tv';
  import { releaseName, UPDATE_DISMISSED, updatesRows } from './panes/updates';

  const errText = (err: unknown) => (err instanceof Error ? err.message : String(err));
  const fail = (err: unknown) => void pushToast(errText(err), 'error');

  /** A choice row's list: a check on the current value, focused on open. */
  function openChoice(spec: ChoiceSpec) {
    void openList({
      title: spec.title,
      ...(spec.sub ? { sub: spec.sub } : {}),
      items: spec.options.map(([v, label]) => ({
        label,
        ...(spec.subs?.[v] ? { sub: spec.subs[v] } : {}),
        checked: v === spec.value,
        testId: `tv-opt-${v}`,
        onPick: () => spec.pick(v),
      })),
    });
  }

  // ---- Cloud saves: read when the pane shows, saved on every change.
  let cloud = $state<CloudSettings | null>(null);
  let cloudError = $state<string | null>(null);
  $effect(() => {
    if (nav.pane !== 'cloud-saves') return;
    api
      .cloudSettings()
      .then((s) => {
        cloud = s;
        cloudError = null;
      })
      .catch((err) => (cloudError = errText(err)));
  });
  function saveCloud(next: CloudSettings) {
    const was = cloud;
    cloud = next;
    api.setCloudSettings(next).catch((err) => {
      cloud = was;
      fail(err);
    });
  }

  // ---- Library: the folder, read again after a move or a relocate.
  $effect(() => {
    void libraryMove.pathVersion;
    if (nav.pane !== 'library') return;
    api
      .getLibraryPath()
      .then((p) => (settingsState.libPath = p))
      .catch(fail);
  });

  function changeLibrary() {
    openFolderBrowser({ title: CHANGE_LIBRARY_TITLE, start: browserStart(settingsState.libPath), onPick: (p) => void pickedFolder(p) });
  }

  async function pickedFolder(picked: string) {
    const current = settingsState.libPath ?? '';
    let check: LibraryPathCheck;
    try {
      check = await api.checkLibraryPath(picked);
    } catch (err) {
      fail(err);
      return;
    }
    const refused = refusalNotice(check);
    if (refused || check.status !== 'ok') {
      if (refused) pushToast(refused.title, 'error', undefined, { sub: refused.sub });
      return;
    }
    const lc = new LibraryChange(() => current, {
      onDone: (outcome, deleted) => {
        const n = freshNotice(outcome, deleted);
        pushToast(n.title, n.error ? 'error' : 'success', undefined, n.sub ? { sub: n.sub } : {});
        if (outcome.status === 'switched') {
          settingsState.libPath = outcome.library_path;
          pushToast(skippedRemovalText(outcome.skipped), 'error', SKIPPED_TOAST_MS);
        }
      },
      onMoveStarted: (outcome) => {
        if (outcome.status !== 'started') return;
        const n = moveStartedNotice(outcome.new_root);
        pushToast(n.title, 'success', undefined, { sub: n.sub });
      },
      onClose: () => {},
    });
    lc.typed = check.path;
    lc.check = check;
    // With no current library there is nothing to move or keep: Start fresh
    // switches straight away, as on the desktop.
    if (!lc.hasOldRoot) {
      await lc.run(false);
      if (lc.error) fail(lc.error);
      return;
    }
    await lc.askMove();
    const moveRefusal = lc.error;
    void openList({
      title: moveListTitle(check.path),
      sub: moveListSub(lc.movePreview, moveRefusal),
      items: [
        { label: 'Move existing files', sub: 'Runs in Downloads', testId: 'tv-lib-move', onPick: () => void moveLibrary(lc, moveRefusal) },
        { label: 'Start fresh', sub: 'Leave or delete the old files', testId: 'tv-lib-fresh', onPick: () => void startFresh(lc, current) },
      ],
    });
  }

  async function moveLibrary(lc: LibraryChange, refusal: string | null) {
    if (lc.movePreview === null) {
      pushToast("Can't move the library", 'error', undefined, refusal ? { sub: refusal } : {});
      return;
    }
    await lc.startMove();
    if (lc.error) pushToast("Can't move the library", 'error', undefined, { sub: lc.error });
  }

  async function startFresh(lc: LibraryChange, current: string) {
    await lc.askDelete();
    const preview = lc.preview;
    const leave = async () => {
      await lc.run(false);
      if (lc.error) fail(lc.error);
    };
    const del = async () => {
      await lc.run(true);
      if (lc.error) fail(lc.error);
    };
    void openList({
      title: FRESH_TITLE,
      items: [
        { label: 'Leave them on disk', testId: 'tv-lib-leave', onPick: () => void leave() },
        ...(preview
          ? [
              {
                label: deleteLabel(preview),
                danger: true,
                testId: 'tv-lib-delete',
                onPick: () => void confirmBox({ ...deleteWords(preview, current), danger: true, onOk: () => void del() }),
              },
            ]
          : []),
      ],
    });
  }

  // ---- RetroAchievements: typed values stay here until Save or Log In. The
  // token and the password are plain (non-reactive) locals, never shown,
  // logged or put in a row.
  let raStatus = $state<RaStatus | null>(null);
  let raUser = $state<string | null>(null);
  let raToken = '';
  let raTokenTyped = $state(false);
  let raPassword = '';
  let raPasswordTyped = $state(false);
  let raBusy = false;
  function forgetRaTyping() {
    raUser = null;
    raToken = '';
    raTokenTyped = false;
    raPassword = '';
    raPasswordTyped = false;
  }
  $effect(() => {
    if (nav.pane !== 'retroachievements') {
      forgetRaTyping();
      return;
    }
    fetchRaStatus()
      .then((s) => (raStatus = s))
      .catch(fail);
    return forgetRaTyping;
  });
  let raUsername = $derived(raUser ?? raStatus?.username ?? '');

  async function raSave() {
    if (raBusy || !raCanSave(raUsername, raToken)) return;
    raBusy = true;
    const token = raToken;
    raToken = '';
    raTokenTyped = false;
    try {
      const line = await saveRaToken(raUsername, token);
      pushToast(RA_SAVED, 'success', undefined, { sub: line });
      raUser = null;
      raStatus = await fetchRaStatus();
    } catch (err) {
      fail(err);
    } finally {
      raBusy = false;
    }
  }

  /** Log In: the password is traded for a token (the desktop's second form).
   *  The password is blanked before the call returns; the cards are raLogin's. */
  async function raLoginNow() {
    if (raBusy || !raCanLogin(raUsername, raPassword)) return;
    raBusy = true;
    const password = raPassword;
    raPassword = '';
    raPasswordTyped = false;
    try {
      const result = await raLogin(raUsername, password);
      if (result.kind === 'ok') {
        raToken = '';
        raTokenTyped = false;
        raUser = null;
      }
      raStatus = await fetchRaStatus();
    } catch (err) {
      fail(err);
    } finally {
      raBusy = false;
    }
  }

  async function raClear() {
    if (raBusy) return;
    raBusy = true;
    try {
      await clearRaCredentials();
      forgetRaTyping();
      raStatus = await fetchRaStatus();
    } catch (err) {
      fail(err);
    } finally {
      raBusy = false;
    }
  }

  // ---- Updates: the version, the last check and Debug logging.
  let version = $state('');
  let debug = $state<boolean | null>(null);
  let now = $state(new Date());
  $effect(() => {
    if (nav.pane !== 'updates') return;
    now = new Date();
    void refreshAppUpdateStatus();
    api
      .appVersion()
      .then((v) => (version = v))
      .catch(() => {});
    api
      .getDebugPrints()
      .then((on) => (debug = on))
      .catch(() => {});
  });

  function openRelease() {
    const rel = appUpdate.stored;
    if (!rel) return;
    void openList({
      title: releaseName(rel.tag),
      sub: rel.url,
      items: [
        {
          label: 'Open release in desktop mode',
          onPick: () =>
            void leaveTv('settings', 'updates').then(() => api.openReleasePage(rel.url).catch(() => {})),
        },
        ...(appUpdate.notice
          ? [
              {
                label: 'Dismiss this notice',
                onPick: () => {
                  dismissUpdate();
                  pushToast(UPDATE_DISMISSED);
                },
              },
            ]
          : []),
      ],
    });
  }

  function setDebug(on: boolean) {
    debug = on;
    api.setDebugPrints(on).catch((err) => {
      debug = !on;
      fail(err);
    });
  }

  function rowsFor(pane: TvPane): Row[] {
    switch (pane) {
      case 'appearance':
        return appearanceRows(
          {
            openOn: uiSettings.openOn,
            theme: uiSettings.theme,
            fade: uiSettings.backgroundFade,
            blur: uiSettings.backgroundBlur,
            hardwareAcceleration: uiSettings.hardwareAcceleration,
          },
          {
            open: openChoice,
            setOpenOn: (v) => void setOpenOn(v).catch(fail),
            setTheme: (v) => void setTheme(v).catch(() => {}),
            setArt: (on) => void setBackgroundEnabled(on).catch(fail),
            setFade: (v) => void commitBackgroundFade(v).catch(fail),
            setBlur: (v) => void commitBackgroundBlur(v).catch(fail),
            setHardwareAcceleration: (on) => void setHardwareAcceleration(on).catch(fail),
          },
        );
      case 'cloud-saves':
        return cloudRows(cloud, cloudError, saveCloud);
      case 'connection':
        return connectionRows(
          { connected: session.connected, offline: session.offline, serverUrl: session.serverUrl, username: session.username },
          {
            reconnect: () => void reconnect(),
            connectToServer,
            disconnect: (w) => void confirmBox({ ...w, danger: true, onOk: () => void disconnect() }),
          },
        );
      case 'library':
        return libraryRows({ path: settingsState.libPath, move: libraryMove.status }, changeLibrary);
      case 'retroachievements':
        return raRows(
          {
            username: raUsername,
            tokenTyped: raTokenTyped,
            tokenPresent: raStatus?.token_present ?? false,
            canSave: raCanSave(raUsername, raTokenTyped ? 'x' : ''),
            passwordTyped: raPasswordTyped,
            canLogin: raCanLogin(raUsername, raPasswordTyped ? 'x' : ''),
          },
          {
            editUsername: () => void openKeyboard({ title: 'Username', value: raUsername, onDone: (v) => (raUser = v) }),
            editToken: () =>
              void openKeyboard({
                title: 'Token',
                mask: true,
                onDone: (v) => {
                  raToken = v;
                  raTokenTyped = v.trim() !== '';
                },
              }),
            save: () => void raSave(),
            editPassword: () =>
              void openKeyboard({
                title: 'Password',
                mask: true,
                onDone: (v) => {
                  raPassword = v;
                  raPasswordTyped = v !== '';
                },
              }),
            login: () => void raLoginNow(),
            clear: () => void raClear(),
          },
        );
      case 'tv': {
        const pad = controllers.current;
        return tvRows(
          { prompts: uiSettings.tvPrompts, padName: controllers.name, assignment: controllers.assignment },
          {
            open: openChoice,
            setPrompts: (v) => void setTvPrompts(v).catch(fail),
            setAssignment: (v) => {
              if (pad) void setTvAssignment(pad.key, v).catch(fail);
            },
            toDesktop: () => void leaveTv(nav.tab, nav.pane),
            quit: () => void quitBox(),
          },
        );
      }
      case 'updates':
        return updatesRows({ version, checkedAt: appUpdate.checkedAt, now, release: appUpdate.stored, debug }, { openRelease, setDebug });
      default:
        // Systems draws itself (TvSystemsPane). TODO(P13): Backup drives.
        return [];
    }
  }

  let rows = $derived(rowsFor(nav.pane));

  useScreen({
    hints: (el) => (el?.hasAttribute('data-adj') ? [{ button: 'DPAD', label: 'Adjust' }] : []),
  });
</script>

<div class="split" data-testid="tv-settings">
  <TvRail label="Settings">
    {#each TV_PANES as p (p)}
      <TvRailRow k={`r-${p}`} selected={nav.pane === p} testId={`tv-set-rail-${p}`} onSelect={() => setPane(p)}>{TV_PANE_LABELS[p]}</TvRailRow>
    {/each}
  </TvRail>
  <div class="pane" id="pane" data-nav="v" data-enter="first" data-rail-target data-scroll="pane" data-testid="tv-pane">
    {#if nav.pane === 'systems'}
      <TvSystemsPane />
    {:else}
      <div class="pane-h"><h1 data-testid="tv-pane-title">{TV_PANE_LABELS[nav.pane]}</h1></div>
      <TvRows {rows} />
    {/if}
  </div>
</div>
