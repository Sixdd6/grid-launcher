// Mock RomM server for the Rust rewrite's E2E harness.
//
// Plain node:http, ESM, no npm dependencies. Mirrors the endpoints the Rust
// client calls (see grid-core/src/romm/mod.rs, covers.rs, library/mod.rs):
//   GET  /api/users/me
//   GET  /api/platforms
//   GET  /api/roms?platform_ids=&limit=&offset=&with_char_index=&with_filter_values=
//   GET  /api/roms/:id
//   GET  /api/roms/:id/content/:file_name?file_ids=[&e2e_throttle=<ms-per-chunk>]
//   GET  /assets/romm/resources/roms/:id/cover/(small|large).png
//   GET  /assets/romm/resources/roms/:id/screenshots/:n.png
//
// `GET/POST /__e2e__/offline` (outside `/api/`, no auth, not logged), used
// by the `images` stage group: POST { offline: true|false } sets whether
// every /api/ or /assets/ request gets its socket destroyed rather than
// answered (an unreachable server, from the client's point of view); GET
// reads the current value.
//
// Cloud save/state sync (grid-core/src/romm/cloud.rs), used by the
// `cloud-saves` stage group:
//   GET  /api/saves?rom_id=            (canned records, from saves.json)
//   GET  /api/saves/:id/content        (raw bytes for one canned record)
//   POST /api/saves                    (multipart upload — captured)
//   POST /api/saves/delete             ({"saves": [id, ...]})
//   GET  /api/states?rom_id=           (always [] — no group needs seeded
//                                        state records; states are queried
//                                        as a side effect of the save-type
//                                        auto-restore/auto-upload flows)
//
// `GET /__e2e__/requests` (outside `/api/`, no auth) returns the live
// request log as JSON so a spec can assert on what the mock received
// (query params, parsed multipart parts, JSON bodies) WHILE the mock is
// still running — the mock is a separate process from the wdio spec
// (scripts/e2e.sh's run_group_attempt), so the in-memory `requestLog`
// array plumbed through `startMockRomm`'s return value is only reachable
// by mock-romm/server.test.mjs's in-process tests, not by an E2E spec.
//
// The content endpoint's `e2e_throttle` query param (or the `defaultThrottleMs`
// / `--throttle-ms` server-wide default, used by e2e.sh for the `downloads`
// stage group) makes it stream the response in fixed-size chunks with a delay
// between each, instead of writing the whole buffer at once — this is what
// gives the downloads E2E spec a real, cancellable in-flight download.
//
// Usage as a library:
//   import { startMockRomm } from "./server.mjs";
//   const handle = await startMockRomm({ port: 0, fixturesDir, defaultThrottleMs: 0 });
//   // handle.port, handle.url, handle.requestLog, await handle.close()
//
// Standalone:
//   node server.mjs --port 8931 [--throttle-ms 100] [--fixtures-dir ../fixtures-x]
//
// `--fixtures-dir` exposes the library API's existing `fixturesDir` option on
// the command line (relative paths resolve against the process's working
// directory, which e2e.sh sets to rewrite/e2e). The `emulator-catalog` stage
// group uses it for a fixture set with a "Sony PlayStation 2" platform, so
// the shared fixtures — and every assertion the other groups make about
// them — stay exactly as they are.

import http from "node:http";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { readFile, writeFile } from "node:fs/promises";

import { buildZip } from "./archives.mjs";

export const FAKE_TOKEN = "FAKE-E2E-TOKEN-not-real";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const DEFAULT_FIXTURES_DIR = path.join(__dirname, "../fixtures");

// --- deterministic dummy content --------------------------------------------

function dummyBytes(length, seed) {
  const buf = Buffer.alloc(length);
  for (let i = 0; i < length; i++) {
    buf[i] = (seed + i * 7) & 0xff;
  }
  return buf;
}

// 1x1 transparent PNG.
const PNG_1X1_BASE64 =
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";

/**
 * Content size of the "Big Arcade Game" fixture (rom 301): large enough that
 * throttled streaming (see THROTTLE_CHUNK_BYTES below) takes a comfortable,
 * multi-chunk amount of wall-clock time — long enough for the downloads E2E
 * spec to observe an in-flight download and cancel it before it completes.
 */
const BIG_CONTENT_BYTES = 300 * 1024;

