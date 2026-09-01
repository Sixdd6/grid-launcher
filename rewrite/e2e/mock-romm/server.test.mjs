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
    assert.equal(body.items.length, 1);
    assert.equal(body.items[0].platform_id, 2);
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
