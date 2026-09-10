#!/usr/bin/env node
/**
 * Seeds the `emulator-catalog` stage group's temp data dir BEFORE the app
 * starts: `config.toml` (just a library path — this group installs its
 * emulators through the catalog UI rather than getting them pre-seeded),
 * `grid-launcher.db` (one installed game: rom 401, "Gran Turismo 3" on
 * "Sony PlayStation 2", from e2e/fixtures-emulator-catalog), and the
 * "installed" ROM file itself.
 *
 * Invoked by rewrite/scripts/e2e.sh's `seed_script_for_group` as
 * `node emulator-catalog-seed.mjs <data-dir>`, after the group's data dir is
 * created and before `wdio run` starts the app — same contract as
 * launch-seed.mjs, whose structure this copies.
 *
 * Why an installed game at all: emulator-catalog.spec.ts sets the freshly
 * installed PCSX2 as the "Sony PlayStation 2" default and then plays this
 * game, which is what proves the installed emulator is actually launchable
 * (the stub records its argv). "Sony PlayStation 2" matches PCSX2's
 * `platform_keywords` (["playstation 2", "ps2"]) through
 * platform_matches_keywords, so `emulator_supports_platform` accepts the
 * pairing.
 *
 * The library path is seeded rather than typed into the UI because the
 * emulator install pipeline needs it before the first catalog Install click;
 * grid_core::session::connect only overwrites server_url/username, so it
 * survives the spec's Connect round trip.
 */

import { execFileSync } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';

const dataDir = process.argv[2];
if (!dataDir) {
  console.error('usage: emulator-catalog-seed.mjs <data-dir>');
  process.exit(1);
}

const PLATFORM = 'Sony PlayStation 2';
const TITLE = 'Gran Turismo 3';
const ROM_ID = 401;

const libraryPath = path.join(dataDir, 'library');
const gameDir = path.join(libraryPath, PLATFORM, TITLE);
mkdirSync(gameDir, { recursive: true });

// --- the "installed" ROM file itself -----------------------------------------

const romPath = path.join(gameDir, 'game.iso');
writeFileSync(romPath, 'fake ps2 rom bytes\n');

// --- config.toml ---------------------------------------------------------------

/** Minimal TOML basic-string escaping — good enough for the plain paths and
 * names this script ever writes (no control characters, no embedded quotes
 * beyond the ones this handles). */
function tomlString(value) {
  return `"${value.replace(/\\/g, '\\\\').replace(/"/g, '\\"')}"`;
}

const configToml = `schema_version = 1
library_path = ${tomlString(libraryPath)}
`;
writeFileSync(path.join(dataDir, 'config.toml'), configToml);

// --- grid-launcher.db ----------------------------------------------------------
//
// Schema copied verbatim from crates/grid-core/src/library/registry.rs's
// SCHEMA_SQL (see launch-seed.mjs for why it has to match exactly).

function sqlString(value) {
  return value.replace(/'/g, "''");
}

const titleKey = TITLE.trim().toLowerCase();
const platformKey = PLATFORM.trim().toLowerCase();
const installedAt = Math.floor(Date.now() / 1000);

const dbPath = path.join(dataDir, 'grid-launcher.db');
const sql = `
CREATE TABLE installed_games (
    id                  INTEGER PRIMARY KEY,
    title               TEXT NOT NULL,
    platform            TEXT NOT NULL,
    title_key           TEXT NOT NULL,
    platform_key        TEXT NOT NULL,
    rom_id              INTEGER,
    rom_file_name       TEXT NOT NULL DEFAULT '',
    archive_path        TEXT NOT NULL DEFAULT '',
    extracted_path      TEXT NOT NULL DEFAULT '',
    extracted_dir       TEXT NOT NULL DEFAULT '',
    multi_file_game_dir TEXT NOT NULL DEFAULT '',
    description         TEXT NOT NULL DEFAULT '',
    rating              TEXT NOT NULL DEFAULT '',
    genres              TEXT NOT NULL DEFAULT '',
    regions             TEXT NOT NULL DEFAULT '',
    languages           TEXT NOT NULL DEFAULT '',
    tags                TEXT NOT NULL DEFAULT '',
    revision            TEXT NOT NULL DEFAULT '',
    companies           TEXT NOT NULL DEFAULT '',
    first_release_date  TEXT NOT NULL DEFAULT '',
    filesize_bytes      INTEGER NOT NULL DEFAULT 0,
    server_updated_at   TEXT NOT NULL DEFAULT '',
    installed_at        INTEGER NOT NULL,
    UNIQUE (title_key, platform_key)
);
PRAGMA user_version = 1;
INSERT INTO installed_games
  (title, platform, title_key, platform_key, rom_id, rom_file_name, extracted_path, installed_at)
VALUES
  ('${sqlString(TITLE)}', '${sqlString(PLATFORM)}', '${sqlString(titleKey)}', '${sqlString(platformKey)}', ${ROM_ID}, 'game.iso', '${sqlString(romPath)}', ${installedAt});
`;

execFileSync('sqlite3', [dbPath], { input: sql, stdio: ['pipe', 'inherit', 'inherit'] });

console.log(`e2e: seeded emulator-catalog stage data dir at ${dataDir}`);
