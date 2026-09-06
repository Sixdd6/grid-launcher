# Round 6 — Art Defaults, Text Legibility, In-App Video, Viewer Filtering Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land the user's round-6 findings: background defaults blur 2 / fade 50%; text over the art readable in both themes; server-hosted videos actually play in the webview; the fullscreen viewer pages only through viewable images.

**Architecture:** Defaults are two constants on each side of the IPC boundary. Legibility is the designer's ruling applied verbatim: three token changes plus one `.over-art` utility class on six view roots. Video: WebKitGTK cannot play media from a custom URI scheme (probed 2026-09-05: `file://` plays, `asset://` with correct 206 range answers fails with MEDIA_ERR_SRC_NOT_SUPPORTED, a `blob:` URL from a secure+CORS-registered scheme plays), so the cached file's bytes travel over IPC as a raw response and play from an object URL. Viewer filtering is a pure helper that maps the gallery list and the shared failure map to the viewable list and index.

**Tech Stack:** Rust (grid-core, Tauri 2.11 `tauri::ipc::Response`), Svelte 5 runes + TypeScript, vitest, WebdriverIO E2E with the mock RomM server.

**Spec:** `docs/superpowers/specs/2026-09-04-desktop-ui-redesign-design.md` (§3 background, §4 typography/colour, §7 Media). Designer ruling for Task 2: `docs/superpowers/specs/2026-09-05-text-over-art-ruling.md` (committed with this plan).

## Global Constraints

- Token secrecy: tokens only in the OS keyring and the redacting in-memory type; never in files, logs, error strings, IPC payloads, console output or URLs. Never print any RetroArch cfg.
- No `git checkout` / `git restore` / `git reset` / `git stash`. Commit from the repo root with explicit pathspecs; subjects start with `rewrite: `.
- Gates for Rust changes, from `rewrite/`: `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo clippy -p app --all-targets --features e2e -- -D warnings`, `cargo test --workspace`; repo root: `bash rewrite/scripts/check_secret_hygiene.sh`. Frontend, from `rewrite/app`: `npm run check` (baseline 3 warnings: Details.svelte ×2, DownloadsFooter.svelte ×1 — no new ones), `npx vitest run`. E2E: `npm run typecheck` in `rewrite/e2e`; groups via `bash rewrite/scripts/e2e.sh <group…>` from the repo root (full build when Rust changed — never `E2E_SKIP_BUILD` then).
- Frontend rules: colours only via `app.css` tokens, motion only via `--m-*` tokens; no component test harness except SSR `render` from `svelte/server` (`// @vitest-environment node`).
- CSP: `https://img.youtube.com` in `img-src` stays the ONLY foreign host. `media-src` may gain `blob:` (Task 3) and nothing else.
- Never delete anything under `assets/`.
- All `rewrite/` paths below are relative to `rewrite/`.

---

### Task 1: Background defaults — blur 2, fade 50

**Files:**
- Modify: `crates/grid-core/src/config.rs` (`default_background_fade` ~line 107 → 50, `default_background_blur` ~line 111 → 2, tests ~lines 338–354 and any `[ui]` literal)
- Modify: `crates/grid-core/src/images/background.rs` (`BACKGROUND_BLUR_DEFAULT` ~line 52 → 2; doc comment: "2 keeps the art recognisable with a slight softening — user ruling 2026-09-05")
- Modify: `app/src-tauri/src/commands.rs` (tests ~lines 2312, 2349–2350, 2370, 2388, 2400–2401: every `25`/`12` default literal)
- Modify: `app/src/lib/theme.ts` (`FADE_DEFAULT` 25 → 50, `BLUR_DEFAULT` 12 → 2) and `app/src/lib/theme.test.ts`, `app/src/lib/stores/uiSettings.test.ts`, `app/src/lib/settings/appearance.test.ts` (any default literal)
- Modify: `e2e/specs/updates.spec.ts` (~lines 358, 389–390: `'12'` → `'2'`; if a fade default is asserted anywhere in `e2e/specs`, `25` → `50`)
- Modify: `docs/superpowers/specs/2026-09-04-desktop-ui-redesign-design.md` (§3 lines ~49 and ~52: "default 12" → "default 2", "default 25%" → "default 50%"), `docs/porting/07-covers-images.md` if it states either default (`grep -n 'default 12\|default 25\|25%' docs/porting/07-covers-images.md`)

