<script lang="ts">
  // The first-run welcome (RommStorefront README, "Welcome (replaces
  // Connect)"; prototype `romm.js` `rwRender` / `rwRow`, `romm.css` `.rw-*`).
  // Step 1 is the library folder (required); step 2 the storefront rows by
  // name: Amazon, Epic and GOG open their login window over the welcome, RomM
  // and Steam open their form inline. Skip for now while nothing is connected,
  // then Continue alone (`welcome.ts`). Continue or Skip stores the folder and
  // calls `finishWelcome()`: Choose your systems after a RomM connect that
  // needs setup, else the shell.
  //
  // The theme toggle (App.svelte), the notifications (Toast) and the store
  // login window (its own window) all sit above this screen.
  import { untrack } from 'svelte';
  import type { RommStatus } from './api';
  import Icon from './Icon.svelte';
  import { pickFolder } from './pickers';
  import StoreMark from './gamestores/desktop/StoreMark.svelte';
  import { initStoreCards, signIn } from './gamestores/desktop/ui.svelte';
  import SteamCard from './settings/SteamCard.svelte';
  import RommMark from './romm/RommMark.svelte';
  import RommSignInForm from './romm/RommSignInForm.svelte';
  import { signedInToast } from './romm/signIn';
  import type { Storefront } from './romm/order';
  import type { SignInMode } from './romm/words';
  import { gameStores, initGameStores } from './stores/gameStores.svelte';
  import { pythonImport } from './stores/pythonImport.svelte';
  import { romm, setupNeeded as checkSetupNeeded } from './stores/romm.svelte';
  import { finishWelcome, session } from './stores/session.svelte';
  import { pushToast, TOAST_DURATION_MS } from './stores/toasts.svelte';
  import {
    CONNECTED_PILL,
    FOLDER_STEP,
    FOOTER,
    folderMissing,
    IMPORT_NOTICE,
    initialOpenRow,
    ROMM_EDIT,
    STEAM_ROW,
    STORE_CONNECT,
    STORES_STEP,
    WELCOME_LEDE,
    WELCOME_TITLE,
    welcomeFooter,
    welcomeRows,
  } from './welcome';
  import { loadLibraryFolder, saveLibraryFolder } from './welcomeFlow';
  import { WELCOME_ROMM_ROW } from './romm/words';

  // The stores' statuses and sign-in events: the shell starts them too
  // (`startShellLifetime`, StoreLayer), sharing these, so nothing is stopped
  // here. A finished sign-in shows its card (Toast is above the welcome).
  $effect(() => {
    untrack(() => {
      void initGameStores();
      initStoreCards();
    });
  });

  let folder = $state('');
  let stored = $state('');
  let folderKnown = $state(false);
  let folderError = $state<string | null>(null);

  $effect(() => {
    void loadLibraryFolder().then((start) => {
      stored = start.stored;
      // A folder typed (or browsed) before the stored one arrived wins.
      if (folder === '') folder = start.initial;
      folderKnown = true;
    });
  });

  let imported = $derived(pythonImport.report !== null);
  let rows = $derived(welcomeRows(romm.status, gameStores.statuses));
  let rommConnected = $derived(romm.status.connection === 'connected');

  /** The row whose form is open (RomM, Steam), or none. */
  let open = $state<Storefront | null>(null);
  // A Python import opens RomM's row (decision 12), once, unless the user has
  // already opened or closed a row.
  let touched = false;
  $effect(() => {
    const start = initialOpenRow(imported, untrack(() => rommConnected));
    if (start !== null && !touched) open = start;
  });

  function openRow(id: Storefront | null) {
    touched = true;
    open = id;
  }

  // "Next: choose the systems you play." once RomM connected here and setup
  // is needed (decision 9: Choose your systems follows Continue).
  let setupNeeded = $state(false);
  $effect(() => {
    if (!rommConnected) {
      setupNeeded = false;
      return;
    }
    void checkSetupNeeded().then((needed) => {
      setupNeeded = needed;
    });
  });

  let finishing = $state(false);
  let footer = $derived(welcomeFooter({ folder, rows, setupNeeded, busy: finishing || !folderKnown }));

  let rommMode = $derived<SignInMode>(rommConnected ? 'edit' : 'connect');

  function rommConnectedHere(status: RommStatus) {
    pushToast(signedInToast(rommMode, status), 'success', TOAST_DURATION_MS);
    open = null;
  }

  async function browse() {
    const picked = await pickFolder(FOLDER_STEP.pickerTitle);
    if (picked !== null) {
      folder = picked;
      folderError = null;
    }
  }

  /** Continue or Skip for now: the folder first, then on. */
  async function finish() {
    if (finishing || folderMissing(folder)) return;
    finishing = true;
    folderError = null;
    try {
      const refused = await saveLibraryFolder(folder, stored);
      if (refused !== null) {
        folderError = refused;
        return;
      }
      stored = folder.trim();
      await finishWelcome();
    } finally {
      finishing = false;
    }
  }
