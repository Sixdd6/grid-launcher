// Mock forge server for the Rust rewrite's E2E harness.
//
// Plain node:http, ESM, no npm dependencies. Stands in for GitHub and
// redream.io while the `emulator-catalog` stage group installs emulators
// from the embedded autoprofile catalog.
//
// Routing mirrors grid-core's E2E redirect (launch/forge.rs `effective_url`,
// enabled by the `e2e` cargo feature): with
// GRID_LAUNCHER_E2E_FORGE_BASE=<this server's url>, every forge request the
// app makes for `https://<host>/<path>` goes to `<base>/<host>/<path>`. The
// app keeps using the real, absolute URLs everywhere else, so the catalog's
// `download_url_regex` patterns still match what this server's HTML page
// links to.
//
// Routes (all GET):
//   /api.github.com/repos/PCSX2/pcsx2/releases/latest   → release JSON
//   /api.github.com/repos/Sixdd6/grid-launcher/releases/latest → release JSON
//   /github.com/PCSX2/pcsx2/releases/download/…AppImage → the AppImage stub
//   /redream.io/download                                → the download page
//   /redream.io/download/redream.x86_64-linux-…tar.gz   → the tar.gz stub
//
// Two rules this server exists to enforce at runtime:
//   * NO credential ever reaches a forge. Any request carrying an
//     `Authorization` header is answered 500 and logged as
//     `AUTH-HEADER-SEEN`, which emulator-catalog.spec.ts greps for.
//   * Everything else is a 404, so a request to an unexpected forge URL
//     fails the install loudly instead of silently succeeding.
//
// Usage as a library:
//   import { startMockForge } from "./mock-forge.mjs";
//   const handle = await startMockForge({ port: 0, logPath });
//   // handle.port, handle.url, handle.requestLog, await handle.close()
//
// Standalone (e2e.sh scrapes the MOCK_FORGE_URL= line off stdout):
//   node mock-forge.mjs --port 0 [--log <path>]

import http from "node:http";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { appendFileSync } from "node:fs";

import { buildTarGz } from "./archives.mjs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

/** Default request log. Appended to live, so a running spec can read it. */
export const DEFAULT_LOG_PATH = path.join(__dirname, "..", "last-run-forge-requests.log");

/** Logged instead of a request line when a request carries a credential. */
export const AUTH_MARKER = "AUTH-HEADER-SEEN";

// --- the served fixtures ------------------------------------------------------

export const PCSX2_TAG = "v9.9-e2e";
export const PCSX2_ASSET_NAME = `pcsx2-${PCSX2_TAG}-linux-appimage-x64-Qt.AppImage`;
export const PCSX2_RELEASE_PATH = "/api.github.com/repos/PCSX2/pcsx2/releases/latest";
export const PCSX2_DOWNLOAD_URL = `https://github.com/PCSX2/pcsx2/releases/download/${PCSX2_TAG}/${PCSX2_ASSET_NAME}`;
export const PCSX2_DOWNLOAD_PATH = `/github.com/PCSX2/pcsx2/releases/download/${PCSX2_TAG}/${PCSX2_ASSET_NAME}`;

/**
 * GRID Launcher's OWN `releases/latest`, for the `updates` group's
 * self-update banner (app/src-tauri/src/app_update.rs). The tag is a
 * deliberately absurd semver so it is newer than every real version the app
 * can report, including the `0.9.0-dev` a source build carries; the check
 * only ever runs in the `e2e` build with GRID_LAUNCHER_E2E_UPDATE_CHECK=1.
 * `assets` is empty on purpose — the check reads `tag_name`/`html_url` and
 * downloads nothing.
 */
export const GRID_LAUNCHER_TAG = "v9.9.9-e2e";
export const GRID_LAUNCHER_RELEASE_PATH =
  "/api.github.com/repos/Sixdd6/grid-launcher/releases/latest";
export const GRID_LAUNCHER_RELEASE_URL = `https://github.com/Sixdd6/grid-launcher/releases/tag/${GRID_LAUNCHER_TAG}`;

