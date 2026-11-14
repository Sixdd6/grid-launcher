// Fake GOG offline installers for the e2e suite (package D fixtures, used by
// package B's offline-installer flow and the RomM installer archives):
//
// - buildLinuxSh: a GOG Linux `.sh` as GOG ships it: a shell script
//   (makeself's place) followed by a ZIP holding `data/noarch/…` (gameinfo,
//   start.sh, game/goggame-<id>.info). GRID reads it as a zip and never runs
//   the script part.
// - buildMacPkg: a file named like a macOS installer (`.pkg`), which GRID
//   refuses.
// - buildUnrecognisedExe: a `setup_*.exe` stand-in with no GOG game info.
// - buildWindowsExe: the fake Inno installer (fake-tools/installer.mjs)
//   input: an Inno-looking head, then JSON with the goggame info and how it
//   behaves.
//
// The ids and titles are the GameStores card's invented ones.

import { buildZip } from "../mock-romm/archives.mjs";
import { FAKE_MARKER } from "./installer.mjs";

/** Wren Hollow, the Linux installer the e2e specs install. */
export const WREN_HOLLOW = { id: "1305771234", name: "Wren Hollow", version: "1.0.3", buildId: "56012345670000001" };

export function goggameInfo({ id, name, buildId = "", rootId = id }) {
  return {
    gameId: id,
    rootGameId: rootId,
    name,
    buildId,
    clientId: "",
    language: "English",
    playTasks: rootId === id
      ? [{ isPrimary: true, type: "FileTask", category: "game", name, path: "run.sh", arguments: "", workingDir: "" }]
      : [],
  };
}

export function buildLinuxSh(game = WREN_HOLLOW) {
  const header = Buffer.from(
    "#!/bin/sh\n# This script was generated using Makeself 2.4.0 (fake, GRID e2e)\n" +
      `label="${game.name} (GOG.com)"\nexit 0\n`,
    "utf8",
  );
  const zip = buildZip([
    { name: "data/noarch/gameinfo", data: Buffer.from(`${game.name}\n${game.version}\n${game.id}\n`, "utf8") },
    { name: "data/noarch/start.sh", data: Buffer.from("#!/bin/sh\nexit 0\n", "utf8") },
    { name: `data/noarch/game/goggame-${game.id}.info`, data: Buffer.from(JSON.stringify(goggameInfo(game)), "utf8") },
    { name: "data/noarch/game/run.sh", data: Buffer.from("#!/bin/sh\nexit 0\n", "utf8") },
  ]);
  return Buffer.concat([header, zip]);
}

export function buildMacPkg() {
  // xar's magic, then nothing a reader could use.
  return Buffer.concat([Buffer.from("xar!", "ascii"), Buffer.alloc(60)]);
}

export function buildUnrecognisedExe() {
  return Buffer.concat([Buffer.from("MZ", "ascii"), Buffer.alloc(254, 0x20)]);
}

/** Foxglove: its installer fails every silent run (TV mode shows the reason). */
export const FOXGLOVE = { id: "1408812345", name: "Foxglove", buildId: "" };
/** Bramble: its installer fails the first silent run; the milder one installs. */
export const BRAMBLE = { id: "1408854321", name: "Bramble", buildId: "" };

/**
 * A `setup_*.exe` stand-in for fake-tools/installer.mjs (run through the
 * wine/umu-run stubs): an Inno Setup loader signature behind an `MZ` head,
 * which GRID's reader recognises but can't parse (so the game's identity
 * comes from the install folder after the run, as for an Inno version
 * innospect doesn't know), then the installer's JSON input.
 */
export function buildWindowsExe(game, mode = "silent") {
  const head = Buffer.alloc(2048, 0);
  head.write("MZ", 0, "latin1");
  Buffer.from([0x72, 0x44, 0x6c, 0x50, 0x74, 0x53, 0xcd, 0xe6, 0xd7, 0x7b, 0x0b, 0x2a]).copy(head, 64);
  const body = Buffer.from(JSON.stringify({ gogGameInfo: goggameInfo(game), mode }), "utf8");
  return Buffer.concat([head, Buffer.from(FAKE_MARKER, "latin1"), body]);
}
