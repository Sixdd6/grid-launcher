// The Backup page (checklist items 457–482; README § Game backups › The
// Backup page; tvmode.js `openBackup`; rulings 11 and 12): the desktop's
// backup dialog as a page of rows. Pure: TvBackupPage passes in what the
// shared `BackupPlanStore` (backups/backupPlan.svelte.ts) holds, and every
// word the dialog shares comes from `backups/dialog.ts`, so the two never
// drift. Sections: Drive, Source (only when a game can be copied from this
// computer), Also copy, the notes, then the footer's summary and main button.
import type { BackupPlan, DriveInfo, SourceChoice } from '../../api';
import {
  anyLocal,
  dialogLede,
  driveRow,
  emulatorLine,
  localLine,
  outdatedRows,
  planNotes,
  runtimeLine,
  sizeText,
  sourceRows,
  wineNote,
  type LocalGameLookup,
  type SourceChoices,
} from '../../backups/dialog';
import { plural, type Bar, type PageMain, type PageRow } from './pageRows';

export type BackupPageModel = {
  title: string;
  /** The server's host ("romm.home.lan"). */
  host: string;
  /** Connected, set-up, writable drives. */
  ready: readonly DriveInfo[];
  /** Every connected drive (for "<label> is connected but not set up"). */
  connected: readonly DriveInfo[];
  /** Each ready drive's plan, by key, as far as it has come. */
  plans: Readonly<Record<string, BackupPlan>>;
  chosen: string | null;
  /** The chosen drive's plan is being (re)made. */
  busy: boolean;
  useLocal: boolean;
  choices: SourceChoices;
  lookup?: LocalGameLookup;
  includeFirmware: boolean;
  includeRuntime: boolean;
  /** "Show each game" is open (more than 6 games, ruling 11). */
  each: boolean;
  /** The chosen drive's planning failure, or a refused start. */
  error: string | null;
  starting: boolean;
};

export type BackupPageActions = {
  choose: (key: string) => void;
  /** "Set up a drive…": Settings › Backup drives. */
  setUp: () => void;
  toggleLocal: () => void;
  /** An out-of-date game's list: Use this computer's copy / Download. */
  choiceFor: (romId: number) => void;
  pickAll: (source: SourceChoice) => void;
  toggleEach: () => void;
  toggleFirmware: () => void;
  toggleRuntime: () => void;
  start: () => void;
};

/** More than this many games sit behind "Show each game (n)" (ruling 11). */
export const EACH_INLINE_MAX = 6;

export const SET_UP_A_DRIVE = 'Set up a drive…';

/** The Drive section with no set-up drive connected (item 464). */
export function noDrivesNote(connected: readonly DriveInfo[]): string {
  const raw = connected.find((d) => d.status === 'not_set_up' && !d.read_only);
  const name = raw ? raw.label || raw.mount : '';
  return `No Backup drive is connected.${name ? ` ${name} is connected but not set up for GRID.` : ''}`;
}

/** The lede (item 458), the desktop's words. */
export function backupLede(host: string, plan: BackupPlan | null): string {
  return dialogLede(host, anyLocal(plan));
}

function packName(d: DriveInfo): string {
  return d.pack?.name ?? d.label;
}

/** One radio row of the Drive section (items 460–463; ruling 12). */
export function driveRadio(d: DriveInfo, plan: BackupPlan | null, chosen: boolean, choose: () => void): PageRow {
  const m = driveRow(d, plan, chosen);
  let sub: string;
  if (plan === null) sub = m.freeText;
  else if (m.over || m.reservedText !== '') sub = m.freeText;
  else sub = `${sizeText(Math.max(0, plan.free_bytes - plan.bytes_to_write))} left after this backup`;
  const segs = m.segments
    .filter((s) => s.kind !== 'free')
    .map((s) => ({
      kind: s.kind === 'adding' ? ('adding' as const) : s.kind === 'reserved' ? ('reserved' as const) : ('used' as const),
      pct: s.pct,
      over: s.over,
    }));
  const bar: Bar = { segs, label: m.usageLabel, after: m.reservedText || undefined };
  return {
    kind: 'row',
    key: `bk-drive-${d.key}`,
    radio: chosen,
    al: 'Choose',
    label: m.name,
    muted: ` · ${m.sub}`,
    sub,
    subErr: m.over,
    bar,
    run: choose,
  };
}

