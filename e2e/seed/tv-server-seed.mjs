#!/usr/bin/env node
/**
 * Seeds the `tv-server` stage group's data dir before the app starts: a
 * library folder and a default emulator for the Discover fixtures' SNES
 * platform, so Server › Discover's New releases has games (the preferred
 * platforms are the ones with a default emulator). The spec connects
 * through the desktop's Connect form (TV mode's startup screens are P12's)
 * — `connect` keeps everything here and only sets the server and user —
 * then enters TV mode with the top bar's TV button.
 *
 * Invoked by scripts/e2e.sh's `seed_script_for_group` as
 * `node tv-server-seed.mjs <data-dir>`.
 */
import { mkdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';

const dataDir = process.argv[2];
if (!dataDir) {
  console.error('usage: tv-server-seed.mjs <data-dir>');
  process.exit(1);
}

const library = path.join(dataDir, 'library');
mkdirSync(library, { recursive: true });

writeFileSync(
  path.join(dataDir, 'config.toml'),
  `schema_version = 1
library_path = ${JSON.stringify(library)}
library_layout_version = 2

[default_emulators]
"Super Nintendo Entertainment System" = "snes9x"
`,
);
