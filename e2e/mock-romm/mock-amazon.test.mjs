// node --test e2e/mock-romm/mock-amazon.test.mjs
import { test } from "node:test";
import assert from "node:assert/strict";

import { startMockAmazon, GOOD_CODE, REFUSED_CODE, SIGNIN_PATH, ASSOC_HANDLE, CODE_PARAM } from "./mock-amazon.mjs";

async function withMock(fn) {
  const mock = await startMockAmazon({ port: 0, logPath: null });
  try {
    await fn(mock);
  } finally {
    await mock.close();
  }
}

const login = (mock, username) =>
  fetch(`${mock.url}${SIGNIN_PATH}`, {
    method: "POST",
    redirect: "manual",
    headers: { "content-type": "application/x-www-form-urlencoded" },
    body: new URLSearchParams({ username, password: "pw" }).toString(),
  });

const state = async (mock) => (await fetch(`${mock.url}/__e2e__/state`)).json();

test("the sign-in page posts to Amazon's finish redirect with a code", async () => {
  await withMock(async (mock) => {
    const page = await (await fetch(`${mock.url}${SIGNIN_PATH}?openid.assoc_handle=amzn_sonic_games_launcher`)).text();
    assert.match(page, /id="username"/);
    assert.match(page, /id="password"/);
    assert.match(page, /id="home" href="\/"/);
    const ok = await login(mock, "sam");
    assert.equal(ok.status, 302);
    const target = new URL(ok.headers.get("location"));
    assert.equal(target.origin, mock.url);
    assert.equal(target.pathname, "/");
    assert.equal(target.searchParams.get(ASSOC_HANDLE[0]), ASSOC_HANDLE[1]);
    assert.equal(target.searchParams.get(CODE_PARAM), GOOD_CODE);
    const refused = new URL((await login(mock, "refused")).headers.get("location"));
    assert.equal(refused.searchParams.get(CODE_PARAM), REFUSED_CODE);
    const denied = new URL((await login(mock, "denied")).headers.get("location"));
    assert.equal(denied.searchParams.get(CODE_PARAM), null);
    assert.equal(denied.searchParams.get(ASSOC_HANDLE[0]), ASSOC_HANDLE[1]);
  });
});

test("the finish counts every load; the home page without the assoc_handle is ordinary", async () => {
  await withMock(async (mock) => {
    assert.deepEqual(await state(mock), { finishLoads: 0, homeLoads: 0 });
    const home = await (await fetch(`${mock.url}/`)).text();
    assert.match(home, /id="mock-amazon-home"/);
    await fetch(`${mock.url}/?openid.assoc_handle=other`);
    assert.deepEqual(await state(mock), { finishLoads: 0, homeLoads: 2 });
    await fetch(`${mock.url}/?openid.assoc_handle=amzn_sonic_games_launcher&${CODE_PARAM}=${GOOD_CODE}`);
    assert.equal((await state(mock)).finishLoads, 1);
  });
});

test("the request log never holds a query string or a body", async () => {
  await withMock(async (mock) => {
    await login(mock, "sam");
    await fetch(`${mock.url}/?openid.assoc_handle=amzn_sonic_games_launcher&${CODE_PARAM}=${GOOD_CODE}`);
    const text = JSON.stringify(mock.requestLog);
    assert.doesNotMatch(text, new RegExp(GOOD_CODE));
    assert.doesNotMatch(text, /assoc_handle/);
    assert.doesNotMatch(text, /pw/);
    assert.ok(mock.requestLog.some((r) => r.path === "/" && r.method === "GET"));
  });
});

test("art is an image; the page host can go down and come back", async () => {
  await withMock(async (mock) => {
    const art = await fetch(`${mock.url}/art/fogline-icon.jpg`);
    assert.equal(art.status, 200);
    assert.equal(art.headers.get("content-type"), "image/png");
    await fetch(`${mock.url}/__e2e__/login-down`, { method: "POST" });
    await assert.rejects(fetch(`${mock.url}${SIGNIN_PATH}`));
    await fetch(`${mock.url}/__e2e__/login-up`, { method: "POST" });
    assert.equal((await fetch(`${mock.url}${SIGNIN_PATH}`)).status, 200);
  });
});
