#!/usr/bin/env node
/**
 * Seeds the `install` and `downloads` stage groups' data dir before the app
 * starts: a config.toml that sets the default fixtures' systems (Super
 * Nintendo Entertainment System, Arcade) to None on purpose, and nothing
 * else (no library folder, no credential: Connect still asks for both).
 *
 * Details' Install brings a system's recommended emulator with the game when
 * the system has none (SPEC § Game Details View, user decision 2026-11-10).
 * These groups test the game's own download rows, so a system set to None
 * keeps that emulator install (from the real forge: these groups run no mock
 * forge) out of their queue.
 *
 * Invoked by scripts/e2e.sh's `seed_script_for_group` as
 * `node no-emulator-seed.mjs <data-dir>`.
 */
import { mkdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';

const dataDir = process.argv[2];
if (!dataDir) {
  console.error('usage: no-emulator-seed.mjs <data-dir>');
  process.exit(1);
}

mkdirSync(dataDir, { recursive: true });
writeFileSync(
  path.join(dataDir, 'config.toml'),
  `schema_version = 1
library_layout_version = 2

[default_emulators]
"Super Nintendo Entertainment System" = "<none>"
Arcade = "<none>"
`,
);
