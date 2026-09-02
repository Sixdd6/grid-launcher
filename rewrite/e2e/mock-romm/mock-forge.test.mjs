// Tests for the mock forge HTTP server used by the E2E harness.
//
// Run with: node --test rewrite/e2e/mock-romm/mock-forge.test.mjs
// (explicit file path — `node --test <dir>` is broken on this Node build).
//
// The shapes asserted here mirror what the Rust side reads:
// grid-core/src/launch/source.rs (select_release / select_asset), the
// catalog's own `download_url_regex` for the direct-scrape path, and
// grid-core/src/library/extract.rs for the tar.gz.

import { test } from "node:test";
import assert from "node:assert/strict";
import path from "node:path";
import os from "node:os";
import zlib from "node:zlib";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";

import {
  ARGV_FILE_ENV,
  AUTH_MARKER,
  PCSX2_APPIMAGE_BYTES,
  PCSX2_ASSET_NAME,
  PCSX2_DOWNLOAD_PATH,
  PCSX2_DOWNLOAD_URL,
  PCSX2_RELEASE_PATH,
  PCSX2_TAG,
  REDREAM_DOWNLOAD_PATH,
  REDREAM_DOWNLOAD_URL,
  REDREAM_MEMBER_NAME,
  REDREAM_PAGE_PATH,
  startMockForge,
} from "./mock-forge.mjs";
import { TAR_BLOCK_BYTES } from "./archives.mjs";

/** The linux `download_url_regex` from the repo-root catalog's Redream profile. */
const REDREAM_URL_REGEX =
  /https:\/\/redream\.io\/download\/redream\.x86_64-linux-v[0-9.]+-[0-9]+-g[0-9a-f]+\.tar\.gz/;

/** The linux `asset_patterns` glob from the catalog's PCSX2 profile. */
const PCSX2_ASSET_GLOB = /^pcsx2-v.*-linux-appimage-x64-Qt\.AppImage$/;

async function withForge(fn, options = {}) {
  const handle = await startMockForge({ port: 0, logPath: null, ...options });
  try {
    await fn(handle);
  } finally {
    await handle.close();
  }
}

// --- github release JSON ------------------------------------------------------

test("serves a GitHub release payload select_release/select_asset can use", async () => {
  await withForge(async ({ url }) => {
    const res = await fetch(`${url}${PCSX2_RELEASE_PATH}`);
    assert.equal(res.status, 200);
    const body = await res.json();

    assert.equal(body.tag_name, PCSX2_TAG);
    assert.equal(body.draft, false);
    assert.equal(body.prerelease, false);
    assert.equal(body.assets.length, 1);

    const [asset] = body.assets;
    assert.equal(asset.name, PCSX2_ASSET_NAME);
    assert.equal(asset.browser_download_url, PCSX2_DOWNLOAD_URL);
    assert.equal(asset.state, "uploaded");
    // select_asset trusts `size` as the download's expected byte count.
    assert.equal(asset.size, PCSX2_APPIMAGE_BYTES.length);
    // The catalog's linux asset_patterns glob has to match this name, or
    // the install fails before it downloads anything.
    assert.match(asset.name, PCSX2_ASSET_GLOB);
  });
});

// --- the AppImage stub ---------------------------------------------------------

test("serves the AppImage stub byte for byte", async () => {
  await withForge(async ({ url }) => {
    const res = await fetch(`${url}${PCSX2_DOWNLOAD_PATH}`);
    assert.equal(res.status, 200);
    const body = Buffer.from(await res.arrayBuffer());
    assert.deepEqual(body, PCSX2_APPIMAGE_BYTES);
    assert.equal(Number(res.headers.get("content-length")), PCSX2_APPIMAGE_BYTES.length);

    const text = body.toString("utf8");
    assert.ok(text.startsWith("#!/bin/sh\n"), "the stub must be directly executable");
    assert.match(text, new RegExp(`\\$${ARGV_FILE_ENV}`), "the stub must record its argv");
  });
});

// --- the redream download page --------------------------------------------------