function buildContentFixtures() {
  const zipBytes = buildZip([{ name: "game.sfc", data: dummyBytes(320, 0xa5) }]);
  const bigZipBytes = buildZip([
    { name: "big.bin", data: dummyBytes(BIG_CONTENT_BYTES, 0x7e) },
  ]);
  const m3uBytes = Buffer.from("disc1.bin\ndisc2.bin\n", "utf8");
  const disc1Bytes = dummyBytes(256, 0x11);
  const disc2Bytes = dummyBytes(272, 0x22);
  const pngBytes = Buffer.from(PNG_1X1_BASE64, "base64");
  return { zipBytes, bigZipBytes, m3uBytes, disc1Bytes, disc2Bytes, pngBytes };
}

/** Picks the content buffer for one RomFile entry, by name. */
function contentForFile(fileName, content) {
  const lower = fileName.toLowerCase();
  if (fileName === "disc1.bin") return content.disc1Bytes;
  if (fileName === "disc2.bin") return content.disc2Bytes;
  if (lower.endsWith(".m3u")) return content.m3uBytes;
  if (lower === "big arcade game.zip") return content.bigZipBytes;
  if (lower.endsWith(".zip")) return content.zipBytes;
  return dummyBytes(64, 0x00);
}

// --- fixture loading ---------------------------------------------------------

/**
 * `saves.json` is optional — only `fixtures-cloud-saves/` has one. Each key
 * is a rom id (string); each value is an array of canned save records with
 * an extra `content` field (plain utf8 text) holding the bytes
 * `GET /api/saves/:id/content` serves for that record. `content` is
 * stripped from the record before it is ever sent to a client — the real
 * `GET /api/saves?rom_id=` response never inlines file bytes.
 */
async function loadSaveFixtures(fixturesDir) {
  try {
    const raw = await readFile(path.join(fixturesDir, "saves.json"), "utf8");
    return JSON.parse(raw);
  } catch {
    return {};
  }
}

async function loadFixtures(fixturesDir) {
  const [platforms, romsByPlatform, romDetails, savesByRom] = await Promise.all([
    readFile(path.join(fixturesDir, "platforms.json"), "utf8").then(JSON.parse),
    readFile(path.join(fixturesDir, "roms.json"), "utf8").then(JSON.parse),
    readFile(path.join(fixturesDir, "rom-details.json"), "utf8").then(JSON.parse),
    loadSaveFixtures(fixturesDir),
  ]);

  const saveContentById = new Map();
  const saveRecordsByRom = {};
  for (const [romId, records] of Object.entries(savesByRom)) {
    saveRecordsByRom[romId] = records.map((record) => {
      const { content, ...rest } = record;
      if (content !== undefined) {
        saveContentById.set(String(record.id), Buffer.from(content, "utf8"));
      }
      return rest;
    });
  }

  const content = buildContentFixtures();

  // Patch each file's declared size (and the rom's total) to match the bytes
  // actually served, so a client that checks Content-Length or an
  // already-downloaded file's size against the fixture never sees a mismatch.
  const contentByKey = new Map();
  for (const detail of Object.values(romDetails)) {
    let total = 0;
    for (const file of detail.files ?? []) {
      const bytes = contentForFile(file.file_name, content);
      file.file_size_bytes = bytes.length;
      total += bytes.length;
      contentByKey.set(`${detail.id}:${file.file_name}`, bytes);
    }
    detail.fs_size_bytes = total;
  }

  return {
    platforms,
    romsByPlatform,
    romDetails,
    contentByKey,
    pngBytes: content.pngBytes,
    saveRecordsByRom,
    saveContentById,
  };
}

// --- HTTP helpers ------------------------------------------------------------

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

/** Chunk size for throttled content streaming (see sendBufferThrottled). */
const THROTTLE_CHUNK_BYTES = 20 * 1024;

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/**
 * Like sendBuffer, but writes `buf` in THROTTLE_CHUNK_BYTES-sized chunks,
 * awaiting `chunkMs` after every chunk (including the last, so even content
 * smaller than one chunk still measurably delays — this is what the mock
 * server's own unit test observes). Bails out cleanly if the client closes
 * the connection mid-stream (e.g. the app cancelling an in-flight download),
 * rather than throwing on a write to a destroyed socket.
 */
async function sendBufferThrottled(res, status, contentType, buf, chunkMs) {
  res.writeHead(status, {
    "Content-Type": contentType,
    "Content-Length": buf.length,
  });
  let aborted = false;
  res.once("close", () => {
    aborted = true;
  });
  for (let offset = 0; offset < buf.length; offset += THROTTLE_CHUNK_BYTES) {
    if (aborted || res.destroyed) return;
    const chunk = buf.subarray(offset, offset + THROTTLE_CHUNK_BYTES);
    const ok = res.write(chunk);
    if (!ok && !aborted && !res.destroyed) {
      await new Promise((resolve) => res.once("drain", resolve));
    }
    if (aborted || res.destroyed) return;
    await sleep(chunkMs);
  }
  if (!aborted && !res.destroyed) res.end();
}

