// Mock RomM server for the Rust rewrite's E2E harness.
//
// Plain node:http, ESM, no npm dependencies. Mirrors the endpoints the Rust
// client calls (see grid-core/src/romm/mod.rs, covers.rs, library/mod.rs):
//   GET  /api/users/me
//   GET  /api/platforms
//   GET  /api/roms?platform_ids=&limit=&offset=&with_char_index=&with_filter_values=
//   GET  /api/roms/:id
//   GET  /api/roms/:id/content/:file_name?file_ids=[&e2e_throttle=<ms-per-chunk>]
//   GET  /assets/romm/resources/roms/:id/cover/small.png
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
//   node server.mjs --port 8931 [--throttle-ms 100]

import http from "node:http";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { readFile, writeFile } from "node:fs/promises";

export const FAKE_TOKEN = "FAKE-E2E-TOKEN-not-real";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const DEFAULT_FIXTURES_DIR = path.join(__dirname, "../fixtures");

// --- CRC32 (used by the stored-entry zip writer below) ---------------------

const CRC_TABLE = buildCrcTable();

function buildCrcTable() {
  const table = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) {
      c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    }
    table[n] = c >>> 0;
  }
  return table;
}

function crc32(buf) {
  let crc = 0xffffffff;
  for (let i = 0; i < buf.length; i++) {
    crc = CRC_TABLE[(crc ^ buf[i]) & 0xff] ^ (crc >>> 8);
  }
  return (crc ^ 0xffffffff) >>> 0;
}

// --- minimal stored-entry (uncompressed) ZIP writer -------------------------

/**
 * Builds a spec-valid ZIP archive from `entries` (`{name, data: Buffer}[]`).
 * Every entry is stored (compression method 0), so no compression code is
 * needed — the archive is still a real ZIP that any conformant reader
 * (including the Rust `zip` crate) can extract.
 */
function buildZip(entries) {
  const localChunks = [];
  const centralChunks = [];
  let offset = 0;
  const DOS_DATE_1980_01_01 = 0x0021;

  for (const { name, data } of entries) {
    const nameBuf = Buffer.from(name, "utf8");
    const crc = crc32(data);

    const local = Buffer.alloc(30);
    local.writeUInt32LE(0x04034b50, 0); // local file header signature
    local.writeUInt16LE(20, 4); // version needed to extract
    local.writeUInt16LE(0, 6); // general purpose bit flag
    local.writeUInt16LE(0, 8); // compression method: stored
    local.writeUInt16LE(0, 10); // last mod file time
    local.writeUInt16LE(DOS_DATE_1980_01_01, 12); // last mod file date
    local.writeUInt32LE(crc, 14);
    local.writeUInt32LE(data.length, 18); // compressed size
    local.writeUInt32LE(data.length, 22); // uncompressed size
    local.writeUInt16LE(nameBuf.length, 26);
    local.writeUInt16LE(0, 28); // extra field length
    localChunks.push(local, nameBuf, data);

    const central = Buffer.alloc(46);
    central.writeUInt32LE(0x02014b50, 0); // central file header signature
    central.writeUInt16LE(20, 4); // version made by
    central.writeUInt16LE(20, 6); // version needed to extract
    central.writeUInt16LE(0, 8);
    central.writeUInt16LE(0, 10);
    central.writeUInt16LE(0, 12);
    central.writeUInt16LE(DOS_DATE_1980_01_01, 14);
    central.writeUInt32LE(crc, 16);
    central.writeUInt32LE(data.length, 20);
    central.writeUInt32LE(data.length, 24);
    central.writeUInt16LE(nameBuf.length, 28);
    central.writeUInt16LE(0, 30); // extra field length
    central.writeUInt16LE(0, 32); // file comment length
    central.writeUInt16LE(0, 34); // disk number start
    central.writeUInt16LE(0, 36); // internal file attributes
    central.writeUInt32LE(0, 38); // external file attributes
    central.writeUInt32LE(offset, 42); // relative offset of local header
    centralChunks.push(central, nameBuf);

    offset += local.length + nameBuf.length + data.length;
  }

  const centralDirStart = offset;
  const centralDir = Buffer.concat(centralChunks);

  const eocd = Buffer.alloc(22);
  eocd.writeUInt32LE(0x06054b50, 0); // end of central dir signature
  eocd.writeUInt16LE(0, 4); // number of this disk
  eocd.writeUInt16LE(0, 6); // disk with the start of the central directory
  eocd.writeUInt16LE(entries.length, 8); // entries on this disk
  eocd.writeUInt16LE(entries.length, 10); // total entries
  eocd.writeUInt32LE(centralDir.length, 12); // size of central directory
  eocd.writeUInt32LE(centralDirStart, 16); // offset of central directory
  eocd.writeUInt16LE(0, 20); // comment length

  return Buffer.concat([...localChunks, centralDir, eocd]);
}

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

async function loadFixtures(fixturesDir) {
  const [platforms, romsByPlatform, romDetails] = await Promise.all([
    readFile(path.join(fixturesDir, "platforms.json"), "utf8").then(JSON.parse),
    readFile(path.join(fixturesDir, "roms.json"), "utf8").then(JSON.parse),
    readFile(path.join(fixturesDir, "rom-details.json"), "utf8").then(JSON.parse),
  ]);

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

  return { platforms, romsByPlatform, romDetails, contentByKey, pngBytes: content.pngBytes };
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

const COVER_PATH_RE = /^\/assets\/romm\/resources\/roms\/\d+\/cover\/small\.png$/;
const ROM_ID_RE = /^\/api\/roms\/(\d+)$/;
const ROM_CONTENT_RE = /^\/api\/roms\/(\d+)\/content\/(.+)$/;

function handleRequest(req, res, state) {
  const requestUrl = new URL(req.url, "http://127.0.0.1");
  const pathname = decodeURIComponent(requestUrl.pathname);

  state.requestLog.push({ method: req.method, path: req.url });

  // Static-style cover asset: not under /api, no auth required — mirrors
  // RomM serving cover images directly off disk.
  if (req.method === "GET" && COVER_PATH_RE.test(pathname)) {
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
  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--port" && args[i + 1]) {
      port = Number(args[i + 1]);
      i++;
    } else if (args[i] === "--throttle-ms" && args[i + 1]) {
      defaultThrottleMs = Number(args[i + 1]);
      i++;
    }
  }

  const handle = await startMockRomm({ port, defaultThrottleMs });
  console.log(`mock RomM server listening at ${handle.url}`);

  const shutdown = async () => {
    await handle.close();
    process.exit(0);
  };
  process.on("SIGINT", shutdown);
  process.on("SIGTERM", shutdown);
}
