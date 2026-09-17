#!/usr/bin/env node
/**
 * Seeds the `layout-migration` stage group's temp data dir with a LEGACY
 * (pre-v1) library, so the app's startup migration has something to move:
 *
 * - `library/Super Nintendo Entertainment System/Game A/game.sfc` — a flat
 *   platform directory, referenced by the registry row's `extracted_path`
 *   and `extracted_dir` (the migration is reference-driven: an unreferenced
 *   top-level directory is left alone).
 * - `library/Emulators/PCSX2 (Playstation 2)-latest/pcsx2-qt` — the tagged
 *   install directory the old layout used, plus a real `memcards/slot1.mcd`
 *   inside it. `memcards` is in the PCSX2 profile's `user_data`, so the
 *   migration moves it under `library/saves/` and leaves a link behind.
 * - `config.toml` with `library_path` and that emulator entry, and
 *   deliberately NO `library_layout_version` key — that absence (version 0)
 *   is what makes the migration run at all.
 *
 * Invoked by scripts/e2e.sh's `seed_script_for_group` as
 * `node layout-migration-seed.mjs <data-dir>`, before the app starts.
 * layout-migration.spec.ts then reads the result off disk; it never connects.
 */

import { chmodSync, mkdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';

import { tomlString, writeRegistry } from './registry-schema.mjs';

const dataDir = process.argv[2];
if (!dataDir) {
  console.error('usage: layout-migration-seed.mjs <data-dir>');
  process.exit(1);
}

const PLATFORM = 'Super Nintendo Entertainment System';
const TITLE = 'Game A';
const ROM_ID = 701;
const EMULATOR_NAME = 'PCSX2 (Playstation 2)';

const libraryPath = path.join(dataDir, 'library');

// --- the legacy game tree ------------------------------------------------------

const extractedDir = path.join(libraryPath, PLATFORM, TITLE);
mkdirSync(extractedDir, { recursive: true });
const extractedPath = path.join(extractedDir, 'game.sfc');
writeFileSync(extractedPath, 'fake snes rom bytes\n');

// --- the legacy emulator install ----------------------------------------------

const installDir = path.join(libraryPath, 'Emulators', `${EMULATOR_NAME}-latest`);
mkdirSync(path.join(installDir, 'memcards'), { recursive: true });
const emulatorPath = path.join(installDir, 'pcsx2-qt');
writeFileSync(emulatorPath, '#!/bin/sh\nexit 0\n');
chmodSync(emulatorPath, 0o755);
writeFileSync(path.join(installDir, 'memcards', 'slot1.mcd'), 'MEMCARD1\n');

// --- config.toml (no library_layout_version on purpose) ------------------------

const configToml = `schema_version = 1
library_path = ${tomlString(libraryPath)}

[[emulators]]
name = ${tomlString(EMULATOR_NAME)}
path = ${tomlString(emulatorPath)}
args = "%rom%"
`;
writeFileSync(path.join(dataDir, 'config.toml'), configToml);

// --- grid-launcher.db ----------------------------------------------------------

function sqlString(value) {
  return value.replace(/'/g, "''");
}

const installedAt = Math.floor(Date.now() / 1000);
writeRegistry(
  path.join(dataDir, 'grid-launcher.db'),
  `
INSERT INTO installed_games
  (title, platform, title_key, platform_key, rom_id, rom_file_name,
   extracted_path, extracted_dir, installed_at, images_version)
VALUES
  ('${sqlString(TITLE)}', '${sqlString(PLATFORM)}', '${sqlString(TITLE.toLowerCase())}',
   '${sqlString(PLATFORM.toLowerCase())}', ${ROM_ID}, 'game.sfc',
   '${sqlString(extractedPath)}', '${sqlString(extractedDir)}', ${installedAt}, 1);
`,
);

console.log(`e2e: seeded layout-migration stage data dir at ${dataDir}`);
