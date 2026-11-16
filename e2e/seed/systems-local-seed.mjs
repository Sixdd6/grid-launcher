#!/usr/bin/env node
/**
 * Seeds the systems-local groups (romm-storefront-plan Step 7): a profile
 * that never connected RomM (welcomed, `stores.romm.connected` false, no
 * server), so the shell opens with no probe and Settings › Systems lists
 * GRID's own catalog.
 *
 * - DuckStation (a stub, named for its profile so its `bios` firmware folder
 *   resolves under `<stubs>/duckstation/bios`) is PlayStation's emulator,
 *   and one installed PlayStation game puts PlayStation in Your systems.
 * - `<data>/firmware-known.json` is a TEST table (E2E_FIRMWARE_KNOWN):
 *   three PlayStation BIOS names with the sizes and MD5s of the fake files
 *   below. Nothing here is real BIOS content or a real BIOS hash.
 * - `<data>/picks/` holds the files Locate files is pointed at: a renamed
 *   dump of the US file, a wrong file under the EU name, a zip holding the
 *   JP file, and a text file that matches nothing.
 */

import { createHash } from 'node:crypto';
import { chmodSync, mkdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';

import { tomlString, writeRegistry } from './registry-schema.mjs';

const dataDir = process.argv[2];
if (!dataDir) {
  console.error('usage: systems-local-seed.mjs <data-dir>');
  process.exit(1);
}

const DUCKSTATION = 'DuckStation (Playstation 1)';
const PLATFORM = 'PlayStation';

const US = Buffer.from('FAKE-PS1-BIOS-US-not-real\n'.repeat(4));
const EU = Buffer.from('FAKE-PS1-BIOS-EU-not-real\n'.repeat(4));
const JP = Buffer.from('FAKE-PS1-BIOS-JP-not-real\n'.repeat(4));
const md5 = (b) => createHash('md5').update(b).digest('hex');

const libraryPath = path.join(dataDir, 'library');
const duckDir = path.join(dataDir, 'stubs', 'duckstation');
const picks = path.join(dataDir, 'picks');
mkdirSync(libraryPath, { recursive: true });
mkdirSync(duckDir, { recursive: true });
mkdirSync(path.join(picks, 'more'), { recursive: true });

const duck = path.join(duckDir, 'duckstation-qt');
writeFileSync(duck, '#!/bin/sh\nexit 0\n');
chmodSync(duck, 0o755);

writeFileSync(
  path.join(dataDir, 'config.toml'),
  `schema_version = 1
library_path = ${tomlString(libraryPath)}
library_layout_version = 2
romm_store_migrated = true

[setup]
completed = true
welcomed = true

[[emulators]]
name = ${tomlString(DUCKSTATION)}
path = ${tomlString(duck)}
args = "%rom%"

[default_emulators]
${tomlString(PLATFORM)} = ${tomlString(DUCKSTATION)}
`,
);

const entry = (name, data, required) => ({
  slug: 'psx',
  name,
  path_hint: '',
  size: data.length,
  md5: md5(data),
  sha1: '',
  crc: '',
  required,
  group: 'psx-bios',
  match: 'content',
  profiles: [],
  source: 'e2e',
});
writeFileSync(
  path.join(dataDir, 'firmware-known.json'),
  JSON.stringify(
    {
      _comment: 'e2e test table: fake files, fake hashes (seed/systems-local-seed.mjs)',
      files: [entry('scph5500.bin', JP, false), entry('scph5501.bin', US, true), entry('scph5502.bin', EU, false)],
    },
    null,
    1,
  ),
);

// --- The picks folder --------------------------------------------------------

const CRC_TABLE = Array.from({ length: 256 }, (_, n) => {
  let c = n;
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  return c >>> 0;
});
function crc32(buf) {
  let c = 0xffffffff;
  for (const b of buf) c = CRC_TABLE[(c ^ b) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

/** A one-member zip, stored (no compression). */
function zipOne(name, data) {
  const n = Buffer.from(name);
  const crc = crc32(data);
  const local = Buffer.alloc(30);
  local.writeUInt32LE(0x04034b50, 0);
  local.writeUInt16LE(20, 4);
  local.writeUInt32LE(crc, 14);
  local.writeUInt32LE(data.length, 18);
  local.writeUInt32LE(data.length, 22);
  local.writeUInt16LE(n.length, 26);
  const central = Buffer.alloc(46);
  central.writeUInt32LE(0x02014b50, 0);
  central.writeUInt16LE(20, 4);
  central.writeUInt16LE(20, 6);
  central.writeUInt32LE(crc, 16);
  central.writeUInt32LE(data.length, 20);
  central.writeUInt32LE(data.length, 24);
  central.writeUInt16LE(n.length, 28);
  const centralOffset = local.length + n.length + data.length;
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0);
  end.writeUInt16LE(1, 8);
  end.writeUInt16LE(1, 10);
  end.writeUInt32LE(central.length + n.length, 12);
  end.writeUInt32LE(centralOffset, 16);
  return Buffer.concat([local, n, data, central, n, end]);
}

writeFileSync(path.join(picks, 'scph5501 (1).bin'), US);
writeFileSync(path.join(picks, 'scph5502.bin'), Buffer.from('FAKE-PS1-BIOS-XX-not-real\n'));
writeFileSync(path.join(picks, 'bios-pack.zip'), zipOne('scph5500.bin', JP));
writeFileSync(path.join(picks, 'readme.txt'), 'notes\n');
writeFileSync(path.join(picks, 'more', 'notes.txt'), 'more notes\n');

// --- One installed PlayStation game ----------------------------------------------

const romDir = path.join(libraryPath, 'games', 'PlayStation', 'Lantern Vale');
mkdirSync(romDir, { recursive: true });
const rom = path.join(romDir, 'Lantern Vale.cue');
writeFileSync(rom, 'FILE "Lantern Vale.bin" BINARY\n');
const q = (s) => `'${String(s).replace(/'/g, "''")}'`;
writeRegistry(
  path.join(dataDir, 'grid-launcher.db'),
  `INSERT INTO installed_games
  (title, platform, title_key, platform_key, platform_slug, rom_id, rom_file_name, extracted_path, extracted_dir, installed_at, images_version)
VALUES
  (${q('Lantern Vale')}, ${q(PLATFORM)}, ${q('lantern vale')}, ${q('playstation')}, 'psx', 501, 'Lantern Vale.cue', ${q(rom)}, ${q(romDir)}, ${Math.floor(Date.now() / 1000)}, 1);`,
);

console.log(`e2e: seeded systems-local stage data dir at ${dataDir}`);
