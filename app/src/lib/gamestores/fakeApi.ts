// A fake of the store commands and events, with the GameStores prototype's
// GOG, Epic Games, Amazon Games and Steam data (`stores.js` g1–g8, e1–e6,
// a1–a3 and s1–s7; placeholder titles only; fake markers, never a real
// account, key or token). Used by the vitest
// suites and, behind the dev-only VITE_FAKE_STORES flag (`devMock.ts`), to
// click through every state in a browser. It answers
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
  SteamTestResult,
} from '../api';

const GB = 1024 ** 3;
const MB = 1024 ** 2;
const NOW = () => Math.floor(Date.now() / 1000);

/** A fake username: never a real account. */
export const FAKE_USERNAME = 'sam_plays';
/** The fake Epic Games account (the prototype's "Signed in as SamPlays"). */
export const FAKE_EPIC_USERNAME = 'SamPlays';

/** The fake Amazon Games account (the prototype's "Signed in as sam.plays"). */
export const FAKE_AMAZON_USERNAME = 'sam.plays';

/** The fake Steam account (the prototype's SamPlays and couchguest, fake SteamID64s). */
export const FAKE_STEAM_PERSONA = 'SamPlays';
export const FAKE_STEAM_ID = '76561198000000001';
export const FAKE_STEAM_GUEST_ID = '76561198000000002';
/** The only key the fake treats as good: the fixture marker, never a real key. */
export const FAKE_STEAM_KEY = 'FAKE-STEAM-KEY-not-real';
export const FAKE_STEAM_CLIENT = '/usr/bin/steam';
export const FAKE_STEAM_NOT_INSTALLED = 'Steam isn’t installed. Install Steam, then try again.';

/** The stores the fake answers for, in the rail's order. */
export type FakeSource = 'gog' | 'epic' | 'amazon' | 'steam';
export const FAKE_SOURCES: readonly FakeSource[] = ['amazon', 'epic', 'gog', 'steam'];

function asFakeSource(src: string | undefined | null): FakeSource {
  return src === 'epic' || src === 'amazon' || src === 'steam' ? src : 'gog';
}

/** Amazon Games keeps no cloud saves and has no DLC (nile has neither); Steam's cloud is Steam's. */
function fakeHasCloud(source: FakeSource): boolean {
  return source === 'gog' || source === 'epic';
}

type Seed = Partial<StoreGame> & { id: string; title: string; hue: string; dl: number; disk: number; source?: FakeSource };

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

/**
 * The prototype's Epic Games games (`stores.js` e1–e6), keyed by a made-up
 * app name. Every Epic game is a Windows build. Iron Orchard and Northern
 * Static start through another company's launcher (Not supported). Glimmer
 * Deep has a DLC so the install dialog's "DLC you own" shows.
 */
const EPIC_SEEDS: Seed[] = [
  {
    source: 'epic', id: 'Starlight', title: 'Starlight Courier', hue: 'hsl(250 45% 36%)', installed: true, installed_platform: 'windows', version: '1.12', dl: 24.0, disk: 31.2,
    cloud: 'synced', platforms: ['windows'], developer: 'Northstar', year: '2022', genres: ['Racing'],
  },
  {
    source: 'epic', id: 'VectorBloom', title: 'Vector Bloom', hue: 'hsl(160 45% 30%)', installed: true, installed_platform: 'windows', version: '3.0.1', dl: 8.8, disk: 11.0,
    cloud: 'none', platforms: ['windows'], developer: 'Petal Logic', year: '2021', genres: ['Puzzle'],
  },
  {
    source: 'epic', id: 'GlimmerDeep', title: 'Glimmer Deep', hue: 'hsl(195 55% 30%)', dl: 45.0, disk: 60.0, platforms: ['windows'], developer: 'Abyssal', year: '2023', genres: ['Action'],
    dlc: [{ id: 'GlimmerDeepTrench', title: 'Glimmer Deep — The Trench', download_bytes: 3.2 * GB, installed: false }],
  },
  {
    source: 'epic', id: 'IronOrchard', title: 'Iron Orchard', hue: 'hsl(20 30% 28%)', dl: 52.0, disk: 70.0, platforms: ['windows'], not_supported: 'the EA app',
    developer: 'Forge North', year: '2022', genres: ['Shooter'],
  },
  {
    source: 'epic', id: 'NorthernStatic', title: 'Northern Static', hue: 'hsl(205 25% 36%)', dl: 38.0, disk: 50.0, platforms: ['windows'], not_supported: 'Ubisoft Connect',
    developer: 'Polar Line', year: '2021', genres: ['Open world'],
  },
  { source: 'epic', id: 'RustbeltRally', title: 'Rustbelt Rally', hue: 'hsl(35 50% 34%)', dl: 12.5, disk: 16.0, platforms: ['windows'], developer: 'Gravel Works', year: '2020', genres: ['Racing'] },
];

