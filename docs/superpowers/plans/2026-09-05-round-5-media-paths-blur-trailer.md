# Round 5 — Resource Paths, Background Blur Slider, Trailer Hand-off Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the three round-5 hand-test findings: server-hosted videos and fanart never load (bare relative resource paths resolve to the web app's HTML), the background art is blurred past recognition (add a Settings › Appearance blur slider), and YouTube trailers show "Video unavailable" (embedded players need an HTTP referrer that a `tauri://` page never sends — hand the trailer to the system browser).

**Architecture:** One resolver change in grid-core (`resolve_image_url`) fixes fanart and video at the source. The blur becomes `ui.background_blur` in `config.toml`, threaded into the variant builder as a sigma and baked into the variant's file name (`<key>.bg<sigma>.jpg`) so every blur level is its own cache entry. The YouTube viewer branch becomes a poster with a "Watch on YouTube" button that opens the system browser through a validated Tauri command; the iframe and its CSP allowance go away.

**Tech Stack:** Rust (grid-core, Tauri 2 app, `image` 0.25, `tauri-plugin-opener` 2), Svelte 5 runes + TypeScript, vitest, WebdriverIO E2E with the mock RomM server (`rewrite/e2e`).

**Spec:** `docs/superpowers/specs/2026-09-04-desktop-ui-redesign-design.md` (§3 background art, §7 Media tab, §10 Appearance). Amendments to it are part of Tasks 4 and 5.

**Evidence behind the plan (2026-09-05):**
- Live server (api-tester, read-only): `path_video` = `roms/20/194/video_normalized/video-normalized.mp4` and `ss_metadata.fanart_path` = `roms/20/194/fanart/fanart.png` — bare relative paths. `resolve_image_url` joins them onto the server root, which answers `200 text/html` (the SPA). `{base}/assets/romm/resources/{path}` answers `200 video/mp4` / `image/png`, with `Accept-Ranges: bytes`. 292 of 555 roms carry fanart, 298 carry a video: more than half the library gets NO background art today because the fanart tier is chosen first and every fanart fetch fails the image gate.
- User cache timeline: hover and the 5 s cycle DO swap the art (variants for consecutive screenshots were built 5 s apart). The "never changes" perception is the failed fanart tier plus sigma-20 blur at 25% opacity.
- YouTube: W3C referrer policy sends no `Referer` from a local scheme; YouTube error 153 ("Video unavailable") follows. `useHttpsScheme` is Windows/Android-only. tauri-apps/tauri#14422 confirms no markup fix works. Only opening the browser works on Linux.

## Global Constraints

- Token secrecy: tokens only in the OS keyring and the redacting in-memory type; never in files, logs, error strings, IPC payloads, console output or URLs. Never print `retroarch.cfg`.
- No `git checkout` / `git restore` / `git reset` / `git stash`. Commit with explicit pathspecs from the repo root; subjects start with `rewrite: `.
- Gates before every commit that touches Rust: `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo clippy -p app --all-targets --features e2e -- -D warnings`, `cargo test --workspace`, `bash rewrite/scripts/check_secret_hygiene.sh`. Frontend: `npm run check` (baseline 3 warnings: Details.svelte ×2, DownloadsFooter.svelte ×1 — no new ones), `npx vitest run`. E2E: `npm run typecheck` in `rewrite/e2e`.
- Frontend rules: colours only via `app.css` tokens, motion only via `--m-*` tokens; no component test harness except SSR `render` from `svelte/server` with `// @vitest-environment node`; `unittest`-style discipline does not apply here (vitest).
- CSP: `https://img.youtube.com` in `img-src` is the ONLY permitted foreign host; nothing else may be added. A plain `<img>` for the thumbnail, never through `ensure_image`.
- Never delete anything under `assets/`.
- All `rewrite/` paths below are relative to `rewrite/`.

---

### Task 1: Resolve bare relative resource paths under `/assets/romm/resources/`

