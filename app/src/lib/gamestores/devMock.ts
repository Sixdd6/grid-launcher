// DEV ONLY (VITE_FAKE_STORES=1 `npm run dev`): runs the desktop UI in a
// plain browser with Tauri's IPC mocked, the store commands answered by
// `FakeStores` and everything else by a small offline stand-in, so every
// GameStores state can be clicked through and screenshot without a backend.
// `main.ts` imports this only when the flag is set; a production build never
// contains it. `window.__gridFake` exposes the fake and its scenarios.
import { emit } from '@tauri-apps/api/event';
import { mockIPC, mockWindows } from '@tauri-apps/api/mocks';
import type { DownloadEntry, DriveInfo, InstalledGame } from '../api';
import { FAKE_INSTALLERS, FakeStores, NOT_HANDLED, type FakeScenario, type FakeSource } from './fakeApi';

const GB = 1024 ** 3;
const now = () => Math.floor(Date.now() / 1000);

const UI = {
  theme: 'dark',
  background_fade: 0.6,
  background_blur: 20,
  card_size_library: 'medium',
  card_size_server: 'medium',
  card_labels: 'always',
  card_display: 'rows',
  hardware_acceleration: true,
  open_on: 'library',
  tv_prompts: 'match',
  tv_assignments: {},
  tv_welcomed: true,
  video_volume: 50,
};

function installedRow(title: string, source: 'gog' | 'epic' | 'amazon' | 'romm', storeId: string, platform: string, played: number): InstalledGame {
  return {
    title,
    platform,
    source,
    store_id: storeId,
    platform_slug: source === 'gog' ? 'gog' : source === 'epic' || source === 'amazon' ? 'win' : platform.toLowerCase(),
    rom_id: source === 'romm' ? Number(storeId) : null,
    rom_file_name: '',
    archive_path: '',
    extracted_path: '',
    extracted_dir: '',
    multi_file_game_dir: '',
    description: '',
    rating: '',
    genres: '',
    regions: '',
    languages: '',
    tags: '',
    revision: '',
    companies: '',
    first_release_date: '',
    filesize_bytes: 0,
    last_played_at: played,
  } as unknown as InstalledGame;
}

const DRIVE: DriveInfo = {
  key: '/run/media/player/T7',
  mount: '/run/media/player/T7',
  label: 'SAMSUNG T7',
  fs: 'exFAT',
  fat32: false,
  total_bytes: 1000 * GB,
  free_bytes: 230 * GB,
  read_only: false,
  status: 'ready',
  problem: '',
  pack: { name: 'Couch kit', os: ['linux'], game_count: 2, gridpack_bytes: 120 * GB, emulators: [], firmware: [], updated: '2026-11-12T10:00:00Z', rom_ids: [] },
} as unknown as DriveInfo;

