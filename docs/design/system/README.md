GRID Launcher is a desktop game launcher for a RomM server. The game's own art is the brand: the interface is a quiet violet-and-ink frame that sits on top of it and never competes with it.

## Principles

- **The art is the canvas.** Behind every view sits the cover of the last game viewed, blurred, at the fade set in Settings › Appearance. Covers are shown whole, at their own shape, never cropped or boxed.
- **Text never sits on raw art.** Every word sits on a **panel** (`panel`, a 92% floor in both themes) or an **overlay** (`overlay`, opaque, with `shadow-overlay`). The fade changes the art only, never those floors, so text keeps WCAG AA on any art.
- **One selected style, one focus ring.** A selected row is `selected` with a 3px inset bar in `primary`. The selected pill or segment is a `primary` fill with `on-primary` text. Keyboard focus is always a 2px `focus` outline at a 2px offset.
- **Cheap at 4K.** No backdrop blur on panels (`panel-blur: none`). Covers at rest carry `shadow-card`, a hairline only. The blurred `shadow-lift` is for a lifted (hovered or focused) cover.

## Surfaces

Use three levels, in this order:

1. **Art**: the blurred cover layer. It carries no text.
2. **Panel**: the top bar, each rail, a view's header and toolbar, lists and forms. Fill `panel`, edge `line`, corners `r-panel`. Panels sit a `gutter` (12px) apart, so the art shows between them.
3. **Overlay**: Details, dialogs, menus, hover labels, toasts. Fill `overlay` (or `surface-2` for toasts and the download strip), edge `line`, shadow `shadow-overlay` or `shadow-lift`. A modal overlay sits over `scrim` with a 6px blur.

Inside a panel or overlay, group things with `surface` (a faint fill) and `line` hairlines. Never put a `panel` inside a `panel`.

## Colour

- `primary` (#553e98 light, #8b74e8 dark) is the one brand hue. Use it for the filled action (Play, Install), the active view pill, the active tab underline, the selected-row bar and the brandmark. Hover is `primary-hover`; pressed is `primary-pressed`.
- Text on a `primary` or `danger` fill is `on-primary` or `on-danger`: white in light, the dark background in dark, where white falls to about 3.6:1.
- Body text is `text`; headings and active labels `text-h`; secondary text `text-muted`. All three hold 4.5:1 on `panel`, `overlay` and `surface` in both themes.
- Status colours are marks, not words. `success` is the installed dot and the online dot. `warning` fills the UPDATE tag (with #1a1a12 text). `danger` marks errors and destructive outlines. Every status also has a word, a tooltip or an `aria-label`.
- Tags drawn on cover art use fixed fills, not theme tokens: black at 70% with white text, and MISSING on #c62828 with a white ring. They sit on the art, not on a themed surface.
- `secondary`, `accent-warm`, `favourite`, `info` and `graph-disk` are accents. Keep them off text on light grounds.

## Type

- One family, `sans` (Segoe UI, then the system UI face). There are no bundled font files.
- The base is `body`, 13px at 1.45. Use `meta` (12px) for counts, meta lines and card labels, and `small` (11px) for rail counts and captions.
- `view-title` (20px, 600) names the platform in a view header and the game in Details. There is one per view.
- `description` (14px at 1.65, at most 68ch) is for long server text: break it into paragraphs.
- `rail-heading` and `tag` are uppercase with tracking (0.1em and 0.06em). Fact labels are `small`, uppercase, 0.05em.
- The wordmark is "GRID" in `wordmark` (15px, 600, 0.08em), in `text-h`, beside the brandmark in `primary`.

## Spacing, radius, size

- `gutter` (12px) is the space between and around shell panels. Inside panels, use 14–16px padding and 8–12px gaps.
- Use the radius that matches the job: `r-control` 4 (inputs, dialog buttons), `r-chip` 6 (icon buttons, cover tags), `r-row` 8 (rows, menus, toasts, labels), `r-card` 10 (covers), `r-dialog` 12 (modals), `r-panel` 14 (shell panels) and `r-pill` (pills, chips, switches, the card Play button).
- Every control in a toolbar row is `control-h` (32px) tall, so search fields and selects share their edges.
- The shell is fixed: `topbar-h` 58px, a 220px rail, `footer-h` 28px for the download strip. Lists cap at 1100px and centre. Grids run full width.
- The minimum pointer target is 28×28 (the icon button).

## Motion

Three durations: 150ms for hover and colour changes, 220ms for the background cross-fade and toasts, 360ms for larger moves. Everything eases. A hovered or focused cover lifts 4px.

## Interaction

- Hover and keyboard focus look the same on a card: the cover lifts, the action bar shows (Play or Install, Cloud sync, Details), and the full title wraps below the cover.
- Every button has a hover state: a `primary-hover` fill on primary buttons, a `surface` fill on secondary and icon buttons, and `text-h` on quiet pills and tabs.
- A destructive action takes two clicks. The first changes the label to "Confirm …" in `danger`.
- A segmented switch (Saves | States) picks a view. A button that toggles between two views reads as broken, so it is never used for that.
- Views stay mounted and switch with Ctrl+1 to Ctrl+5. Escape closes Details and menus. Ctrl+F focuses the view's search field.

## Content and voice

- Use sentence case for labels and buttons: "Install firmware", "Open Downloads", "Remove from library (keeps files)".
- Put the verb first and name the effect. Give the reason in brackets when it matters.
- Status lines use a middle dot as separator: "48 games · 6 installed", "Starfall Circuit · 64% · 38.2 MB/s", "PlayStation firmware · 3 files".
- Progress words end with an ellipsis: "Installing…", "Requested…", "Checking…".
- Errors say what happened and what to do, in plain words: "Play and Uninstall are unavailable until the files are found."
- No emoji. The ✓ before "Installed" is the `check` icon, not a character.

## Iconography

- The app's own icon set is in `assets/Icons/`: 11 marks drawn on one 24×24 grid with a 1.5-unit round stroke. `star`, `play` and `more` are filled.
- In the app, icons paint with `currentColor`, so an icon is always the colour of its text. The SVG files here are drawn in `text` (#111117) because `<img>` cannot inherit colour.
- Sizes: 20 in icon buttons and the brandmark, 14–15 inline and on cards.
- The brandmark is the `grid` icon: four 6×6 cells. The app icon (`assets/Logos/`) is the coloured tile and trophy, for the OS and installers only.

## Components

Everything here is a static rendition of the app's Svelte components, styled by `components/bundle.css` (classes prefixed `g-`). **Current app** puts them together as a clickable prototype of the app as it is today. **Proposed app** is the same shell with the changes under discussion; read its notes for what differs.

## Not synced

- From `app/src/app.css`: `--panel-blur` (always `none`), `--text-h` (kept, same values as `text`), and the motion durations (`m-fast`, `m-base`, `m-slow`; this format has no motion family, so they are listed under Motion above).
- `--sans` is `type.families.sans`, so it is served as `--font-sans`.
- There are no font files: the app uses the system's Segoe UI or UI face.
- Components are hand-written static renditions from the Svelte sources, not a built bundle: GRID is a Svelte app with no React component library.
