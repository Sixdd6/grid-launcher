#!/usr/bin/env node
// A fake legendary (0.21.1's CLI, docs/stores-phase2-plan.md "Research" and
// "Spike results") for GRID's e2e suite. The e2e build runs it as the
// installed tool through GRID_LAUNCHER_E2E_TOOL_LEGENDARY (grid-core
// stores/tools.rs, `installed_path`), directly: it is executable and needs
// only node.
//
// Like the real tool it prints secrets, and more of them: EVERY command
// prints FAKE-STORE-TOKEN-not-real on stderr (a DEBUG line) and on stdout
// (a text line, or a `fake_session` key inside its JSON objects; an empty
// JSON array has nowhere to carry it), `user.json` holds it, and `launch
// --json` puts it in `egl_parameters` as the exchange code. So
// epic-secrets.spec.ts proves GRID never logs or forwards legendary's
// output, and that the exchange code reaches the game and nothing else.
// Its files are written 0644, as legendary's are: GRID makes the tree
// private after every run.
//
// Commands (argv order is free; flags may come before or after the command):
//   auth --disable-webview            reads the code (bare, or the JSON blob)
//                                     from stdin; FAKE-E2E-EPIC-CODE signs in
//   auth --delete                     deletes user.json
//   status --json [--offline]
//   list --json -T [--platform Windows] [--force-refresh]
//   list-installed --json [--check-updates] [--show-dirs]
//   info <app> --json [--offline] [--platform Windows]
//   install|download|update|repair <app> -y --skip-sdl [--skip-dlcs | --with-dlcs]
//         [--platform Windows] [--base-path <dir>] [--game-folder <name>]
//         [--update-only] [--repair] [--max-shared-memory <n>]
//   uninstall <app> -y [--skip-uninstaller] [--keep-files]
//   launch <app> --json [--skip-version-check] [--offline]
//   list-saves [<app>]
//   sync-saves <app> -y --save-path <dir> [--force-upload | --force-download]
//         [--skip-upload] [--skip-download]
//   --version
//
// Its config dir is $LEGENDARY_CONFIG_PATH (else $XDG_CONFIG_HOME/legendary):
// user.json, installed.json, and the "cloud" under fake-cloud/<app>/ (the
// save files and manifests.json, the manifest names with their times).
// Marker files there change its behaviour, so a spec can set them up:
//   .fake-expired           a non-offline login fails (Epic refuses the
//                           saved login; user.json goes, as legendary does)
//   .fake-download-fail     an install/update/repair dies with a
//                           requests.exceptions.ConnectionError traceback
//   .fake-launch-network    an online `launch` dies the same way
// Environment: FAKE_LEGENDARY_FIXTURES (owned.json, default
// e2e/fixtures-epic), FAKE_LEGENDARY_STEPS (progress lines, default 5),
// FAKE_LEGENDARY_STEP_MS (default 150), FAKE_LEGENDARY_REPAIRED (files a
// repair finds damaged, default 2), GRID_LAUNCHER_E2E_EPIC_BASE (the mock
// Epic: the art's origin).

import { chmodSync, cpSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, rmdirSync, statSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
export const FAKE_TOKEN = "FAKE-STORE-TOKEN-not-real";
/** The code the mock Epic's login page hands out (a fixture, not a secret). */
export const GOOD_CODE = "FAKE-E2E-EPIC-CODE";
export const VERSION = "0.21.1";

/** Flags that take a value; every other flag is a switch. */
const VALUE_FLAGS = new Set([
  "platform", "base-path", "game-folder", "save-path", "max-shared-memory", "wrapper", "wine",
  "wine-prefix", "override-exe", "language", "override-username", "code", "token", "sid",
  "max-workers", "dl-timeout", "prefix", "exclude", "install-tag", "dlm-debug",
]);

/** The command, its positional args and its flags. */
export function parseArgs(argv) {
  const positional = [];
  const flags = {};
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a.startsWith("--")) {
      const [name, inline] = a.slice(2).split(/=(.*)/s);
      if (inline !== undefined) flags[name] = inline;
      else if (VALUE_FLAGS.has(name) && i + 1 < argv.length) flags[name] = argv[++i];
      else flags[name] = true;
    } else if (a.startsWith("-") && a.length > 1) {
      for (const c of a.slice(1)) flags[c] = true;
    } else {
      positional.push(a);
    }
  }
  return { command: positional[0], positional: positional.slice(1), flags };
}

