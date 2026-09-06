# Round 8 — Grid Scroll Performance Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Scrolling a platform with hundreds of games stops stuttering.

**Architecture:** Two CSS changes in `GameCard.svelte` — the blurred cover backdrop is rasterised at a quarter of its size and scaled up (the upscale supplies most of the blur, at one sixteenth of the pixels), and `content-visibility: auto` is dropped so cards no longer lay out and paint one row at a time while scrolling — plus an idle gate on the scroll-warm lane so speculative background builds never run while the user is actively scrolling.

**Tech Stack:** Svelte 5 + CSS, vitest; WebdriverIO E2E.

**Spec:** `docs/superpowers/specs/2026-09-04-desktop-ui-redesign-design.md` (§5 cards, D-UI cover "option B", §3 background warm-up).

**Evidence (2026-09-06, WebdriverIO against the mock server serving a real 302×428 cover for every card, a 302-game platform, 1920×1080, 4 s programmatic scroll, `requestAnimationFrame` deltas):**

| Variant | mean ms/frame | p95 | frames > 50 ms |
|---|---|---|---|
| baseline | 64 | 90 | 42 |
| `.card { will-change: auto }` | 64 | 88 | 41 |
| `.over-art { text-shadow: none }` | 62 | 86 | 44 |
| `img.backdrop { filter: none }` | 34 | 59 | 29 |
| `img.backdrop` hidden | 28 | 55 | 29 |
| quarter-size backdrop, `blur(2.5px)`, `scale(4)` | 41 | 65 | 31 |
| no `content-visibility` + `filter: none` | 25 | 40 | 0 |
| **no `content-visibility` + quarter blur** | **28** | **30** | **0** |
| promoting the scroller (`will-change`/`translateZ`) | no change | | |
| document-level scrolling instead of the inner scroller | 128 | 136 | 31 |

Platform open (302 cards): 245 ms with `content-visibility: auto`, 340 ms without. With a 1×1 PNG cover every variant scrolls at 16 ms, so the cost is image raster, not DOM size. The remaining ~28 ms is main-thread raster of the visible cards per frame (WebKitGTK does not scroll `overflow: auto` containers on the compositor); reaching 16 ms would need fewer painted pixels, not fewer nodes.

## Global Constraints

- Colours only via `app.css` tokens, motion only via `--m-*` tokens; no component test harness except SSR `render` from `svelte/server`.
- No `git checkout` / `git restore` / `git reset` / `git stash`. Commit from the repo root with `git commit --only -- <paths>`; subjects start with `rewrite: `.
- Frontend gates, from `rewrite/app`: `npm run check` (baseline 3 warnings: Details.svelte ×2, DownloadsFooter.svelte ×1 — no new ones), `npx vitest run`. E2E: `npm run typecheck` in `rewrite/e2e`; groups via `bash rewrite/scripts/e2e.sh <group…>` from the repo root; never `E2E_SKIP_BUILD=1` after a source change (it also skips the frontend build).
- The E2E harness cannot simulate a hover; never write a hover-based E2E assertion.
- All `rewrite/` paths below are relative to `rewrite/`.

---

### Task 1: Quarter-resolution backdrop, no `content-visibility`, idle-gated scroll warming

**Files:**
- Modify: `app/src/lib/GameCard.svelte` (`.card` rule ~lines 115–130: remove `content-visibility: auto` and `contain-intrinsic-size`, update the comment at ~140 that cites content-visibility's paint containment; `.cover :global(img.backdrop)` ~lines 168–176)
- Modify: `app/src/lib/CardGrid.svelte` (~lines 29–35: `--card-min` was published for the intrinsic-size estimate — keep it only if something else reads it; `grep -rn 'card-min' app/src`)
- Modify: `app/src/lib/backgroundPrefetch.ts`, `app/src/lib/backgroundPrefetch.test.ts` (idle gate)
- Modify: `app/src/lib/visibleWarm.ts`, `app/src/lib/visibleWarm.test.ts` (scroll listener on the scroll parent)
- Modify: `docs/superpowers/specs/2026-09-04-desktop-ui-redesign-design.md` (§5: the backdrop is drawn at quarter size; cards are not content-visibility-gated; §3: warming pauses while scrolling)

**Interfaces:**
- CSS for the backdrop (replace the whole `.cover :global(img.backdrop)` block):

```css
  /* Round 8: the blurred copy is rasterised at a QUARTER of the cover's
     size and scaled up by 4 — the upscale supplies most of the softening,
     so a 2.5px blur at quarter size reads like the old 10px blur at full
     size while touching one sixteenth of the pixels. Measured on a
     302-card platform with real covers: 64 → 28 ms per scroll frame with
     the content-visibility change below, and no frame over 50 ms. */
  .cover :global(img.backdrop) {
    position: absolute;
    top: -12px;
    left: -12px;
    width: calc((100% + 24px) / 4);
    height: calc((100% + 24px) / 4);
    transform: scale(4);
    transform-origin: top left;
    object-fit: cover;
    filter: blur(2.5px) brightness(0.45);
    pointer-events: none;
  }
```

- `.card`: delete `content-visibility: auto;` and the `contain-intrinsic-size` declaration and their comment; add: `/* No content-visibility: with real covers, laying out and painting each row as it scrolled in cost more per frame (64 ms) than painting every card once at open (+100 ms for 300 cards). */`. Keep `overflow: hidden` or whatever clips the scaled backdrop inside `.cover` (the old comment at ~140 relied on content-visibility's paint containment to clip the `inset: -12px` overscan — `.cover` must now clip it itself: `overflow: hidden` on `.cover`, verify it is there or add it).
- `backgroundPrefetch.ts`: `export function setScrollIdle(idle: boolean): void` — while `false`, the drain loop starts no BACK-lane (warm) builds; front-lane (hover) builds are unaffected; flipping to `true` drains. `export const SCROLL_IDLE_MS = 250`. Tests: warms queued during scrolling do not start; they start once idle; a hover during scrolling still starts.
- `visibleWarm.ts`: `observe(grid)` also attaches a passive `scroll` listener on `scrollParent(grid)` (or `window` when none) that calls `setScrollIdle(false)` and re-arms a `SCROLL_IDLE_MS` trailing timer that calls `setScrollIdle(true)`; `disconnect()` removes the listener, clears the timer and calls `setScrollIdle(true)`. Tests with a stub element carrying `addEventListener`/`removeEventListener` and fake timers.

- [ ] **Step 1: Tests first** — the idle-gate cases in `backgroundPrefetch.test.ts` and the listener cases in `visibleWarm.test.ts`. Run `npx vitest run` → FAIL.
- [ ] **Step 2: Implement** the gate and the listener; run → PASS.
- [ ] **Step 3: CSS** — apply the two `GameCard.svelte` changes and the `CardGrid.svelte` cleanup. `npm run check` → no new warnings.
- [ ] **Step 4: E2E** — `bash rewrite/scripts/e2e.sh library images` (full build). The library spec's card-click case relies on the card's centre band being free of overlay controls (`cards/size.ts`), which this change does not touch; the images spec's cover assertion (`renders a real, loaded cover image`) must still pass.
- [ ] **Step 5: Spec** — the §5 and §3 sentences above.
- [ ] **Step 6: Commit** — `git commit --only -- <paths> -m "rewrite: quarter-size card backdrops, no content-visibility, idle-gated warming"`.

---

## Verification after the task

1. Frontend gates; the `library` and `images` E2E groups (Step 4) and then the full suite.
2. Hand-test note: scroll the Arcade platform (255 games); it should feel smooth, and the cards' blurred backdrops should look the same as before at a glance.
