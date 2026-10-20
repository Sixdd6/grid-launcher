# Plan: a windowed CardGrid for every card grid

## Why (measured 2026-10-20)
The user's ~2000-game Arcade platform made hover drop most frames and scrolling crawl. Their webview main thread was pinned at 97% CPU, and 19 of 20 stack samples were in libwebkit2gtk style/layout/paint. An in-page rAF probe in the build container (software rendering), in ms per frame:

| DOM | scroll avg | hover avg |
|---|---|---|
| 2500 cards | 230 | 216 |
| 2500 cards, no `will-change` | 237 | 187 |
| 2500 cards, `content-visibility: auto` | 134 | 65 |
| 120 cards, all covers loaded | 11 | 12 |

The cost is the number of cards in the DOM. Fix: render only the rows near the scroll container's visible part.

## User decision
Universal: both grids (Server `Server.svelte`, Library `Library.svelte`, which covers every rail section) and every platform. No size threshold and no full-grid fallback. A platform that fits on screen renders exactly as today.

## Design
- **`app/src/lib/cards/window.ts`** (pure, vitest):
  - `parseTracks(template)` returns px track widths, or `null` when the template is unresolved or hidden.
  - `rowMetrics(trackPx, gap=16)` returns `{rowHeight, pitch}`, with `CARD_COVER_ASPECT = 3/4` in size.ts.
  - `windowRange({offset, viewportHeight, pitch, columns, count, bufferRows})` returns `{start, end, padTop, padBottom}`.
  - `initialRange(count)` covers the first `INITIAL_WINDOW_CARDS = 96`.
  - `scrollTopForIndex(...)`.
  - `BUFFER_ROWS = 2`.
- **`CardGrid.svelte`**:
  - Props: `items`, `key`, a `card` snippet `(item, absoluteIndex)`, `size`, `gridId`, `bufferRows`, `onRangeChange`.
  - Renders `items.slice(start, end)` and carries the off-screen height in the grid's padding-top/bottom. No spacer elements: `launch.spec.ts` uses `> *:first-child`.
  - Styles: `grid-auto-rows` from the model, `overflow-anchor: none`.
  - Measurement:
    - A passive scroll listener on `scrollParent(el)` recomputes the window, and writes state only when start or end changes.
    - `gridTop` is measured each time, because the header, toolbar and banners sit above the grid.
    - A ResizeObserver tracks grid width and scroller height.
    - Zero-size measurements (a hidden view) are ignored.
  - Exported methods: `columns()`, `element()`, `scrollToIndex(i)` (smooth `scrollIntoView` when the card is rendered, otherwise an instant jump), `range()`.
  - `columnsOf` must return 1 for a hidden grid, not 3.
- **`GameCard`**: an `index` prop rendered as `data-index`.
- **Image limiter** (`imageLoad.ts` / `Image.svelte`):
  - (a) A granted slot is held until its `ensure_image` call settles, even if the card unmounts. Otherwise scrolling past cards frees slots early and piles up backend fetches. A waiting slot still leaves at once.
  - (b) A bounded memo (url → path, about 4096 entries, insertion-order eviction). A remounted card renders its `<img>` at once; an `<img>` error drops the entry and falls back once.
  - Keep `watchNear`, `IMAGE_NEAR_MARGIN` and both lanes.
- **Warmer** (`visibleWarm.ts`): take the index from `data-index` instead of the DOM position; prune detached targets; re-observe when the rendered range changes.
- **Views**:
  - `moveSelection` uses `grid.scrollToIndex(focusIndex)`.
  - The warmer effect also depends on the rendered range.
  - Absolute indexes go to `focused`, `onOpen`, `index` and the unlinked-row test IDs.
- **Accessibility**: no `role="grid"`. Tab order covers rendered cards; buffer rows scroll in as focus reaches them.

## Steps (tests first; gate; `git commit --only`)
1. `window.ts` and the `size.ts` changes.
2. Image limiter hold-until-settled and the path memo.
3. Warmer.
4. GameCard `index`.
5. Windowed CardGrid (SSR tests: 3 items render all with zero padding; 2500 items render `INITIAL_WINDOW_CARDS`).
6. Server.
7. Library.
8. Update `e2e/specs/large-platform.spec.ts`:
   - Test 1: the DOM has at most 200 cards, `game-card-12500` is absent, `scrollHeight` matches the full list, and the first cover loads.
   - Test 2: keep the 2.5 s cover-after-scroll check; at the end the last card renders and the DOM still has at most 200 cards.
   - Optional test 3: scrolling back to the top shows `game-card-10001`'s cover at once.
9. Update the `.card` comment in GameCard and ARCHITECTURE.md (`cards/window.ts`).

## Verification
- Gate, then the full e2e suite (CardGrid touches every grid).
- The rAF probe on the 2500-game platform, target scroll and hover avg under 25 ms. Also:
  - the DOM card count while scrolling stays at most about 120;
  - Small and Large card sizes;
  - a 300-card platform shows no regression from the ~28 ms/frame noted in GameCard.

## Risks (accepted)
- A fast scrollbar drag can show padding for a frame.
- Covers far from the screen no longer pre-load.
- If a focused card button is wheeled far off screen, it unmounts and focus drops to `body`; arrow keys still work.
- Content above the grid that changes without a scroll is absorbed by the buffer until the next recompute.