const COVER_PATH_RE = /^\/assets\/romm\/resources\/roms\/\d+\/cover\/(small|large)\.png$/;
const SCREENSHOT_PATH_RE = /^\/assets\/romm\/resources\/roms\/\d+\/screenshots\/\d+\.png$/;
const ROM_ID_RE = /^\/api\/roms\/(\d+)$/;
const ROM_CONTENT_RE = /^\/api\/roms\/(\d+)\/content\/(.+)$/;
const SAVE_CONTENT_RE = /^\/api\/saves\/([^/]+)\/content$/;

/** Buffers a request body fully. Small fixture payloads only — no streaming. */
function readBody(req) {
  return new Promise((resolve, reject) => {
    const chunks = [];
    req.on("data", (chunk) => chunks.push(chunk));
    req.on("end", () => resolve(Buffer.concat(chunks)));
    req.on("error", reject);
  });
}

/**
 * Minimal `multipart/form-data` parser for the cloud-saves E2E group's
 * `POST /api/saves` capture — plain node:http, no npm dependency. Handles
 * exactly the shape `reqwest::multipart::Form` produces
 * (grid-core/src/romm/cloud.rs's `build_multipart_form`): one or more parts,
 * each `Content-Disposition: form-data; name="..."; filename="..."` plus a
 * `Content-Type:` line, separated by `--<boundary>` markers and terminated
 * by `--<boundary>--`. Returns `[]` when `contentType` carries no boundary
 * (not multipart, or an empty body).
 */
function parseMultipart(buffer, contentType) {
  const boundaryMatch = /boundary=(?:"([^"]+)"|([^;]+))/i.exec(contentType ?? "");
  if (!boundaryMatch) return [];
  const boundary = boundaryMatch[1] ?? boundaryMatch[2];
  const marker = Buffer.from(`--${boundary}`);

  const parts = [];
  let cursor = buffer.indexOf(marker);
  while (cursor !== -1) {
    const next = buffer.indexOf(marker, cursor + marker.length);
    if (next === -1) break;

    let segment = buffer.subarray(cursor + marker.length, next);
    if (segment.subarray(0, 2).toString("latin1") === "\r\n") segment = segment.subarray(2);
    if (segment.subarray(-2).toString("latin1") === "\r\n") segment = segment.subarray(0, -2);

    const headerEnd = segment.indexOf("\r\n\r\n");
    if (headerEnd !== -1) {
      const headerText = segment.subarray(0, headerEnd).toString("utf8");
      const body = segment.subarray(headerEnd + 4);
      const nameMatch = /name="([^"]*)"/i.exec(headerText);
      const filenameMatch = /filename="([^"]*)"/i.exec(headerText);
      const contentTypeMatch = /Content-Type:\s*([^\r\n]+)/i.exec(headerText);
      parts.push({
        name: nameMatch ? nameMatch[1] : "",
        filename: filenameMatch ? filenameMatch[1] : undefined,
        contentType: contentTypeMatch ? contentTypeMatch[1].trim() : undefined,
        size: body.length,
        // Fixture payloads in this group are small text files — decoding
        // as utf8 unconditionally is fine for what these specs assert on.
        text: body.toString("utf8"),
      });
    }
    cursor = next;
  }
  return parts;
}

