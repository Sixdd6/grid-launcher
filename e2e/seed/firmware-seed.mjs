#!/usr/bin/env node
/**
 * Seeds the `firmware` stage group's temp data dir BEFORE the app starts,
 * for the two firmware triggers the spec drives.
 *
 * Trigger 1 — per-game, after an install finalizes
 * (`FirmwareService::spawn_for_game`): a DuckStation stub set as the
 * "PlayStation" default. Its firmware target comes from the
 * `DuckStation (Playstation 1)` profile's `firmware_directories: ["bios"]`,
 * resolved against the emulator's own directory, so the fixture's
 * `scph5501.bin` lands at `<stubs>/duckstation/bios/scph5501.bin`.
 *
 * The entry is NAMED for the profile — `DuckStation (Playstation 1)`, the
 * profile name verbatim — rather than a friendlier "DuckStation".
 * `profile_for_entry` (grid-core/src/launch/profiles.rs) matches the entry
 * name against the profile name first, and this stub's basename
 * (`duckstation-qt`) matches none of that profile's `match_tokens`
 * (`duckstation-qt-x64-ReleaseLTCG.exe`, `DuckStation-x64.AppImage`,
 * `duckstation*.appimage`) by either the exact or the stem rule. Without a
 * profile there are no firmware directories and the trigger silently does
 * nothing.
 *
 * Trigger 2 — a hand-added RPCS3 entry (`spawn_ps3_firmware`, reached from
 * `commands::save_emulator`): the RPCS3 stub file is written here, but NO
 * config entry for it. firmware.spec.ts adds that entry through the
 * Emulators panel, which is the only thing that fires this trigger. The stub
 * records its argv to `<data>/rpcs3-argv.log` and exits, so the spec can
 * prove the `Install PS3 Firmware` button really ran `rpcs3 --installfw`.
 *
 * `xdg-config/` is a hermeticity guard, as in ps3-install-seed.mjs: it keeps
 * every RPCS3 reader probe (`$RPCS3_CONFIG_DIR`, `$XDG_CONFIG_HOME/rpcs3`)
 * inside the temp data dir rather than a developer's real RPCS3 install.
 */

import { chmodSync, mkdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';

import { tomlString, writeRegistry } from './registry-schema.mjs';

const dataDir = process.argv[2];
if (!dataDir) {
  console.error('usage: firmware-seed.mjs <data-dir>');
  process.exit(1);
}

const DUCKSTATION_NAME = 'DuckStation (Playstation 1)';
const PS1_PLATFORM = 'PlayStation';

const libraryPath = path.join(dataDir, 'library');
const duckstationDir = path.join(dataDir, 'stubs', 'duckstation');
const rpcs3Dir = path.join(dataDir, 'stubs', 'rpcs3');
mkdirSync(libraryPath, { recursive: true });
mkdirSync(duckstationDir, { recursive: true });
mkdirSync(rpcs3Dir, { recursive: true });
mkdirSync(path.join(dataDir, 'xdg-config'), { recursive: true });

const duckstation = path.join(duckstationDir, 'duckstation-qt');
writeFileSync(duckstation, '#!/bin/sh\nexit 0\n');
chmodSync(duckstation, 0o755);

const rpcs3ArgvLog = path.join(dataDir, 'rpcs3-argv.log');
const rpcs3 = path.join(rpcs3Dir, 'rpcs3');
writeFileSync(rpcs3, `#!/bin/sh\nprintf '%s\\n' "$@" >> '${rpcs3ArgvLog}'\nexit 0\n`);
chmodSync(rpcs3, 0o755);

const configToml = `schema_version = 1
library_path = ${tomlString(libraryPath)}

[[emulators]]
name = ${tomlString(DUCKSTATION_NAME)}
path = ${tomlString(duckstation)}
args = "%rom%"

[default_emulators]
${tomlString(PS1_PLATFORM)} = ${tomlString(DUCKSTATION_NAME)}
`;
writeFileSync(path.join(dataDir, 'config.toml'), configToml);

writeRegistry(path.join(dataDir, 'grid-launcher.db'));

console.log(`e2e: seeded firmware stage data dir at ${dataDir}`);
