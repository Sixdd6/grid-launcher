# Ruling — text legibility over the background art

Date: 2026-09-05. Read-only design ruling for `rewrite/app/src`. No files were changed.

Scope of the finding: with blur 2 and fade 50%, the art layer is a recognisable image at
half opacity behind every view. Text painted directly on it loses contrast.

---

## 1. Inventory — what sits directly over the art

**Structural fact:** `BackgroundArt.svelte` is `position: fixed; inset: 0; z-index: -1`,
mounted in `Shell.svelte` above all five view roots. **No view root, rail, toolbar, grid
or pane sets a background.** The only opaque boxes in the shell are the top bar, the
download footer strip, the Details popup and a handful of form controls. Everything else
in the five views is text on art.

`--surface` is translucent in both themes (`rgba(255,255,255,.07)` dark,
`rgba(0,0,0,.035)` light). Rows and chips that use it are **not** a surface for contrast
purposes — the art shows through at ~93%/96.5%. Only `--surface-2` (`#14141f` / `#ffffff`)
and `--bg` are opaque.

### Over the art — needs the fix

| File | Selector | Text |
|---|---|---|
| `lib/Shell.svelte` | `.error-line` | session error line (`--danger`) |
| `lib/RailPane.svelte` | `.rail-heading` | "PLATFORMS" style section headings (`--text-muted`) |
| `lib/RailPane.svelte` | `.rail-item` | rail rows, inactive (`--text-muted`) |
| `lib/RailPane.svelte` | `.rail-item.active`, `.rail-item:hover` | active row (`--text-h` on translucent `--surface`) |
| `lib/RailPane.svelte` | `.rail-count` | count badges (`--text-muted`) |
| `lib/Library.svelte` | `.control` | "Sort" / "Size" toolbar labels (`--text-muted`) |
| `lib/Library.svelte` | `.empty` | empty-state line (`--text-muted`) |
| `lib/Library.svelte` | `.error` | launch error (`--danger`) |
| `lib/GameCard.svelte` | `.title` | card titles under every cover (`--text-h`) — highest-count case |
| `lib/Server.svelte` | `.platform-header h2` | 20px view heading (`--text-h`) |
| `lib/Server.svelte` | `.counts` | "N games · M installed" (`--text-muted`) |
| `lib/Server.svelte` | `.chip`, `.chip.link` | header chips (`--text-muted` on translucent `--surface`) |
| `lib/Server.svelte` | `.header-error`, `.error`, `.banner-error` | errors (`--danger`) |
| `lib/Server.svelte` | `.control`, `.empty`, `.offline` | labels, empty state, offline line |
| `lib/Server.svelte` | `.library-banner` | banner text (translucent `--surface`) |
| `lib/Emulators.svelte` | `h2`, `h3` | pane and section headings (`--text-h`) |
| `lib/Emulators.svelte` | `.muted`, `.hint`, `.note`, `.meta`, `.args`, `.path` | captions (`--text-muted`) |
| `lib/Emulators.svelte` | `.name`, `.platform-name` | row titles (`--text-h`, translucent `--surface` row) |
| `lib/Emulators.svelte` | `.defaults-field-label` | field labels (`--text-muted`) |
| `lib/Emulators.svelte` | `.tabs button`, `.tabs button.active`, `.add-btn`, `.section-header` | tab and add-button labels |
| `lib/Emulators.svelte` | `.error` | errors (`--danger`) |
| `lib/Settings.svelte` | `h2` | pane heading (`--text-h`) |
| `lib/settings/AppearancePage.svelte` | `.field`, `.field label`, `.value` | labels and the slider read-out (`--text-muted`) |
| `lib/settings/CloudSavesPage.svelte` | `label`, `.muted`, `.hint`, `.error` | form labels and captions |
| `lib/settings/ConnectionPage.svelte` | `.row`, `dt`, `dd`, `.error` | definition rows (`dt` is `--text-muted`) |
| `lib/settings/RetroAchievementsPage.svelte` | `label`, `.muted`, `.hint`, `.error` | form labels and captions |
| `lib/settings/UpdatesPage.svelte` | `.line`, `.muted`, `.update-line` | version lines and captions |
| `lib/Downloads.svelte` | `.head h1`, `.legend`, `.graph-key`, `.key-item` | view heading and legend |
| `lib/Downloads.svelte` | `.seg-head`, `.seg-count`, `.seg-empty` | segment headings and empty lines |
| `lib/Downloads.svelte` | `.title`, `.platform`, `.detail`, `.kind`, `.graph-caption`, `.row-error` | row text (translucent `--surface` row) |