async function handleRequest(req, res, state) {
  const requestUrl = new URL(req.url, "http://127.0.0.1");
  const pathname = decodeURIComponent(requestUrl.pathname);

  // Live introspection (cloud-saves) and the offline toggle (images): both
  // live outside /api/, no auth, and are NOT recorded in requestLog — the
  // mock runs as a separate process from the wdio spec (scripts/e2e.sh's
  // run_group_attempt), so these are the only way a spec can see or steer
  // the mock WHILE it is still running, rather than only after close()
  // writes last-run-requests.log. These always work, even while "offline".
  if (pathname.startsWith("/__e2e__/")) {
    if (req.method === "GET" && pathname === "/__e2e__/requests") {
      sendJson(res, 200, state.requestLog);
      return;
    }
    if (req.method === "GET" && pathname === "/__e2e__/offline") {
      sendJson(res, 200, { offline: state.offline });
      return;
    }
    if (req.method === "POST" && pathname === "/__e2e__/offline") {
      const body = await readBody(req);
      let parsed = {};
      try {
        parsed = JSON.parse(body.toString("utf8"));
      } catch {
        parsed = {};
      }
      state.offline = Boolean(parsed.offline);
      sendJson(res, 200, { offline: state.offline });
      return;
    }
    sendJson(res, 404, { detail: "not found" });
    return;
  }

  // "Offline" mode (images E2E group): every /api/ or /assets/ request is
  // answered by destroying the socket, not by an error status — this is
  // what an unreachable server looks like to reqwest (a connection error),
  // which is what the app's startup routing and Retry flow actually branch
  // on. Not logged to requestLog: nothing was really served.
  if (
    state.offline &&
    (pathname.startsWith("/api/") || pathname.startsWith("/assets/"))
  ) {
    req.socket.destroy();
    return;
  }

  const logEntry = { method: req.method, path: req.url };
  state.requestLog.push(logEntry);
  let body = Buffer.alloc(0);
  if (req.method === "POST") {
    body = await readBody(req);
  }

  // Static-style cover/screenshot asset: not under /api, no auth required —
  // mirrors RomM serving these images directly off disk.
  if (req.method === "GET" && (COVER_PATH_RE.test(pathname) || SCREENSHOT_PATH_RE.test(pathname))) {
    sendBuffer(res, 200, "image/png", state.pngBytes);
    return;
  }

  if (!pathname.startsWith("/api/")) {
    sendJson(res, 404, { detail: "not found" });
    return;
  }

  const authHeader = req.headers["authorization"];
  if (authHeader !== `Bearer ${FAKE_TOKEN}`) {
    sendJson(res, 401, { detail: "Unauthorized" });
    return;
  }

  if (req.method === "GET" && pathname === "/api/users/me") {
    sendJson(res, 200, { id: 1, username: "e2euser" });
    return;
  }

  if (req.method === "GET" && pathname === "/api/platforms") {
    sendJson(res, 200, state.platforms);
    return;
  }

  if (req.method === "GET" && pathname === "/api/roms") {
    const platformIds = requestUrl.searchParams.getAll("platform_ids");
    const limit = Number(requestUrl.searchParams.get("limit") ?? "200");
    const offset = Number(requestUrl.searchParams.get("offset") ?? "0");

    let pool;
    if (platformIds.length > 0) {
      pool = platformIds.flatMap((id) => state.romsByPlatform[id] ?? []);
    } else {
      pool = Object.values(state.romsByPlatform).flat();
    }

    const items = pool.slice(offset, offset + limit);
    sendJson(res, 200, { items });
    return;
  }

  const romIdMatch = pathname.match(ROM_ID_RE);
  if (req.method === "GET" && romIdMatch) {
    const detail = state.romDetails[romIdMatch[1]];
    if (!detail) {
      sendJson(res, 404, { detail: "rom not found" });
      return;
    }
    sendJson(res, 200, detail);
    return;
  }

  const contentMatch = pathname.match(ROM_CONTENT_RE);
  if (req.method === "GET" && contentMatch) {
    const [, romId, fileName] = contentMatch;
    const bytes = state.contentByKey.get(`${romId}:${fileName}`);
    if (!bytes) {
      sendJson(res, 404, { detail: "file not found" });
      return;
    }
    const contentType = fileName.toLowerCase().endsWith(".zip")
      ? "application/zip"
      : "application/octet-stream";
    // Number(null) and Number("0") are both 0, so reading the param straight
    // into Number() cannot tell "absent" from "explicit ?e2e_throttle=0" —
    // the explicit-zero override then silently lost to defaultThrottleMs.
    // Keep "absent" as null through the parse so it only falls back to the
    // server-wide default when the caller genuinely supplied nothing.
    const rawThrottle = requestUrl.searchParams.get("e2e_throttle");
    let requestedThrottle = rawThrottle === null ? null : Number(rawThrottle);
    if (requestedThrottle !== null && Number.isNaN(requestedThrottle)) {
      requestedThrottle = null;
    }
    const throttleMs =
      requestedThrottle !== null
        ? Math.max(0, requestedThrottle)
        : state.defaultThrottleMs > 0
          ? state.defaultThrottleMs
          : 0;
    if (throttleMs > 0) {
      sendBufferThrottled(res, 200, contentType, bytes, throttleMs).catch(() => {
        // The client (or a test) closing the connection mid-download is an
        // expected way for this to end, not a server bug — nothing to log.
      });
    } else {
      sendBuffer(res, 200, contentType, bytes);
    }
    return;
  }

  // --- cloud saves (grid-core/src/romm/cloud.rs) --------------------------

  if (req.method === "GET" && pathname === "/api/saves") {
    const romId = requestUrl.searchParams.get("rom_id") ?? "";
    logEntry.query = { rom_id: romId };
    sendJson(res, 200, state.saveRecordsByRom[romId] ?? []);
    return;
  }

  const saveContentMatch = pathname.match(SAVE_CONTENT_RE);
  if (req.method === "GET" && saveContentMatch) {
    const bytes = state.saveContentById.get(saveContentMatch[1]);
    if (!bytes) {
      sendJson(res, 404, { detail: "save content not found" });
      return;
    }
    sendBuffer(res, 200, "application/octet-stream", bytes);
    return;
  }

  if (req.method === "POST" && pathname === "/api/saves") {
    logEntry.query = Object.fromEntries(requestUrl.searchParams.entries());
    logEntry.multipart = parseMultipart(body, req.headers["content-type"]);
    sendJson(res, 200, {});
    return;
  }

  if (req.method === "POST" && pathname === "/api/saves/delete") {
    try {
      logEntry.bodyJson = JSON.parse(body.toString("utf8"));
    } catch {
      logEntry.bodyJson = null;
    }
    sendJson(res, 200, {});
    return;
  }

  // States: no stage group seeds records, but the save-type auto-restore
  // and auto-upload flows always probe the state side too (grid-core
  // resolves save/state independently) — an empty list keeps that probe
  // harmless rather than a stray 404 in the mock log.
  if (req.method === "GET" && pathname === "/api/states") {
    sendJson(res, 200, []);
    return;
  }

  sendJson(res, 404, { detail: "not found" });
}

