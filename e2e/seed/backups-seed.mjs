#!/usr/bin/env node
/**
 * Seeds the `backups` stage group's data dir before EACH spec
 * (`group_seeds_per_spec` in scripts/e2e.sh), as
 * `node backups-seed.mjs <data-dir> <spec base name>`.
 *
 * The fake drives live in `<data-dir>/media`, the folder scripts/e2e.sh
 * passes as `GRID_LAUNCHER_E2E_DRIVE_ROOTS`: every child folder is a mounted
 * drive (crates/grid-core/src/backup/drives.rs).
 *
 * - `backups-a`: a library folder and a PCSX2 entry installed from the
 *   server's Emulators platform (`romm:ps2/pcsx2 (playstation 2)`), set as
 *   PlayStation 2's emulator. A backup then carries the server's own PCSX2
 *   builds for Linux and Windows (fixtures-backups rom 951), so nothing goes
 *   to a forge. The media folder starts empty: the spec plugs drives in.
 * - `backups-b`: nothing new. It runs on spec a's profile and drive.
 * - `backups-c`: a fresh profile (no config, no registry, no library; e2e.sh
 *   clears the keyring item first) and one drive, TESTDRIVE, holding a ready-made gridpack:
 *   one PlayStation 2 game, PCSX2 for Linux (a zip of the mock forge's
 *   AppImage stub, a shell script the header check never refuses) and a
 *   PS2 BIOS.
 * - `backups-d`: a fresh profile again, spec a's PCSX2 setup, and three
 *   games installed in the library, each one copied from this computer by
 *   the backup: Cinder Peaks (rom 901, an extracted zip), Tux Trail (rom
 *   961, a Linux game with its `game.json`) and Moss and Marrow (rom 902, a
 *   multi-file game installed before the server's files changed, so it is
 *   out of date). TESTDRIVE is set up for Linux with an empty pack.
 */

