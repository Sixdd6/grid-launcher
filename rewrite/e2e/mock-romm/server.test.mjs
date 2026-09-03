// Tests for the mock RomM HTTP server used by the E2E harness.
//
// Run with: node --test rewrite/e2e/mock-romm/
//
// The shapes asserted here mirror what the Rust client decodes in
// grid-core/src/romm/mod.rs (RawGameSummary, RawRomDetail, RomFile) and the
// content/cover paths built in grid-core/src/library/mod.rs and
// grid-core/src/covers.rs.

import { test } from "node:test";
import assert from "node:assert/strict";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { readFile, rm } from "node:fs/promises";

import { startMockRomm } from "./server.mjs";

const TOKEN = "FAKE-E2E-TOKEN-not-real";
const __dirname = path.dirname(fileURLToPath(import.meta.url));
const fixturesDir = path.join(__dirname, "../fixtures");

function authHeader() {
  return { Authorization: `Bearer ${TOKEN}` };
}

async function withServer(fn) {
  const handle = await startMockRomm({ port: 0 });
  try {
    await fn(handle);
  } finally {
    await handle.close();
  }
}

// --- auth --------------------------------------------------------------

test("rejects a request with no Authorization header", async () => {
  await withServer(async ({ url }) => {
    const res = await fetch(`${url}/api/users/me`);
    assert.equal(res.status, 401);
    const body = await res.json();
    assert.ok(body && typeof body === "object");
  });
});

test("rejects a request with the wrong bearer token", async () => {
  await withServer(async ({ url }) => {
    const res = await fetch(`${url}/api/users/me`, {
      headers: { Authorization: "Bearer not-the-right-token" },
    });
    assert.equal(res.status, 401);
  });
});

// --- users/me ------------------------------------------------------------

test("GET /api/users/me returns the fixture user", async () => {
  await withServer(async ({ url }) => {
    const res = await fetch(`${url}/api/users/me`, { headers: authHeader() });
    assert.equal(res.status, 200);
    const body = await res.json();
    assert.deepEqual(body, { id: 1, username: "e2euser" });
  });
});

// --- platforms -------------------------------------------------------------

test("GET /api/platforms returns two platforms, both with rom_count > 0", async () => {
  await withServer(async ({ url }) => {
    const res = await fetch(`${url}/api/platforms`, { headers: authHeader() });
    assert.equal(res.status, 200);
    const body = await res.json();
    assert.equal(body.length, 2);
    const names = body.map((p) => p.name).sort();
    assert.deepEqual(names, ["Arcade", "Super Nintendo Entertainment System"]);
    for (const platform of body) {
      assert.ok(platform.rom_count > 0, `expected rom_count > 0 for ${platform.name}`);
      assert.ok(platform.id);
      assert.ok(platform.slug);
    }
  });
});

// --- roms list / pagination --------------------------------------------

test("GET /api/roms honors limit/offset and returns an {items} envelope", async () => {
  await withServer(async ({ url }) => {
    const query = "platform_ids=1&with_char_index=false&with_filter_values=false";
    const page1 = await fetch(`${url}/api/roms?${query}&limit=2&offset=0`, {
      headers: authHeader(),
    }).then((r) => r.json());
    assert.equal(page1.items.length, 2);

    const page2 = await fetch(`${url}/api/roms?${query}&limit=2&offset=2`, {
      headers: authHeader(),
    }).then((r) => r.json());
    assert.equal(page2.items.length, 1);

    const ids = [...page1.items, ...page2.items].map((i) => i.id).sort((a, b) => a - b);
    assert.deepEqual(ids, [101, 102, 103]);
  });
});

test("GET /api/roms filters by platform_ids", async () => {
  await withServer(async ({ url }) => {
    const body = await fetch(
      `${url}/api/roms?platform_ids=2&limit=200&offset=0&with_char_index=false&with_filter_values=false`,
      { headers: authHeader() },
    ).then((r) => r.json());
    // Pac-Man (201) plus the "Big Arcade Game" (301) throttle fixture.
    assert.equal(body.items.length, 2);
    assert.ok(body.items.every((i) => i.platform_id === 2));
    assert.deepEqual(
      body.items.map((i) => i.id).sort((a, b) => a - b),
      [201, 301],
    );
  });
});

