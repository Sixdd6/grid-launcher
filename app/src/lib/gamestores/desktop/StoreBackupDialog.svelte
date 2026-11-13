<script lang="ts">
  // The backup dialog for GOG games (GameStores README, "GOG games on backup
  // drives › The dialog"; prototype `stBackupHTML`): Game backups' dialog
  // with Drive radios and "What goes on the drive", a row per game tagged
  // Installer files or Installed copy. Words: `gamestores/backup.ts`. The
  // queued and done cards are the backup batch's (notifications).
  import { api, type DriveInfo, type StoreBackupPlan } from '../../api';
  import Icon from '../../Icon.svelte';
  import { drives } from '../../stores/drives.svelte';
  import {
    driveFreeLine,
    STORE_BACKUP_COVERS_NOTE,
    STORE_BACKUP_LEDE,
    storeBackupView,
    WINDOWS_PREFIX_NOTE,
  } from '../backup';
  import { refusedNotice } from '../notices';
  import StoreDialog from './StoreDialog.svelte';
  import { showCard } from './ui.svelte';

  let {
    keys,
    title,
    onClose,
    onQueued = () => {},
    onSetUp,
  }: { keys: string[]; title: string; onClose: () => void; onQueued?: () => void; onSetUp: () => void } = $props();

  let ready = $derived(drives.ready);
  let chosen = $state<string | null>(null);
  let plans = $state<Record<string, StoreBackupPlan>>({});
  let error = $state<string | null>(null);
  let starting = $state(false);

  $effect(() => {
    if (chosen === null || !ready.some((d) => d.key === chosen)) chosen = ready[0]?.key ?? null;
  });

  // Plans every ready drive (free lines on each radio), again as drives change.
  $effect(() => {
    const list = ready;
    const wanted = keys;
    let cancelled = false;
    for (const d of list) {
      api
        .planStoreBackup(d.key, wanted)
        .then((p) => {
          if (!cancelled) plans = { ...plans, [d.key]: p };
        })
        .catch((err) => {
          if (!cancelled) error = err instanceof Error ? err.message : String(err);
        });
    }
    return () => {
      cancelled = true;
    };
  });

  let drive = $derived<DriveInfo | null>(ready.find((d) => d.key === chosen) ?? null);
  let plan = $derived(chosen !== null ? (plans[chosen] ?? null) : null);
  let view = $derived(plan && drive ? storeBackupView(plan, drive.pack?.name ?? drive.label) : null);

  function usedPct(d: DriveInfo): number {
    return d.total_bytes > 0 ? ((d.total_bytes - d.free_bytes) / d.total_bytes) * 100 : 0;
  }
  function addPct(d: DriveInfo, p: StoreBackupPlan | undefined): number {
    if (!p || d.total_bytes <= 0) return 0;
    return Math.min(100 - usedPct(d), (p.bytes_to_write / d.total_bytes) * 100);
  }

  async function start() {
    if (!view?.canStart || chosen === null) return;
    starting = true;
    try {
      await api.startStoreBackup(chosen, view.rows.map((r) => r.key));
      onQueued();
      onClose();
    } catch (err) {
      showCard(refusedNotice('Couldn’t start the backup', err instanceof Error ? err.message : String(err)));
    } finally {
      starting = false;
    }
  }
</script>

<StoreDialog testId="store-backup-dialog" label={title} width={520} {onClose}>
  <h3 data-testid="store-backup-title">{title}</h3>
  <p class="note">{STORE_BACKUP_LEDE}</p>

  <section class="sub">
    <h4>Drive</h4>
    {#if ready.length === 0}
      <div class="empty" data-testid="store-backup-no-drives">
        <b>No backup drive connected</b>
        <span class="note">Plug in a drive you set up for GRID. Set up a drive once in Settings › Backup drives.</span>
        <div><button class="g-btn secondary" onclick={onSetUp}>Set up a drive…</button></div>
      </div>
    {:else}
      <div class="drives" role="radiogroup" aria-label="Drive">
        {#each ready as d (d.key)}
          {@const p = plans[d.key]}
          <label class="drive" class:on={d.key === chosen} data-testid="store-backup-drive">
            <input type="radio" name="st-bk-drive" value={d.key} checked={d.key === chosen} onchange={() => (chosen = d.key)} />
            <span class="drive-main"><b>{d.pack?.name ?? d.label}</b><span class="note">{d.label} · {d.fs}</span></span>
            <span class="free" class:warn={p !== undefined && !p.fits}>{p ? driveFreeLine(p) : ''}</span>
            <span class="usage" aria-hidden="true">
              <b class="u-grid" style:width={`${usedPct(d).toFixed(2)}%`}></b>
              {#if d.key === chosen}<b class="u-adding" class:over={p !== undefined && !p.fits} style:width={`${addPct(d, p).toFixed(2)}%`}></b>{/if}
            </span>
          </label>
        {/each}
      </div>
    {/if}
  </section>

  {#if ready.length > 0 && view && view.rows.length > 0}
    <section class="sub">
      <h4>What goes on the drive</h4>
      <ul class="src-list" data-testid="store-backup-rows">
        {#each view.rows as r (r.key)}
          <li>
            <span class="src-name"><b>{r.name}</b><span class="note">{r.note}</span></span>
            <span class="src-tag"><Icon name={r.icon} size={12} />{r.tag}</span>
          </li>
        {/each}
      </ul>
      {#if view.windowsNote}<p class="note">{WINDOWS_PREFIX_NOTE}</p>{/if}
    </section>
  {/if}
  {#if ready.length > 0}<p class="note">{STORE_BACKUP_COVERS_NOTE}</p>{/if}
  {#if view && (view.skipped || view.refusals.length > 0)}
    <div class="notes" data-testid="store-backup-notes">
      {#if view.skipped}<p class="note">{view.skipped}</p>{/if}
      {#each view.refusals as r (r)}<p class="note warn">{r}</p>{/each}
    </div>
  {/if}
  {#if error}<p class="error" role="alert">{error}</p>{/if}

  <div class="foot">
    <span class="note">{ready.length > 0 && view ? view.footNote : ''}</span>
    <button class="g-btn secondary" onclick={onClose}>Cancel</button>
    {#if ready.length > 0}
      <button class="g-btn" data-testid="store-backup-go" disabled={!view?.canStart || starting} onclick={start}>
        {view ? view.button : 'Backup'}
      </button>
    {/if}
  </div>
</StoreDialog>

<style>
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

  .u-grid {
    background: var(--line-strong);
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

  .src-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .src-list li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 12px;
    border-radius: var(--r-row);
    border: 1px solid var(--line);
    background: var(--surface);
  }

  .src-name {
    display: flex;
    flex-direction: column;
    gap: 1px;
    flex: 1 1 auto;
    min-width: 0;
    font-size: 13px;
  }

  .src-name b {
    color: var(--text-h);
  }

  .src-tag {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    flex: none;
    font-size: 11px;
    font-weight: 600;
    padding: 2px 8px;
    border-radius: var(--r-pill);
    background: var(--field);
    color: var(--text-h);
  }

  .notes {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .notes .warn {
    color: var(--warning-text);
  }
</style>
