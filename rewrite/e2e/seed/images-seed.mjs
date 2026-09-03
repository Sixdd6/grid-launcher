#!/usr/bin/env node
/**
 * Seeds the `images` stage group's temp data dir BEFORE the app starts:
 * `config.toml` (a library path only — no server_url, since the mock's port
 * is not known until e2e.sh starts it after this script runs; images-a.spec.ts
 * connects through the normal Connect UI, same as `library`/`install`) and
 * `grid-launcher.db` written with the **v1** schema (the pre-image-columns
 * shape, `PRAGMA user_version = 1` — copied from
 * crates/grid-core/src/library/registry.rs's SCHEMA_SQL as it stood before
 * milestone 7's image columns, same approach as launch-seed.mjs/
 * cloud-saves-seed.mjs) holding one row: rom 102, "Chrono Trigger (USA)" on
 * "Super Nintendo Entertainment System" — matching
 * e2e/fixtures/rom-details.json's 102 entry (`name: null`, so the app falls
 * back to `fs_name_no_ext`, which is this exact title).
 *
 * This row is the migration + replenish subject for images-a/-b.spec.ts:
 * `Registry::open` migrates it from v1 to v2 on first open (adding
 * cover_small_path/cover_large_path/screenshot_urls, all empty), and the
 * post-connect replenish job then back-fills those from the server and
 * fetches the cover file — first when images-a connects, again (this time
 * actually completing, since the row starts the run with no rom_id-matched
 * cover cached) when images-b retries after the mock comes back online.
 *
 * The library file itself (`game.rom`) exists on disk so this row looks like
 * a real prior install, matching every other seeded row in this suite.
 */

import { execFileSync } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';

const dataDir = process.argv[2];
if (!dataDir) {
  console.error('usage: images-seed.mjs <data-dir>');
  process.exit(1);
}

const PLATFORM = 'Super Nintendo Entertainment System';
const TITLE = 'Chrono Trigger (USA)';
const ROM_ID = 102;

const libraryPath = path.join(dataDir, 'library');
const gameDir = path.join(libraryPath, PLATFORM, TITLE);
mkdirSync(gameDir, { recursive: true });

const romPath = path.join(gameDir, 'game.rom');
writeFileSync(romPath, 'fake rom bytes for Chrono Trigger (USA)\n');

// --- config.toml ---------------------------------------------------------------

/** Minimal TOML basic-string escaping — good enough for plain paths. */
function tomlString(value) {
  return `"${value.replace(/\\/g, '\\\\').replace(/"/g, '\\"')}"`;
}

const configToml = `schema_version = 1
library_path = ${tomlString(libraryPath)}
`;
writeFileSync(path.join(dataDir, 'config.toml'), configToml);

// --- grid-launcher.db (v1 schema — no image columns yet) -----------------------
//
// Deliberately the PRE-milestone-7 shape: Registry::open migrates a
// user_version=1 database to the current LATEST_USER_VERSION (2) by adding
// cover_small_path/cover_large_path/screenshot_urls as empty-string columns
// (crates/grid-core/src/library/registry.rs's MIGRATE_1_TO_2_SQL). Writing
// the v2 shape directly here would skip the exact migration path this group
// exists to exercise.

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
  ('${sqlString(TITLE)}', '${sqlString(PLATFORM)}', '${sqlString(titleKey)}', '${sqlString(platformKey)}', ${ROM_ID}, 'game.rom', '${sqlString(romPath)}', ${installedAt});
`;

execFileSync('sqlite3', [dbPath], { input: sql, stdio: ['pipe', 'inherit', 'inherit'] });

console.log(`e2e: seeded images stage data dir at ${dataDir}`);