### Already on an opaque surface — change nothing

| File | Selector | Surface |
|---|---|---|
| `lib/Shell.svelte` | `.topbar` and everything in it (`.wordmark`, `.pill`, `.chip`, `.update-badge`) | `--surface-2` |
| `lib/Shell.svelte` | `.server-menu`, `.menu-host` | `--surface-2` |
| `lib/DownloadsFooter.svelte` | whole strip | `--surface-2` |
| `lib/Details.svelte` | `.panel` and all of `lib/details/*` | `--bg` behind a `rgba(0,0,0,.55)` + blur backdrop |
| `lib/Connect.svelte` | all | rendered only when `session.phase === 'none'`, where `BackgroundArt` is not mounted |
| `lib/Toast.svelte` | toasts | own fill |
| everywhere | `input`, `select`, `textarea`, `.search`, `.catalog-search` | `--surface-2` |
| everywhere | filled buttons (`background: var(--primary)` / `var(--danger)` / `rgba(0,0,0,.55)`) | own fill |
| `lib/GameCard.svelte` | `.tag`, `.actions button`, `.primary`, `.cloud-badge` | own fill, and they sit on the cover, not the art |

---

## 2. Approach — (a) + (b), not (c)

**Ruling: raise the two muted greys as tokens, and add one theme-flipped halo token
applied through a single `.over-art` utility class on the five view roots.**

Colour alone cannot fix this: a fully white blurred patch at 50% over the dark theme's
`#07070f` composites to `#838387`, where even pure white text scores **3.78:1** — below
4.5, and **2.74:1** once the slider reaches 60%. The light theme mirrors it: a black
screenshot at 50% over `#f5f5fa` composites to `#7a7a7d`, where `#111117` scores
**4.40:1**, dropping to **3.09:1** at 60%. No solid colour exists that clears AA against a
background that can be any luminance, so option (a) alone is arithmetically impossible and
option (c) — a scrim — is the only alternative that would work, but a scrim behind every
heading, caption and card title reintroduces the opaque plates the redesign removed and
hides the art the user just asked to make more visible. The halo wins because its colour
is the theme's own `--bg`, so it composites to nothing on an opaque surface and to nothing
when the fade is 0, costs no layout, has zero offset (a contrast plate, not a drop shadow,
so the flat look holds), and pins a **16.7:1 to 17.3:1** band around every glyph in the
dark theme's worst case and **15.6:1 to 15.8:1** in the light theme's. The token raise
rides along because it is free on the opaque surfaces and lifts the mid-luminance art case
(a `#c0c0c0` patch at 50%) from 2.14:1 to 3.58:1 in dark and 2.39:1 to 3.76:1 in light,
which is where most real screenshots land.

---

## 3. Tokens

### 3.1 Changed values

`app.css` has four token blocks. Apply as follows.

**`:root`** (the light base, lines 6–66):

```css
  --text-muted: #3d3d52;   /* was #5a5a70 */
  --danger: #c62828;       /* was #ff5050 — light only, see 3.2 */
  --text-halo:
    0 0 2px rgba(245, 245, 250, 0.92),
    0 0 8px rgba(245, 245, 250, 0.6);
```

**`@media (prefers-color-scheme: dark) { :root:not([data-theme='light']) }`** (lines 68–82)
and **`:root[data-theme='dark']`** (lines 84–96) — identical in both:

```css
  --text-muted: #c8c8dc;   /* was #9a9ab0 */
  --text-halo:
    0 0 2px rgba(7, 7, 15, 0.85),
    0 0 8px rgba(7, 7, 15, 0.5);
```

(`--danger` stays `#ff5050` in dark. It is not restated in the dark blocks today and does
not need to be.)

**`:root[data-theme='light']`** (lines 98–109) — restate all three so the explicit
override wins:

```css
  --text-muted: #3d3d52;
  --danger: #c62828;
  --text-halo:
    0 0 2px rgba(245, 245, 250, 0.92),
    0 0 8px rgba(245, 245, 250, 0.6);
```

`--text` and `--text-h` are **unchanged**: they are already `#ffffff` and `#111117`, the
maximum either theme has. The finding's "greyish text" is `--text-muted` and nothing else.