const log = (name, level, msg) => process.stderr.write(`[${name}] ${level}: ${msg}\n`);
const out = (text) => process.stdout.write(text);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const json = (value) => out(`${JSON.stringify(value)}\n`);

export function configDir(env = process.env) {
  if (env.LEGENDARY_CONFIG_PATH) return env.LEGENDARY_CONFIG_PATH;
  const xdg = env.XDG_CONFIG_HOME || path.join(homedir(), ".config");
  return path.join(xdg, "legendary");
}

export function loadFixtures(env = process.env) {
  const file = env.FAKE_LEGENDARY_FIXTURES ?? path.join(here, "..", "fixtures-epic", "owned.json");
  return JSON.parse(readFileSync(file, "utf8"));
}

const artBase = () => (process.env.GRID_LAUNCHER_E2E_EPIC_BASE || "http://127.0.0.1:9").replace(/\/+$/, "");

/** Written 0644 on purpose: legendary sets no file mode. */
function write(file, text) {
  mkdirSync(path.dirname(file), { recursive: true });
  writeFileSync(file, text);
  try { chmodSync(file, 0o644); } catch { /* not unix */ }
}

const readJson = (file, fallback) => {
  try { return JSON.parse(readFileSync(file, "utf8")); } catch { return fallback; }
};

class Ctx {
  constructor(env = process.env) {
    this.dir = configDir(env);
    this.fx = loadFixtures(env);
  }
  file(name) { return path.join(this.dir, name); }
  marker(name) { return existsSync(this.file(name)); }
  user() { return readJson(this.file("user.json"), null); }
  installed() { return readJson(this.file("installed.json"), {}); }
  saveInstalled(map) { write(this.file("installed.json"), JSON.stringify(map, null, 2)); }
  game(app) { return this.fx.games.find((g) => g.app_name === app); }
  /** A DLC and its base game. */
  dlc(app) {
    for (const g of this.fx.games) {
      const d = g.dlcs.find((x) => x.app_name === app);
      if (d) return { dlc: d, base: g };
    }
    return null;
  }
  cloudDir(app) { return path.join(this.dir, "fake-cloud", app); }
}

/** legendary's uncaught "not logged in" (spike 1): a traceback, exit 1. */
function noCredentials() {
  process.stderr.write(
    "Traceback (most recent call last):\n" +
      '  File "legendary/cli.py", line 3000, in main\n' +
      '  File "legendary/core.py", line 218, in login\n' +
      "ValueError: No saved credentials\n",
  );
  return 1;
}

function networkTraceback() {
  process.stderr.write(
    "Traceback (most recent call last):\n" +
      '  File "requests/adapters.py", line 700, in send\n' +
      "requests.exceptions.ConnectionError: HTTPSConnectionPool(host='epicgames-download1.akamaized.net', port=443): Max retries exceeded\n",
  );
  return 1;
}

/** core.login(): false (with legendary's lines) when Epic refuses the saved login. */
function login(ctx) {
  if (!ctx.user()) return "none";
  log("Core", "INFO", "Trying to re-use existing login session...");
  if (ctx.marker(".fake-expired")) {
    log("Core", "INFO", "Logging in...");
    log("Core", "ERROR", "Stored credentials are no longer valid! Please login again.");
    rmSync(ctx.file("user.json"), { force: true });
    return "expired";
  }
  return "ok";
}

function userRecord(fx) {
  return {
    access_token: FAKE_TOKEN,
    refresh_token: FAKE_TOKEN,
    account_id: fx.user.account_id,
    displayName: fx.user.display_name,
    expires_at: "2099-01-01T00:00:00.000Z",
    token_type: "bearer",
    client_id: "34a02cf8f4414e29b15921876da36f9a",
  };
}

