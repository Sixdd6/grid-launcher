# Plan: look and feel rework (readability, surfaces, cover-only cards)

## Why
The design critique of 2026-10-30 measured the real app against a live server
(`scripts/e2e.sh ui-screens`, screenshots in `ctr-out/ui-screens/`):
- Text sits straight on the background art. In light mode at 60% fade every
  piece of secondary text fails WCAG AA (2.7–3.6:1), and the search
  placeholder fails at every fade (2.4:1). Dark mode passes only because the
  sampled art was dark; nothing guarantees it.
- `--surface` is 7% white (dark) or 3.5% black (light): over art the search
  field, chips and selects nearly vanish, and nothing shows what belongs
  together.
- Covers are boxed in a 3:4 frame with a blurred band, and the details
  overlay crops them.

## Decisions (user, 2026-10-30)
- Direction from the clickable mockup (`ctr-out/ui-direction/index.html`):
  frosted panels over the art, cover-only cards, an opaque details overlay.
- **Game labels** is a display setting with two values:
  - **Always visible**: each title has its own small frosted label under the
    cover (mockup option B).
  - **Visible on hover**: covers only; the title shows on hover or keyboard
    focus (mockup option C).
- The fade controls only the art's strength; panels keep a fixed opacity
  floor. Light and dark follow the same rules. (No preference given; these
  were the recommendations.)

## Design rules
- **Three levels.** Canvas (the art at the fade's strength), panels, overlays.
  - A **panel** is the theme's panel colour at a fixed floor of 0.86 with an
    18px backdrop blur and a 1px hairline. Text only ever sits on a panel, a
    field, a filled control or a label, never on raw art. At 0.86 the worst
    case (white art in dark mode, black art in light mode) keeps muted text
    above 4.5:1.
  - An **overlay** (details, dialogs, menus) is opaque, has a shadow and sits
    over a scrim; no coloured border.
- **Layout.** The top bar spans the window; below it, the rail and the view
  are separate rounded panels with a 12px gutter, so the art shows between
  them.
- **One selected style** (a primary tint with a 3px inset bar for rows, the
  primary fill for the view pills) and **one focus ring** (`--focus`, 2px,
  offset 2px) everywhere.
- **Fields** (search, selects, inputs) have a visible fill and a stronger
  hairline; placeholders use `--text-muted`.

## Cards
- A card is the cover at its own shape, fitted inside a uniform cell
  (width × 4/3) and aligned to the cell's bottom edge, so rows line up. The
  image itself has the rounded corners and a soft shadow; no frame, no
  backdrop.
- Badges sit on the cover's own corners (installed dot top-right, UPDATE /
  MISSING top-left, cloud bottom-right). The platform chip shows only where a
  grid mixes platforms (Library), not in the single-platform Server grid.
- Hover and focus show a bottom bar on the cover: Play/Install, Cloud sync,
  More. It stays in the cover's bottom 38px, which keeps the card root's
  centre free for the specs' Details clicks for every cover shape and size
  (`size.test.ts`).
- Label modes change the row height the windowed grid uses
  (`cards/window.ts`): a label row for Always visible, none for Visible on
  hover (the label floats over the gap below the cover).

## Steps (each gated and committed)
1. **Tokens and the panel primitive** — `app/src/app.css` (+ `app.css.test.ts`):
   panel, overlay, line, field, scrim, shadow, selected and focus tokens for
   both themes; a `.panel` class; placeholder colour. The `.over-art` text
   halo is kept only for the few labels that still sit on art until step 3.
2. **Shell and rail** — `Shell.svelte`, `RailPane.svelte`: top bar as a
   panel, rail and view as floating rounded panels with the 12px gutter; the
   unified selected and focus styles.
3. **Views** — `Server.svelte`, `Library.svelte`, `Emulators.svelte`,
   `Settings.svelte`, `Downloads.svelte` and the panes they host: header and
   toolbar in one panel band; lists and forms on panels.
4. **Game labels setting** — grid-core `config::UiSettings` gains
   `card_labels` (`always` | `hover`, default `always`, normalised like the
   card sizes); the Tauri get/set; `api.ts` type; the `uiSettings` store; a
   "Game labels" select on Settings › Appearance. Rust and vitest tests.
5. **Cover-only cards** — `GameCard.svelte`, `CardGrid.svelte`,
   `cards/size.ts`, `cards/window.ts`, `cards/badges.ts` (+ tests): the cell,
   the cover, the bottom hover bar, both label modes, the platform-chip rule.
6. **Remove the letterbox backdrop pipeline**, now unused: `letterbox.ts`,
   `Image.svelte`'s `backdrop`, `imageLoad.ts`'s `backdropPaths`,
   `api.ensureCardBackdrop`, its Tauri command and
   `grid-core/src/images/backdrop.rs`.
7. **Details overlay and dialogs** — `Details.svelte` and the dialogs: the
   overlay level, the whole cover (`object-fit: contain`, no crop).
8. **Verify** — the gate; the full e2e suite (user-visible flows);
   `ui-screens` against the live server, with the contrast measured again at
   fade 0/30/60 in both themes; SPEC (cards, Appearance); the design system
   artifact (tokens, Panel, GameCard, Game labels).

## Not covered (say if wanted)
- A masonry grid (covers at full column width, variable height): it breaks
  row-based windowing and keyboard row movement.
- New typography or a new accent colour.

## Follow-up: cards sized by their artwork (user decisions, 2026-10-30)
Arcade art mixes flyers (0.73), square art and marquees up to 1.86:1, so one
cell shape per grid still leaves cards that are not the size of their art. The
card is now a universal element sized to its artwork, in one of two layouts,
chosen in Settings › Appearance › **Game card display**:
- **Equal height** (default): justified rows. Every row is one height; each
  card is as wide as its art at that height, and the row is stretched to the
  full width (the last row is not).
- **Equal width**: masonry columns. Every column is one width; each card is as
  tall as its art; a card goes in the shortest column.

Rules:
- **Shape source.** A cover's shape is known once it loads. Until then it is
  3:4. Shapes are cached per cover URL and persisted (localStorage, capped), so
  a platform seen before lays out final at once. Shapes are clamped to 0.5–2.
- **Equal height.** Target row height is the card size's 3:4 height (Small 160,
  Medium 213, Large 267). A row breaks where its height lands closest to the
  target, and never below 75% or above 130% of it (a row that would pass 130%
  stays at 130% and is left-aligned).
- **Equal width.** Column width comes from the card size's minimum width.
  A cover wider than the column can show at 112px tall spans two columns, so no
  cover is shorter than 112px.
- **Click safety.** Every cover is at least 112px tall, which keeps the hover
  bar (bottom 38px) clear of the card root's centre with a label row
  (`size.test.ts`).
- **Windowing.** The layout is a pure function from shapes and width to one
  rectangle per card. Cards are absolutely positioned; only those whose
  rectangles meet the visible part plus a buffer are rendered. The container
  height is the layout's height.
- **Keyboard.** Left and right step in reading order. Up and down move to the
  nearest card above or below that overlaps horizontally (the same column in
  Equal width, the nearest card of the next row in Equal height).
- Replaces the per-grid cell shape (`cards/shape.ts`, `coverShapes`), which
  this supersedes, and `cards/window.ts`'s row arithmetic.
