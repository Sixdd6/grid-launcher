#!/usr/bin/env node
/**
 * Seeds an `amazon*` stage group's data dir before each spec (scripts/e2e.sh
 * runs `node amazon-seed.mjs <data-dir> <spec name> [--expired]`, with
 * E2E_AMAZON_URL set to the mock Amazon). Every spec starts from nothing, on
 * a profile used without a server (`offline = true`), then:
 *
 * - `amazon-login`, `amazon-secrets`: Amazon not connected, no store files
 *   (the specs sign in through the login window).
 * - every other Amazon spec (package D's `amazon-library` and `tv-amazon`):
 *   already signed in to Amazon Games as sam.plays: `[stores.amazon]` in
 *   config.toml, nile's own files in `stores/nile/nile/` (the store-token
 *   exception: `current_user.json`, the `<md5(user_id)>.enc` token file
 *   holding FAKE-STORE-TOKEN-not-real, `library.json` from
 *   e2e/fixtures-amazon/library.json with the art at the mock,
 *   `installed.json` and Fogline's manifest), and one installed Amazon game
 *   in the registry: Fogline (its files and fuel.json under
 *   `library/Amazon Games/Fogline`). Kettle Drum and Lowsun are available.
 *   The fake nile's `.fake-update` marker is set, so nile lists an update
 *   for Fogline. No server profile.
 *   `--expired` (or a spec name ending in `-expired`): the fake nile's
 *   `.fake-expired` marker too, so the startup sync finds the login expired
 *   (the prototype's default Amazon state).
 *
 * Always: `stubs/bin` with `umu-run` and `wine` stand-ins that write the
 * game's argv (one argument per line, a `---` line after each run) to
 * `<data>/amazon-game-argv.log` (`E2E_AMAZON_ARGV_FILE`) and its FUEL
 * environment (`NAME=value` lines, a `---` line after each run) to
 * `<data>/amazon-game-env.log` (`E2E_AMAZON_ENV_FILE`), so a spec sees nile's
 * launch prep reach the game. Under `amazon-secrets` the game exits at once
 * with code 3 (the early-exit warning); elsewhere it runs for two seconds.
 */
