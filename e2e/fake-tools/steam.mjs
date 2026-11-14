#!/usr/bin/env node
// A fake Steam client for GRID's e2e suite (docs/stores-phase4-plan.md
// package C "E2E harness"). The e2e build runs it as the Steam executable
// through GRID_LAUNCHER_E2E_STEAM_EXE (stores_service/steam.rs
// `steam_service`, grid-core `SteamService::with_roots_override`),
// directly: it is executable and needs only node. GRID hands it a
// `steam://` link exactly as it would the real client:
//
//   steam steam://install/<id>         Steam's install dialog: after a short
//                                      pause (the user confirming) it writes
//                                      appmanifest_<id>.acf in the root's
//                                      library with StateFlags 1026 and a
//                                      growing BytesDownloaded over ~6 s,
//                                      then StateFlags 4 and the game folder
//   steam steam://uninstall/<id>       deletes the manifest and the game
//                                      folder (in whichever library holds it)
//   steam steam://validate/<id>        records only
//   steam -silent steam://rungameid/<id>
//                                      the game starts: LastPlayed in its
//                                      manifest, RunningAppID = id and
//                                      apps/<id>/Running = 1 in registry.vdf,
//                                      and a game process whose argv holds
//                                      `SteamLaunch AppId=<id>` (the /proc
//                                      fallback's mark); after
//                                      E2E_STEAM_RUN_SECS (default 4) it ends
//                                      and both values go back to 0
//
// Each run appends its argv (one JSON array per line) to
// E2E_STEAM_ARGV_FILE, then hands the work to a detached copy of itself and
// exits at once, as a second Steam process that forwards a link does.
//
// Like the real client it prints things GRID must never read: every run
// prints FAKE-STEAM-KEY-not-real on stdout and stderr (GRID spawns it with
// null stdio, so steam-secrets.spec.ts finds the token nowhere).
//
// The Steam root is GRID_LAUNCHER_E2E_STEAM_ROOT (the e2e build's root
// override); registry.vdf is <root>/../registry.vdf. Marker files in the
// root change its behaviour:
//   .fake-install-cancel   the user closes the install dialog: no manifest ever
//   .fake-install-abort    the user cancels in Steam halfway: the manifest goes
//   .fake-no-run           the game never starts (RunningAppID stays 0)
// Environment: FAKE_STEAM_FIXTURES (default e2e/fixtures-steam/games.json),
// FAKE_STEAM_STEPS (progress writes, default 6), FAKE_STEAM_STEP_MS (default
// 1000), FAKE_STEAM_DIALOG_MS (default 800), FAKE_STEAM_UNINSTALL_MS
// (default 1000), E2E_STEAM_RUN_SECS (default 4).

