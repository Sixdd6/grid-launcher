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

### Task 3: Video rendering on the NVIDIA/Wayland stack

Defined from the research report appended below once it lands (the controller edits this section before dispatching Task 3). Until then this task is NOT dispatchable.

---

## Verification after the last task

1. Full gate list.
2. Full E2E suite: `bash rewrite/scripts/e2e.sh`.
3. Hand-test notes for the user: no card is "selected" until an arrow key or gamepad input; hover swaps the art within ~120 ms plus the cross-fade; closing a popup leaves that game's art in place.