async function readStdin() {
  const chunks = [];
  for await (const c of process.stdin) chunks.push(c);
  return Buffer.concat(chunks).toString("utf8");
}

async function cmdAuth(ctx, { flags }) {
  if (flags.delete) {
    rmSync(ctx.file("user.json"), { force: true });
    out(`Forgot session ${FAKE_TOKEN}\n`);
    log("cli", "INFO", "User data deleted.");
    return 0;
  }
  if (flags.code !== undefined) {
    // GRID never passes the code on argv (docs/stores-phase2-plan.md).
    log("cli", "ERROR", "fake legendary: the code came on argv");
    return 2;
  }
  out("Please login via the epic web login!\n");
  out("If the web page did not open automatically, please manually open the following URL: https://legendary.gl/epiclogin\n");
  out(`Please enter the "authorizationCode" value from the JSON response (session ${FAKE_TOKEN}): `);
  const text = await readStdin();
  if (text === "") {
    process.stderr.write('Traceback (most recent call last):\n  File "legendary/cli.py", line 160, in auth\nEOFError: EOF when reading a line\n');
    return 1;
  }
  let code = text.trim();
  if (code.startsWith("{")) {
    try { code = JSON.parse(code).authorizationCode ?? ""; } catch { code = ""; }
  }
  if (code !== GOOD_CODE) {
    log("EPCAPI", "ERROR", "Login to EGS API failed with errorCode: errors.com.epicgames.account.oauth.authorization_code_not_found");
    log("Core", "ERROR", "Logging in failed with InvalidCredentialsError('errors.com.epicgames.account.oauth.authorization_code_not_found'), please try again.");
    log("cli", "ERROR", "Login attempt failed, please see log for details.");
    return 0;
  }
  write(ctx.file("user.json"), JSON.stringify(userRecord(ctx.fx)));
  log("Core", "DEBUG", `Stored session ${FAKE_TOKEN}`);
  log("cli", "INFO", `Successfully logged in as "${ctx.fx.user.display_name}"`);
  return 0;
}

function cmdStatus(ctx, { flags }) {
  if (!flags.offline && login(ctx) === "expired") {
    log("cli", "ERROR", "Log in failed!");
    return 1;
  }
  const user = ctx.user();
  json({
    account: user ? user.displayName : "<not logged in>",
    games_available: user ? ctx.fx.games.length : 0,
    games_installed: Object.keys(ctx.installed()).length,
    egl_sync_enabled: false,
    config_directory: ctx.dir,
    fake_session: FAKE_TOKEN,
  });
  return 0;
}

function keyImages(g) {
  const base = artBase();
  return [
    { type: "DieselGameBoxTall", url: `${base}/art/${g.app_name}_tall.jpg`, md5: "0", width: 1200, height: 1600, size: 1, uploadedDate: "2024-01-01T00:00:00.000Z" },
    { type: "DieselGameBox", url: `${base}/art/${g.app_name}_wide.jpg`, md5: "0", width: 2560, height: 1440, size: 1, uploadedDate: "2024-01-01T00:00:00.000Z" },
    { type: "DieselGameBoxLogo", url: `${base}/art/${g.app_name}_logo.png`, md5: "0", width: 400, height: 200, size: 1, uploadedDate: "2024-01-01T00:00:00.000Z" },
    { type: "Screenshot", url: `${base}/art/${g.app_name}_shot1.jpg`, md5: "0", width: 1920, height: 1080, size: 1, uploadedDate: "2024-01-01T00:00:00.000Z" },
    // Spike 4: video entries use a non-http scheme the parser must skip.
    { type: "heroCarouselVideo", url: `com.epicgames.video://${g.app_name}`, md5: "0", width: 0, height: 0, size: 0, uploadedDate: "2024-01-01T00:00:00.000Z" },
  ];
}

