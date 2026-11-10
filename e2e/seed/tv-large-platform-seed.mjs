#!/usr/bin/env node
/**
 * Seeds the `tv-large-platform` stage group's data dir before the app
 * starts (TV mode plan risk R8): a library folder and 600 installed games
 * on one platform ("Library Game 0001" … "0600", no server id, a small file
 * each so none reads as missing), so the TV Library's grid is long enough
 * to window. The spec connects on the desktop (the large-platform fixtures:
 * one 2500-game Arcade platform) and enters TV mode with the TV button.
 *
 * Invoked by scripts/e2e.sh's `seed_script_for_group` as
 * `node tv-large-platform-seed.mjs <data-dir>`.
 */
import { mkdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';

import { tomlString, writeRegistry } from './registry-schema.mjs';

const dataDir = process.argv[2];
if (!dataDir) {
  console.error('usage: tv-large-platform-seed.mjs <data-dir>');
  process.exit(1);
}

export const LIBRARY_GAMES = 600;

const library = path.join(dataDir, 'library');
mkdirSync(library, { recursive: true });
writeFileSync(
  path.join(dataDir, 'config.toml'),
  `schema_version = 1
library_path = ${tomlString(library)}
library_layout_version = 2
`,
);

const sql = (value) => `'${String(value).replace(/'/g, "''")}'`;
const now = Math.floor(Date.now() / 1000);
const platform = 'Nintendo Entertainment System';
const dir = path.join(library, 'games', platform);
mkdirSync(dir, { recursive: true });
const values = [];
for (let i = 1; i <= LIBRARY_GAMES; i++) {
  const title = `Library Game ${String(i).padStart(4, '0')}`;
  const file = path.join(dir, `${title}.nes`);
  writeFileSync(file, Buffer.from(title));
  values.push(
    `(${sql(title)}, ${sql(platform)}, ${sql(title.toLowerCase())}, ${sql(platform.toLowerCase())}, 'nes', NULL, ${sql(path.basename(file))}, ${sql(file)}, '', '', '', '', '', ${now - i}, 0, 1)`,
  );
}
writeRegistry(
  path.join(dataDir, 'grid-launcher.db'),
  `
INSERT INTO installed_games
  (title, platform, title_key, platform_key, platform_slug, rom_id, rom_file_name,
   extracted_path, extracted_dir, multi_file_game_dir,
   native_game_dir, native_executable_path, server_updated_at, installed_at, last_played_at, images_version)
VALUES
  ${values.join(',\n  ')};
`,
);
console.log(`e2e: seeded tv-large-platform in ${dataDir}`);
