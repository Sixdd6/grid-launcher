#!/usr/bin/env node
/**
 * Seeds the `cloud-saves` stage group's temp data dir BEFORE the app starts:
 * `config.toml` (a library path plus one stub emulator, "TestEmu", default
 * for the platform, with `save_paths` pointing at one shared save
 * directory), `grid-launcher.db` (three installed games — rom 601/602/603,
 * "SaveSyncManual"/"SaveSyncLaunch"/"SaveSyncRetention", all on "Super
 * Nintendo Entertainment System", matching
 * `e2e/fixtures-cloud-saves/{platforms,roms,rom-details}.json`), local save
 * files for 601 and 603 (601's is the manual-upload source; 603's exists
 * only so its upload job has something to send — the retention scenario is
 * about the SERVER-side records `e2e/fixtures-cloud-saves/saves.json`
 * seeds, not this file's content), and the "TestEmu" stub script.
 *
 * Follows launch-seed.mjs's pattern (Ruling A, task-7-brief.md):
 * pre-seeding here is preferred over installing/configuring through the UI,
 * which earlier groups already cover. cloud-saves.spec.ts still connects
 * through the normal Connect UI.
 *
 * Directory resolution for cloud saves (`resolved_sync_dirs`,
 * grid-core/src/cloud/dirs.rs) requires the configured `save_paths`
 * directory to already exist on disk — matching a real emulator's save
 * folder always existing once the emulator has run once. All three games
 * share ONE save directory; the generic candidate matcher
 * (`game_save_match_tokens`, grid-core/src/cloud/tokens.rs) picks a local
 * file by its lowercased title-derived stem, so `savesyncmanual.sav`,
 * `savesynclaunch.sav`, and `savesyncretention.sav` coexist there without
 * conflict — the same one-directory-many-games shape
 * `cloud/ops/tests.rs`'s own wiremock fixtures use ("zelda.srm" for game
 * "Zelda").
 *
 * "TestEmu" is deliberately not a recognized xemu/redream/RetroArch name,
 * so `cloud_save_scope` resolves every game here to `PerGame` (button text
 * "Manage Saves", not "Emulator Saves" — `cloudButtonLabel`,
 * app/src/lib/details/cloud.ts) and no shared-scope indirection applies.
 *
 * The "TestEmu" stub script (`play-stub.sh`) is only ever actually RUN for
 * rom 602 (the launch/exit scenarios) — 601 and 603 are only ever acted on
 * through the cloud panel's Upload button, never Play, so the emulator
 * executable's behavior does not matter for them. On start it records its
 * argv (proving the process launched, mirroring `launch-seed.mjs`'s
 * `long-runner.sh`), then sleeps in the background so a `SIGTERM` (the
 * Details "Stop" button, `LaunchService::stop`, launch/mod.rs) is caught by
 * a trap BEFORE the process exits: the trap overwrites rom 602's save file
 * with fresh "gameplay" content and exits cleanly. This is what lets the
 * spec observe, in order: (1) the cloud-downloaded save already on disk
 * the moment the stub starts (auto-restore-before-launch completed before
 * the emulator process spawned — commands.rs's `launch_game` awaits it
 * first), then (2) the stub's own new content on disk once it exits, which
 * the post-exit auto-upload (delay set to 0 via the cloud settings UI)
 * then POSTs back to the mock.
 */

