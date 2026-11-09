<script lang="ts">
  // The Backup dialog (GameBackups README, "The backup dialog"; prototype
  // `bkRenderDlg`). It plans the backup for every connected, set-up drive
  // (the chosen one first, so the others reuse the emulator sizes it
  // resolved), shows the chosen drive's plan, and queues it. Every word it
  // shows comes from `dialog.ts`. The queued card is not raised here: the
  // batch notifications announce it (notifications/notifier.ts).
  import { untrack } from 'svelte';
  import { api, type BackupPlan, type SourceChoice } from '../api';
  import Icon from '../Icon.svelte';
  import { isWindowsPlatform } from '../details/actions';
  import { drives } from '../stores/drives.svelte';
  import { installed } from '../stores/installed.svelte';
  import { session } from '../stores/session.svelte';
  import { hostOf } from '../shell';
  import type { BackupDialogOpen } from './dialogState.svelte';
  import {
    anyLocal,
    chooseAll,
    chooseDrive,
    dialogLede,
    driveRow,
    emulatorLine,
    firmwareLine,
    footerModel,
    localLine,
    noDrivesText,
    outdatedRows,
    planNotes,
    runtimeLine,
    sourcePicks,
    sourceRows,
    wineNote,
    type LocalGameInfo,
    type SourceChoices,
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
  /** "Copy installed games from this computer" (ticked by default). */
  let useLocal = $state(true);
  /** The picks for out-of-date local games, by rom id. */
  let choices = $state<Record<number, SourceChoice>>({});
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

  // The Source section. The platform and the Windows flag of a game copied
  // from this computer come from its registry row.
  let localInfo = $derived.by(() => {
    const map = new Map<number, LocalGameInfo>();
    for (const r of installed.list) {
      if (r.rom_id === null || map.has(r.rom_id)) continue;
      map.set(r.rom_id, { platform: r.platform, windows: isWindowsPlatform(r.platform_slug, r.platform) });
    }
    return map;
  });
  const lookup = (romId: number) => localInfo.get(romId);
  let host = $derived(hostOf(session.serverUrl));
  let showSource = $derived(ready.length > 0 && plan !== null && plan.local_available);
  let local = $derived(plan ? localLine(plan, host) : null);
  let sources = $derived(plan && plan.use_local ? sourceRows(plan, choices, lookup) : null);
  let outdated = $derived(plan ? outdatedRows(plan, choices) : null);
  let wine = $derived(plan ? wineNote(plan, lookup) : '');

  function pick(romId: number, source: SourceChoice) {
    choices = { ...choices, [romId]: source };
    startError = null;
  }

  function pickAll(source: SourceChoice) {
    if (plan === null) return;
    choices = chooseAll(plan, choices, source);
    startError = null;
  }

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
    const local = useLocal;
    const picks = sourcePicks($state.snapshot(choices) as SourceChoices);
    const keys = ready.map((d) => d.key);
    void ready.map((d) => `${d.free_bytes}:${d.pack?.updated ?? ''}:${d.pack?.game_count ?? 0}`).join('|');
    const request = $state.snapshot(open.request);
    const first = untrack(() => chosen);
    const mine = ++generation;
    planning = new Set(keys);
    const one = async (key: string) => {
      try {
        const p = await api.planBackup(key, request, fw, rt, local, picks);
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
      await api.startBackup(
        chosen,
        $state.snapshot(open.request),
        includeFirmware,
        includeRuntime,
        useLocal,
        sourcePicks($state.snapshot(choices) as SourceChoices),
      );
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
    <p class="note" data-testid="backup-lede">{dialogLede(host, anyLocal(plan))}</p>

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

    {#if showSource && local}
      <section class="sub" data-testid="backup-source">
        <h4>Source</h4>
        <label class="also">
          <input data-testid="backup-use-local" type="checkbox" bind:checked={useLocal} />
          <span>
            <b>{local.label}</b>
            <span class="note" data-testid="backup-local-line">{local.note}</span>
          </span>
        </label>
        {#if sources && sources.rows.length > 0}
          {#if wine !== ''}
            <p class="note" data-testid="backup-wine-note">{wine}</p>
          {/if}
          {#snippet sourceList()}
            <ul class="src-list" data-testid="backup-sources">
              {#each sources!.rows as r (r.romId)}
                <li data-testid="backup-source-row">
                  <span class="src-name">
                    <b>{r.name}</b>
                    {#if r.sub !== ''}<span class="note">{r.sub}</span>{/if}
                  </span>
                  <span class={`src-tag ${r.tag}`}>
                    {#if r.tag === 'local'}<Icon name="drive" size={12} />{:else if r.tag === 'download'}<Icon
                        name="download"
                        size={12}
                      />{/if}{r.tagText}
                  </span>
                </li>
              {/each}
            </ul>
          {/snippet}
          {#if sources.collapsed}
            <details class="src-each" data-testid="backup-sources-each">
              <summary>{sources.summary} <span class="note">{sources.count}</span></summary>
              {@render sourceList()}
            </details>
          {:else}
            {@render sourceList()}
          {/if}
        {/if}
        {#if outdated}
          <div class="outdated" role="group" aria-label="Out-of-date games" data-testid="backup-outdated">
            <p class="outdated-head">
              <b>{outdated.title}</b>
              <span class="note">{outdated.hint}</span>
            </p>
            {#if outdated.bulk}
              <div class="outdated-bulk">
                <button class="small-btn" type="button" data-testid="backup-choose-all-local" onclick={() => pickAll('local')}>
                  {outdated.bulk.local}
                </button>
                <button class="small-btn" type="button" data-testid="backup-choose-all-download" onclick={() => pickAll('download')}>
                  {outdated.bulk.download}
                </button>
              </div>
            {/if}
            {#each outdated.items as o (o.romId)}
              <fieldset class="choice" data-testid="backup-choice">
                <legend><b>{o.name}</b>{o.detail}</legend>
                <label class="pick">
                  <input
                    type="radio"
                    name={`backup-choice-${o.romId}`}
                    value="local"
                    checked={o.choice === 'local'}
                    onchange={() => pick(o.romId, 'local')}
                  />
                  {o.localLabel}
                </label>
                <label class="pick">
                  <input
                    type="radio"
                    name={`backup-choice-${o.romId}`}
                    value="download"
                    checked={o.choice === 'download'}
                    onchange={() => pick(o.romId, 'download')}
                  />
                  {o.downloadLabel}
                </label>
              </fieldset>
            {/each}
          </div>
        {/if}
      </section>
    {/if}

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

  /* Source: one row per game being written, and the out-of-date choice. */
  .src-list {
    list-style: none;
    margin: 0;
    padding: 0;
    border: 1px solid var(--line);
    border-radius: var(--r-row);
    background: var(--surface);
  }

  .src-list li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 12px;
    font-size: 13px;
  }

  .src-list li + li {
    border-top: 1px solid var(--line);
  }

  .src-name {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
    margin-right: auto;
  }

  .src-name b {
    color: var(--text-h);
    font-weight: 600;
    overflow-wrap: anywhere;
  }

  .src-tag {
    flex: none;
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 2px 8px;
    border-radius: var(--r-pill);
    font-size: 12px;
    white-space: nowrap;
    color: var(--text-muted);
    background: var(--field);
  }

  .src-tag.local {
    color: var(--text-h);
    background: var(--selected);
  }

  .src-tag.ask {
    color: var(--warning-text);
    background: color-mix(in srgb, var(--warning) 14%, transparent);
  }

  .src-each summary {
    cursor: pointer;
    padding: 2px 0;
    font-size: 13px;
    color: var(--text-h);
  }

  .src-each summary .note {
    margin-left: 4px;
  }

  .src-each[open] summary {
    margin-bottom: 6px;
  }

  .outdated {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 10px 12px;
    border-radius: var(--r-control);
    border-left: 3px solid var(--warning);
    background: color-mix(in srgb, var(--warning) 10%, transparent);
  }

  .outdated-head {
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
    font-size: 13px;
  }

  .outdated-head b,
  .choice legend b {
    color: var(--text-h);
    font-weight: 600;
  }

  .outdated-bulk {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .choice {
    margin: 0;
    padding: 0;
    border: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .choice legend {
    padding: 0;
    margin-bottom: 4px;
    font-size: 13px;
    color: var(--text);
  }

  .pick {
    display: flex;
    align-items: center;
    gap: 8px;
    padding-left: 2px;
    font-size: 13px;
    color: var(--text);
    cursor: pointer;
  }

  .pick input {
    margin: 0;
    accent-color: var(--primary);
  }

  /* The compact buttons beside a value (Systems' `.small-btn`). */
  .small-btn {
    font: inherit;
    font-size: 12px;
    padding: 4px 10px;
    border-radius: var(--r-chip);
    border: 1px solid var(--line-strong);
    background: transparent;
    color: var(--text-h);
    cursor: pointer;
    white-space: nowrap;
  }

  .small-btn:hover {
    background: var(--surface);
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