import { spawn } from "node:child_process";
import { appendFileSync, existsSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const self = fileURLToPath(import.meta.url);
export const FAKE_KEY = "FAKE-STEAM-KEY-not-real";
export const OWNER = "76561198000000001";

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const num = (value, fallback) => {
  const n = Number(value);
  return Number.isFinite(n) && n >= 0 ? n : fallback;
};

/** The link GRID passed, and whether `-silent` came first. */
export function parseLink(argv) {
  const silent = argv.includes("-silent");
  const link = argv.find((a) => a.startsWith("steam://")) ?? "";
  const m = /^steam:\/\/(install|uninstall|validate|rungameid)\/(\d+)$/.exec(link);
  return m ? { action: m[1], appid: Number(m[2]), silent } : { action: "", appid: 0, silent };
}

export function steamRoot(env = process.env) {
  return env.GRID_LAUNCHER_E2E_STEAM_ROOT || env.FAKE_STEAM_ROOT || "";
}

export function loadFixtures(env = process.env) {
  const file = env.FAKE_STEAM_FIXTURES ?? path.join(here, "..", "fixtures-steam", "games.json");
  try {
    return JSON.parse(readFileSync(file, "utf8"));
  } catch {
    return { apps: {} };
  }
}

const quote = (text) => `"${String(text).replaceAll("\\", "\\\\").replaceAll('"', '\\"')}"`;

/** A KeyValues text file from a nested object (strings and objects). */
export function vdf(name, value, indent = "") {
  if (typeof value !== "object" || value === null) return `${indent}${quote(name)}\t\t${quote(value)}\n`;
  let out = `${indent}${quote(name)}\n${indent}{\n`;
  for (const [k, v] of Object.entries(value)) out += vdf(k, v, `${indent}\t`);
  return `${out}${indent}}\n`;
}

/** An app manifest's text. */
export function manifestText(fields) {
  return vdf("AppState", {
    appid: String(fields.appid),
    universe: "1",
    name: fields.name,
    StateFlags: String(fields.flags),
    installdir: fields.installdir,
    LastUpdated: String(fields.lastUpdated ?? Math.floor(Date.now() / 1000)),
    LastPlayed: String(fields.lastPlayed ?? 0),
    SizeOnDisk: String(fields.size ?? 0),
    StagingSize: "0",
    buildid: String(fields.buildid ?? 17000000 + (fields.appid % 1000)),
    LastOwner: OWNER,
    BytesToDownload: String(fields.toDownload ?? 0),
    BytesDownloaded: String(fields.downloaded ?? 0),
    BytesToStage: String(fields.toStage ?? 0),
    BytesStaged: String(fields.staged ?? 0),
    TargetBuildID: "0",
    AutoUpdateBehavior: "0",
  });
}

/** Writes `text` the way Steam does: a new file renamed over the old one. */
export function writeAtomic(file, text) {
  mkdirSync(path.dirname(file), { recursive: true });
  const tmp = `${file}.tmp${process.pid}`;
  writeFileSync(tmp, text);
  renameSync(tmp, file);
}

/** Every library's folder: the root, then each `"path"` in libraryfolders.vdf. */
export function libraries(root) {
  const out = [root];
  try {
    const text = readFileSync(path.join(root, "steamapps", "libraryfolders.vdf"), "utf8");
    for (const m of text.matchAll(/"path"\s+"((?:[^"\\]|\\.)*)"/g)) {
      const dir = m[1].replaceAll('\\"', '"').replaceAll("\\\\", "\\");
      if (!out.includes(dir)) out.push(dir);
    }
  } catch {
    // No libraryfolders.vdf: the root alone.
  }
  return out;
}

/** The manifest file for `appid` in any library, or null. */
export function findManifest(root, appid) {
  for (const lib of libraries(root)) {
    const file = path.join(lib, "steamapps", `appmanifest_${appid}.acf`);
    if (existsSync(file)) return file;
  }
  return null;
}

/** registry.vdf's text with RunningAppID and apps/<id>/Running set. */
export function setRunning(text, appid, running) {
  let out = text.replace(/("RunningAppID"\s+)"\d+"/, `$1"${running ? appid : 0}"`);
  const block = new RegExp(`("${appid}"\\s*\\{[^}]*?"Running"\\s+)"\\d+"`);
  out = out.replace(block, `$1"${running ? 1 : 0}"`);
  return out;
}

export function registryFile(root) {
  return path.join(path.dirname(root), "registry.vdf");
}

function setRegistry(root, appid, running) {
  const file = registryFile(root);
  let text;
  try {
    text = readFileSync(file, "utf8");
  } catch {
    return;
  }
  writeAtomic(file, setRunning(text, appid, running));
}

function info(fixtures, appid) {
  const app = fixtures.apps?.[String(appid)];
  const name = app?.name ?? `App ${appid}`;
  return { name, installdir: name, size: app?.size ?? 1_000_000_000 };
}

