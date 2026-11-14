// node --test e2e/fake-tools/nile.test.mjs
import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { parseArgs, CLIENT_ID, FAKE_TOKEN, GOOD_CODE, SERIAL, USERNAME } from "./nile.mjs";

const here = path.dirname(fileURLToPath(import.meta.url));
const tool = path.join(here, "nile.mjs");
const FOG = "amzn1.adg.product.5b0f1a2c-3d4e-4f50-8a6b-0000000000a1";
const KETTLE = "amzn1.adg.product.5b0f1a2c-3d4e-4f50-8a6b-0000000000a2";
const BASE = "http://127.0.0.1:4400";

function fresh() {
  return mkdtempSync(path.join(tmpdir(), "fake-nile-"));
}

/** nile's files: NILE_CONFIG_PATH plus the `/nile` nile appends. */
const cfg = (dir) => path.join(dir, "stores", "nile", "nile");

function run(dir, args, { env = {} } = {}) {
  return spawnSync(process.execPath, [tool, ...args], {
    env: {
      ...process.env,
      NILE_CONFIG_PATH: path.join(dir, "stores", "nile"),
      FAKE_NILE_STEP_MS: "1",
      GRID_LAUNCHER_E2E_AMAZON_BASE: `${BASE}/`,
      ...env,
    },
    input: "",
    encoding: "utf8",
  });
}

/** The first JSON value on stdout (GRID's parsers skip the token lines). */
function firstJson(stdout) {
  const line = stdout.split("\n").find((l) => l.startsWith("{") || l.startsWith("["));
  assert.ok(line, `no JSON on stdout: ${stdout}`);
  return JSON.parse(line);
}

const registerArgs = (code = GOOD_CODE, verifier = FAKE_TOKEN) => [
  "register", "--code", code, "--code-verifier", verifier, "--serial", SERIAL, "--client-id", CLIENT_ID,
];

function signIn(dir) {
  const ok = run(dir, registerArgs());
  assert.equal(ok.status, 0, ok.stderr);
  assert.match(ok.stderr, /^INFO \[AUTH_MANAGER\]:\t Succesfully registered a device$/m);
  return ok;
}

test("flags parse in any order, with values and switches", () => {
  const parsed = parseArgs(["install", "--path", "/g/Fogline", FOG, "--max-workers", "4"]);
  assert.equal(parsed.command, "install");
  assert.deepEqual(parsed.positional, [FOG]);
  assert.equal(parsed.flags.path, "/g/Fogline");
  assert.equal(parsed.flags["max-workers"], "4");
  const auth = parseArgs(["auth", "--login", "--non-interactive"]);
  assert.equal(auth.flags.login, true);
  assert.equal(auth.flags["non-interactive"], true);
  assert.equal(parseArgs(["-d", "auth", "--status"]).flags.d, true);
});

test("every command prints the fake token on stderr and stdout, and the SDK warning", () => {
  const dir = fresh();
  for (const args of [["auth", "--status"], ["auth", "--logout"], ["list-updates", "--json"], ["auth", "--login", "--non-interactive"], ["uninstall", FOG]]) {
    const out = run(dir, args);
    assert.match(out.stderr, new RegExp(FAKE_TOKEN), args.join(" "));
    assert.match(out.stdout, new RegExp(FAKE_TOKEN), args.join(" "));
    assert.match(out.stderr, /^WARNING \[CLI\]:\t There was an error getting sdk$/m);
  }
  const version = run(dir, ["--version"]);
  assert.equal(version.stdout, "1.2.0 Robert Speedwagon\n");
  assert.match(version.stderr, new RegExp(FAKE_TOKEN));
});

test("files land under <NILE_CONFIG_PATH>/nile, written 0644 as nile does", () => {
  const dir = fresh();
  signIn(dir);
  assert.deepEqual(
    readdirSync(cfg(dir)).sort(),
    ["current_user.json", "installed.json", "library.json", "syncpoint.raw", readdirSync(cfg(dir)).find((n) => n.endsWith(".enc"))].sort(),
  );
  const enc = readdirSync(cfg(dir)).find((n) => n.endsWith(".enc"));
  assert.match(enc, /^[0-9a-f]{32}\.enc$/);
  assert.match(readFileSync(path.join(cfg(dir), enc), "utf8"), new RegExp(FAKE_TOKEN));
  assert.equal(statSync(path.join(cfg(dir), enc)).mode & 0o777, 0o644, "GRID tightens it");
  assert.equal(statSync(path.join(cfg(dir), "library.json")).mode & 0o777, 0o644);
  const env = { NILE_CONFIG_PATH: "", XDG_CONFIG_HOME: path.join(dir, "xdg") };
  run(dir, ["auth", "--status"], { env });
  assert.ok(existsSync(path.join(dir, "xdg", "nile", "installed.json")), "XDG_CONFIG_HOME/nile without NILE_CONFIG_PATH");
});

