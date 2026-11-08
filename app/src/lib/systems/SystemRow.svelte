<script lang="ts">
  // One Settings › Systems row: the system's name and game count (a toggle
  // that opens the row), its picker, and its status pill. The body is the
  // caller's. A system with no emulator is always open, with a warning edge.
  import type { Snippet } from 'svelte';
  import Icon from '../Icon.svelte';
  import { STATUS_LABELS, type PickerOption, type SystemStatus } from './rows';

  let {
    testKey,
    label,
    meta,
    open,
    attention = false,
    canToggle = true,
    pickerLabel,
    selected,
    placeholder = null,
    noneLabel = null,
    installed,
    download,
    pickerDisabled = false,
    status,
    statusText = null,
    onToggle,
    onPick,
    body,
  }: {
    testKey: string;
    label: string;
    meta: string;
    open: boolean;
    attention?: boolean;
    canToggle?: boolean;
    pickerLabel: string;
    selected: string;
    /** A disabled first option shown while nothing is picked (Windows). */
    placeholder?: string | null;
    /** The "None" option's text; `null` offers no None. */
    noneLabel?: string | null;
    installed: PickerOption[];
    download: PickerOption[];
    pickerDisabled?: boolean;
    status: SystemStatus;
    /** Overrides the status pill's text (e.g. "Installing firmware…"). */
    statusText?: string | null;
    onToggle: () => void;
    /** Called with the picked value and the select, which the caller puts
     *  back to the real value when the pick starts an install. */
    onPick: (value: string, select: HTMLSelectElement) => void;
    body?: Snippet;
  } = $props();

  const tone: Record<SystemStatus, string> = {
    ready: 'ok',
    update: 'info',
    firmware: 'warn',
    installing: '',
    none: 'warn',
  };
</script>

<li class="sysrow" class:attention class:open data-testid={`sys-row-${testKey}`}>
  <div class="sys-head">
    <button
      class="sys-toggle"
      data-testid={`sys-toggle-${testKey}`}
      aria-expanded={open}
      disabled={!canToggle}
      onclick={onToggle}
    >
      <span class="chev"><Icon name="chevronRight" size={14} /></span>
      <b>{label}</b>
      <span class="muted-s">{meta}</span>
    </button>
    <select
      class="sys-pick"
      data-testid={`sys-pick-${testKey}`}
      aria-label={pickerLabel}
      value={selected}
      disabled={pickerDisabled}
      onchange={(e) => onPick(e.currentTarget.value, e.currentTarget)}
    >
      {#if placeholder !== null}
        <option value="" disabled>{placeholder}</option>
      {/if}
      {#if noneLabel !== null}
        <option value="">{noneLabel}</option>
      {/if}
      {#if installed.length > 0}
        <optgroup label="Installed" data-testid={`sys-pick-installed-${testKey}`}>
          {#each installed as o (o.value)}
            <option value={o.value}>{o.label}</option>
          {/each}
        </optgroup>
      {/if}
      {#if download.length > 0}
        <optgroup label="Download" data-testid={`sys-pick-download-${testKey}`}>
          {#each download as o (o.value)}
            <option value={o.value}>{o.label}</option>
          {/each}
        </optgroup>
      {/if}
    </select>
    <span class={`pill-s ${tone[status]}`} data-testid={`sys-status-${testKey}`} data-status={status}>
      {#if status === 'ready'}<Icon name="check" size={12} />{/if}
      {#if tone[status] === 'warn'}!{/if}
      {statusText ?? STATUS_LABELS[status]}
    </span>
  </div>
  {#if open && body}
    <div class="sys-body">
      {@render body()}
    </div>
  {/if}
</li>

<style>
  .sysrow {
    border-radius: var(--r-row);
    border: 1px solid var(--line);
    background: var(--surface);
  }

  .sysrow.attention {
    border-color: var(--warning);
  }

  .sys-head {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(160px, 260px) 150px;
    align-items: center;
    gap: 12px;
    padding: 8px 12px 8px 6px;
  }

  .sys-toggle {
    display: flex;
    align-items: baseline;
    gap: 8px;
    min-width: 0;
    font: inherit;
    text-align: left;
    border: none;
    background: none;
    color: var(--text);
    padding: 4px 6px;
    border-radius: var(--r-control);
    cursor: pointer;
  }

  .sys-toggle:hover:not(:disabled) {
    background: var(--surface);
  }

  .sys-toggle:disabled {
    cursor: default;
  }

  .sys-toggle b {
    color: var(--text-h);
    font-size: 14px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .chev {
    display: inline-flex;
    align-self: center;
    color: var(--text-muted);
    transition: transform var(--m-fast) ease;
  }

  .sysrow.open .chev {
    transform: rotate(90deg);
  }

  .sys-toggle:disabled .chev {
    visibility: hidden;
  }

  .muted-s {
    font-size: 12px;
    color: var(--text-muted);
    white-space: nowrap;
  }

  .sys-pick {
    width: 100%;
    min-width: 0;
    font: inherit;
    font-size: 13px;
    padding: 6px 8px;
    border-radius: var(--r-control);
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text-h);
    text-overflow: ellipsis;
  }

  .sys-pick:disabled {
    opacity: 0.6;
  }

  .pill-s {
    justify-self: start;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 12px;
    font-weight: 600;
    padding: 2px 10px;
    border-radius: var(--r-pill);
    color: var(--text-muted);
    border: 1px solid var(--line);
    white-space: nowrap;
  }

  .pill-s.ok {
    color: var(--text-h);
  }

  .pill-s.ok :global(svg) {
    color: var(--primary);
  }

  .pill-s.warn {
    background: var(--warning);
    border-color: var(--warning);
    color: #1a1a12;
  }

  .pill-s.info {
    color: var(--text-h);
    border-color: var(--primary);
  }

  .sys-body {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px 14px 12px 34px;
    border-top: 1px solid var(--line);
  }

  @media (prefers-reduced-motion: reduce) {
    .chev {
      transition: none;
    }
  }
</style>
