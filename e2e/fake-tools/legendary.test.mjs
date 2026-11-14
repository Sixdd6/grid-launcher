// node --test e2e/fake-tools/legendary.test.mjs
import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { parseArgs, FAKE_TOKEN, GOOD_CODE } from "./legendary.mjs";

const here = path.dirname(fileURLToPath(import.meta.url));
const tool = path.join(here, "legendary.mjs");

function fresh() {
  return mkdtempSync(path.join(tmpdir(), "fake-legendary-"));
}

function run(dir, args, { input, env = {} } = {}) {
  return spawnSync(process.execPath, [tool, ...args], {
    env: {
      ...process.env,
      LEGENDARY_CONFIG_PATH: path.join(dir, "legendary"),
      FAKE_LEGENDARY_STEP_MS: "1",
      GRID_LAUNCHER_E2E_EPIC_BASE: "http://127.0.0.1:4300/",
      ...env,
    },
    input: input ?? "",
    encoding: "utf8",
  });
}

function signIn(dir) {
  const ok = run(dir, ["auth", "--disable-webview"], { input: `${GOOD_CODE}\n` });
  assert.equal(ok.status, 0, ok.stderr);
  return ok;
}

test("flags parse in any order, with values and short switches", () => {
  const parsed = parseArgs(["install", "Glimmer", "-y", "--skip-sdl", "--base-path", "/g", "--game-folder=Glimmer Deep", "--platform", "Windows"]);
  assert.equal(parsed.command, "install");
  assert.deepEqual(parsed.positional, ["Glimmer"]);
  assert.equal(parsed.flags.y, true);
  assert.equal(parsed.flags["skip-sdl"], true);
  assert.equal(parsed.flags["base-path"], "/g");
  assert.equal(parsed.flags["game-folder"], "Glimmer Deep");
  assert.equal(parseArgs(["list", "--json", "-T"]).flags.T, true);
});

test("every command prints the fake token on stderr", () => {
  const dir = fresh();
  for (const args of [["status", "--json", "--offline"], ["list-installed", "--json"], ["--version"], ["auth", "--delete"]]) {
    const out = run(dir, args);
    assert.match(out.stderr, new RegExp(FAKE_TOKEN), args.join(" "));
  }
});

test("auth reads the code from stdin: the good one signs in, any other fails with exit 0", () => {
  const dir = fresh();
  const bad = run(dir, ["auth", "--disable-webview"], { input: "nope\n" });
  assert.equal(bad.status, 0);
  assert.match(bad.stderr, /\[cli\] ERROR: Login attempt failed/);
  assert.doesNotMatch(bad.stderr, /nope/);
  assert.ok(!existsSync(path.join(dir, "legendary", "user.json")));
  assert.equal(run(dir, ["auth", "--disable-webview"]).status, 1, "empty stdin is an EOFError");
  assert.equal(run(dir, ["auth", "--code", GOOD_CODE]).status, 2, "never on argv");

  const ok = signIn(dir);
  assert.match(ok.stdout, new RegExp(FAKE_TOKEN));
  assert.match(ok.stderr, /Successfully logged in as "SamPlays"/);
  const user = path.join(dir, "legendary", "user.json");
  assert.match(readFileSync(user, "utf8"), new RegExp(FAKE_TOKEN));
  assert.equal(statSync(user).mode & 0o777, 0o644, "written as legendary does: GRID tightens it");
  // The JSON blob works too.
  const blob = fresh();
  assert.equal(run(blob, ["auth", "--disable-webview"], { input: JSON.stringify({ authorizationCode: GOOD_CODE }) }).status, 0);
  assert.ok(existsSync(path.join(blob, "legendary", "user.json")));

  const status = JSON.parse(run(dir, ["status", "--json"]).stdout);
  assert.equal(status.account, "SamPlays");
  assert.equal(status.fake_session, FAKE_TOKEN);
  assert.equal(run(dir, ["auth", "--delete"]).status, 0);
  assert.equal(JSON.parse(run(dir, ["status", "--json", "--offline"]).stdout).account, "<not logged in>");
});

test("signed out, the commands that need a login die with legendary's traceback", () => {
  const dir = fresh();
  for (const args of [["list", "--json", "-T"], ["list-saves", "Starlight"], ["install", "Glimmer", "-y"]]) {
    const out = run(dir, args);
    assert.equal(out.status, 1, args.join(" "));
    assert.match(out.stderr, /ValueError: No saved credentials/);
  }
});

