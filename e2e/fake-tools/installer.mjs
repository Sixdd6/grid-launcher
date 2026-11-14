#!/usr/bin/env node
// A fake GOG Windows offline installer (Inno Setup's silent flags, see
// docs/stores-phase1-plan.md "Spike results") for GRID's e2e suite.
//
//   installer.mjs <setup_*.exe> /SP- /VERYSILENT /SUPPRESSMSGBOXES /NORESTART
//                 /NOCANCEL /NOICONS /LANG=<l> /DIR="<path>" /LOG="<path>"
//
// It is started through the `wine` / `umu-run` stubs next to it (on PATH in
// the stage's data dir), which pass the .exe path first. The "exe" is a file
// the e2e seeds write (gog-installers.mjs `buildWindowsExe`): an Inno-looking
// head GRID can't read before the run, then `FAKE_MARKER` and JSON:
//   { "gogGameInfo": { goggame-*.info fields }, "mode": <mode> }
// (a file that is all JSON works too). Modes:
// - silent: writes goggame-<id>.info, game.exe and the log into /DIR, exits 0;
// - needs-wizard: any silent run (/VERYSILENT or /SILENT) hangs (no output,
//   nothing grows); the setup window (no silent flag) installs;
// - fail: exits 1;
// - fail-first: the first, /VERYSILENT run logs "Access is denied." and a
//   roll-back and exits 1; the milder /SILENT run installs;
// - fail-silent: every silent run logs "There is not enough space on the
//   disk." and a roll-back and exits 1; the setup window installs.
// A path may be given as Z:\… (Wine's view of /); it is mapped back.

import { mkdirSync, readFileSync, writeFileSync, appendFileSync } from "node:fs";
import path from "node:path";

/** Where the JSON starts in a seeded fake `.exe`. */
export const FAKE_MARKER = "\nGRID-E2E-FAKE-INSTALLER\n";

export function unwinePath(p) {
  const trimmed = p.replace(/^"|"$/g, "");
  const m = trimmed.match(/^[Zz]:(.*)$/);
  return m ? m[1].replace(/\\/g, "/") : trimmed;
}

export function parseFlags(args) {
  const flags = {};
  for (const a of args) {
    const m = a.match(/^\/([A-Za-z-]+)(?:=(.*))?$/);
    if (m) flags[m[1].toUpperCase()] = m[2] === undefined ? true : m[2];
  }
  return flags;
}

/** The JSON a fake `.exe` carries. */
export function readSpec(bytes) {
  const text = bytes.toString("latin1");
  const at = text.indexOf(FAKE_MARKER);
  const json = at < 0 ? bytes.toString("utf8") : bytes.subarray(at + FAKE_MARKER.length).toString("utf8");
  return JSON.parse(json);
}

/** Whether this run fails, and what it logs first ("" when it doesn't fail). */
export function failure(mode, flags) {
  const verysilent = flags.VERYSILENT === true;
  const silent = verysilent || flags.SILENT === true;
  if (mode === "fail-first" && verysilent) return "Access is denied.\nRolling back changes.\n";
  if (mode === "fail-silent" && silent) return "There is not enough space on the disk.\nRolling back changes.\n";
  return "";
}

function log(flags, text) {
  if (flags.LOG) appendFileSync(unwinePath(flags.LOG), text);
}

export async function main(argv) {
  const [exe, ...rest] = argv;
  if (!exe) {
    process.stderr.write("usage: installer.mjs <setup.exe> [flags]\n");
    return 2;
  }
  const spec = readSpec(readFileSync(unwinePath(exe)));
  const flags = parseFlags(rest);
  const silent = flags.VERYSILENT === true || flags.SILENT === true;
  if (spec.mode === "fail") return 1;
  if (spec.mode === "needs-wizard" && silent) {
    await new Promise(() => {});
  }
  const failed = failure(spec.mode, flags);
  if (failed !== "") {
    log(flags, failed);
    return 1;
  }
  const dir = flags.DIR ? unwinePath(flags.DIR) : path.join(process.cwd(), spec.gogGameInfo.name);
  mkdirSync(dir, { recursive: true });
  const info = spec.gogGameInfo;
  writeFileSync(path.join(dir, `goggame-${info.gameId}.info`), JSON.stringify(info));
  if (info.gameId === info.rootGameId) writeFileSync(path.join(dir, "game.exe"), "MZ fake");
  log(flags, `Installed ${info.name}\nInstallation process succeeded.\n`);
  return 0;
}

if (process.argv[1] && import.meta.url === `file://${process.argv[1]}`) {
  main(process.argv.slice(2)).then((code) => process.exit(code));
}
