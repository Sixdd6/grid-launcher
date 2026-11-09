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

function seedA() {
  mkdirSync(media, { recursive: true });
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

function seedC() {
  // A fresh profile: everything spec a and b left behind goes. The RomM
  // credential they saved in the run's private keyring is cleared by
  // scripts/e2e.sh before this seed runs (never from here: run by hand, a
  // seed would reach the developer's real keyring).
  for (const name of ['config.toml', 'grid-launcher.db', 'library', 'media', 'stubs', 'covers', 'cache']) {
    rmSync(path.join(dataDir, name), { recursive: true, force: true });
  }

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

switch (spec) {
  case 'backups-a':
    seedA();
    break;
  case 'backups-b':
    break;
  case 'backups-c':
    seedC();
    break;
  default:
    console.error(`backups-seed.mjs: unknown spec ${spec}`);
    process.exit(1);
}

console.log(`e2e: seeded ${spec} in ${dataDir}`);