export function installDevMock(): void {
  mockWindows('main');
  const fake = new FakeStores((event, payload) => {
    void emit(event, payload);
  });
  fake.bootJobs();
  fake.driveGames = [
    { key: 'gog:1006', name: 'Marrowind Keep', platform: 'linux', layout: 'installer', version: '1.2', size: 22 * GB, cover: fake.games[5].cover, installed: false, added: '2026-11-12', summary: '' },
    { key: 'gog:1007', name: 'Paper Lanterns', platform: 'linux', layout: 'installed', version: '1.1.0', size: 1.6 * GB, cover: fake.games[6].cover, installed: false, added: '2026-11-12', summary: '' },
  ];
  const rommRows: DownloadEntry[] = [];
  const installed = (): InstalledGame[] => [
    ...fake
      .gamesOf('gog')
      .filter((g) => g.installed)
      .map((g, i) => installedRow(g.title, 'gog', g.store_id, 'GOG', i === 0 ? now() - 2 * 86400 : i === 1 ? now() - 86400 : now() - 21 * 86400)),
    // Starlight Courier was played 4 days ago, Vector Bloom never (the prototype's e1, e2).
    ...fake
      .gamesOf('epic')
      .filter((g) => g.installed)
      .map((g) => installedRow(g.title, 'epic', g.store_id, 'Epic Games', g.store_id === 'Starlight' ? now() - 4 * 86400 : 0)),
    // Fogline was played last month (the prototype's a1).
    ...fake
      .gamesOf('amazon')
      .filter((g) => g.installed)
      .map((g) => installedRow(g.title, 'amazon', g.store_id, 'Amazon Games', g.title === 'Fogline' ? now() - 30 * 86400 : 0)),
    installedRow('Velvet Static', 'romm', '11', 'PlayStation', now() - 3600),
    installedRow('Cinder Peaks', 'romm', '12', 'Super Nintendo', 0),
  ];
  const handlers: Record<string, (args: Record<string, unknown>) => unknown> = {
    restore_session: () => ({ kind: 'offline', server_url: 'https://romm.example.test', username: 'player' }),
    get_ui_mode: () => ({ mode: 'desktop', started_with_tv: false, fullscreen: false }),
    get_ui_settings: () => UI,
    list_installed: () => installed(),
    list_downloads: () => ({ entries: [...fake.entries, ...rommRows].reverse() }),
    list_sessions: () => ({ sessions: [], warning: null }),
    list_updates: () => [],
    list_compat_tools: () => ({ tools: [{ name: 'GE-Proton10-17', kind: 'proton', path: '/x', source: 'managed' }], default_tool: 'GE-Proton10-17', umu_run: { source: 'system', path: '/usr/bin/umu-run', release_tag: '', source_id: '' } }),
    list_drives: () => ({ connected: [DRIVE], remembered: [] }),
    drive_games: () => [],
    library_move_status: () => ({ phase: null, running: false, old_root: '', new_root: '', resumed: false, error: null }),
    library_unreachable_status: () => ({ status: 'reachable' }),
    python_import_notice: () => null,
    layout_migration_notice: () => null,
    get_launch_defaults: () => ({ default_emulators: {}, retroarch_cores: {}, launch_args: '' }),
    get_backup_settings: () => ({ include_firmware: true }),
    app_update_status: () => null,
    // The fake's covers are inline SVGs: "cached" as themselves.
    ensure_image: (args) => String(args.url ?? ''),
  };
  mockIPC(
    (cmd, payload) => {
      const args = (payload ?? {}) as Record<string, unknown>;
      const answer = fake.handle(cmd, args);
      if (answer !== NOT_HANDLED) return answer;
      if (cmd === 'launch_game_key') {
        const key = String(args.key);
        if (fake.cloud[key]?.state === 'conflict') {
          throw { code: 'store_cloud_conflict', message: 'These saves changed in two places' };
        }
        return { id: 1, key, rom_id: null, title: fake.games.find((g) => g.key === key)?.title ?? '', emulator_name: '', started_at: now(), pid: 1 };
      }
      if (cmd === 'cancel_install' || cmd === 'dismiss_download' || cmd === 'retry_install') {
        const id = Number(args.entryId);
        const e = fake.entries.find((x) => x.id === id);
        if (e && cmd === 'cancel_install') e.status = 'cancelled';
        if (e && cmd === 'retry_install') e.status = 'queued';
        if (cmd === 'dismiss_download') {
          fake.entries = fake.entries.filter((x) => x.id !== id);
          rommRows.splice(0, rommRows.length, ...rommRows.filter((x) => x.id !== id));
        }
        fake.emitDownloads();
        return null;
      }
      const h = handlers[cmd];
      return h ? h(args) : null;
    },
    { shouldMockEvents: true },
  );
  setInterval(() => fake.tick(), 700);
  // RomM installer-archive rows for the Downloads states.
  rommRows.push(
    fake.rommInstallerRow('Grey Meridian', 'Windows', 'installer_silent', 0.4),
    fake.failedInstallerRow('Iron Lullaby', 'Windows', { code: 'installer_unrecognised', base_title: '', base_gog_id: '', kept: false },
      'Its archive holds an installer GRID doesn’t recognise (setup.exe has no GOG game info). The download was removed.'),
    fake.failedInstallerRow('Low Tide Radio', 'Windows', { code: 'installer_dlc_base_missing', base_title: 'Pale Summit', base_gog_id: '', kept: false }, ''),
  );
  const origEmit = fake.emitDownloads.bind(fake);
  fake.emitDownloads = () => {
    void emit('downloads-changed', { entries: [...fake.entries, ...rommRows].reverse() });
    void origEmit;
  };
  (window as unknown as { __gridFake: unknown }).__gridFake = {
    fake,
    scenario: (s: FakeScenario, source: FakeSource = 'gog') => fake.scenario(s, source),
    installers: FAKE_INSTALLERS.map((f) => f.path),
  };
}