test("GET /api/roms includes one game with a null name (fs_name_no_ext fallback)", async () => {
  await withServer(async ({ url }) => {
    const body = await fetch(
      `${url}/api/roms?platform_ids=1&limit=200&offset=0&with_char_index=false&with_filter_values=false`,
      { headers: authHeader() },
    ).then((r) => r.json());
    const nullNamed = body.items.find((i) => i.name === null);
    assert.ok(nullNamed, "expected a game with name === null");
    assert.ok(nullNamed.fs_name_no_ext, "expected a fs_name_no_ext fallback");
  });
});

// --- rom detail --------------------------------------------------------

test("GET /api/roms/:id returns full detail for a single-file game", async () => {
  await withServer(async ({ url }) => {
    const detail = await fetch(`${url}/api/roms/101`, { headers: authHeader() }).then((r) =>
      r.json(),
    );
    assert.equal(detail.id, 101);
    assert.equal(detail.name, "Super Mario World");
    assert.equal(detail.platform_id, 1);
    assert.equal(detail.platform_display_name, "Super Nintendo Entertainment System");
    assert.equal(detail.files.length, 1);
    assert.equal(detail.files[0].is_top_level, true);
    assert.ok(detail.files[0].file_size_bytes > 0);
    assert.ok(detail.metadatum);
  });
});

test("GET /api/roms/:id: null-name game falls back to fs_name_no_ext", async () => {
  await withServer(async ({ url }) => {
    const detail = await fetch(`${url}/api/roms/102`, { headers: authHeader() }).then((r) =>
      r.json(),
    );
    assert.equal(detail.name, null);
    assert.equal(detail.fs_name_no_ext, "Chrono Trigger (USA)");
  });
});

test("GET /api/roms/:id: multi-file game has an .m3u launch entry plus two other top-level files", async () => {
  await withServer(async ({ url }) => {
    const detail = await fetch(`${url}/api/roms/103`, { headers: authHeader() }).then((r) =>
      r.json(),
    );
    assert.equal(detail.files.length, 3);
    const m3uFiles = detail.files.filter((f) => f.file_name.toLowerCase().endsWith(".m3u"));
    assert.equal(m3uFiles.length, 1);
    assert.ok(detail.files.every((f) => f.is_top_level));
  });
});

// --- content download ----------------------------------------------------

test("content endpoint streams a spec-valid zip containing game.sfc", async () => {
  await withServer(async ({ url }) => {
    const detail = await fetch(`${url}/api/roms/101`, { headers: authHeader() }).then((r) =>
      r.json(),
    );
    const file = detail.files[0];
    const res = await fetch(
      `${url}/api/roms/101/content/${encodeURIComponent(file.file_name)}?file_ids=${file.id}`,
      { headers: authHeader() },
    );
    assert.equal(res.status, 200);
    const buf = Buffer.from(await res.arrayBuffer());
    assert.equal(buf.length, file.file_size_bytes);
    // Local file header signature "PK\x03\x04".
    assert.deepEqual(buf.subarray(0, 4), Buffer.from([0x50, 0x4b, 0x03, 0x04]));
    assert.ok(buf.includes(Buffer.from("game.sfc")), "zip bytes should contain the entry name");
  });
});

test("content endpoint streams plain (non-zip) files for a multi-file game", async () => {
  await withServer(async ({ url }) => {
    const detail = await fetch(`${url}/api/roms/103`, { headers: authHeader() }).then((r) =>
      r.json(),
    );
    for (const file of detail.files) {
      const res = await fetch(
        `${url}/api/roms/103/content/${encodeURIComponent(file.file_name)}?file_ids=${file.id}`,
        { headers: authHeader() },
      );
      assert.equal(res.status, 200);
      const buf = Buffer.from(await res.arrayBuffer());
      assert.equal(buf.length, file.file_size_bytes);
      assert.notDeepEqual(buf.subarray(0, 4), Buffer.from([0x50, 0x4b, 0x03, 0x04]));
    }
  });
});

test("content endpoint 404s for an unknown file", async () => {
  await withServer(async ({ url }) => {
    const res = await fetch(`${url}/api/roms/101/content/nope.zip?file_ids=9999`, {
      headers: authHeader(),
    });
    assert.equal(res.status, 404);
  });
});

// --- e2e_throttle (chunked slow streaming) --------------------------------

