#!/usr/bin/env node
/**
 * Seeds the `tv-romm` stage group (romm-storefront-plan Step 6): a profile
 * whose welcome is not done yet, with only a library folder stored, so the
 * TV welcome shows step 1 with the folder filled in. No RomM server, no
 * credential (e2e.sh clears the run's keyring before the spec).
 *
 * Invoked by scripts/e2e.sh's `seed_script_for_group` as
 * `node tv-romm-seed.mjs <data-dir>`.
 */
import { mkdirSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import path from 'node:path';

const dataDir = process.argv[2];
if (!dataDir) {
  console.error('usage: tv-romm-seed.mjs <data-dir>');
  process.exit(1);
}

mkdirSync(dataDir, { recursive: true });
for (const name of readdirSync(dataDir)) rmSync(path.join(dataDir, name), { recursive: true, force: true });
const library = path.join(dataDir, 'library');
mkdirSync(library, { recursive: true });

writeFileSync(
  path.join(dataDir, 'config.toml'),
  `schema_version = 1
library_path = ${JSON.stringify(library)}
library_layout_version = 2
`,
);