test("refuses -d, --base-path, a new install without --path and the interactive login", () => {
  const dir = fresh();
  assert.equal(run(dir, ["-d", "auth", "--status"]).status, 2);
  signIn(dir);
  assert.equal(run(dir, ["install", "--base-path", path.join(dir, "g"), FOG]).status, 2);
  assert.equal(run(dir, ["install", FOG]).status, 2);
  assert.equal(run(dir, ["auth", "--login"]).status, 2);
});

test("auth --login prints nile's JSON: the mock's sign-in page and the fake token as the verifier", () => {
  const dir = fresh();
  const out = run(dir, ["auth", "--login", "--non-interactive"]);
  assert.equal(out.status, 0, out.stderr);
  const start = firstJson(out.stdout);
  assert.equal(start.code_verifier, FAKE_TOKEN);
  assert.equal(start.serial, SERIAL);
  assert.equal(start.client_id, CLIENT_ID);
  assert.equal(start.client_id.length, 94);
  const url = new URL(start.url);
  assert.equal(url.origin, BASE);
  assert.equal(url.pathname, "/ap/signin");
  assert.equal(url.searchParams.get("openid.assoc_handle"), "amzn_sonic_games_launcher");
  assert.equal(url.searchParams.get("openid.oa2.client_id"), `device:${CLIENT_ID}`);
  assert.equal(url.searchParams.get("openid.return_to"), "https://www.amazon.com");
  // Without the e2e base: Amazon's own host, as nile prints it.
  const real = firstJson(run(dir, ["auth", "--login", "--non-interactive"], { env: { GRID_LAUNCHER_E2E_AMAZON_BASE: "" } }).stdout);
  assert.equal(new URL(real.url).host, "amazon.com");
});

test("register: the good code with the printed values signs in and syncs; anything else fails with exit 0", () => {
  const dir = fresh();
  for (const args of [registerArgs("FAKE-E2E-AMAZON-CODE-refused"), registerArgs(GOOD_CODE, "other-verifier")]) {
    const bad = run(dir, args);
    assert.equal(bad.status, 0);
    assert.match(bad.stderr, /^ERROR \[AUTH_MANAGER\]:\t Failed to register a device$/m);
    assert.match(firstJson(bad.stdout).response.error.code, /InvalidValue/);
    assert.doesNotMatch(bad.stderr, /FAKE-E2E-AMAZON-CODE/, "nile never echoes the code");
    assert.ok(!existsSync(path.join(cfg(dir), "current_user.json")));
  }
  const ok = signIn(dir);
  assert.doesNotMatch(ok.stderr, new RegExp(GOOD_CODE));
  assert.match(ok.stderr, new RegExp(`Registerring a device\\. ID: ${CLIENT_ID}`));
  const status = firstJson(run(dir, ["auth", "--status"]).stdout);
  assert.deepEqual([status.Username, status.LoggedIn], [USERNAME, true]);
  assert.equal(status.fake_session, FAKE_TOKEN);
  const library = JSON.parse(readFileSync(path.join(cfg(dir), "library.json"), "utf8"));
  assert.equal(library.length, 4, "with Fogline's second entitlement (GRID dedups)");
  assert.ok(library.every((e) => !("fake" in e)), "nile's own shape");
});

test("library sync: signed out exits 1; an expired token logs the refresh error; the list has the art at the mock", () => {
  const dir = fresh();
  const out = run(dir, ["library", "sync"]);
  assert.equal(out.status, 1);
  assert.match(out.stderr, /^ERROR \[CLI\]:\t User not logged in$/m);
  assert.equal(run(dir, ["library", "list", "--json"]).status, 1, "no library.json: nile's crash");
  signIn(dir);
  writeFileSync(path.join(cfg(dir), ".fake-expired"), "");
  const expired = run(dir, ["library", "sync", "--force"]);
  assert.equal(expired.status, 0);
  assert.match(expired.stderr, /^ERROR \[AUTH_MANAGER\]:\t Failed to refresh the token <Response \[400\]>$/m);
  assert.match(expired.stderr, /There was an error syncing library/);
  const list = firstJson(run(dir, ["library", "list", "--json"]).stdout);
  const titles = [...new Set(list.map((e) => e.product.title))];
  assert.deepEqual(titles, ["Fogline", "Kettle Drum", "Lowsun"]);
  assert.equal(list[0].product.productDetail.iconUrl, `${BASE}/art/fogline-icon.jpg`);
  assert.equal(list[0].product.id, FOG);
});