test("content endpoint streams instantly with no throttle requested", async () => {
  await withServer(async ({ url }) => {
    const detail = await fetch(`${url}/api/roms/101`, { headers: authHeader() }).then((r) =>
      r.json(),
    );
    const file = detail.files[0];
    const start = Date.now();
    const res = await fetch(
      `${url}/api/roms/101/content/${encodeURIComponent(file.file_name)}?file_ids=${file.id}`,
      { headers: authHeader() },
    );
    const buf = Buffer.from(await res.arrayBuffer());
    assert.equal(buf.length, file.file_size_bytes);
    assert.ok(Date.now() - start < 200, "an unthrottled response should not be artificially delayed");
  });
});

// A single chunk under THROTTLE_CHUNK_BYTES delivers its whole
// Content-Length-declared body in one write, so a client that only measures
// "time to full body" never observes the trailing delay before res.end() —
// it already has everything it declared it would get. So a *meaningful*
// timing assertion needs content that spans multiple chunks: the "Big
// Arcade Game" fixture (rom 301, ~300KB against a 20KB chunk size).

test("content endpoint honors ?e2e_throttle= by streaming in delayed chunks", async () => {
  await withServer(async ({ url }) => {
    const detail = await fetch(`${url}/api/roms/301`, { headers: authHeader() }).then((r) =>
      r.json(),
    );
    const file = detail.files[0];
    assert.ok(file.file_size_bytes > 20 * 1024, "expected the big fixture to exceed one chunk");
    const start = Date.now();
    const res = await fetch(
      `${url}/api/roms/301/content/${encodeURIComponent(file.file_name)}?file_ids=${file.id}&e2e_throttle=10`,
      { headers: authHeader() },
    );
    assert.equal(res.status, 200);
    const buf = Buffer.from(await res.arrayBuffer());
    const elapsed = Date.now() - start;
    // This server was started with no defaultThrottleMs, so any delay here
    // is proof the query param alone drove the throttling.
    assert.equal(buf.length, file.file_size_bytes);
    assert.ok(elapsed >= 100, `expected a multi-chunk throttled response, took ${elapsed}ms`);
  });
});

test("a server-wide defaultThrottleMs throttles content requests with no query param", async () => {
  const handle = await startMockRomm({ port: 0, defaultThrottleMs: 10 });
  try {
    const detail = await fetch(`${handle.url}/api/roms/301`, { headers: authHeader() }).then(
      (r) => r.json(),
    );
    const file = detail.files[0];
    const start = Date.now();
    const res = await fetch(
      `${handle.url}/api/roms/301/content/${encodeURIComponent(file.file_name)}?file_ids=${file.id}`,
      { headers: authHeader() },
    );
    const buf = Buffer.from(await res.arrayBuffer());
    const elapsed = Date.now() - start;
    assert.equal(buf.length, file.file_size_bytes);
    assert.ok(elapsed >= 100, `expected the server-wide default to throttle, took ${elapsed}ms`);
  } finally {
    await handle.close();
  }
});

test("an explicit ?e2e_throttle=0 request is not throttled even with a server-wide default", async () => {
  const handle = await startMockRomm({ port: 0, defaultThrottleMs: 5000 });
  try {
    const detail = await fetch(`${handle.url}/api/roms/301`, { headers: authHeader() }).then(
      (r) => r.json(),
    );
    const file = detail.files[0];
    // Must exceed one chunk (like the other multi-chunk tests above) — a
    // fetch() resolves as soon as headers arrive, not once the body is
    // fully read, so a single-chunk body would let this pass even if the
    // throttling logic were still broken. Reading the whole body below is
    // what actually exercises the override.
    assert.ok(file.file_size_bytes > 20 * 1024, "expected the big fixture to exceed one chunk");
    const start = Date.now();
    const res = await fetch(
      `${handle.url}/api/roms/301/content/${encodeURIComponent(file.file_name)}?file_ids=${file.id}&e2e_throttle=0`,
      { headers: authHeader() },
    );
    const buf = Buffer.from(await res.arrayBuffer());
    const elapsed = Date.now() - start;
    assert.equal(buf.length, file.file_size_bytes);
    // At the 5000ms server-wide default this fixture (~15 chunks) would take
    // well over a minute; a threshold two orders of magnitude below that
    // still leaves plenty of room for slow CI without masking a regression.
    assert.ok(
      elapsed < 1000,
      `e2e_throttle=0 should override the server-wide default, took ${elapsed}ms`,
    );
  } finally {
    await handle.close();
  }
});

