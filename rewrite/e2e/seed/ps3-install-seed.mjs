#!/usr/bin/env node
/**
 * Seeds the `ps3-install` stage group's temp data dir BEFORE the app starts.
 *
 * This group deliberately configures NO emulator: it exercises the
 * "no RPCS3 anywhere" fallback in `ps3_roots_from_config`
 * (grid-core/src/library/mod.rs), which routes a PS3 install into
 * `<library>/PlayStation 3/.vfs/dev_hdd0`. ps3-install.spec.ts sets the
 * library path through the UI, exactly like install-a.spec.ts does, so this
 * script writes no `config.toml` either.
 *
 * What it does write:
 *
 * - an EMPTY `grid-launcher.db` at the current schema version, so the app
 *   opens a registry it recognizes rather than creating one (see
 *   registry-schema.mjs for why the copy has to stay in sync);
 * - an empty `xdg-config/` directory. That one is a hermeticity guard, not
 *   a fixture: with no emulator entry to resolve against,
 *   `readers::rpcs3_data_root_candidates` probes `$RPCS3_CONFIG_DIR` and
 *   `$XDG_CONFIG_HOME/rpcs3` for a `vfs.yml` BEFORE it reaches the library
 *   fallback. On a developer machine with a real RPCS3 installed, the
 *   install would then be routed into that person's own `dev_hdd0`.
 *   rewrite/scripts/e2e.sh exports this directory as `E2E_XDG_CONFIG_HOME`
 *   when it exists, and wdio.conf.ts points both variables at it, so the
 *   probe finds nothing and the fallback is reached.
 */

import { mkdirSync } from 'node:fs';
import path from 'node:path';

import { writeRegistry } from './registry-schema.mjs';

const dataDir = process.argv[2];
if (!dataDir) {
  console.error('usage: ps3-install-seed.mjs <data-dir>');
  process.exit(1);
}

mkdirSync(path.join(dataDir, 'xdg-config'), { recursive: true });
writeRegistry(path.join(dataDir, 'grid-launcher.db'));

console.log(`e2e: seeded ps3-install stage data dir at ${dataDir}`);