### 3.2 `--danger` in light mode

`#ff5050` scores **2.97:1** on `#f5f5fa` — it fails AA on the light theme's plain
background today, art or no art. `#c62828` scores **5.17:1** on `--bg` and **5.62:1** on
`--surface-2`, and stays the same hue family. This follows the precedent already in §4
("primary darkens in light mode"). If the reviewer prefers to treat this as out of scope,
drop it; the halo still carries the error text over art, but the flat-background failure
remains.

### 3.3 The utility class

Add to `app.css`, after `.view-content`:

```css
/* The five view roots paint straight onto the fixed background-art layer —
   nothing between them sets a background. The halo is the theme's own `--bg`
   at zero offset, so it composites to nothing on an opaque surface and to
   nothing when the fade is 0; over art it holds a ~2px high-contrast band
   at every glyph edge. Not a drop shadow: no offset, no colour of its own. */
.over-art {
  text-shadow: var(--text-halo);
}

/* Anything that carries its own fill opts out — the halo is for text on the
   art, and inside a filled control it would only bloom the label. */
.over-art input,
.over-art select,
.over-art textarea,
.over-art .primary,
.over-art .tag,
.over-art .actions button,
.over-art .form-actions button,
.over-art .row-actions button,
.over-art .library-banner button,
.over-art .catalog-row button,
.over-art .ps3-firmware button,
.over-art .chip button,
.over-art .offline button,
.over-art .update-line button,
.over-art .browse-secondary {
  text-shadow: none;
}
```

`app.css` is imported globally in `main.ts`, so these descendant selectors reach into
Svelte-scoped components without a `:global()` wrapper. No component sets `text-shadow`
today, so nothing is overridden.

### 3.4 Where the class goes — six edits, six files

| File | Element | New class |
|---|---|---|
| `lib/Library.svelte:249` | `<section data-testid="library-section" class="library">` | `class="library over-art"` |
| `lib/Server.svelte:396` | `<section data-testid="server-section" class="server">` | `class="server over-art"` |
| `lib/Emulators.svelte:503` | `<section data-testid="emulators-view" class="emulators">` | `class="emulators over-art"` |
| `lib/Settings.svelte:45` | `<section class="settings" aria-label="Settings">` | `class="settings over-art"` |
| `lib/Downloads.svelte:71` | `<section class="downloads view-content">` | `class="downloads view-content over-art"` |
| `lib/Shell.svelte:190` | `<p data-testid="session-error" class="error-line">` | `class="error-line over-art"` |

`text-shadow` inherits through the DOM, so the class on `.library` and `.server` also
covers `RailPane.svelte` and every `GameCard.svelte` title inside them without touching
either component. No `data-testid` changes, so the E2E ids in §11 are untouched.

### 3.5 Contrast ratios

All figures are sRGB WCAG 2.x ratios, computed at fade 50% and fade 60% (the slider's
maximum).

**Changed tokens, on opaque surfaces (no art):**

| Token | Theme | On `--bg` | On `--surface-2` | Before |
|---|---|---|---|---|
| `--text-muted #c8c8dc` | dark | 12.19:1 | 11.09:1 | 7.29 / 6.63 |
| `--text-muted #3d3d52` | light | 9.72:1 | 10.56:1 | 6.17 / 6.71 |
| `--danger #c62828` | light | 5.17:1 | 5.62:1 | 2.97 / 3.22 |

Hierarchy is preserved: dark muted 12.19 still reads as secondary against `--text` at
20.07; light muted 9.72 against 17.31.

**Worst-case art, flat (halo band ignored):**

| Case | Composite | `--text` | `--text-muted` (new) | `--text-muted` (old) |
|---|---|---|---|---|
| dark, white screenshot, 50% | `#838387` | 3.78:1 | 2.29:1 | 1.37:1 |
| dark, white screenshot, 60% | `#9c9c9f` | 2.74:1 | 1.66:1 | 1.01:1 |
| dark, mid `#c0c0c0`, 50% | `#646468` | 5.89:1 | 3.58:1 | 2.14:1 |
| light, black screenshot, 50% | `#7a7a7d` | 4.40:1 | 2.47:1 | 1.57:1 |
| light, black screenshot, 60% | `#626264` | 3.09:1 | 1.74:1 | 1.10:1 |
| light, mid `#404040`, 50% | `#9a9a9d` | 6.70:1 | 3.76:1 | 2.39:1 |