/**
 * The prototype's Amazon Games games (`stores.js` a1–a3), keyed by a made-up
 * product id (`amzn1.adg.product.<uuid>`, with dots, as nile's). Every Amazon
 * game is a Windows build. Versions are opaque version ids, as nile's
 * `installed.json` holds them: the UI never shows them.
 */
export const FAKE_AMAZON_VERSION = '4b1f0e2c-7a3d-4c55-9e10-fake-version-1';
export const FAKE_AMAZON_UPDATE = '9d2a6c31-0f4e-4b7a-8c21-fake-version-2';
const AMAZON_SEEDS: Seed[] = [
  {
    source: 'amazon', id: 'amzn1.adg.product.0f6c1d2e-f0a1-4a1e-9c3b-000000000a01', title: 'Fogline', hue: 'hsl(220 20% 34%)', installed: true, installed_platform: 'windows',
    version: FAKE_AMAZON_VERSION, dl: 5.5, disk: 7.0, platforms: ['windows'], developer: 'Mistral', year: '2020', genres: ['Adventure'],
  },
  {
    source: 'amazon', id: 'amzn1.adg.product.5a7e2b9c-e7a2-4d0f-8a61-000000000a02', title: 'Kettle Drum', hue: 'hsl(5 45% 36%)', dl: 2.2, disk: 3.0, platforms: ['windows'],
    developer: 'Brass Band', year: '2019', genres: ['Rhythm'],
  },
  {
    source: 'amazon', id: 'amzn1.adg.product.c3d84f10-10a3-47b2-b5e9-000000000a03', title: 'Lowsun', hue: 'hsl(45 55% 38%)', dl: 15.0, disk: 19.0, platforms: ['windows'],
    developer: 'Dusk Yard', year: '2021', genres: ['Survival'],
  },
];

/**
 * The prototype's Steam games (`stores.js` s1–s7), keyed by a made-up app id.
 * Moonlit Ferry was installed through GRID's Install in Steam; Deep Field
 * Radio and Old Brick Road were installed in Steam itself (outside GRID).
 * Steam's API gives no download size, so a game not installed has none.
 */
const STEAM_SEEDS: Seed[] = [
  { source: 'steam', id: '9100001', title: 'Moonlit Ferry', hue: 'hsl(235 35% 30%)', installed: true, origin: 'steam', dl: 0, disk: 9.1, platforms: [], developer: 'Tideglass', year: '2024' },
  { source: 'steam', id: '9100002', title: 'Deep Field Radio', hue: 'hsl(170 30% 28%)', installed: true, origin: 'steam_outside', dl: 0, disk: 4.0, platforms: [], developer: 'Static Choir', year: '2022' },
  { source: 'steam', id: '9100003', title: 'Old Brick Road', hue: 'hsl(15 35% 32%)', installed: true, origin: 'steam_outside', dl: 0, disk: 14.0, platforms: [], developer: 'Kiln', year: '2018' },
  { source: 'steam', id: '9100004', title: 'Quiet Comet', hue: 'hsl(265 30% 36%)', dl: 0, disk: 8.0, platforms: [], developer: 'Slow Orbit', year: '2025' },
  { source: 'steam', id: '9100005', title: 'Harbor of Bells', hue: 'hsl(200 40% 38%)', dl: 0, disk: 24.0, platforms: [], developer: 'Chime', year: '2023' },
  { source: 'steam', id: '9100006', title: 'Rime Assembly', hue: 'hsl(185 25% 40%)', dl: 0, disk: 6.0, platforms: [], developer: 'Frostbyte Co.', year: '2021' },
  { source: 'steam', id: '9100007', title: 'Signal Lost', hue: 'hsl(0 0% 26%)', dl: 0, disk: 35.0, platforms: [], developer: 'Dead Air', year: '2020' },
];

/** The store tool's name and the origin its installs get (Steam: "steam", its client's). */
function toolOf(source: FakeSource): 'gogdl' | 'legendary' | 'nile' | 'steam' {
  return source === 'epic' ? 'legendary' : source === 'amazon' ? 'nile' : source === 'steam' ? 'steam' : 'gogdl';
}

/** The store's name as the UI writes it (kept local: the fake imports no UI module). */
function nameOf(source: FakeSource): string {
  return source === 'epic' ? 'Epic Games' : source === 'amazon' ? 'Amazon Games' : source === 'steam' ? 'Steam' : 'GOG';
}

/** Where Steam keeps a game (the fake's library folder). */
function steamDir(title: string): string {
  return `~/.local/share/Steam/steamapps/common/${title.replace(/[^A-Za-z0-9]+/g, '')}`;
}

function folderOf(source: FakeSource, title: string): string {
  return `~/Games/GRID/stores/${source}/${title.replace(/[^A-Za-z0-9]+/g, '')}`;
}