test("the big-content fixture (rom 301) streams in multiple throttled chunks", async () => {
  const handle = await startMockRomm({ port: 0, defaultThrottleMs: 20 });
  try {
    const detail = await fetch(`${handle.url}/api/roms/301`, { headers: authHeader() }).then(
      (r) => r.json(),
    );
    const file = detail.files[0];
    assert.ok(file.file_size_bytes > 20 * 1024, "expected the big fixture to exceed one chunk");
    const start = Date.now();
    const res = await fetch(
      `${handle.url}/api/roms/301/content/${encodeURIComponent(file.file_name)}?file_ids=${file.id}`,
      { headers: authHeader() },
    );
    const buf = Buffer.from(await res.arrayBuffer());
    const elapsed = Date.now() - start;
    assert.equal(buf.length, file.file_size_bytes);
    // Multiple ~20KB chunks at 20ms each: comfortably more than one chunk's
    // delay proves this really streamed in pieces, not one big write.
    assert.ok(elapsed >= 40, `expected multiple throttled chunks, took ${elapsed}ms`);
  } finally {
    await handle.close();
  }
});

test("a throttled response stops cleanly when the client aborts mid-stream", async () => {
  const handle = await startMockRomm({ port: 0, defaultThrottleMs: 100 });
  try {
    const detail = await fetch(`${handle.url}/api/roms/301`, { headers: authHeader() }).then(
      (r) => r.json(),
    );
    const file = detail.files[0];
    const controller = new AbortController();
    const res = await fetch(
      `${handle.url}/api/roms/301/content/${encodeURIComponent(file.file_name)}?file_ids=${file.id}`,
      { headers: authHeader(), signal: controller.signal },
    );
    assert.equal(res.status, 200);
    // Headers arrive immediately (writeHead is not throttled); the body is
    // still streaming in throttled chunks when the abort fires.
    const readPromise = res.arrayBuffer().catch(() => {});
    setTimeout(() => controller.abort(), 150);
    await readPromise;
    // The server must still be alive and answering other requests — an
    // unhandled exception from writing to the aborted response would have
    // crashed the process.
    const health = await fetch(`${handle.url}/api/users/me`, { headers: authHeader() });
    assert.equal(health.status, 200);
  } finally {
    await handle.close();
  }
});

// --- cover -----------------------------------------------------------------

test("cover endpoint returns a 1x1 PNG", async () => {
  await withServer(async ({ url }) => {
    const detail = await fetch(`${url}/api/roms/101`, { headers: authHeader() });
    void detail; // cover path is fixed by fixtures, not by this response
    const res = await fetch(`${url}/assets/romm/resources/roms/101/cover/small.png`);
    assert.equal(res.status, 200);
    assert.equal(res.headers.get("content-type"), "image/png");
    const buf = Buffer.from(await res.arrayBuffer());
    assert.deepEqual(
      buf.subarray(0, 8),
      Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    );
  });
});

test("cover endpoint serves the large variant too", async () => {
  await withServer(async ({ url }) => {
    const res = await fetch(`${url}/assets/romm/resources/roms/101/cover/large.png`);
    assert.equal(res.status, 200);
    assert.equal(res.headers.get("content-type"), "image/png");
    const buf = Buffer.from(await res.arrayBuffer());
    assert.deepEqual(
      buf.subarray(0, 8),
      Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    );
  });
});

test("screenshot endpoint returns a PNG", async () => {
  await withServer(async ({ url }) => {
    const res = await fetch(`${url}/assets/romm/resources/roms/101/screenshots/1.png`);
    assert.equal(res.status, 200);
    assert.equal(res.headers.get("content-type"), "image/png");
    const buf = Buffer.from(await res.arrayBuffer());
    assert.deepEqual(
      buf.subarray(0, 8),
      Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    );
  });
});

// --- offline toggle ------------------------------------------------------

