#!/usr/bin/env node
/**
 * Seeds the `romm-library` stage group (romm-storefront-plan Step 3) before
 * each of its specs. `scripts/e2e.sh` runs it per spec as
 * `node romm-library-seed.mjs <data-dir> <spec base name>`:
 *
 * - `romm-library-a`: a fresh profile with a library folder and nothing
 *   installed (never connected, no RomM games: no RomM group).
 * - `romm-library-b`: a fresh profile again (Connect asks for the server),
 *   with three installed RomM games on the Super Nintendo:
 *   - Super Mario World, rom 101 on the mock: merged with the server's game;
 *   - Hidden Kiln, no rom id: NOT LINKED (no server game has its title, so
 *     the relink pass after a connect leaves it alone);
 *   - Sky Lantern, rom 999 from another RomM server: NOT ON ROMM.
 * - `romm-library-c`, `romm-library-d`: nothing; they relaunch on what the
 *   earlier specs left.
 *
 * Both systems are set to None, so nothing installs an emulator.
 */
import { mkdirSync, rmSync, writeFileSync } from 'node:fs';
import path from 'node:path';

import { tomlString, writeRegistry } from './registry-schema.mjs';

const dataDir = process.argv[2];
const spec = process.argv[3] ?? '';
if (!dataDir) {
  console.error('usage: romm-library-seed.mjs <data-dir> <spec base name>');
  process.exit(1);
}
if (spec !== 'romm-library-a' && spec !== 'romm-library-b') process.exit(0);

const library = path.join(dataDir, 'library');
mkdirSync(library, { recursive: true });
writeFileSync(
  path.join(dataDir, 'config.toml'),
  `schema_version = 1
library_path = ${tomlString(library)}
library_layout_version = 2

[default_emulators]
"Super Nintendo Entertainment System" = "<none>"
Arcade = "<none>"
`,
);

const db = path.join(dataDir, 'grid-launcher.db');
// Each seeded spec starts from a fresh registry (part A's app made one).
for (const f of [db, `${db}-wal`, `${db}-shm`]) rmSync(f, { force: true });
if (spec === 'romm-library-a') {
  writeRegistry(db);
  process.exit(0);
}

const sql = (value) => `'${String(value).replace(/'/g, "''")}'`;
const now = Math.floor(Date.now() / 1000);
const PLATFORM = 'Super Nintendo Entertainment System';
const GAMES = [
  { title: 'Super Mario World', rom: 101, server: '' },
  { title: 'Hidden Kiln', rom: null, server: '' },
  { title: 'Sky Lantern', rom: 999, server: 'http://romm.old.invalid' },
];
const values = GAMES.map((g, i) => {
  const dir = path.join(library, 'games', PLATFORM, g.title);
  const file = path.join(dir, `${g.title}.sfc`);
  mkdirSync(dir, { recursive: true });
  writeFileSync(file, Buffer.alloc(2048, 0x40 + i));
  return `(${sql(g.title)}, ${sql(PLATFORM)}, ${sql(g.title.toLowerCase())}, ${sql(PLATFORM.toLowerCase())}, 'snes',
   ${g.rom === null ? 'NULL' : g.rom}, ${sql(path.basename(file))}, ${sql(file)}, 2048, ${now - 100 + i}, 0, 1, ${sql(g.server)})`;
});
writeRegistry(
  db,
  `
INSERT INTO installed_games
  (title, platform, title_key, platform_key, platform_slug, rom_id, rom_file_name,
   extracted_path, filesize_bytes, installed_at, last_played_at, images_version, source_server)
VALUES
  ${values.join(',\n  ')};
`,
);