export const REDREAM_PAGE_PATH = "/redream.io/download";
export const REDREAM_ASSET_NAME = "redream.x86_64-linux-v1.5.0-1000-gabcdef0.tar.gz";
export const REDREAM_DOWNLOAD_URL = `https://redream.io/download/${REDREAM_ASSET_NAME}`;
export const REDREAM_DOWNLOAD_PATH = `/redream.io/download/${REDREAM_ASSET_NAME}`;

/**
 * The `redream` tar.gz member name: the bare `redream` the real tarball
 * ships, with no suffix. Selecting it exercises the executable-bit rule in
 * `launchable_installed_file` (grid-core/src/launch/emu_install.rs) — an
 * extracted file with no `.` in its name and its executable bit set is
 * launchable on unix — which is why the member below is written 0755.
 */
export const REDREAM_MEMBER_NAME = "redream";

/**
 * Env var the stub emulators append their argv to, one argument per line.
 * The same contract as the `launch` group's seeded stubs
 * (e2e/seed/launch-seed.mjs), except the path comes from the environment:
 * this stub's bytes are baked into the mock, so it cannot know the stage's
 * temp directory the way a seed script does. e2e.sh exports it, wdio.conf.ts
 * forwards it into the app process, and the app's spawned emulator inherits
 * it.
 */
export const ARGV_FILE_ENV = "GRID_E2E_ARGV_FILE";

/**
 * Bytes served for BOTH stub emulators: a shell script that records its
 * argv, then sleeps long enough for the spec to see a live session and stop
 * it (30s, mirroring launch-seed.mjs's long-runner). The marker line makes
 * the installed file's provenance checkable from a spec.
 */
function stubEmulatorScript(label) {
  return Buffer.from(
    "#!/bin/sh\n" +
      `# grid-launcher e2e mock forge stub: ${label}\n` +
      `if [ -n "\${${ARGV_FILE_ENV}:-}" ]; then\n` +
      `  printf '%s\\n' "$@" >> "\$${ARGV_FILE_ENV}"\n` +
      "fi\n" +
      "sleep 30\n",
    "utf8",
  );
}

export const PCSX2_APPIMAGE_BYTES = stubEmulatorScript("pcsx2");
export const REDREAM_MEMBER_BYTES = stubEmulatorScript("redream");
export const REDREAM_TAR_GZ_BYTES = buildTarGz([
  { name: REDREAM_MEMBER_NAME, data: REDREAM_MEMBER_BYTES, mode: 0o755 },
]);

/** The GitHub "latest release" payload for PCSX2. */
export function pcsx2Release() {
  return {
    tag_name: PCSX2_TAG,
    name: `PCSX2 ${PCSX2_TAG}`,
    draft: false,
    prerelease: false,
    assets: [
      {
        name: PCSX2_ASSET_NAME,
        browser_download_url: PCSX2_DOWNLOAD_URL,
        size: PCSX2_APPIMAGE_BYTES.length,
        state: "uploaded",
      },
    ],
  };
}

/**
 * The GitHub "latest release" payload for GRID Launcher itself. The URL
 * must stay under `RELEASE_URL_PREFIX` (commands/updates.rs), or the
 * banner's "Open release" button would refuse to open it.
 */
export function gridLauncherRelease() {
  return {
    tag_name: GRID_LAUNCHER_TAG,
    html_url: GRID_LAUNCHER_RELEASE_URL,
    assets: [],
  };
}

/**
 * The redream download page. The matching href is absolute, exactly as the
 * catalog's linux `download_url_regex` expects; the decoys are there so the
 * scrape has to actually apply that regex rather than take the first link.
 */
export function redreamPage() {
  return [
    "<!doctype html>",
    "<html><head><title>redream — download</title></head>",
    "<body>",
    '  <a href="/">Home</a>',
    '  <a href="https://redream.io/download/redream.x86_64-windows-v1.5.0-1000-gabcdef0.zip">Windows</a>',
    '  <a href="https://redream.io/download/redream.universal-raspbian-v1.5.0-1000-gabcdef0.tar.gz">Raspberry Pi</a>',
    `  <a href="${REDREAM_DOWNLOAD_URL}">Linux x86_64</a>`,
    '  <a href="https://redream.io/changelog">Changelog</a>',
    "</body></html>",
    "",
  ].join("\n");
}

// --- HTTP --------------------------------------------------------------------

