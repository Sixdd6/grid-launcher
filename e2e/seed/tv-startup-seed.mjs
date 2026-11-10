#!/usr/bin/env node
/**
 * Seeds the `tv-startup` stage group (TV mode plan § P12: the startup
 * screens). It runs before EVERY spec of the group (scripts/e2e.sh
 * `group_seeds_per_spec`), and every variant empties the data dir first, so
 * each spec starts on a profile of its own:
 *
 *   node tv-startup-seed.mjs <data-dir> tv-startup-a   # a fresh profile: Connect
 *   node tv-startup-seed.mjs <data-dir> tv-startup-b   # an unreachable library
 *   node tv-startup-seed.mjs <data-dir> tv-startup-c   # fresh, one Backup drive
 *   node tv-startup-seed.mjs <data-dir> tv-startup-d   # fresh again: Install N
 *   node tv-startup-seed.mjs <data-dir> tv-startup-e   # unreachable again: Close
 *
 * - a, d: no config at all; `<data>/library` exists for Connect's library
 *   folder (the spec pastes its path).
 * - b, e: `library-move-seed.mjs library-move-c` (a stored library root that
 *   does not exist, the files in `<data>/relocate-real`).
 * - c: as a, plus a fake Backup drive `<data>/media/COUCHDRIVE` holding the
 *   "Couch kit" pack with one game (e2e.sh points the drive probe at
 *   `<data>/media` for this group).
 */
import { execFileSync } from 'node:child_process';
import { mkdirSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const [dataDir, variant] = process.argv.slice(2);
if (!dataDir || !variant) {
  console.error('usage: tv-startup-seed.mjs <data-dir> <tv-startup-a|b|c|d|e>');
  process.exit(1);
}

const here = path.dirname(fileURLToPath(import.meta.url));

mkdirSync(dataDir, { recursive: true });
for (const name of readdirSync(dataDir)) rmSync(path.join(dataDir, name), { recursive: true, force: true });

function freshProfile() {
  mkdirSync(path.join(dataDir, 'library'), { recursive: true });
  mkdirSync(path.join(dataDir, 'media'), { recursive: true });
}

function couchDrive() {
  const drive = path.join(dataDir, 'media', 'COUCHDRIVE');
  const pack = path.join(drive, 'gridpack');
  mkdirSync(drive, { recursive: true });
  writeFileSync(path.join(drive, '.e2e-drive.json'), JSON.stringify({ label: 'COUCHDRIVE', fs: 'exfat' }));
  const rel = 'games/snes/Couch Quest.sfc';
  const data = Buffer.alloc(4096, 7);
  mkdirSync(path.dirname(path.join(pack, rel)), { recursive: true });
  writeFileSync(path.join(pack, rel), data);
  const stamp = '2026-11-01T12:00:00Z';
  writeFileSync(
    path.join(pack, 'gridpack.json'),
    JSON.stringify(
      {
        format: 1,
        name: 'Couch kit',
        os: ['linux'],
        created: stamp,
        updated: stamp,
        games: [
          {
            platform: 'snes',
            platform_name: 'Super Nintendo Entertainment System',
            source: { server: 'http://romm.e2e.invalid', rom_id: 901, fs_name: 'Couch Quest.sfc' },
            name: 'Couch Quest',
            summary: 'A game on the seeded Backup drive.',
            region: 'USA',
            size: data.length,
            files: [{ path: rel, size: data.length, server_file_id: 9011, name: 'Couch Quest.sfc', category: '' }],
            assets: {},
            added: stamp,
          },
        ],
        emulators: [],
        firmware: [],
      },
      null,
      2,
    ),
  );
}

switch (variant) {
  case 'tv-startup-a':
  case 'tv-startup-d':
    freshProfile();
    break;
  case 'tv-startup-b':
  case 'tv-startup-e':
    mkdirSync(path.join(dataDir, 'media'), { recursive: true });
    execFileSync(process.execPath, [path.join(here, 'library-move-seed.mjs'), dataDir, 'library-move-c'], { stdio: 'inherit' });
    break;
  case 'tv-startup-c':
    freshProfile();
    couchDrive();
    break;
  default:
    console.error(`tv-startup-seed.mjs: unknown variant ${variant}`);
    process.exit(1);
}

console.log(`e2e: seeded ${variant} in ${dataDir}`);
