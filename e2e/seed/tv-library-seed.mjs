#!/usr/bin/env node
/**
 * Seeds the `tv-library` stage group's data dir before the app starts (TV
 * mode plan § P8): a profile used without a server, three installed games
 * on two platforms, and one Backup drive, TESTDRIVE, holding the "Travel
 * kit" pack with two games: Travel Game (PlayStation 2, not installed, 48
 * MiB so its copy shows a percent) and Alpha Quest (installed here), plus
 * PCSX2 for Linux and a PS2 BIOS, like backups-seed.mjs's backups-c.
 *
 * - Alpha Quest (rom 101, SNES), played 2 days ago, installed 100 s ago
 * - Bravo Run (rom 102, SNES), never played, installed 50 s ago
 * - Delta Drive (rom 103, Sega Genesis), played 40 days ago, installed 10 s ago
 *
 * Invoked by scripts/e2e.sh's `seed_script_for_group` as
 * `node tv-library-seed.mjs <data-dir>`; e2e.sh points the fake drive probe
 * at `<data-dir>/media` for this group.
 */
import { createHash } from 'node:crypto';
import { mkdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';

import { buildZip } from '../mock-romm/archives.mjs';
import { PCSX2_APPIMAGE_BYTES, PCSX2_ASSET_NAME } from '../mock-romm/mock-forge.mjs';
import { tomlString, writeRegistry } from './registry-schema.mjs';

const dataDir = process.argv[2];
if (!dataDir) {
  console.error('usage: tv-library-seed.mjs <data-dir>');
  process.exit(1);
}

const library = path.join(dataDir, 'library');
const media = path.join(dataDir, 'media');
mkdirSync(library, { recursive: true });
mkdirSync(media, { recursive: true });

writeFileSync(
  path.join(dataDir, 'config.toml'),
  `schema_version = 1
library_path = ${tomlString(library)}
library_layout_version = 2
offline = true
`,
);

const sql = (value) => `'${String(value).replace(/'/g, "''")}'`;
const now = Math.floor(Date.now() / 1000);
const DAY = 24 * 60 * 60;

function bytes(length, seed) {
  const buf = Buffer.alloc(length);
  for (let i = 0; i < length; i++) buf[i] = (seed + i * 13) & 0xff;
  return buf;
}

const GAMES = [
  { rom: 101, title: 'Alpha Quest', platform: 'Super Nintendo Entertainment System', slug: 'snes', ext: 'sfc', played: now - 2 * DAY, installed: now - 100 },
  { rom: 102, title: 'Bravo Run', platform: 'Super Nintendo Entertainment System', slug: 'snes', ext: 'sfc', played: 0, installed: now - 50 },
  { rom: 103, title: 'Delta Drive', platform: 'Sega Genesis', slug: 'genesis', ext: 'md', played: now - 40 * DAY, installed: now - 10 },
];

const values = GAMES.map((g) => {
  const dir = path.join(library, 'games', g.platform, g.title);
  const file = path.join(dir, `${g.title}.${g.ext}`);
  mkdirSync(dir, { recursive: true });
  writeFileSync(file, bytes(4096, g.rom));
  return `(${sql(g.title)}, ${sql(g.platform)}, ${sql(g.title.toLowerCase())}, ${sql(g.platform.toLowerCase())}, ${sql(g.slug)}, ${g.rom}, ${sql(path.basename(file))},
   ${sql(file)}, '', '', '', '', '2026-01-01T00:00:00Z', ${g.installed}, ${g.played}, 1)`;
});

writeRegistry(
  path.join(dataDir, 'grid-launcher.db'),
  `
INSERT INTO installed_games
  (title, platform, title_key, platform_key, platform_slug, rom_id, rom_file_name,
   extracted_path, extracted_dir, multi_file_game_dir,
   native_game_dir, native_executable_path, server_updated_at, installed_at, last_played_at, images_version)
VALUES
  ${values.join(',\n  ')};
`,
);

// The drive and its pack.
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

const travelRel = 'games/ps2/Travel Game.iso';
const travelSize = put(travelRel, bytes(48 * 1024 * 1024, 0x21));
const alphaRel = 'games/snes/Alpha Quest.sfc';
const alphaSize = put(alphaRel, bytes(4096, 101));

const emuAsset = 'PCSX2 (v2.0.2) (arch-linux-x64).zip';
const emuRel = `emulators/PCSX2 (Playstation 2)/v2.0.2/linux-x86_64/${emuAsset}`;
const emuSize = put(emuRel, buildZip([{ name: `PCSX2/${PCSX2_ASSET_NAME}`, data: PCSX2_APPIMAGE_BYTES }]));
const biosRel = 'firmware/ps2/ps2-bios.bin';
const bios = bytes(512, 0x4b);
put(biosRel, bios);

const stamp = '2026-11-01T12:00:00Z';
const SERVER = 'http://romm.e2e.invalid';
put(
  'gridpack.json',
  Buffer.from(
    JSON.stringify(
      {
        format: 1,
        name: 'Travel kit',
        os: ['linux'],
        created: stamp,
        updated: stamp,
        games: [
          {
            platform: 'ps2',
            platform_name: 'PlayStation 2',
            source: { server: SERVER, rom_id: 777, fs_name: 'Travel Game.iso' },
            name: 'Travel Game',
            summary: 'A game that only exists on the seeded backup drive.',
            region: 'USA',
            size: travelSize,
            files: [{ path: travelRel, size: travelSize, server_file_id: 7771, name: 'Travel Game.iso', category: '' }],
            assets: {},
            added: stamp,
          },
          {
            platform: 'snes',
            platform_name: 'Super Nintendo Entertainment System',
            source: { server: SERVER, rom_id: 101, fs_name: 'Alpha Quest.sfc' },
            name: 'Alpha Quest',
            summary: 'Installed here and on the drive.',
            region: 'USA',
            size: alphaSize,
            files: [{ path: alphaRel, size: alphaSize, server_file_id: 1011, name: 'Alpha Quest.sfc', category: '' }],
            assets: {},
            added: stamp,
          },
        ],
        emulators: [
          {
            name: 'PCSX2 (Playstation 2)',
            kind: 'emulator',
            source_id: 'romm:ps2/pcsx2 (playstation 2)',
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
            files: [{ name: 'ps2-bios.bin', path: biosRel, size: bios.length, md5: createHash('md5').update(bios).digest('hex') }],
          },
        ],
      },
      null,
      2,
    ),
  ),
);

console.log(`e2e: seeded tv-library in ${dataDir}`);