// --- public API --------------------------------------------------------------

/**
 * Starts the mock RomM server.
 *
 * @param {{port?: number, fixturesDir?: string, defaultThrottleMs?: number}} [options]
 * @returns {Promise<{port: number, url: string, close: () => Promise<void>, requestLog: Array<{method: string, path: string}>}>}
 */
export async function startMockRomm({
  port = 0,
  fixturesDir = DEFAULT_FIXTURES_DIR,
  defaultThrottleMs = 0,
} = {}) {
  const state = await loadFixtures(fixturesDir);
  state.requestLog = [];
  // Server-wide fallback used when a content request carries no explicit
  // ?e2e_throttle=: set by e2e.sh for the `downloads` stage group only, so
  // every content download made through this instance is throttled without
  // the live app needing to add the query param itself.
  state.defaultThrottleMs = defaultThrottleMs;
  // Toggled by POST /__e2e__/offline (the `images` stage group): while true,
  // every /api/ and /assets/ request gets its socket destroyed instead of a
  // response, simulating an unreachable server.
  state.offline = false;

  const server = http.createServer((req, res) => {
    handleRequest(req, res, state).catch((err) => {
      if (!res.headersSent) sendJson(res, 500, { detail: String(err) });
    });
  });

  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(port, "127.0.0.1", () => {
      server.removeListener("error", reject);
      resolve();
    });
  });

  const actualPort = server.address().port;

  async function close() {
    await writeRequestLog(fixturesDir, state.requestLog);
    await new Promise((resolve, reject) => {
      server.close((err) => (err ? reject(err) : resolve()));
    });
  }

  return {
    port: actualPort,
    url: `http://127.0.0.1:${actualPort}`,
    requestLog: state.requestLog,
    close,
  };
}

/** Best-effort: a read-only fixtures dir must not make close() throw. */
async function writeRequestLog(fixturesDir, requestLog) {
  try {
    const logPath = path.join(fixturesDir, "..", "last-run-requests.log");
    const lines = requestLog.map((entry) => JSON.stringify(entry)).join("\n");
    await writeFile(logPath, lines.length > 0 ? `${lines}\n` : "", "utf8");
  } catch {
    // Ignore — the request log is a debugging aid, not a hard requirement.
  }
}

// --- standalone entry point ---------------------------------------------

function isMainModule() {
  return process.argv[1] && import.meta.url === `file://${process.argv[1]}`;
}

if (isMainModule()) {
  const args = process.argv.slice(2);
  let port = 0;
  let defaultThrottleMs = 0;
  let fixturesDir = DEFAULT_FIXTURES_DIR;
  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--port" && args[i + 1]) {
      port = Number(args[i + 1]);
      i++;
    } else if (args[i] === "--throttle-ms" && args[i + 1]) {
      defaultThrottleMs = Number(args[i + 1]);
      i++;
    } else if (args[i] === "--fixtures-dir" && args[i + 1]) {
      fixturesDir = path.resolve(args[i + 1]);
      i++;
    }
  }

  const handle = await startMockRomm({ port, defaultThrottleMs, fixturesDir });
  console.log(`mock RomM server listening at ${handle.url}`);

  const shutdown = async () => {
    await handle.close();
    process.exit(0);
  };
  process.on("SIGINT", shutdown);
  process.on("SIGTERM", shutdown);
}