**Worst-case art, with the halo band** — the ~2px ring the inner shadow lays down at
alpha 0.85/0.92, which is the surface the eye actually resolves the glyph edge against:

| Case | Halo band | `--text` | `--text-muted` (new) | `--danger` |
|---|---|---|---|---|
| dark, white screenshot, 50% | `#1a1a21` | 17.30:1 | 10.51:1 | 5.37:1 |
| dark, white screenshot, 60% | `#1d1d25` | 16.74:1 | 10.16:1 | 5.19:1 |
| light, black screenshot, 50% | `#ebebf0` | 15.83:1 | 8.89:1 | 4.73:1 |
| light, black screenshot, 60% | `#e9e9ee` | 15.55:1 | 8.73:1 | 4.65:1 |

Reasoning about the worst cases. A bright, low-blur screenshot in the dark theme is the
harder of the two, because blur 2 keeps small near-white specular patches intact and a
card title can land on one. Flat, that patch beats every possible text colour. The halo
replaces the local background with `#07070f` at 85% for the first ~1px around each stroke
and at 50% out to ~4px, so the glyph edge resolves against `#1a1a21`, and the wider
8px pass stops the transition from reading as a hard sticker outline. In the light theme
a dark screenshot at 60% is the worst point; the same construction inverted puts the glyph
edge on `#e9e9ee`, one step off `--bg`, at 15.55:1. Both cases clear AA for body text with
margin, and the 8px pass is what makes the result read as a soft vignette rather than the
heavy drop shadow the flat look forbids.

---

## 4. Do not change

- **Card surfaces.** `GameCard.svelte`'s `.cover` (`--surface-2`), `.overlay` gradient,
  `.tag`, `.actions button`, `.primary`, `.dot`, `.cloud-badge` all carry their own fill
  on the cover image, not the art. `.title` is the only card text that changes, and it
  changes only by inheriting the halo.
- **Popups.** `Details.svelte` (`.panel` on `--bg`, `.backdrop`), `MediaViewer.svelte` and
  all of `lib/details/*`. They sit above the art on an opaque panel and are outside the
  `.over-art` roots.
- **The top bar and the footer strip.** `Shell.svelte`'s `.topbar`, `.pills`, `.pill`,
  `.chip`, `.update-badge`, `.server-menu`, `.menu-host`, and `DownloadsFooter.svelte` —
  all on `--surface-2`. The `.over-art` class goes on `.error-line` only, not on the
  header.
- **Buttons.** No button changes colour, fill, radius, padding or hover state. Filled
  buttons only opt out of the inherited halo through the reset list in §3.3.
- **`Connect.svelte`.** No `BackgroundArt` is mounted at that phase.
- **`--surface`, `--surface-2`, `--border`, `--bg`, `--primary*`, `--secondary`,
  `--accent-warm`, `--favourite`, `--success`, `--warning`, `--info`, `--graph-disk`.**
  Unchanged.
- **Motion tokens** `--m-fast` / `--m-base` / `--m-slow`, and every radius. Unchanged.
- **`BackgroundArt.svelte`.** The blur, fade, tier order, 5s rotation and 360ms cross-fade
  are the user's rulings and stay as they are. The halo does not need to react to the fade
  value, because it is the background colour: at fade 0 it is invisible by construction.
- **No new dependency, no new component, no `data-testid` change.**

---

## 5. Spec amendment

Add to **§4 Theme tokens**, as a new bullet after the Light bullet:

> Text over the background art: the five view roots carry the global `.over-art` class
> from `app.css`, which sets `text-shadow: var(--text-halo)` — `0 0 2px` plus `0 0 8px` of
> the theme's own background colour (`rgba(7,7,15,.85)/.5` dark,
> `rgba(245,245,250,.92)/.6` light) at zero offset, so it composites to nothing on an
> opaque surface or at fade 0 and holds a ≥15:1 band at every glyph edge over the
> brightest or darkest art at 60% fade; elements with their own fill (inputs, selects and
> filled buttons) opt out through the companion reset list. `--text-muted` is `#c8c8dc`
> dark and `#3d3d52` light, and `--danger` darkens to `#c62828` in light mode, because the
> published `#ff5050` scores 2.97:1 on `#f5f5fa` (added 2026-09-05, background-contrast
> ruling).
