// Mock GOG for the E2E harness (docs/stores-phase1-plan.md package D).
//
// Plain node:http, ESM, no npm dependencies. One server stands in for every
// GOG host GRID talks to: an e2e build of the app reads
// GRID_LAUNCHER_E2E_GOG_BASE (stores_service.rs `gog_endpoints`) and points
// all five `GogEndpoints` (auth, embed, api, content, cloud) here, so every
// GOG path below lives at this server's root. The account and its games come
// from e2e/fixtures-gog/products.json. No real account, product or token:
// the only token is FAKE-STORE-TOKEN-not-real.
//
// Routes:
//   GET  /  and  /auth…                 the login page (a form; GRID's login
//                                       window opens it)
//   POST /auth/login                     302 to /on_login_success?origin=client&code=…
//                                        user "denied"  → …?error=access_denied (no code)
//                                        user "refused" → a code fake gogdl refuses
//   GET  /on_login_success               a page GRID must never let load
//   GET|POST /token                      auth.gog.com/token (code or refresh)
//   GET  /userData.json                  embed: the signed-in user
//   GET  /user/data/games                embed: owned ids (DLC too)
//   GET  /account/getFilteredProducts    embed: the owned games, one page
//   GET  /products?ids=a,b               api.gog.com, a batch (expand=…)
//   GET  /products/<id>                  api.gog.com/products/<id>?expand=…
//   GET  /v2/games/<id>                  api.gog.com/v2/games/<id>
//   GET  /products/<id>/os/<os>/builds   content-system builds (generation 2)
//   GET  /platforms/gog/external_releases/<id>   gamesdb (art, genres)
//   GET  /users/<uid>/releases           galaxy-library
//   GET  /components/galaxy_client/clients/<clientId>   remote-config (cloud
//                                        save locations)
//   GET  /v1/<uid>/<clientId>            cloudstorage listing ([] at first)
//   GET|PUT|DELETE /v1/<uid>/<clientId>/<path>   cloudstorage files (in memory)
//   GET  /gogdl/good/gogdl_linux_x86_64  a gogdl stand-in asset
//   GET  /gogdl/bad/gogdl_linux_x86_64   the same name, other bytes (a SHA-256
//                                        mismatch against the good one's pin)
//   GET  /__e2e__/gogdl-asset.json       {good:{sha256,size}, bad:{sha256,size}}
//
// Control routes:
//   POST /__e2e__/auth-down | /__e2e__/auth-up   the login page's host drops
//        every connection (the login window's network error) / answers again
//   POST /__e2e__/expire                 from here on a refresh is refused (400)
//
// The request log (one JSON line per request) carries the method, the path
// and whether an Authorization header came along; never a header value, a
// query string or a body.
//
// Standalone (e2e.sh scrapes the MOCK_GOG_URL= line off stdout):
//   node mock-gog.mjs --port 0 [--log <path>] [--fixtures-dir <dir>]

import http from "node:http";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { appendFileSync, readFileSync } from "node:fs";
import { createHash } from "node:crypto";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

export const DEFAULT_LOG_PATH = path.join(__dirname, "..", "last-run-gog-requests.log");
export const FAKE_TOKEN = "FAKE-STORE-TOKEN-not-real";
/** The login code the login page hands out. Not a secret: a fixture. */
export const GOOD_CODE = "FAKE-E2E-GOG-CODE";
/** A code fake gogdl refuses (the "sign-in failed" case). */
export const REFUSED_CODE = "FAKE-E2E-GOG-CODE-refused";
/** GOG Galaxy's public client id, which gogdl uses too. */
export const CLIENT_ID = "46899977096215655";

const GOOD_ASSET = Buffer.from("#!/usr/bin/env python3\n# fake gogdl for GRID's e2e suite\n", "utf8");
const BAD_ASSET = Buffer.from("#!/usr/bin/env python3\n# tampered gogdl for GRID's e2e suite\n", "utf8");
const sha256 = (buf) => createHash("sha256").update(buf).digest("hex");

export function loadFixtures(dir = path.join(__dirname, "..", "fixtures-gog")) {
  return JSON.parse(readFileSync(path.join(dir, "products.json"), "utf8"));
}

function sendJson(res, status, body) {
  const text = JSON.stringify(body);
  res.writeHead(status, { "content-type": "application/json", "content-length": Buffer.byteLength(text) });
  res.end(text);
}

function sendHtml(res, status, html) {
  res.writeHead(status, { "content-type": "text/html; charset=utf-8", "content-length": Buffer.byteLength(html) });
  res.end(html);
}

function readBody(req) {
  return new Promise((resolve) => {
    const chunks = [];
    req.on("data", (c) => chunks.push(c));
    req.on("end", () => resolve(Buffer.concat(chunks)));
    req.on("error", () => resolve(Buffer.alloc(0)));
  });
}

