// A system's page in TV mode (checklist items 326–336; tvmode.js
// `systemPage`, `pickEmu`, `emuItems`; README § Pages › System). Pure: the
// rows for each kind of page, with the desktop's Settings › Systems rules
// (SPEC.md) where the prototype simplifies — companions, the PS3 firmware
// line, the shared setup tips, and an emulator no system uses (Other
// emulators). TvSystemPage reads the shared SystemsStore and runs the
// actions.
import type { FirmwareAction } from '../../emulators/firmware';
import type { Ps3FirmwareView } from '../../systems/ps3Firmware';
import type { PickerOption } from '../../systems/rows';
import type { ListItem } from '../overlays/overlays.svelte';
import type { Row } from '../rows';

/** One emulator's line on the page (the desktop's EmulatorBlock). */
export type EmuLine = {
  name: string;
  /** "2.3.1 · up to date", the installed tag, or ''. */
  version: string;
  /** The newer version when one is out. */
  updateTo: string | null;
  updating: boolean;
  /** "Also plays Dreamcast, Saturn" or ''. */
  alsoPlays: string;
  /** Setup tips (emulators/notes.ts). */
  tips: string[];
};

export type CoreLine = { options: string[]; selected: string; disabled: boolean };

export type FirmwareLine = {
  /** "PlayStation firmware · 3 files". */
  label: string;
  /** "PlayStation": the installed line's label. */
  platformName: string;
  action: FirmwareAction;
};

/** A companion under the emulator (ShadPS4's Qt launcher). */
export type CompanionLine =
  | { kind: 'installed'; key: string; line: EmuLine }
  | { kind: 'missing'; key: string; profile: string; text: string; installing: boolean; canInstall: boolean };

export type SystemPageModel =
  | { kind: 'loading'; system: string }
  | { kind: 'unknown'; system: string }
  | { kind: 'none'; system: string; status: string; recommended: string | null; missing: string; installing: boolean }
  | {
      kind: 'emulator';
      system: string;
      status: string;
      emu: EmuLine;
      core: CoreLine | null;
      firmware: FirmwareLine | null;
      ps3: Ps3FirmwareView;
      companions: CompanionLine[];
      removing: boolean;
    }
  | { kind: 'other'; system: string; note: string; emu: EmuLine; ps3: Ps3FirmwareView; removing: boolean }
  | {
      kind: 'windows';
      status: string;
      tool: string | null;
      umu: string | null;
      umuAction: string | null;
      umuBusy: boolean;
      pickerBusy: boolean;
    };

export type SystemPageActions = {
  pickEmulator: () => void;
  installRecommended: () => void;
  pickCore: () => void;
  update: (name: string) => void;
  installFirmware: () => void;
  installPs3Firmware: (name: string) => void;
  open: (name: string) => void;
  settings: (name: string) => void;
  remove: (name: string) => void;
  installCompanion: (key: string) => void;
  pickProton: () => void;
  installUmu: () => void;
};

/** Ruling 19: Open <emulator> opens its window over TV mode. */
export const OPEN_SUB = 'Opens the emulator on its own to change its settings. Its window needs a mouse.';
export const SETTINGS_SUB = 'Program, launch options, cloud save folders';
export const FIRMWARE_SUB = 'Each file is checked against the size and MD5 the server lists';
export const PROTON_SUB = 'Runs Windows games on Linux';
export const FIRST_WINDOWS_GAME = 'Downloads the Steam Linux Runtime (about 200 MB)';
/** A on a row whose action is already running (consultant decision). */
export const BUSY_WHY = 'Still in progress. Try again when it finishes.';

/** The card Open shows (ruling 19). */
export function openingNotice(name: string): string {
  return `Opening ${name}…`;
}

/** The title's line (326): "<emulator> · <status>", or the status alone. */
export function pageSub(m: SystemPageModel): string {
  switch (m.kind) {
    case 'emulator':
      return `${m.emu.name} · ${m.status}`;
    case 'none':
      return m.status;
    case 'windows':
      return m.tool ? `${m.tool} · ${m.status}` : m.status;
    case 'other':
      return m.note;
    case 'loading':
      return 'Loading…';
    default:
      return '';
  }
}

/** The Remove confirmation (333). An emulator no system uses has no
 *  system to name. */
