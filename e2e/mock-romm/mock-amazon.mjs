// Mock Amazon for the E2E harness (docs/stores-phase3-plan.md "E2E plan").
//
// Plain node:http, ESM, no npm dependencies. One server stands in for every
// Amazon address GRID's login window and art cache use: an e2e build of the
// app reads GRID_LAUNCHER_E2E_AMAZON_BASE (stores_service/amazon.rs
// `amazon_endpoints`, grid-core `AmazonEndpoints::e2e`), so the sign-in
// page nile's login URL names, the finish redirect and the art all live at
// this server's root. The fake nile (fake-tools/nile.mjs) prints the login
// URL (`<base>/ap/signin?…`) and owns the owned list; nothing here sees a
// token. No real account: the only code is the fixture FAKE-E2E-AMAZON-CODE.
//
// Routes:
//   GET  /ap/signin…                     the sign-in page (a form; GRID's
//                                        login window opens nile's URL)
//   POST /ap/signin                      302 to /?openid.assoc_handle=amzn_sonic_games_launcher
//                                              &openid.oa2.authorization_code=FAKE-E2E-AMAZON-CODE
//                                        user "denied"  → the same redirect with no code
//                                        user "refused" → a code the fake nile refuses
//   GET  /  with openid.assoc_handle=amzn_sonic_games_launcher
//                                        the finish: must NEVER be served (GRID
//                                        refuses the navigation; amazon-login
//                                        reads the count)
//   GET  /  without it                   Amazon's home page: an ordinary page the
//                                        login window must load (the sign-in
//                                        page links to it, as Amazon's logo does)
//   GET  /art/<name>                     a tiny image (library.json's art)
//
// Control routes:
//   POST /__e2e__/login-down | /__e2e__/login-up   the sign-in page's host
//        drops every connection (the login window's network error) / answers
//   GET  /__e2e__/state                  {finishLoads, homeLoads}
//
// The request log (one JSON line per request) carries the method and the
// path; never a header, a query string or a body.
//
// Standalone (e2e.sh scrapes the MOCK_AMAZON_URL= line off stdout):
//   node mock-amazon.mjs --port 0 [--log <path>]

import http from "node:http";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { appendFileSync } from "node:fs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

export const DEFAULT_LOG_PATH = path.join(__dirname, "..", "last-run-amazon-requests.log");
/** The sign-in code the page hands out. Not a secret: a fixture. */
export const GOOD_CODE = "FAKE-E2E-AMAZON-CODE";
/** A code the fake nile refuses (the "sign-in failed" case). */
export const REFUSED_CODE = "FAKE-E2E-AMAZON-CODE-refused";
/** The sign-in page's path (nile's login URL). */
export const SIGNIN_PATH = "/ap/signin";
/** The query pair that marks Amazon's finish redirect. */
export const ASSOC_HANDLE = ["openid.assoc_handle", "amzn_sonic_games_launcher"];
/** The finish redirect's code parameter. */
export const CODE_PARAM = "openid.oa2.authorization_code";

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

/** The sign-in page: two fields, a button and a link home, no script. */
export function loginPage() {
  return `<!doctype html><html><head><meta charset="utf-8"><title>Amazon Sign-In (mock)</title>
<style>body{font:15px system-ui,sans-serif;margin:24px}label{display:block;margin:12px 0 4px}</style></head>
<body><p><a id="home" href="/">amazon.com</a></p>
<h1 id="mock-amazon-login">Sign in</h1>
<form id="login-form" method="post" action="${SIGNIN_PATH}">
<label for="username">Email or mobile phone number</label><input id="username" name="username" type="text" autocomplete="off">
<label for="password">Password</label><input id="password" name="password" type="password">
<p><button id="login" type="submit">Sign in</button></p>
</form>
</body></html>`;
}

/** Amazon's home page (the same path as the finish, without its query). */
export function homePage() {
  return `<!doctype html><html><head><meta charset="utf-8"><title>Amazon.com (mock)</title></head>
<body><h1 id="mock-amazon-home">Amazon.com</h1><p><a id="signin" href="${SIGNIN_PATH}">Sign in</a></p></body></html>`;
}

/** Whether `url` (a URL) is Amazon's finish redirect. */
export function isFinish(url) {
  return url.pathname === "/" && url.searchParams.get(ASSOC_HANDLE[0]) === ASSOC_HANDLE[1];
}

async function handleRequest(req, res, state) {
  const url = new URL(req.url, "http://mock");
  const p = url.pathname;
  state.log({ method: req.method, path: p });

  if (req.method === "POST" && p === "/__e2e__/login-down") { state.loginDown = true; return sendJson(res, 200, {}); }
  if (req.method === "POST" && p === "/__e2e__/login-up") { state.loginDown = false; return sendJson(res, 200, {}); }
  if (req.method === "GET" && p === "/__e2e__/state") {
    return sendJson(res, 200, { finishLoads: state.finishLoads, homeLoads: state.homeLoads });
  }

  if (req.method === "GET" && p === SIGNIN_PATH) {
    if (state.loginDown) { req.socket.destroy(); return; }
    return sendHtml(res, 200, loginPage());
  }
  if (req.method === "POST" && p === SIGNIN_PATH) {
    if (state.loginDown) { req.socket.destroy(); return; }
    const form = new URLSearchParams((await readBody(req)).toString("utf8"));
    const user = (form.get("username") ?? "").trim();
    const host = req.headers.host ?? "127.0.0.1";
    const target = new URL("/", `http://${host}`);
    target.searchParams.set(ASSOC_HANDLE[0], ASSOC_HANDLE[1]);
    target.searchParams.set("openid.mode", "id_res");
    if (user !== "denied") target.searchParams.set(CODE_PARAM, user === "refused" ? REFUSED_CODE : GOOD_CODE);
    res.writeHead(302, { location: target.toString() });
    res.end();
    return;
  }
  if (req.method === "GET" && p === "/") {
    if (isFinish(url)) {
      state.finishLoads += 1;
      return sendHtml(res, 200, '<!doctype html><title>Finished</title><p id="loaded-finish">Finished</p>');
    }
    state.homeLoads += 1;
    return sendHtml(res, 200, homePage());
  }
  if (req.method === "GET" && p.startsWith("/art/")) {
    res.writeHead(200, { "content-type": "image/png", "content-length": PIXEL.length });
    res.end(PIXEL);
    return;
  }
  sendJson(res, 404, { detail: "not found" });
}

/**
 * Starts the mock Amazon server.
 * @returns {Promise<{port:number,url:string,requestLog:Array<object>,state:object,close:()=>Promise<void>}>}
 */
export async function startMockAmazon({ port = 0, logPath = DEFAULT_LOG_PATH } = {}) {
  const requestLog = [];
  const state = {
    loginDown: false,
    finishLoads: 0,
    homeLoads: 0,
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
  const handle = await startMockAmazon({ port, logPath });
  // e2e.sh scrapes this exact line.
  console.log(`MOCK_AMAZON_URL=${handle.url}`);
  const shutdown = async () => { await handle.close(); process.exit(0); };
  process.on("SIGINT", shutdown);
  process.on("SIGTERM", shutdown);
}