**Interfaces:**
- Produces: `FADE_DEFAULT = 50`, `BLUR_DEFAULT = 2` (TS); `default_background_fade() = 50`, `default_background_blur() = 2`, `BACKGROUND_BLUR_DEFAULT = 2` (Rust). Ranges (`FADE_MAX` 60, `BLUR_MAX` 40) unchanged.

- [ ] **Step 1: Failing tests** — change the Rust config round-trip assertions to 50/2 and the TS `clampFade(NaN) === 50` / `clampBlur(NaN) === 2` cases. Run `cargo test -p grid-core config` and `npx vitest run theme` → FAIL.
- [ ] **Step 2: Implement** the six constants. Run the focused tests → PASS.
- [ ] **Step 3: Sweep literals** — `grep -rn 'background_fade: 25\|background_blur: 12\|fade(25)\|blur(12)' app/src-tauri/src crates` and fix each; `grep -rn "toHaveValue('12')" e2e/specs` → `'2'`.
- [ ] **Step 4: Docs** — the two design-spec lines; the porting doc if it names a default.
- [ ] **Step 5: Gates** — full Rust gate list, `npm run check`, `npx vitest run`, e2e `npm run typecheck`. E2E `updates` group (full build).
- [ ] **Step 6: Commit** — `git add` the files above; `git commit -m "rewrite: default the background art to blur 2 and fade 50"`.

Note: a `config.toml` that already stores `background_fade`/`background_blur` keeps its values; only a missing key takes the new default. That is the intended contract (say so in the commit body).

---

### Task 2: Text over the art — tokens and the `.over-art` class