export function removeConfirm(name: string, system: string | null): { title: string; body: string; ok: string } {
  return {
    title: `Remove ${name}?`,
    body: system ? `Games and saves stay. ${system} games can't start until another emulator is picked.` : 'Games and saves stay.',
    ok: 'Remove',
  };
}

const busy = (on: boolean) => (on ? { disabled: true, why: BUSY_WHY } : {});

/** Version, Update to, Open, Emulator settings for one emulator line. */
function lineRows(prefix: string, e: EmuLine, a: SystemPageActions, opts: { label?: string; sub?: string } = {}): Row[] {
  const rows: Row[] = [];
  rows.push({ type: 'info', key: `${prefix}ver`, label: opts.label ?? 'Version', ...(opts.sub ? { sub: opts.sub } : {}), value: e.version || 'Unknown' });
  if (e.updateTo) {
    rows.push({
      type: 'action',
      key: `${prefix}update`,
      label: `Update to ${e.updateTo}`,
      ...(e.updating ? { sub: `Updating ${e.name}…` } : {}),
      ...busy(e.updating),
      run: () => a.update(e.name),
    });
  }
  return rows;
}

function ps3Rows(view: Ps3FirmwareView, name: string, a: SystemPageActions): Row[] {
  if (view?.kind === 'installed') return [{ type: 'info', key: 'ps3fw', label: view.text, value: '' }];
  if (view?.kind === 'install') {
    return [{ type: 'action', key: 'ps3fw', label: 'PS3 firmware', sub: view.text, value: 'Install', run: () => a.installPs3Firmware(name) }];
  }
  return [];
}

function tailRows(e: EmuLine, removing: boolean, a: SystemPageActions): Row[] {
  const rows: Row[] = [
    { type: 'action', key: 'open', label: `Open ${e.name}`, sub: OPEN_SUB, run: () => a.open(e.name) },
    { type: 'action', key: 'settings', label: 'Emulator settings', sub: SETTINGS_SUB, run: () => a.settings(e.name) },
    { type: 'action', key: 'remove', label: `Remove ${e.name}`, danger: true, ...busy(removing), run: () => a.remove(e.name) },
  ];
  if (e.tips.length) rows.push({ type: 'info', key: 'tips', label: 'Setup tips', sub: e.tips.join(' '), value: '' });
  return rows;
}

