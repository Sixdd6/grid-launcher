// The store install dialog (GameStores README, "Install dialog"; prototype
// `stInstallHTML`): Version (GOG, when both builds exist), Options
// (compatibility tool, language, install folder), DLC you own, the space line
// and the button. Epic Games has neither Version nor Language: its games are
// Windows builds, in one language. Pure: the dialog keeps the choice; this
// words it.
import type { StoreGame, StoreInstallOptions, StoreInstallRequest, StorePlatform, StoreSource } from '../api';
import { storeCaps } from './marks';
import { sizeGb } from './text';

export type InstallChoice = {
  platform: StorePlatform;
  language: string;
  dlcIds: string[];
  installDir: string;
  compatTool: string;
};

/** The dialog's first choice: the recommended build (Linux when there is one), the default language, every DLC ticked. */
export function initialChoice(opts: StoreInstallOptions): InstallChoice {
  const rec = opts.versions.find((v) => v.recommended) ?? opts.versions.find((v) => v.platform === 'linux') ?? opts.versions[0];
  return {
    platform: rec?.platform ?? 'windows',
    language: opts.default_language || opts.languages[0] || '',
    dlcIds: opts.dlc.map((d) => d.id),
    installDir: opts.install_dir,
    compatTool: opts.compat_tool,
  };
}

export type VersionRow = { platform: StorePlatform; label: string; note: string; recommended: boolean };

/** The Version radios, Linux first, shown only when both builds exist (a store with versions: GOG). */
export function versionRows(opts: StoreInstallOptions, compat: string, source: StoreSource = 'gog'): VersionRow[] {
  if (!storeCaps(source).hasVersions || opts.versions.length < 2) return [];
  const order: StorePlatform[] = ['linux', 'windows'];
  return order.flatMap((p) => {
    const v = opts.versions.find((x) => x.platform === p);
    if (!v) return [];
    return [
      {
        platform: p,
        label: p === 'linux' ? 'Linux' : 'Windows',
        note: p === 'linux' ? 'Runs natively on this computer' : `Runs through ${compat}`,
        recommended: v.recommended,
      },
    ];
  });
}

export type InstallSizes = { downloadBytes: number; diskBytes: number };

/** The build's sizes plus the ticked DLC (DLC takes about 1.3× its download on disk). */
export function installSizes(opts: StoreInstallOptions, choice: InstallChoice): InstallSizes {
  const v = opts.versions.find((x) => x.platform === choice.platform) ?? opts.versions[0];
  const dlc = opts.dlc.filter((d) => choice.dlcIds.includes(d.id)).reduce((t, d) => t + d.download_bytes, 0);
  return {
    downloadBytes: (v?.download_bytes ?? 0) + dlc,
    diskBytes: (v?.disk_bytes ?? 0) + Math.round(dlc * 1.3),
  };
}

export type InstallDialogView = {
  /** "Install Marrowind Keep". */
  title: string;
  /** "GOG · 22.0 GB download" (after the mark). */
  sub: string;
  versions: VersionRow[];
  /** The compatibility-tool select shows for the Windows build. */
  showCompat: boolean;
  /** The language select shows when the store offers several. */
  showLanguage: boolean;
  /** "34.0 GB on disk · 412 GB free on /home", or "Needs 34.0 GB · 18.4 GB free on /home". */
  spaceLine: string;
  /** Not enough space: the line is red and the button disabled. */
  tooBig: boolean;
  /** "24.7 GB to download". */
  footNote: string;
  button: string;
};

export function installDialogView(
  game: StoreGame,
  storeName: string,
  opts: StoreInstallOptions,
  choice: InstallChoice,
): InstallDialogView {
  const sizes = installSizes(opts, choice);
  const base = opts.versions.find((x) => x.platform === choice.platform) ?? opts.versions[0];
  const volume = opts.volume || '/';
  const tooBig = sizes.diskBytes > opts.free_bytes;
  return {
    title: `Install ${game.title}`,
    sub: `${storeName} · ${sizeGb(base?.download_bytes ?? game.download_bytes)} download`,
    versions: versionRows(opts, choice.compatTool, game.source),
    showCompat: choice.platform === 'windows',
    showLanguage: storeCaps(game.source).hasLanguages && opts.languages.length > 1,
    spaceLine: tooBig
      ? `Needs ${sizeGb(sizes.diskBytes)} · ${sizeGb(opts.free_bytes)} free on ${volume}`
      : `${sizeGb(sizes.diskBytes)} on disk · ${sizeGb(opts.free_bytes)} free on ${volume}`,
    tooBig,
    footNote: `${sizeGb(sizes.downloadBytes)} to download`,
    button: tooBig ? 'Not enough space' : 'Install',
  };
}

/** What `store_install` takes. */
export function installRequest(game: StoreGame, choice: InstallChoice): StoreInstallRequest {
  return {
    key: game.key,
    platform: choice.platform,
    language: choice.language,
    dlc_ids: [...choice.dlcIds],
    install_dir: choice.installDir,
    compat_tool: choice.platform === 'windows' ? choice.compatTool : '',
  };
}

export const COMPAT_HELP = 'The default from Settings › Systems › Windows. GRID makes a Wine prefix for this game.';
export function folderHelp(storeName: string): string {
  return `The default folder for ${storeName} games is set in Settings › Stores.`;
}
