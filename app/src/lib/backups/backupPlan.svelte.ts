// The Backup dialog's plan (GameBackups README, "The backup dialog";
// prototype `bkRenderDlg`), apart from its layout: the backup is planned for
// every connected, set-up drive (the chosen one first, so the others reuse
// the emulator sizes it resolved), the chosen drive's plan is read into the
// dialog's lines, and Start queues it. The desktop dialog and the TV Backup
// page both read it. Every word comes from `dialog.ts`. The queued card is
// not raised here: the batch notifications announce it.
import { untrack } from 'svelte';
import { api, type BackupPlan, type SourceChoice } from '../api';
import { isWindowsPlatform } from '../details/actions';
import { hostOf } from '../shell';
import { drives } from '../stores/drives.svelte';
import { installed } from '../stores/installed.svelte';
import { session } from '../stores/session.svelte';
import {
  chooseAll,
  chooseDrive,
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
import type { BackupDialogOpen } from './dialogState.svelte';

function errorMessage(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

export class BackupPlanStore {
  readonly #open: () => BackupDialogOpen;

  constructor(open: () => BackupDialogOpen) {
    this.#open = open;
  }

  get open(): BackupDialogOpen {
    return this.#open();
  }

  includeFirmware = $state(true);
  includeRuntime = $state(false);
  /** "Copy installed games from this computer" (ticked by default). */
  useLocal = $state(true);
  /** The picks for out-of-date local games, by rom id. */
  choices = $state<Record<number, SourceChoice>>({});
  settingsLoaded = $state(false);
  chosen = $state<string | null>(null);
  plans = $state.raw<Record<string, BackupPlan>>({});
  planErrors = $state.raw<Record<string, string>>({});
  /** Drive keys whose plan is being (re)made. */
  planning = $state.raw<ReadonlySet<string>>(new Set());
  starting = $state(false);
  startError = $state<string | null>(null);
  #generation = 0;

  ready = $derived(drives.ready);
  chosenDrive = $derived(this.ready.find((d) => d.key === this.chosen) ?? null);
  plan = $derived(this.chosen === null ? null : (this.plans[this.chosen] ?? null));
  rows = $derived(this.ready.map((d) => driveRow(d, this.plans[d.key] ?? null, d.key === this.chosen)));
  emus = $derived(this.plan ? emulatorLine(this.plan) : null);
  firmware = $derived(this.plan ? firmwareLine(this.plan) : null);
  runtime = $derived(this.plan ? runtimeLine(this.plan) : null);
  notes = $derived(this.plan && this.chosenDrive ? planNotes(this.plan, this.chosenDrive) : []);
  busy = $derived(this.chosen !== null && this.planning.has(this.chosen));
  footer = $derived(footerModel(this.busy ? null : this.plan));
  error = $derived(this.startError ?? (this.chosen !== null ? (this.planErrors[this.chosen] ?? null) : null));
  empty = $derived(noDrivesText(drives.connected));

  // The Source section. The platform and the Windows flag of a game copied
  // from this computer come from its registry row.
  localInfo = $derived.by(() => {
    const map = new Map<number, LocalGameInfo>();
    for (const r of installed.list) {
      if (r.rom_id === null || map.has(r.rom_id)) continue;
      map.set(r.rom_id, { platform: r.platform, windows: isWindowsPlatform(r.platform_slug, r.platform) });
    }
    return map;
  });
  readonly #lookup = (romId: number) => this.localInfo.get(romId);
  host = $derived(hostOf(session.serverUrl));
  showSource = $derived(this.ready.length > 0 && this.plan !== null && this.plan.local_available);
  local = $derived(this.plan ? localLine(this.plan, this.host) : null);
  sources = $derived(this.plan && this.plan.use_local ? sourceRows(this.plan, this.choices, this.#lookup) : null);
  outdated = $derived(this.plan ? outdatedRows(this.plan, this.choices) : null);
  wine = $derived(this.plan ? wineNote(this.plan, this.#lookup) : '');

  pick(romId: number, source: SourceChoice): void {
    this.choices = { ...this.choices, [romId]: source };
    this.startError = null;
  }

  pickAll(source: SourceChoice): void {
    if (this.plan === null) return;
    this.choices = chooseAll(this.plan, this.choices, source);
    this.startError = null;
  }

  choose(key: string): void {
    this.chosen = key;
    this.startError = null;
  }

  /** The firmware default comes from Settings › Backup drives. */
  loadSettings(): void {
    api
      .getBackupSettings()
      .then((s) => (this.includeFirmware = s.include_firmware))
      .catch(() => {
        // Unreadable: the README's default (on) stands.
      })
      .finally(() => (this.settingsLoaded = true));
  }

  /** Keeps a drive chosen while drives come and go. Reads only the ready
   *  drives, so an `$effect` calling it re-runs when they change. */
  keepChosen(): void {
    const next = chooseDrive(this.ready, untrack(() => this.chosen));
    if (next !== untrack(() => this.chosen)) this.chosen = next;
  }

  /**
   * Plans every ready drive again. Reads the checkboxes, the picks, the
   * request and each drive's free space and pack first, so an `$effect`
   * calling it re-plans when any of them changes; an answer for an older
   * round is dropped.
   */
  planAll(): void {
    if (!this.settingsLoaded) return;
    const fw = this.includeFirmware;
    const rt = this.includeRuntime;
    const local = this.useLocal;
    const picks = sourcePicks($state.snapshot(this.choices) as SourceChoices);
    const keys = this.ready.map((d) => d.key);
    void this.ready.map((d) => `${d.free_bytes}:${d.pack?.updated ?? ''}:${d.pack?.game_count ?? 0}`).join('|');
    const request = $state.snapshot(this.open.request);
    const first = untrack(() => this.chosen);
    const mine = ++this.#generation;
    this.planning = new Set(keys);
    const one = async (key: string) => {
      try {
        const p = await api.planBackup(key, request, fw, rt, local, picks);
        if (mine !== this.#generation) return;
        this.plans = { ...this.plans, [key]: p };
        if (key in this.planErrors) {
          const rest = { ...this.planErrors };
          delete rest[key];
          this.planErrors = rest;
        }
      } catch (err) {
        if (mine !== this.#generation) return;
        this.planErrors = { ...this.planErrors, [key]: errorMessage(err) };
      } finally {
        if (mine === this.#generation) {
          const next = new Set(this.planning);
          next.delete(key);
          this.planning = next;
        }
      }
    };
    void (async () => {
      const head = first !== null && keys.includes(first) ? first : keys[0];
      if (head === undefined) return;
      await one(head);
      if (mine !== this.#generation) return;
      await Promise.all(keys.filter((k) => k !== head).map(one));
    })();
  }

  /** Queues the chosen drive's backup. Answers whether it was queued (the
   *  `onQueued` hook has run; the dialog closes); a refusal is `startError`. */
  async start(): Promise<boolean> {
    if (this.chosen === null || this.footer.disabled || this.starting) return false;
    this.starting = true;
    this.startError = null;
    try {
      await api.startBackup(
        this.chosen,
        $state.snapshot(this.open.request),
        this.includeFirmware,
        this.includeRuntime,
        this.useLocal,
        sourcePicks($state.snapshot(this.choices) as SourceChoices),
      );
      this.open.onQueued?.();
      return true;
    } catch (err) {
      this.startError = errorMessage(err);
      return false;
    } finally {
      this.starting = false;
    }
  }
}