test("install --info, install, update and verify keep installed.json, the manifest and fuel.json", () => {
  const dir = fresh();
  signIn(dir);
  assert.equal(firstJson(run(dir, ["install", "--info", "--json", KETTLE]).stdout).download_size, 2200000);
  const games = path.join(dir, "games");
  const fogDir = path.join(games, "Fogline");
  const out = run(dir, ["install", "--path", fogDir, FOG]);
  assert.equal(out.status, 0, out.stderr);
  assert.match(out.stderr, /^INFO \[PROGRESS\]:\t = Progress: 100\.00 7000000\/7000000, Running for: /m);
  assert.match(out.stderr, /^INFO \[PROGRESS\]:\t  \+ Download\t- 41\.20 MiB\/s$/m);
  assert.match(out.stderr, /^INFO \[CLI\]:\t Download complete$/m);
  assert.ok(existsSync(path.join(fogDir, "bin", "Fogline.exe")));
  const fuel = readFileSync(path.join(fogDir, "fuel.json"), "utf8");
  assert.match(fuel, /\/\/ written by the fake nile/);
  assert.match(fuel, /"Command": "bin\\\\Fogline\.exe"/);
  assert.match(fuel, /"WorkingSubdirOverride": "bin",/);
  // Hotline Miami's shape: a PostInstall step GRID runs silently.
  assert.match(fuel, /"PostInstall": \[\n\s+\{"Command": "oalinst\.exe", "Args": \["\/silent"\],\},/);
  assert.ok(existsSync(path.join(fogDir, "oalinst.exe")));
  let installed = JSON.parse(readFileSync(path.join(cfg(dir), "installed.json"), "utf8"));
  assert.deepEqual(installed, [{ id: FOG, version: "e2e-fogline-version-1", path: fogDir, size: 7000000 }]);
  assert.ok(existsSync(path.join(cfg(dir), "manifests", `${FOG}.raw`)));

  // Nothing newer: up to date.
  assert.match(run(dir, ["update", FOG]).stderr, /^INFO \[DOWNLOAD\]:\t Game is up to date$/m);
  assert.deepEqual(firstJson(run(dir, ["list-updates", "--json"]).stdout), []);
  writeFileSync(path.join(cfg(dir), ".fake-update"), "");
  assert.deepEqual(firstJson(run(dir, ["list-updates", "--json"]).stdout), [FOG]);
  assert.equal(run(dir, ["update", FOG]).status, 0);
  installed = JSON.parse(readFileSync(path.join(cfg(dir), "installed.json"), "utf8"));
  assert.equal(installed[0].version, "e2e-fogline-version-2");
  assert.deepEqual(firstJson(run(dir, ["list-updates", "--json"]).stdout), []);

  // Verify puts a missing file back, same version.
  writeFileSync(path.join(fogDir, "bin", "Fogline.exe"), "damaged");
  const verify = run(dir, ["verify", FOG]);
  assert.equal(verify.status, 0, verify.stderr);
  assert.match(verify.stderr, /Verifying files/);
  assert.equal(readFileSync(path.join(fogDir, "bin", "Fogline.exe"), "utf8"), "MZ fake amazon game\n");
  assert.equal(JSON.parse(readFileSync(path.join(cfg(dir), "installed.json"), "utf8"))[0].version, "e2e-fogline-version-2");
  // verify of a game that isn't installed: nile's "Download complete" after a failure.
  const missing = run(dir, ["verify", KETTLE]);
  assert.match(missing.stderr, /Unable to load manifest[\s\S]*Download complete/);
});

test("failures: no space and a dropped download, nile's way", () => {
  const dir = fresh();
  signIn(dir);
  const games = path.join(dir, "games");
  writeFileSync(path.join(cfg(dir), ".fake-no-space"), "");
  const space = run(dir, ["install", "--path", path.join(games, "Kettle Drum"), KETTLE]);
  assert.equal(space.status, 0);
  assert.match(space.stderr, /^ERROR \[DOWNLOAD\]:\t Not enough space available$/m);
  assert.match(space.stderr, /Download complete/);
  assert.deepEqual(JSON.parse(readFileSync(path.join(cfg(dir), "installed.json"), "utf8")), []);
  rmSync(path.join(cfg(dir), ".fake-no-space"));
  writeFileSync(path.join(cfg(dir), ".fake-download-fail"), "");
  const failed = run(dir, ["install", "--path", path.join(games, "Lowsun"), "amzn1.adg.product.5b0f1a2c-3d4e-4f50-8a6b-0000000000a3"]);
  assert.equal(failed.status, 1);
  assert.match(failed.stderr, /^requests\.exceptions\.ConnectionError: /m);
  // Signed out, an install dies on the token read.
  const out = fresh();
  run(out, ["auth", "--status"]);
  writeFileSync(path.join(cfg(out), "library.json"), readFileSync(path.join(cfg(dir), "library.json")));
  assert.equal(run(out, ["install", "--path", path.join(out, "g"), KETTLE]).status, 1);
});

test("launch --json --no-wine gives the dry run with the five FUEL variables", () => {
  const dir = fresh();
  signIn(dir);
  const fogDir = path.join(dir, "games", "Fogline");
  assert.match(run(dir, ["launch", "--json", "--no-wine", FOG]).stderr, /No game is installed/);
  run(dir, ["install", "--path", fogDir, FOG]);
  const out = run(dir, ["launch", "--json", "--no-wine", FOG]);
  assert.equal(out.status, 0, out.stderr);
  const prep = firstJson(out.stdout);
  assert.equal(prep.command.instruction, path.join(fogDir, "bin", "Fogline.exe"));
  assert.deepEqual(prep.command.arguments, ["-fuel-e2e"]);
  assert.equal(prep.game_directory, fogDir);
  assert.deepEqual(Object.keys(prep.env).sort(), [
    "AMAZON_GAMES_FUEL_DISPLAY_NAME", "AMAZON_GAMES_FUEL_ENTITLEMENT_ID", "AMAZON_GAMES_FUEL_PRODUCT_SKU",
    "AMAZON_GAMES_SDK_PATH", "FUEL_DIR",
  ]);
  assert.equal(prep.env.AMAZON_GAMES_FUEL_DISPLAY_NAME, USERNAME);
  assert.equal(prep.env.AMAZON_GAMES_FUEL_PRODUCT_SKU, "FAKE-SKU-FOGLINE");
  assert.equal(prep.env.FUEL_DIR, path.join(cfg(dir), "SDK", "Amazon Games Services", "Legacy"));
  assert.match(run(dir, ["launch", "--json", "--no-wine", "amzn1.adg.product.nope"]).stderr, /No game match/);
  // Signed out, it still works (nile reads only its local files), with no name.
  run(dir, ["auth", "--logout"]);
  assert.equal(firstJson(run(dir, ["launch", "--json", "--no-wine", FOG]).stdout).env.AMAZON_GAMES_FUEL_DISPLAY_NAME, "");
});

test("uninstall: not installed is an error line with exit 0; a missing manifest is a traceback", () => {
  const dir = fresh();
  signIn(dir);
  const fogDir = path.join(dir, "games", "Fogline");
  run(dir, ["install", "--path", fogDir, FOG]);
  const gone = run(dir, ["uninstall", FOG]);
  assert.equal(gone.status, 0, gone.stderr);
  assert.ok(!existsSync(fogDir));
  assert.deepEqual(JSON.parse(readFileSync(path.join(cfg(dir), "installed.json"), "utf8")), []);
  assert.ok(!existsSync(path.join(cfg(dir), "manifests", `${FOG}.raw`)));
  const again = run(dir, ["uninstall", FOG]);
  assert.equal(again.status, 0);
  assert.match(again.stderr, /^ERROR \[UNINSTALL\]:\t Game isn't installed$/m);
  run(dir, ["install", "--path", fogDir, FOG]);
  rmSync(path.join(cfg(dir), "manifests", `${FOG}.raw`));
  const crash = run(dir, ["uninstall", FOG]);
  assert.equal(crash.status, 1);
  assert.match(crash.stderr, /^AttributeError: /m);
  assert.ok(existsSync(path.join(fogDir, "bin", "Fogline.exe")), "files untouched");
});

test("list-updates with nothing installed prints [] and nile's error line", () => {
  const dir = fresh();
  const out = run(dir, ["list-updates", "--json"]);
  assert.equal(out.status, 0);
  assert.deepEqual(firstJson(out.stdout), []);
  assert.match(out.stderr, /^ERROR \[CLI\]:\t No games installed$/m);
});

test("auth --logout deletes the login; with .fake-logout-fail it dies first and keeps it", () => {
  const dir = fresh();
  signIn(dir);
  writeFileSync(path.join(cfg(dir), ".fake-logout-fail"), "");
  const failed = run(dir, ["auth", "--logout"]);
  assert.equal(failed.status, 1);
  assert.ok(existsSync(path.join(cfg(dir), "current_user.json")));
  const ok = fresh();
  signIn(ok);
  assert.equal(run(ok, ["auth", "--logout"]).status, 0);
  assert.ok(!existsSync(path.join(cfg(ok), "current_user.json")));
  assert.ok(!readdirSync(cfg(ok)).some((n) => n.endsWith(".enc")));
  assert.equal(firstJson(run(ok, ["auth", "--status"]).stdout).LoggedIn, false);
  assert.match(run(ok, ["auth", "--logout"]).stderr, /You are not logged in/);
});
