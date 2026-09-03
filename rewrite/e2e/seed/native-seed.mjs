#!/usr/bin/env node
/**
 * Seeds the `native` stage group's temp data dir BEFORE the app starts:
 * a library path, `default_compat_tool = "wine"`, an empty registry at the
 * current schema version, and a stub `wine` on the app's PATH.
 *
 * The stub lives at `<data>/stubs/bin/wine` because that exact directory is
 * the contract with the runner: rewrite/scripts/e2e.sh exports
 * `E2E_STUB_BIN="<data>/stubs/bin"` when it exists after seeding, and
 * wdio.conf.ts prepends it to the app process's `PATH`. That is what
 * `build_native_command`'s `which("wine")` (grid-core/src/launch/native.rs)
 * resolves — a native launch with `default_compat_tool = "wine"` runs
 * `wine <exe> <params...>`, so this stub IS the launch as far as the spec
 * can see.
 *
 * It records its argv one-per-line to `<data>/wine-argv.log` and then sleeps,
 * the same shape `launch-seed.mjs`'s `long-runner.sh` uses: the sleep keeps
 * the session alive long enough for the spec to observe the playing state
 * and press Stop, and the log is what proves the executable path and the
 * saved launch parameters both reached the command line. Appended (`>>`),
 * not overwritten, so a second launch cannot erase the evidence of the
 * first.
 *
 * No emulator entries: a Windows-platform game launches through the native
 * path, which consults `default_compat_tool` and never
 * `[default_emulators]`.
 */

import { chmodSync, mkdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';

import { tomlString, writeRegistry } from './registry-schema.mjs';

const dataDir = process.argv[2];
if (!dataDir) {
  console.error('usage: native-seed.mjs <data-dir>');
  process.exit(1);
}

const libraryPath = path.join(dataDir, 'library');
const stubBinDir = path.join(dataDir, 'stubs', 'bin');
mkdirSync(libraryPath, { recursive: true });
mkdirSync(stubBinDir, { recursive: true });

const argvLog = path.join(dataDir, 'wine-argv.log');
const wineStub = path.join(stubBinDir, 'wine');
writeFileSync(wineStub, `#!/bin/sh\nprintf '%s\\n' "$@" >> '${argvLog}'\nsleep 30\n`);
chmodSync(wineStub, 0o755);

const configToml = `schema_version = 1
library_path = ${tomlString(libraryPath)}
default_compat_tool = "wine"
`;
writeFileSync(path.join(dataDir, 'config.toml'), configToml);

writeRegistry(path.join(dataDir, 'grid-launcher.db'));

console.log(`e2e: seeded native stage data dir at ${dataDir}`);
