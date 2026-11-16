<script lang="ts">
  // A system's firmware block (RommStorefront README, "Firmware block";
  // prototype `rmFwBlock`): "Firmware · 3 files · 2 installed", Install from
  // RomM and Locate files…, the reason line, then one line per expected
  // file. The model is `firmwareBlock.ts`; the actions are the caller's.
  import Icon from '../Icon.svelte';
  import { REASON_WORDS, type FirmwareBlockView, type FirmwareFileLine } from './firmwareBlock';

  let {
    view,
    testKey,
    onRomm,
    onLocate,
    onConnectRomm,
  }: {
    view: FirmwareBlockView;
    testKey: string;
    onRomm: () => void;
    onLocate: () => void;
    onConnectRomm: () => void;
  } = $props();

  let showOptional = $state(false);
</script>

{#snippet fileLine(f: FirmwareFileLine)}
  <li data-testid={`sys-fw-file-${testKey}-${f.name}`} data-state={f.state}>
    {#if f.state === 'verified'}
      <span class="st ok"><Icon name="check" size={12} /></span>
    {:else if f.state === 'unverified'}
      <span class="st info" title="Not verified">?</span>
    {:else if f.state === 'missing'}
      <span class="st warn">!</span>
    {:else}
      <span class="st off"></span>
    {/if}
    <code>{f.name}</code>
    <span class="muted-s">{f.meta}</span>
    <span class="fw-st" class:warn={f.state === 'missing'}>{f.status}</span>
  </li>
{/snippet}

<div class="fw" data-testid={`sys-fw-${testKey}`}>
  <div class="fw-head">
    <b>Firmware</b>
    <span class="muted-s" data-testid={`sys-fw-count-${testKey}`}>{view.count}</span>
    <span class="acts">
      <button
        class="small-btn"
        class:primary={view.romm.primary}
        data-testid={`sys-fw-romm-${testKey}`}
        disabled={view.romm.disabled}
        title={view.romm.title || undefined}
        onclick={onRomm}
      >
        {view.romm.label}
      </button>
      <button
        class="small-btn"
        class:primary={view.locate.primary}
        data-testid={`sys-fw-locate-${testKey}`}
        disabled={view.locate.disabled}
        onclick={onLocate}
      >
        {view.locate.label}
      </button>
    </span>
  </div>
  {#if view.reason === 'needs-romm'}
    <p class="fw-why" data-testid={`sys-fw-why-${testKey}`}>
      {REASON_WORDS['needs-romm'].before}<button class="linkish" data-testid={`sys-fw-connect-${testKey}`} onclick={onConnectRomm}
        >{REASON_WORDS['needs-romm'].link}</button
      >{REASON_WORDS['needs-romm'].after}
    </p>
  {:else if view.reason !== null}
    <p class="fw-why" data-testid={`sys-fw-why-${testKey}`}>{REASON_WORDS[view.reason]}</p>
  {/if}
  {#if view.files.length > 0}
    <ul class="fw-files">
      {#each view.files as f (f.name)}
        {@render fileLine(f)}
      {/each}
      {#if showOptional}
        {#each view.moreOptional as f (f.name)}
          {@render fileLine(f)}
        {/each}
      {/if}
    </ul>
    {#if view.moreOptional.length > 0}
      <button class="linkish more" data-testid={`sys-fw-more-${testKey}`} onclick={() => (showOptional = !showOptional)}>
        {showOptional
          ? 'Show fewer files'
          : `${view.moreOptional.length} more optional ${view.moreOptional.length === 1 ? 'file' : 'files'} GRID knows`}
      </button>
    {/if}
  {/if}
</div>

<style>
  .fw {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding-top: 8px;
    border-top: 1px solid var(--line);
  }

  .fw-head {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px;
    font-size: 13px;
    color: var(--text);
  }

  .fw-head b {
    color: var(--text-h);
    font-weight: 600;
  }

  .acts {
    margin-left: auto;
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .fw-why {
    margin: 0;
    font-size: 12px;
    color: var(--text-muted);
  }

  .fw-files {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .fw-files li {
    display: grid;
    grid-template-columns: 18px minmax(110px, max-content) minmax(0, 1fr) auto;
    align-items: center;
    gap: 8px;
    font-size: 12px;
  }

  code {
    font: 12px ui-monospace, monospace;
    color: var(--text-h);
    overflow-wrap: anywhere;
  }

  .muted-s {
    font-size: 12px;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .fw-st {
    color: var(--text-muted);
    white-space: nowrap;
  }

  .fw-st.warn {
    color: var(--warning-text);
    font-weight: 600;
  }

  .st {
    flex: none;
    width: 16px;
    height: 16px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    font-size: 10px;
    font-weight: 700;
  }

  .st.ok {
    color: var(--on-primary);
    background: var(--primary);
  }

  .st.warn {
    color: #1a1a12;
    background: var(--warning);
  }

  .st.info {
    color: var(--text-h);
    border: 1.5px solid var(--text-muted);
    box-sizing: border-box;
  }

  .st.off {
    border: 1.5px solid var(--line);
    box-sizing: border-box;
  }

  .small-btn {
    font: inherit;
    font-size: 12px;
    padding: 4px 10px;
    border-radius: var(--r-chip);
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text-h);
    cursor: pointer;
    white-space: nowrap;
  }

  .small-btn.primary {
    border-color: var(--primary);
    background: var(--primary);
    color: var(--on-primary);
  }

  .small-btn.primary:hover:not(:disabled) {
    background: var(--primary-hover);
  }

  .small-btn:disabled {
    opacity: 0.6;
    cursor: default;
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

  .more {
    align-self: flex-start;
    font-size: 12px;
  }
</style>