function attrs(g) {
  const a = {
    CanRunOffline: { type: "BOOLEAN", value: g.can_run_offline ? "true" : "false" },
    FolderName: { type: "STRING", value: g.title.replace(/[^A-Za-z0-9]/g, "") },
    PresenceId: { type: "STRING", value: g.app_name },
  };
  if (g.cloud_save_folder) a.CloudSaveFolder = { type: "STRING", value: g.cloud_save_folder };
  if (g.third_party) a.ThirdPartyManagedApp = { type: "STRING", value: g.third_party };
  return a;
}

function assetInfo(app, ns, build) {
  return {
    Windows: {
      app_name: app,
      asset_id: app,
      build_version: build,
      catalog_item_id: `${app}-catalog`,
      label_name: "Live",
      metadata: { installationPoolId: "fake" },
      namespace: ns,
    },
  };
}

function listEntry(g) {
  return {
    app_name: g.app_name,
    app_title: g.title,
    asset_infos: assetInfo(g.app_name, g.namespace, g.build_version),
    base_urls: [],
    metadata: {
      id: `${g.app_name}-catalog`,
      namespace: g.namespace,
      title: g.title,
      description: `${g.title} by ${g.developer}.`,
      developer: g.developer,
      developerId: g.namespace,
      keyImages: keyImages(g),
      categories: [{ path: "games" }, { path: "applications" }],
      customAttributes: attrs(g),
      releaseInfo: [{ appId: g.app_name, platform: ["Windows"], dateAdded: `${g.year}-03-14T00:00:00.000Z` }],
      creationDate: `${g.year}-03-14T00:00:00.000Z`,
      lastModifiedDate: "2026-01-01T00:00:00.000Z",
    },
    dlcs: g.dlcs.map((d) => ({
      app_name: d.app_name,
      app_title: d.title,
      asset_infos: assetInfo(d.app_name, g.namespace, g.build_version),
      base_urls: [],
      metadata: {
        id: `${d.app_name}-catalog`,
        namespace: g.namespace,
        title: d.title,
        description: d.title,
        developer: g.developer,
        keyImages: [],
        categories: [{ path: "dlc" }],
        customAttributes: {},
        mainGameItem: { id: `${g.app_name}-catalog`, namespace: g.namespace },
        releaseInfo: [{ appId: d.app_name, platform: ["Windows"] }],
      },
    })),
    fake_session: FAKE_TOKEN,
  };
}

function cmdList(ctx, { flags }) {
  const state = login(ctx);
  if (state === "none") return noCredentials();
  if (state === "expired") { log("cli", "ERROR", "Login failed, cannot continue!"); return 1; }
  log("cli", "INFO", "Logging in...");
  log("cli", "INFO", "Getting game list... (this may take a while)");
  const games = ctx.fx.games.filter((g) => flags.T || flags["third-party"] || !g.third_party);
  json(games.map(listEntry));
  return 0;
}

function cmdListInstalled(ctx) {
  const map = ctx.installed();
  json(Object.values(map).map((e) => ({ ...e, fake_session: FAKE_TOKEN })));
  return 0;
}

