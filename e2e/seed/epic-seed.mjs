#!/usr/bin/env node
/**
 * Seeds an `epic*` stage group's data dir before each spec (scripts/e2e.sh
 * runs `node epic-seed.mjs <data-dir> <spec name>`). Every spec starts from
 * nothing, on a profile used without a server (`offline = true`), then:
 *
 * - `epic-login`, `epic-secrets`: Epic not connected, no store files (the
 *   specs sign in through the login window).
 * - every other Epic spec (package D's `epic-library`, `epic-cloud`, package
 *   E's `tv-epic`): already signed in to Epic as SamPlays: `[stores.epic]`
 *   in config.toml, legendary's own files in `stores/legendary/` (its
 *   `user.json` holds FAKE-STORE-TOKEN-not-real, the store-token
 *   exception; `installed.json`), and two installed Epic games in the
 *   registry: Starlight Courier (1.12, cloud saves on, a save in its Wine
 *   prefix newer than its last sync) and Vector Bloom (3.0.1, no cloud
 *   saves; the owned list has 3.0.2, an update). The owned list is the
 *   prototype's six games (e2e/fixtures-epic/owned.json, served by the fake
 *   legendary).
 *   `epic-cloud` (`--cloud-conflict`): the fake legendary's cloud holds a
 *   Starlight Courier save newer than its last sync, so its next sync is a
 *   conflict (`E2E_EPIC_CONFLICT_KEY`). Otherwise the cloud save is the one
 *   last synced.
 *
 * Always: `stubs/bin` with `umu-run` and `wine` stand-ins that write their
 * argv (one argument per line, a `---` line after each run) to
 * `<data>/epic-game-argv.log` (`E2E_EPIC_ARGV_FILE`), so a spec sees the
 * launch ticket reach the game. Under `epic-secrets` the game exits at once
 * with code 3 (the early-exit warning, which must show `<redacted>`);
 * elsewhere it runs for two seconds and exits 0.
 */