function sendJson(res, status, body) {
  const payload = Buffer.from(JSON.stringify(body), "utf8");
  res.writeHead(status, {
    "Content-Type": "application/json",
    "Content-Length": payload.length,
  });
  res.end(payload);
}

function sendBuffer(res, status, contentType, buf) {
  res.writeHead(status, {
    "Content-Type": contentType,
    "Content-Length": buf.length,
  });
  res.end(buf);
}

function handleRequest(req, res, state) {
  const requestUrl = new URL(req.url, "http://127.0.0.1");
  const pathname = decodeURIComponent(requestUrl.pathname);

  // The credential check comes first, and on its own line: a forge request
  // carrying an Authorization header is a spec violation whatever it asks
  // for, and it must be visible in the log even for an unknown path.
  if (req.headers["authorization"] !== undefined) {
    state.log({ method: req.method, path: req.url, note: AUTH_MARKER });
    sendJson(res, 500, { detail: AUTH_MARKER });
    return;
  }

  state.log({ method: req.method, path: req.url });

  if (req.method === "GET" && pathname === PCSX2_RELEASE_PATH) {
    sendJson(res, 200, pcsx2Release());
    return;
  }

  if (req.method === "GET" && pathname === PCSX2_DOWNLOAD_PATH) {
    sendBuffer(res, 200, "application/octet-stream", PCSX2_APPIMAGE_BYTES);
    return;
  }

  if (req.method === "GET" && pathname === REDREAM_PAGE_PATH) {
    sendBuffer(res, 200, "text/html; charset=utf-8", Buffer.from(redreamPage(), "utf8"));
    return;
  }

  if (req.method === "GET" && pathname === REDREAM_DOWNLOAD_PATH) {
    sendBuffer(res, 200, "application/gzip", REDREAM_TAR_GZ_BYTES);
    return;
  }

  if (req.method === "GET" && pathname === GRID_LAUNCHER_RELEASE_PATH) {
    sendJson(res, 200, gridLauncherRelease());
    return;
  }

  sendJson(res, 404, { detail: "not found" });
}

/**
 * Starts the mock forge server.
 *
 * @param {{port?: number, logPath?: string}} [options] `logPath` is appended
 *   to as requests arrive (so a running spec can read it); pass `null` to
 *   keep the log in memory only.
 * @returns {Promise<{port: number, url: string, logPath: string|null, requestLog: Array<object>, close: () => Promise<void>}>}
 */
export async function startMockForge({ port = 0, logPath = DEFAULT_LOG_PATH } = {}) {
  const requestLog = [];
  const state = {
    log(entry) {
      requestLog.push(entry);
      if (!logPath) return;
      try {
        appendFileSync(logPath, `${JSON.stringify(entry)}\n`, "utf8");
      } catch {
        // The request log is a debugging/assertion aid, not a hard
        // requirement: an unwritable path must not fail a served request.
      }
    },
  };

  const server = http.createServer((req, res) => handleRequest(req, res, state));

  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(port, "127.0.0.1", () => {
      server.removeListener("error", reject);
      resolve();
    });
  });

  const actualPort = server.address().port;

  async function close() {
    await new Promise((resolve, reject) => {
      server.close((err) => (err ? reject(err) : resolve()));
    });
  }

  return {
    port: actualPort,
    url: `http://127.0.0.1:${actualPort}`,
    logPath,
    requestLog,
    close,
  };
}

// --- standalone entry point ---------------------------------------------

function isMainModule() {
  return process.argv[1] && import.meta.url === `file://${process.argv[1]}`;
}

if (isMainModule()) {
  const args = process.argv.slice(2);
  let port = 0;
  let logPath = DEFAULT_LOG_PATH;
  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--port" && args[i + 1]) {
      port = Number(args[i + 1]);
      i++;
    } else if (args[i] === "--log" && args[i + 1]) {
      logPath = path.resolve(args[i + 1]);
      i++;
    }
  }

  const handle = await startMockForge({ port, logPath });
  // e2e.sh scrapes this exact line to learn the URL.
  console.log(`MOCK_FORGE_URL=${handle.url}`);

  const shutdown = async () => {
    await handle.close();
    process.exit(0);
  };
  process.on("SIGINT", shutdown);
  process.on("SIGTERM", shutdown);
}
