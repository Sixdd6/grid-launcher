#!/usr/bin/env node
// A fake nile (1.2.0's CLI, docs/stores-phase3-plan.md "Research" and
// "Spike results") for GRID's e2e suite. The e2e build runs it as the
// installed tool through GRID_LAUNCHER_E2E_TOOL_NILE (grid-core
// stores/tools.rs, `installed_path`), directly: it is executable and needs
// only node.
//
// Like the real tool it prints secrets, and more of them: EVERY command
// prints FAKE-STORE-TOKEN-not-real on stderr (a DEBUG line) and on stdout
// (a text line, or a `fake_session` key inside its JSON objects; a JSON
// array gets a text line before it), the token file holds it, and `auth
// --login --non-interactive` hands it out as the code_verifier. So
// amazon-secrets.spec.ts proves GRID never logs or forwards nile's output.
// Its files are written 0644 and its folders with the default umask, as
// nile's are: GRID makes the tree private around every run.
//
// It follows nile's habits GRID must cope with: logging on stderr as
// `LEVEL [Name]:\t message`, JSON on stdout, exit 0 for almost every
// failure, `Download complete` printed even after a failure, and the SDK
// warning on every command while there is no SDK folder.
//
// Commands (flags may come before or after the command):
//   --version
//   auth --login --non-interactive    one JSON line: client_id, code_verifier,
//                                     serial, url (<GRID_LAUNCHER_E2E_AMAZON_BASE>/ap/signin?…)
//   auth --status | auth --logout
//   register --code C --code-verifier V --serial S --client-id I
//                                     FAKE-E2E-AMAZON-CODE with the values
//                                     `auth --login` printed signs in (and syncs)
//   library sync [--force] | library list --json
//   install --info --json <id>        {"download_size": …}
//   install --path <dir> <id> | update <id> | verify <id>
//   uninstall <id>
//   list-updates --json
//   launch --json --no-wine <id>      the dry run: command, game_directory, env
// It refuses `-d` (at DEBUG nile logs response bodies) and `--base-path`
// (GRID picks the exact folder) with exit 2, so a wrong argv fails loudly.
//
// Its config dir is $NILE_CONFIG_PATH/nile (else $XDG_CONFIG_HOME/nile):
// current_user.json, <md5(user_id)>.enc (the token, in plain text here),
// library.json, installed.json, manifests/<id>.raw. Marker files there
// change its behaviour, so a spec can set them up:
//   .fake-expired         Amazon refuses the token refresh (`library sync`,
//                         `install`, `list-updates`)
//   .fake-download-fail   an install/update/verify dies with a
//                         requests.exceptions.ConnectionError traceback
//   .fake-no-space        an install/update ends "Not enough space available"
//                         (then "Download complete", exit 0)
//   .fake-update          Fogline (any game whose fixture has a newer
//                         update_version) is listed by `list-updates`
//   .fake-logout-fail     `auth --logout` dies before deleting anything
// Environment: FAKE_NILE_FIXTURES (library.json, default
// e2e/fixtures-amazon/library.json), FAKE_NILE_STEPS (progress lines,
// default 5), FAKE_NILE_STEP_MS (default 150), GRID_LAUNCHER_E2E_AMAZON_BASE
// (the mock Amazon: the login page's and the art's origin), and
// E2E_NILE_ARGV_FILE: when set, every run appends its argv there (one run
// per line, arguments joined by spaces), so a spec can tell whether GRID
// called nile at all.

