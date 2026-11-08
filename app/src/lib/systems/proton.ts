// Settings › Systems › Windows (Linux only): the default Proton version is
// a system row whose picker is the installer, grouped like the emulator
// pickers (ProposedApp README, "Pickers group their choices"). Pure.
import type { CatalogEntry, CompatTool, DownloadEntry } from '../api';
import type { PickerOption, SystemStatus } from './rows';

/** GE-Proton: what GRID recommends for Windows games. */
export const RECOMMENDED_PROTON_SOURCE = 'GloriousEggroll/proton-ge-custom';

const INSTALL = 'install:';
/** The placeholder value shown while no default is set. */
export const NO_PROTON_VALUE = '';

/** An installed tool's line: system Wine, Steam's Proton, GRID's downloads. */
export function protonToolLabel(tool: CompatTool): string {
  if (tool.source === 'steam') return `${tool.name} (Steam)`;
  if (tool.source === 'managed') return `${tool.name} (downloaded by GRID)`;
  return tool.name;
}

/** A catalog entry's Download line. The catalog's tag is the release it
 *  follows ("latest"); no forge call is spent learning the real one. */
export function protonDownloadLabel(entry: CatalogEntry, recommended: boolean): string {
  const tag = entry.tag.trim() || 'latest';
  return `Install ${entry.name} (${tag}${recommended ? ', recommended' : ''})`;
}

export type ProtonPicker = {
  selected: string;
  /** No default is set: the picker shows a placeholder. */
  unset: boolean;
  installed: PickerOption[];
  download: PickerOption[];
};

/**
 * Installed: system Wine, Steam's Proton, then GRID's downloads (discovery
 * order). Download: the catalog entries not installed, GE-Proton first.
 * Values are a tool's path (what `default_compat_tool` stores) or
 * `install:<source_id>`.
 */
export function protonPicker(
  tools: readonly CompatTool[],
  catalog: readonly CatalogEntry[],
  defaultTool: string,
): ProtonPicker {
  const order = (t: CompatTool) => (t.kind === 'wine' ? 0 : t.source === 'steam' ? 1 : 2);
  const installed = [...tools]
    .sort((a, b) => order(a) - order(b))
    .map((t) => ({ value: t.path, label: protonToolLabel(t), recommended: false }));
  const download = catalog
    .filter((e) => !e.installed)
    .sort(
      (a, b) =>
        Number(b.source_id === RECOMMENDED_PROTON_SOURCE) - Number(a.source_id === RECOMMENDED_PROTON_SOURCE),
    )
    .map((e) => {
      const recommended = e.source_id === RECOMMENDED_PROTON_SOURCE;
      return { value: INSTALL + e.source_id, label: protonDownloadLabel(e, recommended), recommended };
    });
  const known = installed.some((o) => o.value === defaultTool);
  return {
    selected: known ? defaultTool : NO_PROTON_VALUE,
    unset: !known,
    installed,
    download,
  };
}

/** The catalog `source_id` a picker value installs, or `null` for an installed tool. */
export function protonInstallSource(value: string): string | null {
  return value.startsWith(INSTALL) ? value.slice(INSTALL.length) : null;
}

const LIVE = new Set<DownloadEntry['status']>(['queued', 'downloading', 'installing', 'cancelling']);

/** Whether a compat-tool download from the catalog is running. umu-launcher
 *  shares the Downloads kind, so only catalog sources count. */
export function protonInstalling(entries: readonly DownloadEntry[], catalog: readonly CatalogEntry[]): boolean {
  const sources = new Set(catalog.map((e) => e.source_id));
  return entries.some((e) => e.kind === 'compat_tool' && LIVE.has(e.status) && sources.has(e.source_id));
}

/** The Windows row's status: Installing… while a Proton download runs,
 *  Ready once a default is set, else no emulator. */
export function protonStatus(unset: boolean, installing: boolean): SystemStatus {
  if (installing) return 'installing';
  return unset ? 'none' : 'ready';
}
