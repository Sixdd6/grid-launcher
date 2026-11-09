<script lang="ts">
  // The Backup dialog (GameBackups README, "The backup dialog"; prototype
  // `bkRenderDlg`). It plans the backup for every connected, set-up drive
  // (the chosen one first, so the others reuse the emulator sizes it
  // resolved), shows the chosen drive's plan, and queues it. Every word it
  // shows comes from `dialog.ts`. The queued card is not raised here: the
  // batch notifications announce it (notifications/notifier.ts).
  import { untrack } from 'svelte';
  import { api, type BackupPlan } from '../api';
  import Icon from '../Icon.svelte';
  import { drives } from '../stores/drives.svelte';
  import { session } from '../stores/session.svelte';
  import { hostOf } from '../shell';
  import type { BackupDialogOpen } from './dialogState.svelte';
  import {
    chooseDrive,
    dialogLede,
    driveRow,
    emulatorLine,
    firmwareLine,
    footerModel,
    noDrivesText,
    planNotes,
    runtimeLine,
  } from './dialog';

  let {
    open,
    onClose,
    onSetUp,
  }: {
    open: BackupDialogOpen;
    onClose: () => void;
    /** "Set up a drive…": opens Settings › Backup drives. */
    onSetUp: () => void;
  } = $props();

  let includeFirmware = $state(true);
  let includeRuntime = $state(false);
  let settingsLoaded = $state(false);
  let chosen = $state<string | null>(null);
  let plans = $state.raw<Record<string, BackupPlan>>({});
  let planErrors = $state.raw<Record<string, string>>({});
  /** Drive keys whose plan is being (re)made. */
  let planning = $state.raw<ReadonlySet<string>>(new Set());
  let starting = $state(false);
  let startError = $state<string | null>(null);
  let panelEl = $state<HTMLElement | null>(null);

  let ready = $derived(drives.ready);
  let chosenDrive = $derived(ready.find((d) => d.key === chosen) ?? null);
  let plan = $derived(chosen === null ? null : (plans[chosen] ?? null));
  let rows = $derived(ready.map((d) => driveRow(d, plans[d.key] ?? null, d.key === chosen)));
  let emus = $derived(plan ? emulatorLine(plan) : null);
  let firmware = $derived(plan ? firmwareLine(plan) : null);
  let runtime = $derived(plan ? runtimeLine(plan) : null);
  let notes = $derived(plan && chosenDrive ? planNotes(plan, chosenDrive) : []);
  let busy = $derived(chosen !== null && planning.has(chosen));
  let footer = $derived(footerModel(busy ? null : plan));
  let error = $derived(startError ?? (chosen !== null ? (planErrors[chosen] ?? null) : null));
  let empty = $derived(noDrivesText(drives.connected));

  // The firmware default comes from Settings › Backup drives.
  $effect(() => {
    api
      .getBackupSettings()
      .then((s) => (includeFirmware = s.include_firmware))
      .catch(() => {
        // Unreadable: the README's default (on) stands.
      })
      .finally(() => (settingsLoaded = true));
  });

  // Keep a drive chosen while drives come and go.
  $effect(() => {
    const next = chooseDrive(ready, untrack(() => chosen));
    if (next !== untrack(() => chosen)) chosen = next;
  });

  let generation = 0;

  // Plans every ready drive again when a checkbox changes or a drive
  // appears, goes, or changes (its free space, its pack).
  $effect(() => {
    if (!settingsLoaded) return;
    const fw = includeFirmware;
    const rt = includeRuntime;
    const keys = ready.map((d) => d.key);
    void ready.map((d) => `${d.free_bytes}:${d.pack?.updated ?? ''}:${d.pack?.game_count ?? 0}`).join('|');
    const request = $state.snapshot(open.request);
    const first = untrack(() => chosen);
    const mine = ++generation;
    planning = new Set(keys);
    const one = async (key: string) => {
      try {
        const p = await api.planBackup(key, request, fw, rt);
        if (mine !== generation) return;
        plans = { ...plans, [key]: p };
        if (key in planErrors) {
          const rest = { ...planErrors };
          delete rest[key];
          planErrors = rest;
        }
      } catch (err) {
        if (mine !== generation) return;
        planErrors = { ...planErrors, [key]: err instanceof Error ? err.message : String(err) };
      } finally {
        if (mine === generation) {
          const next = new Set(planning);
          next.delete(key);
          planning = next;
        }
      }
    };
    void (async () => {
      const head = first !== null && keys.includes(first) ? first : keys[0];
      if (head === undefined) return;
      await one(head);
      if (mine !== generation) return;
      await Promise.all(keys.filter((k) => k !== head).map(one));
    })();
  });

  async function start() {
    if (chosen === null || footer.disabled || starting) return;
    starting = true;
    startError = null;
    try {
      await api.startBackup(chosen, $state.snapshot(open.request), includeFirmware, includeRuntime);
      open.onQueued?.();
      onClose();
    } catch (err) {
      startError = err instanceof Error ? err.message : String(err);
    } finally {
      starting = false;
    }
  }

  function choose(key: string) {
    chosen = key;
    startError = null;
  }

  // Focus the dialog on open and hand focus back to the opener on close.
  $effect(() => {
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    untrack(() => panelEl?.focus());
    return () => {
      if (opener?.isConnected) opener.focus();
    };
  });

  function trapTab(e: KeyboardEvent) {
    if (!panelEl) return;
    const focusable = Array.from(
      panelEl.querySelectorAll<HTMLElement>('button:not([disabled]), input:not([disabled])'),
    ).filter((el) => !(el instanceof HTMLInputElement && el.type === 'radio' && !el.checked));
    if (focusable.length === 0) return;
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    const active = document.activeElement;
    if (e.shiftKey && (active === first || active === panelEl)) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && active === last) {
      e.preventDefault();
      first.focus();
    }
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      onClose();
    } else if (e.key === 'Tab') {
      trapTab(e);
    }
  }

  function onBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) onClose();
  }