import { createHash } from 'node:crypto';
import { chmodSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import path from 'node:path';

import { buildZip } from '../mock-romm/archives.mjs';
import { PCSX2_APPIMAGE_BYTES, PCSX2_ASSET_NAME } from '../mock-romm/mock-forge.mjs';
import { tomlString, writeRegistry } from './registry-schema.mjs';

const [dataDir, spec] = process.argv.slice(2);
if (!dataDir || !spec) {
  console.error('usage: backups-seed.mjs <data-dir> <spec base name>');
  process.exit(1);
}

const media = path.join(dataDir, 'media');
const library = path.join(dataDir, 'library');

/** The server PCSX2 entry's name and source, as a server install records them. */
export const PCSX2_NAME = 'PCSX2 (Playstation 2)';
export const PCSX2_SOURCE = 'romm:ps2/pcsx2 (playstation 2)';

/** A library folder, and PCSX2 from the server as PlayStation 2's emulator. */
function writeConfigWithPcsx2() {
  mkdirSync(library, { recursive: true });
  const stubDir = path.join(dataDir, 'stubs', 'pcsx2');
  mkdirSync(stubDir, { recursive: true });
  const stub = path.join(stubDir, PCSX2_ASSET_NAME);
  writeFileSync(stub, PCSX2_APPIMAGE_BYTES);
  chmodSync(stub, 0o755);
  writeFileSync(
    path.join(dataDir, 'config.toml'),
    `schema_version = 1
library_path = ${tomlString(library)}
library_layout_version = 2

[[emulators]]
name = ${tomlString(PCSX2_NAME)}
path = ${tomlString(stub)}
args = "%rom%"
source_id = ${tomlString(PCSX2_SOURCE)}

[default_emulators]
"PlayStation 2" = ${tomlString(PCSX2_NAME)}
`,
  );
}

function seedA() {
  mkdirSync(media, { recursive: true });
  writeConfigWithPcsx2();
  writeRegistry(path.join(dataDir, 'grid-launcher.db'));
}

/** Deterministic filler bytes. */
function bytes(length, seed) {
  const buf = Buffer.alloc(length);
  for (let i = 0; i < length; i++) buf[i] = (seed + i * 13) & 0xff;
  return buf;
}

const md5 = (buf) => createHash('md5').update(buf).digest('hex');

/** The seeded pack's one game, for backups-c.spec.ts. */
export const SEEDED_ROM_ID = 777;

/**
 * A fresh profile: everything the earlier specs left behind goes. The RomM
 * credential they saved in the run's private keyring is cleared by
 * scripts/e2e.sh before this seed runs (never from here: run by hand, a
 * seed would reach the developer's real keyring).
 */
function freshProfile() {
  for (const name of ['config.toml', 'grid-launcher.db', 'library', 'media', 'stubs', 'covers', 'cache']) {
    rmSync(path.join(dataDir, name), { recursive: true, force: true });
  }
}

function seedC() {
  freshProfile();

  const drive = path.join(media, 'TESTDRIVE');
  const pack = path.join(drive, 'gridpack');
  const put = (rel, data) => {
    const file = path.join(pack, rel);
    mkdirSync(path.dirname(file), { recursive: true });
    writeFileSync(file, data);
    return data.length;
  };
  mkdirSync(drive, { recursive: true });
  writeFileSync(path.join(drive, '.e2e-drive.json'), JSON.stringify({ label: 'TESTDRIVE', fs: 'exfat' }));

  // 48 MiB, so the copy off the drive lasts long enough for its
  // "Copying from TESTDRIVE" line to render; an ISO is copied, not extracted.
  const gameRel = 'games/ps2/Travel Game.iso';
  const gameSize = put(gameRel, bytes(48 * 1024 * 1024, 0x21));

  const emuAsset = 'PCSX2 (v2.0.2) (arch-linux-x64).zip';
  const emuRel = `emulators/PCSX2 (Playstation 2)/v2.0.2/linux-x86_64/${emuAsset}`;
  const emuZip = buildZip([{ name: `PCSX2/${PCSX2_ASSET_NAME}`, data: PCSX2_APPIMAGE_BYTES }]);
  const emuSize = put(emuRel, emuZip);

  const biosRel = 'firmware/ps2/ps2-bios.bin';
  const bios = bytes(512, 0x4b);
  put(biosRel, bios);

  const now = '2026-11-01T12:00:00Z';
  const index = {
    format: 1,
    name: 'Travel kit',
    os: ['linux'],
    created: now,
    updated: now,
    games: [
      {
        platform: 'ps2',
        platform_name: 'PlayStation 2',
        source: { server: 'http://romm.e2e.invalid', rom_id: SEEDED_ROM_ID, fs_name: 'Travel Game.iso' },
        name: 'Travel Game',
        summary: 'A game that only exists on the seeded backup drive.',
        region: 'USA',
        size: gameSize,
        files: [{ path: gameRel, size: gameSize, server_file_id: 7771, name: 'Travel Game.iso', category: '' }],
        assets: {},
        added: now,
      },
    ],
    emulators: [
      {
        name: PCSX2_NAME,
        kind: 'emulator',
        source_id: PCSX2_SOURCE,
        version: 'v2.0.2',
        os: 'linux-x86_64',
        platforms: ['ps2'],
        path: emuRel,
        asset_name: emuAsset,
        size: emuSize,
      },
    ],
    firmware: [
      {
        platform: 'ps2',
        platform_name: 'PlayStation 2',
        files: [{ name: 'ps2-bios.bin', path: biosRel, size: bios.length, md5: md5(bios) }],
      },
    ],
  };
  put('gridpack.json', Buffer.from(JSON.stringify(index, null, 2)));
}

/** SQL text literal. */
const sql = (value) => `'${String(value).replace(/'/g, "''")}'`;

/** The games backups-d.spec.ts backs up from this computer. */
export const LOCAL_GAMES = {
  zip: { romId: 901, title: 'Cinder Peaks' },
  native: { romId: 961, title: 'Tux Trail' },
  outdated: { romId: 902, title: 'Moss and Marrow' },
};
/** The fixture's `updated_at` for every rom of fixtures-backups. */
export const SERVER_UPDATED_AT = '2026-01-01T00:00:00Z';
/** Moss and Marrow's install-time server timestamp: older, so it is out of date. */
export const OUTDATED_UPDATED_AT = '2025-06-01T00:00:00Z';

function seedD() {
  freshProfile();
  writeConfigWithPcsx2();

  const drive = path.join(media, 'TESTDRIVE');
  mkdirSync(path.join(drive, 'gridpack'), { recursive: true });
  writeFileSync(path.join(drive, '.e2e-drive.json'), JSON.stringify({ label: 'TESTDRIVE', fs: 'exfat' }));
  const now = '2026-11-01T12:00:00Z';
  writeFileSync(
    path.join(drive, 'gridpack', 'gridpack.json'),
    JSON.stringify(
      { format: 1, name: 'TESTDRIVE', os: ['linux'], created: now, updated: now, games: [], emulators: [], firmware: [] },
      null,
      2,
    ),
  );

  const put = (file, data, mode) => {
    mkdirSync(path.dirname(file), { recursive: true });
    writeFileSync(file, data);
    if (mode !== undefined) chmodSync(file, mode);
  };
  const ps2 = path.join(library, 'games', 'PlayStation 2');
  const linux = path.join(library, 'games', 'Linux');

  // 1. Cinder Peaks: its zip, extracted into a folder named after it.
  const zipDir = path.join(ps2, 'Cinder Peaks');
  const zipLaunch = path.join(zipDir, 'Cinder Peaks.iso');
  put(zipLaunch, bytes(2 * 1024 * 1024, 0x31));
  put(path.join(zipDir, 'readme.txt'), Buffer.from('Cinder Peaks\n'));

  // 2. Tux Trail: a native home holding the extracted game/ tree and the
  //    server's game.json sidecar.
  const nativeDir = path.join(linux, 'Tux Trail');
  const nativeExe = path.join(nativeDir, 'game', 'tuxtrail.sh');
  put(nativeExe, Buffer.from('#!/bin/sh\nexit 0\n'), 0o755);
  put(path.join(nativeDir, 'game', 'data', 'level1.dat'), bytes(512 * 1024, 0x52));
  put(path.join(nativeDir, 'game.json'), Buffer.from(JSON.stringify({ version: '1.0', year: 2004, tags: ['indie'] })));

  // 3. Moss and Marrow: a multi-file game, the files as the server lists them.
  const multiDir = path.join(ps2, 'Moss and Marrow');
  put(path.join(multiDir, 'disc1.bin'), bytes(1024 * 1024, 0x61));
  put(path.join(multiDir, 'disc2.bin'), bytes(1024 * 1024, 0x62));
  const multiLaunch = path.join(multiDir, 'game.m3u');
  put(multiLaunch, Buffer.from('disc1.bin\ndisc2.bin\n'));

  const installedAt = Math.floor(Date.now() / 1000);
  const ps2Key = sql('playstation 2');
  // images_version = 1: no cover re-fetch for these rows.
  writeRegistry(
    path.join(dataDir, 'grid-launcher.db'),
    `
INSERT INTO installed_games
  (title, platform, title_key, platform_key, platform_slug, rom_id, rom_file_name,
   extracted_path, extracted_dir, multi_file_game_dir,
   native_game_dir, native_executable_path, server_updated_at, installed_at, images_version)
VALUES
  (${sql('Cinder Peaks')}, ${sql('PlayStation 2')}, ${sql('cinder peaks')}, ${ps2Key}, 'ps2', 901, 'Cinder Peaks.zip',
   ${sql(zipLaunch)}, ${sql(zipDir)}, '', '', '', ${sql(SERVER_UPDATED_AT)}, ${installedAt}, 1),
  (${sql('Tux Trail')}, 'Linux', ${sql('tux trail')}, 'linux', 'linux', 961, 'tuxtrail.zip',
   ${sql(nativeExe)}, ${sql(path.join(nativeDir, 'game'))}, '', ${sql(nativeDir)}, ${sql(nativeExe)},
   ${sql(SERVER_UPDATED_AT)}, ${installedAt}, 1),
  (${sql('Moss and Marrow')}, ${sql('PlayStation 2')}, ${sql('moss and marrow')}, ${ps2Key}, 'ps2', 902, 'Moss and Marrow',
   ${sql(multiLaunch)}, '', ${sql(multiDir)}, '', '', ${sql(OUTDATED_UPDATED_AT)}, ${installedAt}, 1);
`,
  );
}

switch (spec) {
  case 'backups-a':
    seedA();
    break;
  case 'backups-b':
    break;
  case 'backups-c':
    seedC();
    break;
  case 'backups-d':
    seedD();
    break;
  default:
    console.error(`backups-seed.mjs: unknown spec ${spec}`);
    process.exit(1);
}

console.log(`e2e: seeded ${spec} in ${dataDir}`);
