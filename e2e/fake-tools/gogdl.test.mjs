// node --test e2e/fake-tools/gogdl.test.mjs
import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, existsSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { parseArgs, FAKE_TOKEN, GOOD_CODE } from "./gogdl.mjs";

const here = path.dirname(fileURLToPath(import.meta.url));
const tool = path.join(here, "gogdl.mjs");

function run(args, env = {}) {
  return spawnSync(process.execPath, [tool, ...args], {
    env: { ...process.env, FAKE_GOGDL_STEP_MS: "1", ...env },
    encoding: "utf8",
  });
}

test("the auth config path is top-level only", () => {
  const parsed = parseArgs(["--auth-config-path", "/a.json", "info", "1", "--platform", "linux", "--with-dlcs"]);
  assert.equal(parsed.top["auth-config-path"], "/a.json");
  assert.equal(parsed.command, "info");
  assert.deepEqual(parsed.positional, ["1"]);
  assert.equal(parsed.flags["with-dlcs"], true);
});

test("auth with the good code writes a private token file and prints the token", () => {
  const dir = mkdtempSync(path.join(tmpdir(), "fake-gogdl-"));
  const auth = path.join(dir, "auth.json");
  const ok = run(["--auth-config-path", auth, "auth", "--code", GOOD_CODE]);
  assert.equal(ok.status, 0);
  assert.match(ok.stdout, new RegExp(FAKE_TOKEN));
  assert.equal(statSync(auth).mode & 0o777, 0o600);
  const again = run(["--auth-config-path", auth, "auth"]);
  assert.equal(again.status, 0);
  const refused = run(["--auth-config-path", path.join(dir, "b.json"), "auth", "--code", "nope"]);
  assert.equal(refused.status, 1);
  assert.equal(run(["--auth-config-path", auth, "auth"], { FAKE_GOGDL_EXPIRED: "1" }).status, 1);
});

test("info prints one JSON line", () => {
  const out = run(["--auth-config-path", "/x", "info", "1453375253", "--platform", "windows", "--with-dlcs"]);
  const info = JSON.parse(out.stdout.trim());
  assert.equal(info.buildId, "56012345678905678");
  assert.equal(info.dlcs[0].id, "1453375254");
  assert.equal(run(["--auth-config-path", "/x", "info", "1580232252", "--platform", "linux"]).status, 1);
});

test("download prints progress on stderr and writes the tree and the manifest", () => {
  const dir = mkdtempSync(path.join(tmpdir(), "fake-gogdl-"));
  const out = run(
    ["--auth-config-path", "/x", "download", "1453375253", "--platform", "linux", "--path", dir, "--with-dlcs", "--dlcs", "1453375254"],
    { GOGDL_CONFIG_PATH: path.join(dir, "cfg") },
  );
  assert.equal(out.status, 0, out.stderr);
  assert.match(out.stderr, /\[PROGRESS\] INFO: = Progress: 100\.00 2097152\/2097152/);
  const game = path.join(dir, "Lanternfall");
  const info = JSON.parse(readFileSync(path.join(game, "goggame-1453375253.info"), "utf8"));
  assert.equal(info.playTasks[0].path, "start.sh");
  assert.ok(existsSync(path.join(game, "goggame-1453375254.info")));
  assert.ok(existsSync(path.join(game, ".gogdl-linux-manifest")));
  const win = run(["--auth-config-path", "/x", "update", "1580232252", "--platform", "windows", "--path", path.join(dir, "Grey"), "--skip-dlcs"], { GOGDL_CONFIG_PATH: path.join(dir, "cfg") });
  assert.equal(win.status, 0, win.stderr);
  assert.ok(existsSync(path.join(dir, "Grey", "goggame-1580232252.info")));
  assert.ok(existsSync(path.join(dir, "cfg", "heroic_gogdl", "manifests", "1580232252")));
});

test("a worker failure exits 0 without a manifest", () => {
  const dir = mkdtempSync(path.join(tmpdir(), "fake-gogdl-"));
  const out = run(["--auth-config-path", "/x", "download", "1207664643", "--platform", "linux", "--path", dir], {
    GOGDL_CONFIG_PATH: path.join(dir, "cfg"),
    FAKE_GOGDL_WORKER_FAIL: "1",
  });
  assert.equal(out.status, 0);
  assert.ok(!existsSync(path.join(dir, "Ashen Tide", ".gogdl-linux-manifest")));
});

test("save-sync prints a timestamp", () => {
  const out = execFileSync(process.execPath, [tool, "--auth-config-path", "/x", "save-sync", "/s", "1", "--os", "linux", "--ts", "0", "--name", "saves", "--force-upload"], { encoding: "utf8" });
  assert.match(out.trim(), /^\d+\.\d+$/);
});

test("redist writes the listed installers and the repository, and drops what a later call leaves out", () => {
  const dir = mkdtempSync(path.join(tmpdir(), "fake-gogdl-"));
  const first = run(["--auth-config-path", "/x", "redist", "--ids", "ISI,MSVC2019_x64,openAL,DOSBox074", "--path", dir]);
  assert.equal(first.status, 0, first.stderr);
  assert.match(first.stderr, /\[PROGRESS\] INFO: = Progress: 100\.00 /);
  assert.ok(existsSync(path.join(dir, "__redist", "ISI", "scriptinterpreter.exe")));
  assert.ok(existsSync(path.join(dir, "__redist", "openAL", "oalinst.exe")));
  const manifest = JSON.parse(readFileSync(path.join(dir, ".gogdl-redist-manifest"), "utf8"));
  assert.deepEqual(manifest.HGLInstalled, ["ISI", "MSVC2019_x64", "openAL"]);
  assert.equal(manifest.depots.find((d) => d.dependencyId === "openAL").executable.arguments, "/S");
  const second = run(["--auth-config-path", "/x", "redist", "--ids", "ISI,openAL", "--path", dir]);
  assert.equal(second.status, 0, second.stderr);
  assert.ok(!existsSync(path.join(dir, "__redist", "MSVC2019_x64", "VC_redist.x64.exe")), "gogdl deletes what isn't listed");
  assert.ok(existsSync(path.join(dir, "__redist", "openAL", "oalinst.exe")));
});

test("a Windows build's manifest carries its script interpreter flag and dependencies", () => {
  const dir = mkdtempSync(path.join(tmpdir(), "fake-gogdl-"));
  const out = run(
    ["--auth-config-path", "/x", "update", "1453375253", "--platform", "windows", "--path", path.join(dir, "Lanternfall"), "--skip-dlcs"],
    { GOGDL_CONFIG_PATH: path.join(dir, "cfg") },
  );
  assert.equal(out.status, 0, out.stderr);
  const manifest = JSON.parse(readFileSync(path.join(dir, "cfg", "heroic_gogdl", "manifests", "1453375253"), "utf8"));
  assert.equal(manifest.scriptInterpreter, true);
  assert.deepEqual(manifest.dependencies, ["MSVC2019_x64", "openAL"]);
  assert.equal(manifest.products[0].productId, "1453375253");
});
