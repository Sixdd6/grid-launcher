#!/usr/bin/env node
/**
 * Seeds the `gog` stage group's data dir before each spec: a fresh profile
 * used without a server (`offline = true` and a library folder), so the app
 * starts straight into the shell, with GOG not connected and no store files.
 * `stubs/bin` holds `wine` and `umu-run` stand-ins that run the fake GOG
 * installer (e2e/fake-tools/installer.mjs); e2e.sh puts it first on PATH.
 *
 * Invoked by scripts/e2e.sh as `node gog-seed.mjs <data-dir> <spec name>`.
 */
import { chmodSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const dataDir = process.argv[2];
if (!dataDir) {
  console.error('usage: gog-seed.mjs <data-dir> [spec]');
  process.exit(1);
}

// Each spec starts from nothing: no config, no db, no store files.
rmSync(dataDir, { recursive: true, force: true });
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

const stubs = path.join(dataDir, 'stubs', 'bin');
mkdirSync(stubs, { recursive: true });
const installer = path.join(here, '..', 'fake-tools', 'installer.mjs');
for (const name of ['wine', 'umu-run']) {
  const file = path.join(stubs, name);
  writeFileSync(file, `#!/bin/sh\nexec node ${JSON.stringify(installer)} "$@"\n`);
  chmodSync(file, 0o755);
}