/** The firmware switch's line (item 474): "<n> systems · <n> files", or the
 *  backend's reason it is off (ruling 12, the desktop's words; no TV-only
 *  words, consultant after P13). With no reason and no set to write it is
 *  blank and off, like `dialog.ts` `firmwareLine`. */
export function firmwareText(plan: BackupPlan): { text: string; disabled: boolean } {
  if (plan.firmware_reason.trim() !== '') return { text: plan.firmware_reason.trim(), disabled: true };
  const sets = plan.firmware.filter((f) => !f.already);
  if (sets.length === 0) return { text: '', disabled: true };
  const files = sets.reduce((n, f) => n + f.files, 0);
  return { text: `${plural(sets.length, 'system')} · ${plural(files, 'file')}`, disabled: false };
}

/** An out-of-date game's row and list words (items 467–469). */
export function outdatedWords(o: { name: string; server_version: string; installed_version: string }): {
  label: string;
  sub: string;
  listSub: string;
  local: string;
  download: string;
  chosenLocal: string;
  chosenDownload: string;
} {
  const srv = o.server_version.trim();
  const have = o.installed_version.trim() || 'an older copy';
  return {
    label: `${o.name}: the server has newer files`,
    sub: srv ? `${srv} on the server; this computer has ${have}` : `This computer has ${have}`,
    listSub: `The server has newer files (${srv ? `${srv}; ` : ''}this computer has ${have})`,
    local: 'Use this computer’s copy',
    download: srv ? `Download ${srv} from the server` : 'Download the newer version from the server',
    chosenLocal: 'Use this computer’s copy',
    chosenDownload: srv ? `Download ${srv}` : 'Download the newer version',
  };
}

/** Why the main button is off (item 481). */
export const CHOOSE_WHY = 'Choose a copy for each game the server has newer files for.';
export const nothingWhy = (pack: string) => `Every game is already on ${pack}.`;
export const spaceWhy = (pack: string) => `${pack} doesn’t have room for this backup.`;
export const CHECKING_WHY = 'GRID is still checking the drive.';

/** The footer's summary and main button (items 479–481). */
export function backupMain(m: BackupPageModel, start: () => void): PageMain | null {
  const drive = m.ready.find((d) => d.key === m.chosen) ?? null;
  if (drive === null) return null;
  const plan = m.plans[drive.key] ?? null;
  const pack = packName(drive);
  if (m.busy || plan === null) {
    return { label: 'Checking the drive…', summary: '', disabled: true, why: CHECKING_WHY, hint: 'Backup', press: start };
  }
  const n = plan.games_to_write.length;
  const split = anyLocal(plan) && plan.download_bytes > 0 ? ` · ${sizeText(plan.download_bytes)} to download` : '';
  const summary = n === 0 ? '' : `${sizeText(plan.bytes_to_write)} to write${split}`;
  const off = (label: string, why: string): PageMain => ({ label, summary, disabled: true, why, hint: 'Backup', press: start });
  if (n === 0) return off('Nothing to backup', nothingWhy(pack));
  const ask = plan.use_local ? plan.needs_choice : 0;
  if (ask > 0) return off(`Choose for ${plural(ask, 'game')}`, CHOOSE_WHY);
  if (!plan.fits) return off('Not enough space', spaceWhy(pack));
  if (m.starting) return off('Starting…', 'Starting the backup.');
  return { label: `Backup ${plural(n, 'game')}`, summary, disabled: false, hint: 'Backup', press: start };
}