import { createHash } from "node:crypto";
import { appendFileSync, chmodSync, existsSync, mkdirSync, readFileSync, rmSync, rmdirSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
export const FAKE_TOKEN = "FAKE-STORE-TOKEN-not-real";
/** The code the mock Amazon's login page hands out (a fixture, not a secret). */
export const GOOD_CODE = "FAKE-E2E-AMAZON-CODE";
export const VERSION = "1.2.0 Robert Speedwagon";
/** The account the fake signs in (nile's `Username`: the given name). */
export const USERNAME = "sam.plays";
export const USER_ID = "amzn1.account.FAKEE2EUSER0000000001";
/** The device values `auth --login` prints and `register` must get back. */
export const SERIAL = "0f1e2d3c4b5a69788796a5b4c3d2e1f0";
export const CLIENT_ID = Buffer.from(`${SERIAL}#A2UMVHOX7UP4V7`, "utf8").toString("hex");

/** Flags that take a value; every other flag is a switch. */
const VALUE_FLAGS = new Set([
  "code", "code-verifier", "serial", "client-id", "path", "base-path", "max-workers",
  "wine", "wrapper", "wine-prefix", "bottle",
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

const log = (level, name, msg) => process.stderr.write(`${level} [${name}]:\t ${msg}\n`);
const out = (text) => process.stdout.write(text);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const json = (value) => out(`${JSON.stringify({ ...value, fake_session: FAKE_TOKEN })}\n`);
/** A JSON array has nowhere to carry the token: a text line goes first. */
const jsonArray = (value) => {
  out(`Session ${FAKE_TOKEN}\n`);
  out(`${JSON.stringify(value)}\n`);
};
const tokenLine = () => out(`Session ${FAKE_TOKEN}\n`);

/** NILE_CONFIG_PATH first, then XDG_CONFIG_HOME; nile appends `/nile`. */
export function configDir(env = process.env) {
  const root = env.NILE_CONFIG_PATH || env.XDG_CONFIG_HOME || path.join(homedir(), ".config");
  return path.join(root, "nile");
}

export function loadFixtures(env = process.env) {
  const file = env.FAKE_NILE_FIXTURES ?? path.join(here, "..", "fixtures-amazon", "library.json");
  const base = (env.GRID_LAUNCHER_E2E_AMAZON_BASE || "http://127.0.0.1:9").replace(/\/+$/, "");
  return JSON.parse(readFileSync(file, "utf8").replaceAll("{BASE}", base));
}

/** Written 0644 on purpose: nile sets no file mode. */
function write(file, text) {
  mkdirSync(path.dirname(file), { recursive: true });
  writeFileSync(file, text);
  try { chmodSync(file, 0o644); } catch { /* not unix */ }
}

const readJson = (file, fallback) => {
  try { return JSON.parse(readFileSync(file, "utf8")); } catch { return fallback; }
};

const md5 = (text) => createHash("md5").update(text).digest("hex");

class Ctx {
  constructor(env = process.env) {
    this.env = env;
    this.dir = configDir(env);
    this.fx = loadFixtures(env);
  }
  file(name) { return path.join(this.dir, name); }
  marker(name) { return existsSync(this.file(name)); }
  user() { return readJson(this.file("current_user.json"), null); }
  tokenFile(user) { return this.file(`${md5(user.user_id)}.enc`); }
  /** Signed in: the user file and its token file. */
  token() {
    const user = this.user();
    return user && existsSync(this.tokenFile(user)) ? readJson(this.tokenFile(user), null) : null;
  }
  library() { return readJson(this.file("library.json"), null); }
  installed() { return readJson(this.file("installed.json"), []); }
  saveInstalled(list) { write(this.file("installed.json"), JSON.stringify(list, null, 2)); }
  /** The fixture entry for a product id. */
  game(id) { return this.fx.find((g) => g.product.id === id); }
  manifest(id) { return this.file(path.join("manifests", `${id}.raw`)); }
}

function traceback(kind, text) {
  process.stderr.write(
    "Traceback (most recent call last):\n" +
      '  File "nile/cli.py", line 160, in main\n' +
      `${kind}: ${text}\n`,
  );
  return 1;
}

const networkTraceback = () =>
  traceback(
    "requests.exceptions.ConnectionError",
    "HTTPSConnectionPool(host='download-cdn.amazongames.com', port=443): Max retries exceeded",
  );

/** The token read every online command starts with. */
function needToken(ctx) {
  if (!ctx.user()) {
    log("ERROR", "CLI", "User not logged in");
    return 1;
  }
  if (!ctx.token()) return traceback("TypeError", "'NoneType' object is not subscriptable");
  return 0;
}

/** The refresh before an online call: false (with nile's lines) when Amazon refuses it. */
function refresh(ctx) {
  log("INFO", "AUTH_MANAGER", "Refreshing token");
  if (ctx.marker(".fake-expired")) {
    log("ERROR", "AUTH_MANAGER", "Failed to refresh the token <Response [400]>");
    return false;
  }
  return true;
}

/** library.json as nile writes it: the fixture without the fake's own keys. */
function libraryJson(ctx) {
  return ctx.fx.map(({ fake: _fake, ...entry }) => entry);
}

function sync(ctx) {
  log("INFO", "LIBRARY", "Synchronizing library");
  write(ctx.file("library.json"), JSON.stringify(libraryJson(ctx)));
  write(ctx.file("syncpoint.raw"), "fake syncpoint\n");
  log("INFO", "LIBRARY", "Successfully synced the library");
}

function loginUrl(env) {
  const base = (env.GRID_LAUNCHER_E2E_AMAZON_BASE || "https://amazon.com").replace(/\/+$/, "");
  const q = new URLSearchParams({
    "openid.ns": "http://specs.openid.net/auth/2.0",
    "openid.claimed_id": "http://specs.openid.net/auth/2.0/identifier_select",
    "openid.identity": "http://specs.openid.net/auth/2.0/identifier_select",
    "openid.mode": "checkid_setup",
    "openid.oa2.scope": "device_auth_access",
    "openid.ns.oa2": "http://www.amazon.com/ap/ext/oauth/2",
    "openid.oa2.response_type": "code",
    "openid.oa2.code_challenge_method": "S256",
    "openid.oa2.client_id": `device:${CLIENT_ID}`,
    language: "en_US",
    marketPlaceId: "ATVPDKIKX0DER",
    "openid.return_to": "https://www.amazon.com",
    "openid.pape.max_auth_age": "0",
    "openid.assoc_handle": "amzn_sonic_games_launcher",
    pageId: "amzn_sonic_games_launcher",
    "openid.oa2.code_challenge": "FAKEchallengeFAKEchallengeFAKEchallenge0000",
  });
  return `${base}/ap/signin?${q.toString()}`;
}

function cmdAuth(ctx, { flags }) {
  if (flags.login) {
    if (!flags["non-interactive"]) {
      log("ERROR", "CLI", "fake nile: GRID uses --non-interactive");
      return 2;
    }
    json({ client_id: CLIENT_ID, code_verifier: FAKE_TOKEN, serial: SERIAL, url: loginUrl(ctx.env) });
    return 0;
  }
  if (flags.status) {
    const user = ctx.user();
    json({ Username: user ? user.name : "<not logged in>", LoggedIn: Boolean(user) });
    return 0;
  }
  if (flags.logout) {
    tokenLine();
    const user = ctx.user();
    if (!user) {
      log("ERROR", "AUTH_MANAGER", "You are not logged in");
      return 0;
    }
    if (!ctx.token()) return traceback("TypeError", "'NoneType' object is not subscriptable");
    if (ctx.marker(".fake-logout-fail")) return networkTraceback();
    rmSync(ctx.tokenFile(user), { force: true });
    rmSync(ctx.file("current_user.json"), { force: true });
    log("INFO", "AUTH_MANAGER", "Successfully deregistered a device");
    return 0;
  }
  log("ERROR", "CLI", "fake nile: unknown auth flags");
  return 2;
}

function cmdRegister(ctx, { flags }) {
  log("INFO", "AUTH_MANAGER", `Registerring a device. ID: ${flags["client-id"] ?? ""}`);
  const ok =
    flags.code === GOOD_CODE &&
    flags["code-verifier"] === FAKE_TOKEN &&
    flags.serial === SERIAL &&
    flags["client-id"] === CLIENT_ID;
  if (!ok) {
    log("ERROR", "AUTH_MANAGER", "Failed to register a device");
    // nile prints Amazon's answer body on stdout, and exits 0.
    json({ response: { error: { code: "InvalidValue", message: "The authorization code is invalid" } } });
    return 0;
  }
  const user = { name: USERNAME, user_id: USER_ID };
  write(ctx.file("current_user.json"), JSON.stringify(user));
  write(
    ctx.tokenFile(user),
    JSON.stringify({
      tokens: {
        bearer: { access_token: FAKE_TOKEN, refresh_token: FAKE_TOKEN, expires_in: "3600" },
        mac_dms: { device_private_key: FAKE_TOKEN, adp_token: FAKE_TOKEN },
      },
      extensions: { customer_info: { given_name: USERNAME, user_id: USER_ID }, device_info: { device_serial_number: SERIAL } },
    }),
  );
  log("INFO", "AUTH_MANAGER", "Succesfully registered a device");
  tokenLine();
  sync(ctx);
  return 0;
}

function cmdLibrary(ctx, { positional, flags }) {
  const sub = positional[0];
  if (sub === "list") {
    const lib = ctx.library();
    if (!lib) return traceback("AttributeError", "'NoneType' object has no attribute 'get'");
    if (flags.json) jsonArray(lib);
    else for (const e of lib) out(`${e.product.title} GENRES: ${(e.product.productDetail.details.genres ?? []).join(", ")}\n`);
    return 0;
  }
  if (sub === "sync") {
    const need = needToken(ctx);
    if (need) return need;
    tokenLine();
    if (!refresh(ctx)) {
      log("ERROR", "LIBRARY", "There was an error syncing library");
      return 0;
    }
    sync(ctx);
    return 0;
  }
  log("ERROR", "CLI", "fake nile: unknown library command");
  return 2;
}

async function progress(total, steps, stepMs) {
  for (let s = 1; s <= steps; s++) {
    const pct = ((s / steps) * 100).toFixed(2);
    const done = Math.round((total * s) / steps);
    log("INFO", "PROGRESS", `= Progress: ${pct} ${done}/${total}, Running for: 00:00:0${s}, ETA: 00:00:0${steps - s}`);
    log("INFO", "PROGRESS", `= Downloaded: ${(done / 1048576).toFixed(2)} MiB, Written: ${(done / 1048576).toFixed(2)} MiB`);
    log("INFO", "PROGRESS", ` + Download\t- ${(41.2).toFixed(2)} MiB/s`);
    log("INFO", "PROGRESS", ` + Disk\t- ${(80).toFixed(2)} MiB/s`);
    await sleep(stepMs);
  }
}

/** The redistributable step every fake game lists (Hotline Miami's OpenAL installer). */
export const POST_INSTALL_EXE = "oalinst.exe";

/**
 * The game's files: its executable, the OpenAL installer its PostInstall
 * step runs (`oalinst.exe /silent`, as Hotline Miami's fuel.json lists it)
 * and its fuel.json (JSON5: a comment and trailing commas).
 */
function writeGame(dir, g) {
  const exe = path.join(dir, ...g.fake.executable.split(/[\\/]/));
  mkdirSync(path.dirname(exe), { recursive: true });
  writeFileSync(exe, "MZ fake amazon game\n");
  writeFileSync(path.join(dir, POST_INSTALL_EXE), "MZ fake OpenAL installer\n");
  write(
    path.join(dir, "fuel.json"),
    `{\n  "SchemaVersion": "2",\n  // written by the fake nile\n  "Main": {\n    "Command": ${JSON.stringify(g.fake.executable)},\n    "Args": ${JSON.stringify(g.fake.args)},\n` +
      (g.fake.working_dir ? `    "WorkingSubdirOverride": ${JSON.stringify(g.fake.working_dir)},\n` : "") +
      `  },\n  "PostInstall": [\n    {"Command": ${JSON.stringify(POST_INSTALL_EXE)}, "Args": ["/silent"],},\n  ],\n}\n`,
  );
  return exe;
}

async function cmdInstall(ctx, { command, positional, flags }) {
  const id = positional[0];
  if (flags["base-path"]) {
    log("ERROR", "CLI", "fake nile: GRID passes the exact folder (--path), never --base-path");
    return 2;
  }
  const lib = ctx.library();
  if (!lib) return traceback("AttributeError", "'NoneType' object has no attribute 'get'");
  const entry = lib.find((e) => e.product.id === id);
  const g = ctx.game(id);
  if (!entry || !g) {
    log("ERROR", "CLI", "Couldn't find what you are looking for");
    return 0;
  }
  log("INFO", "CLI", `Found: ${entry.product.title}`);
  const need = needToken(ctx);
  if (need) return need;
  if (!refresh(ctx)) return traceback("TypeError", "'NoneType' object is not subscriptable");
  log("DEBUG", "DOWNLOAD", `Using session ${FAKE_TOKEN}`);
  if (flags.info) {
    json({ download_size: g.fake.download_size });
    return 0;
  }
  tokenLine();
  const steps = Number(process.env.FAKE_NILE_STEPS ?? 5);
  const stepMs = Number(process.env.FAKE_NILE_STEP_MS ?? 150);
  const list = ctx.installed();
  const existing = list.find((e) => e.id === id);
  const isVerify = command === "verify";
  const isUpdate = command === "update" || Boolean(existing && !isVerify);
  if ((isVerify || command === "update") && (!existing || !existsSync(ctx.manifest(id)))) {
    log("ERROR", "DOWNLOAD", "Unable to load manifest");
    log("INFO", "CLI", "Download complete");
    return 0;
  }
  if (!existing && typeof flags.path !== "string") {
    log("ERROR", "CLI", "fake nile: GRID always passes --path for a new install");
    return 2;
  }
  const latest = ctx.marker(".fake-update") ? g.fake.update_version : g.fake.version;
  if (isUpdate && !isVerify && existing.version === latest) {
    log("INFO", "DOWNLOAD", "Game is up to date");
    log("INFO", "CLI", "Download complete");
    return 0;
  }
  if (ctx.marker(".fake-no-space") && !isVerify) {
    log("ERROR", "DOWNLOAD", "Not enough space available");
    log("INFO", "CLI", "Download complete");
    return 0;
  }
  // installed.json's saved path wins over --path.
  const dir = existing ? existing.path : flags.path;
  const total = isUpdate && !isVerify ? Math.round(g.fake.download_size / 4) : g.fake.download_size;
  if (ctx.marker(".fake-download-fail")) {
    await progress(total, Math.min(2, steps), stepMs);
    return networkTraceback();
  }
  if (isVerify) log("INFO", "DOWNLOAD", "Verifying files");
  await progress(total, steps, stepMs);
  writeGame(dir, g);
  write(ctx.manifest(id), `fake manifest ${latest}\n`);
  const version = isVerify ? existing.version : latest;
  const next = list.filter((e) => e.id !== id);
  next.push({ id, version, path: dir, size: g.fake.download_size });
  ctx.saveInstalled(next);
  log("INFO", "CLI", "Download complete");
  return 0;
}

function removeEmptyDirs(dir, stop) {
  let current = dir;
  while (current.startsWith(stop) && current.length >= stop.length) {
    try { rmdirSync(current); } catch { return; }
    if (current === stop) return;
    current = path.dirname(current);
  }
}

function cmdUninstall(ctx, { positional }) {
  const id = positional[0];
  tokenLine();
  const list = ctx.installed();
  const entry = list.find((e) => e.id === id);
  if (!entry) {
    log("ERROR", "UNINSTALL", "Game isn't installed");
    return 0;
  }
  if (!existsSync(ctx.manifest(id))) return traceback("AttributeError", "'NoneType' object has no attribute 'files'");
  const g = ctx.game(id);
  if (g) {
    const exe = path.join(entry.path, ...g.fake.executable.split(/[\\/]/));
    rmSync(exe, { force: true });
    rmSync(path.join(entry.path, "fuel.json"), { force: true });
    rmSync(path.join(entry.path, POST_INSTALL_EXE), { force: true });
    removeEmptyDirs(path.dirname(exe), entry.path);
  }
  try { rmdirSync(entry.path); } catch { /* leftovers (the prefix) stay for GRID */ }
  rmSync(ctx.manifest(id), { force: true });
  ctx.saveInstalled(list.filter((e) => e.id !== id));
  log("INFO", "UNINSTALL", "Game removed successfully");
  return 0;
}

function cmdListUpdates(ctx) {
  const list = ctx.installed();
  if (!list.length) {
    jsonArray([]);
    log("ERROR", "CLI", "No games installed");
    return 0;
  }
  const need = needToken(ctx);
  if (need) return need;
  if (!refresh(ctx)) return traceback("TypeError", "'NoneType' object is not subscriptable");
  const ids = ctx.marker(".fake-update")
    ? list.filter((e) => {
        const g = ctx.game(e.id);
        return g && e.version !== g.fake.update_version;
      }).map((e) => e.id)
    : [];
  jsonArray(ids);
  return 0;
}

function cmdLaunch(ctx, { positional, flags }) {
  const id = positional[0];
  const lib = ctx.library();
  if (!lib) return traceback("AttributeError", "'NoneType' object has no attribute 'get'");
  const entry = lib.find((e) => e.product.id === id);
  if (!entry) {
    log("ERROR", "CLI", "No game match");
    return 0;
  }
  const list = ctx.installed();
  const inst = list.find((e) => e.id === id);
  if (!list.length || !inst) {
    log("ERROR", "CLI", "No game is installed");
    return 0;
  }
  if (!flags.json) {
    log("ERROR", "CLI", "fake nile: GRID launches the game itself (--json)");
    return 2;
  }
  let fuel;
  try {
    // JSON5 (as nile reads it): drop the comments and trailing commas.
    const text = readFileSync(path.join(inst.path, "fuel.json"), "utf8")
      .replace(/^\s*\/\/.*$/gm, "")
      .replace(/,(\s*[}\]])/g, "$1");
    fuel = JSON.parse(text);
  } catch {
    return traceback("FileNotFoundError", "fuel.json");
  }
  const user = ctx.user();
  const sdk = path.join(ctx.dir, "SDK", "Amazon Games Services");
  json({
    command: {
      instruction: path.join(inst.path, ...fuel.Main.Command.split(/[\\/]/)),
      arguments: fuel.Main.Args ?? [],
    },
    game_directory: inst.path,
    env: {
      FUEL_DIR: path.join(sdk, "Legacy"),
      AMAZON_GAMES_SDK_PATH: path.join(sdk, "AmazonGamesSDK"),
      AMAZON_GAMES_FUEL_ENTITLEMENT_ID: entry.id,
      AMAZON_GAMES_FUEL_PRODUCT_SKU: entry.product.sku,
      AMAZON_GAMES_FUEL_DISPLAY_NAME: user ? user.name : "",
    },
  });
  return 0;
}

export async function main(argv, env = process.env) {
  if (env.E2E_NILE_ARGV_FILE) {
    try { appendFileSync(env.E2E_NILE_ARGV_FILE, `${argv.join(" ")}\n`); } catch { /* the log is only for specs */ }
  }
  const args = parseArgs(argv);
  if (args.flags.version) {
    out(`${VERSION}\n`);
    log("DEBUG", "CLI", `session ${FAKE_TOKEN}`);
    return 0;
  }
  if (args.flags.d || args.flags.debug) {
    log("ERROR", "CLI", "fake nile: GRID must never pass -d (nile logs response bodies at DEBUG)");
    return 2;
  }
  const ctx = new Ctx(env);
  mkdirSync(ctx.dir, { recursive: true });
  // The id migration nile runs on every start rewrites installed.json.
  ctx.saveInstalled(ctx.installed());
  // Every command logs the token (and puts it on stdout below).
  log("DEBUG", "AUTH_MANAGER", `Loaded session ${FAKE_TOKEN}`);
  if (!existsSync(path.join(ctx.dir, "SDK"))) log("WARNING", "CLI", "There was an error getting sdk");
  switch (args.command) {
    case "auth": return cmdAuth(ctx, args);
    case "register": return cmdRegister(ctx, args);
    case "library": return cmdLibrary(ctx, args);
    case "install":
    case "update":
    case "verify": return cmdInstall(ctx, args);
    case "uninstall": return cmdUninstall(ctx, args);
    case "list-updates": return cmdListUpdates(ctx, args);
    case "launch": return cmdLaunch(ctx, args);
    default:
      log("ERROR", "CLI", `unknown command ${args.command ?? ""}`);
      return 2;
  }
}

if (process.argv[1] && import.meta.url === `file://${process.argv[1]}`) {
  main(process.argv.slice(2)).then((code) => process.exit(code));
}