**Files:**
- Modify: `crates/grid-core/src/images/urls.rs` (`resolve_image_url`, ~line 173; test `resolve_relative_and_normalize`, ~line 742)
- Modify: `docs/porting/07-covers-images.md` (add the rule under the resolver section that documents `resolve_cover_url`)

**Interfaces:**
- Produces: unchanged signature `pub fn resolve_image_url(value: &str, base_url: &str) -> String`; new behaviour: a value with no scheme and no leading `/` resolves to `{base_url}/assets/romm/resources/{value}`.
- Consumers that now work without change: `fanart_urls_from_payload` (fanart), `commands::ensure_video` (hosted video), `background_source_url`, `spawn_prefetch`.

- [ ] **Step 1: Write the failing tests** — in the existing `mod tests` of `urls.rs`, change the `api/x.png` assertion and add two cases:

```rust
        // RomM's `*_path` metadata fields (`ss_metadata.fanart_path`,
        // `path_video`) are relative to the resources root, unlike
        // `path_cover_*`, which arrive as absolute `/assets/...` paths.
        assert_eq!(
            resolve_image_url("api/x.png", "https://h"),
            "https://h/assets/romm/resources/api/x.png"
        );
        assert_eq!(
            resolve_image_url("roms/20/194/fanart/fanart.png", "http://192.168.1.137:8092"),
            "http://192.168.1.137:8092/assets/romm/resources/roms/20/194/fanart/fanart.png"
        );
        assert_eq!(
            resolve_image_url("roms/20/194/video_normalized/video-normalized.mp4", "https://h/"),
            "https://h/assets/romm/resources/roms/20/194/video_normalized/video-normalized.mp4"
        );
```

If `base_url` with a trailing slash is not already tolerated by the function, trim ONE trailing `/` from `base_url` before joining (covers the last case); check the existing tests for the trailing-slash contract first and keep them passing.

- [ ] **Step 2: Run to see them fail** — `cargo test -p grid-core resolve_relative_and_normalize` → FAIL on the first changed assertion.

- [ ] **Step 3: Implement** — in `resolve_image_url`, replace the final `else` arm:

```rust
    } else {
        // RomM serves every `*_path` metadata value (`fanart_path`,
        // `path_video`, `marquee_path`, …) from its resources root, and
        // sends them WITHOUT the `/assets/romm/resources/` prefix that
        // `path_cover_small`/`path_cover_large` carry. Joined onto the server
        // root they hit the SPA's index.html (200 text/html) and fail every
        // image/video gate downstream — verified against a live server
        // 2026-09-05.
        format!("{base}/assets/romm/resources/{candidate}", base = base_url.trim_end_matches('/'))
    };
```

Keep the `starts_with('/')` arm as it is.

- [ ] **Step 4: Run** — `cargo test -p grid-core images` → PASS. Then the full gate list from Global Constraints.

- [ ] **Step 5: Document** — in `docs/porting/07-covers-images.md`, next to the `resolve_cover_url` description, add one paragraph: relative `*_path` values are resources-relative; the rewrite prefixes `/assets/romm/resources/`; the Python app never consumed `path_video`/`fanart_path`, so this is rewrite-only behaviour.

- [ ] **Step 6: Commit**

```bash
git add rewrite/crates/grid-core/src/images/urls.rs docs/porting/07-covers-images.md
git commit -m "rewrite: resolve bare relative resource paths under /assets/romm/resources"
```

---

### Task 2: E2E proof that fanart and a hosted video travel the whole chain

**Files:**
- Modify: `e2e/mock-romm/server.mjs` (asset routes ~line 436–437 and ~588–590)
- Modify: `e2e/mock-romm/server.test.mjs` (asset route tests ~line 380–410)
- Modify: `e2e/fixtures/rom-details.json` (rom 101)
- Modify: `e2e/specs/images-a.spec.ts` (the Media/viewer case ~line 133–160 and the background case if present there; otherwise `e2e/specs/library.spec.ts` "paints the pre-blurred background variant" ~line 148)

