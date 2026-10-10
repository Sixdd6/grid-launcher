#!/usr/bin/env node
/**
 * Seeds the `library-move` stage group (9b, Settings › Library › Move
 * existing files). Unlike the other seeds this one runs before EVERY spec of
 * the group (scripts/e2e.sh `group_seeds_per_spec`), because each spec needs
 * a different starting state:
 *
 *   node library-move-seed.mjs <data-dir> library-move-a   # a move to run
 *   node library-move-seed.mjs <data-dir> library-move-b   # a move to resume
 *   node library-move-seed.mjs <data-dir> library-move-c   # an unreachable root
 *
 * Every variant writes a fresh `config.toml` and `grid-launcher.db` (the
 * earlier spec's are replaced) and uses library folders of its own, so what
 * an earlier spec left on disk is never read.
 *
 * The library is the current layout (v2): `games/`, `emulators/`, `saves/`.
 * - `games/Super Nintendo Entertainment System/Super Mario World/game.sfc`,
 *   the registry row of rom 101 (the mock's default fixture set has it).
 * - `emulators/Stub Runner/stub-runner.sh`, the SNES default: it writes its
 *   argv to `<data-dir>/library-move-argv.log` and exits after a second.
 * - `emulators/PCSX2 (Playstation 2)/pcsx2-qt` with `inis` and `memcards`
 *   as relative links into `saves/PCSX2 (Playstation 2)/` (the user-data
 *   links), and `saves/.../inis/PCSX2.ini` whose `[Folders] Bios` is an
 *   absolute library path.
 * - `emulators/Xemu (Xbox)/xemu` with `xemu.toml` beside it, whose
 *   `[sys.files]` keys are absolute library paths (single-quoted).
 * - `notes.txt` at the library root: not a library folder, so it stays.
 *
 * Variant b adds a Copy-phase move record (`library-move.json`) and a
 * partial staging folder under the new root. Variant c stores a library
 * root that does not exist while the files are in another folder.
 */

import {
  chmodSync,
  lstatSync,
  mkdirSync,
  readdirSync,
  realpathSync,
  rmSync,
  symlinkSync,
  utimesSync,
  writeFileSync,
} from 'node:fs';
import path from 'node:path';

import { tomlString, writeRegistry } from './registry-schema.mjs';

const [dataDir, variant] = process.argv.slice(2);
if (!dataDir || !variant) {
  console.error('usage: library-move-seed.mjs <data-dir> <library-move-a|library-move-b|library-move-c>');
  process.exit(1);
}

// Shared with e2e/helpers/libraryMove.ts; keep both in step.
const PLATFORM = 'Super Nintendo Entertainment System';
const TITLE = 'Super Mario World';
const ROM_ID = 101;
const STUB = 'Stub Runner';
const PCSX2 = 'PCSX2 (Playstation 2)';
const XEMU = 'Xemu (Xbox)';
const ARGV_FILE = path.join(dataDir, 'library-move-argv.log');

const ROOTS = {
  'library-move-a': { old: 'library', new: 'library-new' },
  'library-move-b': { old: 'resume-old', new: 'resume-new' },
  'library-move-c': { old: 'relocate-missing', new: 'relocate-real' },
};
const roots = ROOTS[variant];
if (!roots) {
  console.error(`library-move-seed: unknown variant ${variant}`);
  process.exit(1);
}
const oldRoot = path.join(dataDir, roots.old);
const newRoot = path.join(dataDir, roots.new);

// --- reset ------------------------------------------------------------------

for (const name of [
  'config.toml',
  'grid-launcher.db',
  'grid-launcher.db-wal',
  'grid-launcher.db-shm',
  'grid-launcher.db-journal',
  'library-move.json',
  'library-move.json.tmp',
  'library-move-argv.log',
]) {
  rmSync(path.join(dataDir, name), { force: true });
}
rmSync(oldRoot, { recursive: true, force: true });
rmSync(newRoot, { recursive: true, force: true });

// --- the library tree ---------------------------------------------------------

/**
 * Writes the library at `root`. Every absolute path written INTO a file
 * (the ini, the toml) names `storedRoot`, the root the config stores: the
 * same folder for variants a and b, the missing one for variant c.
 */