async function install(root, appid, env) {
  if (existsSync(path.join(root, ".fake-install-cancel"))) return;
  await sleep(num(env.FAKE_STEAM_DIALOG_MS, 800));
  const fixtures = loadFixtures(env);
  const { name, installdir, size } = info(fixtures, appid);
  const file = path.join(root, "steamapps", `appmanifest_${appid}.acf`);
  const steps = Math.max(1, num(env.FAKE_STEAM_STEPS, 6));
  const stepMs = num(env.FAKE_STEAM_STEP_MS, 1000);
  const abort = existsSync(path.join(root, ".fake-install-abort"));
  for (let i = 0; i < steps; i++) {
    writeAtomic(file, manifestText({ appid, name, installdir, flags: 1026, size: 0, toDownload: size, downloaded: Math.floor((size * i) / steps) }));
    if (abort && i >= Math.floor(steps / 2)) {
      await sleep(stepMs);
      rmSync(file, { force: true });
      return;
    }
    await sleep(stepMs);
  }
  const dir = path.join(root, "steamapps", "common", installdir);
  mkdirSync(dir, { recursive: true });
  writeFileSync(path.join(dir, `${installdir.replace(/[^A-Za-z0-9]+/g, "")}.x86_64`), "fake game\n");
  writeAtomic(file, manifestText({ appid, name, installdir, flags: 4, size }));
}

async function uninstall(root, appid, env) {
  await sleep(num(env.FAKE_STEAM_UNINSTALL_MS, 1000));
  const file = findManifest(root, appid);
  if (!file) return;
  const text = readFileSync(file, "utf8");
  const dir = /"installdir"\s+"((?:[^"\\]|\\.)*)"/.exec(text)?.[1];
  rmSync(file, { force: true });
  if (dir) rmSync(path.join(path.dirname(file), "common", dir), { recursive: true, force: true });
}

async function run(root, appid, env) {
  if (existsSync(path.join(root, ".fake-no-run"))) return;
  await sleep(500);
  const file = findManifest(root, appid);
  if (file) {
    const now = Math.floor(Date.now() / 1000);
    const text = readFileSync(file, "utf8").replace(/("LastPlayed"\s+)"[^"]*"/, `$1"${now}"`);
    writeAtomic(file, text);
  }
  const secs = num(env.E2E_STEAM_RUN_SECS, 4);
  const game = spawn(process.execPath, ["-e", `setTimeout(() => {}, ${Math.ceil(secs * 1000) + 5000})`, "--", "reaper", "SteamLaunch", `AppId=${appid}`], {
    stdio: "ignore",
  });
  setRegistry(root, appid, true);
  await sleep(secs * 1000);
  setRegistry(root, appid, false);
  game.kill("SIGTERM");
}

function record(argv, env) {
  const file = env.E2E_STEAM_ARGV_FILE;
  if (!file) return;
  try {
    appendFileSync(file, `${JSON.stringify(argv)}\n`);
  } catch {
    // A debugging aid only.
  }
}

async function main(argv, env = process.env) {
  const root = steamRoot(env);
  if (argv[0] === "--_work") {
    const [, action, id] = argv;
    const appid = Number(id);
    if (!root || !appid) return 0;
    if (action === "install") await install(root, appid, env);
    else if (action === "uninstall") await uninstall(root, appid, env);
    else if (action === "rungameid") await run(root, appid, env);
    return 0;
  }
  record(argv, env);
  // The real client prints account details; this prints the fixture key.
  process.stdout.write(`[fake steam] session ${FAKE_KEY}\n`);
  process.stderr.write(`[fake steam] debug ${FAKE_KEY}\n`);
  const { action, appid } = parseLink(argv);
  if (!action || action === "validate") return 0;
  const child = spawn(process.execPath, [self, "--_work", action, String(appid)], {
    detached: true,
    stdio: "ignore",
    env,
  });
  child.unref();
  return 0;
}

function isMainModule() {
  return process.argv[1] && import.meta.url === `file://${process.argv[1]}`;
}

if (isMainModule()) {
  main(process.argv.slice(2)).then(
    (code) => process.exit(code),
    () => process.exit(1),
  );
}

export { main };