/** The login page: two fields, a button and a link, no script. */
export function loginPage() {
  return `<!doctype html><html><head><meta charset="utf-8"><title>Log in to GOG (mock)</title>
<style>body{font:15px system-ui,sans-serif;margin:24px}label{display:block;margin:12px 0 4px}</style></head>
<body><h1 id="mock-gog-login">Log in to GOG</h1>
<form id="login-form" method="post" action="/auth/login">
<label for="username">E-mail</label><input id="username" name="username" type="text" autocomplete="off">
<label for="password">Password</label><input id="password" name="password" type="password">
<p><button id="login" type="submit">Log in now</button></p>
</form>
<p><a id="forgot" href="/auth/forgot">Forgot your password?</a></p>
</body></html>`;
}

const productById = (fx, id) => fx.products.find((p) => p.id === String(id));
const allIds = (fx) => fx.products.flatMap((p) => [p.id, ...p.dlcs.map((d) => d.id)]);

function apiProduct(p) {
  return {
    id: Number(p.id),
    title: p.title,
    game_type: "game",
    slug: p.slug,
    content_system_compatibility: {
      windows: p.platforms.includes("windows"),
      osx: false,
      linux: p.platforms.includes("linux"),
    },
    languages: Object.fromEntries(p.languages.map((l) => [l, l])),
    images: {
      background: `//images.gog.example/${p.slug}_bg.jpg`,
      logo: `//images.gog.example/${p.slug}_logo.jpg`,
      logo2x: `//images.gog.example/${p.slug}_logo2x.jpg`,
      icon: `//images.gog.example/${p.slug}_icon.png`,
    },
    description: { lead: p.description, full: p.description },
    screenshots: [],
    release_date: `${p.release_date}T00:00:00+0300`,
    expanded_dlcs: p.dlcs.map((d) => ({ id: Number(d.id), title: d.title })),
    dlcs: { products: p.dlcs.map((d) => ({ id: Number(d.id) })) },
    downloads: {
      installers: p.platforms.map((os) => ({
        id: `installer_${os}_en`,
        os,
        language: "en",
        version: p.version,
        total_size: p.download_size,
      })),
    },
  };
}

/** A game's builds, newest first: a fixture with `update_build_id` has a
 *  newer build than the one a fresh install gets (Lanternfall 2.2). */
function builds(p, os) {
  const items = [
    {
      build_id: p.build_id,
      product_id: p.id,
      os,
      branch: null,
      version_name: p.version,
      generation: 2,
      date_published: "2026-01-01T00:00:00+0000",
      link: `/content-system/v2/meta/${p.build_id}`,
    },
  ];
  if (p.update_build_id) {
    items.unshift({
      build_id: p.update_build_id,
      product_id: p.id,
      os,
      branch: null,
      version_name: p.update_version,
      generation: 2,
      date_published: "2026-06-01T00:00:00+0000",
      link: `/content-system/v2/meta/${p.update_build_id}`,
    });
  }
  return { total_count: items.length, count: items.length, items, has_private_branches: false };
}