import { execFileSync } from 'node:child_process';
import { chmodSync, mkdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';

const dataDir = process.argv[2];
if (!dataDir) {
  console.error('usage: cloud-saves-seed.mjs <data-dir>');
  process.exit(1);
}

const PLATFORM = 'Super Nintendo Entertainment System';
const EMULATOR_NAME = 'TestEmu';

const GAMES = [
  { romId: 601, title: 'SaveSyncManual' },
  { romId: 602, title: 'SaveSyncLaunch' },
  { romId: 603, title: 'SaveSyncRetention' },
];

const stubsDir = path.join(dataDir, 'stubs');
const libraryPath = path.join(dataDir, 'library');
const savesDir = path.join(dataDir, 'cloud-saves');
mkdirSync(stubsDir, { recursive: true });
mkdirSync(savesDir, { recursive: true });

// --- the "TestEmu" stub emulator ---------------------------------------------

const playArgv = path.join(stubsDir, 'play-stub.args');
const playStub = path.join(stubsDir, 'play-stub.sh');
const launchSaveFile = path.join(savesDir, 'savesynclaunch.sav');
writeFileSync(
  playStub,
  `#!/bin/sh\n` +
    `printf '%s\\n' "$@" > '${playArgv}'\n` +
    `trap 'printf "post-play-save-content" > "${launchSaveFile}"; exit 0' TERM\n` +
    `sleep 30 &\n` +
    `wait $!\n`,
);
chmodSync(playStub, 0o755);

// --- local save files -----------------------------------------------------
//
// 601 (manual upload) and 603 (retention) both need an existing local file
// for their upload job to have something to send. 602 (launch/exit) starts
// with NO local file — its content only appears once the auto-restore
// download writes it, which is exactly what the launch scenario asserts.

writeFileSync(path.join(savesDir, 'savesyncmanual.sav'), 'local-save-for-manual-upload');
writeFileSync(path.join(savesDir, 'savesyncretention.sav'), 'local-save-for-retention-upload');

// --- the "installed" ROM files (registry NOT NULL columns only) -------------

const gameDirs = new Map();
for (const { romId, title } of GAMES) {
  const gameDir = path.join(libraryPath, PLATFORM, title);
  mkdirSync(gameDir, { recursive: true });
  const romPath = path.join(gameDir, 'game.rom');
  writeFileSync(romPath, `fake rom bytes for ${title}\n`);
  gameDirs.set(romId, romPath);
}

// --- config.toml ---------------------------------------------------------------

/** Minimal TOML basic-string escaping — good enough for plain paths/names. */
function tomlString(value) {
  return `"${value.replace(/\\/g, '\\\\').replace(/"/g, '\\"')}"`;
}

const configToml = `schema_version = 1
library_path = ${tomlString(libraryPath)}

[[emulators]]
name = ${tomlString(EMULATOR_NAME)}
path = ${tomlString(playStub)}
args = "%rom%"
save_paths = ${tomlString(savesDir)}

[default_emulators]
${tomlString(PLATFORM)} = ${tomlString(EMULATOR_NAME)}
`;
writeFileSync(path.join(dataDir, 'config.toml'), configToml);

// --- grid-launcher.db ----------------------------------------------------------
//
// Schema copied verbatim from crates/grid-core/src/library/registry.rs's
// SCHEMA_SQL — same approach and same caveat as launch-seed.mjs: a
// mismatch here silently diverges from what the running app expects
// instead of failing loudly, so keep this in sync with that schema.

function sqlString(value) {
  return value.replace(/'/g, "''");
}

const installedAt = Math.floor(Date.now() / 1000);

const dbPath = path.join(dataDir, 'grid-launcher.db');
let sql = `
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
`;

const platformKey = PLATFORM.trim().toLowerCase();
for (const { romId, title } of GAMES) {
  const titleKey = title.trim().toLowerCase();
  const romPath = gameDirs.get(romId);
  sql += `
INSERT INTO installed_games
  (title, platform, title_key, platform_key, rom_id, rom_file_name, extracted_path, installed_at)
VALUES
  ('${sqlString(title)}', '${sqlString(PLATFORM)}', '${sqlString(titleKey)}', '${sqlString(platformKey)}', ${romId}, 'game.rom', '${sqlString(romPath)}', ${installedAt});
`;
}

execFileSync('sqlite3', [dbPath], { input: sql, stdio: ['pipe', 'inherit', 'inherit'] });

console.log(`e2e: seeded cloud-saves stage data dir at ${dataDir}`);