test("POST /__e2e__/offline {offline:true} makes /api/ requests fail with a connection error, and {offline:false} restores them", async () => {
  await withServer(async ({ url }) => {
    const setOffline = await fetch(`${url}/__e2e__/offline`, {
      method: "POST",
      body: JSON.stringify({ offline: true }),
    });
    assert.equal(setOffline.status, 200);
    assert.deepEqual(await setOffline.json(), { offline: true });

    await assert.rejects(fetch(`${url}/api/users/me`, { headers: authHeader() }));

    const getOffline = await fetch(`${url}/__e2e__/offline`);
    assert.equal(getOffline.status, 200);
    assert.deepEqual(await getOffline.json(), { offline: true });

    const setOnline = await fetch(`${url}/__e2e__/offline`, {
      method: "POST",
      body: JSON.stringify({ offline: false }),
    });
    assert.deepEqual(await setOnline.json(), { offline: false });

    const res = await fetch(`${url}/api/users/me`, { headers: authHeader() });
    assert.equal(res.status, 200);
    assert.deepEqual(await res.json(), { id: 1, username: "e2euser" });
  });
});

test("the offline toggle also blocks /assets/ but /__e2e__/ routes keep working", async () => {
  await withServer(async ({ url }) => {
    await fetch(`${url}/__e2e__/offline`, {
      method: "POST",
      body: JSON.stringify({ offline: true }),
    });

    await assert.rejects(fetch(`${url}/assets/romm/resources/roms/101/cover/small.png`));

    // The introspection route still works while offline.
    const res = await fetch(`${url}/__e2e__/requests`);
    assert.equal(res.status, 200);

    await fetch(`${url}/__e2e__/offline`, {
      method: "POST",
      body: JSON.stringify({ offline: false }),
    });
  });
});

// --- request log -------------------------------------------------------

test("requestLog records {method, path} for each request", async () => {
  const handle = await startMockRomm({ port: 0 });
  try {
    await fetch(`${handle.url}/api/users/me`, { headers: authHeader() });
    await fetch(`${handle.url}/api/platforms?foo=bar`, { headers: authHeader() });
    assert.ok(handle.requestLog.length >= 2);
    const meEntry = handle.requestLog.find((e) => e.path.startsWith("/api/users/me"));
    assert.ok(meEntry);
    assert.equal(meEntry.method, "GET");
    const platformsEntry = handle.requestLog.find((e) => e.path.startsWith("/api/platforms"));
    assert.ok(platformsEntry);
    assert.ok(platformsEntry.path.includes("foo=bar"));
  } finally {
    await handle.close();
  }
});

test("close() writes the request log to <fixturesDir>/../last-run-requests.log", async () => {
  const handle = await startMockRomm({ port: 0 });
  await fetch(`${handle.url}/api/users/me`, { headers: authHeader() });
  await handle.close();

  const logPath = path.join(fixturesDir, "..", "last-run-requests.log");
  try {
    const content = await readFile(logPath, "utf8");
    const lines = content.trim().split("\n").filter(Boolean);
    assert.ok(lines.length >= 1);
    const parsed = JSON.parse(lines[0]);
    assert.ok(parsed.method);
    assert.ok(parsed.path);
  } finally {
    await rm(logPath, { force: true });
  }
});

// --- standalone port option -------------------------------------------

test("startMockRomm honors an explicit port", async () => {
  const handle = await startMockRomm({ port: 0 });
  try {
    assert.ok(handle.port > 0);
    assert.equal(handle.url, `http://127.0.0.1:${handle.port}`);
  } finally {
    await handle.close();
  }
});

// --- file_ids by-id resolution (PS4 / Xbox 360 update+DLC) -----------------
//
// RomM's content endpoint takes the GAME's `fs_name` in the path and names
// the file(s) actually wanted in one `file_ids=<csv>` query pair, which is
// how grid-core fetches an update from the same path as its base game
// (library/mod.rs's `content_job`). These pin the mock's side of that.

const contentFixturesDir = path.join(__dirname, "../fixtures-content");
const firmwareFixturesDir = path.join(__dirname, "../fixtures-firmware");
const nativeFixturesDir = path.join(__dirname, "../fixtures-native");

async function withFixtureServer(dir, fn) {
  const handle = await startMockRomm({ port: 0, fixturesDir: dir });
  try {
    await fn(handle);
  } finally {
    await handle.close();
  }
}

