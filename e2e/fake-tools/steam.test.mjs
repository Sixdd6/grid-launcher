// node --test e2e/fake-tools/steam.test.mjs
import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { cpSync, existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { FAKE_KEY, findManifest, libraries, manifestText, parseLink, registryFile, setRunning } from "./steam.mjs";

const here = path.dirname(fileURLToPath(import.meta.url));
const tool = path.join(here, "steam.mjs");
const fixtures = path.join(here, "..", "fixtures-steam");

/** A fake Steam layout like the seed's: <dir>/steam/{root,library2,registry.vdf}. */
function fresh() {
  const dir = mkdtempSync(path.join(tmpdir(), "fake-steam-"));
  const steam = path.join(dir, "steam");
  mkdirSync(steam, { recursive: true });
  cpSync(path.join(fixtures, "root"), path.join(steam, "root"), { recursive: true });
  cpSync(path.join(fixtures, "library2"), path.join(steam, "library2"), { recursive: true });
  cpSync(path.join(fixtures, "registry.vdf"), path.join(steam, "registry.vdf"));
  // The seed's placeholders (seed/steam-seed.mjs).
  const fill = (file) =>
    writeFileSync(
      file,
      readFileSync(file, "utf8")
        .replaceAll("{ROOT}", path.join(steam, "root"))
        .replaceAll("{LIB2}", path.join(steam, "library2"))
        .replaceAll("{TODAY}", "1731000000")
        .replaceAll("{LAST_WEEK}", "1730000000"),
    );
  for (const lib of ["root", "library2"]) {
    const apps = path.join(steam, lib, "steamapps");
    for (const name of readdirSync(apps).filter((n) => /\.(vdf|acf)$/.test(n))) fill(path.join(apps, name));
  }
  fill(path.join(steam, "registry.vdf"));
  return { dir, root: path.join(steam, "root"), lib2: path.join(steam, "library2"), argv: path.join(dir, "argv.log") };
}

function run(env, args) {
  return spawnSync(process.execPath, [tool, ...args], {
    env: {
      ...process.env,
      GRID_LAUNCHER_E2E_STEAM_ROOT: env.root,
      E2E_STEAM_ARGV_FILE: env.argv,
      FAKE_STEAM_STEPS: "3",
      FAKE_STEAM_STEP_MS: "60",
      FAKE_STEAM_DIALOG_MS: "10",
      FAKE_STEAM_UNINSTALL_MS: "10",
      E2E_STEAM_RUN_SECS: "0.4",
    },
    input: "",
    encoding: "utf8",
  });
}

async function waitFor(test, what, ms = 5000) {
  const end = Date.now() + ms;
  while (Date.now() < end) {
    if (test()) return;
    await new Promise((r) => setTimeout(r, 20));
  }
  assert.fail(`timed out waiting for ${what}`);
}

const field = (text, name) => new RegExp(`"${name}"\\s+"([^"]*)"`).exec(text)?.[1];

test("links parse, with -silent only before rungameid", () => {
  assert.deepEqual(parseLink(["-silent", "steam://rungameid/620"]), { action: "rungameid", appid: 620, silent: true });
  assert.deepEqual(parseLink(["steam://install/3000040"]), { action: "install", appid: 3000040, silent: false });
  assert.deepEqual(parseLink(["steam://nonsense"]), { action: "", appid: 0, silent: false });
});

test("every run records its argv and prints the key, which GRID never reads", () => {
  const env = fresh();
  const out = run(env, ["steam://validate/3000010"]);
  assert.equal(out.status, 0);
  assert.match(out.stdout, new RegExp(FAKE_KEY));
  assert.match(out.stderr, new RegExp(FAKE_KEY));
  run(env, ["-silent", "steam://rungameid/3000010"]);
  const lines = readFileSync(env.argv, "utf8").trim().split("\n").map((l) => JSON.parse(l));
  assert.deepEqual(lines, [["steam://validate/3000010"], ["-silent", "steam://rungameid/3000010"]]);
});

test("the root lists both libraries; manifests are found in either", () => {
  const env = fresh();
  assert.deepEqual(libraries(env.root), [env.root, env.lib2]);
  assert.equal(findManifest(env.root, 3000010), path.join(env.root, "steamapps", "appmanifest_3000010.acf"));
  assert.equal(findManifest(env.root, 3000030), path.join(env.lib2, "steamapps", "appmanifest_3000030.acf"));
  assert.equal(findManifest(env.root, 1), null);
});

test("install writes a growing 1026 manifest, then 4 and the game folder", async () => {
  const env = fresh();
  assert.equal(run(env, ["steam://install/3000040"]).status, 0);
  const file = path.join(env.root, "steamapps", "appmanifest_3000040.acf");
  const seen = new Set();
  await waitFor(() => {
    if (!existsSync(file)) return false;
    const text = readFileSync(file, "utf8");
    seen.add(`${field(text, "StateFlags")}:${field(text, "BytesDownloaded")}`);
    return field(text, "StateFlags") === "4";
  }, "the install to finish");
  const text = readFileSync(file, "utf8");
  assert.equal(field(text, "name"), "Quiet Comet");
  assert.equal(field(text, "SizeOnDisk"), "6000000000");
  assert.ok([...seen].some((s) => s.startsWith("1026:")), [...seen].join(" "));
  assert.ok(existsSync(path.join(env.root, "steamapps", "common", "Quiet Comet")));
});

test(".fake-install-cancel writes no manifest; .fake-install-abort removes it halfway", async () => {
  const env = fresh();
  writeFileSync(path.join(env.root, ".fake-install-cancel"), "");
  run(env, ["steam://install/3000050"]);
  await new Promise((r) => setTimeout(r, 400));
  assert.equal(existsSync(path.join(env.root, "steamapps", "appmanifest_3000050.acf")), false);

  const other = fresh();
  writeFileSync(path.join(other.root, ".fake-install-abort"), "");
  run(other, ["steam://install/3000050"]);
  const file = path.join(other.root, "steamapps", "appmanifest_3000050.acf");
  await waitFor(() => existsSync(file), "the manifest to appear");
  await waitFor(() => !existsSync(file), "the manifest to go");
});

test("uninstall deletes the manifest and the game folder in its library", async () => {
  const env = fresh();
  mkdirSync(path.join(env.lib2, "steamapps", "common", "Old Brick Road"), { recursive: true });
  run(env, ["steam://uninstall/3000030"]);
  await waitFor(() => !existsSync(path.join(env.lib2, "steamapps", "appmanifest_3000030.acf")), "the manifest to go");
  assert.equal(existsSync(path.join(env.lib2, "steamapps", "common", "Old Brick Road")), false);
  assert.ok(existsSync(path.join(env.root, "steamapps", "appmanifest_3000010.acf")), "the others stay");
});

test("rungameid sets RunningAppID and Running, then clears them; LastPlayed moves", async () => {
  const env = fresh();
  const reg = registryFile(env.root);
  run(env, ["-silent", "steam://rungameid/3000010"]);
  await waitFor(() => field(readFileSync(reg, "utf8"), "RunningAppID") === "3000010", "the game to run");
  assert.match(readFileSync(reg, "utf8"), /"3000010"\s*\{\s*"Installed"\s+"1"\s*"Running"\s+"1"/);
  const played = Number(field(readFileSync(findManifest(env.root, 3000010), "utf8"), "LastPlayed"));
  assert.ok(played > Date.now() / 1000 - 60);
  await waitFor(() => field(readFileSync(reg, "utf8"), "RunningAppID") === "0", "the game to end");
  assert.match(readFileSync(reg, "utf8"), /"AutoLoginUser"\s+"FAKE-ACCOUNT-NAME-not-shown"/, "the rest is untouched");

  const quiet = fresh();
  writeFileSync(path.join(quiet.root, ".fake-no-run"), "");
  run(quiet, ["-silent", "steam://rungameid/3000010"]);
  await new Promise((r) => setTimeout(r, 900));
  assert.equal(field(readFileSync(registryFile(quiet.root), "utf8"), "RunningAppID"), "0");
});

test("setRunning and manifestText write Steam's KeyValues", () => {
  const text = '"Steam"\n{\n\t"RunningAppID"\t\t"0"\n\t"apps"\n\t{\n\t\t"5"\n\t\t{\n\t\t\t"Running"\t\t"0"\n\t\t}\n\t}\n}\n';
  const on = setRunning(text, 5, true);
  assert.match(on, /"RunningAppID"\t\t"5"/);
  assert.match(on, /"Running"\t\t"1"/);
  assert.equal(setRunning(on, 5, false), text);
  const m = manifestText({ appid: 7, name: 'A "quoted" name', installdir: "A", flags: 1026, toDownload: 10, downloaded: 4 });
  assert.match(m, /^"AppState"\n\{\n/);
  assert.match(m, /"name"\t\t"A \\"quoted\\" name"/);
  assert.match(m, /"StateFlags"\t\t"1026"/);
  assert.match(m, /"BytesDownloaded"\t\t"4"/);
});