function writeLibrary(root, storedRoot) {
  const gameDir = path.join(root, 'games', PLATFORM, TITLE);
  mkdirSync(gameDir, { recursive: true });
  writeFileSync(path.join(gameDir, 'game.sfc'), 'fake snes rom bytes\n');

  const stubDir = path.join(root, 'emulators', STUB);
  mkdirSync(stubDir, { recursive: true });
  const stub = path.join(stubDir, 'stub-runner.sh');
  writeFileSync(stub, `#!/bin/sh\nprintf '%s\\n' "$@" > '${ARGV_FILE}'\nsleep 1\nexit 0\n`);
  chmodSync(stub, 0o755);

  const pcsx2Dir = path.join(root, 'emulators', PCSX2);
  const pcsx2Saves = path.join(root, 'saves', PCSX2);
  mkdirSync(pcsx2Dir, { recursive: true });
  mkdirSync(path.join(pcsx2Saves, 'inis'), { recursive: true });
  mkdirSync(path.join(pcsx2Saves, 'memcards'), { recursive: true });
  mkdirSync(path.join(pcsx2Saves, 'bios'), { recursive: true });
  const pcsx2 = path.join(pcsx2Dir, 'pcsx2-qt');
  writeFileSync(pcsx2, '#!/bin/sh\nexit 0\n');
  chmodSync(pcsx2, 0o755);
  writeFileSync(path.join(pcsx2Saves, 'memcards', 'Mcd001.ps2'), 'MEMCARD1\n');
  writeFileSync(
    path.join(pcsx2Saves, 'inis', 'PCSX2.ini'),
    `[UI]\nSettingsVersion = 1\n\n[Folders]\nBios = ${path.join(storedRoot, 'saves', PCSX2, 'bios')}\n`,
  );
  // Relative, as grid-core makes them on unix (user_data_links.rs).
  for (const name of ['inis', 'memcards']) {
    symlinkSync(path.join('..', '..', 'saves', PCSX2, name), path.join(pcsx2Dir, name));
  }

  const xemuDir = path.join(root, 'emulators', XEMU);
  const xemuSaves = path.join(root, 'saves', XEMU);
  mkdirSync(xemuDir, { recursive: true });
  mkdirSync(xemuSaves, { recursive: true });
  const xemu = path.join(xemuDir, 'xemu');
  writeFileSync(xemu, '#!/bin/sh\nexit 0\n');
  chmodSync(xemu, 0o755);
  writeFileSync(path.join(xemuSaves, 'xbox_hdd.qcow2'), 'HDD IMAGE\n');
  writeFileSync(path.join(xemuSaves, 'eeprom.bin'), 'EEPROM\n');
  const storedXemu = path.join(storedRoot, 'emulators', XEMU);
  const storedXemuSaves = path.join(storedRoot, 'saves', XEMU);
  writeFileSync(
    path.join(xemuDir, 'xemu.toml'),
    [
      '[sys.files]',
      `bootrom_path = '${path.join(storedXemu, 'mcpx_1.0.bin')}'`,
      `flashrom_path = '${path.join(storedXemu, 'complex_4627.bin')}'`,
      `hdd_path = '${path.join(storedXemuSaves, 'xbox_hdd.qcow2')}'`,
      `eeprom_path = '${path.join(storedXemuSaves, 'eeprom.bin')}'`,
      '',
    ].join('\n'),
  );

  writeFileSync(path.join(root, 'notes.txt'), 'not part of the library\n');
}

// --- config.toml and grid-launcher.db -------------------------------------------

function writeConfig(storedRoot) {
  const emu = (name, file) => tomlString(path.join(storedRoot, 'emulators', name, file));
  // No server_url: every spec connects through the Connect form.
  const configToml = `schema_version = 1
library_path = ${tomlString(storedRoot)}
library_layout_version = 2

[[emulators]]
name = ${tomlString(STUB)}
path = ${emu(STUB, 'stub-runner.sh')}
args = "%rom%"

[[emulators]]
name = ${tomlString(PCSX2)}
path = ${emu(PCSX2, 'pcsx2-qt')}
args = "%rom%"

[[emulators]]
name = ${tomlString(XEMU)}
path = ${emu(XEMU, 'xemu')}
args = "%rom%"

[default_emulators]
${tomlString(PLATFORM)} = ${tomlString(STUB)}
`;
  writeFileSync(path.join(dataDir, 'config.toml'), configToml);
}

