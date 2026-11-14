// Mock Steam Web API for the E2E harness (docs/stores-phase4-plan.md
// package C "E2E harness").
//
// Plain node:http, ESM, no npm dependencies. One server stands in for every
// Steam address GRID uses: an e2e build of the app reads
// GRID_LAUNCHER_E2E_STEAM_BASE (stores_service/steam.rs `steam_endpoints`,
// grid-core `SteamEndpoints::e2e`), so the Web API lives at this server's
// root and the art at `<base>/art/`. No real account and no real key: the
// only good key is the fixture FAKE-STEAM-KEY-not-real.
//
// Routes (data from e2e/fixtures-steam/games.json):
//   GET /IPlayerService/GetOwnedGames/v1/?key&steamid&…
//       key FAKE-STEAM-KEY-not-real          the owned list for steamid:
//                                            …0001 the fixture games, …0002
//                                            {"response": {}} (private game
//                                            details), any other {"game_count": 0}
//       key FAKE-STEAM-KEY-not-real-revoked  401 with Steam's HTML
//       any other key, or none               401 with Steam's HTML
//   GET /ISteamUser/GetPlayerSummaries/v2/?key&steamids
//       a bad key 403; …0009 an empty `players` list; else the persona
//   GET /IStoreBrowseService/GetItems/v1/?input_json={"ids":[{"appid":N}…]…}
//       no key needed; one item per asked id, in order; the tool ids and
//       unknown ids answer `success: 15` with `appid: 0`, as Steam does
//   GET /art/…                               a tiny image (covers, screenshots)
//
// It refuses (400) any request that carries a Referer or a Cookie, and any
// request for /redirected (a client that followed a redirect).
//
// Control routes:
//   POST /__e2e__/fail | /__e2e__/up     every API route answers 503 (no
//                                        network for GRID) / answers again
//   POST /__e2e__/redirect | /__e2e__/no-redirect
//                                        GetOwnedGames answers 302 to /redirected
//   GET  /__e2e__/requests               the request log (below), as JSON
//
// The request log (one JSON line per request but the control routes, also
// kept in memory) carries
// the method, the path and what the key was (`good`, `revoked`, `other` or
// `null`), never the key, a query string, a header value or a body; plus
// whether a Referer or Cookie came and whether the request was refused.
//
// Standalone (e2e.sh scrapes the MOCK_STEAM_URL= line off stdout):
//   node mock-steam.mjs --port 0 [--log <path>]

import http from "node:http";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { appendFileSync, readFileSync } from "node:fs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

export const DEFAULT_LOG_PATH = path.join(__dirname, "..", "last-run-steam-requests.log");
export const FIXTURES_PATH = path.join(__dirname, "..", "fixtures-steam", "games.json");
/** The good key. A fixture, not a secret. */
export const GOOD_KEY = "FAKE-STEAM-KEY-not-real";
/** A key Steam revoked (401). */
export const REVOKED_KEY = `${GOOD_KEY}-revoked`;
export const SAM = "76561198000000001";
export const PRIVATE_ID = "76561198000000002";
export const NO_SUCH_ID = "76561198000000009";

export const OWNED_PATH = "/IPlayerService/GetOwnedGames/v1/";
export const SUMMARIES_PATH = "/ISteamUser/GetPlayerSummaries/v2/";
export const ITEMS_PATH = "/IStoreBrowseService/GetItems/v1/";

// A 1x1 PNG.
const PIXEL = Buffer.from(
  "89504e470d0a1a0a0000000d4948445200000001000000010806000000" +
    "1f15c4890000000d49444154789c6360000002000154a24f5d0000000049454e44ae426082",
  "hex",
);

/** Steam's own words for a refused key (HTML; GRID reads only the status). */
const DENIED = (status) =>
  `<html><head><title>${status === 401 ? "Unauthorized" : "Forbidden"}</title></head><body><h1>${status === 401 ? "Unauthorized" : "Forbidden"}</h1>Access is denied. Retrying will not help. Please verify your <pre>key=</pre> parameter.</body></html>`;

export function loadFixtures(file = FIXTURES_PATH) {
  return JSON.parse(readFileSync(file, "utf8"));
}

/** What a request's key was, never the key itself. */
export function keyKind(key) {
  if (key === null || key === undefined || key === "") return null;
  if (key === GOOD_KEY) return "good";
  if (key === REVOKED_KEY) return "revoked";
  return "other";
}

function sendJson(res, status, body) {
  const text = JSON.stringify(body);
  res.writeHead(status, { "content-type": "application/json; charset=UTF-8", "content-length": Buffer.byteLength(text) });
  res.end(text);
}

function sendHtml(res, status, html) {
  res.writeHead(status, { "content-type": "text/html; charset=UTF-8", "content-length": Buffer.byteLength(html) });
  res.end(html);
}

/** One GetItems store item, in Steam's shape. */
export function storeItem(fixtures, appid) {
  const app = fixtures.apps[String(appid)];
  if (!app || fixtures.tools.includes(appid)) return { appid: 0, success: 15 };
  return {
    item_type: 0,
    id: appid,
    success: 1,
    visible: true,
    name: app.name,
    store_url_path: `app/${appid}/${app.name.replace(/[^A-Za-z0-9]+/g, "_")}`,
    appid,
    type: app.type,
    basic_info: {
      short_description: app.description,
      publishers: [{ name: app.developer }],
      developers: [{ name: app.developer }],
    },
    release: { steam_release_date: app.release },
    assets: {
      asset_url_format: `steam/apps/${appid}/\${FILENAME}?t=1731000000`,
      main_capsule: "capsule_616x353.jpg",
      header: "header.jpg",
      library_capsule: "library_600x900.jpg",
      library_hero: "library_hero.jpg",
    },
    screenshots: {
      all_ages_screenshots: [
        { filename: `steam/apps/${appid}/ss_1.1920x1080.jpg`, ordinal: 0 },
        { filename: `steam/apps/${appid}/ss_2.1920x1080.jpg`, ordinal: 1 },
      ],
    },
  };
}