**Interfaces:**
- Consumes: Task 1's resolver.
- Produces: mock routes `GET /assets/romm/resources/roms/:id/fanart/fanart.png` (200 `image/png`, `state.pngBytes`) and `GET /assets/romm/resources/roms/:id/video_normalized/video-normalized.mp4` (200 `video/mp4`, `MP4_BYTES`), no auth, same as covers.

- [ ] **Step 1: Mock server** — add beside `COVER_PATH_RE`:

```js
const FANART_PATH_RE = /^\/assets\/romm\/resources\/roms\/\d+\/fanart\/fanart\.png$/;
const VIDEO_PATH_RE = /^\/assets\/romm\/resources\/roms\/\d+\/video_normalized\/video-normalized\.mp4$/;
// A minimal ISO-BMFF header: a 32-byte `ftyp` box (brand isom) and an empty
// `mdat`. `ensure_video` gates on Content-Type and the `ftyp` magic at
// offset 4, not on decodability, so the spec can assert the viewer got a
// local .mp4 path without shipping a real clip.
const MP4_BYTES = Buffer.concat([
  Buffer.from([0x00, 0x00, 0x00, 0x20]), Buffer.from("ftypisom"),
  Buffer.from([0x00, 0x00, 0x02, 0x00]), Buffer.from("isomiso2avc1mp41"),
  Buffer.from([0x00, 0x00, 0x00, 0x08]), Buffer.from("mdat"),
]);
```

and in the handler, next to the cover/screenshot branch:

```js
  if (req.method === "GET" && FANART_PATH_RE.test(pathname)) {
    sendBuffer(res, 200, "image/png", state.pngBytes);
    return;
  }
  if (req.method === "GET" && VIDEO_PATH_RE.test(pathname)) {
    sendBuffer(res, 200, "video/mp4", MP4_BYTES);
    return;
  }
```

Add two `server.test.mjs` cases mirroring the existing cover-route test (status 200, content type, first bytes `ftyp` at offset 4 for the video). Run `node --test e2e/mock-romm/` (or the existing test command in `e2e/package.json`).

- [ ] **Step 2: Fixture** — in `rom-details.json`, rom 101 gains, EXACTLY as the live server shapes them (relative, no leading slash):

```json
"path_video": "roms/1/101/video_normalized/video-normalized.mp4",
"ss_metadata": { "fanart_path": "roms/1/101/fanart/fanart.png", "fanart_url": "https://img.example/fanart-external.jpg" }
```

`fanart_url` is a foreign host and must be dropped by the host filter — that is part of what the assertion below proves.

- [ ] **Step 3: Spec — hosted video** — in `images-a.spec.ts`'s Media/viewer case the gallery order is screenshots, trailer, then video (`galleryItems`). After the existing trailer assertion (Task 5 rewrites that assertion; only ADD here), step once more and assert:

```ts
    await $(testId('media-viewer-next')).click();
    await expect($(testId('media-viewer-caption'))).toHaveText('Super Mario World — video');
    await $(testId('media-viewer-video')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the hosted video never resolved to a local file — check resolve_image_url',
    });
    expect(await $(testId('media-viewer-video')).getAttribute('src')).toMatch(/\.mp4$/);
```

Then fix the wrap-around assertion that follows (one more `next` returns to screenshot 1). Also assert the Media tab now shows `details-media-3` (the video tile).

- [ ] **Step 4: Spec — fanart is the background** — in the background case (`library.spec.ts` "paints the pre-blurred background variant after a game is viewed"), replace the loose `.bg.jpg` check with the exact key: the visible layer's file name must start with the SHA-256 of the RESOLVED fanart URL. Use node's `crypto`:

```ts
import { createHash } from 'node:crypto';
// …
    const fanartKey = createHash('sha256')
      .update(`${mockUrl()}/assets/romm/resources/roms/1/101/fanart/fanart.png`)
      .digest('hex');
    await browser.waitUntil(
      async () => {
        const images = await browser.execute(() =>
          Array.from(document.querySelectorAll('[data-testid="background-art"] .layer'))
            .filter((el) => el.classList.contains('visible'))
            .map((el) => (el as HTMLElement).style.backgroundImage)
            .join(' '),
        );
        return images.includes(`${fanartKey}.bg`);
      },
      { timeout: TRANSITION_TIMEOUT, timeoutMsg: "the visible background layer never showed rom 101's fanart variant" },
    );
```

`mockUrl()` has no trailing slash (check `helpers/env.ts`; if it does, trim it — the key is the exact string the backend hashes, which is the resolved URL with no double slash). Match on `${fanartKey}.bg` (Task 3 renames the extension to `bg<sigma>.jpg`; this prefix survives both).

- [ ] **Step 5: Run** — `cd rewrite/e2e && npm run typecheck`, then `E2E_SKIP_BUILD=1 bash rewrite/scripts/e2e.sh images library` if `target/debug/.e2e-build-stamp` is present and the binary is newer than Task 1's commit — otherwise run without `E2E_SKIP_BUILD` (full build). Expected: both groups pass.

- [ ] **Step 6: Commit**

```bash
git add rewrite/e2e/mock-romm/server.mjs rewrite/e2e/mock-romm/server.test.mjs rewrite/e2e/fixtures/rom-details.json rewrite/e2e/specs/images-a.spec.ts rewrite/e2e/specs/library.spec.ts
git commit -m "rewrite: prove fanart and hosted video resolve end to end in the images spec"
```

---

### Task 3: `ui.background_blur` in the config and sigma-keyed background variants

**Files:**
- Modify: `crates/grid-core/src/config.rs` (`UiSettings` ~line 70–120 and its tests ~320–365, ~925–950)
- Modify: `crates/grid-core/src/images/background.rs`
- Modify: `crates/grid-core/src/images/replenish.rs` (`plan`, `run`, `NeedsVariant`)
- Modify: `crates/grid-core/tests/images_background.rs`, `crates/grid-core/tests/images_replenish.rs`
- Modify: `app/src-tauri/src/commands.rs` (`normalize_ui_settings` ~line 380, `ensure_background_variant` ~line 267, tests ~2255–2330)
- Modify: `app/src-tauri/src/images.rs` (`spawn_prefetch`, `replenish_once`)
- Modify: `e2e/specs/library.spec.ts` (only if a `.bg.jpg` literal remains after Task 2)

**Interfaces:**
- Produces:
  - `UiSettings.background_blur: u8` (serde default `default_background_blur()` = 12), clamped to `MAX_BACKGROUND_BLUR = 40` by `normalize_ui_settings`.
  - `pub const BACKGROUND_BLUR_DEFAULT: u8 = 12;` `pub const BACKGROUND_BLUR_MAX: u8 = 40;` and `pub fn background_variant_ext(sigma: u8) -> String` → `format!("bg{sigma}.jpg")` in `background.rs`. `BACKGROUND_VARIANT_EXT` and `BACKGROUND_BLUR_SIGMA` are removed.
  - `build_background_variant(source, dir, key, sigma: u8)`, `ensure_background_variant(cache, client, url, sigma: u8)`.
  - `replenish::plan(rows, cache, base_url, sigma: u8)`, `replenish::run(client, cache, registry, base_url, items, sigma: u8)`.
  - Tauri command `ensure_background_variant(url: String, blur: u8)`; the frontend passes the store value (Task 4). The command clamps `blur` with `.min(MAX_BACKGROUND_BLUR)`.
- Cache contract: `<key>.bg<sigma>.jpg`; sigma 0 means no blur (still downscaled). Old `<key>.bg.jpg` files are orphans; the sweep already reaps unpinned files by cap and pins by key prefix, so no migration — note this in the module doc.

- [ ] **Step 1: Config tests** — extend the existing `UiSettings` round-trip tests: a `[ui]` table without `background_blur` loads as 12; a table with `background_blur = 30` round-trips; `Config::save` emits `background_blur`. Run `cargo test -p grid-core config` → FAIL.

