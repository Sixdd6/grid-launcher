#!/usr/bin/env node
// A fake heroic-gogdl (v1.4.0's CLI, docs/stores-phase1-plan.md "Spike
// results") for GRID's e2e suite. The e2e build runs it as the installed
// tool through GRID_LAUNCHER_E2E_TOOL_GOGDL (grid-core stores/tools.rs,
// `installed_path`), directly: it is executable and needs only node.
//
// Like the real tool it prints secrets: `auth` prints the token JSON on
// stdout, and every command logs a DEBUG line with the token on stderr, so
// gog-secrets.spec.ts proves GRID never logs or forwards tool output.
// The only token is FAKE-STORE-TOKEN-not-real.
//
//   gogdl --auth-config-path <file> auth [--code <code> | --client-id <id> --client-secret <s>]
//   gogdl --auth-config-path <file> info <id> --platform <os> [--lang <l>] [--with-dlcs]
//   gogdl --auth-config-path <file> download|update|repair <id> --platform <os> --path <dir>
//         [--lang <l>] [--with-dlcs --dlcs a,b | --skip-dlcs] [--support <dir>] [--max-workers <n>]
//   (GRID installs with `update --path <the game's own folder>`; `download`
//   appends the folder name. A Linux build gets start.sh, gameinfo and
//   <dir>/.gogdl-linux-manifest; a Windows build goggame-<id>.info, game.exe
//   and $GOGDL_CONFIG_PATH/heroic_gogdl/manifests/<id>.)
//   gogdl --auth-config-path <file> save-sync <path> <id> --os <os> --ts <float> --name <n>
//         --force-upload|--force-download|--skip-upload|--skip-download
//
// Environment: GOGDL_CONFIG_PATH (manifests under heroic_gogdl/manifests/),
// FAKE_GOGDL_STEPS (progress lines, default 5), FAKE_GOGDL_STEP_MS (default
// 150), FAKE_GOGDL_FIXTURES (products.json, default e2e/fixtures-gog),
// FAKE_GOGDL_WORKER_FAIL=1 (download exits 0 WITHOUT writing the manifest,
// as gogdl does after a fatal worker error).