/** The ids GetItems asked about, in order (`input_json`). */
export function askedIds(url) {
  try {
    const input = JSON.parse(url.searchParams.get("input_json") ?? "{}");
    return (input.ids ?? []).map((entry) => Number(entry.appid)).filter((id) => Number.isFinite(id));
  } catch {
    return [];
  }
}

function handleRequest(req, res, state) {
  const url = new URL(req.url, "http://mock");
  const p = url.pathname;
  const referer = req.headers.referer !== undefined;
  const cookie = req.headers.cookie !== undefined;
  const entry = { method: req.method, path: p, key: keyKind(url.searchParams.get("key")), referer, cookie, refused: false };
  if (p === ITEMS_PATH) entry.ids = askedIds(url).length;
  if (!p.startsWith("/__e2e__/")) state.log(entry);

  if (req.method === "POST" && p === "/__e2e__/fail") { state.failing = true; return sendJson(res, 200, {}); }
  if (req.method === "POST" && p === "/__e2e__/up") { state.failing = false; return sendJson(res, 200, {}); }
  if (req.method === "POST" && p === "/__e2e__/redirect") { state.redirect = true; return sendJson(res, 200, {}); }
  if (req.method === "POST" && p === "/__e2e__/no-redirect") { state.redirect = false; return sendJson(res, 200, {}); }
  if (req.method === "GET" && p === "/__e2e__/requests") return sendJson(res, 200, state.requestLog);

  if (referer || cookie || p === "/redirected") {
    entry.refused = true;
    return sendJson(res, 400, { error: "refused: a referer, a cookie or a followed redirect" });
  }
  if (req.method === "GET" && p.startsWith("/art/")) {
    res.writeHead(200, { "content-type": "image/png", "content-length": PIXEL.length });
    res.end(PIXEL);
    return;
  }
  const api = p === OWNED_PATH || p === SUMMARIES_PATH || p === ITEMS_PATH;
  if (api && state.failing) return sendHtml(res, 503, "<html><body>Service Unavailable</body></html>");

  if (req.method === "GET" && p === OWNED_PATH) {
    const key = entry.key;
    if (key !== "good") return sendHtml(res, 401, DENIED(401));
    if (state.redirect) {
      res.writeHead(302, { location: "/redirected" });
      res.end();
      return;
    }
    const id = url.searchParams.get("steamid") ?? "";
    if (id === PRIVATE_ID) return sendJson(res, 200, { response: {} });
    const games = state.fixtures.owned[id] ?? [];
    const withInfo = url.searchParams.get("include_appinfo") === "1" || url.searchParams.get("include_appinfo") === "true";
    return sendJson(res, 200, {
      response: {
        game_count: games.length,
        games: games.map((g) => (withInfo ? g : { appid: g.appid, playtime_forever: g.playtime_forever, rtime_last_played: g.rtime_last_played })),
      },
    });
  }
  if (req.method === "GET" && p === SUMMARIES_PATH) {
    if (entry.key !== "good") return sendHtml(res, 403, DENIED(403));
    const id = (url.searchParams.get("steamids") ?? "").split(",")[0];
    if (id === NO_SUCH_ID) return sendJson(res, 200, { response: { players: [] } });
    const persona = state.fixtures.personas[id] ?? "Steam Player";
    return sendJson(res, 200, {
      response: {
        players: [{ steamid: id, communityvisibilitystate: 3, profilestate: 1, personaname: persona, profileurl: `https://steamcommunity.com/profiles/${id}/` }],
      },
    });
  }
  if (req.method === "GET" && p === ITEMS_PATH) {
    const items = askedIds(url).map((id) => storeItem(state.fixtures, id));
    return sendJson(res, 200, { response: { store_items: items } });
  }
  sendHtml(res, 404, "<html><body>Not Found</body></html>");
}

/**
 * Starts the mock Steam server.
 * @returns {Promise<{port:number,url:string,requestLog:Array<object>,state:object,close:()=>Promise<void>}>}
 */
export async function startMockSteam({ port = 0, logPath = DEFAULT_LOG_PATH, fixtures = loadFixtures() } = {}) {
  const requestLog = [];
  const state = {
    fixtures,
    failing: false,
    redirect: false,
    requestLog,
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
    try {
      handleRequest(req, res, state);
    } catch {
      try { sendJson(res, 500, { error: "mock error" }); } catch { /* closed */ }
    }
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
  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--port" && args[i + 1]) { port = Number(args[++i]); }
    else if (args[i] === "--log" && args[i + 1]) { logPath = path.resolve(args[++i]); }
  }
  const handle = await startMockSteam({ port, logPath });
  // e2e.sh scrapes this exact line.
  console.log(`MOCK_STEAM_URL=${handle.url}`);
  const shutdown = async () => { await handle.close(); process.exit(0); };
  process.on("SIGINT", shutdown);
  process.on("SIGTERM", shutdown);
}