function cmdInfo(ctx, { positional }) {
  const app = positional[0];
  const g = ctx.game(app);
  const d = g ? null : ctx.dlc(app);
  if (!g && !d) {
    json({ game: null, install: null, manifest: null, fake_session: FAKE_TOKEN });
    return 0;
  }
  const inst = ctx.installed()[app];
  const title = g ? g.title : d.dlc.title;
  const size = g ? { download: g.download_size, disk: g.disk_size } : { download: d.dlc.download_size, disk: d.dlc.download_size };
  json({
    game: {
      app_name: app,
      title,
      version: (g ?? d.base).build_version,
      platform_versions: { Windows: (g ?? d.base).build_version },
      cloud_saves_supported: Boolean(g?.cloud_save_folder),
      cloud_save_folder: g?.cloud_save_folder || null,
      is_dlc: Boolean(d),
      external_activation: null,
      launch_options: [],
      owned_dlc: g ? g.dlcs.map((x) => ({ app_name: x.app_name, title: x.title, id: `${x.app_name}-catalog` })) : [],
    },
    install: inst ? { platform: "Windows", version: inst.version, disk_size: inst.install_size, install_path: inst.install_path, save_path: inst.save_path } : null,
    manifest: {
      size: 1024,
      type: "binary",
      version: 21,
      app_name: app,
      launch_exe: g ? g.executable.replace(/\//g, "\\") : "",
      launch_command: "",
      build_version: (g ?? d.base).build_version,
      build_id: "fake",
      prerequisites: null,
      uninstaller: null,
      install_tags: [""],
      num_files: 2,
      num_chunks: 1,
      disk_size: size.disk,
      download_size: size.download,
    },
    fake_session: FAKE_TOKEN,
  });
  return 0;
}

async function progress(total, steps, stepMs) {
  for (let s = 1; s <= steps; s++) {
    const pct = ((s / steps) * 100).toFixed(2);
    const done = Math.round((total * s) / steps);
    log("DLManager", "INFO", `= Progress: ${pct}% (${s * 10}/${steps * 10}), Running for 00:00:0${s}, ETA: 00:00:0${steps - s}`);
    log("DLManager", "INFO", ` - Downloaded: ${(done / 1048576).toFixed(2)} MiB, Written: ${(done / 1048576).toFixed(2)} MiB`);
    log("DLManager", "INFO", ` + Download\t- ${(41.2).toFixed(2)} MiB/s (raw) / ${(82.4).toFixed(2)} MiB/s (decompressed)`);
    log("DLManager", "INFO", ` + Disk\t- ${(80).toFixed(2)} MiB/s (write) / ${(0).toFixed(2)} MiB/s (read)`);
    await sleep(stepMs);
  }
}

async function cmdInstall(ctx, { command, positional, flags }) {
  const app = positional[0];
  const state = login(ctx);
  if (state === "none") return noCredentials();
  if (state === "expired") { log("cli", "ERROR", "Login failed! Cannot continue with download process."); return 1; }
  const steps = Number(process.env.FAKE_LEGENDARY_STEPS ?? 5);
  const stepMs = Number(process.env.FAKE_LEGENDARY_STEP_MS ?? 150);
  const map = ctx.installed();
  const g = ctx.game(app);
  const d = g ? null : ctx.dlc(app);
  if (!g && !d) { log("cli", "ERROR", `Could not find "${app}" in list of available games, did you type the name correctly?`); return 1; }
  if (g?.third_party) {
    log("cli", "ERROR", `The selected title has to be installed via a third-party store: ${g.third_party}`);
    return 0;
  }
  const isRepair = command === "repair" || flags.repair;
  const isUpdate = command === "update" || flags["update-only"];
  if ((isRepair || isUpdate) && !map[app]) {
    log("cli", "ERROR", `Update requested for "${app}", but app not installed!`);
    return 1;
  }
  if (d && !map[d.base.app_name]) {
    log("cli", "ERROR", `Base game "${d.base.app_name}" is not installed!`);
    return 1;
  }
  log("cli", "INFO", `Preparing download for "${g ? g.title : d.dlc.title}" (${app})...`);
  log("Core", "DEBUG", `Using session ${FAKE_TOKEN}`);
  out(`Fetching manifest with session ${FAKE_TOKEN}\n`);
  if (isRepair) {
    const bad = Number(process.env.FAKE_LEGENDARY_REPAIRED ?? 2);
    log("cli", "INFO", "Verifying files...");
    for (let s = 1; s <= steps; s++) {
      out(`Verification progress: ${s * 10}/${steps * 10} (${((s / steps) * 100).toFixed(1)}%) [120.0 MiB/s]\t\r`);
      await sleep(stepMs);
    }
    out("\n");
    for (let i = 0; i < bad; i++) log("cli", "ERROR", `File does not match hash: Content/Paks/pak${i}.pak`);
    if (bad > 0) log("cli", "ERROR", `Verification failed, ${bad} file(s) corrupted, 0 file(s) are missing.`);
    else {
      log("cli", "INFO", "Verification finished successfully.");
      log("cli", "INFO", "Game has been verified, nothing to repair.");
      return 0;
    }
  }
  const total = g ? g.download_size : d.dlc.download_size;
  const size = isRepair ? Math.round(total / 10) : total;
  log("cli", "INFO", `Install size: ${((g ? g.disk_size : total) / 1048576).toFixed(2)} MiB`);
  log("cli", "INFO", `Download size: ${(size / 1048576).toFixed(2)} MiB`);
  if (ctx.marker(".fake-download-fail")) {
    await progress(size, Math.min(2, steps), stepMs);
    return networkTraceback();
  }
  await progress(size, steps, stepMs);
  if (d) {
    const base = map[d.base.app_name];
    map[app] = {
      ...installedEntry(app, d.dlc.title, d.base.build_version, base.install_path, d.dlc.download_size, "", false),
      is_dlc: true,
    };
  } else {
    const existing = map[app];
    const dir = existing
      ? existing.install_path
      : path.join(flags["base-path"] || path.join(ctx.dir, "..", "Games"), flags["game-folder"] || g.title.replace(/[^A-Za-z0-9]/g, ""));
    const exe = path.join(dir, ...g.executable.split("/"));
    mkdirSync(path.dirname(exe), { recursive: true });
    writeFileSync(exe, "MZ fake epic game\n");
    write(path.join(dir, ".egstore", `${app}.manifest`), "fake manifest\n");
    map[app] = installedEntry(app, g.title, g.build_version, dir, g.disk_size, g.executable, g.can_run_offline);
    if (existing?.save_path) map[app].save_path = existing.save_path;
    // `-y` without --skip-dlcs installs every owned DLC.
    if (!flags["skip-dlcs"] && !existing) {
      for (const x of g.dlcs) map[x.app_name] = { ...installedEntry(x.app_name, x.title, g.build_version, dir, x.download_size, "", false), is_dlc: true };
    }
  }
  ctx.saveInstalled(map);
  log("cli", "INFO", "Finished installation process in 1.00 seconds.");
  return 0;
}

function installedEntry(app, title, version, dir, size, exe, offline) {
  return {
    app_name: app,
    base_urls: [],
    can_run_offline: offline,
    egl_guid: "",
    executable: exe,
    install_path: dir,
    install_size: size,
    install_tags: [],
    is_dlc: false,
    launch_parameters: "",
    manifest_path: null,
    needs_verification: false,
    platform: "Windows",
    prereq_info: null,
    requires_ot: false,
    save_path: null,
    title,
    uninstaller: null,
    version,
    is_preloaded: false,
  };
}

function removeEmptyDirs(dir, stop) {
  let current = dir;
  while (current.startsWith(stop) && current !== path.dirname(stop)) {
    try { rmdirSync(current); } catch { return; }
    current = path.dirname(current);
  }
}

function cmdUninstall(ctx, { positional, flags }) {
  const app = positional[0];
  const map = ctx.installed();
  const entry = map[app];
  if (!entry) {
    log("cli", "ERROR", `Game ${app} not installed, cannot uninstall!`);
    return 0;
  }
  log("Core", "DEBUG", `Session ${FAKE_TOKEN}`);
  out(`Uninstalling with session ${FAKE_TOKEN}\n`);
  const removeFiles = (e) => {
    if (flags["keep-files"] || !e.executable) return;
    const exe = path.join(e.install_path, ...e.executable.split("/"));
    rmSync(exe, { force: true });
    removeEmptyDirs(path.dirname(exe), e.install_path);
    rmSync(path.join(e.install_path, ".egstore"), { recursive: true, force: true });
    try { rmdirSync(e.install_path); } catch { /* leftovers (the prefix) stay for GRID */ }
  };
  if (!entry.is_dlc) {
    for (const [name, e] of Object.entries(map)) {
      if (e.is_dlc && e.install_path === entry.install_path) {
        log("cli", "INFO", `Uninstalling DLC "${name}"...`);
        delete map[name];
      }
    }
  }
  log("cli", "INFO", `Removing "${entry.title}" from "${entry.install_path}"...`);
  removeFiles(entry);
  delete map[app];
  ctx.saveInstalled(map);
  log("cli", "INFO", "Game has been uninstalled.");
  return 0;
}

function cmdLaunch(ctx, { positional, flags }) {
  const app = positional[0];
  const entry = ctx.installed()[app];
  if (!entry) {
    log("cli", "ERROR", `Game ${app} is not currently installed!`);
    return 1;
  }
  if (entry.is_dlc) {
    log("cli", "ERROR", `${app} is DLC without an executable; please launch the base game instead!`);
    return 1;
  }
  const offline = Boolean(flags.offline);
  if (!offline) {
    log("cli", "INFO", "Logging in...");
    const state = login(ctx);
    if (state !== "ok") {
      log("cli", "ERROR", "Login failed, cannot continue!");
      return 1;
    }
    if (ctx.marker(".fake-launch-network")) return networkTraceback();
  }
  const exe = entry.executable.replace(/\\/g, "/").replace(/^\/+/, "");
  const user = ctx.user() ?? userRecord(ctx.fx);
  const g = ctx.game(app);
  const egl = [
    "-AUTH_LOGIN=unused",
    ...(offline ? [] : [`-AUTH_PASSWORD=${FAKE_TOKEN}`, "-AUTH_TYPE=exchangecode"]),
    `-epicapp=${app}`,
    "-epicenv=Prod",
    "-EpicPortal",
    `-epicusername=${user.displayName}`,
    `-epicuserid=${user.account_id}`,
    "-epiclocale=en",
    `-epicsandboxid=${g?.namespace ?? app}`,
  ];
  json({
    game_parameters: [],
    game_executable: exe,
    game_directory: entry.install_path,
    egl_parameters: egl,
    launch_command: [],
    working_directory: path.dirname(path.join(entry.install_path, exe)),
    user_parameters: entry.launch_parameters ? entry.launch_parameters.split(" ") : [],
    environment: {},
    pre_launch_command: "",
    pre_launch_wait: false,
  });
  return 0;
}

function manifestName(date = new Date()) {
  const p = (n) => String(n).padStart(2, "0");
  return `${date.getUTCFullYear()}.${p(date.getUTCMonth() + 1)}.${p(date.getUTCDate())}-${p(date.getUTCHours())}.${p(date.getUTCMinutes())}.${p(date.getUTCSeconds())}.manifest`;
}

function cloudManifests(ctx, app) {
  return readJson(path.join(ctx.cloudDir(app), "manifests.json"), []);
}

function cmdListSaves(ctx, { positional }) {
  const state = login(ctx);
  if (state === "none") return noCredentials();
  if (state === "expired") { log("cli", "ERROR", "Login failed! Cannot continue with download process."); return 1; }
  log("cli", "INFO", "Getting list of saves...");
  log("Core", "DEBUG", `Session ${FAKE_TOKEN}`);
  const apps = positional[0] ? [positional[0]] : ctx.fx.games.map((g) => g.app_name);
  out(`Session ${FAKE_TOKEN}\n`);
  out("Save games:\n");
  for (const app of apps) {
    const manifests = cloudManifests(ctx, app);
    if (!manifests.length) continue;
    out(`- ${ctx.game(app)?.title ?? "Unknown"} ("${app}")\n`);
    for (const m of [...manifests].sort()) out(` + ${m}\n`);
  }
  return 0;
}

function filesUnder(dir) {
  const outList = [];
  const walk = (d) => {
    let names;
    try { names = readdirSync(d); } catch { return; }
    for (const n of names) {
      const full = path.join(d, n);
      if (statSync(full).isDirectory()) walk(full);
      else outList.push(full);
    }
  };
  walk(dir);
  return outList;
}

function cmdSyncSaves(ctx, { positional, flags }) {
  const app = positional[0];
  const state = login(ctx);
  if (state === "none") return noCredentials();
  if (state === "expired") { log("cli", "ERROR", "Login failed! Cannot continue with download process."); return 1; }
  const entry = ctx.installed()[app];
  if (!entry) { log("cli", "CRITICAL", `Game not installed: ${app}`); return 1; }
  const savePath = typeof flags["save-path"] === "string" ? flags["save-path"] : entry.save_path;
  log("Core", "DEBUG", `Session ${FAKE_TOKEN}`);
  out(`Session ${FAKE_TOKEN}\n`);
  log("cli", "INFO", `Checking "${entry.title}" (${app})`);
  if (!savePath) { log("cli", "INFO", "Save path for this title has not been set, skipping due to --yes"); return 0; }
  if (typeof flags["save-path"] === "string") {
    log("cli", "INFO", `Overriding save path with "${savePath}"...`);
    const map = ctx.installed();
    map[app].save_path = savePath;
    ctx.saveInstalled(map);
  }
  const cloud = ctx.cloudDir(app);
  const filesDir = path.join(cloud, "files");
  if (flags["force-download"] && !flags["skip-download"]) {
    if (!cloudManifests(ctx, app).length) { log("cli", "INFO", "No cloud or local savegame found."); return 0; }
    log("cli", "INFO", "Downloading saves...");
    // A download cleans the save folder first (clean_dir=True).
    rmSync(savePath, { recursive: true, force: true });
    mkdirSync(savePath, { recursive: true });
    if (existsSync(filesDir)) cpSync(filesDir, savePath, { recursive: true });
    return 0;
  }
  if (flags["force-upload"] && !flags["skip-upload"]) {
    const files = filesUnder(savePath);
    if (!files.length) { log("cli", "INFO", "No cloud or local savegame found."); return 0; }
    log("cli", "INFO", "Uploading local savegame...");
    rmSync(filesDir, { recursive: true, force: true });
    mkdirSync(filesDir, { recursive: true });
    cpSync(savePath, filesDir, { recursive: true });
    const manifests = cloudManifests(ctx, app);
    manifests.push(manifestName());
    write(path.join(cloud, "manifests.json"), JSON.stringify(manifests));
    return 0;
  }
  log("cli", "INFO", `Save game for "${entry.title}" is up to date, skipping...`);
  return 0;
}

export async function main(argv, env = process.env) {
  const args = parseArgs(argv);
  if (args.flags.version || args.flags.V) {
    out(`legendary version "${VERSION}", codename "Lowlife"\n`);
    log("cli", "DEBUG", `session ${FAKE_TOKEN}`);
    return 0;
  }
  const ctx = new Ctx(env);
  mkdirSync(ctx.dir, { recursive: true });
  if (!existsSync(ctx.file("config.ini"))) write(ctx.file("config.ini"), "[Legendary]\ndisable_update_check = false\ndisable_update_notice = true\n");
  // Every command logs the token (and puts it on stdout below).
  log("Core", "DEBUG", `Loaded session ${FAKE_TOKEN}`);
  switch (args.command) {
    case "auth": return cmdAuth(ctx, args);
    case "status": return cmdStatus(ctx, args);
    case "list":
    case "list-games": return cmdList(ctx, args);
    case "list-installed": return cmdListInstalled(ctx, args);
    case "info": return cmdInfo(ctx, args);
    case "install":
    case "download":
    case "update":
    case "repair": return cmdInstall(ctx, args);
    case "uninstall": return cmdUninstall(ctx, args);
    case "launch": return cmdLaunch(ctx, args);
    case "list-saves": return cmdListSaves(ctx, args);
    case "sync-saves": return cmdSyncSaves(ctx, args);
    default:
      log("cli", "ERROR", `unknown command ${args.command ?? ""}`);
      return 2;
  }
}

if (process.argv[1] && import.meta.url === `file://${process.argv[1]}`) {
  main(process.argv.slice(2)).then((code) => process.exit(code));
}