import { mkdirSync, readFileSync, writeFileSync, existsSync, chmodSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
export const FAKE_TOKEN = "FAKE-STORE-TOKEN-not-real";
export const GOOD_CODE = "FAKE-E2E-GOG-CODE";
const CLIENT_ID = "46899977096215655";

function fixtures() {
  const file = process.env.FAKE_GOGDL_FIXTURES ?? path.join(here, "..", "fixtures-gog", "products.json");
  return JSON.parse(readFileSync(file, "utf8"));
}

/** Splits argv into the top-level options, the command and its own args. */
export function parseArgs(argv) {
  const top = {};
  let i = 0;
  while (i < argv.length && argv[i].startsWith("--")) {
    top[argv[i].slice(2)] = argv[i + 1];
    i += 2;
  }
  const command = argv[i];
  const rest = argv.slice(i + 1);
  const positional = [];
  const flags = {};
  for (let j = 0; j < rest.length; j++) {
    const a = rest[j];
    if (a.startsWith("--")) {
      const next = rest[j + 1];
      if (next === undefined || next.startsWith("--")) flags[a.slice(2)] = true;
      else { flags[a.slice(2)] = next; j++; }
    } else positional.push(a);
  }
  return { top, command, positional, flags };
}

const log = (name, level, msg) => process.stderr.write(`[${name}] ${level}: ${msg}\n`);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

function authRecord(user) {
  return {
    [CLIENT_ID]: {
      access_token: FAKE_TOKEN,
      refresh_token: FAKE_TOKEN,
      user_id: user.user_id,
      expires_in: 3600,
      token_type: "bearer",
      session_id: "1",
      loginTime: Date.now() / 1000,
    },
  };
}

function writePrivate(file, text) {
  mkdirSync(path.dirname(file), { recursive: true });
  writeFileSync(file, text);
  try { chmodSync(file, 0o600); } catch { /* not unix */ }
}

function cmdAuth({ top, flags }, fx) {
  const file = top["auth-config-path"];
  if (!file) { log("AUTH", "ERROR", "no auth config path"); return 1; }
  if (flags.code !== undefined) {
    if (flags.code !== GOOD_CODE) {
      log("AUTH", "ERROR", "Failed to get token");
      process.stdout.write(JSON.stringify({ error: true }) + "\n");
      return 1;
    }
    const record = authRecord(fx.user);
    writePrivate(file, JSON.stringify(record));
    log("AUTH", "DEBUG", `stored credentials ${FAKE_TOKEN}`);
    process.stdout.write(JSON.stringify(record[CLIENT_ID]) + "\n");
    return 0;
  }
  if (!existsSync(file)) {
    process.stdout.write(JSON.stringify({ error: true }) + "\n");
    return 1;
  }
  if (process.env.FAKE_GOGDL_EXPIRED === "1") {
    log("AUTH", "ERROR", "Failed to refresh credentials");
    process.stdout.write(JSON.stringify({ error: true }) + "\n");
    return 1;
  }
  const record = JSON.parse(readFileSync(file, "utf8"))[CLIENT_ID];
  if (flags["client-id"] !== undefined) {
    // A game client's credentials (cloud saves).
    process.stdout.write(JSON.stringify({ ...record, client_id: flags["client-id"] }) + "\n");
    return 0;
  }
  process.stdout.write(JSON.stringify(record) + "\n");
  return 0;
}

const product = (fx, id) => fx.products.find((p) => p.id === String(id) || p.dlcs.some((d) => d.id === String(id)));

function cmdInfo({ positional, flags }, fx) {
  const g = product(fx, positional[0]);
  const os = flags.platform ?? "windows";
  if (!g || !g.platforms.includes(os)) { log("INFO", "ERROR", "No builds for this platform"); return 1; }
  const dlcs = flags["with-dlcs"] ? g.dlcs.map((d) => ({ title: d.title, id: d.id, size: { "*": { download_size: d.download_size, disk_size: d.download_size } } })) : [];
  const out = os === "linux"
    ? { download_size: g.download_size, disk_size: g.disk_size, dlcs, languages: g.languages, folder_name: g.folder_name, versionName: g.version }
    : {
        size: Object.fromEntries([["*", { download_size: g.download_size, disk_size: g.disk_size }], ...g.languages.map((l) => [l, { download_size: 0, disk_size: 0 }])]),
        dlcs, buildId: g.build_id, languages: g.languages, folder_name: g.folder_name,
        dependencies: [], versionEtag: "etag", versionName: g.version,
      };
  log("API", "DEBUG", `authorization: Bearer ${FAKE_TOKEN}`);
  process.stdout.write(JSON.stringify(out) + "\n");
  return 0;
}

function gameInfo(g, os, lang, dlcId) {
  const isLinux = os === "linux";
  return {
    gameId: dlcId ?? g.id,
    rootGameId: g.id,
    name: dlcId ? g.dlcs.find((d) => d.id === dlcId).title : g.title,
    buildId: g.build_id,
    clientId: g.client_id,
    language: lang ?? "English",
    languages: [lang ?? "en-US"],
    version: 1,
    playTasks: dlcId ? [] : [
      { isPrimary: true, type: "FileTask", category: "game", name: g.title, path: isLinux ? "start.sh" : "game.exe", arguments: "", workingDir: "" },
    ],
  };
}

async function cmdDownload({ command, positional, flags }, fx) {
  const g = product(fx, positional[0]);
  const os = flags.platform ?? "windows";
  if (!g || !g.platforms.includes(os)) { log("DOWNLOAD", "ERROR", "No builds for this platform"); return 1; }
  if (!flags.path) { log("DOWNLOAD", "ERROR", "--path is required"); return 1; }
  const dir = command === "download" ? path.join(flags.path, g.folder_name) : flags.path;
  const steps = Number(process.env.FAKE_GOGDL_STEPS ?? 5);
  const stepMs = Number(process.env.FAKE_GOGDL_STEP_MS ?? 150);
  const total = command === "repair" ? g.disk_size : g.download_size;
  log("AUTH", "DEBUG", `Authorization: Bearer ${FAKE_TOKEN}`);
  process.stdout.write(`{"access_token": "${FAKE_TOKEN}"}\n`);
  for (let s = 1; s <= steps; s++) {
    const done = Math.round((total * s) / steps);
    const pct = ((done / total) * 100).toFixed(2);
    log("PROGRESS", "INFO", `= Progress: ${pct} ${done}/${total}, Running for: 00:00:0${s}, ETA: 00:00:0${steps - s}`);
    log("PROGRESS", "INFO", `= Downloaded: ${(done / 1048576).toFixed(2)} MiB, Written: ${(done / 1048576).toFixed(2)} MiB`);
    log("PROGRESS", "INFO", `+ Download\t- ${(1).toFixed(2)} MiB/s (raw) / ${(1).toFixed(2)} MiB/s (decompressed)`);
    log("PROGRESS", "INFO", `+ Disk\t- ${(1).toFixed(2)} MiB/s (write) / ${(1).toFixed(2)} MiB/s (read)`);
    await sleep(stepMs);
  }
  if (process.env.FAKE_GOGDL_WORKER_FAIL === "1") {
    log("TASK_EXEC", "ERROR", "worker failed");
    return 0;
  }
  mkdirSync(dir, { recursive: true });
  const lang = typeof flags.lang === "string" ? flags.lang : undefined;
  writeFileSync(path.join(dir, `goggame-${g.id}.info`), JSON.stringify(gameInfo(g, os, lang)));
  const dlcIds = flags["with-dlcs"] && typeof flags.dlcs === "string" ? flags.dlcs.split(",").filter(Boolean) : [];
  for (const id of dlcIds) {
    writeFileSync(path.join(dir, `goggame-${id}.info`), JSON.stringify(gameInfo(g, os, lang, id)));
  }
  if (os === "linux") {
    writeFileSync(path.join(dir, "start.sh"), "#!/bin/sh\nexit 0\n");
    try { chmodSync(path.join(dir, "start.sh"), 0o755); } catch { /* not unix */ }
    writeFileSync(path.join(dir, "gameinfo"), `${g.title}\n${g.version}\n`);
  } else {
    writeFileSync(path.join(dir, "game.exe"), "MZ fake");
  }
  mkdirSync(path.join(dir, "saves"), { recursive: true });
  const config = process.env.GOGDL_CONFIG_PATH;
  const manifest = os === "linux"
    ? path.join(dir, ".gogdl-linux-manifest")
    : config ? path.join(config, "heroic_gogdl", "manifests", g.id) : null;
  if (manifest) {
    mkdirSync(path.dirname(manifest), { recursive: true });
    // A fresh `update` gets the build a new install gets; the update build
    // comes from an `update` over an existing install.
    const build = g.build_id;
    writeFileSync(manifest, JSON.stringify({
      clientId: g.client_id, clientSecret: "fake-client-secret",
      version: 2, buildId: build, platform: os, install_directory: dir,
      languages: [lang ?? "en-US"], dlcs: dlcIds.map((id) => ({ id })), HGLInstallLanguage: lang ?? "en-US",
      HGLPlatform: os, HGLdlcs: dlcIds.map((id) => ({ id })),
      versionName: g.version,
    }));
  }
  return 0;
}

function cmdSaveSync({ positional, flags }) {
  if (positional.length < 2 || !flags.os || !flags.name) { log("SAVES", "ERROR", "bad arguments"); return 1; }
  log("SAVES", "DEBUG", `token ${FAKE_TOKEN}`);
  process.stdout.write(`${(Date.now() / 1000).toFixed(6)}\n`);
  return 0;
}

export async function main(argv) {
  const args = parseArgs(argv);
  const fx = fixtures();
  switch (args.command) {
    case "auth": return cmdAuth(args, fx);
    case "info": return cmdInfo(args, fx);
    case "download":
    case "update":
    case "repair": return cmdDownload(args, fx);
    case "save-sync": return cmdSaveSync(args);
    case "--version":
    case "version":
      process.stdout.write("1.4.0\n");
      return 0;
    default:
      log("MAIN", "ERROR", `unknown command ${args.command ?? ""}`);
      return 2;
  }
}

if (process.argv[1] && import.meta.url === `file://${process.argv[1]}`) {
  main(process.argv.slice(2)).then((code) => process.exit(code));
}
