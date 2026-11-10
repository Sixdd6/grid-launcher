<script lang="ts">
  import { untrack } from 'svelte';
  import Connect from './lib/Connect.svelte';
  import { connectPrefill } from './lib/connectPrefill';
  import Shell from './lib/Shell.svelte';
  import ChooseSystems from './lib/setup/ChooseSystems.svelte';
  import Toast from './lib/Toast.svelte';
  import SetupProgress from './lib/setup/SetupProgress.svelte';
  import { session, restore, listenUnauthorized, finishSetup } from './lib/stores/session.svelte';
  import { initSetupRun, setupRun, startSetupRun } from './lib/stores/setupRun.svelte';
  import { init as initDownloads } from './lib/stores/downloads.svelte';
  import { init as initSessions } from './lib/stores/sessions.svelte';
  import { init as initCompatTools } from './lib/stores/compatTools.svelte';
  import { init as initUpdates } from './lib/stores/updates.svelte';
  import { init as initDrives } from './lib/stores/drives.svelte';
  import { initAppUpdate } from './lib/stores/appUpdate.svelte';
  import { initPythonImport } from './lib/stores/pythonImport.svelte';
  import { initLayoutMigration } from './lib/stores/layoutMigration.svelte';
  import { initLibraryMove } from './lib/stores/libraryMove.svelte';
  import { initLibraryUnreachable, libraryUnreachable } from './lib/stores/libraryUnreachable.svelte';
  import LibraryUnreachableDialog from './lib/LibraryUnreachableDialog.svelte';
  import { initReplenishListener } from './lib/stores/installed.svelte';
  import { initUiSettings, toggleTheme, uiSettings } from './lib/stores/uiSettings.svelte';
  import ThemeToggle from './lib/ThemeToggle.svelte';
  import { startShellLifetime } from './lib/lifetime.svelte';
  import { navigation } from './lib/stores/navigation.svelte';
  import { initUiMode, uiMode } from './lib/stores/uiMode.svelte';
  import TvShell from './lib/tv/TvShell.svelte';

  let restored = false; // restore() must fire once on mount, not on every phase change

  // Which interface is on screen (TV mode plan § 1.1): `--tv` has already
  // put Rust in TV mode by now, so ask before mounting either shell.
  $effect(() => {
    initUiMode();
  });

  // TV mode's CSS is scoped to html[data-ui="tv"] (lib/tv/tv.css), set only
  // while TvShell is mounted, so the desktop's layout is never touched.
  $effect(() => {
    const root = document.documentElement;
    if (uiMode.mode === 'tv') root.dataset.ui = 'tv';
    else delete root.dataset.ui;
  });

  // Declared before the restore effect below, and reading no reactive state,
  // so it registers once and as early as possible: `restore_session` and
  // `connect` both spawn the replenish job on the Rust side, and a job that
  // finished before the listener existed would leave the Library showing the
  // stale, cover-less rows until the next refresh. Registering it in
  // Shell.svelte was too late — the shell only mounts once the session is
  // already restored.
  $effect(() => {
    const un = initReplenishListener();
    return () => {
    };
  });

  // The self-update notice can arrive before the shell mounts, same
  // reasoning as initReplenishListener above.
  $effect(() => {
    const un = initAppUpdate();
    return () => {
    };
  });

  // The import ran before the window existed, so its notice is a pull with
  // nothing to unsubscribe from — but it belongs with the other pre-shell
  // effects so the toast appears at the first paint, not after a connect.
  $effect(() => {
    initPythonImport();
  });

  // The layout migration also ran before the window existed; same pull-only
  // shape, and its failure toast belongs at the first paint too.
  $effect(() => {
    initLayoutMigration();
  });

  // 9b: the unreachable-library check and a resumed library move both ran
  // before the window existed; pull their state at the first paint and
  // follow their events for the app's whole life.
  $effect(() => {
    const un = initLibraryUnreachable();
    return () => {
    };
  });

  $effect(() => {
    const un = initLibraryMove();
    return () => {
    };
  });

  // Q6: a 401 mid-session sends the user back to Connect, pre-filled. One
  // listener for the app's whole life, registered before `restore()` below.
  $effect(() => {
    const un = listenUnauthorized();
    return () => {
    };
  });

  // First-run setup's defaults land as emulators install, which can be long
  // after the picker handed over: one listener for the app's whole life.
  $effect(() => {
    const un = initSetupRun();
    return () => {
      un.then((f) => f());
    };
  });

  // The theme must be on `<html>` before the first paint the user sees, so
  // this registers alongside the other pre-shell effects rather than inside
  // Shell.svelte.
  $effect(() => {
    const un = initUiSettings();
    return () => {
    };
  });

  $effect(() => {
    if (!restored) {
      restored = true;
      restore();
    }
    const unDownloads = session.phase === 'shell' ? initDownloads() : undefined;
    const unSessions = session.phase === 'shell' ? initSessions() : undefined;
    const unCompatTools = session.phase === 'shell' ? initCompatTools() : undefined;
    const unUpdates = session.phase === 'shell' ? initUpdates() : undefined;
    const unDrives = session.phase === 'shell' ? initDrives() : undefined;
    return () => {
      unDownloads?.then((f) => f());
      unSessions?.then((f) => f());
      unCompatTools?.then((f) => f());
      unUpdates?.then((f) => f());
      unDrives?.then((f) => f());
    };
  });

  // The shell's lifetime listeners (notification cards, emulator notices,
  // cloud notices, the registry load): up exactly while a shell is on
  // screen — the phase is `shell` and the unreachable-library dialog is not
  // blocking — whichever shell that is.
  let shellUp = $derived(session.phase === 'shell' && !libraryUnreachable.blocking);
  $effect(() => {
    if (shellUp) return untrack(startShellLifetime);
  });