</script>

<div class="backdrop" onclick={onBackdropClick} role="presentation">
  <div
    data-testid="backup-dialog"
    class="panel"
    bind:this={panelEl}
    role="dialog"
    aria-modal="true"
    aria-labelledby="backup-dialog-title"
    tabindex="-1"
    onkeydown={onKey}
  >
    <button data-testid="backup-dialog-close" class="close icon-btn" onclick={onClose} aria-label="Close">
      <Icon name="close" size={20} />
    </button>

    <h3 id="backup-dialog-title" data-testid="backup-dialog-title">{open.request.title}</h3>
    <p class="note">{dialogLede(hostOf(session.serverUrl))}</p>

    <section class="sub">
      <h4 id="backup-drive-heading">Drive</h4>
      {#if ready.length === 0}
        <div class="empty" data-testid="backup-no-drives">
          <b>{empty.title}</b>
          <span class="note">{empty.body}</span>
          <div>
            <button data-testid="backup-set-up" class="g-btn secondary" onclick={onSetUp}>Set up a drive…</button>
          </div>
        </div>
      {:else}
        <div class="drives" role="radiogroup" aria-labelledby="backup-drive-heading">
          {#each rows as row (row.key)}
            <label class="drive" class:on={row.key === chosen} data-testid="backup-drive">
              <input
                type="radio"
                name="backup-drive"
                value={row.key}
                checked={row.key === chosen}
                onchange={() => choose(row.key)}
              />
              <span class="drive-main">
                <b>{row.name}</b>
                <span class="note">{row.sub}</span>
              </span>
              <span class="free" class:warn={row.over} data-testid="backup-drive-free">{row.freeText}</span>
              <span class="usage" role="img" aria-label={row.usageLabel}>
                {#each row.segments as s (s.kind)}
                  <b class={`u-${s.kind}`} class:over={s.over} style:width={`${s.pct.toFixed(2)}%`}></b>
                {/each}
              </span>
            </label>
          {/each}
        </div>
      {/if}
    </section>

    {#if ready.length > 0}
      <section class="sub">
        <h4>Also copy</h4>
        <div class="also">
          <span class="fixed-mark" aria-hidden="true"><Icon name="check" size={13} /></span>
          <span>
            <b>Emulators</b>
            <span class="note" data-testid="backup-emulators">{emus ? emus.text : 'Checking…'}</span>
            {#each emus?.notes ?? [] as n (n)}<span class="note">{n}</span>{/each}
          </span>
        </div>
        <label class="also">
          <input
            data-testid="backup-firmware"
            type="checkbox"
            bind:checked={includeFirmware}
            disabled={firmware === null || firmware.disabled}
          />
          <span>
            <b>Firmware</b>
            <span class="note" data-testid="backup-firmware-text">{firmware ? firmware.text : 'Checking…'}</span>
          </span>
        </label>
        {#if runtime}
          <label class="also">
            <input data-testid="backup-runtime" type="checkbox" bind:checked={includeRuntime} disabled={runtime.disabled} />
            <span>
              <b>{runtime.label}</b>
              <span class="note">{runtime.text}</span>
            </span>
          </label>
        {/if}
        <p class="note">Covers, fanart, screenshots, logos and game details always go with each game, and so do the emulators.</p>
      </section>
    {/if}

    {#if notes.length > 0}
      <div class="notes" data-testid="backup-notes">
        {#each notes as n (n.text)}<p class="note" class:warn={n.warn}>{n.text}.</p>{/each}
      </div>
    {/if}

    {#if error}
      <p data-testid="backup-error" class="error" role="alert">{error}</p>
    {/if}

    <div class="foot">
      <span class="note" data-testid="backup-footer-note">{ready.length > 0 ? footer.note : ''}</span>
      <button data-testid="backup-cancel" class="g-btn secondary" onclick={onClose}>Cancel</button>
      {#if ready.length > 0}
        <button
          data-testid="backup-start"
          class="g-btn"
          disabled={footer.disabled || starting || (error !== null && startError === null)}
          onclick={start}
        >
          {starting ? 'Starting…' : footer.button}
        </button>
      {/if}
    </div>
  </div>
</div>

<style>
  /* Over Details (z 20), like its other dialogs. */
  .backdrop {
    position: fixed;
    inset: 0;
    background: var(--scrim);
    display: grid;
    place-items: center;
    z-index: 25;
  }

  .panel {
    position: relative;
    width: min(520px, calc(100vw - 48px));
    max-height: calc(100vh - 48px);
    overflow-y: auto;
    box-sizing: border-box;
    padding: 22px 24px;
    border-radius: var(--r-dialog);
    background: var(--overlay);
    border: 1px solid var(--line);
    box-shadow: var(--shadow-overlay);
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .panel:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: 2px;
  }

  .close {
    position: absolute;
    top: 14px;
    right: 14px;
    color: var(--text-muted);
  }

  h3 {
    margin: 0;
    padding-right: 36px;
    font-size: 17px;
    font-weight: 600;
    color: var(--text-h);
    overflow-wrap: anywhere;
  }

  h4 {
    margin: 0;
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .note {
    margin: 0;
    font-size: 12px;
    line-height: 1.45;
    color: var(--text-muted);
  }

  .sub {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .empty {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 12px 14px;
    border-radius: var(--r-row);
    border: 1px dashed var(--line-strong);
    font-size: 13px;
    color: var(--text-h);
  }

  .empty div {
    margin-top: 4px;
  }

  .drives {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .drive {
    display: grid;
    grid-template-columns: auto 1fr auto;
    grid-template-areas:
      'r m f'
      'u u u';
    align-items: center;
    gap: 6px 10px;
    padding: 10px 12px;
    border-radius: var(--r-row);
    border: 1px solid var(--line);
    background: var(--surface);
    cursor: pointer;
    transition: border-color var(--m-fast) ease, background var(--m-fast) ease;
  }

  .drive input {
    grid-area: r;
    margin: 0;
    accent-color: var(--primary);
  }

  .drive.on {
    border-color: var(--primary);
    background: var(--selected);
  }

  .drive-main {
    grid-area: m;
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
    font-size: 13px;
  }

  .drive-main b {
    color: var(--text-h);
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .free {
    grid-area: f;
    font-size: 12px;
    color: var(--text-muted);
    white-space: nowrap;
  }

  .free.warn {
    color: var(--danger);
    font-weight: 600;
  }

  /* The same bar as the drive cards (DriveCard.svelte), with this backup's
     share hatched in the disk colour (danger when it does not fit). */
  .usage {
    grid-area: u;
    display: flex;
    height: 6px;
    border-radius: var(--r-pill);
    background: var(--field);
    overflow: hidden;
  }

  .usage b {
    display: block;
    height: 100%;
  }

  .u-other {
    background: var(--line-strong);
  }

  .u-grid {
    background: var(--primary);
  }

  .u-adding {
    background: repeating-linear-gradient(
      135deg,
      var(--graph-disk) 0 4px,
      color-mix(in srgb, var(--graph-disk) 55%, transparent) 4px 8px
    );
  }

  .u-adding.over {
    background: var(--danger);
  }

  .also {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    font-size: 13px;
    color: var(--text);
  }

  label.also {
    cursor: pointer;
  }

  .also input {
    margin: 2px 0 0;
    accent-color: var(--primary);
  }

  .also input:disabled + span {
    opacity: 0.6;
  }

  .also > span:last-child {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }

  .also b {
    color: var(--text-h);
    font-weight: 600;
  }

  /* Emulators always go: a ticked box that is not a control. */
  .fixed-mark {
    flex: none;
    display: inline-grid;
    place-items: center;
    width: 15px;
    height: 15px;
    margin-top: 2px;
    border-radius: 3px;
    background: var(--primary);
    color: var(--on-primary);
  }

  .notes {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 8px 10px;
    border-radius: var(--r-control);
    background: var(--field);
  }

  .notes .note {
    color: var(--text);
  }

  .notes .note.warn {
    color: var(--warning-text);
  }

  .error {
    margin: 0;
    font-size: 13px;
    color: var(--danger);
  }

  .foot {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 8px;
  }

  .foot > .note {
    margin-right: auto;
  }
</style>