test("serves a download page whose href matches the catalog's download_url_regex", async () => {
  await withForge(async ({ url }) => {
    const res = await fetch(`${url}${REDREAM_PAGE_PATH}`);
    assert.equal(res.status, 200);
    const page = await res.text();

    const match = page.match(REDREAM_URL_REGEX);
    assert.ok(match, "the page must contain a regex-matching linux tar.gz href");
    assert.equal(match[0], REDREAM_DOWNLOAD_URL);
    // Decoys: the scrape must not simply take the first link on the page.
    assert.ok(page.includes("redream.x86_64-windows-"), "expected a windows decoy href");
    assert.ok(page.includes("/changelog"), "expected a non-download decoy href");
  });
});

// --- the redream tar.gz ----------------------------------------------------------

test("serves a real gzipped tar with one 0755 member", async () => {
  await withForge(async ({ url }) => {
    const res = await fetch(`${url}${REDREAM_DOWNLOAD_PATH}`);
    assert.equal(res.status, 200);
    const gz = Buffer.from(await res.arrayBuffer());
    // extract.rs sniffs gzip by magic, not by file name.
    assert.equal(gz[0], 0x1f);
    assert.equal(gz[1], 0x8b);

    const tar = zlib.gunzipSync(gz);
    assert.equal(tar.length % TAR_BLOCK_BYTES, 0, "a tar is a whole number of blocks");

    const header = tar.subarray(0, TAR_BLOCK_BYTES);
    const readField = (offset, length) =>
      header.subarray(offset, offset + length).toString("ascii").replace(/\0.*$/, "").trim();

    assert.equal(readField(0, 100), REDREAM_MEMBER_NAME);
    assert.equal(parseInt(readField(100, 8), 8), 0o755);
    assert.equal(readField(156, 1), "0", "expected a regular-file typeflag");
    assert.equal(readField(257, 6), "ustar");

    // The stored checksum must match a recomputation with the checksum
    // field blanked out, or the Rust `tar` crate rejects the header.
    const stored = parseInt(readField(148, 8), 8);
    const blanked = Buffer.from(header);
    blanked.write("        ", 148, 8, "ascii");
    let computed = 0;
    for (const byte of blanked) computed += byte;
    assert.equal(stored, computed);

    const size = parseInt(readField(124, 12), 8);
    const content = tar.subarray(TAR_BLOCK_BYTES, TAR_BLOCK_BYTES + size).toString("utf8");
    assert.ok(content.startsWith("#!/bin/sh\n"));
    assert.match(content, new RegExp(`\\$${ARGV_FILE_ENV}`));

    // Two zero blocks end the archive.
    const tail = tar.subarray(tar.length - TAR_BLOCK_BYTES * 2);
    assert.ok(tail.every((byte) => byte === 0), "expected the end-of-archive marker");
  });
});

// --- the no-credential rule --------------------------------------------------------

test("answers 500 and logs AUTH-HEADER-SEEN for any Authorization header", async () => {
  await withForge(async ({ url, requestLog }) => {
    const res = await fetch(`${url}${PCSX2_RELEASE_PATH}`, {
      headers: { Authorization: "Bearer anything-at-all" },
    });
    assert.equal(res.status, 500);
    const body = await res.json();
    assert.equal(body.detail, AUTH_MARKER);
    assert.equal(requestLog.length, 1);
    assert.equal(requestLog[0].note, AUTH_MARKER);
    // The header value itself is never recorded.
    assert.ok(!JSON.stringify(requestLog[0]).includes("anything-at-all"));
  });
});

test("an unknown path is a 404", async () => {
  await withForge(async ({ url }) => {
    const res = await fetch(`${url}/api.github.com/repos/someone/else/releases/latest`);
    assert.equal(res.status, 404);
  });
});

// --- the request log ----------------------------------------------------------------

test("appends every request to the log file as it arrives", async () => {
  const dir = mkdtempSync(path.join(os.tmpdir(), "mock-forge-log-"));
  const logPath = path.join(dir, "forge-requests.log");
  try {
    await withForge(
      async ({ url }) => {
        await fetch(`${url}${PCSX2_RELEASE_PATH}`);
        // Read while the server is still running: the emulator-catalog spec
        // greps this file mid-run, so it must not wait for close().
        const lines = readFileSync(logPath, "utf8").trim().split("\n").filter(Boolean);
        assert.equal(lines.length, 1);
        const entry = JSON.parse(lines[0]);
        assert.equal(entry.method, "GET");
        assert.equal(entry.path, PCSX2_RELEASE_PATH);
        assert.equal(entry.note, undefined);
      },
      { logPath },
    );
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
