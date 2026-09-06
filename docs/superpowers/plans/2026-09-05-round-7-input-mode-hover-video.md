# Round 7 — Input-Mode Selection, Hover Responsiveness, Video Rendering Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land the user's round-7 findings: the card "selection grow" and the focus-driven background follow only the input method actually in use (pointer hover vs keyboard/gamepad focus); closing the details popup no longer snaps the background back to the focused card; the hover background swap feels immediate; hosted video renders with correct colours on the NVIDIA/Wayland stack.

**Architecture:** A module-scoped `inputMode` store records the last input kind (pointer, keyboard, gamepad) from the three places input already enters (window pointer events in Shell, the arrow-key maps in Library/Server, the `nav` Tauri event in App). Card focus styling and the focus dwell are gated on a directional mode; a click also moves the focus index so keyboard continuity survives. Hover latency drops by shortening the dwell, starting the fetch on enter, warming variants for cards as they scroll into view through a small concurrency-limited queue, and using the base motion token for the cross-fade. The video task is defined by the research report appended in Task 3.

**Tech Stack:** Svelte 5 runes + TypeScript, vitest, WebdriverIO E2E with the mock RomM server; Rust/Tauri 2.11 for Task 3.

**Spec:** `docs/superpowers/specs/2026-09-04-desktop-ui-redesign-design.md` (§3 background art and input, §5 cards, D-UI-9). Amendments are part of Tasks 1 and 2.

**Evidence behind the plan (2026-09-05):**
- Details-close revert: `Library.svelte`/`Server.svelte` run a focus-dwell `$effect` whose dependencies include the open-popup flag; when the popup closes the effect re-runs and, 500 ms later, `noteViewed(rows[focusIndex])` fires. `focusIndex` starts at 0 and a click never moves it, so the first card's art takes over.
- Selection grow: `GameCard` renders `class:focused` whenever `i === focusIndex`, so card 0 is always "selected" even for a pure mouse user.
- Hover latency: `HOVER_DELAY_MS = 500`, `PREFETCH_DELAY_MS = 150`, cross-fade `CROSS_FADE_MS = 360` (`--m-slow`); a cold variant means download + decode + blur before anything shows.

## Global Constraints

- Token secrecy: tokens only in the OS keyring and the redacting in-memory type; never in files, logs, error strings, IPC payloads, console output or URLs. Never print any RetroArch cfg.
- No `git checkout` / `git restore` / `git reset` / `git stash`. Commit from the repo root with explicit pathspecs; subjects start with `rewrite: `.
- Frontend gates, from `rewrite/app`: `npm run check` (baseline 3 warnings: Details.svelte ×2, DownloadsFooter.svelte ×1 — no new ones), `npx vitest run`. E2E: `npm run typecheck` in `rewrite/e2e`; groups via `bash rewrite/scripts/e2e.sh <group…>` from the repo root. `E2E_SKIP_BUILD=1` skips BOTH the Rust and the frontend build — never use it after any source change.
- Rust gates (Task 3 only), from `rewrite/`: `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo clippy -p app --all-targets --features e2e -- -D warnings`, `cargo test --workspace`; repo root: `bash rewrite/scripts/check_secret_hygiene.sh`.
- Frontend rules: colours only via `app.css` tokens, motion only via `--m-*` tokens; no component test harness except SSR `render` from `svelte/server` (`// @vitest-environment node`).
- The E2E harness cannot simulate a hover (WebDriver `moveTo` yields a single `mousemove`, no `mouseenter`): never write a hover-based E2E assertion; prove hover behaviour with unit tests.
- Never delete anything under `assets/`.
- All `rewrite/` paths below are relative to `rewrite/`.

---

### Task 1: Input mode gates selection and the focus dwell; clicks move focus; details close keeps the art

**Files:**
- Create: `app/src/lib/stores/inputMode.svelte.ts`, `app/src/lib/stores/inputMode.test.ts`
- Modify: `app/src/lib/Shell.svelte` (window listeners ~line 113: `onpointerdown` exists; add `onpointermove`), `app/src/App.svelte` (~line 55 `nav` listener), `app/src/lib/Library.svelte` (arrow map ~line 215, focus effect ~lines 149–169, card props ~line 307), `app/src/lib/Server.svelte` (arrow map ~line 348, focus effect ~lines 286–303, card props ~line 513)
- Modify: `e2e/specs/library.spec.ts` ("ArrowRight moves the focused card" ~lines 98–122; add one details-close case)
- Modify: `docs/superpowers/specs/2026-09-04-desktop-ui-redesign-design.md` (§3 input paragraph; D-UI-9 row)

**Interfaces:**
- Produces `stores/inputMode.svelte.ts`:

```ts
export type InputKind = 'pointer' | 'keyboard' | 'gamepad';
const state = $state<{ current: InputKind }>({ current: 'pointer' });
export const inputMode = {
  get current(): InputKind { return state.current; },
  /** Keyboard and gamepad move a selection; the pointer only hovers. */
  get directional(): boolean { return state.current !== 'pointer'; },
};
/** Records the input that just happened. A no-op when unchanged, so a
 *  stream of pointer moves does not churn the store. */
export function noteInput(kind: InputKind): void {
  if (state.current !== kind) state.current = kind;
}
/** Test-only reset. */
export function resetInputMode(): void { state.current = 'pointer'; }
```

- Writers: Shell `onpointermove`/`onpointerdown` → `noteInput('pointer')`; Library/Server arrow handler → `noteInput('keyboard')` before `handleNav(action)`; App `nav` listener → `noteInput('gamepad')` before `shell?.handleNav(...)`.
- Consumers: `GameCard focused={i === focusIndex && inputMode.directional}`; the focus-dwell effect returns early unless `inputMode.directional` (read it INSIDE the effect so a switch to keyboard arms the dwell for the current index, and a switch back to pointer tears it down).
- Click: `onOpen={() => { focusIndex = i; openDetails(row); }}` in both views (the `{#each}` already has `i`), so keyboard navigation continues from the clicked card and the details-close re-run (which stays, for keyboard users) targets the SAME game — no revert.
- The `handleNav('accept')` path is unchanged (opens `rows[focusIndex]`).

- [ ] **Step 1: Store tests** (`inputMode.test.ts`, `// @vitest-environment node`): default is pointer and not directional; `noteInput('keyboard')` → directional; `noteInput('gamepad')` → directional; `noteInput('pointer')` → not; reset works. Run → FAIL (module missing).
- [ ] **Step 2: Implement** the store; run → PASS.
- [ ] **Step 3: Wire the writers and consumers** per Interfaces. In the two focus effects, add `const directional = inputMode.directional;` as the FIRST read and `if (!active || <popup open> || !directional) return;`. Keep the existing comments and add one line: "Pointer users never have a keyboard selection; the grow and the art follow the hover instead (user ruling 2026-09-05)."
- [ ] **Step 4: E2E** — in `library.spec.ts` "ArrowRight moves the focused card": replace the initial-focus wait with an assertion that NO card carries `focused` before any key is pressed (pointer mode by default), then `ArrowRight` and expect `game-card-102` focused (`moveFocus` from index 0). Add a case "closing details keeps that game's background": click `platform-btn-1`, click `game-card-103` (its detail fixture has screenshots), wait for `details-panel`, wait until the visible background layer's image contains the sha256 key of rom 103's first screenshot URL (`${mockUrl()}/assets/romm/resources/roms/103/screenshots/1.png`, same helper pattern as the existing fanart case), click `details-close`, `browser.pause(1200)`, assert the visible layer still shows that key. Before this change the case fails (rom 101's fanart returns).
- [ ] **Step 5: Gates** — `npm run check`, `npx vitest run`, e2e `npm run typecheck`; `bash rewrite/scripts/e2e.sh library` (full build).
- [ ] **Step 6: Spec** — §3: "Keyboard and gamepad input select a card (the focus ring and grow); pointer input only hovers, and no card is selected until a directional input happens. A click moves the selection to the clicked card. The focus dwell feeds the background only while a directional input is the active mode." D-UI-9: append "grow follows the active input method".
- [ ] **Step 7: Commit** — `git commit -m "rewrite: show card selection and the focus dwell only for the active input method"`.

---

### Task 2: Hover background feels immediate

**Files:**
- Modify: `app/src/lib/background.ts` (`HOVER_DELAY_MS` 500 → 120, `PREFETCH_DELAY_MS` 150 → 0, `CROSS_FADE_MS` 360 → 220 with the comment now naming `--m-base`), `app/src/lib/background.test.ts`
- Modify: `app/src/lib/BackgroundArt.svelte` (`transition: opacity var(--m-base) ease`)
- Modify: `app/src/lib/backgroundPrefetch.ts`, `app/src/lib/backgroundPrefetch.test.ts` (warm queue)
- Create: `app/src/lib/visibleWarm.ts`, `app/src/lib/visibleWarm.test.ts`
- Modify: `app/src/lib/Library.svelte`, `app/src/lib/Server.svelte` (mount the warmer on the grid element)
- Modify: `app/src/lib/lastViewedHover.test.ts` (timer values), `docs/superpowers/specs/2026-09-04-desktop-ui-redesign-design.md` (§3 numbers)

**Interfaces:**
- `backgroundPrefetch.ts` gains `warmBackground(subject: BackgroundSubject): void` — enqueues the subject's first URL unless memoised or already queued; a module queue drains at most `WARM_CONCURRENCY = 2` builds at a time through the existing `prefetchBackground` path (so covers on the grid keep the backend's download slots); `export const WARM_CONCURRENCY = 2`; `resetWarmQueue()` for tests.
- `visibleWarm.ts`:

```ts
export function createVisibleWarmer(subjectAt: (index: number) => BackgroundSubject | null): {
  observe: (grid: HTMLElement) => void;   // observes every current child; re-call after the list changes
  disconnect: () => void;
}
```

Uses `IntersectionObserver` with `rootMargin: '200px'` (warm one row ahead); on each intersecting entry, finds the child's index in `grid.children`, calls `warmBackground(subjectAt(index))` when non-null; unobserves that element after warming once. Guard `typeof IntersectionObserver === 'undefined'` (SSR/vitest) → a no-op warmer.
- Library/Server: `$effect` that, once the grid element exists and whenever `rows`/`visible` changes, calls `warmer.observe(grid.element())`; teardown disconnects.

- [ ] **Step 1: Tests first** — `background.test.ts`: the three constants' new values; `backgroundPrefetch.test.ts`: `warmBackground` dedupes, respects `WARM_CONCURRENCY` (two in flight, third waits until one resolves), skips memoised URLs; `visibleWarm.test.ts` (node env): with a stub `IntersectionObserver` class on `globalThis`, `observe` registers each child, an intersecting entry warms exactly its index once, and a missing `IntersectionObserver` yields a no-op. Run → FAIL.
- [ ] **Step 2: Implement** per Interfaces. `lastViewedHover.test.ts`: update fake-timer expectations (prefetch at 0 ms fires synchronously inside `start` — call `prefetchBackground` directly when `prefetchMs === 0` rather than through a zero timer).
- [ ] **Step 3: Gates** — `npm run check`, `npx vitest run`, e2e `npm run typecheck`; `bash rewrite/scripts/e2e.sh library images` (full build; the warmer changes request timing, so the images group's replenish/offline cases must still pass).
- [ ] **Step 4: Spec** — §3: hover dwell "more than 120 ms" (was 500), fetch starts on enter, cards warm their first background as they scroll into view (two at a time), cross-fade 220 ms.
- [ ] **Step 5: Commit** — `git commit -m "rewrite: make the hover background immediate — shorter dwell, warm visible cards"`.

---

### Task 3: Hosted video plays through a local HTTP range server (blob frames render corrupted)

**Evidence (2026-09-05, on-screen captures of a standalone WebKitGTK 2.52 window with the app's `WEBKIT_DISABLE_DMABUF_RENDERER=1`, NVIDIA RTX 4070 / driver 610 / Wayland):** the same H.264 file renders correctly from `file://` and from `http://127.0.0.1:<port>/…` (a range-capable server) inside the app's exact overlay layout (blurred backdrop + 90% scrim), and renders corrupted (green or dark, blocky) from a `blob:` object URL in EVERY layout, simple or overlaid. `WEBKIT_DISABLE_COMPOSITING_MODE=1`, `GST_PLUGIN_FEATURE_RANK=vah264dec:0`, `WEBKIT_GST_USE_VIDEOCONVERT_SCALE=1` and `__NV_DISABLE_EXPLICIT_SYNC=1` do not help; enabling the DMABUF renderer still crashes the window (Wayland protocol error 71). So the blob media path is the defect and a loopback HTTP source is the clean path.

**Files:**
- Create: `app/src-tauri/src/media_server.rs`
- Modify: `app/src-tauri/src/lib.rs` (start the server in `setup`, keep its handle in `AppState`; register `video_url`, unregister `read_video`), `app/src-tauri/src/commands.rs` (replace `read_video`/`read_cached_video`/`MAX_INLINE_VIDEO_BYTES` with `video_url`), `app/src-tauri/tauri.conf.json` (`media-src`: `'self' http://127.0.0.1:*` — drop `blob:`, `asset:` and `http://asset.localhost`)
- Modify: `app/src/lib/api.ts` (`readVideo` → `videoUrl`), `app/src/lib/details/MediaViewer.svelte` (video effect: `videoSrc = await api.videoUrl(item.url)`; no Blob, no object URL, no revoke), `app/src/lib/details/media.ts` + `media.test.ts` (delete `videoMimeType`; keep `videoLoadMessage`)
- Modify: `e2e/specs/images-a.spec.ts` (`src` matches `/^http:\/\/127\.0\.0\.1:\d+\//`; the stub's decode error stays the expected outcome)
- Modify: `docs/superpowers/specs/2026-09-04-desktop-ui-redesign-design.md` (§7: "`path_video` streamed from the app's loopback media server"), `docs/porting/07-covers-images.md` (replace the blob paragraph with the evidence above and the server contract)

**Interfaces:**
- `media_server.rs`:

```rust
/// Serves cached video files to the webview over loopback HTTP/1.1 with
/// Range support. WebKitGTK plays http(s) media through its normal network
/// path; `blob:` frames render corrupted on the NVIDIA/Wayland stack and a
/// custom URI scheme is refused by the media player outright.
pub struct MediaServer { port: u16, nonce: String, dir: PathBuf }
impl MediaServer {
    /// Binds 127.0.0.1:0 (kernel-chosen port), spawns the accept loop on the
    /// Tauri async runtime, and returns the handle. `dir` is the image cache
    /// directory; only files directly inside it with a video extension are
    /// ever served.
    pub async fn start(dir: PathBuf) -> std::io::Result<Arc<MediaServer>>;
    /// `http://127.0.0.1:<port>/<nonce>/<file name>` for a file the server
    /// will serve, `None` when `path` is not inside `dir` or has no video
    /// extension.
    pub fn url_for(&self, path: &Path) -> Option<String>;
}
```

  Request handling (one task per connection, `Connection: close`): read up to 8 KiB of headers; accept only `GET` (and `HEAD`) with a path `/<nonce>/<name>` where `<name>` has no `/`, `..` or NUL and ends with one of `grid_core::images::video::VIDEO_EXTENSIONS`; anything else → `404 Not Found` with an empty body (never reveal why). Resolve `dir.join(name)`, `fs::metadata` for the length; parse `Range: bytes=a-b` / `bytes=a-` / `bytes=-n` (RFC 9110 §14.1.2): valid → `206 Partial Content` with `Content-Range: bytes a-b/len`, unsatisfiable → `416` with `Content-Range: bytes */len`, absent → `200`. Always `Accept-Ranges: bytes`, `Content-Length`, `Content-Type` from the extension (`video/mp4`, `video/webm`, `video/quicktime`), `Cache-Control: no-store`. Stream the body in 64 KiB chunks with `tokio::fs::File` + `seek`. The nonce is 32 random bytes hex (`rand` is already a workspace dependency — check `Cargo.lock`; otherwise `getrandom`) generated per launch; it is a loopback capability token, NOT a secret in the token-secrecy sense, but it must still never be logged.
- Command `video_url(state, url: String) -> Result<String, String>`: same resolve + host filter as before, `ensure_video` to get the cached path, then `state.media_server.url_for(&path).ok_or("the video is not in the cache directory")`.
- `api.videoUrl(url: string): Promise<string>`.

- [ ] **Step 1: Rust tests** (`media_server.rs` `#[cfg(test)]`, tokio runtime): start with a temp dir holding `a.mp4` (4 KiB of known bytes) and `notes.txt`; GET `/<nonce>/a.mp4` → 200, full bytes, `Accept-Ranges`; `Range: bytes=100-199` → 206, `Content-Range: bytes 100-199/4096`, exactly those bytes; `bytes=4000-` → 206 with the tail; `bytes=5000-` → 416; wrong nonce → 404; `/<nonce>/notes.txt` → 404; `/<nonce>/../a.mp4` → 404; `POST` → 404; `url_for` on a path outside `dir` → `None`. Use a raw `tokio::net::TcpStream` client in the tests (no HTTP client dependency). Run → FAIL (module missing).
- [ ] **Step 2: Implement** the server and the command; wire `setup` (`tauri::async_runtime::block_on(MediaServer::start(cache_dir))` or spawn + store in a `OnceLock` in `AppState` — pick what `AppState` construction allows; the command must fail with a clear message if the server did not start). Remove the old command, cap and helper. Rust gate list → PASS.
- [ ] **Step 3: Frontend** — `api.ts`, viewer effect, delete `videoMimeType` + its tests. `npm run check`, `npx vitest run` → PASS.
- [ ] **Step 4: CSP + E2E** — `media-src` per Files; `images-a.spec.ts` assertion; `bash rewrite/scripts/e2e.sh images` (full build).
- [ ] **Step 5: Docs** — §7 sentence; porting doc paragraph with the evidence and the server contract (loopback only, random port, per-launch nonce, video files only, Range support).
- [ ] **Step 6: Commit** — `git commit -m "rewrite: stream hosted videos from a loopback range server — blob frames render corrupted"`.

---

## Verification after the last task

1. Full gate list.
2. Full E2E suite: `bash rewrite/scripts/e2e.sh`.
3. Hand-test notes for the user: no card is "selected" until an arrow key or gamepad input; hover swaps the art within ~120 ms plus the cross-fade; closing a popup leaves that game's art in place.