/** Every row of the page, in order. */
export function backupPageRows(m: BackupPageModel, act: BackupPageActions): PageRow[] {
  const R: PageRow[] = [];
  const drive = m.ready.find((d) => d.key === m.chosen) ?? null;
  const plan = drive ? (m.plans[drive.key] ?? null) : null;

  // ── Drive (items 460–464)
  R.push({ kind: 'heading', key: 'bk-h-drive', text: 'Drive' });
  if (m.ready.length === 0) {
    R.push({ kind: 'note', key: 'bk-no-drives', text: noDrivesNote(m.connected) });
    R.push({ kind: 'row', key: 'bk-setup', label: SET_UP_A_DRIVE, run: act.setUp });
    return R;
  }
  for (const d of m.ready) R.push(driveRadio(d, m.plans[d.key] ?? null, d.key === m.chosen, () => act.choose(d.key)));

  // ── Source (items 465–472, 476)
  if (plan !== null && plan.local_available) {
    R.push({ kind: 'heading', key: 'bk-h-source', text: 'Source' });
    const line = localLine(plan, m.host);
    R.push({ kind: 'row', key: 'bk-local', toggle: m.useLocal, label: line.label, sub: line.note, run: act.toggleLocal });
    if (plan.use_local) {
      const wine = wineNote(plan, m.lookup);
      if (wine !== '') R.push({ kind: 'note', key: 'bk-wine', text: wine });
      const ood = outdatedRows(plan, m.choices);
      if (ood) {
        if (ood.items.length > 1) {
          R.push({
            kind: 'note',
            key: 'bk-ood-note',
            warn: true,
            text: `${ood.items.length} games have newer files on the server. Choose a copy for each.`,
          });
        }
        for (const item of ood.items) {
          const o = plan.outdated.find((x) => x.rom_id === item.romId)!;
          const w = outdatedWords(o);
          R.push({
            kind: 'row',
            key: `bk-ood-${item.romId}`,
            warn: item.choice === null,
            al: 'Choose',
            label: w.label,
            sub: w.sub,
            value: item.choice === 'local' ? w.chosenLocal : item.choice === 'download' ? w.chosenDownload : 'Choose',
            valueWarn: item.choice === null,
            run: () => act.choiceFor(item.romId),
          });
        }
        if (ood.bulk) {
          R.push({ kind: 'row', key: 'bk-all-local', label: ood.bulk.local, run: () => act.pickAll('local') });
          R.push({ kind: 'row', key: 'bk-all-download', label: ood.bulk.download, run: () => act.pickAll('download') });
        }
      }
      const src = sourceRows(plan, m.choices, m.lookup);
      if (src.rows.length > 0) {
        const collapsed = src.rows.length > EACH_INLINE_MAX;
        if (collapsed) {
          R.push({
            kind: 'row',
            key: 'bk-each',
            label: `${m.each ? 'Hide' : 'Show'} each game`,
            value: String(src.count),
            al: m.each ? 'Hide' : 'Show',
            run: act.toggleEach,
          });
        }
        if (!collapsed || m.each) {
          for (const r of src.rows) {
            R.push({ kind: 'info', key: `bk-src-${r.romId}`, label: r.name, sub: r.sub || undefined, tag: r.tagText });
          }
        }
      }
    }
  }

  // ── Also copy (items 473–475)
  R.push({ kind: 'heading', key: 'bk-h-also', text: 'Also copy' });
  if (plan === null) {
    R.push({ kind: 'info', key: 'bk-emus', label: 'Emulators', sub: 'Checking…', check: true });
    R.push({ kind: 'row', key: 'bk-fw', toggle: m.includeFirmware, label: 'Firmware', sub: 'Checking…', disabled: true, why: CHECKING_WHY, run: act.toggleFirmware });
  } else {
    const emus = emulatorLine(plan);
    R.push({ kind: 'info', key: 'bk-emus', label: 'Emulators', sub: [emus.text, ...emus.notes].join(' · '), check: true });
    const fw = firmwareText(plan);
    R.push({
      kind: 'row',
      key: 'bk-fw',
      toggle: fw.disabled ? false : m.includeFirmware,
      label: 'Firmware',
      sub: fw.text,
      disabled: fw.disabled,
      why: fw.disabled ? fw.text : undefined,
      run: act.toggleFirmware,
    });
    const rt = runtimeLine(plan);
    if (rt) {
      R.push({
        kind: 'row',
        key: 'bk-runtime',
        toggle: rt.disabled ? false : m.includeRuntime,
        label: rt.label,
        sub: rt.text,
        disabled: rt.disabled,
        why: rt.disabled ? rt.text : undefined,
        run: act.toggleRuntime,
      });
    }
    // ── Notes (items 477–478): skipped games, the FAT32 limit.
    if (drive) {
      for (const [i, n] of planNotes(plan, drive).entries()) {
        R.push({ kind: 'note', key: `bk-note-${i}`, text: `${n.text}.`, warn: n.warn });
      }
    }
  }
  if (m.error) R.push({ kind: 'note', key: 'bk-error', text: m.error, err: true });
  return R;
}