- [ ] **Step 2: Config** — add to `UiSettings`:

```rust
    /// Background-art blur sigma, 0–40, applied at the variant's 960px scale
    /// (`images::background`). Baked into the variant's file name, so a
    /// change never serves a stale blur. Clamped on write by
    /// `normalize_ui_settings`.
    #[serde(default = "default_background_blur")]
    pub background_blur: u8,
```

with `fn default_background_blur() -> u8 { 12 }`, the `Default` impl, and every struct literal in tests. Run → PASS.

- [ ] **Step 3: Variant builder tests** — in `tests/images_background.rs` switch every `BACKGROUND_VARIANT_EXT` use to `background_variant_ext(12)` and add: two sigmas for one source yield two files (`<key>.bg12.jpg` and `<key>.bg0.jpg`), and sigma 0 still produces a JPEG no wider than 960. Run → FAIL (compile).

- [ ] **Step 4: Builder** — in `background.rs`: replace the constants with `BACKGROUND_BLUR_DEFAULT`, `BACKGROUND_BLUR_MAX`, `background_variant_ext`; thread `sigma: u8` through `build_background_variant` (skip `fast_blur` when `sigma == 0`, otherwise `fast_blur(&rgb, f32::from(sigma))`), `ensure_background_variant`, `build_once`, and the `variant_failed`/`variant_in_flight` keys (key them by `format!("{key}.{ext}")` so two sigmas of one source are independent). Update the module doc's file-name paragraph and add the orphan note. Run `cargo test -p grid-core` → PASS.

- [ ] **Step 5: Replenish** — `plan`/`run` take `sigma` and use `background_variant_ext(sigma)` for the existence check and the build; update `tests/images_replenish.rs` (pass 12) and add one case: with a `bg20.jpg` on disk and sigma 12 requested, `plan` still emits `NeedsVariant`. Run → PASS.

- [ ] **Step 6: App layer** — `commands.rs`: `MAX_BACKGROUND_BLUR: u8 = 40`; `normalize_ui_settings` clamps `background_blur`; the command gains `blur: u8` and passes `blur.min(MAX_BACKGROUND_BLUR)`; extend the existing normalize tests (blur 41 → 40, default literal gains `background_blur: 12`). `images.rs`: `replenish_once` and `spawn_prefetch` read the sigma once via `Config::load(&Config::default_path()).map(|c| c.ui.background_blur).unwrap_or(BACKGROUND_BLUR_DEFAULT)` inside their existing `spawn_blocking`/async bodies (a failed load falls back to the default — the art is not worth an error). Run the full gate list.

- [ ] **Step 7: E2E literal** — grep `e2e/specs` for `.bg.jpg`; any remaining literal becomes a `.bg` prefix match (Task 2 already did this for the fanart case).

- [ ] **Step 8: Commit**

```bash
git add rewrite/crates/grid-core/src/config.rs rewrite/crates/grid-core/src/images/background.rs rewrite/crates/grid-core/src/images/replenish.rs rewrite/crates/grid-core/tests/images_background.rs rewrite/crates/grid-core/tests/images_replenish.rs rewrite/app/src-tauri/src/commands.rs rewrite/app/src-tauri/src/images.rs rewrite/e2e/specs/library.spec.ts
git commit -m "rewrite: store ui.background_blur and key background variants by sigma"
```

---

### Task 4: Appearance blur slider, blur-aware fetches, and a failed-tier fallback

