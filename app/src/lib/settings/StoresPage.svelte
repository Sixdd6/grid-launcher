<script lang="ts">
  // Settings › Stores (GameStores README, "Settings › Stores"; prototype
  // `renderStoresPane`, `stStoreCard`). A lede, then one card per store in
  // the rail's order (Epic Games, GOG). Words come from
  // `gamestores/settings.ts`.
  import { api, type StoreStatus } from '../api';
  import Icon from '../Icon.svelte';
  import { pickFolder } from '../pickers';
  import { gameStores } from '../stores/gameStores.svelte';
  import StoreMark from '../gamestores/desktop/StoreMark.svelte';
  import { hasCloud, hasInstaller, storeName } from '../gamestores/marks';
  import { disconnectedNotice, storeCloudSwitchNotice, refusedNotice } from '../gamestores/notices';
  import {
    accountLine,
    CLOUD_SWITCH_TEXT,
    disconnectConfirm,
    expiredWarning,
    licenseLine,
    notConnectedLede,
    ownedLine,
    STORES_LEDE,
    storePill,
    toolLine,
  } from '../gamestores/settings';
  import { installTool, openInstaller, refreshStore, showCard, showInLibrary, signIn } from '../gamestores/desktop/ui.svelte';

  let { active = true }: { active?: boolean } = $props();

  let confirmFor = $state<string | null>(null);
  let now = $state(Math.floor(Date.now() / 1000));

  $effect(() => {
    if (!active) confirmFor = null;
    now = Math.floor(Date.now() / 1000);
  });

  let statuses = $derived(gameStores.statuses);

  async function disconnect(s: StoreStatus) {
    confirmFor = null;
    try {
      await api.storeDisconnect(s.source);
      showCard(disconnectedNotice(s.source), s.source);
    } catch (err) {
      showCard(refusedNotice(`Couldn’t disconnect ${storeName(s.source)}`, err instanceof Error ? err.message : String(err)));
    }
  }

  async function changeFolder(s: StoreStatus) {
    const picked = await pickFolder(`Install folder for ${storeName(s.source)} games`);
    if (!picked) return;
    try {
      await api.storeSetInstallDir(s.source, picked);
    } catch (err) {
      showCard(refusedNotice('Couldn’t change the install folder', err instanceof Error ? err.message : String(err)));
    }
  }

  async function setCloud(s: StoreStatus, on: boolean) {
    try {
      await api.storeSetCloudSaves(s.source, on);
      showCard(storeCloudSwitchNotice(s.source, on));
    } catch (err) {
      showCard(refusedNotice('Couldn’t change cloud saves', err instanceof Error ? err.message : String(err)));
    }
  }
</script>