test("file_ids selects the update archive even though the path names the base game", async () => {
  await withFixtureServer(contentFixturesDir, async ({ url }) => {
    const detail = await fetch(`${url}/api/roms/501`, { headers: authHeader() }).then((r) =>
      r.json(),
    );
    const base = detail.files.find((f) => f.file_name === "ps4-base.zip");
    const update = detail.files.find((f) => f.file_name === "ps4-update.zip");

    // Both requests use the game's own fs_name in the path; only file_ids differs.
    const baseRes = await fetch(`${url}/api/roms/501/content/ps4-base.zip?file_ids=${base.id}`, {
      headers: authHeader(),
    });
    const updateRes = await fetch(
      `${url}/api/roms/501/content/ps4-base.zip?file_ids=${update.id}`,
      { headers: authHeader() },
    );
    assert.equal(baseRes.status, 200);
    assert.equal(updateRes.status, 200);

    const baseBuf = Buffer.from(await baseRes.arrayBuffer());
    const updateBuf = Buffer.from(await updateRes.arrayBuffer());
    assert.equal(baseBuf.length, base.file_size_bytes);
    assert.equal(updateBuf.length, update.file_size_bytes);
    assert.ok(baseBuf.includes(Buffer.from("CUSA12345/sce_sys/param.sfo")));
    assert.ok(updateBuf.includes(Buffer.from("CUSA12345/patch.txt")));
    assert.ok(!baseBuf.includes(Buffer.from("CUSA12345/patch.txt")));
  });
});

test("a file_ids value naming no file of this rom falls back to the path's file name", async () => {
  await withFixtureServer(contentFixturesDir, async ({ url }) => {
    const res = await fetch(`${url}/api/roms/501/content/ps4-base.zip?file_ids=999999`, {
      headers: authHeader(),
    });
    assert.equal(res.status, 200);
    const buf = Buffer.from(await res.arrayBuffer());
    assert.ok(buf.includes(Buffer.from("CUSA12345/sce_sys/param.sfo")));
  });
});

test("the Xbox 360 update archive carries one STFS package with the fixture's title id", async () => {
  await withFixtureServer(contentFixturesDir, async ({ url }) => {
    const detail = await fetch(`${url}/api/roms/601`, { headers: authHeader() }).then((r) =>
      r.json(),
    );
    const update = detail.files.find((f) => f.file_name === "x360-update.zip");
    const res = await fetch(`${url}/api/roms/601/content/x360.zip?file_ids=${update.id}`, {
      headers: authHeader(),
    });
    const buf = Buffer.from(await res.arrayBuffer());
    assert.ok(buf.includes(Buffer.from("tu00000001")), "the STFS member name is in the zip");
    // Stored (uncompressed) entries, so the STFS header sits verbatim in the
    // archive: magic, then the big-endian content type and title id.
    assert.ok(buf.includes(Buffer.from("LIVE", "ascii")));
    assert.ok(buf.includes(Buffer.from([0x00, 0x0b, 0x00, 0x00])));
    assert.ok(buf.includes(Buffer.from([0x41, 0x56, 0x08, 0xc3])));
  });
});

test("a rom file's fixture e2e_throttle is stripped from the detail but throttles its content", async () => {
  await withFixtureServer(nativeFixturesDir, async ({ url }) => {
    const detail = await fetch(`${url}/api/roms/702`, { headers: authHeader() }).then((r) =>
      r.json(),
    );
    const file = detail.files[0];
    assert.equal(
      file.e2e_throttle,
      undefined,
      "e2e_throttle is a mock-only directive and must never reach a client",
    );

    const start = Date.now();
    const res = await fetch(`${url}/api/roms/702/content/big.zip?file_ids=${file.id}`, {
      headers: authHeader(),
    });
    const buf = Buffer.from(await res.arrayBuffer());
    const elapsed = Date.now() - start;
    assert.equal(buf.length, file.file_size_bytes);
    // ~300KB at 20KB per chunk, 100ms apart: well over a second. The bound
    // is deliberately loose — this asserts "throttled at all", not a rate.
    assert.ok(elapsed > 500, `expected a throttled stream, took ${elapsed}ms`);
  });
});

test("an unthrottled file in the same fixture set streams instantly", async () => {
  await withFixtureServer(nativeFixturesDir, async ({ url }) => {
    const start = Date.now();
    const res = await fetch(`${url}/api/roms/701/content/mygame.zip?file_ids=4001`, {
      headers: authHeader(),
    });
    await res.arrayBuffer();
    assert.ok(Date.now() - start < 400, "only the file carrying e2e_throttle is throttled");
  });
});

// --- server firmware -------------------------------------------------------

