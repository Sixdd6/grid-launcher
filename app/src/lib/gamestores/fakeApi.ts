// A fake of the store commands and events, with the GameStores prototype's
// GOG data (placeholder titles only; fake markers, never a real account or
// token). Used by the vitest suites and, behind the dev-only VITE_FAKE_STORES
// flag (`devMock.ts`), to click through every state in a browser. It answers
// `invoke(cmd, args)` calls by command name and emits events through the
// `emit` it is given; it never touches the network or the disk.
import type {
  DownloadEntry,
  DownloadKind,
  DriveStoreGame,
  InstallerInspection,
  JobFailure,
  JobPhase,
  StoreBackupPlan,
  StoreCloudState,
  StoreGame,
  StoreGameSettings,
  StoreInstallOptions,
  StoreInstallRequest,
  StoreLibrary,
  StoreNotice,
  StoreStatus,
} from '../api';

const GB = 1024 ** 3;
const MB = 1024 ** 2;
const NOW = () => Math.floor(Date.now() / 1000);

/** A fake username: never a real account. */
export const FAKE_USERNAME = 'sam_plays';

type Seed = Partial<StoreGame> & { id: string; title: string; hue: string; dl: number; disk: number };

const SEEDS: Seed[] = [
  {
    id: '1001', title: 'Ashen Tide', hue: 'hsl(12 55% 38%)', installed: true, installed_platform: 'linux', version: '1.0.4', dl: 14.2, disk: 21.6,
    cloud: 'synced', platforms: ['linux', 'windows'], origin: 'gogdl', developer: 'Lantern Works', year: '2021', genres: ['RPG'],
    dlc: [
      { id: '2001', title: 'Ashen Tide — Original Soundtrack', download_bytes: 0.41 * GB, installed: true },
      { id: '2002', title: 'Ashen Tide — Digital Artbook', download_bytes: 0.12 * GB, installed: false },
    ],
  },
  {
    id: '1002', title: 'Lanternfall', hue: 'hsl(42 60% 36%)', installed: true, installed_platform: 'windows', version: '2.1', update_version: '2.2', dl: 32.8, disk: 41.0,
    cloud: 'conflict', platforms: ['windows'], origin: 'gogdl', developer: 'Brine & Ember', year: '2023', genres: ['Strategy'],
  },
  {
    id: '1003', title: 'Brightwater', hue: 'hsl(190 50% 32%)', installed: true, installed_platform: 'linux', version: '1.3', dl: 3.1, disk: 4.4,
    cloud: 'off', platforms: ['linux', 'windows'], origin: 'offline', developer: 'Small Harbor', year: '2019', genres: ['Puzzle'],
  },
  { id: '1004', title: 'Copper Saints', hue: 'hsl(25 45% 30%)', dl: 18.4, disk: 26.0, platforms: ['linux', 'windows'], developer: 'Iron Bell', year: '2022', genres: ['Action'] },
  { id: '1005', title: 'Hush Valley', hue: 'hsl(140 30% 30%)', dl: 6.2, disk: 8.0, platforms: ['windows'], developer: 'Fernlight', year: '2020', genres: ['Adventure'] },
  {
    id: '1006', title: 'Marrowind Keep', hue: 'hsl(300 25% 30%)', dl: 22.0, disk: 30.5, platforms: ['linux', 'windows'], developer: 'Quill Hall', year: '2018', genres: ['RPG'],
    dlc: [
      { id: '2003', title: 'Marrowind Keep — The Sunken Crypt', download_bytes: 2.4 * GB, installed: false },
      { id: '2004', title: 'Marrowind Keep — Soundtrack', download_bytes: 0.3 * GB, installed: false },
    ],
  },
  { id: '1007', title: 'Paper Lanterns', hue: 'hsl(350 45% 40%)', dl: 1.1, disk: 1.6, platforms: ['linux'], developer: 'Origami House', year: '2017', genres: ['Puzzle'] },
  { id: '1008', title: 'Quarry Nine', hue: 'hsl(210 15% 34%)', dl: 9.7, disk: 13.2, platforms: ['windows'], developer: 'Deepcut', year: '2024', genres: ['Simulation'] },
];