</script>

<!-- One shell at a time (TV mode plan § 1.1): TV mode's TvShell (its own
     startup screens included), or the desktop's screens below. Nothing
     until `get_ui_mode` answers, so a `--tv` start never flashes the
     desktop. -->
{#if !uiMode.loaded}
  <!-- waiting for get_ui_mode -->
{:else if uiMode.mode === 'tv'}
  <TvShell />
<!-- 9b: GRID cannot run without a valid library. While the library folder
     cannot be reached, the blocking dialog replaces the whole app. -->
{:else if libraryUnreachable.blocking}
  <LibraryUnreachableDialog />
{:else if session.phase === 'setup'}
  <ChooseSystems
    onDone={(started, names) => {
      if (started !== null) startSetupRun(started, names);
      finishSetup();
    }}
  />
{:else if session.phase === 'shell' && uiSettings.loaded}
  <!-- Waits for the stored settings: Open GRID on picks the shell's first
       view, once, when it mounts. -->
  <Shell initialView={uiMode.desktopStart?.view} initialSettingsPage={uiMode.desktopStart?.settingsPage ?? undefined} />
{:else if session.phase === 'none'}
  <Connect prefill={connectPrefill(session)} />
{/if}

<!-- The startup screens (Connect, first-run setup) carry the light / dark
     toggle in the window's top-right corner; the shell moves it into its
     top bar. -->
{#if uiMode.mode === 'desktop' && !libraryUnreachable.blocking && (session.phase === 'none' || session.phase === 'setup')}
  <ThemeToggle
    corner
    testId="theme-toggle-start"
    resolved={uiSettings.resolved}
    onToggle={() => toggleTheme().catch(() => {})}
  />
{/if}

<!-- The first-run setup card leads the notification stack while the shell
     is up; its Open Downloads switches the shell's view. -->
{#snippet setupCard()}
  <SetupProgress onOpenDownloads={() => navigation.go('downloads')} />
{/snippet}

<!-- Outside the phase branch: the startup Python-import notice is pushed
     before any session exists, and the user it was written for sees the
     Connect form, not the shell. `position: fixed` global chrome, so it
     needs no layout slot of its own. -->
<Toast lead={session.phase === 'shell' && setupRun.active ? setupCard : undefined} />
