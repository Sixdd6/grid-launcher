#!/usr/bin/env node
/**
 * Seeds the `tv-settings` stage group's data dir before the app starts: a
 * library folder and, beside it, an empty `library-new` that Settings ›
 * Library › Change library folder… picks through the TV folder browser.
 * The spec connects through the desktop's Connect form (TV mode's startup
 * screens are P12's) and enters TV mode with the top bar's TV button.
 *
 * Invoked by scripts/e2e.sh's `seed_script_for_group` as
 * `node tv-settings-seed.mjs <data-dir>`.
 */
import { mkdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';

const dataDir = process.argv[2];
if (!dataDir) {
  console.error('usage: tv-settings-seed.mjs <data-dir>');
  process.exit(1);
}

const library = path.join(dataDir, 'library');
mkdirSync(library, { recursive: true });
mkdirSync(path.join(dataDir, 'library-new'), { recursive: true });

writeFileSync(
  path.join(dataDir, 'config.toml'),
  `schema_version = 1
library_path = ${JSON.stringify(library)}
library_layout_version = 2
`,
);
