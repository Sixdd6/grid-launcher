#!/usr/bin/env node
/**
 * Seeds the `settings-romm` stage group's data dir before the app starts: a
 * profile past the welcome (`[setup] welcomed = true`, already migrated to
 * RomM as a storefront) with a library folder, no RomM server stored and
 * nothing installed. The app opens the shell with RomM never connected, and
 * a first connect from Settings › Game stores needs setup (no emulator, no
 * game, setup never finished).
 *
 * Invoked by scripts/e2e.sh's `seed_script_for_group` as
 * `node settings-romm-seed.mjs <data-dir>`.
 */
import { mkdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';

const dataDir = process.argv[2];
if (!dataDir) {
  console.error('usage: settings-romm-seed.mjs <data-dir>');
  process.exit(1);
}

const library = path.join(dataDir, 'library');
mkdirSync(library, { recursive: true });

const tomlString = (s) => JSON.stringify(s);

writeFileSync(
  path.join(dataDir, 'config.toml'),
  `schema_version = 1
library_path = ${tomlString(library)}
library_layout_version = 2
romm_store_migrated = true

[setup]
welcomed = true
`,
);
