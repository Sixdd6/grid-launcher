<script lang="ts">
  // Settings › Stores › Steam (GameStores README, "Steam"; prototype
  // `stSteamCard`): the client line, the Account radios (the accounts found
  // in Steam's files, or a SteamID64 typed in), the Web API key (saved: dots
  // with Change… and Remove; not saved: a password field), the help with the
  // steamcommunity.com/dev/apikey link, the Test result, then Save / Connect
  // Steam, Test, Show in Library and Disconnect… with its inline confirm.
  //
  // The key never sits in a Svelte store or `$state`: the field is read once
  // when Test or Save is pressed, goes straight to the command, and is
  // cleared after a Save. A saved key shows as dots only (user decision 2).
  import { untrack } from 'svelte';
  import { api, type StoreStatus, type SteamTestResult } from '../api';
  import Icon from '../Icon.svelte';
  import StoreMark from '../gamestores/desktop/StoreMark.svelte';
  import { refusedNotice, disconnectedNotice } from '../gamestores/notices';
  import {
    accountNote,
    ACCOUNT_HELP,
    ASKING_STEAM,
    CLIENT_FOUND,
    CLIENT_MISSING_LINE,
    clientPlaceIsPath,
    ENTER_STEAM_ID,
    initialManual,
    initialPick,
    KEY_HELP_AFTER,
    KEY_HELP_BEFORE,
    KEY_HELP_LINK,
    KEY_PLACEHOLDER,
    KEY_URL,
    keySaved,
    pickedSteamId,
    SAVED_KEY_DOTS,
    saveLabel,
    STEAM_DISCONNECT,
    STEAM_ID_PLACEHOLDER,
    STEAM_LEDE,
    steamCard,
    steamClientPlace,
    steamIdLine,
    steamPill,
    testLine,
  } from '../gamestores/steam';
  import { refreshLibrary } from '../stores/gameStores.svelte';
  import { showCard, showInLibrary, steamRecheck, steamRemoveKey } from '../gamestores/desktop/ui.svelte';

  let { status, active = true }: { status: StoreStatus; active?: boolean } = $props();

  let card = $derived(steamCard(status));
  let pill = $derived(steamPill(status));
  let on = $derived(status.connection !== 'not_connected');
  let place = $derived(steamClientPlace(status));

  let pick = $state('manual');
  let manual = $state('');
  let editKey = $state(false);
  let confirming = $state(false);
  let busy = $state(false);
  let result = $state<SteamTestResult | null>(null);
  /** The key field: read on Test / Save, never mirrored into state. */
  let keyField = $state<HTMLInputElement | null>(null);

  // The radios start on the saved SteamID64 (or the most recent local
  // account) and follow a change the backend makes (a Save, a Disconnect).
  let savedId = $derived(card?.steam_id ?? '');
  $effect(() => {
    void savedId;
    void on;
    pick = initialPick(status);
    manual = initialManual(status);
  });

  // Leaving the pane, or a change of connection (Connect, Disconnect),
  // forgets the form: no typed key stays in the field, no old result shows.
  let wasOn = untrack(() => on);
  $effect(() => {
    const leaving = !active || (wasOn && !on);
    wasOn = on;
    if (!leaving) return;
    confirming = false;
    untrack(() => {
      clearKey();
      result = null;
      editKey = false;
    });
  });

  let showSaved = $derived(keySaved(status) && !editKey);
  let line = $derived(result ? testLine(result) : null);

  function clearKey(): void {
    if (keyField) keyField.value = '';
  }

  function errText(err: unknown): string {
    return err instanceof Error ? err.message : String(err);
  }

  async function ask(save: boolean): Promise<void> {
    if (busy) return;
    const id = pickedSteamId(pick, manual);
    const typed = keyField?.value.trim() ?? '';
    busy = true;
    result = null;
    try {
      const r = save ? await api.steamSave(id, typed === '' ? undefined : typed) : await api.steamTest(id, typed === '' ? undefined : typed);
      result = r;
      if (save && (r.kind === 'ok' || r.kind === 'private')) {
        clearKey();
        editKey = false;
        void refreshLibrary('steam');
      }
    } catch (err) {
      showCard(refusedNotice(save ? 'Couldn’t save Steam' : 'Couldn’t test the key', errText(err)), 'steam');
    } finally {
      busy = false;
    }
  }

  async function removeKey(): Promise<void> {
    result = null;
    if (await steamRemoveKey()) {
      editKey = true;
      void refreshLibrary('steam');
    }
  }

  async function disconnect(): Promise<void> {
    confirming = false;
    try {
      await api.storeDisconnect('steam');
      clearKey();
      result = null;
      editKey = false;
      showCard(disconnectedNotice('steam'), 'steam');
    } catch (err) {
      showCard(refusedNotice('Couldn’t disconnect Steam', errText(err)));
    }
  }