<div data-testid="settings-stores-pane" class="stores">
  <p class="g-note lede">{STORES_LEDE}</p>

  {#if !gameStores.loaded}
    <p class="g-note">Looking for stores…</p>
  {/if}

  {#each statuses as s (s.source)}
    {@const pill = storePill(s)}
    {@const tool = toolLine(s)}
    {@const lic = licenseLine(s)}
    {@const on = s.connection !== 'not_connected'}
    {@const owned = ownedLine(s, now)}
    {@const loginError = gameStores.loginError(s.source)}
    <article class="st-store" class:off={!on} data-testid={`store-card-${s.source}`}>
      <header class="st-store-head">
        <StoreMark source={s.source} size="lg" />
        <div class="st-store-id">
          <b>{storeName(s.source)}</b>
          <span class="g-note" data-testid={`store-card-account-${s.source}`}>{accountLine(s)}</span>
        </div>
        <span class="pill {pill.tone}" data-testid={`store-card-pill-${s.source}`}>
          {#if pill.tone === 'ok'}<Icon name="check" size={12} />{/if}{pill.text}
        </span>
      </header>

      {#if confirmFor === s.source}
        {@const c = disconnectConfirm(s)}
        <div class="confirm" data-testid={`store-disconnect-confirm-${s.source}`}>
          <p><b>{c.bold}</b> {c.rest}</p>
          <div class="g-actions">
            <button class="g-btn danger" data-testid={`store-disconnect-go-${s.source}`} onclick={() => disconnect(s)}>Disconnect</button>
            <button class="g-btn secondary" data-testid={`store-disconnect-cancel-${s.source}`} onclick={() => (confirmFor = null)}>Cancel</button>
          </div>
        </div>
      {:else if !on}
        <p class="g-note st-lede">{notConnectedLede(s)}</p>
        {#if loginError}<p class="errline" data-testid={`store-login-error-${s.source}`}>{loginError}</p>{/if}
        <div class="g-actions">
          <button class="g-btn" data-testid={`store-connect-${s.source}`} onclick={() => signIn(s.source)}>Connect {storeName(s.source)}…</button>
          {#if hasInstaller(s.source)}
            <button class="g-btn secondary" data-testid="store-installer-open-settings" onclick={openInstaller}>Install from installer…</button>
          {/if}
        </div>
        <dl class="g-kv">
          <div><dt>Store tool</dt><dd><span class="tool">{tool.text}</span></dd></div>
          <div>
            <dt>License</dt>
            <dd>{lic.text} · <a class="linkish" href={lic.url} target="_blank" rel="noreferrer">{lic.label}</a></dd>
          </div>
        </dl>
      {:else}
        {#if s.connection === 'expired'}
          {@const w = expiredWarning(s)}
          <p class="warnline" data-testid={`store-expired-${s.source}`}><b>{w.bold}</b> {w.rest}</p>
        {/if}
        {#if loginError}<p class="errline" data-testid={`store-login-error-${s.source}`}>{loginError}</p>{/if}
        <dl class="g-kv">
          <div><dt>Account</dt><dd>{s.username}</dd></div>
          <div>
            <dt>Games you own</dt>
            <dd data-testid={`store-card-owned-${s.source}`}>
              {#if owned === null}<span class="spin"></span>&nbsp;Loading…{:else}{owned}{/if}
            </dd>
          </div>
          <div>
            <dt>Store tool</dt>
            <dd>
              <span class="tool" data-testid={`store-card-tool-${s.source}`}>
                {#if tool.kind === 'installing'}
                  <span>{tool.text}</span><span class="bar"><b style:width={`${tool.pct}%`}></b></span>
                {:else if tool.kind === 'failed' || tool.kind === 'python'}
                  <span class="errline">{tool.text}</span>
                  {#if tool.kind === 'failed'}<button class="small-btn primary" onclick={() => installTool(s.source)}>{tool.button}</button>{/if}
                {:else if tool.kind === 'missing'}
                  <span>{tool.text}</span><button class="small-btn primary" onclick={() => installTool(s.source)}>{tool.button}</button>
                {:else}
                  <span>{tool.text}</span>
                {/if}
              </span>
            </dd>
          </div>
          <div>
            <dt>License</dt>
            <dd>{lic.text} · <a class="linkish" href={lic.url} target="_blank" rel="noreferrer">{lic.label}</a></dd>
          </div>
          <div>
            <dt>Install folder</dt>
            <dd class="folder"><code>{s.install_dir}</code><button class="small-btn" onclick={() => changeFolder(s)}>Change…</button></dd>
          </div>
          {#if hasCloud(s.source)}
            <div>
              <dt>Cloud saves</dt>
              <dd>
                <label class="g-check">
                  <input
                    type="checkbox"
                    data-testid={`store-cloud-switch-${s.source}`}
                    checked={s.cloud_saves}
                    onchange={(e) => setCloud(s, (e.currentTarget as HTMLInputElement).checked)}
                  />
                  {CLOUD_SWITCH_TEXT}
                </label>
              </dd>
            </div>
          {/if}
        </dl>
        <div class="g-actions">
          {#if s.connection === 'expired'}
            <button class="g-btn" data-testid={`store-signin-${s.source}`} onclick={() => signIn(s.source)}>Sign in again…</button>
          {:else}
            <button class="g-btn" data-testid={`store-show-${s.source}`} onclick={() => showInLibrary(s.source)}>Show in Library</button>
          {/if}
          {#if hasInstaller(s.source)}
            <button class="g-btn secondary" data-testid="store-installer-open-settings" onclick={openInstaller}>Install from installer…</button>
          {/if}
          <button class="g-btn secondary" data-testid={`store-refresh-${s.source}`} onclick={() => refreshStore(s.source)}>Refresh games</button>
          <button class="g-btn secondary destructive" data-testid={`store-disconnect-${s.source}`} onclick={() => (confirmFor = s.source)}
            >Disconnect…</button
          >
        </div>
      {/if}
    </article>
  {/each}
</div>

<style>
  .stores {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .lede,
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

  .st-store :global(.g-kv > div) {
    grid-template-columns: 150px minmax(0, 1fr);
    padding: 6px 0;
  }

  .st-store dd {
    flex-wrap: wrap;
    gap: 6px 8px;
  }

  .st-store code {
    font: 12px ui-monospace, monospace;
  }

  .tool {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }

  .bar {
    display: block;
    width: 140px;
    height: 6px;
    border-radius: var(--r-pill);
    background: var(--field);
    overflow: hidden;
  }

  .bar b {
    display: block;
    height: 100%;
    background: var(--primary);
  }

  .errline {
    margin: 0;
    color: var(--danger);
    font-size: 13px;
  }

  .warnline {
    margin: 0;
    padding: 8px 12px;
    border-radius: var(--r-row);
    border-left: 3px solid var(--warning);
    background: color-mix(in srgb, var(--warning) 10%, transparent);
    font-size: 13px;
    color: var(--text);
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

  .small-btn.primary {
    background: var(--primary);
    border-color: var(--primary);
    color: var(--on-primary);
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
