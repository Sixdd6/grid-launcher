#!/usr/bin/env node
/**
 * Seeds a `gog*` stage group's data dir before each spec (scripts/e2e.sh
 * runs `node gog-seed.mjs <data-dir> <spec name>`). Every spec starts from
 * nothing, then:
 *
 * - `gog-login`, `gog-secrets`: a profile used without a server, GOG not
 *   connected, no store files (the specs sign in through the login window).
 * - every other GOG spec (package E's `gog-library`, `gog-cloud`,
 *   `gog-offline`, `gog-romm-installer`, `gog-backups`): a fresh profile
 *   whose connect form the spec fills (the mock RomM), already signed in to
 *   GOG as `sam_plays`: `[stores.gog]` in config.toml, gogdl's token file
 *   (`FAKE-STORE-TOKEN-not-real`, the store-token exception) and two
 *   installed GOG games in the registry: Ashen Tide (Linux, from gogdl) and
 *   Grey Meridian (Windows, cloud saves on, a local save newer than its last
 *   sync; the mock GOG holds a newer cloud save too, so launching it
 *   conflicts: `E2E_GOG_CONFLICT_KEY`). Lanternfall stays available.
 *
 * Always: `stubs/bin` with `wine` and `umu-run` stand-ins that run the fake
 * GOG installer (e2e/fake-tools/installer.mjs), and `installers/` with a
 * GOG Linux `.sh` (`E2E_GOG_INSTALLER_SH`) and a macOS `.pkg`
 * (`E2E_GOG_INSTALLER_PKG`).
 */