test("an expired login fails the online status and drops user.json", () => {
  const dir = fresh();
  signIn(dir);
  writeFileSync(path.join(dir, "legendary", ".fake-expired"), "");
  assert.equal(run(dir, ["status", "--json", "--offline"]).status, 0);
  const out = run(dir, ["status", "--json"]);
  assert.equal(out.status, 1);
  assert.match(out.stderr, /Stored credentials are no longer valid/);
  assert.ok(!existsSync(path.join(dir, "legendary", "user.json")));
});

test("the owned list has the six games, third-party ones only with -T, art at the mock", () => {
  const dir = fresh();
  signIn(dir);
  const all = JSON.parse(run(dir, ["list", "--json", "-T"]).stdout);
  assert.deepEqual(all.map((g) => g.app_title), ["Starlight Courier", "Vector Bloom", "Glimmer Deep", "Iron Orchard", "Northern Static", "Rustbelt Rally"]);
  assert.equal(JSON.parse(run(dir, ["list", "--json"]).stdout).length, 4);
  const glimmer = all.find((g) => g.app_name === "Glimmer");
  assert.equal(glimmer.asset_infos.Windows.build_version, "2.4");
  assert.equal(glimmer.dlcs[0].app_name, "GlimmerAbyss");
  assert.equal(glimmer.metadata.customAttributes.CloudSaveFolder.value, "{UserDir}/My Games/GlimmerDeep");
  assert.equal(glimmer.metadata.keyImages[0].url, "http://127.0.0.1:4300/art/Glimmer_tall.jpg");
  assert.equal(all.find((g) => g.app_name === "IronOrchard").metadata.customAttributes.ThirdPartyManagedApp.value, "Origin");
  assert.equal(glimmer.fake_session, FAKE_TOKEN);
});

test("info gives the sizes, also signed out", () => {
  const dir = fresh();
  const info = JSON.parse(run(dir, ["info", "Glimmer", "--json"]).stdout);
  assert.equal(info.manifest.download_size, 45000000);
  assert.equal(info.manifest.disk_size, 60000000);
  assert.equal(info.game.owned_dlc[0].app_name, "GlimmerAbyss");
  const dlc = JSON.parse(run(dir, ["info", "GlimmerAbyss", "--json"]).stdout);
  assert.equal(dlc.manifest.download_size, 3200000);
  assert.equal(JSON.parse(run(dir, ["info", "Nope", "--json"]).stdout).game, null);
});

test("install, DLC, update, repair and uninstall keep installed.json and the files", () => {
  const dir = fresh();
  signIn(dir);
  const games = path.join(dir, "games");
  const out = run(dir, ["install", "Glimmer", "-y", "--skip-sdl", "--skip-dlcs", "--platform", "Windows", "--base-path", games, "--game-folder", "Glimmer Deep"]);
  assert.equal(out.status, 0, out.stderr);
  assert.match(out.stderr, /\[cli\] INFO: Download size: 42\.92 MiB/);
  assert.match(out.stderr, /\[DLManager\] INFO: = Progress: 100\.00% \(50\/50\)/);
  assert.match(out.stderr, /\+ Download\t- 41\.20 MiB\/s \(raw\)/);
  assert.ok(existsSync(path.join(games, "Glimmer Deep", "Game", "Glimmer.exe")));
  let installed = JSON.parse(run(dir, ["list-installed", "--json"]).stdout);
  assert.deepEqual(installed.map((e) => e.app_name), ["Glimmer"], "--skip-dlcs");
  assert.equal(installed[0].install_path, path.join(games, "Glimmer Deep"));
  assert.equal(installed[0].executable, "Game/Glimmer.exe");
  assert.equal(installed[0].version, "2.4");

  assert.equal(run(dir, ["install", "GlimmerAbyss", "-y", "--skip-sdl"]).status, 0);
  installed = JSON.parse(run(dir, ["list-installed", "--json"]).stdout);
  assert.ok(installed.find((e) => e.app_name === "GlimmerAbyss").is_dlc);

  const repair = run(dir, ["repair", "Glimmer", "-y", "--skip-sdl"]);
  assert.equal(repair.status, 0, repair.stderr);
  assert.match(repair.stdout, /Verification progress: 50\/50 \(100\.0%\) \[120\.0 MiB\/s\]/);
  assert.match(repair.stderr, /Verification failed, 2 file\(s\) corrupted, 0 file\(s\) are missing\./);
  const clean = run(dir, ["repair", "Glimmer", "-y"], { env: { FAKE_LEGENDARY_REPAIRED: "0" } });
  assert.match(clean.stderr, /Verification finished successfully\./);

  const third = run(dir, ["install", "IronOrchard", "-y"]);
  assert.equal(third.status, 0);
  assert.match(third.stderr, /third-party store: Origin/);

  const gone = run(dir, ["uninstall", "Glimmer", "-y", "--skip-uninstaller"]);
  assert.equal(gone.status, 0);
  assert.deepEqual(JSON.parse(run(dir, ["list-installed", "--json"]).stdout), []);
  assert.ok(!existsSync(path.join(games, "Glimmer Deep", "Game", "Glimmer.exe")));
  const again = run(dir, ["uninstall", "Glimmer", "-y"]);
  assert.equal(again.status, 0);
  assert.match(again.stderr, /not installed, cannot uninstall!/);
});