test("GET /api/firmware returns the fixture records without their content_key", async () => {
  await withFixtureServer(firmwareFixturesDir, async ({ url }) => {
    const res = await fetch(`${url}/api/firmware?platform_id=1`, { headers: authHeader() });
    assert.equal(res.status, 200);
    const body = await res.json();
    assert.deepEqual(body, [{ id: 9001, file_name: "scph5501.bin" }]);

    const ps3 = await fetch(`${url}/api/firmware?platform_id=2`, {
      headers: authHeader(),
    }).then((r) => r.json());
    assert.deepEqual(ps3, [{ id: 9002, file_name: "PS3UPDAT.PUP" }]);
  });
});

test("GET /api/firmware for a platform with no records returns an empty list", async () => {
  await withFixtureServer(firmwareFixturesDir, async ({ url }) => {
    const res = await fetch(`${url}/api/firmware?platform_id=77`, { headers: authHeader() });
    assert.equal(res.status, 200);
    assert.deepEqual(await res.json(), []);
  });
});

test("a fixture set with no firmware.json serves an empty firmware list", async () => {
  await withServer(async ({ url }) => {
    const res = await fetch(`${url}/api/firmware?platform_id=1`, { headers: authHeader() });
    assert.equal(res.status, 200);
    assert.deepEqual(await res.json(), []);
  });
});

test("GET /api/firmware/:id/content/:name serves the bytes its content_key names", async () => {
  await withFixtureServer(firmwareFixturesDir, async ({ url }) => {
    const bios = await fetch(`${url}/api/firmware/9001/content/scph5501.bin`, {
      headers: authHeader(),
    });
    assert.equal(bios.status, 200);
    const biosBuf = Buffer.from(await bios.arrayBuffer());
    assert.equal(biosBuf.length, 512);

    const pup = await fetch(`${url}/api/firmware/9002/content/PS3UPDAT.PUP`, {
      headers: authHeader(),
    });
    assert.equal(pup.status, 200);
    const pupBuf = Buffer.from(await pup.arrayBuffer());
    assert.equal(pupBuf.length, 1024);
    assert.equal(pupBuf.subarray(0, 5).toString("ascii"), "SCEUF");
  });
});

test("GET /api/firmware/:id/content 404s for an unknown firmware id", async () => {
  await withFixtureServer(firmwareFixturesDir, async ({ url }) => {
    const res = await fetch(`${url}/api/firmware/4242/content/whatever.bin`, {
      headers: authHeader(),
    });
    assert.equal(res.status, 404);
  });
});

test("the firmware routes require the bearer token like every other /api/ route", async () => {
  await withFixtureServer(firmwareFixturesDir, async ({ url }) => {
    assert.equal((await fetch(`${url}/api/firmware?platform_id=1`)).status, 401);
    assert.equal((await fetch(`${url}/api/firmware/9001/content/scph5501.bin`)).status, 401);
  });
});

// --- category passthrough --------------------------------------------------

test("a rom detail's files keep their RomM category verbatim", async () => {
  await withFixtureServer(contentFixturesDir, async ({ url }) => {
    const detail = await fetch(`${url}/api/roms/601`, { headers: authHeader() }).then((r) =>
      r.json(),
    );
    assert.deepEqual(
      detail.files.map((f) => [f.file_name, f.category]),
      [
        ["x360.zip", "game"],
        ["x360-update.zip", "update"],
      ],
    );
  });
});

// --- the new fixture sets' own shape --------------------------------------

test("the PS3 fixture archive holds the BLUS30336/PS3_GAME tree", async () => {
  await withFixtureServer(path.join(__dirname, "../fixtures-ps3-install"), async ({ url }) => {
    const res = await fetch(`${url}/api/roms/401/content/game.zip?file_ids=1401`, {
      headers: authHeader(),
    });
    const buf = Buffer.from(await res.arrayBuffer());
    assert.ok(buf.includes(Buffer.from("BLUS30336/PS3_GAME/USRDIR/EBOOT.BIN")));
    assert.ok(buf.includes(Buffer.from("BLUS30336/PS3_GAME/PARAM.SFO")));
  });
});

test("the native fixture serves game.json as its own JSON body", async () => {
  await withFixtureServer(nativeFixturesDir, async ({ url }) => {
    const res = await fetch(`${url}/api/roms/701/content/game.json?file_ids=4002`, {
      headers: authHeader(),
    });
    const body = JSON.parse(Buffer.from(await res.arrayBuffer()).toString("utf8"));
    assert.deepEqual(body, { version: "1.0", year: 2004, tags: ["indie"] });
  });
});
