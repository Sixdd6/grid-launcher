// node --test e2e/mock-romm/mock-epic.test.mjs
import { test } from "node:test";
import assert from "node:assert/strict";

import { startMockEpic, GOOD_CODE, REFUSED_CODE, FINISH_PATH } from "./mock-epic.mjs";

async function withMock(fn) {
  const mock = await startMockEpic({ port: 0, logPath: null });
  try {
    await fn(mock);
  } finally {
    await mock.close();
  }
}

const login = (mock, username) =>
  fetch(`${mock.url}/id/login`, {
    method: "POST",
    redirect: "manual",
    headers: { "content-type": "application/x-www-form-urlencoded" },
    body: new URLSearchParams({ username, password: "pw" }).toString(),
  });

test("the sign-in page posts to the launcher redirect with a code", async () => {
  await withMock(async (mock) => {
    const page = await (await fetch(`${mock.url}/id/login?responseType=code`)).text();
    assert.match(page, /id="username"/);
    assert.match(page, /id="password"/);
    const ok = await login(mock, "sam");
    assert.equal(ok.status, 302);
    const target = new URL(ok.headers.get("location"));
    assert.equal(target.origin, mock.url);
    assert.equal(target.pathname, FINISH_PATH);
    assert.equal(target.searchParams.get("code"), GOOD_CODE);
    const refused = new URL((await login(mock, "refused")).headers.get("location"));
    assert.equal(refused.searchParams.get("code"), REFUSED_CODE);
    const denied = new URL((await login(mock, "denied")).headers.get("location"));
    assert.equal(denied.searchParams.get("code"), null);
    assert.equal(denied.searchParams.get("error"), "access_denied");
    const local = new URL((await login(mock, "local")).headers.get("location"));
    assert.equal(local.hostname, "localhost");
    assert.equal(Number(local.port), mock.port + 1);
  });
});

test("the finish page counts every load (GRID must never let it load)", async () => {
  await withMock(async (mock) => {
    const state = async () => (await fetch(`${mock.url}/__e2e__/state`)).json();
    assert.equal((await state()).authorizedLoads, 0);
    await fetch(`${mock.url}${FINISH_PATH}?code=${GOOD_CODE}`);
    assert.equal((await state()).authorizedLoads, 1);
    await fetch(`${mock.url}/id/login?popup=1`);
    assert.equal((await state()).popupLoads, 1);
  });
});

test("the request log never holds a query string or a body", async () => {
  await withMock(async (mock) => {
    await login(mock, "sam");
    await fetch(`${mock.url}${FINISH_PATH}?code=${GOOD_CODE}`);
    const text = JSON.stringify(mock.requestLog);
    assert.doesNotMatch(text, new RegExp(GOOD_CODE));
    assert.doesNotMatch(text, /pw/);
    assert.ok(mock.requestLog.some((r) => r.path === FINISH_PATH));
  });
});

test("art is an image; the page host can go down and come back", async () => {
  await withMock(async (mock) => {
    const art = await fetch(`${mock.url}/art/Starlight_tall.jpg`);
    assert.equal(art.status, 200);
    assert.equal(art.headers.get("content-type"), "image/png");
    await fetch(`${mock.url}/__e2e__/login-down`, { method: "POST" });
    await assert.rejects(fetch(`${mock.url}/id/login`));
    await fetch(`${mock.url}/__e2e__/login-up`, { method: "POST" });
    assert.equal((await fetch(`${mock.url}/id/login`)).status, 200);
    assert.match(await (await fetch(`${mock.url}/id/popup`)).text(), /window\.open/);
  });
});
