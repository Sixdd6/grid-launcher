#!/usr/bin/env node
/**
 * Seeds the `updates` stage group's temp data dir BEFORE the app starts:
 * a library path, `default_compat_tool = "wine"`, and a registry holding
 * four already-installed games that between them cover every outcome of
 * the server-update check (`game_has_server_update`,
 * grid-core/src/library/update_detection.rs).
 *
 * Nothing here is ever launched — the compat tool is written only because a
 * Windows-platform row without one is an unusual shape for the app to see,
 * and no `wine` stub is needed for it. The four rows pair with
 * `e2e/fixtures-updates/rom-details.json`:
 *
 *   801 "Old Rom"     SNES     installed `oldrom.zip` @ 2025-01-01, server
 *                              `newrom.zip` @ 2026-06-01 -> UPDATE, by the
 *                              timestamp rule. Neither name carries a
 *                              version tag, so the button reads "Update".
 *   802 "My Game"     Windows  installed `mygame (v1.0.0).zip`, server
 *                              `mygame (v1.1.0).zip` (same timestamp) ->
 *                              UPDATE, by the Windows file-name rule; the
 *                              button reads "Update to v1.1.0".
 *   803 "Current Rom" SNES     same file name and same timestamp as the
 *                              server -> no update.
 *   804 "Ghost Rom"   SNES     no `rom-details.json` entry at all, so the
 *                              mock answers 404 and the check treats it as
 *                              "no update" rather than failing the pass.
 *
 * On-disk shapes matter for the two updates:
 *
 * - 801 is a plain re-install (`install_update`), which re-downloads the
 *   server's `newrom.zip` into `<library>/SNES/` and extracts it beside the
 *   seeded `oldrom/` directory. The seeded files only have to make the row
 *   look genuinely installed.
 * - 802 is a MERGE (`install_native_update`), which refuses to run unless
 *   `extracted_dir` exists as a directory on disk. Its `saves/slot1.sav`
 *   is the whole point of the native path: the update archive does not
 *   contain it, so finding it unchanged afterwards is what proves the merge
 *   preserved user data instead of replacing the tree. `native_game_dir` is
 *   the game's own home directory — the archive is downloaded there, beside
 *   `extracted_dir`, so the merge never crosses a filesystem.
 */

import { mkdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';

import { tomlString, writeRegistry } from './registry-schema.mjs';

const dataDir = process.argv[2];
if (!dataDir) {
  console.error('usage: updates-seed.mjs <data-dir>');
  process.exit(1);
}

const libraryPath = path.join(dataDir, 'library');
mkdirSync(libraryPath, { recursive: true });

const configToml = `schema_version = 1
library_path = ${tomlString(libraryPath)}
default_compat_tool = "wine"
`;
writeFileSync(path.join(dataDir, 'config.toml'), configToml);

// --- on-disk installs ---------------------------------------------------------

/** Creates `<library>/<platform>/<name>/<file>` and returns both paths. */
function seedPlainInstall(platform, dirName, fileName) {
  const extractedDir = path.join(libraryPath, platform, dirName);
  mkdirSync(extractedDir, { recursive: true });
  const extractedPath = path.join(extractedDir, fileName);
  writeFileSync(extractedPath, `fake ${platform} rom bytes\n`);
  return { extractedDir, extractedPath };
}

const oldRom = seedPlainInstall('SNES', 'oldrom', 'old.sfc');
const currentRom = seedPlainInstall('SNES', 'current', 'game.sfc');
const ghostRom = seedPlainInstall('SNES', 'ghost', 'game.sfc');

const nativeGameDir = path.join(libraryPath, 'Windows', 'My Game');
const nativeExtractedDir = path.join(nativeGameDir, 'game');
const nativeExePath = path.join(nativeExtractedDir, 'MyGame', 'mygame.exe');
const nativeSavePath = path.join(nativeExtractedDir, 'saves', 'slot1.sav');
mkdirSync(path.dirname(nativeExePath), { recursive: true });
mkdirSync(path.dirname(nativeSavePath), { recursive: true });
writeFileSync(nativeExePath, 'MYGAME1\n');
writeFileSync(nativeSavePath, 'SAVE1');

// --- grid-launcher.db ----------------------------------------------------------

function sqlString(value) {
  return value.replace(/'/g, "''");
}

const installedAt = Math.floor(Date.now() / 1000);

const ROWS = [
  {
    romId: 801,
    title: 'Old Rom',
    platform: 'SNES',
    romFileName: 'oldrom.zip',
    serverUpdatedAt: '2025-01-01T00:00:00Z',
    extractedPath: oldRom.extractedPath,
    extractedDir: oldRom.extractedDir,
    nativeGameDir: '',
  },
  {
    romId: 802,
    title: 'My Game',
    platform: 'Windows',
    romFileName: 'mygame (v1.0.0).zip',
    serverUpdatedAt: '2026-01-01T00:00:00Z',
    extractedPath: nativeExePath,
    extractedDir: nativeExtractedDir,
    nativeGameDir,
  },
  {
    romId: 803,
    title: 'Current Rom',
    platform: 'SNES',
    romFileName: 'current.zip',
    serverUpdatedAt: '2026-01-01T00:00:00Z',
    extractedPath: currentRom.extractedPath,
    extractedDir: currentRom.extractedDir,
    nativeGameDir: '',
  },
  {
    romId: 804,
    title: 'Ghost Rom',
    platform: 'SNES',
    romFileName: 'ghost.zip',
    serverUpdatedAt: '2025-01-01T00:00:00Z',
    extractedPath: ghostRom.extractedPath,
    extractedDir: ghostRom.extractedDir,
    nativeGameDir: '',
  },
];

const inserts = ROWS.map((row) => {
  const values = [
    `'${sqlString(row.title)}'`,
    `'${sqlString(row.platform)}'`,
    `'${sqlString(row.title.trim().toLowerCase())}'`,
    `'${sqlString(row.platform.trim().toLowerCase())}'`,
    String(row.romId),
    `'${sqlString(row.romFileName)}'`,
    `'${sqlString(row.extractedPath)}'`,
    `'${sqlString(row.extractedDir)}'`,
    `'${sqlString(row.nativeGameDir)}'`,
    `'${sqlString(row.serverUpdatedAt)}'`,
    String(installedAt),
  ].join(', ');
  return `
INSERT INTO installed_games
  (title, platform, title_key, platform_key, rom_id, rom_file_name, extracted_path,
   extracted_dir, native_game_dir, server_updated_at, installed_at)
VALUES
  (${values});
`;
}).join('');

writeRegistry(path.join(dataDir, 'grid-launcher.db'), inserts);

console.log(`e2e: seeded updates stage data dir at ${dataDir}`);