**Files:**
- Read first (requirements, verbatim values): `docs/superpowers/specs/2026-09-05-text-over-art-ruling.md`
- Modify: `app/src/app.css` (token blocks: `:root`, the dark/light theme blocks as they exist; add `--text-halo` per theme; add the `.over-art` rule and its opt-out selector list exactly as §3.3 of the ruling gives them)
- Modify: `app/src/lib/Library.svelte`, `app/src/lib/Server.svelte`, `app/src/lib/Emulators.svelte`, `app/src/lib/Settings.svelte`, `app/src/lib/Downloads.svelte`, `app/src/lib/Shell.svelte` (the six class additions in §3.4 of the ruling — class attributes only, no other edits)
- Modify: `docs/superpowers/specs/2026-09-04-desktop-ui-redesign-design.md` (§4: the bullet given verbatim in §5 of the ruling)
- Test: `app/src/app.css.test.ts` (create if absent; a node-environment vitest that reads `app.css` as text and asserts: `--text-halo` is defined in every theme block that defines `--text-muted`; `.over-art` sets `text-shadow: var(--text-halo)`; the opt-out list resets `text-shadow: none`; the three changed token values are the ruling's exact hex strings)

**Interfaces:**
- Produces: `--text-halo` token (per theme); `.over-art` utility class; changed `--text-muted` (both themes) and light-theme `--danger`.
- Nothing else in `app.css` changes. Cards, popups, top bar, footer, buttons are explicitly out of scope (ruling §4).

- [ ] **Step 1: Failing test** — write `app/src/app.css.test.ts` per the Test line above (assert the exact values from ruling §3.1 and §3.3). Run `npx vitest run app.css` → FAIL.
- [ ] **Step 2: Implement** `app.css` per ruling §3.1–§3.3, then the six class additions per §3.4. Run → PASS.
- [ ] **Step 3: SSR smoke** — extend an existing SSR render test (or add `Shell.svelte`-free ones for `Library`/`Server` only if they already have SSR tests — do not add a harness) to assert the root element carries `over-art`. If no SSR test exists for those components, skip this step and say so in the report.
- [ ] **Step 4: Gates** — `npm run check` (no new warnings), `npx vitest run`, e2e `npm run typecheck`; E2E `library` and `emulators` groups with `E2E_SKIP_BUILD=1` (no Rust changed in this task; confirm `target/debug/.e2e-build-stamp` exists and the binary is newer than Task 1's last Rust change — otherwise build).
- [ ] **Step 5: Spec** — §4 bullet from ruling §5.
- [ ] **Step 6: Commit** — `git commit -m "rewrite: make text over the background art legible in both themes"`.

---

### Task 3: Hosted video plays from a blob URL

**Files:**
- Modify: `app/src-tauri/src/commands.rs` (`ensure_video` ~line 248 becomes the internal helper; new command `read_video`)
- Modify: `app/src-tauri/src/lib.rs` (register `read_video` in `generate_handler!`, replacing `ensure_video`)
- Modify: `app/src-tauri/tauri.conf.json` (`media-src`: add `blob:`; keep the existing entries)
- Modify: `app/src/lib/api.ts` (`ensureVideo` → `readVideo`)
- Modify: `app/src/lib/details/MediaViewer.svelte` (video effect: bytes → `Blob` → `URL.createObjectURL`; revoke on teardown; error text)
- Modify: `app/src/lib/details/media.ts` + `media.test.ts` (`videoObjectUrl` helper is NOT needed — keep the Blob code in the component; add nothing to `media.ts` unless a pure piece falls out)
- Modify: `e2e/specs/images-a.spec.ts` (the `.mp4` src assertion → `blob:` prefix)
- Modify: `docs/superpowers/specs/2026-09-04-desktop-ui-redesign-design.md` (§7 Media: "`path_video` played in-app from an object URL — WebKitGTK cannot play media from a custom URI scheme"), `docs/porting/07-covers-images.md` (one paragraph under the video section)

**Interfaces:**
- Produces: Tauri command `read_video(url: String) -> Result<tauri::ipc::Response, String>`: resolves and host-filters `url` exactly as `ensure_video` did, calls `grid_core::images::video::ensure_video` to get the cached path, refuses files larger than `MAX_INLINE_VIDEO_BYTES = 64 * 1024 * 1024` with `"video too large to play in-app"`, reads the file with `tokio::fs::read`, returns `tauri::ipc::Response::new(bytes)`. `pub const MAX_INLINE_VIDEO_BYTES: u64`.
- `api.readVideo(url: string): Promise<ArrayBuffer>` — `invoke<ArrayBuffer>('read_video', { url })` (Tauri 2 delivers a raw `Response` as `ArrayBuffer`).
- Viewer: `videoSrc` is an object URL; the teardown calls `URL.revokeObjectURL`; the fetch-failure text stays "This video could not be loaded", the decode-failure text stays "This video could not be played", and a too-large error shows the backend's message verbatim (it carries no secret — assert that in the Rust test by checking the string).
- The E2E mock's 32-byte stub still cannot decode: the spec asserts `src` starts with `blob:` and that `media-viewer-video-error` appears (the decode error is now the EXPECTED outcome for the stub — write the comment accordingly).

- [ ] **Step 1: Rust tests** — in `commands.rs` tests: a cached file above the cap → `Err` whose message is exactly `"video too large to play in-app"`; a small cached file → `Ok` with the same bytes. Use a temp cache dir the way the existing `ensure_video`/image tests do (`grep -n 'fn .*video' app/src-tauri/src/commands.rs crates/grid-core/tests/*.rs` for the fixture pattern). Run → FAIL.
- [ ] **Step 2: Implement** the command and registration; remove the `ensure_video` command wrapper if nothing else invokes it (`grep -rn ensure_video app/src`). Run the Rust gate list → PASS.
- [ ] **Step 3: Frontend** — `api.ts`; the viewer effect:

```ts
  $effect(() => {
    const item = current;
    videoSrc = null;
    videoError = false;
    videoLoadError = null;
    if (item === null || item.kind !== 'video') return;
    let cancelled = false;
    let objectUrl: string | null = null;
    api
      .readVideo(item.url)
      .then((bytes) => {
        if (cancelled) return;
        objectUrl = URL.createObjectURL(new Blob([bytes], { type: 'video/mp4' }));
        videoSrc = objectUrl;
      })
      .catch((e) => {
        if (!cancelled) videoLoadError = e instanceof Error ? e.message : String(e);
      });
    return () => {
      cancelled = true;
      if (objectUrl !== null) URL.revokeObjectURL(objectUrl);
    };
  });
```

The `type` is `video/mp4` for `.mp4`; derive `video/webm` / `video/quicktime` from the cached extension if the backend reports it — simplest: have `read_video` return the bytes only and let the viewer use `video/mp4` unless `item.url` ends with `.webm`/`.mov` (a pure helper `videoMimeType(url)` in `media.ts` with three tests). Show `videoLoadError` in place of the generic "could not be loaded" line when it is set.

- [ ] **Step 4: CSP** — `media-src`: `'self' asset: http://asset.localhost blob:`.
- [ ] **Step 5: Gates + E2E** — frontend gates; `images` group (full build).
- [ ] **Step 6: Docs** — §7 sentence; porting doc paragraph naming the probe result (file:// plays, asset:// fails with MEDIA_ERR_SRC_NOT_SUPPORTED even with 206 ranges, blob: plays).
- [ ] **Step 7: Commit** — `git commit -m "rewrite: play hosted videos from a blob URL — WebKitGTK cannot play a custom scheme"`.

---

### Task 4: The viewer pages only through viewable images

**Files:**
- Modify: `app/src/lib/details/media.ts`, `app/src/lib/details/media.test.ts`
- Modify: `app/src/lib/Details.svelte` (~lines 126–140 and 749–760)
- Modify: `app/src/lib/details/MediaViewer.svelte` (remove the "This screenshot could not be loaded" branch and the `media-viewer-image-error` test id; keep `onScreenshotError`)
- Modify: `app/src/lib/details/MediaTab.svelte` only if its `onOpen` index is an index into the FULL list (check: if it already skips failed tiles, what index does it emit?)
- Modify: `e2e/specs/images-a.spec.ts` / `images-b.spec.ts` (whichever asserts `media-viewer-image-error`; `grep -rn 'media-viewer-image-error' e2e/specs`)
- Modify: `docs/superpowers/specs/2026-09-04-desktop-ui-redesign-design.md` (§7: "the viewer walks the same viewable list as the gallery; a screenshot that failed to load is skipped, and if the current one fails the viewer moves to the next viewable item or closes when none is left")

**Interfaces:**
- Produces in `media.ts`:
  - `viewableItems(items: MediaItem[], failed: Record<string, true>): MediaItem[]` — drops `screenshot` items whose `url` is in `failed`; keeps `youtube`/`video`.
  - `viewableIndex(items: MediaItem[], failed: Record<string, true>, index: number): number | null` — the position of `items[index]` within `viewableItems`; when `items[index]` itself is failed, the position of the next viewable item AFTER it (wrapping), or `null` when nothing is viewable.
- `Details.svelte`: `viewerItems = $derived(viewableItems(mediaItems, failedMedia))`; the viewer receives `items={viewerItems}`; `viewerIndex` is an index into `viewerItems`; `onOpen(i)` from the tab maps through `viewableIndex(mediaItems, failedMedia, i)` (if the tab already emits full-list indices) — when it returns `null`, do not open. An `$effect` clamps `viewerIndex` when `viewerItems` shrinks: if `viewerIndex >= viewerItems.length`, set it to `viewerItems.length - 1`, and close (`null`) when the list is empty.
- The viewer no longer renders an error line for a screenshot; `onScreenshotError` still marks the URL, which removes it from `viewerItems` and the clamp effect moves the viewer on.

- [ ] **Step 1: Pure tests** — `viewableItems`: drops failed screenshots, keeps trailer/video, preserves order; `viewableIndex`: identity when nothing failed; shifts left past earlier failures; failed current → next viewable (wrapping); all failed → `null`. Run → FAIL.
- [ ] **Step 2: Implement** the helpers; wire `Details.svelte`; strip the viewer branch. `npm run check`, `npx vitest run` → PASS.
- [ ] **Step 3: E2E** — rom 103's detail fixture has `['…/103/screenshots/1.png', '…/103/screenshots/missing.png']`. Update the existing viewer case: open Media (one tile after `missing` fails), open the viewer, click next, expect the caption to stay "… — screenshot 1" (a one-item list wraps to itself) and `media-viewer-image-error` to NOT exist; if the case currently opens the viewer BEFORE the tab has marked the failure, keep that ordering and instead assert the viewer moves off `missing` by itself (caption "screenshot 1", never the error line). Run the `images` group (`E2E_SKIP_BUILD=1` allowed if no Rust changed since the last build in Task 3).
- [ ] **Step 4: Spec** — §7 sentence.
- [ ] **Step 5: Commit** — `git commit -m "rewrite: page the media viewer only through viewable images"`.

---

## Verification after the last task

1. Full gate list.
2. Full E2E suite: `bash rewrite/scripts/e2e.sh`.
3. Hand-test notes for the user: defaults apply only to a config without the keys (theirs already stores values — set the sliders by hand once); text halo visible over bright art in both themes; hosted video plays; viewer skips dead screenshots.
