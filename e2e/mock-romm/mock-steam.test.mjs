// node --test e2e/mock-romm/mock-steam.test.mjs
import { test } from "node:test";
import assert from "node:assert/strict";

import {
  startMockSteam,
  GOOD_KEY,
  REVOKED_KEY,
  SAM,
  PRIVATE_ID,
  NO_SUCH_ID,
  OWNED_PATH,
  SUMMARIES_PATH,
  ITEMS_PATH,
} from "./mock-steam.mjs";

async function withMock(fn) {
  const mock = await startMockSteam({ port: 0, logPath: null });
  try {
    await fn(mock);
  } finally {
    await mock.close();
  }
}

const owned = (mock, key, steamid, init = {}) => {
  const q = new URLSearchParams({ steamid, include_appinfo: "1", include_played_free_games: "1", format: "json" });
  if (key !== null) q.set("key", key);
  return fetch(`${mock.url}${OWNED_PATH}?${q}`, { redirect: "manual", ...init });
};

test("the good key lists SamPlays's games; private game details answer {}", async () => {
  await withMock(async (mock) => {
    const res = await owned(mock, GOOD_KEY, SAM);
    assert.equal(res.status, 200);
    const body = await res.json();
    assert.equal(body.response.game_count, 6);
    assert.deepEqual(
      body.response.games.map((g) => g.name),
      ["Quiet Comet", "Harbor of Bells", "Rime Assembly", "Signal Lost", "Desk Lights", "Proton 9.0"],
    );
    assert.ok(body.response.games.every((g) => "rtime_last_played" in g));
    const priv = await (await owned(mock, GOOD_KEY, PRIVATE_ID)).json();
    assert.deepEqual(priv, { response: {} });
    const empty = await (await owned(mock, GOOD_KEY, NO_SUCH_ID)).json();
    assert.equal(empty.response.game_count, 0);
  });
});

test("a revoked, wrong or missing key answers 401 HTML; summaries answer 403", async () => {
  await withMock(async (mock) => {
    for (const key of [REVOKED_KEY, "FAKE-STEAM-KEY-not-real-other", null]) {
      const res = await owned(mock, key, SAM);
      assert.equal(res.status, 401);
      assert.match(res.headers.get("content-type"), /text\/html/);
      assert.match(await res.text(), /Access is denied/);
    }
    const summaries = await fetch(`${mock.url}${SUMMARIES_PATH}?key=${REVOKED_KEY}&steamids=${SAM}`);
    assert.equal(summaries.status, 403);
  });
});

test("summaries give the persona, or no player for …0009", async () => {
  await withMock(async (mock) => {
    const sam = await (await fetch(`${mock.url}${SUMMARIES_PATH}?key=${GOOD_KEY}&steamids=${SAM}`)).json();
    assert.equal(sam.response.players[0].personaname, "SamPlays");
    const none = await (await fetch(`${mock.url}${SUMMARIES_PATH}?key=${GOOD_KEY}&steamids=${NO_SUCH_ID}`)).json();
    assert.deepEqual(none.response.players, []);
  });
});

test("GetItems answers by position, success 15 with appid 0 for the tools", async () => {
  await withMock(async (mock) => {
    const input = { ids: [{ appid: 3000040 }, { appid: 2805730 }, { appid: 3000080 }, { appid: 42 }], context: { language: "english", country_code: "US" } };
    const res = await fetch(`${mock.url}${ITEMS_PATH}?input_json=${encodeURIComponent(JSON.stringify(input))}`);
    assert.equal(res.status, 200);
    const items = (await res.json()).response.store_items;
    assert.equal(items.length, 4);
    assert.equal(items[0].appid, 3000040);
    assert.equal(items[0].type, 0);
    assert.equal(items[0].basic_info.developers[0].name, "Slow Orbit");
    assert.equal(items[0].assets.library_capsule, "library_600x900.jpg");
    assert.match(items[0].assets.asset_url_format, /^steam\/apps\/3000040\/\$\{FILENAME\}/);
    assert.deepEqual(items[1], { appid: 0, success: 15 });
    assert.equal(items[2].type, 6, "software");
    assert.deepEqual(items[3], { appid: 0, success: 15 });
    const art = await fetch(`${mock.url}/art/steam/apps/3000040/library_600x900.jpg`);
    assert.equal(art.status, 200);
    assert.equal(art.headers.get("content-type"), "image/png");
  });
});

test("the request log names the key's kind, never the key or the query", async () => {
  await withMock(async (mock) => {
    await owned(mock, GOOD_KEY, SAM);
    await owned(mock, REVOKED_KEY, SAM);
    await owned(mock, null, SAM);
    await fetch(`${mock.url}${ITEMS_PATH}?input_json=${encodeURIComponent('{"ids":[{"appid":3000040}]}')}`);
    const text = JSON.stringify(mock.requestLog);
    assert.doesNotMatch(text, /FAKE-STEAM-KEY/);
    assert.doesNotMatch(text, /steamid|input_json/);
    assert.deepEqual(mock.requestLog.map((r) => r.key), ["good", "revoked", null, null]);
    assert.equal(mock.requestLog[3].ids, 1);
    const fromRoute = await (await fetch(`${mock.url}/__e2e__/requests`)).json();
    assert.equal(fromRoute.length, 4);
  });
});

test("a referer, a cookie or a followed redirect is refused; fail gives 503", async () => {
  await withMock(async (mock) => {
    assert.equal((await owned(mock, GOOD_KEY, SAM, { headers: { referer: "http://x/" } })).status, 400);
    assert.equal((await owned(mock, GOOD_KEY, SAM, { headers: { cookie: "a=b" } })).status, 400);
    await fetch(`${mock.url}/__e2e__/redirect`, { method: "POST" });
    const moved = await owned(mock, GOOD_KEY, SAM);
    assert.equal(moved.status, 302);
    assert.equal(moved.headers.get("location"), "/redirected");
    assert.equal((await fetch(`${mock.url}/redirected`)).status, 400);
    await fetch(`${mock.url}/__e2e__/no-redirect`, { method: "POST" });
    await fetch(`${mock.url}/__e2e__/fail`, { method: "POST" });
    assert.equal((await owned(mock, GOOD_KEY, SAM)).status, 503);
    await fetch(`${mock.url}/__e2e__/up`, { method: "POST" });
    assert.equal((await owned(mock, GOOD_KEY, SAM)).status, 200);
    assert.equal(mock.requestLog.filter((r) => r.refused).length, 3);
  });
});
