<script lang="ts">
  // One drive in Settings › Backup drives (GameBackups README, "Settings ›
  // Backup drives"). A connected drive (`drive`) shows its pack, or the
  // setup form when it has none; a remembered one (`remembered`) shows when
  // it was last seen and Forget. Every text comes from the pure
  // `backups/drives.ts`; this file holds only the inline forms' state and
  // the command calls. After a command the drives store is pulled again, so
  // the card re-renders from the new snapshot.
  import { api, type DriveInfo, type PackOs, type RememberedDrive } from '../api';
  import Icon from '../Icon.svelte';
  import { expectOsChangeBatch } from '../notifications/notifier';
  import { refresh } from '../stores/drives.svelte';
  import { pushToast } from '../stores/toasts.svelte';
  import {
    addedOses,
    defaultPackName,
    driveCardModel,
    emulatorNames,
    FAT32_WARNING,
    osChanged,
    osChangeError,
    osChangeHelp,
    osChangeToasts,
    PACK_OSES,
    packFacts,
    rememberedCardModel,
    renameError,
    setupError,
    usageLabel,
    usageSegments,
    wipeConfirmText,
  } from './drives';

  let {
    drive = undefined,
    remembered = undefined,
    hostOs = null,
    now = new Date(),
    onShowInLibrary = undefined,
  }: {
    drive?: DriveInfo;
    remembered?: RememberedDrive;
    /** This computer's pack OS, ticked by default in the setup form. */
    hostOs?: PackOs | null;
    /** For "Last seen 3 days ago". */
    now?: Date;
    /** Opens Library on this drive. Absent until Library › Backup drives exists (plan Task 20): the button is then left out. */
    onShowInLibrary?: (key: string) => void;
  } = $props();

  const uid = $props.id();

  type Mode = 'idle' | 'setup' | 'rename' | 'os' | 'confirm';
  let mode = $state<Mode>('idle');
  let busy = $state(false);
  let error = $state<string | null>(null);

  let setupName = $state('');
  let setupOs = $state<PackOs[]>([]);
  let renameValue = $state('');
  let osNext = $state<PackOs[]>([]);
  let wipeBytes = $state(0);

  let model = $derived(drive ? driveCardModel(drive) : null);
  let away = $derived(remembered ? rememberedCardModel(remembered, now) : null);
  let segments = $derived(
    drive?.pack ? usageSegments(drive.total_bytes, drive.free_bytes, drive.pack.gridpack_bytes) : [],
  );
  let facts = $derived(drive ? packFacts(drive) : []);
  let emulatorCount = $derived(drive?.pack ? emulatorNames(drive.pack.emulators).length : 0);
  let wipeText = $derived(
    drive ? wipeConfirmText(drive.label, drive.pack ? drive.pack.game_count : null, wipeBytes) : null,
  );
  let setupProblem = $derived(setupError(setupName, setupOs));
  let osProblem = $derived(osChangeError(osNext));

  function errorText(err: unknown): string {
    return err instanceof Error ? err.message : String(err);
  }

  function open(next: Mode) {
    error = null;
    mode = next;
  }

  function close() {
    error = null;
    mode = 'idle';
  }

  /** Runs one command with the card busy; a refusal stays on the card as its error line. */
  async function run(work: () => Promise<void>) {
    busy = true;
    error = null;
    try {
      await work();
    } catch (err) {
      error = errorText(err);
    } finally {
      busy = false;
    }
  }

  function toggle(list: PackOs[], os: PackOs, on: boolean): PackOs[] {
    const rest = list.filter((o) => o !== os);
    return on ? [...rest, os] : rest;
  }

  function startSetup() {
    if (!drive) return;
    setupName = defaultPackName(drive);
    setupOs = hostOs ? [hostOs] : [];
    open('setup');
  }

  function submitSetup() {
    const d = drive;
    if (!d || setupProblem !== null) return;
    const name = setupName.trim();
    void run(async () => {
      await api.setupDrive(d.mount, name, setupOs);
      mode = 'idle';
      pushToast(`${name} is ready for backups`);
      await refresh();
    });
  }

  function startRename() {
    renameValue = drive?.pack?.name ?? '';
    open('rename');
  }

  function submitRename() {
    const d = drive;
    if (!d || renameError(renameValue) !== null) return;
    const name = renameValue.trim();
    if (name === d.pack?.name) {
      close();
      return;
    }
    void run(async () => {
      await api.renameDrive(d.key, name);
      mode = 'idle';
      await refresh();
    });
  }

  function startOs() {
    osNext = [...(drive?.pack?.os ?? [])];
    open('os');
  }

  function submitOs() {
    const d = drive;
    const pack = d?.pack;
    if (!d || !pack || osProblem !== null) return;
    if (!osChanged(pack.os, osNext)) {
      close();
      return;
    }
    const before = [...pack.os];
    const after = [...osNext];
    void run(async () => {
      // Adding a computer needs a server; the backend's refusal ("Adding a
      // computer needs a server") stays on the form as its error line.
      const outcome = await api.setDriveOs(d.key, after);
      // The queued builds are announced by the backup batch's own cards.
      expectOsChangeBatch(outcome, addedOses(before, after));
      mode = 'idle';
      for (const line of osChangeToasts(pack.name, before, after, outcome)) pushToast(line);
      await refresh();
    });
  }

  function startWipe() {
    const d = drive;
    if (!d) return;
    void run(async () => {
      // The size is measured on the drive; when that fails the pack's own
      // count is the best figure there is.
      wipeBytes = await api.removeGridFilesSize(d.key).catch(() => d.pack?.gridpack_bytes ?? 0);
      mode = 'confirm';
    });
  }

  function submitWipe() {
    const d = drive;
    if (!d) return;
    void run(async () => {
      await api.removeGridFiles(d.key);
      mode = 'idle';
      pushToast(`GRID files removed from ${d.label}`);
      await refresh();
    });
  }

  function openFolder() {
    if (!drive) return;
    api.openDriveFolder(drive.key).catch((err) => pushToast(errorText(err), 'error'));
  }

  function forget() {
    const r = remembered;
    if (!r) return;
    void run(async () => {
      await api.forgetDrive(r.mount);
      await refresh();
    });
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape' && mode !== 'idle' && !busy) {
      e.stopPropagation();
      close();
    }
  }