**Files:**
- Modify: `app/src/lib/theme.ts`, `app/src/lib/theme.test.ts` (or wherever `clampFade` is tested — `grep -rn clampFade app/src`)
- Modify: `app/src/lib/api.ts` (`UiSettings`, `ensureBackgroundVariant`)
- Modify: `app/src/lib/stores/uiSettings.svelte.ts`
- Modify: `app/src/lib/settings/AppearancePage.svelte`
- Modify: `app/src/lib/background.ts`, `app/src/lib/background.test.ts`
- Modify: `app/src/lib/backgroundPrefetch.ts` (+ its test if one exists)
- Modify: `app/src/lib/BackgroundArt.svelte`
- Modify: `app/src/lib/stores/lastViewed.svelte.ts` (only if the failed set lives there — see Step 5; prefer the component)
- Modify: `docs/superpowers/specs/2026-09-04-desktop-ui-redesign-design.md` (§3 background paragraph, §10 Appearance line)
- Modify: the E2E spec that exercises the Appearance page (`grep -rln 'theme-select' e2e/specs`; if none exists, add the assertion to `updates.spec.ts` which already opens Settings — check its structure first)

**Interfaces:**
- Consumes: Task 3's command signature `ensure_background_variant(url, blur)` and `UiSettings.background_blur`.
- Produces:
  - `theme.ts`: `BLUR_DEFAULT = 12`, `BLUR_MAX = 40`, `clampBlur(value: number): number` (NaN → default, round, clamp 0–40).
  - `uiSettings.backgroundBlur: number` getter; `commitBackgroundBlur(value: number): Promise<void>` (applies to the store, then persists via `payload()`). No drag preview: every distinct value is a full rebuild, so the slider persists on `onchange` only; the art re-fetches through the effect below.
  - `api.ensureBackgroundVariant(url: string, blur: number)`.
  - `background.ts`: `backgroundUrls(subject, failed: ReadonlySet<string> = EMPTY_SET)` — each tier is filtered by `failed` BEFORE the first-non-empty-tier rule, so a fanart that cannot be built falls through to the screenshots, then the cover.
  - `backgroundPrefetch.ts`: memo key `${blur}\n${url}`; `prefetchBackground(subject)` reads `uiSettings.backgroundBlur`; exported `variantKey(blur, url)`.

- [ ] **Step 1: Pure tests first** — `clampBlur` (NaN → 12, 41 → 40, -1 → 0, 12.6 → 13); `backgroundUrls` with `failed`: fanart failed → screenshots; all fanart and screenshots failed → cover; nothing failed → unchanged. Run `npx vitest run` → FAIL.

- [ ] **Step 2: Implement `theme.ts` and `background.ts`** to the interfaces above. `EMPTY_SET` is a module-level `new Set<string>()` — never mutated.

- [ ] **Step 3: Store + API** — `api.ts`: `background_blur: number` in `UiSettings`; `ensureBackgroundVariant: (url: string, blur: number) => invoke<string>('ensure_background_variant', { url, blur })`. Store: `backgroundBlur` in `state` (default `BLUR_DEFAULT`), in `payload()`, loaded in `initUiSettings` via `clampBlur(stored.background_blur)`, plus `commitBackgroundBlur`. Extend the store test if one exists (`grep -rn uiSettings app/src --include=*.test.ts`).

- [ ] **Step 4: Appearance page** — after the fade field:

```svelte
<div class="field">
  <label for="background-blur">Background art blur</label>
  <input
    data-testid="background-blur"
    id="background-blur"
    type="range"
    min="0"
    max={BLUR_MAX}
    step="1"
    value={uiSettings.backgroundBlur}
    onchange={(e) => {
      commitBackgroundBlur(Number((e.currentTarget as HTMLInputElement).value)).catch(() => {});
    }}
  />
  <span class="value">{uiSettings.backgroundBlur}</span>
</div>
```

Update the file's header comment (blur is commit-on-release; the fade remains live-preview).

