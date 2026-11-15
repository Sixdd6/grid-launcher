<script lang="ts">
  // The welcome, step 1 (TVRommStorefront README § The welcome; prototype
  // `rWelcome`): "Welcome to GRID", the Library folder field (A "Choose"
  // opens the folder browser, whose Y uses the folder shown), the line under
  // it and Next, disabled until a folder is chosen. Next stores the folder
  // and pushes step 2. After a Python import the folder is filled in and a
  // dialog says so first. Start opens the short Menu (Switch to desktop
  // mode, Quit GRID), as on Connect before.
  import { tick } from 'svelte';
  import { api } from '../../api';
  import { tvf } from '../focus/actions';
  import { openFolderBrowser } from '../gameActions';
  import { openLayer, pushPage, scope, setFocus } from '../nav.svelte';
  import TvAlert from '../overlays/TvAlert.svelte';
  import TvButton from '../ui/TvButton.svelte';
  import TvWelcomeStores from './TvWelcomeStores.svelte';
  import { welcome } from './welcomeState.svelte';
  import {
    folderRefusal,
    IMPORTED_DIALOG,
    IMPORTED_NOTE,
    LIBRARY_BROWSER_TITLE,
    LIBRARY_LABEL,
    LIBRARY_LINE,
    LIBRARY_PLACEHOLDER,
    NEXT,
    NEXT_WHY,
    nextReady,
    parentOf,
    STEP_1,
    WELCOME_LEDE,
    WELCOME_TITLE,
    CONNECT_TITLE,
  } from './welcomePage';

  let ready = $derived(nextReady(welcome.lib));

  // After a Python import, the dialog first, once.
  $effect(() => {
    if (!welcome.loaded || !welcome.imported || welcome.dialogShown) return;
    welcome.dialogShown = true;
    void openLayer({ component: TvAlert, props: { title: IMPORTED_DIALOG.title, body: IMPORTED_DIALOG.body, testId: 'tv-welcome-imported' } });
  });

  async function choose() {
    const start = welcome.lib !== '' ? parentOf(welcome.lib) : parentOf(await api.suggestedLibraryPath().catch(() => ''));
    openFolderBrowser({
      title: LIBRARY_BROWSER_TITLE,
      start,
      onPick: (p) => {
        welcome.lib = p.trim();
        welcome.error = null;
        // On to Next, as the prototype does.
        void tick().then(() => setFocus(scope()?.querySelector<HTMLElement>('[data-k="w-next"]') ?? null));
      },
    });
  }

  async function next() {
    if (!ready || welcome.busy) return;
    const lib = welcome.lib.trim();
    if (lib !== welcome.libStored) {
      welcome.busy = true;
      try {
        await api.setLibraryPath(lib);
        welcome.libStored = lib;
      } catch (err) {
        welcome.error = folderRefusal(err);
        return;
      } finally {
        welcome.busy = false;
      }
    }
    welcome.error = null;
    void pushPage({ title: CONNECT_TITLE, component: TvWelcomeStores, props: {} });
  }
</script>

<div class="start" data-testid="tv-welcome">
  <div class="start-box rwel" data-nav="v">
    <div class="rstep">{STEP_1}</div>
    <h1>{WELCOME_TITLE}</h1>
    <p class="sub">{WELCOME_LEDE}</p>
    {#if welcome.imported}<p class="note" data-testid="tv-welcome-imported-note">{IMPORTED_NOTE}</p>{/if}
    {#if welcome.error}<p class="errline" role="alert" data-testid="tv-welcome-error">{welcome.error}</p>{/if}
    <div class="start-body" data-nav="v" id="wel-rows">
      <div class="field" data-k="w-lib" data-al="Choose" data-testid="tv-welcome-lib" use:tvf={{ a: () => void choose() }}>
        <span class="lab">{LIBRARY_LABEL}</span><span class="v" class:ph={welcome.lib === ''} data-testid="tv-welcome-lib-value">{welcome.lib || LIBRARY_PLACEHOLDER}</span>
      </div>
      <p class="note" style="margin:.4rem 1.2rem 0">{LIBRARY_LINE}</p>
    </div>
    <div class="btns start-foot" data-nav="h">
      <TvButton k="w-next" primary disabled={!ready} why={ready ? undefined : NEXT_WHY} testId="tv-welcome-next" onA={() => void next()}>{NEXT}</TvButton>
    </div>
  </div>
</div>
