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
//   input: JSON with the goggame info and how it behaves.
//
// The ids and titles are the GameStores card's invented ones.

import { buildZip } from "../mock-romm/archives.mjs";

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

/** Input for fake-tools/installer.mjs (run through the wine/umu-run stubs). */
export function buildWindowsExe(game, mode = "silent") {
  return Buffer.from(JSON.stringify({ gogGameInfo: goggameInfo(game), mode }), "utf8");
}