- [ ] **Step 5: BackgroundArt + prefetch** — `BackgroundArt.svelte`: `let failed = $state(new Set<string>())` (replace the Set, never mutate in place, so `$derived` notices); `let urls = $derived(backgroundUrls(lastViewed.subject, failed))` — this replaces the `lastViewed.urls` getter read (keep the getter for other callers, or delete it if nothing else uses it — `grep -rn 'lastViewed.urls'`). The fetch effect reads `const blur = uiSettings.backgroundBlur` and calls `api.ensureBackgroundVariant(url, blur)`; the memo lookup uses `variantKey(blur, url)`. In the `.catch`, record the failure: `failed = new Set([...failed, url])` — that re-derives `urls`, which drops to the next tier or the next image. Reset `failed` when the subject changes (in the index-reset effect, `failed = new Set()` — but only when the subject object changed, not on the failed-set change; read `lastViewed.subject` there, not `urls`). Keep the ordering comment: the reset effect stays declared before the fetch effect. `backgroundPrefetch.ts`: `variantKey` + blur-aware `prefetchBackground`.

- [ ] **Step 6: Frontend gates** — `npm run check` (no new warnings), `npx vitest run` → PASS.

- [ ] **Step 7: Spec + E2E** — design spec §3: replace "blurred by the backend once" with "scaled to 960px and blurred by the backend once at the Settings › Appearance blur level (0–40, default 12, stored as `ui.background_blur`, baked into the cached variant's name); a tier whose images cannot be built falls through to the next tier". §10: add "background blur slider 0–40 (commits on release)". E2E: in the spec that opens Settings › Appearance, assert `background-blur` exists with `max="40"` and that setting it to `0` and back persists (read the value after re-opening the page). Run that group with `E2E_SKIP_BUILD` only if the binary is newer than Task 3's commit; otherwise a full build.

- [ ] **Step 8: Commit**

```bash
git add rewrite/app/src/lib/theme.ts rewrite/app/src/lib/api.ts rewrite/app/src/lib/stores/uiSettings.svelte.ts rewrite/app/src/lib/settings/AppearancePage.svelte rewrite/app/src/lib/background.ts rewrite/app/src/lib/backgroundPrefetch.ts rewrite/app/src/lib/BackgroundArt.svelte rewrite/app/src/lib/stores/lastViewed.svelte.ts docs/superpowers/specs/2026-09-04-desktop-ui-redesign-design.md rewrite/e2e/specs
git add rewrite/app/src/lib/*.test.ts rewrite/app/src/lib/**/*.test.ts
git commit -m "rewrite: add the background blur slider and fall through failed art tiers"
```

(Adjust the test-file pathspecs to the files actually touched; `git status --short` first.)

---

### Task 5: Trailers open in the system browser; hosted-video element errors surface

**Files:**
- Create: nothing
- Modify: `app/src-tauri/src/commands.rs` (new command near `ensure_video`), `app/src-tauri/src/lib.rs` (register it in the `generate_handler!` list ~line 292)
- Modify: `app/src-tauri/tauri.conf.json` (remove `frame-src`)
- Modify: `app/src/lib/api.ts`, `app/src/lib/details/media.ts`, `app/src/lib/details/media.test.ts`
- Modify: `app/src/lib/details/MediaViewer.svelte`, `app/src/lib/details/MediaTab.svelte` (pass `coverUrl` to the viewer)
- Modify: `e2e/specs/images-a.spec.ts` (the `media-viewer-youtube` iframe assertion ~line 153)
- Modify: `docs/superpowers/specs/2026-09-04-desktop-ui-redesign-design.md` (§7 Media line 127 and a decision row)

**Interfaces:**
- Produces:
  - Tauri command `open_youtube_video(video_id: String) -> Result<(), String>`: rejects anything but an 11-character `[A-Za-z0-9_-]` id with `"not a YouTube video id"`, then `app.opener().open_url(format!("https://www.youtube.com/watch?v={id}"), None::<&str>)`. Pattern: `commands/updates.rs:104`. Unit test the validator (`youtube_watch_url(id) -> Option<String>`) in `commands.rs` tests.
  - `media.ts`: `youtubeWatchUrl(videoId): string` replaces `youtubeEmbedUrl` (keep `isYoutubeId`).
  - `api.openYoutubeVideo(videoId: string): Promise<void>`.
  - Viewer markup for a `youtube` item: the poster (`trailerPoster(videoId, coverUrl, thumbnailFailed)` — pass `coverUrl` into `MediaViewer` from `MediaTab`, the same prop the tab already holds) inside the stage, a centred primary button `data-testid="media-viewer-youtube-open"` labelled "Watch on YouTube", and a one-line note `data-testid="media-viewer-youtube-note"`: "Trailers open in your browser." Any open failure renders inline as `media-viewer-youtube-error`.
  - The `<video>` element gains `onerror={() => (videoError = true)}` so a file the webview cannot decode shows "This video could not be played" (distinct from the fetch failure text "This video could not be loaded").