import { chmodSync, mkdirSync, rmSync, utimesSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { writeRegistry } from './registry-schema.mjs';

const dataDir = process.argv[2];
const spec = process.argv[3] ?? '';
if (!dataDir) {
  console.error('usage: epic-seed.mjs <data-dir> [spec]');
  process.exit(1);
}

export const FAKE_TOKEN = 'FAKE-STORE-TOKEN-not-real';
const ACCOUNT_ID = '7f3e9c2a1b4d4e5f8a6b0c1d2e3f4a5b';

rmSync(dataDir, { recursive: true, force: true });
const library = path.join(dataDir, 'library');
mkdirSync(library, { recursive: true });
const tomlString = (s) => JSON.stringify(s);
const sql = (s) => `'${String(s).replace(/'/g, "''")}'`;

// The compat tool stand-ins: they record the argv the game got.
const stubs = path.join(dataDir, 'stubs', 'bin');
mkdirSync(stubs, { recursive: true });
const argvFile = path.join(dataDir, 'epic-game-argv.log');
const tail = spec === 'epic-secrets' ? 'exit 3' : 'sleep 2\nexit 0';
for (const name of ['umu-run', 'wine']) {
  const file = path.join(stubs, name);
  writeFileSync(
    file,
    `#!/bin/sh\n# e2e stand-in for ${name}: records the game's argv.\nfor a in "$@"; do printf '%s\\n' "$a" >> ${JSON.stringify(argvFile)}; done\nprintf -- '---\\n' >> ${JSON.stringify(argvFile)}\n${tail}\n`,
  );
  chmodSync(file, 0o755);
}

const signedOut = spec === 'epic-login' || spec === 'epic-secrets';
if (signedOut) {
  writeFileSync(
    path.join(dataDir, 'config.toml'),
    `schema_version = 1
library_path = ${tomlString(library)}
library_layout_version = 2
offline = true
`,
  );
  process.exit(0);
}

writeFileSync(
  path.join(dataDir, 'config.toml'),
  `schema_version = 1
library_path = ${tomlString(library)}
library_layout_version = 2
offline = true

[stores.epic]
connected = true
username = "SamPlays"
install_dir = ""
cloud_saves = true
`,
);

const now = Math.floor(Date.now() / 1000);
const lastSync = now - 7 * 86400;
const legendary = path.join(dataDir, 'stores', 'legendary');
mkdirSync(legendary, { recursive: true, mode: 0o700 });
chmodSync(path.join(dataDir, 'stores'), 0o700);
chmodSync(legendary, 0o700);
const privateFile = (file, text) => {
  mkdirSync(path.dirname(file), { recursive: true });
  writeFileSync(file, text);
  chmodSync(file, 0o600);
};
privateFile(
  path.join(legendary, 'user.json'),
  JSON.stringify({
    access_token: FAKE_TOKEN,
    refresh_token: FAKE_TOKEN,
    account_id: ACCOUNT_ID,
    displayName: 'SamPlays',
    expires_at: '2099-01-01T00:00:00.000Z',
    token_type: 'bearer',
  }),
);
privateFile(path.join(legendary, 'config.ini'), '[Legendary]\ndisable_update_check = false\ndisable_update_notice = true\n');

const epicDir = path.join(library, 'Epic Games');

// Starlight Courier: cloud saves on, a local save changed since the last sync.
const star = path.join(epicDir, 'Starlight Courier');
const starExe = path.join(star, 'Binaries', 'Win64', 'Starlight.exe');
mkdirSync(path.dirname(starExe), { recursive: true });
writeFileSync(starExe, 'MZ fake epic game\n');
const starPrefix = path.join(star, 'prefix');
const starSaves = path.join(starPrefix, 'drive_c', 'users', 'steamuser', 'AppData', 'Local', 'StarlightCourier', 'Saved', 'SaveGames');
mkdirSync(starSaves, { recursive: true });
const localSave = path.join(starSaves, 'slot1.sav');
writeFileSync(localSave, 'local save');
utimesSync(localSave, now - 60, now - 60);

// Vector Bloom: no cloud saves, an update waiting.
const vector = path.join(epicDir, 'Vector Bloom');
const vectorExe = path.join(vector, 'VectorBloom.exe');
mkdirSync(path.join(vector, 'prefix'), { recursive: true });
writeFileSync(vectorExe, 'MZ fake epic game\n');

const installedEntry = (e) => ({
  app_name: e.app,
  base_urls: [],
  can_run_offline: e.offline,
  egl_guid: '',
  executable: e.exe,
  install_path: e.dir,
  install_size: e.size,
  install_tags: [],
  is_dlc: false,
  launch_parameters: '',
  manifest_path: null,
  needs_verification: false,
  platform: 'Windows',
  prereq_info: null,
  requires_ot: false,
  save_path: e.save ?? null,
  title: e.title,
  uninstaller: null,
  version: e.version,
  is_preloaded: false,
});
privateFile(
  path.join(legendary, 'installed.json'),
  JSON.stringify({
    Starlight: installedEntry({ app: 'Starlight', title: 'Starlight Courier', exe: 'Binaries/Win64/Starlight.exe', dir: star, size: 31200000, version: '1.12', offline: true, save: starSaves }),
    Vector: installedEntry({ app: 'Vector', title: 'Vector Bloom', exe: 'VectorBloom.exe', dir: vector, size: 11000000, version: '3.0.1', offline: false }),
  }),
);

// The fake legendary's cloud (fake-tools/legendary.mjs): the save last
// synced, or (epic-cloud) a newer one.
const manifestName = (seconds) => {
  const d = new Date(seconds * 1000);
  const p = (n) => String(n).padStart(2, '0');
  return `${d.getUTCFullYear()}.${p(d.getUTCMonth() + 1)}.${p(d.getUTCDate())}-${p(d.getUTCHours())}.${p(d.getUTCMinutes())}.${p(d.getUTCSeconds())}.manifest`;
};
const cloud = path.join(legendary, 'fake-cloud', 'Starlight');
const conflict = spec === 'epic-cloud';
privateFile(path.join(cloud, 'files', 'slot1.sav'), conflict ? 'cloud save, newer' : 'local save, as last synced');
privateFile(path.join(cloud, 'manifests.json'), JSON.stringify([manifestName(conflict ? now - 30 : lastSync)]));

const meta = (m) =>
  JSON.stringify({
    app_name: '',
    version: '',
    platform: 'Windows',
    dlc: [],
    cloud_saves: true,
    cloud_save_folder: '',
    save_path: '',
    last_sync: 0,
    can_run_offline: false,
    exe_user_pinned: false,
    ...m,
  });

const row = (g) => `(${sql(g.title)}, 'Epic Games', ${sql(`${g.title.toLowerCase()}\u001fepic:${g.id}`)}, 'epic games', 'win',
   ${sql(g.dir)}, ${sql(g.dir)}, ${sql(g.exe)}, ${sql(g.workdir)}, 'sidecar', 'GE-Proton9-20', ${sql(g.prefix)}, ${sql(g.version)},
   ${g.size}, ${now - 86400}, 'epic', ${sql(g.id)}, ${sql(g.meta)})`;

writeRegistry(
  path.join(dataDir, 'grid-launcher.db'),
  `
ALTER TABLE installed_games ADD COLUMN source TEXT NOT NULL DEFAULT 'romm';
ALTER TABLE installed_games ADD COLUMN store_id TEXT NOT NULL DEFAULT '';
ALTER TABLE installed_games ADD COLUMN store_meta TEXT NOT NULL DEFAULT '';
CREATE UNIQUE INDEX IF NOT EXISTS installed_games_store_identity
    ON installed_games (source, store_id) WHERE source <> 'romm';
PRAGMA user_version = 11;
INSERT INTO installed_games
  (title, platform, title_key, platform_key, platform_slug,
   extracted_dir, native_game_dir, native_executable_path, native_working_dir, native_launch_source, native_compat_tool,
   native_wineprefix, revision, filesize_bytes, installed_at, source, store_id, store_meta)
VALUES
  ${row({
    title: 'Starlight Courier',
    id: 'Starlight',
    dir: star,
    exe: starExe,
    workdir: 'Binaries/Win64',
    prefix: starPrefix,
    version: '1.12',
    size: 31200000,
    meta: meta({
      app_name: 'Starlight',
      version: '1.12',
      cloud_save_folder: '{AppData}/StarlightCourier/Saved/SaveGames',
      save_path: starSaves,
      last_sync: lastSync,
      can_run_offline: true,
    }),
  })},
  ${row({
    title: 'Vector Bloom',
    id: 'Vector',
    dir: vector,
    exe: vectorExe,
    workdir: '',
    prefix: path.join(vector, 'prefix'),
    version: '3.0.1',
    size: 11000000,
    meta: meta({ app_name: 'Vector', version: '3.0.1' }),
  })};
`,
);