import { createHash } from 'node:crypto';
import { chmodSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { writeRegistry } from './registry-schema.mjs';

const dataDir = process.argv[2];
const spec = process.argv[3] ?? '';
const expired = process.argv.includes('--expired') || spec.endsWith('-expired');
if (!dataDir) {
  console.error('usage: amazon-seed.mjs <data-dir> [spec] [--expired]');
  process.exit(1);
}

const here = path.dirname(fileURLToPath(import.meta.url));
export const FAKE_TOKEN = 'FAKE-STORE-TOKEN-not-real';
const USERNAME = 'sam.plays';
const USER_ID = 'amzn1.account.FAKEE2EUSER0000000001';
const FOG = 'amzn1.adg.product.5b0f1a2c-3d4e-4f50-8a6b-0000000000a1';

rmSync(dataDir, { recursive: true, force: true });
const library = path.join(dataDir, 'library');
mkdirSync(library, { recursive: true });
const tomlString = (s) => JSON.stringify(s);
const sql = (s) => `'${String(s).replace(/'/g, "''")}'`;

// The compat tool stand-ins: they record the game's argv and FUEL env.
const stubs = path.join(dataDir, 'stubs', 'bin');
mkdirSync(stubs, { recursive: true });
const argvFile = path.join(dataDir, 'amazon-game-argv.log');
const envFile = path.join(dataDir, 'amazon-game-env.log');
const tail = spec === 'amazon-secrets' ? 'exit 3' : 'sleep 2\nexit 0';
for (const name of ['umu-run', 'wine']) {
  const file = path.join(stubs, name);
  writeFileSync(
    file,
    `#!/bin/sh
# e2e stand-in for ${name}: records the game's argv and its FUEL environment.
for a in "$@"; do printf '%s\\n' "$a" >> ${JSON.stringify(argvFile)}; done
printf -- '---\\n' >> ${JSON.stringify(argvFile)}
env | grep -E '^(FUEL_DIR|AMAZON_GAMES_[A-Z_]+)=' | sort >> ${JSON.stringify(envFile)}
printf -- '---\\n' >> ${JSON.stringify(envFile)}
${tail}
`,
  );
  chmodSync(file, 0o755);
}

const signedOut = spec === 'amazon-login' || spec === 'amazon-secrets';
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

[stores.amazon]
connected = true
username = ${tomlString(USERNAME)}
install_dir = ""
`,
);

const now = Math.floor(Date.now() / 1000);
// nile's folder: NILE_CONFIG_PATH is stores/nile, and nile appends /nile.
const stores = path.join(dataDir, 'stores');
const tool = path.join(stores, 'nile');
const cfg = path.join(tool, 'nile');
for (const dir of [stores, tool, cfg, path.join(cfg, 'manifests')]) {
  mkdirSync(dir, { recursive: true, mode: 0o700 });
  chmodSync(dir, 0o700);
}
const privateFile = (file, text) => {
  mkdirSync(path.dirname(file), { recursive: true });
  writeFileSync(file, text);
  chmodSync(file, 0o600);
};

const base = (process.env.E2E_AMAZON_URL || 'http://127.0.0.1:9').replace(/\/+$/, '');
const fixture = JSON.parse(
  readFileSync(path.join(here, '..', 'fixtures-amazon', 'library.json'), 'utf8').replaceAll('{BASE}', base),
);
const fog = fixture.find((e) => e.product.id === FOG);

privateFile(path.join(cfg, 'current_user.json'), JSON.stringify({ name: USERNAME, user_id: USER_ID }));
privateFile(
  path.join(cfg, `${createHash('md5').update(USER_ID).digest('hex')}.enc`),
  JSON.stringify({
    tokens: { bearer: { access_token: FAKE_TOKEN, refresh_token: FAKE_TOKEN, expires_in: '3600' } },
    extensions: { customer_info: { given_name: USERNAME, user_id: USER_ID } },
  }),
);
privateFile(path.join(cfg, 'library.json'), JSON.stringify(fixture.map(({ fake: _fake, ...entry }) => entry)));
privateFile(path.join(cfg, '.fake-update'), '');
if (expired) privateFile(path.join(cfg, '.fake-expired'), '');

// Fogline: installed at its first version, with its fuel.json.
const amazonDir = path.join(library, 'Amazon Games');
const fogDir = path.join(amazonDir, 'Fogline');
const exeRel = fog.fake.executable.split(/[\\/]/);
const fogExe = path.join(fogDir, ...exeRel);
mkdirSync(path.dirname(fogExe), { recursive: true });
writeFileSync(fogExe, 'MZ fake amazon game\n');
writeFileSync(
  path.join(fogDir, 'fuel.json'),
  `{\n  "SchemaVersion": "2",\n  // seeded\n  "Main": {\n    "Command": ${JSON.stringify(fog.fake.executable)},\n    "Args": ${JSON.stringify(fog.fake.args)},\n    "WorkingSubdirOverride": ${JSON.stringify(fog.fake.working_dir)},\n  },\n}\n`,
);
const fogPrefix = path.join(fogDir, 'prefix');
mkdirSync(fogPrefix, { recursive: true });
privateFile(path.join(cfg, 'manifests', `${FOG}.raw`), `fake manifest ${fog.fake.version}\n`);
privateFile(
  path.join(cfg, 'installed.json'),
  JSON.stringify([{ id: FOG, version: fog.fake.version, path: fogDir, size: fog.fake.download_size }]),
);

const meta = JSON.stringify({ product_id: FOG, version: fog.fake.version, exe_user_pinned: false });
const details = fog.product.productDetail.details;

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
   extracted_dir, native_game_dir, native_executable_path, native_launch_parameters, native_working_dir,
   native_launch_source, native_compat_tool, native_wineprefix, revision, filesize_bytes, installed_at,
   source, store_id, store_meta)
VALUES
  (${sql('Fogline')}, 'Amazon Games', ${sql(`fogline\u001famazon:${FOG}`)}, 'amazon games', 'win',
   ${sql(fogDir)}, ${sql(fogDir)}, ${sql(fogExe)}, ${sql(fog.fake.args.join(' '))}, ${sql(fog.fake.working_dir)},
   'sidecar', 'GE-Proton9-20', ${sql(fogPrefix)}, ${sql(fog.fake.version)}, ${fog.fake.download_size}, ${now - 30 * 86400},
   'amazon', ${sql(FOG)}, ${sql(meta)});
UPDATE installed_games SET
  companies = ${sql(details.developer)},
  first_release_date = '2020',
  last_played_at = ${now - 25 * 86400},
  genres = ${sql(details.genres.join(', '))}
WHERE store_id = ${sql(FOG)};
`,
);