test("an update moves the version to the owned build; a download failure is a network traceback", () => {
  const dir = fresh();
  signIn(dir);
  const games = path.join(dir, "games");
  const cfg = path.join(dir, "legendary");
  writeFileSync(path.join(cfg, "installed.json"), JSON.stringify({
    Vector: { app_name: "Vector", title: "Vector Bloom", version: "3.0.1", install_path: path.join(games, "Vector Bloom"), executable: "VectorBloom.exe", install_size: 1, is_dlc: false, platform: "Windows" },
  }));
  assert.equal(run(dir, ["update", "Vector", "-y", "--skip-sdl"]).status, 0);
  assert.equal(JSON.parse(run(dir, ["list-installed", "--json"]).stdout)[0].version, "3.0.2");
  writeFileSync(path.join(cfg, ".fake-download-fail"), "");
  const failed = run(dir, ["update", "Vector", "-y"]);
  assert.equal(failed.status, 1);
  assert.match(failed.stderr, /^requests\.exceptions\.ConnectionError: /m);
});

test("launch --json carries the exchange code in egl_parameters, never offline", () => {
  const dir = fresh();
  signIn(dir);
  const games = path.join(dir, "games");
  run(dir, ["install", "Starlight", "-y", "--skip-sdl", "--base-path", games, "--game-folder", "Starlight Courier"]);
  const params = JSON.parse(run(dir, ["launch", "Starlight", "--json", "--skip-version-check"]).stdout);
  assert.equal(params.game_executable, "Binaries/Win64/Starlight.exe");
  assert.equal(params.game_directory, path.join(games, "Starlight Courier"));
  assert.equal(params.working_directory, path.join(games, "Starlight Courier", "Binaries", "Win64"));
  assert.ok(params.egl_parameters.includes(`-AUTH_PASSWORD=${FAKE_TOKEN}`));
  const offline = JSON.parse(run(dir, ["launch", "Starlight", "--json", "--offline"]).stdout);
  assert.ok(!offline.egl_parameters.some((p) => p.includes(FAKE_TOKEN)));
  assert.equal(run(dir, ["launch", "Glimmer", "--json"]).status, 1);
  writeFileSync(path.join(dir, "legendary", ".fake-launch-network"), "");
  assert.match(run(dir, ["launch", "Starlight", "--json"]).stderr, /requests\.exceptions\.ConnectionError/);
  assert.equal(run(dir, ["launch", "Starlight", "--json", "--offline"]).status, 0);
});

test("sync-saves uploads to and downloads from the fake cloud; list-saves names the manifests", () => {
  const dir = fresh();
  signIn(dir);
  const games = path.join(dir, "games");
  run(dir, ["install", "Starlight", "-y", "--skip-sdl", "--base-path", games, "--game-folder", "Starlight Courier"]);
  const saves = path.join(dir, "saves");
  mkdirSync(saves, { recursive: true });
  writeFileSync(path.join(saves, "slot1.sav"), "local save");
  assert.doesNotMatch(run(dir, ["list-saves", "Starlight"]).stdout, /\.manifest/);
  const up = run(dir, ["sync-saves", "Starlight", "-y", "--save-path", saves, "--force-upload"]);
  assert.equal(up.status, 0, up.stderr);
  const listed = run(dir, ["list-saves", "Starlight"]).stdout;
  assert.match(listed, /^Save games:$/m);
  assert.match(listed, /^- Starlight Courier \("Starlight"\)$/m);
  assert.match(listed, /^ \+ \d{4}\.\d{2}\.\d{2}-\d{2}\.\d{2}\.\d{2}\.manifest$/m);
  writeFileSync(path.join(saves, "stray.tmp"), "x");
  writeFileSync(path.join(saves, "slot1.sav"), "changed");
  const down = run(dir, ["sync-saves", "Starlight", "-y", "--save-path", saves, "--force-download"]);
  assert.equal(down.status, 0, down.stderr);
  assert.equal(readFileSync(path.join(saves, "slot1.sav"), "utf8"), "local save");
  assert.ok(!existsSync(path.join(saves, "stray.tmp")), "a download cleans the folder first");
  assert.equal(JSON.parse(run(dir, ["list-installed", "--json"]).stdout)[0].save_path, saves);
});