import { chmodSync, mkdirSync, rmSync, utimesSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { buildLinuxSh, buildMacPkg } from '../fake-tools/gog-installers.mjs';
import { writeRegistry } from './registry-schema.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const dataDir = process.argv[2];
const spec = process.argv[3] ?? '';
if (!dataDir) {
  console.error('usage: gog-seed.mjs <data-dir> [spec]');
  process.exit(1);
}

export const FAKE_TOKEN = 'FAKE-STORE-TOKEN-not-real';
const USER_ID = '48628349957132247';
const GALAXY_CLIENT = '46899977096215655';

rmSync(dataDir, { recursive: true, force: true });
const library = path.join(dataDir, 'library');
mkdirSync(library, { recursive: true });
const tomlString = (s) => JSON.stringify(s);
const sql = (s) => `'${String(s).replace(/'/g, "''")}'`;

// The wine / umu-run stand-ins and the installer files.
const stubs = path.join(dataDir, 'stubs', 'bin');
mkdirSync(stubs, { recursive: true });
const installer = path.join(here, '..', 'fake-tools', 'installer.mjs');
for (const name of ['wine', 'umu-run']) {
  const file = path.join(stubs, name);
  writeFileSync(file, `#!/bin/sh\nexec node ${JSON.stringify(installer)} "$@"\n`);
  chmodSync(file, 0o755);
}
const installers = path.join(dataDir, 'installers');
mkdirSync(installers, { recursive: true });
writeFileSync(path.join(installers, 'wren_hollow_1_0_3_12345.sh'), buildLinuxSh());
writeFileSync(path.join(installers, 'quarry_nine_1_2_0_54321.pkg'), buildMacPkg());

const signedOut = spec === 'gog-login' || spec === 'gog-secrets';
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

// Signed in to GOG; the spec connects to the mock RomM itself.
writeFileSync(
  path.join(dataDir, 'config.toml'),
  `schema_version = 1
library_path = ${tomlString(library)}
library_layout_version = 2

[stores.gog]
connected = true
username = "sam_plays"
`,
);

const gogdl = path.join(dataDir, 'stores', 'gogdl');
mkdirSync(gogdl, { recursive: true, mode: 0o700 });
chmodSync(gogdl, 0o700);
const authFile = path.join(gogdl, 'auth.json');
writeFileSync(
  authFile,
  JSON.stringify({
    [GALAXY_CLIENT]: {
      access_token: FAKE_TOKEN,
      refresh_token: FAKE_TOKEN,
      user_id: USER_ID,
      expires_in: 3600,
      token_type: 'bearer',
      session_id: '1',
      loginTime: Date.now() / 1000,
    },
  }),
);
chmodSync(authFile, 0o600);

const gogDir = path.join(library, 'GOG');
const now = Math.floor(Date.now() / 1000);

// Ashen Tide, Linux, from gogdl.
const ashen = path.join(gogDir, 'Ashen Tide');
mkdirSync(path.join(ashen, 'saves'), { recursive: true });
writeFileSync(path.join(ashen, 'start.sh'), '#!/bin/sh\nexit 0\n');
chmodSync(path.join(ashen, 'start.sh'), 0o755);
writeFileSync(path.join(ashen, 'gameinfo'), 'Ashen Tide\n1.4.2\n');
writeFileSync(
  path.join(ashen, '.gogdl-linux-manifest'),
  JSON.stringify({ buildId: '56012345678901234', platform: 'linux', versionName: '1.4.2', clientId: '50225266424144145' }),
);

// Grey Meridian, Windows, cloud saves on, a save changed since the last sync.
const grey = path.join(gogDir, 'Grey Meridian');
mkdirSync(path.join(grey, 'saves'), { recursive: true });
mkdirSync(path.join(grey, 'prefix'), { recursive: true });
writeFileSync(path.join(grey, 'game.exe'), 'MZ fake');
writeFileSync(
  path.join(grey, 'goggame-1580232252.info'),
  JSON.stringify({
    gameId: '1580232252',
    rootGameId: '1580232252',
    name: 'Grey Meridian',
    buildId: '56012345678900001',
    clientId: '52095557858325425',
    playTasks: [{ isPrimary: true, type: 'FileTask', category: 'game', path: 'game.exe' }],
  }),
);
const save = path.join(grey, 'saves', 'slot1.sav');
writeFileSync(save, 'local save');
utimesSync(save, now - 60, now - 60);
const manifests = path.join(gogdl, 'heroic_gogdl', 'manifests');
mkdirSync(manifests, { recursive: true });
writeFileSync(
  path.join(manifests, '1580232252'),
  JSON.stringify({
    version: 2,
    buildId: '56012345678900001',
    platform: 'windows',
    clientId: '52095557858325425',
    clientSecret: 'fake-client-secret',
    versionName: '1.0',
  }),
);

const meta = (m) =>
  JSON.stringify({
    origin: 'gogdl',
    version: '',
    build_id: '',
    language: 'en-US',
    installer: [],
    installer_bytes: 0,
    dlc: [],
    cloud_saves: true,
    save_locations: [],
    last_sync: 0,
    exe_user_pinned: false,
    ...m,
  });

const row = (g) => `(${sql(g.title)}, 'GOG', ${sql(`${g.title.toLowerCase()}\u001fgog:${g.id}`)}, 'gog', ${sql(g.slug)},
   ${sql(g.dir)}, ${sql(g.dir)}, ${sql(g.exe)}, 'sidecar', ${sql(g.compat)}, ${sql(g.prefix)}, ${sql(g.version)},
   ${g.size}, ${now - 86400}, 'gog', ${sql(g.id)}, ${sql(g.meta)})`;

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
   extracted_dir, native_game_dir, native_executable_path, native_launch_source, native_compat_tool,
   native_wineprefix, revision, filesize_bytes, installed_at, source, store_id, store_meta)
VALUES
  ${row({
    title: 'Ashen Tide',
    id: '1207664643',
    slug: 'linux',
    dir: ashen,
    exe: path.join(ashen, 'start.sh'),
    compat: '',
    prefix: '',
    version: '1.4.2',
    size: 2097152,
    meta: meta({ platform: 'linux', version: '1.4.2', build_id: '56012345678901234' }),
  })},
  ${row({
    title: 'Grey Meridian',
    id: '1580232252',
    slug: 'win',
    dir: grey,
    exe: path.join(grey, 'game.exe'),
    compat: 'GE-Proton9-20',
    prefix: path.join(grey, 'prefix'),
    version: '1.0',
    size: 1048576,
    meta: meta({
      platform: 'windows',
      version: '1.0',
      build_id: '56012345678900001',
      save_locations: [{ name: 'saves', path: path.join(grey, 'saves') }],
      last_sync: now - 7 * 86400,
    }),
  })};
`,
);
