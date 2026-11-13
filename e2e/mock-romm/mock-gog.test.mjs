// node --test e2e/mock-romm/mock-gog.test.mjs
import { test } from "node:test";
import assert from "node:assert/strict";

import { startMockGog, GOOD_CODE, REFUSED_CODE, FAKE_TOKEN } from "./mock-gog.mjs";

async function withMock(fn) {
  const mock = await startMockGog({ port: 0, logPath: null });
  try {
    await fn(mock);
  } finally {
    await mock.close();
  }
}

const login = (mock, username) =>
  fetch(`${mock.url}/auth/login`, {
    method: "POST",
    redirect: "manual",
    headers: { "content-type": "application/x-www-form-urlencoded" },
    body: new URLSearchParams({ username, password: "pw" }).toString(),
  });

test("the login page posts to a redirect with a code", async () => {
  await withMock(async (mock) => {
    const page = await (await fetch(`${mock.url}/auth?client_id=1`)).text();
    assert.match(page, /id="username"/);
    assert.match(page, /id="password"/);
    const ok = await login(mock, "sam");
    assert.equal(ok.status, 302);
    const target = new URL(ok.headers.get("location"));
    assert.equal(target.pathname, "/on_login_success");
    assert.equal(target.searchParams.get("code"), GOOD_CODE);
    const refused = new URL((await login(mock, "refused")).headers.get("location"));
    assert.equal(refused.searchParams.get("code"), REFUSED_CODE);
    const denied = new URL((await login(mock, "denied")).headers.get("location"));
    assert.equal(denied.searchParams.get("code"), null);
    assert.equal(denied.searchParams.get("error"), "access_denied");
  });
});

test("the token route trades the good code only, and refresh stops after expire", async () => {
  await withMock(async (mock) => {
    const good = await fetch(`${mock.url}/token?grant_type=authorization_code&code=${GOOD_CODE}`);
    assert.equal((await good.json()).access_token, FAKE_TOKEN);
    const bad = await fetch(`${mock.url}/token?grant_type=authorization_code&code=${REFUSED_CODE}`);
    assert.equal(bad.status, 400);
    assert.equal((await fetch(`${mock.url}/token?grant_type=refresh_token`)).status, 200);
    await fetch(`${mock.url}/__e2e__/expire`, { method: "POST" });
    assert.equal((await fetch(`${mock.url}/token?grant_type=refresh_token`)).status, 400);
  });
});

test("the owned list, products and builds come from the fixtures", async () => {
  await withMock(async (mock) => {
    const owned = await (await fetch(`${mock.url}/account/getFilteredProducts?mediaType=1&page=1`)).json();
    assert.deepEqual(owned.products.map((p) => p.title), ["Ashen Tide", "Lanternfall", "Grey Meridian"]);
    const product = await (await fetch(`${mock.url}/products/1453375253?expand=downloads`)).json();
    assert.equal(product.expanded_dlcs[0].title, "Lanternfall: Ember Roads");
    const builds = await (await fetch(`${mock.url}/products/1453375253/os/linux/builds?generation=2`)).json();
    assert.equal(builds.items[0].version_name, "2.2");
    const none = await (await fetch(`${mock.url}/products/1580232252/os/linux/builds?generation=2`)).json();
    assert.equal(none.count, 0);
  });
});

test("auth-down drops the login page's connections", async () => {
  await withMock(async (mock) => {
    await fetch(`${mock.url}/__e2e__/auth-down`, { method: "POST" });
    await assert.rejects(fetch(`${mock.url}/auth`));
    await fetch(`${mock.url}/__e2e__/auth-up`, { method: "POST" });
    assert.equal((await fetch(`${mock.url}/auth`)).status, 200);
  });
});

test("the two gogdl assets differ and say their SHA-256", async () => {
  await withMock(async (mock) => {
    const info = await (await fetch(`${mock.url}/__e2e__/gogdl-asset.json`)).json();
    assert.notEqual(info.good.sha256, info.bad.sha256);
    const good = Buffer.from(await (await fetch(`${mock.url}/gogdl/good/gogdl_linux_x86_64`)).arrayBuffer());
    assert.equal(good.length, info.good.size);
  });
});

test("cloud storage keeps what is put", async () => {
  await withMock(async (mock) => {
    const base = `${mock.url}/v1/48628349957132247/51154268886064420`;
    assert.deepEqual(await (await fetch(base)).json(), []);
    await fetch(`${base}/saves/slot1.sav`, { method: "PUT", body: "abc" });
    const list = await (await fetch(base)).json();
    assert.equal(list[0].name, "saves/slot1.sav");
    assert.equal(await (await fetch(`${base}/saves/slot1.sav`)).text(), "abc");
  });
});

test("the request log never records a query", async () => {
  await withMock(async (mock) => {
    await fetch(`${mock.url}/token?grant_type=authorization_code&code=${GOOD_CODE}`, {
      headers: { authorization: `Bearer ${FAKE_TOKEN}` },
    });
    const text = JSON.stringify(mock.requestLog);
    assert.ok(!text.includes(GOOD_CODE));
    assert.ok(!text.includes(FAKE_TOKEN));
    assert.equal(mock.requestLog[0].auth, true);
  });
});
