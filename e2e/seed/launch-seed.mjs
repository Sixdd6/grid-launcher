#!/usr/bin/env node
/**
 * Seeds the `launch` stage group's temp data dir BEFORE the app starts:
 * `config.toml` (a library path plus three stub emulator entries and one
 * default), `grid-launcher.db` (one installed game — rom 101, "Super Mario
 * World" on "Super Nintendo Entertainment System", matching the fixture
 * used by install-a/b.spec.ts), the "installed" ROM file itself, and the
 * three emulator stub scripts.
 *
 * Invoked by rewrite/scripts/e2e.sh's `seed_script_for_group` (only the
 * `launch` group runs this) as `node launch-seed.mjs <data-dir>`, after the
 * group's data dir is created and before `wdio run` starts the app. This is
 * Ruling A from task-7-brief.md: pre-seeding here is preferred over
 * installing through the UI, which Task 6 already covers. launch.spec.ts
 * still connects through the normal Connect UI — grid_core::session::connect
 * loads this config.toml first and only overwrites server_url/username, so
 * library_path/emulators/default_emulators all survive that round trip.
 *
 * Stub shapes reuse crates/grid-core/tests/launch_service.rs's `Harness`:
 * the long-runner records its argv (one arg per line) to a sibling file,
 * then sleeps; the instant-exit stub exits 3 immediately.
 *
 * The RetroArch stub's basename ("retroarch") is a literal entry in the
 * repo-root emulator-autoprofiles.json's match_tokens for the "RetroArch
 * (Multi-System)" profile. Under design D-RC-1 that profile's
 * `all_platforms: true` no longer implies support: `emulator_supports_platform`
 * (launch/selection.rs) runs the RetroArch core gate FIRST, so the stub
 * needs a real core file beside it to be selectable at all. This seed writes
 * `stubs/cores/snes9x_libretro.so` for exactly that reason, and deliberately
 * writes NO [retroarch_cores] table — which is the setup launch.spec.ts's
 * "no RetroArch core configured" test needs.
 */

import { execFileSync } from 'node:child_process';
import { chmodSync, mkdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';

const dataDir = process.argv[2];
if (!dataDir) {
  console.error('usage: launch-seed.mjs <data-dir>');
  process.exit(1);
}

const PLATFORM = 'Super Nintendo Entertainment System';
const TITLE = 'Super Mario World';
const ROM_ID = 101;

const stubsDir = path.join(dataDir, 'stubs');
const libraryPath = path.join(dataDir, 'library');
const gameDir = path.join(libraryPath, PLATFORM, TITLE);
mkdirSync(stubsDir, { recursive: true });
mkdirSync(gameDir, { recursive: true });

// --- stub emulators ----------------------------------------------------------

const longRunnerArgv = path.join(stubsDir, 'long-runner.args');
const longRunner = path.join(stubsDir, 'long-runner.sh');
writeFileSync(longRunner, `#!/bin/sh\nprintf '%s\\n' "$@" > '${longRunnerArgv}'\nsleep 30\n`);
chmodSync(longRunner, 0o755);

const instantExit = path.join(stubsDir, 'instant-exit.sh');
writeFileSync(instantExit, '#!/bin/sh\nexit 3\n');
chmodSync(instantExit, 0o755);

const retroarch = path.join(stubsDir, 'retroarch');
writeFileSync(retroarch, '#!/bin/sh\nexit 0\n');
chmodSync(retroarch, 0o755);

const coresDir = path.join(stubsDir, 'cores');
mkdirSync(coresDir, { recursive: true });
writeFileSync(path.join(coresDir, 'snes9x_libretro.so'), '');

// --- the "installed" ROM file itself -----------------------------------------

const romPath = path.join(gameDir, 'game.sfc');
writeFileSync(romPath, 'fake snes rom bytes\n');

// --- config.toml ---------------------------------------------------------------

/** Minimal TOML basic-string escaping — good enough for the plain paths and
 * names this script ever writes (no control characters, no embedded quotes
 * beyond the ones this handles). */
function tomlString(value) {
  return `"${value.replace(/\\/g, '\\\\').replace(/"/g, '\\"')}"`;
}

const configToml = `schema_version = 1
library_path = ${tomlString(libraryPath)}

[[emulators]]
name = "LongRunner"
path = ${tomlString(longRunner)}
args = "%rom%"

[[emulators]]
name = "InstantExit"
path = ${tomlString(instantExit)}
args = "%rom%"

[[emulators]]
name = "RetroArch"
path = ${tomlString(retroarch)}
args = "-L \\"%core%\\" \\"%rom%\\""

[default_emulators]
${tomlString(PLATFORM)} = "LongRunner"
`;
writeFileSync(path.join(dataDir, 'config.toml'), configToml);

// --- grid-launcher.db ----------------------------------------------------------
//
// Schema copied verbatim from crates/grid-core/src/library/registry.rs's
// SCHEMA_SQL. Registry::open only runs that CREATE TABLE when
// PRAGMA user_version is 0 (it trusts a database already at
// LATEST_USER_VERSION to have the right shape), so a mismatch here would
// silently diverge from what the running app expects instead of failing
// loudly.

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
  ('${sqlString(TITLE)}', '${sqlString(PLATFORM)}', '${sqlString(titleKey)}', '${sqlString(platformKey)}', ${ROM_ID}, 'game.sfc', '${sqlString(romPath)}', ${installedAt});
`;

execFileSync('sqlite3', [dbPath], { input: sql, stdio: ['pipe', 'inherit', 'inherit'] });

console.log(`e2e: seeded launch stage data dir at ${dataDir}`);
