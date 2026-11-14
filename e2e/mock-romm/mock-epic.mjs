// Mock Epic for the E2E harness (docs/stores-phase2-plan.md "E2E plan").
//
// Plain node:http, ESM, no npm dependencies. One server stands in for every
// Epic address GRID's login window and art cache use: an e2e build of the
// app reads GRID_LAUNCHER_E2E_EPIC_BASE (stores_service/epic.rs
// `epic_endpoints`, grid-core `EpicEndpoints::e2e`), so the sign-in page,
// the finish redirect and the art all live at this server's root. The
// owned games come from the fake legendary (fake-tools/legendary.mjs);
// nothing here sees a token. No real account: the only code is the fixture
// FAKE-E2E-EPIC-CODE.
//
// Routes:
//   GET  /  and  /id/login…              the sign-in page (a form; GRID's
//                                        login window opens it)
//   POST /id/login                       302 to /launcher/authorized?code=FAKE-E2E-EPIC-CODE
//                                        user "denied"  → …?error=access_denied (no code)
//                                        user "refused" → a code the fake legendary refuses
//                                        user "local"   → 302 to http://localhost:<port+1>/
//                                                          (a loopback page GRID must refuse)
//   GET  /launcher/authorized            must NEVER be served: GRID refuses the
//                                        navigation (epic-login reads the count)
//   GET  /id/popup                       a page whose button window.open()s
//                                        /id/login?popup=1 (a third-party sign-in)
//   GET  /art/<name>                     a tiny image (keyImages point here)
//
// Control routes:
//   POST /__e2e__/login-down | /__e2e__/login-up   the sign-in page's host
//        drops every connection (the login window's network error) / answers
//   GET  /__e2e__/state                  {authorizedLoads, popupLoads}
//
// The request log (one JSON line per request) carries the method and the
// path; never a header, a query string or a body.
//
// Standalone (e2e.sh scrapes the MOCK_EPIC_URL= line off stdout):
//   node mock-epic.mjs --port 0 [--log <path>]

import http from "node:http";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { appendFileSync } from "node:fs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

export const DEFAULT_LOG_PATH = path.join(__dirname, "..", "last-run-epic-requests.log");
/** The sign-in code the page hands out. Not a secret: a fixture. */
export const GOOD_CODE = "FAKE-E2E-EPIC-CODE";
/** A code the fake legendary refuses (the "sign-in failed" case). */
export const REFUSED_CODE = "FAKE-E2E-EPIC-CODE-refused";
/** The finish path GRID's login window catches and refuses. */
export const FINISH_PATH = "/launcher/authorized";

// A 1x1 PNG.
const PIXEL = Buffer.from(
  "89504e470d0a1a0a0000000d4948445200000001000000010806000000" +
    "1f15c4890000000d49444154789c6360000002000154a24f5d0000000049454e44ae426082",
  "hex",
);

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

/** The sign-in page: two fields, a button and a link, no script. */
export function loginPage() {
  return `<!doctype html><html><head><meta charset="utf-8"><title>Sign in to Epic Games (mock)</title>
<style>body{font:15px system-ui,sans-serif;margin:24px}label{display:block;margin:12px 0 4px}</style></head>
<body><h1 id="mock-epic-login">Sign in to Epic Games</h1>
<form id="login-form" method="post" action="/id/login">
<label for="username">Email</label><input id="username" name="username" type="text" autocomplete="off">
<label for="password">Password</label><input id="password" name="password" type="password">
<p><button id="login" type="submit">Sign in</button></p>
</form>
<p><a id="popup" href="/id/popup">Sign in with another account</a></p>
</body></html>`;
}

/** A page that opens a pop-up, as a third-party sign-in button does. */
export function popupPage() {
  return `<!doctype html><html><head><meta charset="utf-8"><title>Other accounts (mock)</title></head>
<body><button id="open-popup" onclick="window.open('/id/login?popup=1','signin','width=400,height=600')">Sign in with Google</button></body></html>`;
}

async function handleRequest(req, res, state) {
  const url = new URL(req.url, "http://mock");
  const p = url.pathname;
  state.log({ method: req.method, path: p });

  if (req.method === "POST" && p === "/__e2e__/login-down") { state.loginDown = true; return sendJson(res, 200, {}); }
  if (req.method === "POST" && p === "/__e2e__/login-up") { state.loginDown = false; return sendJson(res, 200, {}); }
  if (req.method === "GET" && p === "/__e2e__/state") {
    return sendJson(res, 200, { authorizedLoads: state.authorizedLoads, popupLoads: state.popupLoads });
  }

  if (req.method === "GET" && (p === "/" || p === "/id/login")) {
    if (state.loginDown) { req.socket.destroy(); return; }
    if (url.searchParams.get("popup") === "1") state.popupLoads += 1;
    return sendHtml(res, 200, loginPage());
  }
  if (req.method === "GET" && p === "/id/popup") return sendHtml(res, 200, popupPage());
  if (req.method === "POST" && p === "/id/login") {
    if (state.loginDown) { req.socket.destroy(); return; }
    const form = new URLSearchParams((await readBody(req)).toString("utf8"));
    const user = (form.get("username") ?? "").trim();
    const host = req.headers.host ?? "127.0.0.1";
    if (user === "local") {
      const port = Number(host.split(":")[1] ?? 80) + 1;
      res.writeHead(302, { location: `http://localhost:${port}/` });
      res.end();
      return;
    }
    const target = new URL(FINISH_PATH, `http://${host}`);
    if (user === "denied") target.searchParams.set("error", "access_denied");
    else target.searchParams.set("code", user === "refused" ? REFUSED_CODE : GOOD_CODE);
    res.writeHead(302, { location: target.toString() });
    res.end();
    return;
  }
  if (p === FINISH_PATH) {
    state.authorizedLoads += 1;
    return sendHtml(res, 200, '<!doctype html><title>Authorized</title><p id="loaded-authorized">Authorized</p>');
  }
  if (req.method === "GET" && p.startsWith("/art/")) {
    res.writeHead(200, { "content-type": "image/png", "content-length": PIXEL.length });
    res.end(PIXEL);
    return;
  }
  sendJson(res, 404, { detail: "not found" });
}

/**
 * Starts the mock Epic server.
 * @returns {Promise<{port:number,url:string,requestLog:Array<object>,state:object,close:()=>Promise<void>}>}
 */
export async function startMockEpic({ port = 0, logPath = DEFAULT_LOG_PATH } = {}) {
  const requestLog = [];
  const state = {
    loginDown: false,
    authorizedLoads: 0,
    popupLoads: 0,
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
  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--port" && args[i + 1]) { port = Number(args[++i]); }
    else if (args[i] === "--log" && args[i + 1]) { logPath = path.resolve(args[++i]); }
  }
  const handle = await startMockEpic({ port, logPath });
  // e2e.sh scrapes this exact line.
  console.log(`MOCK_EPIC_URL=${handle.url}`);
  const shutdown = async () => { await handle.close(); process.exit(0); };
  process.on("SIGINT", shutdown);
  process.on("SIGTERM", shutdown);
}