function sqlString(value) {
  return value.replace(/'/g, "''");
}

function writeDb(storedRoot) {
  const gameDir = path.join(storedRoot, 'games', PLATFORM, TITLE);
  const romPath = path.join(gameDir, 'game.sfc');
  const installedAt = Math.floor(Date.now() / 1000);
  // images_version = 1: no cover re-fetch for this row.
  writeRegistry(
    path.join(dataDir, 'grid-launcher.db'),
    `
INSERT INTO installed_games
  (title, platform, title_key, platform_key, rom_id, rom_file_name,
   extracted_path, extracted_dir, installed_at, images_version)
VALUES
  ('${sqlString(TITLE)}', '${sqlString(PLATFORM)}', '${sqlString(TITLE.toLowerCase())}',
   '${sqlString(PLATFORM.toLowerCase())}', ${ROM_ID}, 'game.sfc',
   '${sqlString(romPath)}', '${sqlString(gameDir)}', ${installedAt}, 1);
`,
  );
}

// --- variant b: the interrupted Copy phase ---------------------------------------

/** Bytes and regular files under `dir`, links not followed (tree_size). */
function treeSize(dir) {
  let bytes = 0;
  let files = 0;
  for (const name of readdirSync(dir)) {
    const p = path.join(dir, name);
    const meta = lstatSync(p);
    if (meta.isSymbolicLink()) continue;
    if (meta.isDirectory()) {
      const inner = treeSize(p);
      bytes += inner.bytes;
      files += inner.files;
    } else if (meta.isFile()) {
      bytes += meta.size;
      files += 1;
    }
  }
  return { bytes, files };
}

function writeResumeState() {
  mkdirSync(newRoot, { recursive: true });
  const staging = path.join(newRoot, '.grid-library-move');

  // A copy the crash cut short: shorter than the source, so the resume
  // copies it again.
  const partialGame = path.join(staging, 'games', PLATFORM, TITLE, 'game.sfc');
  mkdirSync(path.dirname(partialGame), { recursive: true });
  writeFileSync(partialGame, 'fake snes');

  // A finished copy: same length and modified time as its source, so the
  // resume keeps it.
  const when = new Date('2026-01-02T03:04:05Z');
  const source = path.join(oldRoot, 'saves', PCSX2, 'memcards', 'Mcd001.ps2');
  const copied = path.join(staging, 'saves', PCSX2, 'memcards', 'Mcd001.ps2');
  mkdirSync(path.dirname(copied), { recursive: true });
  writeFileSync(copied, 'MEMCARD1\n');
  utimesSync(source, when, when);
  utimesSync(copied, when, when);

  // Shape of relocate.rs `Journal` (JOURNAL_VERSION 1).
  const journal = {
    version: 1,
    old_root: oldRoot,
    new_root: newRoot,
    old_canonical: realpathSync(oldRoot),
    new_canonical: realpathSync(newRoot),
    created_new_root: true,
    phase: 'copy',
    items: ['games', 'emulators', 'saves'].map((name) => ({
      name,
      strategy: 'copy',
      state: 'pending',
      ...treeSize(path.join(oldRoot, name)),
      keep_source: false,
      source_digest: '',
    })),
    links: [],
    commit_steps: 0,
    report: [],
  };
  writeFileSync(path.join(dataDir, 'library-move.json'), `${JSON.stringify(journal, null, 2)}\n`);
}

// --- main -----------------------------------------------------------------------

switch (variant) {
  case 'library-move-a':
    writeLibrary(oldRoot, oldRoot);
    writeConfig(oldRoot);
    writeDb(oldRoot);
    break;
  case 'library-move-b':
    writeLibrary(oldRoot, oldRoot);
    writeConfig(oldRoot);
    writeDb(oldRoot);
    writeResumeState();
    break;
  case 'library-move-c':
    // The stored root is missing; the files are in `newRoot`.
    writeLibrary(newRoot, oldRoot);
    writeConfig(oldRoot);
    writeDb(oldRoot);
    break;
}

console.log(`e2e: seeded ${variant} in ${dataDir} (old ${oldRoot}, new ${newRoot})`);