async function handleRequest(req, res, state) {
  const url = new URL(req.url, "http://mock");
  const p = url.pathname;
  state.log({ method: req.method, path: p, auth: Boolean(req.headers.authorization) });
  const fx = state.fixtures;

  // Control routes.
  if (req.method === "POST" && p === "/__e2e__/auth-down") { state.authDown = true; return sendJson(res, 200, {}); }
  if (req.method === "POST" && p === "/__e2e__/auth-up") { state.authDown = false; return sendJson(res, 200, {}); }
  if (req.method === "POST" && p === "/__e2e__/expire") { state.expired = true; return sendJson(res, 200, {}); }
  if (req.method === "GET" && p === "/__e2e__/gogdl-asset.json") {
    return sendJson(res, 200, {
      good: { sha256: sha256(GOOD_ASSET), size: GOOD_ASSET.length },
      bad: { sha256: sha256(BAD_ASSET), size: BAD_ASSET.length },
    });
  }

  // Login.
  if (req.method === "GET" && (p === "/" || p === "/auth" || p.startsWith("/auth/"))) {
    if (state.authDown) { req.socket.destroy(); return; }
    return sendHtml(res, 200, loginPage());
  }
  if (req.method === "POST" && p === "/auth/login") {
    if (state.authDown) { req.socket.destroy(); return; }
    const form = new URLSearchParams((await readBody(req)).toString("utf8"));
    const user = (form.get("username") ?? "").trim();
    const target = new URL("/on_login_success", `http://${req.headers.host}`);
    target.searchParams.set("origin", "client");
    if (user === "denied") target.searchParams.set("error", "access_denied");
    else target.searchParams.set("code", user === "refused" ? REFUSED_CODE : GOOD_CODE);
    res.writeHead(302, { location: target.toString() });
    res.end();
    return;
  }
  if (req.method === "GET" && p === "/on_login_success") {
    state.successPageLoads += 1;
    return sendHtml(res, 200, "<!doctype html><title>Signed in</title><p id=\"loaded-success\">Signed in</p>");
  }
  if (p === "/token") {
    const query = req.method === "POST" ? new URLSearchParams((await readBody(req)).toString("utf8")) : url.searchParams;
    const grant = query.get("grant_type");
    if ((grant === "authorization_code" && query.get("code") !== GOOD_CODE) || (grant === "refresh_token" && state.expired)) {
      return sendJson(res, 400, { error: "invalid_grant", error_description: "refused by the mock" });
    }
    return sendJson(res, 200, {
      expires_in: 3600,
      scope: "",
      token_type: "bearer",
      access_token: FAKE_TOKEN,
      refresh_token: FAKE_TOKEN,
      user_id: fx.user.user_id,
      session_id: "1",
    });
  }

  // embed.gog.com
  if (p === "/userData.json") {
    return sendJson(res, 200, {
      isLoggedIn: true,
      username: fx.user.username,
      userId: fx.user.user_id,
      galaxyUserId: fx.user.user_id,
      email: fx.user.email,
    });
  }
  if (p === "/user/data/games") return sendJson(res, 200, { owned: allIds(fx).map(Number) });
  if (p === "/account/getFilteredProducts") {
    return sendJson(res, 200, {
      page: 1,
      totalPages: 1,
      totalProducts: fx.products.length,
      productsPerPage: 100,
      products: fx.products.map((g) => ({
        id: Number(g.id),
        title: g.title,
        slug: g.slug,
        image: `//images.gog.example/${g.slug}`,
        url: `/game/${g.slug}`,
        isGame: true,
        category: g.genres[0] ?? "",
        worksOn: { Windows: g.platforms.includes("windows"), Mac: false, Linux: g.platforms.includes("linux") },
        dlcCount: g.dlcs.length,
        releaseDate: { date: `${g.release_date} 00:00:00.000000` },
      })),
    });
  }

  // content-system builds
  let m = p.match(/^\/products\/(\d+)\/os\/(\w+)\/builds$/);
  if (m) {
    const g = productById(fx, m[1]);
    if (!g || !g.platforms.includes(m[2] === "osx" ? "mac" : m[2])) return sendJson(res, 200, { total_count: 0, count: 0, items: [] });
    return sendJson(res, 200, builds(g, m[2]));
  }
  // api.gog.com, a batch: /products?ids=a,b&expand=…
  if (p === "/products") {
    const ids = (url.searchParams.get("ids") ?? "").split(",").filter(Boolean);
    return sendJson(res, 200, ids.map((id) => productById(fx, id)).filter((g) => g && g.id === id).map(apiProduct));
  }
  // api.gog.com
  m = p.match(/^\/products\/(\d+)$/);
  if (m) {
    const g = productById(fx, m[1]);
    return g ? sendJson(res, 200, apiProduct(g)) : sendJson(res, 404, { error: "not_found" });
  }
  m = p.match(/^\/v2\/games\/(\d+)$/);
  if (m) {
    const g = productById(fx, m[1]);
    if (!g) return sendJson(res, 404, { error: "not_found" });
    return sendJson(res, 200, {
      _embedded: {
        product: { id: Number(g.id), title: g.title },
        developers: [{ name: g.developer }],
        publisher: { name: g.publisher },
        tags: g.genres.map((name) => ({ name })),
      },
    });
  }
  // gamesdb
  m = p.match(/^\/platforms\/gog\/external_releases\/(\d+)$/);
  if (m) {
    const g = productById(fx, m[1]);
    if (!g) return sendJson(res, 404, {});
    return sendJson(res, 200, {
      title: { "*": g.title },
      type: "game",
      dlcs_ids: g.dlcs.map((d) => d.id),
      supported_operating_systems: g.platforms.map((slug) => ({ slug })),
      game: {
        title: { "*": g.title },
        summary: { "*": g.description },
        genres: g.genres.map((name) => ({ name: { "*": name } })),
        developers: [{ name: g.developer }],
        publishers: [{ name: g.publisher }],
        first_release_date: `${g.release_date}T00:00:00+0000`,
        vertical_cover: { url_format: `http://${req.headers.host}/art/${g.slug}_cover{formatter}.{ext}` },
        background: { url_format: `http://${req.headers.host}/art/${g.slug}_bg{formatter}.{ext}` },
        screenshots: [],
      },
    });
  }
  if (p.startsWith("/art/")) {
    res.writeHead(200, { "content-type": "image/png" });
    res.end(Buffer.from("89504e470d0a1a0a", "hex"));
    return;
  }
  // galaxy-library
  m = p.match(/^\/users\/(\d+)\/releases$/);
  if (m) {
    return sendJson(res, 200, {
      total_count: fx.products.length,
      items: allIds(fx).map((id) => ({ platform_id: "gog", external_id: id, date_created: 1700000000 })),
    });
  }
  // remote-config
  m = p.match(/^\/components\/galaxy_client\/clients\/(\d+)$/);
  if (m) {
    const cloud = { enabled: true, locations: [{ name: "saves", location: "<?INSTALL?>/saves" }] };
    return sendJson(res, 200, { content: { Windows: { cloudStorage: cloud }, MacOS: { cloudStorage: cloud } } });
  }
  // cloudstorage
  m = p.match(/^\/v1\/(\d+)\/(\d+)(?:\/(.+))?$/);
  if (m) {
    const bucket = `${m[1]}/${m[2]}`;
    const files = state.cloud.get(bucket) ?? new Map();
    state.cloud.set(bucket, files);
    if (!m[3]) {
      return sendJson(res, 200, [...files.entries()].map(([name, f]) => ({ name, hash: f.hash, last_modified: f.at, bytes: f.data.length })));
    }
    const name = decodeURIComponent(m[3]);
    if (req.method === "PUT") {
      const data = await readBody(req);
      // GOG's listing time has no zone suffix.
      files.set(name, { data, hash: createHash("md5").update(data).digest("hex"), at: new Date().toISOString().replace("Z", "") });
      return sendJson(res, 200, {});
    }
    if (req.method === "DELETE") { files.delete(name); return sendJson(res, 200, {}); }
    const f = files.get(name);
    if (!f) return sendJson(res, 404, {});
    res.writeHead(200, { "content-type": "application/octet-stream", "last-modified": new Date(`${f.at}Z`).toUTCString() });
    res.end(f.data);
    return;
  }

  // The tool asset.
  if (p === "/gogdl/good/gogdl_linux_x86_64" || p === "/gogdl/bad/gogdl_linux_x86_64") {
    const body = p.includes("/good/") ? GOOD_ASSET : BAD_ASSET;
    res.writeHead(200, { "content-type": "application/octet-stream", "content-length": body.length });
    res.end(body);
    return;
  }

  sendJson(res, 404, { detail: "not found" });
}