</script>

<div class="welcome" data-testid="welcome">
  <section class="fr-card">
    <header class="fr-head">
      <span class="logo"><Icon name="grid" size={28} /></span>
      <h1>{WELCOME_TITLE}</h1>
      <p class="lede">{WELCOME_LEDE}</p>
    </header>

    {#if imported}
      <p class="warn" data-testid="welcome-import-notice"><b>{IMPORT_NOTICE.bold}</b> {IMPORT_NOTICE.rest}</p>
    {/if}

    <div class="step" data-testid="welcome-step-folder">
      <h2><span class="n">1</span>{FOLDER_STEP.title}</h2>
      <p class="g-note">{FOLDER_STEP.note}</p>
      <span class="inline">
        <input
          class="g-field"
          data-testid="welcome-library-path"
          aria-label={FOLDER_STEP.label}
          bind:value={folder}
          oninput={() => (folderError = null)}
          spellcheck="false"
          autocomplete="off"
        />
        <button type="button" class="g-btn secondary" data-testid="welcome-library-browse" onclick={browse}>{FOLDER_STEP.browse}</button>
      </span>
      {#if folderKnown && folderMissing(folder)}
        <p class="errline" data-testid="welcome-library-missing">{FOLDER_STEP.missing}</p>
      {/if}
      {#if folderError}<p class="errline" role="alert" data-testid="welcome-library-error">{folderError}</p>{/if}
    </div>

    <div class="step" data-testid="welcome-step-stores">
      <h2><span class="n">2</span>{STORES_STEP.title} <span class="muted">{STORES_STEP.optional}</span></h2>
      <p class="g-note">{STORES_STEP.note}</p>
      <ul class="rows">
        {#each rows as row (row.id)}
          {@const isOpen = open === row.id}
          <li class="row" class:open={isOpen} class:on={row.connected} data-testid={`welcome-row-${row.id}`}>
            <div class="head">
              {#if row.id === 'romm'}<RommMark size="lg" />{:else}<StoreMark source={row.id} size="lg" />{/if}
              <span class="id">
                <b>{row.name}</b>
                <span class="sub" data-testid={`welcome-row-sub-${row.id}`}>{row.sub}</span>
              </span>
              {#if row.connected}
                <span class="pill ok" data-testid={`welcome-row-pill-${row.id}`}><Icon name="check" size={12} />{CONNECTED_PILL}</span>
                {#if row.id === 'romm' && !isOpen}
                  <button class="small-btn" data-testid="welcome-romm-edit" onclick={() => openRow('romm')}>{ROMM_EDIT}</button>
                {/if}
              {:else if !isOpen}
                {#if row.kind === 'login' && row.id !== 'romm' && row.id !== 'steam'}
                  {@const source = row.id}
                  <button class="small-btn" data-testid={`welcome-connect-${row.id}`} onclick={() => void signIn(source)}>{STORE_CONNECT}</button>
                {:else if row.kind === 'romm'}
                  <button class="small-btn" data-testid="welcome-connect-romm" onclick={() => openRow('romm')}>{WELCOME_ROMM_ROW.action}</button>
                {:else}
                  <button class="small-btn" data-testid="welcome-connect-steam" onclick={() => openRow('steam')}>{STEAM_ROW.action}</button>
                {/if}
              {/if}
            </div>
            {#if isOpen && row.kind === 'romm'}
              <div class="body">
                <RommSignInForm
                  mode={rommMode}
                  from="welcome"
                  testId="welcome-romm"
                  submitLabel={WELCOME_ROMM_ROW.action.replace('…', '')}
                  onConnected={rommConnectedHere}
                  onCancel={() => openRow(null)}
                />
              </div>
            {:else if isOpen && row.kind === 'steam'}
              {@const steam = gameStores.status('steam')}
              <div class="body">
                {#if steam !== null}
                  <SteamCard status={steam} active={isOpen} />
                {:else}
                  <p class="g-note">Looking for Steam…</p>
                {/if}
                <div class="actions">
                  <button class="g-btn secondary" data-testid="welcome-steam-close" onclick={() => openRow(null)}>{STEAM_ROW.close}</button>
                </div>
              </div>
            {/if}
          </li>
        {/each}
      </ul>
    </div>

    <footer class="fr-foot">
      <div class="actions">
        {#if footer.skipShown}
          <button class="g-btn secondary" data-testid="welcome-skip" disabled={!footer.skipEnabled} onclick={finish}>{FOOTER.skip}</button>
        {/if}
        <button class="g-btn" data-testid="welcome-continue" disabled={!footer.continueEnabled} onclick={finish}>{FOOTER.continue}</button>
      </div>
      <p class="help" data-testid="welcome-help">{footer.help}</p>
      {#if session.error}<p class="errline" role="alert" data-testid="welcome-error">{session.error}</p>{/if}
    </footer>
  </section>
</div>

<style>
  /* Prototype `fresh.css` `.fresh` / `.fr-card` and `romm.css` `.fr-card.rw`:
     a centred 640px column that scrolls in the window, 32px above, 20px
     between blocks. `#app` is one viewport tall. */
  .welcome {
    flex: 1 1 auto;
    min-height: 0;
    overflow-y: auto;
    display: flex;
    justify-content: center;
    background: var(--bg);
  }

  .fr-card {
    width: min(640px, 100%);
    margin: auto;
    padding: 32px 24px;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    gap: 20px;
  }

  .fr-head {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    text-align: center;
  }

  .logo {
    display: flex;
    color: var(--primary);
  }

  h1 {
    margin: 0;
    font-size: 22px;
    font-weight: 600;
    color: var(--text-h);
  }

  .lede {
    margin: 0;
    font-size: 13px;
    color: var(--text-muted);
  }

  /* Prototype `.bk-warn`. */
  .warn {
    margin: 0;
    padding: 8px 10px;
    border-radius: var(--r-control);
    border-left: 3px solid var(--warning);
    background: color-mix(in srgb, var(--warning) 12%, transparent);
    color: var(--text);
    font-size: 13px;
  }

  .warn b {
    color: var(--text-h);
  }

  .step {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  h2 {
    margin: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 15px;
    font-weight: 600;
    color: var(--text-h);
  }

  h2 .muted {
    font-weight: 400;
    font-size: 12px;
    color: var(--text-muted);
  }

  .n {
    display: inline-grid;
    place-items: center;
    width: 22px;
    height: 22px;
    flex: none;
    border-radius: 50%;
    background: var(--primary);
    color: var(--on-primary);
    font-size: 12px;
    font-weight: 700;
  }

  .inline {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .inline .g-field {
    flex: 1 1 auto;
  }

  .errline {
    margin: 0;
    color: var(--danger);
    font-size: 13px;
  }

  .rows {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .row {
    border: 1px solid var(--line);
    border-radius: var(--r-row);
    background: var(--surface);
  }

  .row.open {
    border-color: var(--line-strong);
  }

  .head {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
  }

  .id {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
    flex: 1 1 auto;
  }

  .id b {
    color: var(--text-h);
    font-size: 14px;
  }

  .sub {
    font-size: 12px;
    color: var(--text-muted);
  }

  /* Prototype `.pill-s`. */
  .pill {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    flex: none;
    font-size: 12px;
    font-weight: 600;
    padding: 2px 10px;
    border-radius: var(--r-pill);
    border: 1px solid var(--line);
    color: var(--text-muted);
    white-space: nowrap;
  }

  .pill.ok {
    color: var(--text-h);
  }

  .pill.ok :global(svg) {
    color: var(--primary);
  }

  .small-btn {
    flex: none;
    font: inherit;
    font-size: 12px;
    padding: 5px 10px;
    border-radius: var(--r-chip);
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text-h);
    cursor: pointer;
  }

  .small-btn:hover {
    background: var(--surface-2, var(--field));
  }

  /* Prototype `.rw-body`: the form under the row's name. */
  .body {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 2px 12px 12px 58px;
  }

  /* RomM's form: Connect, then Cancel, on the left (prototype `rwRow`). */
  .body :global(.signin .foot) {
    flex-direction: row-reverse;
    justify-content: flex-end;
  }

  .body :global(.signin .foot .g-btn) {
    padding: 6px 14px;
  }

  /* Steam's card without its frame, head and lede (prototype `.rw-body .st-store`). */
  .body :global(article.st-store) {
    padding: 0;
    border: 0;
    background: none;
  }

  .body :global(article.st-store > .st-store-head),
  .body :global(article.st-store > .st-lede) {
    display: none;
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .body .actions .g-btn {
    padding: 6px 14px;
  }

  .fr-foot {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 8px;
  }

  .help {
    margin: 0;
    font-size: 11px;
    color: var(--text-muted);
    text-align: right;
  }

  input:focus-visible,
  button:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: 1px;
  }
</style>