/** A cover as an inline SVG: the prototype's coloured tile with the title. */
export function fakeCover(title: string, hue: string): string {
  const esc = title.replace(/&/g, '&amp;').replace(/</g, '&lt;');
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="300" height="400"><rect width="300" height="400" fill="${hue}"/><text x="150" y="210" font-family="system-ui,sans-serif" font-size="26" font-weight="600" fill="rgba(255,255,255,.85)" text-anchor="middle">${esc}</text></svg>`;
  return `data:image/svg+xml;utf8,${encodeURIComponent(svg)}`;
}

function game(s: Seed): StoreGame {
  return {
    key: `gog:${s.id}`,
    source: 'gog',
    store_id: s.id,
    title: s.title,
    installed: s.installed ?? false,
    cover: fakeCover(s.title, s.hue),
    screenshots: [0, 1, 2, 3].map((i) => fakeCover(`${s.title} ${i + 1}`, s.hue)),
    description: `${s.title} is a placeholder title. The real Overview shows the store’s own description here, split into paragraphs.\n\nRelated games aren’t shown for store games: the store doesn’t say which games it relates.`,
    developer: s.developer ?? '',
    genres: s.genres ?? [],
    year: s.year ?? '',
    platforms: s.platforms ?? ['windows'],
    installed_platform: s.installed_platform ?? null,
    download_bytes: Math.round(s.dl * GB),
    disk_bytes: Math.round(s.disk * GB),
    version: s.version ?? '',
    update_version: s.update_version ?? '',
    origin: s.installed ? (s.origin ?? 'gogdl') : null,
    not_supported: '',
    dlc: (s.dlc ?? []).map((d) => ({ ...d, download_bytes: Math.round(d.download_bytes) })),
    cloud: s.installed ? (s.cloud ?? 'synced') : 'unknown',
    last_sync: s.installed ? NOW() - 2 * 3600 : 0,
    install_dir: s.installed ? `~/Games/GRID/stores/gog/${s.title.replace(/[^A-Za-z0-9]+/g, '')}` : '',
    compat_tool: s.installed_platform === 'windows' ? 'GE-Proton10-17' : '',
  };
}

export function baseStatus(): StoreStatus {
  return {
    source: 'gog',
    connection: 'connected',
    username: FAKE_USERNAME,
    tool: {
      name: 'gogdl',
      version: '1.4.0',
      state: 'installed',
      size_bytes: Math.round(0.94 * MB),
      error: '',
      license: 'GPL-3.0',
      source_url: 'https://github.com/Heroic-Games-Launcher/heroic-gogdl',
      done: 0,
      total: 0,
    },
    owned: 'ready',
    owned_error: '',
    owned_count: 8,
    installed_count: 3,
    installed_bytes: 0,
    refreshed_at: NOW() - 300,
    cloud_saves: true,
    install_dir: '~/Games/GRID/stores/gog',
    in_rail: true,
    last_error: '',
  };
}

/** The prototype's dashed "Prototype" select, as named scenarios. */
export type FakeScenario =
  | 'normal'
  | 'loading'
  | 'emptyInstalled'
  | 'everythingInstalled'
  | 'toolMissing'
  | 'toolInstalling'
  | 'toolFailed'
  | 'needsPython'
  | 'expired'
  | 'notConnected'
  | 'notConnectedNothing';

export const FAKE_SCENARIOS: readonly FakeScenario[] = [
  'normal', 'loading', 'emptyInstalled', 'everythingInstalled', 'toolMissing', 'toolInstalling', 'toolFailed', 'needsPython', 'expired', 'notConnected', 'notConnectedNothing',
];

/** The offline installers the fake file chooser offers (the prototype's ST_FILES). */
export const FAKE_INSTALLERS: readonly { path: string; insp: () => InstallerInspection }[] = [
  { path: '/home/player/Downloads/paper_lanterns_1_1_0_60321.sh', insp: () => found('linux_sh', '1007', 'Paper Lanterns', '1.1.0', 1.1, { kind: 'install' }) },
  { path: '/home/player/Downloads/setup_quarry_nine_1.4_(64bit)_(69001).exe', insp: () => found('windows_exe', '1008', 'Quarry Nine', '1.4', 9.4, { kind: 'install' }) },
  { path: '/home/player/Downloads/ashen_tide_1_0_5_71240.sh', insp: () => found('linux_sh', '1001', 'Ashen Tide', '1.0.5', 6.3, { kind: 'update', installed_version: '1.0.4' }) },
  { path: '/home/player/Downloads/ashen_tide_digital_artbook_71299.sh', insp: () => found('linux_sh', '2002', 'Ashen Tide — Digital Artbook', '', 0.118, { kind: 'dlc', base_title: 'Ashen Tide' }) },
  { path: '/home/player/Downloads/setup_ashen_tide_1.0.4_(64bit)_(71234).exe', insp: () => refused('windows_exe', 'Ashen Tide is installed here as the Linux version', 'This is the Windows version’s installer. To switch, uninstall Ashen Tide first, then open this installer again.') },
  { path: '/home/player/Downloads/setup_marrowind_keep_1.2_(64bit)_(70555).exe', insp: () => refused('windows_exe', 'Part of this installer is missing', 'This installer comes in 6 parts. setup_marrowind_keep_1.2_(64bit)_(70555)-1.bin isn’t in the same folder. Download every part from your GOG library and put them beside the .exe.') },
  { path: '/home/player/Downloads/setup_tidewater_tales_dlc_(55120).exe', insp: () => refused('windows_exe', 'This is DLC for Tidewater Tales, which isn’t installed', 'Install Tidewater Tales first, from GOG or from its own installer. Then install this DLC.') },
  { path: '/home/player/Downloads/ashen_tide_1.0.4_71234.pkg', insp: () => refused('mac_pkg', 'This is a macOS installer', 'GRID installs GOG’s Linux (.sh) and Windows (.exe) installers. Download one of those from your GOG library.') },
  { path: '/home/player/Downloads/Moonrise_Setup.exe', insp: () => refused('windows_exe', 'This isn’t a GOG installer', 'Moonrise_Setup.exe has no GOG game info in it. GRID reads GOG’s own setup_….exe and ….sh installers.') },
];

function found(kind: InstallerInspection['kind'], id: string, title: string, version: string, gb: number, verdict: InstallerInspection['verdict']): InstallerInspection {
  const seed = SEEDS.find((s) => s.id === id);
  return { path: '', kind, key: `gog:${id}`, title, cover: seed ? fakeCover(seed.title, seed.hue) : '', version, bytes: Math.round(gb * GB), verdict };
}

function refused(kind: InstallerInspection['kind'], title: string, body: string): InstallerInspection {
  return { path: '', kind, key: null, title: '', cover: '', version: '', bytes: 0, verdict: { kind: 'refused', title, body } };
}

export type FakeEmit = (event: string, payload: unknown) => void;

/** Thrown for a command the fake does not answer. */
export const NOT_HANDLED = Symbol('not handled');

let nextEntryId = 9000;

function entry(partial: Partial<DownloadEntry> & { kind: DownloadKind; title: string; source_id: string }): DownloadEntry {
  nextEntryId += 1;
  return {
    id: nextEntryId,
    job: 'store',
    rom_id: 0,
    platform: 'GOG',
    status: 'queued',
    downloaded_bytes: 0,
    total_bytes: 0,
    speed_bps: 0,
    install_processed_bytes: 0,
    install_total_bytes: 0,
    error: '',
    detail: '',
    drive_id: '',
    drive_name: '',
    drive_label: '',
    batch: 0,
    phase: 'none',
    failure: null,
    ...partial,
  };
}

/**
 * The fake backend. `emit` sends events to the app (the dev mock wires it to
 * Tauri's mocked event bus; tests pass a spy). `tick()` moves running jobs on
 * (the dev mock calls it on a timer; tests call it by hand).
 */
export class FakeStores {
  status: StoreStatus = baseStatus();
  games: StoreGame[] = SEEDS.map(game);
  entries: DownloadEntry[] = [];
  driveGames: DriveStoreGame[] = [];
  cloud: Record<string, StoreCloudState> = {};
  settings: Record<string, StoreGameSettings> = {};
  /** The next sign-in's outcome (the prototype's dashed buttons). */
  nextLogin: 'connected' | 'failed' | 'cancelled' = 'connected';
  emit: FakeEmit;
  /** The game key a launch reports as a cloud conflict until it is resolved. */
  private snapshot: StoreGame[] | null = null;

  constructor(emit: FakeEmit = () => {}) {
    this.emit = emit;
    this.cloud['gog:1002'] = {
      state: 'conflict',
      synced_at: NOW() - 3 * 86400,
      locations: ['~/Games/GRID/stores/gog/Lanternfall/pfx/drive_c/users/steamuser/Saved Games/Lanternfall'],
      conflict: { local_at: NOW() - 3600, local_files: 3, local_bytes: Math.round(2.1 * MB), cloud_at: NOW() - 3 * 3600 },
    };
  }

  /** The prototype's boot: one GOG install running at 38%, one queued. */
  bootJobs(): void {
    const cs = this.games.find((g) => g.store_id === '1004')!;
    const hv = this.games.find((g) => g.store_id === '1005')!;
    this.entries.push(
      entry({ kind: 'store_install', title: cs.title, source_id: cs.key, status: 'downloading', phase: 'downloading', total_bytes: cs.download_bytes, downloaded_bytes: Math.round(cs.download_bytes * 0.38), speed_bps: 41.2 * MB }),
      entry({ kind: 'store_install', title: hv.title, source_id: hv.key, status: 'queued' }),
    );
  }

  library(): StoreLibrary {
    const installed = this.games.filter((g) => g.installed);
    this.status.installed_count = installed.length;
    this.status.installed_bytes = installed.reduce((t, g) => t + g.disk_bytes, 0);
    this.status.owned_count = this.games.length;
    const avail = this.status.owned === 'ready' && this.status.connection === 'connected' ? this.games.filter((g) => !g.installed) : [];
    return { status: { ...this.status }, installed, available: avail };
  }

  /** Applies a prototype state. The base data comes back first, then the state applies. */
  scenario(s: FakeScenario): void {
    this.status = baseStatus();
    if (this.snapshot) {
      this.games = this.snapshot;
      this.snapshot = null;
    }
    const keep = () => (this.snapshot = this.games.map((g) => ({ ...g })));
    switch (s) {
      case 'loading':
        this.status.owned = 'loading';
        break;
      case 'emptyInstalled':
        keep();
        this.games = this.games.map((g) => ({ ...g, installed: false, origin: null, installed_platform: null }));
        break;
      case 'everythingInstalled':
        keep();
        this.games = this.games.map((g) => ({ ...g, installed: true, origin: g.origin ?? 'gogdl', installed_platform: g.installed_platform ?? g.platforms[0], version: g.version || '1.0' }));
        break;
      case 'toolMissing':
        this.status.tool = { ...this.status.tool, state: 'not_installed', version: '' };
        break;
      case 'toolInstalling':
        this.status.tool = { ...this.status.tool, state: 'installing', done: 400_000, total: this.status.tool.size_bytes };
        break;
      case 'toolFailed':
        this.status.tool = { ...this.status.tool, state: 'failed', error: 'the download didn’t match its checksum' };
        break;
      case 'needsPython':
        this.status.tool = { ...this.status.tool, state: 'failed', error: 'needs Python 3.10' };
        break;
      case 'expired':
        this.status.connection = 'expired';
        break;
      case 'notConnected':
        this.status.connection = 'not_connected';
        this.status.username = '';
        this.status.tool = { ...this.status.tool, state: 'not_installed' };
        break;
      case 'notConnectedNothing':
        keep();
        this.status.connection = 'not_connected';
        this.status.username = '';
        this.status.in_rail = false;
        this.games = this.games.map((g) => ({ ...g, installed: false }));
        break;
      default:
        break;
    }
    this.status.in_rail = this.status.connection !== 'not_connected' || this.games.some((g) => g.installed);
    this.changed();
  }

  /** Emits `stores-changed` and the downloads snapshot. */
  changed(): void {
    this.library();
    this.emit('stores-changed', [{ ...this.status }]);
    this.emitDownloads();
  }

  emitDownloads(): void {
    this.emit('downloads-changed', { entries: [...this.entries].reverse() });
  }

  notice(message: string, opts: Partial<StoreNotice> = {}): void {
    this.emit('store-notice', { source: 'gog', message, detail: '', error: false, action: null, ...opts });
  }

  private queue(kind: DownloadKind, g: StoreGame | null, title?: string, extra: Partial<DownloadEntry> = {}): DownloadEntry {
    const busy = this.entries.some((e) => e.kind !== 'store_tool' && (e.status === 'downloading' || e.status === 'installing'));
    const e = entry({
      kind,
      title: title ?? g?.title ?? '',
      source_id: g?.key ?? 'gog',
      status: busy && kind !== 'store_tool' ? 'queued' : 'downloading',
      total_bytes: g?.download_bytes ?? this.status.tool.size_bytes,
      phase: kind === 'store_repair' ? 'verifying' : kind === 'store_installer' ? 'unpacking' : kind === 'store_tool' ? 'checksum' : 'downloading',
      ...extra,
    });
    this.entries.push(e);
    this.emitDownloads();
    return e;
  }

  /** Moves running jobs on; finishes those that reach 100%. */
  tick(): void {
    let moved = false;
    for (const e of this.entries) {
      if (e.status !== 'downloading' && e.status !== 'installing') continue;
      moved = true;
      const step = e.kind === 'store_tool' ? 0.22 : e.kind === 'store_repair' ? 0.06 : 0.02;
      if (e.status === 'downloading') {
        const total = e.total_bytes || GB;
        e.total_bytes = total;
        e.downloaded_bytes = Math.min(total, e.downloaded_bytes + Math.round(total * step));
        e.speed_bps = 41.2 * MB;
        if (e.downloaded_bytes >= total) {
          if (e.kind === 'store_install' || e.kind === 'store_update' || e.kind === 'store_dlc') {
            e.status = 'installing';
            e.phase = 'installing';
            e.install_total_bytes = total;
            e.install_processed_bytes = Math.round(total * 0.85);
          } else this.finish(e);
        }
      } else {
        e.install_processed_bytes = Math.min(e.install_total_bytes, e.install_processed_bytes + Math.round(e.install_total_bytes * 0.05));
        if (e.install_processed_bytes >= e.install_total_bytes) this.finish(e);
      }
    }
    if (!this.entries.some((e) => e.kind !== 'store_tool' && (e.status === 'downloading' || e.status === 'installing'))) {
      const q = this.entries.find((e) => e.status === 'queued');
      if (q) {
        q.status = 'downloading';
        q.phase = q.kind === 'store_repair' ? 'verifying' : q.kind === 'store_installer' ? 'unpacking' : 'downloading';
        moved = true;
      }
    }
    if (moved) this.emitDownloads();
  }

  private finish(e: DownloadEntry): void {
    e.status = 'completed';
    const g = this.games.find((x) => x.key === e.source_id);
    if (e.kind === 'store_tool') {
      this.status.tool = { ...this.status.tool, state: 'installed', version: '1.4.0' };
      e.detail = '1.4.0';
      this.notice('gogdl installed');
      this.changed();
      return;
    }
    if (!g) return;
    if (e.kind === 'store_update') {
      g.version = g.update_version;
      g.update_version = '';
      this.notice(`${g.title} updated to ${g.version}`);
    } else if (e.kind === 'store_repair') {
      e.detail = '2 files repaired';
      this.notice(`${g.title}: 2 damaged files were downloaded again`);
    } else if (e.kind === 'store_dlc') {
      for (const d of g.dlc) if (e.title === d.title) d.installed = true;
      this.notice(`${e.title} installed`);
    } else {
      g.installed = true;
      g.installed_platform = g.installed_platform ?? (g.platforms.includes('linux') ? 'linux' : 'windows');
      g.version = g.version || '1.0';
      g.origin = e.kind === 'store_installer' ? 'offline' : e.kind === 'store_restore' ? 'drive' : 'gogdl';
      g.cloud = 'synced';
      g.last_sync = NOW();
      g.install_dir = `~/Games/GRID/stores/gog/${g.title.replace(/[^A-Za-z0-9]+/g, '')}`;
      this.notice(e.kind === 'store_installer' ? `${g.title} installed from its installer` : `${g.title} installed`, { action: { kind: 'play', key: g.key } });
    }
    this.changed();
  }

  /** Answers one `invoke`; `NOT_HANDLED` for a command that isn't a store command. */
  handle(cmd: string, args: Record<string, unknown>): unknown {
    const key = typeof args.key === 'string' ? args.key : '';
    const g = this.games.find((x) => x.key === key) ?? null;
    switch (cmd) {
      case 'stores_status':
        this.library();
        return [{ ...this.status }];
      case 'store_library':
        return this.library();
      case 'store_refresh':
        this.status.owned = 'loading';
        this.changed();
        setTimeout(() => {
          this.status.owned = 'ready';
          this.status.refreshed_at = NOW();
          this.notice(`GOG: ${this.games.length} games found · ${this.games.filter((x) => !x.installed).length} to install`);
          this.changed();
        }, 1200);
        return null;
      case 'store_check_updates':
        return null;
      case 'store_connect':
        // The login window is the backend's own window; the fake finishes it at once.
        setTimeout(() => {
          const outcome = this.nextLogin;
          if (outcome === 'connected') {
            this.status.connection = 'connected';
            this.status.username = FAKE_USERNAME;
            this.status.in_rail = true;
            if (this.status.tool.state !== 'installed') this.installTool();
          }
          this.emit('store-login', {
            source: 'gog',
            outcome,
            username: outcome === 'connected' ? FAKE_USERNAME : '',
            error: outcome === 'failed' ? 'GOG returned an error. Try again.' : '',
          });
          this.changed();
        }, 600);
        return null;
      case 'store_cancel_login':
        return null;
      case 'store_disconnect':
        this.status.connection = 'not_connected';
        this.status.username = '';
        this.status.tool = { ...this.status.tool, state: 'not_installed' };
        this.status.in_rail = this.games.some((x) => x.installed);
        this.changed();
        return null;
      case 'store_install_tool':
        this.installTool();
        return null;
      case 'store_set_install_dir':
        this.status.install_dir = String(args.path ?? '');
        this.changed();
        return null;
      case 'store_set_cloud_saves':
        this.status.cloud_saves = Boolean(args.on);
        this.changed();
        return null;
      case 'store_install_options':
        return this.installOptions(g);
      case 'store_install': {
        const req = args.request as StoreInstallRequest;
        const target = this.games.find((x) => x.key === req.key);
        if (!target) throw new Error('No such game');
        target.installed_platform = req.platform;
        this.queue('store_install', target);
        return null;
      }
      case 'store_update':
        if (g) this.queue('store_update', g);
        return null;
      case 'store_repair':
        if (g) this.queue('store_repair', g);
        return null;
      case 'store_uninstall':
        if (g) {
          g.installed = false;
          g.update_version = '';
          g.origin = null;
          this.changed();
        }
        return null;
      case 'store_install_dlc': {
        const d = g?.dlc.find((x) => x.id === args.dlcId);
        if (g && d) this.queue('store_dlc', g, d.title, { total_bytes: d.download_bytes });
        return null;
      }
      case 'store_remove_dlc': {
        const d = g?.dlc.find((x) => x.id === args.dlcId);
        if (d) d.installed = false;
        this.changed();
        return null;
      }
      case 'store_game_settings':
        return this.settings[key] ?? this.gameSettings(g);
      case 'store_set_game_settings': {
        this.settings[key] = args.settings as StoreGameSettings;
        if (g) g.cloud = (args.settings as StoreGameSettings).cloud_saves ? (g.cloud === 'off' ? 'synced' : g.cloud) : 'off';
        this.changed();
        return null;
      }
      case 'store_cloud_state':
        return this.cloudState(g);
      case 'store_cloud_sync': {
        const st = this.cloudState(g);
        if (g) g.last_sync = NOW();
        return { ...st, state: st.state === 'conflict' ? 'conflict' : 'synced', synced_at: NOW() };
      }
      case 'store_cloud_resolve': {
        if (g) {
          g.cloud = 'synced';
          g.last_sync = NOW();
        }
        delete this.cloud[key];
        this.changed();
        return this.cloudState(g);
      }
      case 'offline_inspect': {
        const path = String(args.path ?? '');
        const hit = FAKE_INSTALLERS.find((f) => f.path === path || f.path.endsWith(`/${path}`));
        const insp = hit ? hit.insp() : refused('windows_exe', 'This isn’t a GOG installer', `${path.split('/').pop()} has no GOG game info in it. GRID reads GOG’s own setup_….exe and ….sh installers.`);
        return { ...insp, path };
      }
      case 'offline_install': {
        const path = String(args.path ?? '');
        const hit = FAKE_INSTALLERS.find((f) => f.path === path);
        const insp = hit?.insp();
        const target = insp?.key ? this.games.find((x) => x.key === insp.key) ?? null : null;
        if (insp?.verdict.kind === 'dlc') {
          const base = this.games.find((x) => x.title === (insp.verdict as { base_title: string }).base_title) ?? null;
          this.queue('store_installer', base, insp.title, { phase: 'unpacking' });
        } else {
          const e = this.queue('store_installer', target, undefined, { phase: insp?.kind === 'windows_exe' ? 'installer_silent' : 'unpacking' });
          if (insp?.kind === 'windows_exe') {
            setTimeout(() => {
              e.phase = 'installer_wizard';
              this.notice(`The ${e.title} installer couldn’t run silently, so its setup window opened. Finish it there; GRID adds the game when it closes.`);
              this.emitDownloads();
            }, 2400);
          }
        }
        return null;
      }
      case 'plan_store_backup':
        return this.backupPlan(String(args.key ?? ''), (args.gameKeys as string[]) ?? []);
      case 'start_store_backup': {
        const keys = (args.gameKeys as string[]) ?? [];
        return { batch: 1, games: keys.length, emulators: 0, firmware: 0 };
      }
      case 'drive_store_games':
        return this.driveGames;
      case 'install_store_from_drive': {
        const dg = this.driveGames.find((x) => x.key === args.gameKey);
        const target = this.games.find((x) => x.key === args.gameKey) ?? null;
        if (dg && target) {
          this.queue('store_restore', target, undefined, {
            phase: dg.layout === 'installer' ? 'copying_installer' : 'copying_from_drive',
            drive_label: 'SAMSUNG T7',
            drive_name: 'Couch kit',
          });
        }
        return null;
      }
      case 'remove_store_from_drive': {
        const keys = (args.gameKeys as string[]) ?? [];
        const before = this.driveGames.length;
        this.driveGames = this.driveGames.filter((x) => !keys.includes(x.key));
        return before - this.driveGames.length;
      }
      default:
        return NOT_HANDLED;
    }
  }

  private installTool(): void {
    this.status.tool = { ...this.status.tool, state: 'installing', done: 0, total: this.status.tool.size_bytes };
    this.queue('store_tool', null, 'gogdl', { source_id: 'gog' });
    this.notice('Installing gogdl (1 MB) for GOG…');
    this.changed();
  }

  installOptions(g: StoreGame | null): StoreInstallOptions {
    const linux = g?.platforms.includes('linux') ?? false;
    const versions: StoreInstallOptions['versions'] = (g?.platforms ?? ['windows']).map((p) => ({
      platform: p,
      recommended: p === 'linux' || !linux,
      download_bytes: g?.download_bytes ?? 0,
      disk_bytes: g?.disk_bytes ?? 0,
    }));
    return {
      versions,
      languages: g?.store_id === '1006' ? ['English', 'Deutsch', 'Français', 'Polski'] : ['English'],
      default_language: 'English',
      dlc: (g?.dlc ?? []).map((d) => ({ ...d })),
      install_dir: `~/Games/GRID/stores/gog/${(g?.title ?? '').replace(/[^A-Za-z0-9]+/g, '')}`,
      free_bytes: 412 * GB,
      volume: '/home',
      compat_tool: 'GE-Proton10-17',
    };
  }

  private gameSettings(g: StoreGame | null): StoreGameSettings {
    const win = g?.installed_platform === 'windows';
    const name = (g?.title ?? '').replace(/[^A-Za-z]+/g, '');
    return {
      install_dir: g?.install_dir ?? '',
      executable: win ? `${name}.exe` : 'start.sh',
      exe_user_pinned: false,
      launch_parameters: '',
      compat_tool: win ? 'GE-Proton10-17' : '',
      wineprefix: win ? `${g?.install_dir}/pfx` : '',
      cloud_saves: g?.cloud !== 'off',
    };
  }

  private cloudState(g: StoreGame | null): StoreCloudState {
    if (g && this.cloud[g.key]) return this.cloud[g.key];
    const loc = g?.installed_platform === 'linux' ? `~/.local/share/${(g?.title ?? '').replace(/[^A-Za-z]+/g, '')}/saves` : `${g?.install_dir}/pfx/drive_c/users/steamuser/Saved Games/${g?.title}`;
    const state = g?.cloud === 'off' ? 'off' : g?.cloud === 'none' ? 'none' : 'synced';
    return { state, synced_at: g?.last_sync ?? 0, locations: [loc], conflict: null };
  }

  private backupPlan(drive: string, keys: string[]): StoreBackupPlan {
    const rows = keys.flatMap((k) => {
      const g = this.games.find((x) => x.key === k);
      if (!g) return [];
      const busy = this.entries.some((e) => e.source_id === k && ['queued', 'downloading', 'installing'].includes(e.status));
      const already = this.driveGames.some((x) => x.key === k);
      const installer = g.origin === 'offline';
      return [
        {
          key: k,
          name: g.title,
          layout: (installer ? 'installer' : 'installed') as 'installer' | 'installed',
          detail: installer
            ? `brightwater_1_3_58810.sh · 2.9 GB, and its saves`
            : `Installed with gogdl, which keeps no installer, so the installed files are copied · ${(g.disk_bytes / GB).toFixed(1)} GB, and its saves`,
          bytes: installer ? Math.round(2.9 * GB) : g.disk_bytes,
          windows_prefix_note: g.installed_platform === 'windows',
          already,
          refused: busy ? `${g.title} is being installed or updated. Wait for it to finish.` : '',
        },
      ];
    });
    const bytes = rows.filter((r) => !r.already && !r.refused).reduce((t, r) => t + r.bytes, 0);
    const free = 230 * GB;
    return { drive, rows, bytes_to_write: bytes, free_bytes: free, fits: bytes <= free };
  }

  /** A failed RomM installer-archive row for the Downloads states (fake). */
  failedInstallerRow(title: string, platform: string, failure: JobFailure, error: string): DownloadEntry {
    return entry({ job: 'game', kind: 'base', title, source_id: '', platform, status: 'failed', failure, error });
  }

  /** A running RomM installer-archive row (fake). */
  rommInstallerRow(title: string, platform: string, phase: JobPhase, pct: number): DownloadEntry {
    return entry({
      job: 'game',
      kind: 'base',
      title,
      source_id: '',
      platform,
      status: phase === 'downloading' ? 'downloading' : 'installing',
      phase,
      total_bytes: GB,
      downloaded_bytes: phase === 'downloading' ? Math.round(GB * pct) : GB,
      install_total_bytes: GB,
      install_processed_bytes: Math.round(GB * pct),
      speed_bps: 38.2 * MB,
    });
  }
}