/**
 * Starts the mock GOG server.
 * @returns {Promise<{port:number,url:string,requestLog:Array<object>,state:object,close:()=>Promise<void>}>}
 */
export async function startMockGog({ port = 0, logPath = DEFAULT_LOG_PATH, fixturesDir } = {}) {
  const requestLog = [];
  const state = {
    fixtures: loadFixtures(fixturesDir),
    authDown: false,
    expired: false,
    cloud: new Map(),
    successPageLoads: 0,
    log(entry) {
      requestLog.push(entry);
      if (!logPath) return;
      try {
        appendFileSync(logPath, `${JSON.stringify(entry)}\n`, "utf8");
      } catch {
        // A debugging aid only.
      }
    },
  };
  const server = http.createServer((req, res) => {
    handleRequest(req, res, state).catch(() => {
      try { sendJson(res, 500, { detail: "mock error" }); } catch { /* closed */ }
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
  return {
    port: actualPort,
    url: `http://127.0.0.1:${actualPort}`,
    requestLog,
    state,
    close: () => new Promise((resolve, reject) => {
      server.closeAllConnections?.();
      server.close((err) => (err ? reject(err) : resolve()));
    }),
  };
}

function isMainModule() {
  return process.argv[1] && import.meta.url === `file://${process.argv[1]}`;
}

if (isMainModule()) {
  const args = process.argv.slice(2);
  let port = 0;
  let logPath = DEFAULT_LOG_PATH;
  let fixturesDir;
  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--port" && args[i + 1]) { port = Number(args[++i]); }
    else if (args[i] === "--log" && args[i + 1]) { logPath = path.resolve(args[++i]); }
    else if (args[i] === "--fixtures-dir" && args[i + 1]) { fixturesDir = path.resolve(__dirname, "..", args[++i]); }
  }
  const handle = await startMockGog({ port, logPath, fixturesDir });
  // e2e.sh scrapes this exact line.
  console.log(`MOCK_GOG_URL=${handle.url}`);
  const shutdown = async () => { await handle.close(); process.exit(0); };
  process.on("SIGINT", shutdown);
  process.on("SIGTERM", shutdown);
}
