#!/usr/bin/env node
/**
 * Seeds the `tv-shell` stage group's data dir before the app starts: a
 * profile used without a server ("Use without a server": `offline = true`
 * and a library folder), so the app restores straight into the shell with no
 * credential, in TV mode (`--tv`) or the desktop. TV mode's startup screens
 * are P12's; the shell is what this group exercises.
 *
 * Invoked by scripts/e2e.sh's `seed_script_for_group` as
 * `node tv-shell-seed.mjs <data-dir>`.
 */
import { mkdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';

const dataDir = process.argv[2];
if (!dataDir) {
  console.error('usage: tv-shell-seed.mjs <data-dir>');
  process.exit(1);
}

const library = path.join(dataDir, 'library');
mkdirSync(library, { recursive: true });

const tomlString = (s) => JSON.stringify(s);

writeFileSync(
  path.join(dataDir, 'config.toml'),
  `schema_version = 1
library_path = ${tomlString(library)}
library_layout_version = 2
offline = true
`,
);
