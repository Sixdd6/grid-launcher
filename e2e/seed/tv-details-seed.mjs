#!/usr/bin/env node
/**
 * Seeds the `tv-details` stage group's data dir before the app starts
 * (docs/tv-mode/plan.md P9), against `e2e/fixtures-tv-details`:
 *
 * - SNES: SaveSyncManual (601), SaveSyncLaunch (602) and SaveSyncRetention
 *   (603) installed, with the "TestEmu" stub as the platform's default and
 *   one shared save folder (the `cloud-saves` group's set-up), so Saves can
 *   upload, restore and delete; 603 has four server saves and a long
 *   summary with two Related games on the server. The stub runs 3 s for
 *   SaveSyncManual (Playing closes by itself) and until stopped otherwise.
 * - Mystery Cart: an installed SNES row with no server id (Link to server
 *   game; Link Target, 604, is the server game not installed).
 * - Lost Cart (605): installed, its folder gone (Missing); a copy waits in
 *   `<data>/elsewhere` for Locate….
 * - My Game (701, Windows): a native game with two launch files.
 * - No Emu Game (801, Nintendo 64): installed, its system set to no emulator
 *   (TestEmu would otherwise be picked for any platform).
 *
 * Invoked by scripts/e2e.sh's `seed_script_for_group` as
 * `node tv-details-seed.mjs <data-dir>`. tv-details-a connects through the
 * desktop's Connect form; b and c start with --tv on the saved session.
 */
import { chmodSync, mkdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { tomlString, writeRegistry } from './registry-schema.mjs';

const dataDir = process.argv[2];
if (!dataDir) {
  console.error('usage: tv-details-seed.mjs <data-dir>');
  process.exit(1);
}

const SNES = 'Super Nintendo Entertainment System';
const library = path.join(dataDir, 'library');
const stubs = path.join(dataDir, 'stubs');
const saves = path.join(dataDir, 'cloud-saves');
for (const d of [library, stubs, saves]) mkdirSync(d, { recursive: true });

// The TestEmu stub: SaveSyncManual exits after 3 s; any other game runs
// until stopped (SIGTERM ends it cleanly).
const stub = path.join(stubs, 'play-stub.sh');
writeFileSync(
  stub,
  `#!/bin/sh\n` +
    `case "$*" in *SaveSyncManual*) sleep 3; exit 0 ;; esac\n` +
    `trap 'exit 0' TERM\n` +
    `sleep 60 &\n` +
    `wait $!\n`,
);
chmodSync(stub, 0o755);

// Local saves for the upload and restore (matched by the title's stem).
writeFileSync(path.join(saves, 'savesyncmanual.sav'), 'local-save-manual');
writeFileSync(path.join(saves, 'savesyncretention.sav'), 'local-save-retention');

writeFileSync(
  path.join(dataDir, 'config.toml'),
  `schema_version = 1
library_path = ${tomlString(library)}
library_layout_version = 2
default_compat_tool = "wine"

[[emulators]]
name = "TestEmu"
path = ${tomlString(stub)}
args = "%rom%"
save_paths = ${tomlString(saves)}

[default_emulators]
${tomlString(SNES)} = "TestEmu"
"Nintendo 64" = "<none>"
`,
);

const sql = (v) => (v === null ? 'NULL' : `'${String(v).replace(/'/g, "''")}'`);
const now = Math.floor(Date.now() / 1000);

function game({ rom, title, platform, slug, ext, dir = null, exists = true, native = false }) {
  const folder = dir ?? path.join(library, 'games', platform, title);
  const file = path.join(folder, native ? 'mygame.exe' : `${title}.${ext}`);
  if (exists) {
    mkdirSync(folder, { recursive: true });
    writeFileSync(file, `fake bytes for ${title}\n`);
  }
  return `(${sql(title)}, ${sql(platform)}, ${sql(title.toLowerCase())}, ${sql(platform.toLowerCase())}, ${sql(slug)}, ${rom === null ? 'NULL' : rom},
   ${sql(path.basename(file))}, ${sql(native ? '' : file)}, ${sql(folder)}, ${sql(native ? folder : '')}, ${sql(native ? file : '')},
   '2026-01-01T00:00:00Z', ${now - 100}, 0, 1)`;
}

// My Game's second launch file.
const myGame = path.join(library, 'games', 'Windows', 'My Game');
mkdirSync(path.join(myGame, 'tools'), { recursive: true });
writeFileSync(path.join(myGame, 'tools', 'setup.exe'), 'fake setup\n');

// Lost Cart's copy, outside the library.
const elsewhere = path.join(dataDir, 'elsewhere', 'Lost Cart');
mkdirSync(elsewhere, { recursive: true });
writeFileSync(path.join(elsewhere, 'Lost Cart.sfc'), 'fake bytes for Lost Cart\n');
// Locate… opens in the folder the game was in: the platform's.
mkdirSync(path.join(library, 'games', SNES), { recursive: true });

const rows = [
  game({ rom: 601, title: 'SaveSyncManual', platform: SNES, slug: 'snes', ext: 'sfc' }),
  game({ rom: 602, title: 'SaveSyncLaunch', platform: SNES, slug: 'snes', ext: 'sfc' }),
  game({ rom: 603, title: 'SaveSyncRetention', platform: SNES, slug: 'snes', ext: 'sfc' }),
  game({ rom: null, title: 'Mystery Cart', platform: SNES, slug: 'snes', ext: 'sfc' }),
  game({ rom: 605, title: 'Lost Cart', platform: SNES, slug: 'snes', ext: 'sfc', exists: false }),
  game({ rom: 701, title: 'My Game', platform: 'Windows', slug: 'win', ext: 'exe', native: true }),
  game({ rom: 801, title: 'No Emu Game', platform: 'Nintendo 64', slug: 'n64', ext: 'z64' }),
];

writeRegistry(
  path.join(dataDir, 'grid-launcher.db'),
  `
INSERT INTO installed_games
  (title, platform, title_key, platform_key, platform_slug, rom_id,
   rom_file_name, extracted_path, extracted_dir, native_game_dir, native_executable_path,
   server_updated_at, installed_at, last_played_at, images_version)
VALUES
  ${rows.join(',\n  ')};
`,
);

console.log(`e2e: seeded tv-details in ${dataDir}`);
