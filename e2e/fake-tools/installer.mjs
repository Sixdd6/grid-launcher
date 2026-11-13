#!/usr/bin/env node
// A fake GOG Windows offline installer (Inno Setup's silent flags, see
// docs/stores-phase1-plan.md "Spike results") for GRID's e2e suite.
//
//   installer.mjs <setup_*.exe> /SP- /VERYSILENT /SUPPRESSMSGBOXES /NORESTART
//                 /NOCANCEL /NOICONS /LANG=<l> /DIR="<path>" /LOG="<path>"
//
// It is started through the `wine` / `umu-run` stubs next to it (on PATH in
// the stage's data dir), which pass the .exe path first. The "exe" is a JSON
// file the e2e seeds write:
//   { "gogGameInfo": { goggame-*.info fields }, "mode": "silent" | "needs-wizard" | "fail" }
// - silent: writes goggame-<id>.info, game.exe and the log into /DIR, exits 0;
// - needs-wizard: with /VERYSILENT it hangs (no output, nothing grows), as
//   an installer that needs its setup window does; without it, it installs;
// - fail: exits 1.
// A path may be given as Z:\… (Wine's view of /); it is mapped back.

import { mkdirSync, readFileSync, writeFileSync, appendFileSync } from "node:fs";
import path from "node:path";

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

export async function main(argv) {
  const [exe, ...rest] = argv;
  if (!exe) {
    process.stderr.write("usage: installer.mjs <setup.exe> [flags]\n");
    return 2;
  }
  const spec = JSON.parse(readFileSync(unwinePath(exe), "utf8"));
  const flags = parseFlags(rest);
  const silent = flags.VERYSILENT === true || flags.SILENT === true;
  if (spec.mode === "fail") return 1;
  if (spec.mode === "needs-wizard" && silent) {
    await new Promise(() => {});
  }
  const dir = flags.DIR ? unwinePath(flags.DIR) : path.join(process.cwd(), spec.gogGameInfo.name);
  mkdirSync(dir, { recursive: true });
  const info = spec.gogGameInfo;
  writeFileSync(path.join(dir, `goggame-${info.gameId}.info`), JSON.stringify(info));
  if (info.gameId === info.rootGameId) writeFileSync(path.join(dir, "game.exe"), "MZ fake");
  if (flags.LOG) appendFileSync(unwinePath(flags.LOG), `Installed ${info.name}\n`);
  return 0;
}

if (process.argv[1] && import.meta.url === `file://${process.argv[1]}`) {
  main(process.argv.slice(2)).then((code) => process.exit(code));
}