- Removed: the `<iframe>`, `youtubeEmbedUrl`, and the CSP `frame-src` entry (no frame is created any more; `default-src 'self'` covers the rest).

- [ ] **Step 1: Tests** — `media.test.ts`: `youtubeWatchUrl('dQw4w9WgXcQ')` → `https://www.youtube.com/watch?v=dQw4w9WgXcQ`; the embed helper is gone. Rust: `youtube_watch_url("dQw4w9WgXcQ")` → `Some(...)`, `youtube_watch_url("../evil")` → `None`, `youtube_watch_url("dQw4w9WgXcQ&list=x")` → `None`. Run → FAIL.

- [ ] **Step 2: Implement** the command, registration, `api.ts` entry, `media.ts`, and the viewer branch. Viewer button handler: `api.openYoutubeVideo(current.videoId).catch((e) => (youtubeError = e instanceof Error ? e.message : String(e)))`. Poster styling reuses the existing `.frame` box (16:9, `object-fit: cover` on the `<img>`/`Image`), the play badge from `MediaTab.svelte`, and `app.css` tokens only.

- [ ] **Step 3: Gates** — Rust gate list; `npm run check`; `npx vitest run`.

- [ ] **Step 4: E2E** — in `images-a.spec.ts` replace the iframe `src` assertion with:

```ts
    await expect($(testId('media-viewer-youtube-open'))).toBeExisting();
    await expect($(testId('media-viewer-youtube-note'))).toHaveText('Trailers open in your browser.');
```

Do NOT click the button in E2E (it would launch a browser on the host). Run the `images` group (`E2E_SKIP_BUILD` only if the binary is newer than this task's Rust changes — it will not be, so build).

- [ ] **Step 5: Design spec** — §7 Media: "video (`youtube_video_id` as a poster that opens the trailer in the system browser — embedded players need an HTTP referrer that a `tauri://` page never sends (W3C referrer policy, YouTube error 153); `path_video` played in-app from the local cache)". Add a decision row `D-UI-…` recording the ruling and the date 2026-09-05.

- [ ] **Step 6: Commit**

```bash
git add rewrite/app/src-tauri/src/commands.rs rewrite/app/src-tauri/src/lib.rs rewrite/app/src-tauri/tauri.conf.json rewrite/app/src/lib/api.ts rewrite/app/src/lib/details/media.ts rewrite/app/src/lib/details/media.test.ts rewrite/app/src/lib/details/MediaViewer.svelte rewrite/app/src/lib/details/MediaTab.svelte rewrite/e2e/specs/images-a.spec.ts docs/superpowers/specs/2026-09-04-desktop-ui-redesign-design.md
git commit -m "rewrite: open YouTube trailers in the system browser and surface video decode errors"
```

---

## Verification after the last task

1. Full gate list (Rust, frontend, secret hygiene, e2e typecheck).
2. Full E2E suite: `bash rewrite/scripts/e2e.sh` (all 15 groups) on the final tree.
3. api-tester (read-only): GET the resolved fanart URL and video URL for two live roms through the NEW resolver rule and report Content-Type — PASS means `image/*` and `video/mp4`.
4. Hand-test notes for the user: hosted video playback in WebKitGTK is unverified here (GStreamer decodes H.264 on this machine, but playback from the `asset:` scheme is the open question); the blur slider default is 12; trailers open the browser.
