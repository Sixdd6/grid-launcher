#!/usr/bin/env node
/**
 * Seeds the `content` stage group's temp data dir BEFORE the app starts:
 * a library path, an empty registry at the current schema version, and one
 * stub Xenia executable set as the "Xbox 360" default.
 *
 * Why the stub is named `xenia_edge`, in a directory named `xenia-edge`,
 * with a `portable.txt` beside it — all three matter:
 *
 * - `profile_for_entry` (grid-core/src/launch/profiles.rs) matches the
 *   basename `xenia_edge` against the `Xenia Edge (Xbox 360)` profile's
 *   `xenia_edge` token. Naming it `xenia_canary` instead would match
 *   `Xenia Canary (Xbox 360)`, which is in `WINDOWS_ONLY_SLUGS` — and
 *   `InstallService::xenia_content_root` refuses a Windows-only profile on
 *   a Linux host with "The configured Xbox 360 emulator only runs on
 *   Windows.", so the whole content flow would fail before it started.
 * - `portable.txt` next to the executable is what makes
 *   `readers::xenia_directory_settings` treat the install as portable, which
 *   sets `storage_root` to the emulator directory and therefore
 *   `content_root` to `<dir>/content` — the path content.spec.ts asserts on.
 *   Without it the reader falls back to the user's real home directory.
 *
 * The stub is never spawned (applying content is a file copy, not a launch),
 * so `exit 0` is all it has to do; it only needs to exist and be executable.
 *
 * content.spec.ts still connects through the normal Connect UI:
 * `grid_core::session::connect` loads this `config.toml` first and only
 * overwrites `server_url`/`username`, so `library_path`, `[[emulators]]` and
 * `[default_emulators]` all survive that round trip.
 */

import { chmodSync, mkdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';

import { tomlString, writeRegistry } from './registry-schema.mjs';

const dataDir = process.argv[2];
if (!dataDir) {
  console.error('usage: content-seed.mjs <data-dir>');
  process.exit(1);
}

const EMULATOR_NAME = 'Xenia Edge';
const XBOX_PLATFORM = 'Xbox 360';

const libraryPath = path.join(dataDir, 'library');
const xeniaDir = path.join(dataDir, 'stubs', 'xenia-edge');
mkdirSync(libraryPath, { recursive: true });
mkdirSync(xeniaDir, { recursive: true });

const xeniaExe = path.join(xeniaDir, 'xenia_edge');
writeFileSync(xeniaExe, '#!/bin/sh\nexit 0\n');
chmodSync(xeniaExe, 0o755);
writeFileSync(path.join(xeniaDir, 'portable.txt'), '');

const configToml = `schema_version = 1
library_path = ${tomlString(libraryPath)}

[[emulators]]
name = ${tomlString(EMULATOR_NAME)}
path = ${tomlString(xeniaExe)}
args = "%rom%"

[default_emulators]
${tomlString(XBOX_PLATFORM)} = ${tomlString(EMULATOR_NAME)}
`;
writeFileSync(path.join(dataDir, 'config.toml'), configToml);

writeRegistry(path.join(dataDir, 'grid-launcher.db'));

console.log(`e2e: seeded content stage data dir at ${dataDir}`);