/** A cover as an inline SVG: the prototype's coloured tile with the title. */
export function fakeCover(title: string, hue: string): string {
  const esc = title.replace(/&/g, '&amp;').replace(/</g, '&lt;');
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="300" height="400"><rect width="300" height="400" fill="${hue}"/><text x="150" y="210" font-family="system-ui,sans-serif" font-size="26" font-weight="600" fill="rgba(255,255,255,.85)" text-anchor="middle">${esc}</text></svg>`;
  return `data:image/svg+xml;utf8,${encodeURIComponent(svg)}`;
}

function game(s: Seed): StoreGame {
  const source = s.source ?? 'gog';
  const cloudy = fakeHasCloud(source);
  return {
    key: `${source}:${s.id}`,
    source,
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
    origin: s.installed ? (s.origin ?? toolOf(source)) : null,
    not_supported: s.not_supported ?? '',
    dlc: (s.dlc ?? []).map((d) => ({ ...d, download_bytes: Math.round(d.download_bytes) })),
    cloud: s.installed && cloudy ? (s.cloud ?? 'synced') : 'unknown',
    last_sync: s.installed && cloudy ? NOW() - 2 * 3600 : 0,
    install_dir: s.installed ? (source === 'steam' ? steamDir(s.title) : folderOf(source, s.title)) : '',
    compat_tool: s.installed_platform === 'windows' ? 'GE-Proton10-17' : '',
  };
}

export function baseStatus(source: FakeSource = 'gog'): StoreStatus {
  if (source === 'epic') return baseEpicStatus();
  if (source === 'amazon') return baseAmazonStatus();
  if (source === 'steam') return baseSteamStatus();
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
    steam: null,
    last_error: '',
  };
}

/** The Epic Games status: signed in as SamPlays, legendary installed (the pin's data, plan package C). */
export function baseEpicStatus(): StoreStatus {
  return {
    source: 'epic',
    connection: 'connected',
    username: FAKE_EPIC_USERNAME,
    tool: {
      name: 'legendary',
      version: '0.21.1',
      state: 'installed',
      size_bytes: Math.round(3.1 * MB),
      error: '',
      license: 'GPL-3.0',
      source_url: 'https://github.com/legendary-gl/legendary',
      done: 0,
      total: 0,
    },
    owned: 'ready',
    owned_error: '',
    owned_count: 6,
    installed_count: 2,
    installed_bytes: 0,
    refreshed_at: NOW() - 300,
    cloud_saves: true,
    install_dir: '~/Games/GRID/stores/epic',
    in_rail: true,
    steam: null,
    last_error: '',
  };
}

/**
 * The Amazon Games status: signed in as sam.plays, nile installed (the pin's
 * real data, plan package C: 1.2.0, the Linux x86_64 asset's 11 MB; the
 * prototype's "1.1.2 · 9 MB" are placeholders). No cloud saves.
 */
export function baseAmazonStatus(): StoreStatus {
  return {
    source: 'amazon',
    connection: 'connected',
    username: FAKE_AMAZON_USERNAME,
    tool: {
      name: 'nile',
      version: '1.2.0',
      state: 'installed',
      size_bytes: 11113264,
      error: '',
      license: 'GPL-3.0',
      source_url: 'https://github.com/imLinguin/nile',
      done: 0,
      total: 0,
    },
    owned: 'ready',
    owned_error: '',
    owned_count: 3,
    installed_count: 1,
    installed_bytes: 0,
    refreshed_at: NOW() - 3 * 86400,
    cloud_saves: false,
    install_dir: '~/Games/GRID/stores/amazon',
    in_rail: true,
    steam: null,
    last_error: '',
  };
}

/**
 * The Steam status (prototype `ST.stores.steam`): SteamID64 …0001 as SamPlays,
 * the key saved and working, the client at /usr/bin/steam, two local
 * accounts. No tool (an empty `ToolStatus`), no cloud saves, no install
 * folder of GRID's. The key itself is never in a status.
 */
export function baseSteamStatus(): StoreStatus {
  return {
    source: 'steam',
    connection: 'connected',
    username: FAKE_STEAM_PERSONA,
    tool: { name: '', version: '', state: 'not_installed', size_bytes: 0, error: '', license: '', source_url: '', done: 0, total: 0 },
    owned: 'ready',
    owned_error: '',
    owned_count: 7,
    installed_count: 3,
    installed_bytes: 0,
    refreshed_at: NOW() - 300,
    cloud_saves: false,
    install_dir: '',
    in_rail: true,
    steam: {
      client_path: FAKE_STEAM_CLIENT,
      steam_id: FAKE_STEAM_ID,
      key: 'ok',
      key_tail: '',
      accounts: [
        { persona: FAKE_STEAM_PERSONA, steam_id: FAKE_STEAM_ID, most_recent: true },
        { persona: 'couchguest', steam_id: FAKE_STEAM_GUEST_ID, most_recent: false },
      ],
    },
    last_error: '',
  };
}

/** The tool error on a computer older than nile's glibc floor: grid-core's `NILE_SYSTEM_TOO_OLD`, word for word (plan, spike 1). */
export const FAKE_NILE_GLIBC_ERROR = "nile can't run on this computer: it needs a newer Linux system (glibc 2.35 or later).";

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
  | 'needsGlibc'
  | 'expired'
  | 'notConnected'
  | 'notConnectedNothing'
  | 'ownedError'
  | 'update'
  | 'cloudConflict'
  // Steam's (prototype `stProtoSelect`'s Steam list).
  | 'nokey'
  | 'private'
  | 'badkey'
  | 'noclient';

export const FAKE_SCENARIOS: readonly FakeScenario[] = [
  'normal', 'loading', 'emptyInstalled', 'everythingInstalled', 'toolMissing', 'toolInstalling', 'toolFailed', 'needsPython', 'needsGlibc', 'expired', 'notConnected', 'notConnectedNothing',
  'ownedError', 'update', 'cloudConflict', 'nokey', 'private', 'badkey', 'noclient',
];

/** The SteamID64 form GRID checks before it asks Steam (17 digits, starting 7656). */
function steamIdForm(id: string): boolean {
  return /^7656[0-9]{13}$/.test(id.trim());
}

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
  return { path: '', kind, key: `gog:${id}`, title, cover: seed ? fakeCover(seed.title, seed.hue) : '', version, bytes: Math.round(gb * GB), verdict, free_bytes: 412 * GB };
}

function refused(kind: InstallerInspection['kind'], title: string, body: string): InstallerInspection {
  return { path: '', kind, key: null, title: '', cover: '', version: '', bytes: 0, verdict: { kind: 'refused', title, body }, free_bytes: null };
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

/** The store a command is for: its `source` argument, else its game key's store, else GOG. */
function sourceOf(args: Record<string, unknown>, g: StoreGame | null): FakeSource {
  const src = typeof args.source === 'string' ? args.source : (g?.source ?? (typeof args.key === 'string' ? args.key.split(':')[0] : ''));
  return asFakeSource(src);
}

/**
 * The fake backend. `emit` sends events to the app (the dev mock wires it to
 * Tauri's mocked event bus; tests pass a spy). `tick()` moves running jobs on
 * (the dev mock calls it on a timer; tests call it by hand).
 */
export class FakeStores {
  statuses: Record<FakeSource, StoreStatus> = { gog: baseStatus('gog'), epic: baseEpicStatus(), amazon: baseAmazonStatus(), steam: baseSteamStatus() };
  games: StoreGame[] = [...SEEDS.map(game), ...EPIC_SEEDS.map(game), ...AMAZON_SEEDS.map(game), ...STEAM_SEEDS.map(game)];
  /** The next Steam Test's answer, once (the prototype's "next Test result" button); `null` follows the key and SteamID64. */
  nextSteamTest: SteamTestResult | null = null;
  /** How long a Steam Test waits, so "Asking Steam…" shows (ms; tests set 0). */
  steamDelayMs = 700;
  entries: DownloadEntry[] = [];
  driveGames: DriveStoreGame[] = [];
  cloud: Record<string, StoreCloudState> = {};
  settings: Record<string, StoreGameSettings> = {};
  /** The next sign-in's outcome (the prototype's dashed buttons). */
  nextLogin: 'connected' | 'failed' | 'cancelled' = 'connected';
  emit: FakeEmit;
  /** Each store's games before a scenario that changes what is installed, so `normal` restores them. */
  private snapshots: Partial<Record<FakeSource, StoreGame[]>> = {};

  /** GOG's status (the fake's first store; most tests read only this one). */
  get status(): StoreStatus {
    return this.statuses.gog;
  }
  set status(next: StoreStatus) {
    this.statuses.gog = next;
  }

  constructor(emit: FakeEmit = () => {}) {
    this.emit = emit;
    // The prototype starts Amazon Games with its login expired (`ST.proto.amazon`).
    this.statuses.amazon.connection = 'expired';
    this.cloud['gog:1002'] = {
      state: 'conflict',
      synced_at: NOW() - 3 * 86400,
      locations: ['~/Games/GRID/stores/gog/Lanternfall/pfx/drive_c/users/steamuser/Saved Games/Lanternfall'],
      conflict: { local_at: NOW() - 3600, local_files: 3, local_bytes: Math.round(2.1 * MB), cloud_at: NOW() - 3 * 3600, cloud_files: 3, cloud_bytes: Math.round(2.0 * MB) },
    };
  }

  /** The prototype's boot: one GOG install running at 38%, one queued, and a failed Epic Games install. */
  bootJobs(): void {
    const cs = this.games.find((g) => g.store_id === '1004')!;
    const hv = this.games.find((g) => g.store_id === '1005')!;
    this.entries.push(
      entry({ kind: 'store_install', title: cs.title, source_id: cs.key, status: 'downloading', phase: 'downloading', total_bytes: cs.download_bytes, downloaded_bytes: Math.round(cs.download_bytes * 0.38), speed_bps: 41.2 * MB }),
      entry({ kind: 'store_install', title: hv.title, source_id: hv.key, status: 'queued' }),
    );
    // The prototype's failed Epic Games row (`stBoot`).
    const gd = this.games.find((g) => g.key === 'epic:GlimmerDeep')!;
    this.entries.push(
      entry({ kind: 'store_install', title: gd.title, source_id: gd.key, platform: 'Epic Games', status: 'failed', error: 'legendary stopped: Epic’s download server didn’t answer' }),
    );
    // The prototype's Steam install that Steam runs, at 52% (`stBoot`'s jboot3).
    const qc = this.games.find((g) => g.key === 'steam:9100004')!;
    this.entries.push(this.externalRow(qc, 0.52));
  }

  /** A Steam install row: no slot, "Managed by Steam" (`queue::admit_store_external`). */
  externalRow(g: StoreGame, pct = 0): DownloadEntry {
    const total = g.disk_bytes || 6 * GB;
    return entry({
      kind: 'store_external',
      title: g.title,
      source_id: g.key,
      platform: 'Steam',
      status: 'downloading',
      phase: 'none',
      total_bytes: total,
      downloaded_bytes: Math.round(total * pct),
    });
  }

  /** One store's games. */
  gamesOf(source: FakeSource): StoreGame[] {
    return this.games.filter((g) => g.source === source);
  }

  library(source: FakeSource = 'gog'): StoreLibrary {
    const status = this.statuses[source];
    const own = this.gamesOf(source);
    const installed = own.filter((g) => g.installed);
    status.installed_count = installed.length;
    status.installed_bytes = installed.reduce((t, g) => t + g.disk_bytes, 0);
    status.owned_count = own.length;
    const keyOk = source !== 'steam' || status.steam?.key === 'ok';
    const avail = status.owned === 'ready' && status.connection === 'connected' && keyOk ? own.filter((g) => !g.installed) : [];
    const steam = status.steam ? { ...status.steam, accounts: status.steam.accounts.map((a) => ({ ...a })) } : null;
    // Steam: its rows go on Disconnect, and the Library lists none while the client is missing.
    if (source === 'steam' && (status.connection === 'not_connected' || status.steam?.client_path === '')) {
      return { status: { ...status, steam, installed_count: 0, installed_bytes: 0 }, installed: [], available: avail };
    }
    return { status: { ...status, steam }, installed, available: avail };
  }

  /** Replaces one store's games, keeping the others. */
  private setGamesOf(source: FakeSource, list: StoreGame[]): void {
    this.games = [...this.games.filter((g) => g.source !== source), ...list];
  }

  /** Applies a prototype state to one store. The base data comes back first, then the state applies. */
  scenario(s: FakeScenario, source: FakeSource = 'gog'): void {
    this.statuses[source] = baseStatus(source);
    const saved = this.snapshots[source];
    if (saved) {
      this.setGamesOf(source, saved);
      delete this.snapshots[source];
    }
    for (const k of Object.keys(this.cloud)) if (k.startsWith('epic:') && source === 'epic') delete this.cloud[k];
    const keep = () => (this.snapshots[source] = this.gamesOf(source).map((g) => ({ ...g })));
    const map = (fn: (g: StoreGame) => StoreGame) => this.setGamesOf(source, this.gamesOf(source).map(fn));
    const tool = toolOf(source);
    const status = this.statuses[source];
    switch (s) {
      case 'loading':
        status.owned = 'loading';
        break;
      case 'emptyInstalled':
        keep();
        map((g) => ({ ...g, installed: false, origin: null, installed_platform: null }));
        break;
      case 'everythingInstalled':
        keep();
        map((g) => ({ ...g, installed: true, origin: g.origin ?? tool, installed_platform: g.installed_platform ?? g.platforms[0], version: g.version || '1.0' }));
        break;
      case 'toolMissing':
        status.tool = { ...status.tool, state: 'not_installed', version: '' };
        break;
      case 'toolInstalling':
        status.tool = { ...status.tool, state: 'installing', done: 400_000, total: status.tool.size_bytes };
        break;
      case 'toolFailed':
        status.tool = { ...status.tool, state: 'failed', error: 'the download didn’t match its checksum' };
        break;
      case 'needsPython':
        status.tool = { ...status.tool, state: 'failed', error: 'needs Python 3.10' };
        break;
      case 'needsGlibc':
        status.tool = { ...status.tool, state: 'failed', error: FAKE_NILE_GLIBC_ERROR };
        break;
      case 'expired':
        status.connection = 'expired';
        break;
      case 'notConnected':
        status.connection = 'not_connected';
        status.username = '';
        status.tool = { ...status.tool, state: 'not_installed' };
        if (status.steam) status.steam = { ...status.steam, steam_id: '', key: 'none' };
        break;
      case 'notConnectedNothing':
        keep();
        status.connection = 'not_connected';
        status.username = '';
        status.in_rail = false;
        map((g) => ({ ...g, installed: false }));
        break;
      case 'ownedError':
        status.owned = 'error';
        status.owned_error = `${nameOf(source)} didn’t answer. Check your connection and try again.`;
        break;
      case 'update': {
        keep();
        const first = this.gamesOf(source).find((g) => g.installed);
        const next = source === 'epic' ? '1.13' : source === 'amazon' ? FAKE_AMAZON_UPDATE : '2.2';
        if (first) map((g) => (g.key === first.key ? { ...g, update_version: next } : g));
        break;
      }
      case 'nokey':
        if (status.steam) status.steam.key = 'none';
        break;
      case 'private':
        if (status.steam) status.steam.key = 'private';
        break;
      case 'badkey':
        if (status.steam) status.steam.key = 'bad';
        break;
      case 'noclient':
        if (status.steam) status.steam.client_path = '';
        break;
      case 'cloudConflict': {
        if (!fakeHasCloud(source)) break;
        keep();
        const first = this.gamesOf(source).find((g) => g.installed && g.cloud !== 'none' && g.cloud !== 'off');
        if (first) {
          map((g) => (g.key === first.key ? { ...g, cloud: 'conflict' } : g));
          // Epic's list-saves gives no file list: the cloud side has no file count.
          this.cloud[first.key] = {
            state: 'conflict',
            synced_at: NOW() - 3 * 86400,
            locations: [`${first.install_dir}/prefix/drive_c/users/steamuser/Saved Games/${first.title}`],
            conflict: {
              local_at: NOW() - 3600,
              local_files: 3,
              local_bytes: Math.round(2.1 * MB),
              cloud_at: NOW() - 3 * 3600,
              cloud_files: source === 'epic' ? null : 3,
              cloud_bytes: Math.round(2.0 * MB),
            },
          };
        }
        break;
      }
      default:
        break;
    }
    status.in_rail = status.connection !== 'not_connected' || (source !== 'steam' && this.gamesOf(source).some((g) => g.installed));
    this.changed();
  }

  /** Emits `stores-changed` and the downloads snapshot. */
  changed(): void {
    this.emit('stores-changed', this.allStatuses());
    this.emitDownloads();
  }

  /** Every store's status, in the rail's order (Amazon Games, Epic Games, GOG). */
  allStatuses(): StoreStatus[] {
    return FAKE_SOURCES.map((src) => this.library(src).status);
  }

  emitDownloads(): void {
    this.emit('downloads-changed', { entries: [...this.entries].reverse() });
  }

  notice(message: string, opts: Partial<StoreNotice> = {}, source: FakeSource = 'gog'): void {
    this.emit('store-notice', { source, message, detail: '', error: false, action: null, ...opts });
  }

  private queue(
    kind: DownloadKind,
    g: StoreGame | null,
    title?: string,
    extra: Partial<DownloadEntry> = {},
    source: FakeSource = asFakeSource(g?.source),
  ): DownloadEntry {
    const busy = this.entries.some((e) => e.kind !== 'store_tool' && e.kind !== 'store_external' && (e.status === 'downloading' || e.status === 'installing'));
    const e = entry({
      kind,
      title: title ?? g?.title ?? '',
      source_id: g?.key ?? source,
      platform: nameOf(source),
      status: busy && kind !== 'store_tool' ? 'queued' : 'downloading',
      total_bytes: g?.download_bytes ?? this.statuses[source].tool.size_bytes,
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
      const step = e.kind === 'store_tool' ? 0.22 : e.kind === 'store_repair' ? 0.06 : e.kind === 'store_external' ? 0.003 : 0.02;
      if (e.status === 'downloading') {
        const total = e.total_bytes || GB;
        e.total_bytes = total;
        e.downloaded_bytes = Math.min(total, e.downloaded_bytes + Math.round(total * step));
        e.speed_bps = e.kind === 'store_external' ? 0 : 41.2 * MB;
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
    if (!this.entries.some((e) => e.kind !== 'store_tool' && e.kind !== 'store_external' && (e.status === 'downloading' || e.status === 'installing'))) {
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
      const src: FakeSource = asFakeSource(e.source_id);
      const version = baseStatus(src).tool.version;
      this.statuses[src].tool = { ...this.statuses[src].tool, state: 'installed', version };
      e.detail = version;
      this.notice(`${toolOf(src)} installed`, {}, src);
      this.changed();
      return;
    }
    if (!g) return;
    const src: FakeSource = asFakeSource(g.source);
    if (e.kind === 'store_external') {
      g.installed = true;
      g.origin = 'steam';
      g.install_dir = steamDir(g.title);
      e.detail = 'Installed by Steam';
      this.notice(`${g.title} installed`, { action: { kind: 'play', key: g.key } }, src);
      this.changed();
      return;
    }
    if (e.kind === 'store_update') {
      g.version = g.update_version;
      g.update_version = '';
      // Amazon's versions are opaque ids: "Fogline updated" (user decision 2026-11-13).
      this.notice(src === 'amazon' ? `${g.title} updated` : `${g.title} updated to ${g.version}`, {}, src);
    } else if (e.kind === 'store_repair') {
      // nile's verify gives no repaired-file count: the row reads "Verified".
      if (src === 'amazon') this.notice(`${g.title} verified`, {}, src);
      else {
        e.detail = '2 files repaired';
        this.notice(`${g.title}: 2 damaged files were downloaded again`, {}, src);
      }
    } else if (e.kind === 'store_dlc') {
      for (const d of g.dlc) if (e.title === d.title) d.installed = true;
      this.notice(`${e.title} installed`, {}, src);
    } else {
      g.installed = true;
      g.installed_platform = g.installed_platform ?? (g.platforms.includes('linux') ? 'linux' : 'windows');
      g.version = g.version || (src === 'amazon' ? FAKE_AMAZON_VERSION : '1.0');
      g.origin = e.kind === 'store_installer' ? 'offline' : e.kind === 'store_restore' ? 'drive' : toolOf(src);
      g.cloud = fakeHasCloud(src) ? 'synced' : 'unknown';
      g.last_sync = fakeHasCloud(src) ? NOW() : 0;
      g.install_dir = folderOf(src, g.title);
      g.compat_tool = g.installed_platform === 'windows' ? 'GE-Proton10-17' : '';
      this.notice(
        e.kind === 'store_installer' ? `${g.title} installed from its installer` : `${g.title} installed`,
        { action: { kind: 'play', key: g.key } },
        src,
      );
    }
    this.changed();
  }

  /** Answers one `invoke`; `NOT_HANDLED` for a command that isn't a store command. */
  handle(cmd: string, args: Record<string, unknown>): unknown {
    const key = typeof args.key === 'string' ? args.key : '';
    const g = this.games.find((x) => x.key === key) ?? null;
    const source: FakeSource = sourceOf(args, g);
    const status = this.statuses[source];
    switch (cmd) {
      case 'stores_status':
        return this.allStatuses();
      case 'store_library':
        return this.library(source);
      case 'store_refresh':
        status.owned = 'loading';
        this.changed();
        setTimeout(() => {
          const own = this.gamesOf(source);
          status.owned = 'ready';
          status.refreshed_at = NOW();
          this.notice(`${nameOf(source)}: ${own.length} games found · ${own.filter((x) => !x.installed).length} to install`, {}, source);
          this.changed();
        }, 1200);
        return null;
      case 'store_check_updates':
        return null;
      case 'store_connect':
        // The login window is the backend's own window; the fake finishes it at once.
        setTimeout(() => {
          const outcome = this.nextLogin;
          const user = source === 'epic' ? FAKE_EPIC_USERNAME : source === 'amazon' ? FAKE_AMAZON_USERNAME : FAKE_USERNAME;
          if (outcome === 'connected') {
            status.connection = 'connected';
            status.username = user;
            status.in_rail = true;
            if (status.tool.state !== 'installed') this.installTool(source);
          }
          this.emit('store-login', {
            source,
            outcome,
            username: outcome === 'connected' ? user : '',
            error: outcome === 'failed' ? `${nameOf(source)} returned an error. Try again.` : '',
          });
          this.changed();
        }, 600);
        return null;
      case 'store_cancel_login':
        return null;
      case 'store_disconnect':
        if (source === 'steam' && status.steam) {
          // GRID forgets the SteamID64 and the key; Steam's games leave the Library.
          status.steam = { ...status.steam, steam_id: '', key: 'none' };
          for (const e of this.entries) if (e.kind === 'store_external' && (e.status === 'downloading' || e.status === 'queued')) e.status = 'cancelled';
        }
        status.connection = 'not_connected';
        status.username = '';
        status.tool = { ...status.tool, state: 'not_installed' };
        status.in_rail = source !== 'steam' && this.gamesOf(source).some((x) => x.installed);
        this.changed();
        return null;
      case 'steam_test':
        return this.steamTest(String(args.steamId ?? ''), typeof args.key === 'string' ? args.key : null, false);
      case 'steam_save':
        return this.steamTest(String(args.steamId ?? ''), typeof args.key === 'string' ? args.key : null, true);
      case 'steam_remove_key':
        if (this.statuses.steam.steam) this.statuses.steam.steam.key = 'none';
        this.changed();
        return null;
      case 'steam_recheck_client':
        if (this.statuses.steam.steam) this.statuses.steam.steam.client_path = FAKE_STEAM_CLIENT;
        this.changed();
        return null;
      case 'steam_open':
        this.steamOpen(g, String(args.action ?? ''));
        return null;
      case 'store_install_tool':
        this.installTool(source);
        return null;
      case 'store_set_install_dir':
        status.install_dir = String(args.path ?? '');
        this.changed();
        return null;
      case 'store_set_cloud_saves':
        status.cloud_saves = Boolean(args.on);
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
      case 'store_game_executables': {
        // The prototype's two choices (`stSettingsHTML`).
        const dir = g?.install_dir ?? '';
        const name = (g?.title ?? '').replace(/[^A-Za-z]+/g, '');
        return g?.installed_platform === 'windows'
          ? [`${dir}/${name}.exe`, `${dir}/Launcher.exe`]
          : [`${dir}/start.sh`, `${dir}/game/${name.toLowerCase()}`];
      }
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
      case 'store_free_space':
        return 412 * GB;
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

  /** `steam_test` / `steam_save`: the SteamID64's form, then the key (the fixture marker only), then Steam's answer. */
  private async steamTest(steamId: string, key: string | null, save: boolean): Promise<SteamTestResult> {
    const status = this.statuses.steam;
    const card = status.steam!;
    if (!steamIdForm(steamId)) return { kind: 'bad_steam_id' };
    if (key === null && card.key === 'none') throw new Error('Paste your key first.');
    if (this.steamDelayMs > 0) await new Promise((r) => setTimeout(r, this.steamDelayMs));
    let result: SteamTestResult;
    if (this.nextSteamTest !== null) {
      result = this.nextSteamTest;
      this.nextSteamTest = null;
    } else if (key !== null && key !== FAKE_STEAM_KEY) result = { kind: 'bad_key', status: 401 };
    else if (steamId === FAKE_STEAM_GUEST_ID) result = { kind: 'private' };
    else result = { kind: 'ok', owned_count: this.gamesOf('steam').length };
    if (save && (result.kind === 'ok' || result.kind === 'private')) {
      const persona = card.accounts.find((a) => a.steam_id === steamId)?.persona ?? FAKE_STEAM_PERSONA;
      status.connection = 'connected';
      status.username = persona;
      status.in_rail = true;
      status.steam = { ...card, steam_id: steamId, key: result.kind === 'ok' ? 'ok' : 'private' };
      this.notice(
        result.kind === 'ok'
          ? `Steam connected · ${result.owned_count} games owned`
          : 'Steam connected, but your game details are private, so Available is empty',
        { error: result.kind !== 'ok' },
        'steam',
      );
      this.changed();
    }
    return result;
  }

  /** `steam_open`: install admits a Steam row the client "runs"; uninstall removes the game a moment later; validate only opens Steam. */
  private steamOpen(g: StoreGame | null, action: string): void {
    if (!g || g.source !== 'steam') throw new Error('That isn’t a Steam game.');
    if (this.statuses.steam.steam?.client_path === '') throw new Error(FAKE_STEAM_NOT_INSTALLED);
    if (action === 'install') {
      if (this.entries.some((e) => e.source_id === g.key && e.kind === 'store_external' && e.status === 'downloading')) return;
      this.entries.push(this.externalRow(g));
      this.emitDownloads();
    } else if (action === 'uninstall') {
      setTimeout(() => {
        g.installed = false;
        g.origin = null;
        this.notice(`${g.title} uninstalled`, {}, 'steam');
        this.changed();
      }, 2500);
    }
  }

  private installTool(source: FakeSource = 'gog'): void {
    const status = this.statuses[source];
    status.tool = { ...status.tool, state: 'installing', done: 0, total: status.tool.size_bytes };
    this.queue('store_tool', null, toolOf(source), { source_id: source }, source);
    this.notice(`Installing ${toolOf(source)} (${Math.max(1, Math.round(status.tool.size_bytes / MB))} MB) for ${nameOf(source)}…`, {}, source);
    this.changed();
  }

  installOptions(g: StoreGame | null): StoreInstallOptions {
    if (g?.source === 'epic' || g?.source === 'amazon') {
      // legendary and nile install the Windows build only, in one language:
      // no Version, no Language; nile has no DLC.
      return {
        versions: [{ platform: 'windows', recommended: true, download_bytes: g.download_bytes, disk_bytes: g.disk_bytes }],
        languages: [],
        default_language: '',
        dlc: g.dlc.map((d) => ({ ...d })),
        install_dir: folderOf(g.source, g.title),
        free_bytes: 412 * GB,
        volume: '/home',
        compat_tool: 'GE-Proton10-17',
      };
    }
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
      executable: win ? `${g?.install_dir ?? ''}/${name}.exe` : `${g?.install_dir ?? ''}/start.sh`,
      exe_user_pinned: false,
      launch_parameters: '',
      compat_tool: win ? 'GE-Proton10-17' : '',
      wineprefix: win ? `${g?.install_dir}/${g?.source === 'epic' || g?.source === 'amazon' ? 'prefix' : 'pfx'}` : '',
      cloud_saves: g?.source === 'amazon' ? false : g?.cloud !== 'off',
    };
  }

  private cloudState(g: StoreGame | null): StoreCloudState {
    if (g && this.cloud[g.key]) return this.cloud[g.key];
    // Amazon Games has no cloud saves (the backend answers `off`).
    if (g?.source === 'amazon') return { state: 'off', synced_at: 0, locations: [], conflict: null };
    if (g?.source === 'epic') {
      const state = g.cloud === 'off' ? 'off' : g.cloud === 'none' ? 'none' : 'synced';
      return { state, synced_at: g.last_sync, locations: state === 'none' ? [] : [`${g.install_dir}/prefix/drive_c/users/steamuser/Saved Games/${g.title}`], conflict: null };
    }
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