</script>

{#if drive && model}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <article
    class="drive"
    class:fresh={model.canSetUp}
    data-testid="backup-drive"
    data-drive-key={drive.key}
    aria-labelledby={`${uid}-title`}
    onkeydown={onKey}
  >
    <header class="head">
      <span class="drive-icon"><Icon name="drive" size={20} /></span>
      <div class="id">
        <b id={`${uid}-title`}>{model.title}</b>
        <span class="sub">{model.subtitle}</span>
      </div>
      <span class="pill" class:ok={model.tone === 'ok'} class:warn={model.tone === 'warn'} data-testid="backup-drive-status">
        {#if model.tone === 'ok'}<Icon name="check" size={12} />{/if}{model.status}
      </span>
    </header>

    {#if mode === 'rename'}
      <form
        class="rename"
        onsubmit={(e) => {
          e.preventDefault();
          submitRename();
        }}
      >
        <!-- svelte-ignore a11y_autofocus -->
        <input
          class="g-field"
          data-testid="backup-drive-rename-input"
          aria-label="Pack name"
          bind:value={renameValue}
          disabled={busy}
          autofocus
        />
        <button class="small-btn primary" type="submit" disabled={busy || renameError(renameValue) !== null}>Save</button>
        <button class="small-btn" type="button" disabled={busy} onclick={close}>Cancel</button>
      </form>
      {#if error}<p class="g-error" role="alert">{error}</p>{/if}
    {/if}

    {#if model.showsPack && drive.pack}
      <div class="usage" role="img" aria-label={usageLabel(drive.total_bytes, drive.free_bytes)}>
        {#each segments as s (s.kind)}
          {#if s.kind !== 'free'}<b class={`u-${s.kind}`} style:width={`${s.pct.toFixed(2)}%`}></b>{/if}
        {/each}
      </div>
      <p class="legend">
        {#each segments as s (s.kind)}<span><i class={`u-${s.kind}`}></i>{s.legend}</span>{/each}
      </p>

      {#if mode === 'os'}
        <form
          class="os-edit"
          onsubmit={(e) => {
            e.preventDefault();
            submitOs();
          }}
        >
          <fieldset class="os">
            <legend>Computers this pack is for</legend>
            {#each PACK_OSES as o (o.os)}
              <label class="g-check">
                <input
                  type="checkbox"
                  data-testid={`backup-drive-os-${o.os}`}
                  checked={osNext.includes(o.os)}
                  disabled={busy}
                  onchange={(e) => (osNext = toggle(osNext, o.os, e.currentTarget.checked))}
                />
                {o.label}
              </label>
            {/each}
            <span class="g-help">{osChangeHelp(emulatorCount)}</span>
          </fieldset>
          {#if osProblem}<p class="g-error">{osProblem}</p>{/if}
          {#if error}<p class="g-error" role="alert">{error}</p>{/if}
          <div class="g-actions">
            <button class="small-btn primary" type="submit" disabled={busy || osProblem !== null}>Save</button>
            <button class="small-btn" type="button" disabled={busy} onclick={close}>Cancel</button>
          </div>
        </form>
      {:else}
        <p class="for">
          <span class="muted">For</span> <b data-testid="backup-drive-for">{model.forLine}</b>
          {#if model.canEdit}
            <button class="small-btn" data-testid="backup-drive-os-change" disabled={busy || mode !== 'idle'} onclick={startOs}>
              Change…
            </button>
          {/if}
        </p>
      {/if}

      {#if model.note}<p class="g-note">{model.note}</p>{/if}

      <dl class="g-kv facts">
        {#each facts as f (f.label)}
          <div>
            <dt>{f.label}</dt>
            <dd>{#if f.mono}<code>{f.value}</code>{:else}{f.value}{/if}</dd>
          </div>
        {/each}
      </dl>
    {:else}
      {#if model.note && mode !== 'setup'}<p class="g-note">{model.note}</p>{/if}

      {#if mode === 'setup'}
        <form
          class="g-form setup"
          onsubmit={(e) => {
            e.preventDefault();
            submitSetup();
          }}
        >
          <label>
            Pack name
            <input class="g-field" data-testid="backup-drive-setup-name" bind:value={setupName} disabled={busy} />
            <span class="g-help">Shown in Library and on other computers. You can change it later.</span>
          </label>
          <fieldset class="os">
            <legend>Computers this pack is for</legend>
            {#each PACK_OSES as o (o.os)}
              <label class="g-check">
                <input
                  type="checkbox"
                  data-testid={`backup-drive-setup-os-${o.os}`}
                  checked={setupOs.includes(o.os)}
                  disabled={busy}
                  onchange={(e) => (setupOs = toggle(setupOs, o.os, e.currentTarget.checked))}
                />
                {o.label}{o.os === hostOs ? ' (this computer)' : ''}
              </label>
            {/each}
            <span class="g-help">
              GRID adds the emulators each system needs, built for every computer you pick. You can change this later.
            </span>
          </fieldset>
          <p class="g-note">
            GRID makes a <code>gridpack</code> folder at the root of {drive.label}. Nothing else on the drive is changed.
          </p>
          {#if drive.fat32}
            <p class="warn-box" data-testid="backup-drive-fat32"><b>{FAT32_WARNING.title}</b> {FAT32_WARNING.body}</p>
          {/if}
          {#if setupProblem}<p class="g-error">{setupProblem}</p>{/if}
          {#if error}<p class="g-error" role="alert">{error}</p>{/if}
          <div class="g-actions">
            <button class="g-btn" type="submit" data-testid="backup-drive-setup-go" disabled={busy || setupProblem !== null}>
              Set up drive
            </button>
            <button class="g-btn secondary" type="button" disabled={busy} onclick={close}>Cancel</button>
          </div>
        </form>
      {:else if model.canSetUp}
        <div class="g-actions">
          <button class="g-btn" data-testid="backup-drive-setup" onclick={startSetup}>Set up for GRID…</button>
        </div>
      {/if}
    {/if}

    {#if mode === 'confirm' && wipeText}
      <div class="confirm" role="group" aria-labelledby={`${uid}-wipe`}>
        <p><b id={`${uid}-wipe`}>{wipeText.title}</b> {wipeText.body}</p>
        {#if error}<p class="g-error" role="alert">{error}</p>{/if}
        <div class="g-actions">
          <button class="g-btn danger" data-testid="backup-drive-wipe-go" disabled={busy} onclick={submitWipe}>
            Delete GRID files
          </button>
          <button class="g-btn secondary" disabled={busy} onclick={close}>Cancel</button>
        </div>
      </div>
    {:else if mode !== 'setup' && drive.status !== 'not_set_up'}
      {#if error && mode === 'idle'}<p class="g-error" role="alert">{error}</p>{/if}
      <div class="g-actions">
        {#if model.showsPack && onShowInLibrary}
          <button class="g-btn" data-testid="backup-drive-show" onclick={() => onShowInLibrary?.(drive.key)}>
            Show in Library
          </button>
        {/if}
        <button class="g-btn secondary" data-testid="backup-drive-open" onclick={openFolder}>Open folder</button>
        {#if model.canEdit}
          <button
            class="g-btn secondary"
            data-testid="backup-drive-rename"
            disabled={busy || mode !== 'idle'}
            onclick={startRename}>Rename…</button
          >
        {/if}
        {#if model.canRemove}
          <button
            class="g-btn secondary destructive"
            data-testid="backup-drive-wipe"
            disabled={busy || mode !== 'idle'}
            onclick={startWipe}>Remove GRID files…</button
          >
        {/if}
      </div>
    {/if}
  </article>
{:else if remembered && away}
  <article class="drive away" data-testid="backup-drive-remembered" data-drive-key={remembered.mount} aria-labelledby={`${uid}-title`}>
    <header class="head">
      <span class="drive-icon"><Icon name="drive" size={20} /></span>
      <div class="id">
        <b id={`${uid}-title`}>{away.title}</b>
        <span class="sub">{away.subtitle}</span>
      </div>
      <span class="pill">Not connected</span>
    </header>
    <p class="g-note">{away.note}</p>
    {#if error}<p class="g-error" role="alert">{error}</p>{/if}
    <div class="g-actions">
      <button class="small-btn" data-testid="backup-drive-forget" disabled={busy} onclick={forget}>Forget</button>
    </div>
  </article>
{/if}

<style>
  /* A drive card (prototype `.bk-drive`): the surface fill and a hairline,
     a drive not set up yet dashed so it reads as unclaimed. */
  .drive {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px 16px;
    border-radius: var(--r-row);
    border: 1px solid var(--line);
    background: var(--surface);
  }

  .drive.fresh {
    border-style: dashed;
    border-color: var(--line-strong);
  }

  .drive.away {
    opacity: 0.72;
  }

  .head {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .drive-icon {
    display: inline-grid;
    place-items: center;
    width: 34px;
    height: 34px;
    flex: none;
    border-radius: var(--r-control);
    background: var(--field);
    color: var(--text-h);
  }

  .id {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
    margin-right: auto;
  }

  .id b {
    color: var(--text-h);
    font-size: 14px;
    overflow-wrap: anywhere;
  }

  .sub {
    font-size: 12px;
    color: var(--text-muted);
  }

  /* The status (prototype `.pill-s`): outlined and muted; Ready for backups
     in heading colour with a primary tick, a problem on the warning fill. */
  .pill {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    flex: none;
    padding: 2px 10px;
    border-radius: var(--r-pill);
    border: 1px solid var(--line);
    font-size: 12px;
    font-weight: 600;
    color: var(--text-muted);
    white-space: nowrap;
  }

  .pill.ok {
    color: var(--text-h);
  }

  .pill.ok :global(.icon) {
    color: var(--primary);
  }

  .pill.warn {
    background: var(--warning);
    border-color: var(--warning);
    color: #1a1a12;
  }

  /* Other files, GRID, free: the free part is the track itself. */
  .usage {
    display: flex;
    height: 8px;
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

  .legend {
    display: flex;
    flex-wrap: wrap;
    gap: 14px;
    margin: -2px 0 0;
    font-size: 12px;
    color: var(--text-muted);
  }

  .legend i {
    display: inline-block;
    width: 8px;
    height: 8px;
    margin-right: 5px;
    border-radius: 2px;
    border: 1px solid transparent;
    vertical-align: -1px;
  }

  .legend i.u-free {
    background: var(--field);
    border-color: var(--line-strong);
  }

  .for {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0;
    font-size: 13px;
  }

  .for b {
    color: var(--text-h);
  }

  .muted {
    color: var(--text-muted);
  }

  .facts > div {
    grid-template-columns: 120px minmax(0, 1fr);
    padding: 6px 0;
  }

  .facts > div:last-child {
    border-bottom: none;
  }

  code {
    font: 12px ui-monospace, 'Cascadia Mono', Consolas, 'SFMono-Regular', Menlo, monospace;
    user-select: text;
  }

  .os {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 0;
    padding: 0;
    border: 0;
  }

  .os legend {
    margin-bottom: 4px;
    padding: 0;
    font-size: 12px;
    color: var(--text-muted);
  }

  .os-edit {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px 12px;
    border-radius: var(--r-control);
    border: 1px solid var(--line);
  }

  .rename {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .rename .g-field {
    width: 280px;
  }

  /* The FAT32 note: a warning edge on a faint warning wash, plain text. */
  .warn-box {
    margin: 0;
    padding: 8px 10px;
    border-left: 3px solid var(--warning);
    border-radius: var(--r-control);
    background: color-mix(in srgb, var(--warning) 12%, transparent);
    color: var(--text);
    font-size: 13px;
  }

  .confirm {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 10px 12px;
    border-radius: var(--r-control);
    border: 1px solid var(--danger);
  }

  .confirm p {
    margin: 0;
    font-size: 13px;
    color: var(--text);
  }

  .confirm b {
    color: var(--text-h);
  }

  /* Destructive, as in Details: the secondary shape with a danger label,
     and the confirming button on the danger fill. */
  .g-btn.secondary.destructive {
    color: var(--danger);
  }

  .g-btn.danger {
    background: var(--danger);
    border-color: var(--danger);
    color: var(--on-danger);
  }

  .g-btn.danger:hover:not(:disabled) {
    background: var(--danger);
    border-color: var(--danger);
    filter: brightness(1.08);
  }

  /* The compact buttons beside a value (Systems' `.small-btn`). */
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

  .small-btn:hover:not(:disabled) {
    background: var(--surface);
  }

  .small-btn.primary:hover:not(:disabled) {
    background: var(--primary-hover);
  }

  .small-btn:disabled {
    opacity: 0.6;
    cursor: default;
  }
</style>