/** The page's rows for each kind (327–336). */
export function systemPageRows(m: SystemPageModel, a: SystemPageActions): Row[] {
  switch (m.kind) {
    case 'windows': {
      const rows: Row[] = [
        { type: 'action', key: 'proton', label: 'Default Proton version', sub: PROTON_SUB, value: m.tool ?? 'Not set', ...busy(m.pickerBusy), run: a.pickProton },
      ];
      if (m.umu !== null) rows.push({ type: 'info', key: 'umu', label: 'Proton launcher (umu-run)', value: m.umu });
      if (m.umuAction) {
        rows.push({
          type: 'action',
          key: 'umu-install',
          label: `${m.umuAction} umu-launcher`,
          ...(m.umuBusy ? { sub: 'Installing umu-launcher…' } : {}),
          ...busy(m.umuBusy),
          run: a.installUmu,
        });
      }
      rows.push({ type: 'info', key: 'first', label: 'First Windows game', value: FIRST_WINDOWS_GAME });
      return rows;
    }
    case 'none': {
      const rows: Row[] = [];
      if (m.recommended) {
        rows.push({
          type: 'action',
          key: 'install',
          label: `Install ${m.recommended} (recommended)`,
          ...(m.installing ? { sub: `Installing ${m.recommended}…` } : {}),
          ...busy(m.installing),
          run: a.installRecommended,
        });
      } else {
        rows.push({ type: 'info', key: 'missing', label: m.missing, value: '' });
      }
      rows.push({ type: 'action', key: 'choose', label: 'Choose another emulator', ...busy(m.installing), run: a.pickEmulator });
      return rows;
    }
    case 'emulator': {
      const rows: Row[] = [
        { type: 'action', key: 'emu', label: 'Emulator', ...(m.emu.alsoPlays ? { sub: m.emu.alsoPlays } : {}), value: m.emu.name, run: a.pickEmulator },
        ...lineRows('', m.emu, a),
      ];
      if (m.core) {
        rows.push({
          type: 'action',
          key: 'core',
          label: 'RetroArch core',
          value: m.core.disabled ? 'No installed core' : m.core.selected,
          ...(m.core.disabled ? { disabled: true, why: 'No installed core' } : {}),
          run: a.pickCore,
        });
      }
      for (const c of m.companions) {
        if (c.kind === 'installed') {
          rows.push(...lineRows(`comp-${c.key}-`, c.line, a, { label: c.line.name, sub: 'Companion' }));
          rows.push({ type: 'action', key: `comp-${c.key}-open`, label: `Open ${c.line.name}`, sub: OPEN_SUB, run: () => a.open(c.line.name) });
          // Its settings and Remove (ruling E4; "Remove <Companion>?" / "Games and saves stay.").
          rows.push({ type: 'action', key: `comp-${c.key}-settings`, label: `${c.line.name} settings`, sub: SETTINGS_SUB, run: () => a.settings(c.line.name) });
          rows.push({ type: 'action', key: `comp-${c.key}-remove`, label: `Remove ${c.line.name}`, danger: true, ...busy(m.removing), run: () => a.remove(c.line.name) });
        } else if (c.canInstall) {
          rows.push({
            type: 'action',
            key: `comp-${c.key}-install`,
            label: `Install ${c.profile}`,
            sub: c.installing ? `Installing ${c.profile}…` : c.text,
            ...busy(c.installing),
            run: () => a.installCompanion(c.key),
          });
        } else {
          rows.push({ type: 'info', key: `comp-${c.key}-missing`, label: c.profile, sub: c.text, value: '' });
        }
      }
      const fw = m.firmware;
      if (fw && fw.action === 'installed') rows.push({ type: 'info', key: 'fw', label: `${fw.platformName} firmware`, value: 'Installed' });
      else if (fw && (fw.action === 'install' || fw.action === 'requested')) {
        rows.push({
          type: 'action',
          key: 'fw',
          label: `${fw.label} · from your server`,
          sub: FIRMWARE_SUB,
          value: fw.action === 'requested' ? 'Installing…' : 'Install',
          ...busy(fw.action === 'requested'),
          run: a.installFirmware,
        });
      }
      rows.push(...ps3Rows(m.ps3, m.emu.name, a));
      rows.push(...tailRows(m.emu, m.removing, a));
      return rows;
    }
    case 'other':
      return [...lineRows('', m.emu, a), ...ps3Rows(m.ps3, m.emu.name, a), ...tailRows(m.emu, m.removing, a)];
    default:
      return [];
  }
}

type Groups = { selected: string; installed: PickerOption[]; download: PickerOption[] };

/** Installed and Download groups (headings not focusable), the current
 *  option checked. Labels drop the desktop's "Install " (the group says it). */
function groupedItems(groups: Groups, pick: (value: string) => void): ListItem[] {
  const items: ListItem[] = [];
  for (const [name, list] of [['Installed', groups.installed], ['Download', groups.download]] as const) {
    if (!list.length) continue;
    items.push({ group: name });
    list.forEach((o, i) =>
      items.push({
        label: o.label.replace(/^Install /, ''),
        checked: o.value === groups.selected,
        testId: `tv-emu-opt-${name.toLowerCase()}-${i}`,
        onPick: () => pick(o.value),
      }),
    );
  }
  return items;
}

/**
 * The emulator list (327–328; tvmode.js `emuItems`): Installed and Download
 * groups, "(recommended)" after the recommended one, then None at the end.
 */
export function emulatorListItems(groups: Groups, noneValue: string, pick: (value: string) => void): ListItem[] {
  return [
    ...groupedItems(groups, pick),
    { group: ' ' },
    { label: 'None', checked: groups.selected === noneValue, testId: 'tv-emu-opt-none', onPick: () => pick(noneValue) },
  ];
}

/** The Proton list ("Default Proton version"): the same two groups; no
 *  None (the desktop's picker has none either). */
export function protonListItems(picker: Groups, pick: (value: string) => void): ListItem[] {
  return groupedItems(picker, pick);
}

/** The Proton launcher line's value (336): which umu-run a launch uses. */
export function umuValue(status: { source: string; path?: string | null; release_tag?: string | null }): string {
  if (status.source === 'system') return status.path ? `System · ${status.path}` : 'System';
  if (status.source === 'managed') return status.release_tag ? `GRID-managed · ${status.release_tag}` : 'GRID-managed';
  return 'Not installed';
}