</script>

<article class="st-store" class:off={!on} data-testid="store-card-steam">
  <header class="st-store-head">
    <StoreMark source="steam" size="lg" />
    <div class="st-store-id">
      <b>Steam</b>
      <span class="g-note" data-testid="store-card-account-steam">{steamIdLine(status)}</span>
    </div>
    <span class="pill {pill.tone}" data-testid="store-card-pill-steam">
      {#if pill.tone === 'ok'}<Icon name="check" size={12} />{/if}{pill.text}
    </span>
  </header>

  {#if confirming}
    <div class="confirm" data-testid="store-disconnect-confirm-steam">
      <p><b>{STEAM_DISCONNECT.bold}</b> {STEAM_DISCONNECT.rest}</p>
      <div class="g-actions">
        <button class="g-btn danger" data-testid="store-disconnect-go-steam" onclick={disconnect}>Disconnect</button>
        <button class="g-btn secondary" data-testid="store-disconnect-cancel-steam" onclick={() => (confirming = false)}>Cancel</button>
      </div>
    </div>
  {:else}
    <p class="g-note st-lede">{STEAM_LEDE}</p>

    {#if place !== null}
      <p class="okline" data-testid="steam-client-line">
        <Icon name="check" size={14} />{CLIENT_FOUND} ·
        {#if clientPlaceIsPath(place)}<code>{place}</code>{:else}{place}{/if}
      </p>
    {:else}
      <p class="errline" data-testid="steam-client-missing">
        {CLIENT_MISSING_LINE}
        <button class="small-btn" data-testid="steam-recheck" onclick={() => void steamRecheck()}>Check again</button>
      </p>
    {/if}

    <fieldset class="acct" data-testid="steam-accounts">
      <legend>Account</legend>
      {#each card?.accounts ?? [] as a (a.steam_id)}
        <label>
          <input type="radio" name="steam-acct" value={a.steam_id} bind:group={pick} data-testid={`steam-acct-${a.steam_id}`} />
          <b>{a.persona}</b> <code>{a.steam_id}</code> <span class="g-note">{accountNote(a.most_recent)}</span>
        </label>
      {/each}
      <label>
        <input type="radio" name="steam-acct" value="manual" bind:group={pick} data-testid="steam-acct-manual" />
        {ENTER_STEAM_ID}
      </label>
      {#if pick === 'manual'}
        <input class="field sid" data-testid="steam-id-field" placeholder={STEAM_ID_PLACEHOLDER} bind:value={manual} inputmode="numeric" autocomplete="off" />
      {/if}
      <span class="help">{ACCOUNT_HELP}</span>
    </fieldset>

    <div class="sub">
      <h4>Web API key</h4>
      {#if showSaved}
        <div class="keyrow">
          <input class="field" value={SAVED_KEY_DOTS} disabled aria-label="Web API key, saved" data-testid="steam-key-saved" />
          <button class="small-btn" data-testid="steam-key-change" onclick={() => (editKey = true)}>Change…</button>
          <button class="small-btn" data-testid="steam-key-remove" onclick={removeKey}>Remove</button>
        </div>
      {:else}
        <div class="keyrow">
          <input
            class="field"
            type="password"
            data-testid="steam-key-field"
            placeholder={KEY_PLACEHOLDER}
            autocomplete="off"
            spellcheck="false"
            aria-label="Web API key"
            bind:this={keyField}
          />
        </div>
      {/if}
      <span class="help">{KEY_HELP_BEFORE}<a class="linkish" href={KEY_URL} target="_blank" rel="noreferrer">{KEY_HELP_LINK}</a>{KEY_HELP_AFTER}</span>
      {#if busy}
        <p class="g-note asking" data-testid="steam-test-busy"><span class="spin"></span> {ASKING_STEAM}</p>
      {:else if line}
        {#if line.ok}
          <p class="okline" data-testid="steam-test-result"><Icon name="check" size={14} />{line.text}</p>
        {:else}
          <p class="errline" data-testid="steam-test-result">{line.text}</p>
          {#if line.help}<p class="help" data-testid="steam-test-help">{line.help}</p>{/if}
        {/if}
      {/if}
    </div>

    <div class="g-actions">
      <button class="g-btn" data-testid="steam-save" disabled={busy} onclick={() => void ask(true)}>{saveLabel(status)}</button>
      <button class="g-btn secondary" data-testid="steam-test" disabled={busy} onclick={() => void ask(false)}>Test</button>
      {#if on}
        <button class="g-btn secondary" data-testid="store-show-steam" onclick={() => showInLibrary('steam')}>Show in Library</button>
        <button class="g-btn secondary destructive" data-testid="store-disconnect-steam" onclick={() => (confirming = true)}>Disconnect…</button>
      {/if}
    </div>
  {/if}
</article>

<style>
  .st-lede {
    max-width: 640px;
  }

  .st-store {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px 16px;
    border-radius: var(--r-row);
    border: 1px solid var(--line);
    background: var(--surface);
  }

  .st-store.off {
    border-style: dashed;
    border-color: var(--line-strong);
  }

  .st-store-head {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .st-store-id {
    display: flex;
    flex-direction: column;
    gap: 1px;
    margin-right: auto;
    min-width: 0;
  }

  .st-store-id b {
    color: var(--text-h);
    font-size: 14px;
  }

  /* The prototype's `.pill-s` (Settings › Systems' status pill). */
  .pill {
    display: inline-flex;
    align-items: center;
    gap: 4px;
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

  .pill.warn {
    background: var(--warning);
    border-color: var(--warning);
    color: #1a1a12;
  }

  code {
    font: 12px ui-monospace, monospace;
  }

  /* Prototype `.st-okline` / `.st-errline`. */
  .okline {
    margin: 0;
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
    color: var(--text-h);
    font-size: 13px;
  }

  .okline :global(svg) {
    color: var(--primary);
  }

  .errline {
    margin: 0;
    color: var(--danger);
    font-size: 13px;
  }

  .errline .small-btn {
    margin-left: 4px;
  }

  /* Prototype `.st-acct`. */
  .acct {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 0;
    padding: 0;
    border: 0;
  }

  .acct legend {
    padding: 0;
    margin-bottom: 4px;
    font-size: 12px;
    color: var(--text-muted);
  }

  .acct label {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
    font-size: 13px;
    color: var(--text);
    cursor: pointer;
  }

  .acct label b {
    color: var(--text-h);
  }

  .acct input[type='radio'] {
    accent-color: var(--primary);
    margin: 0;
  }

  .acct code {
    color: var(--text-muted);
  }

  .field {
    font: inherit;
    font-size: 13px;
    padding: 7px 10px;
    border-radius: var(--r-control);
    border: 1px solid var(--line);
    background: var(--field);
    color: var(--text-h);
    min-width: 0;
  }

  .field:disabled {
    color: var(--text-muted);
  }

  .field.sid {
    max-width: 320px;
  }

  .sub {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .sub h4 {
    margin: 0;
    font-size: 13px;
    color: var(--text-h);
  }

  /* Prototype `.st-keyrow`. */
  .keyrow {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
  }

  .keyrow .field {
    flex: 1 1 260px;
    font-family: ui-monospace, monospace;
  }

  .help {
    margin: 0;
    font-size: 12px;
    color: var(--text-muted);
  }

  .asking {
    margin: 0;
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .confirm {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px 14px;
    border-radius: var(--r-row);
    border: 1px solid var(--danger);
    background: color-mix(in srgb, var(--danger) 6%, transparent);
  }

  .confirm p {
    margin: 0;
    font-size: 13px;
    color: var(--text);
  }

  .confirm b {
    color: var(--text-h);
  }

  .g-btn.danger {
    background: var(--danger);
    border-color: var(--danger);
    color: var(--on-danger);
  }

  .g-btn.secondary.destructive {
    color: var(--danger);
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
  }

  .linkish {
    color: var(--primary);
    text-decoration: underline;
    text-underline-offset: 2px;
  }

  .spin {
    display: inline-block;
    width: 10px;
    height: 10px;
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
    .spin {
      animation: none;
    }
  }
</style>
