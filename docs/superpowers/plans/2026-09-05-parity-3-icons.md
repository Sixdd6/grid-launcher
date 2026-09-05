# Parity 3 — one inline-SVG icon system, no glyphs-as-icons

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace all thirteen Unicode-glyph-as-icon uses in the rewrite UI with one `Icon.svelte` component backed by a pure path-data module, so every icon is legible, consistently sized, theme-coloured and accessible. The user's requirement: "iconography should be clear and have proper aspect and dimensions to make it clearly legible; this rewrite should be the more polished version."

**Architecture:** Two new files carry the whole system. `lib/icons.ts` is a pure module holding nine hand-authored path strings on one fixed 24×24 grid — it is unit-tested like every other pure module in `lib/`. `lib/Icon.svelte` is a thin inline-SVG shell around it, modelled on the app's one existing correct SVG (`lib/downloads/Sparkline.svelte`): explicit `width`/`height`, `viewBox`, `currentColor` only, `display: block; flex: none`, and the ARIA split (`aria-hidden` when paired with text, `role="img"` + `aria-label` when the icon is the only label). One global `.icon-btn` class in `app.css` collapses the four hand-rolled, already-drifted icon-button CSS blocks onto a single 28×28 target. Every call site keeps its existing test id and its existing colour token; no call site gains a colour literal. Two call sites (the header rating star, the downloads footer arrow) currently live inside a *string* returned by a pure function — those two move the mark out of the string into markup, which changes the element's text, so each is landed together with its unit tests and its E2E assertion in one task.

**Tech Stack:** Svelte 5 runes + TypeScript + vitest, WebdriverIO E2E against the mock RomM server. No Rust changes in this plan.

**Spec:**
- `/tmp/claude-1000/-home-six-Documents-Programming-grid-launcher/d527a4be-8a2d-487c-bc02-e067fbdcf4ce/scratchpad/research-icons.md` — the iconography audit. §1 is the inventory of all thirteen uses, §4 is the design this plan implements, §4.5 lists the two blocking E2E assertions.
- `docs/superpowers/specs/2026-09-04-desktop-ui-redesign-design.md` — §4 theme tokens (the only colour source), §5 library cards, §7 details popup, §11 test ids. Task 7 adds the icon system to §4 and §11.

All paths below are relative to `rewrite/` unless they start with `docs/`.

## User decisions / rulings (binding)

1. **One component, one grid.** Every icon is a `<path>` on `viewBox="0 0 24 24"`. The old app's `assets/svg/` files are **not** reused: their viewBoxes run from `-0.5 0 7 7` to `0 0 1000 1000` (audit §2), so dropping them into a fixed grid gives visibly different optical weight per icon. The nine paths are authored fresh in this plan. **Nothing under `assets/` is read, copied, moved, edited or deleted by any task.**
2. **No colour literal anywhere in the icon system.** The `<svg>` carries `stroke="currentColor"` (or `fill="currentColor"` for the two solid marks); colour comes from the parent's `color`, which is a token at every call site.
3. **Solid vs outline.** `star` and `play` are solid (`fill="currentColor" stroke="none"` on the path). Every other icon, **including `cloud`**, is a 1.5-unit outline. The audit left `cloud` open; an outline is chosen because the cloud sits at 14px on a dark card scrim where a solid blob loses its silhouette, and because eight outline icons plus two solid ones read as one system. The solid set is exported from `icons.ts` as `FILLED_ICONS` so it is unit-testable rather than hidden inside the component.
4. **Size scale: 14 / 16 / 20. No other value.** 14 inline with 12–13px text (card cloud badge, header rating star, downloads footer prefix, the small dismiss/remove buttons). 16 default (the CloudPanel back button). 20 for standalone icon buttons and the brandmark.
5. **Minimum icon-only pointer target is 28×28.** The dismiss button (18×18) and the CloudPanel remove button (22×22) grow to it.
6. **`.icon-btn` lives in `app.css`, not per component.** `app.css` is the only file allowed to hold global styles, and this is exactly the case it exists for: `Details.svelte`'s and `NativeSettings.svelte`'s close-button CSS are duplicates that have already drifted (`var(--r-chip)` vs a literal `6px`). The class carries box, radius, reset and `font: inherit`; each component keeps only its own `position`, `color` and `:hover`/`:focus-visible` background, because those genuinely differ (panel `var(--border)` vs media-viewer `rgba(255,255,255,.24)`).
7. **The header rating star moves out of the string.** `ratingText()` is deleted; `headerLine()` keeps every part except the rating; a new `ratingValue()` returns the trimmed number; `Details.svelte` renders the star as an `<Icon>` before it. `details-header-line` therefore reads `… · Platformer · 9.2`. The E2E assertion and the `header.test.ts` cases are updated in the same task. Same shape for the downloads footer `⬇`.
8. **The rating star is `var(--primary)`** (controller ruling 2026-09-05, resolving the open question at the end of this plan: `--warning` is ~1.5:1 on the light theme, `--primary` is legible in both). The old app wrapped the rating in the accent colour (`grid_launcher/ui/game_views.py:582`), so `--primary` is also the closer match. The **number** stays `var(--text-h)`.
9. **The CloudPanel back button drops its `aria-label`.** The visible word "Back" stays and becomes the accessible name, so voice control ("click Back") matches. The arrow icon is `aria-hidden`.
10. **The `#e5484d` / `#e5a53a` literals in `CloudPanel.svelte` and `NativeSettings.svelte` become `var(--danger)` / `var(--warning)`.** `Connect.svelte:96` has the same literal but is outside the audit's scope and is **not** touched.
11. **Parity-2 lands first.** Another plan edits `CloudPanel.svelte` and `NativeSettings.svelte` before this one runs. In those two files every task **must** re-read the file and locate elements by `data-testid` or class, never by the line numbers quoted here.

**Amendment 2026-09-05:** `.svelte` files MAY be tested at the markup level with `render` from `svelte/server` (node environment, no jsdom, no new dependency) — see `Icon.svelte.test.ts`. The no-harness rule still forbids adding a DOM test harness.

## Global Constraints

- **Token secrecy (hard):** tokens live only in the OS keyring and the redacting in-memory type; never in files, logs, errors, IPC, or console output.
- **Only `app.css` tokens for colours**; `--m-*` motion tokens. The existing literal `rgba()` scrims inside a card cover and inside the media viewer's dark overlay are allowed (those files already use them and they are scrims, not palette).
- **Every test id E2E asserts today stays**: `details-close`, `media-viewer-close`, `media-viewer-prev`, `media-viewer-next`, `native-settings-close`, `cloud-back`, `details-warning-dismiss`, `card-cloud-badge-*`, `cloud-native-path-remove-*`, `installed-badge-*`, `details-header-line`, `downloads-aggregate`, `details-media-<i>`, `shell-topbar`. No id is added or removed by this plan.
- **Every task ends with**, from `rewrite/`: `cargo fmt`; `cargo clippy --workspace --all-targets -- -D warnings` and `cargo clippy -p app --all-targets --features e2e -- -D warnings` clean; `cargo test --workspace` green **when Rust changed** (this plan changes no Rust, so the cargo steps are a no-op guard); and from `rewrite/app`: `npm run check` (**record the current warning count before Task 1 starts and treat that as the baseline — no new warnings**) and `npx vitest run` green. Then a commit whose subject starts `rewrite: `.
- **Never** run `git checkout`, `git restore`, `git reset`, or `git stash`. Commit with explicit pathspecs.
- **No component test harness exists** (no `@testing-library/svelte`, no jsdom). `.svelte` changes are verified by `npm run check` and E2E, never by a fabricated component test. All unit tests in this plan are for `.ts` modules.
- **Never** modify, move or delete anything under `assets/`. It is stock artwork and is read-only for this plan; no task needs to read it either.
- The final task runs the E2E groups `images`, `downloads`, `library`, `install`, `cloud-saves`, `native` (`rewrite/scripts/e2e.sh images downloads library install cloud-saves native`, detached, log to a file) and they must be green.

---

## File map

| File | Responsibility |
|---|---|
| `app/src/lib/icons.ts` | **new.** The nine 24-grid path strings, `IconName`, `FILLED_ICONS`. Pure, no imports. |
| `app/src/lib/icons.test.ts` | **new.** Vitest: the name set, path validity, the solid set. |
| `app/src/lib/Icon.svelte` | **new.** The inline-SVG shell: `name`, `size`, `label`. |
| `app/src/app.css` | one `.icon-btn` block — the shared 28×28 icon-only button |
| `app/src/lib/Details.svelte` | close button, warning dismiss button, header rating star |
| `app/src/lib/details/NativeSettings.svelte` | close button, `#e5484d` → `var(--danger)` |
| `app/src/lib/details/CloudPanel.svelte` | remove button, back arrow, `#e5484d`/`#e5a53a` → tokens |
| `app/src/lib/details/MediaViewer.svelte` | close, prev/next chevrons, the missing `translateY(-50%)` |
| `app/src/lib/details/MediaTab.svelte` | play icon on the video tile, `aria-hidden` |
| `app/src/lib/Shell.svelte` | `▦` brandmark → `grid` icon |
| `app/src/lib/GameCard.svelte` | `☁` cloud badge → `cloud` icon |
| `app/src/lib/details/header.ts`, `header.test.ts` | `ratingText` → `ratingValue`; the star leaves `headerLine` |
| `app/src/lib/downloads/format.ts`, `format.test.ts` | the `⬇` prefix leaves `footerLine` |
| `app/src/lib/DownloadsFooter.svelte` | renders the `download` icon before `.line` |
| `e2e/specs/images-a.spec.ts`, `e2e/specs/downloads.spec.ts` | the two text assertions that contain a glyph |
| `docs/superpowers/specs/2026-09-04-desktop-ui-redesign-design.md` | §4 and §11 record the icon system |

---

### Task 1: The icon set, the component, and the shared icon-button class

**Files:**
- Create: `app/src/lib/icons.ts`
- Create: `app/src/lib/icons.test.ts`
- Create: `app/src/lib/Icon.svelte`
- Modify: `app/src/app.css` (append one block at the end, after `.view-content`)

**Interfaces:**
- Produces: `export const ICONS` — nine entries, `Record<IconName, string>` by construction, `as const`.
- Produces: `export type IconName = keyof typeof ICONS`.
- Produces: `export const FILLED_ICONS: readonly IconName[]` — the solid marks (`star`, `play`). Every other name is a 1.5-unit outline.
- Produces: `Icon.svelte` with props `{ name: IconName; size?: number; label?: string }`, `size` defaulting to 16.
- Produces: the global CSS class `.icon-btn` (28×28 icon-only button).

- [ ] **Step 1: Write the failing test** — create `app/src/lib/icons.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import { FILLED_ICONS, ICONS, type IconName } from './icons';

// The nine names the UI asks for. Written out here rather than derived from
// `ICONS` so that deleting an icon a call site still uses fails this test
// instead of silently shrinking the set.
const EXPECTED: IconName[] = [
  'close',
  'chevronLeft',
  'chevronRight',
  'arrowLeft',
  'cloud',
  'star',
  'download',
  'play',
  'grid',
];

// Every SVG path command letter, plus the number/separator characters a
// coordinate can use. Anything else (a colour, a `<`, a stray identifier)
// means the entry is not path data.
const PATH_CHARS = /^M[MmLlHhVvCcSsQqTtAaZz0-9 ,.-]*$/;

describe('ICONS', () => {
  it('has exactly the names the UI asks for', () => {
    expect(Object.keys(ICONS).sort()).toEqual([...EXPECTED].sort());
  });

  it.each(EXPECTED)('%s is a non-empty path string starting with a moveto', (name) => {
    const d = ICONS[name];
    expect(typeof d).toBe('string');
    expect(d.length).toBeGreaterThan(0);
    expect(d.startsWith('M')).toBe(true);
  });

  it.each(EXPECTED)('%s uses only SVG path commands and coordinates', (name) => {
    expect(ICONS[name]).toMatch(PATH_CHARS);
  });

  it.each(EXPECTED)('%s carries at least two drawing commands', (name) => {
    // A single moveto draws nothing. Every icon in the set is a real shape.
    const commands = ICONS[name].match(/[MmLlHhVvCcSsQqTtAaZz]/g) ?? [];
    expect(commands.length).toBeGreaterThanOrEqual(2);
  });

  it.each(EXPECTED)('%s has no scientific notation or NaN', (name) => {
    expect(ICONS[name]).not.toMatch(/e[+-]?\d/i);
    expect(ICONS[name]).not.toContain('NaN');
  });
});

describe('FILLED_ICONS', () => {
  it('is the two solid marks', () => {
    expect([...FILLED_ICONS].sort()).toEqual(['play', 'star']);
  });

  it('only names icons that exist', () => {
    for (const name of FILLED_ICONS) expect(ICONS[name]).toBeDefined();
  });
});
```

- [ ] **Step 2: Run** `npx vitest run src/lib/icons.test.ts` from `app/` — expect a resolve failure (`./icons` does not exist).

- [ ] **Step 3: Create `app/src/lib/icons.ts`** with exactly this content:

```ts
// The app's icon artwork. One pure module so the paths are unit-testable and
// so `Icon.svelte` stays a five-line shell.
//
// Every path is hand-authored on ONE 24×24 grid, optically centred on
// (12, 12), drawn to read at a 1.5-unit stroke. The old PySide6 app's
// `assets/svg/` files are deliberately NOT reused: their viewBoxes run from
// `-0.5 0 7 7` to `0 0 1000 1000`, so re-fitting them would still leave each
// icon a different optical weight. Nothing here references those files.
//
// No colour appears in this module. The component paints with
// `currentColor`, so an icon is always the colour of the text around it.

export const ICONS = {
  /** Two full-length diagonals. Close, and (at 14) dismiss and remove. */
  close: 'M6 6l12 12M18 6L6 18',

  /** Apex at x=8.5, arms to x=15.5, so the mark is centred on x=12. */
  chevronLeft: 'M15.5 5L8.5 12l7 7',

  /** The mirror of `chevronLeft`, same span and same centre. */
  chevronRight: 'M8.5 5l7 7-7 7',

  /** A 15-unit shaft on the centre line with a 6.5-unit head. */
  arrowLeft: 'M19.5 12H4.5M11 5.5L4.5 12l6.5 6.5',

  /**
   * Outline cloud: a flat base at y=17, a 4-radius right lobe, a shallow
   * 5.2-radius top and a 3.3-radius left lobe. Spans x 3.9–20.5, y 6.9–17.
   */
  cloud: 'M6.5 17h10a4 4 0 0 0 0-8h-.6A5.2 5.2 0 0 0 6.7 10.6 3.3 3.3 0 0 0 6.5 17z',

  /**
   * Solid five-point star. Outer radius 9 and inner radius 3.6 about
   * (12, 12), first point at -90° (straight up), then every 36°.
   */
  star:
    'M12 3L14.12 9.09L20.56 9.22L15.42 13.11L17.29 19.28L12 15.6L6.71 19.28L8.58 13.11L3.44 9.22L9.88 9.09Z',

  /** A shaft and head down the centre line into an open tray at y=20.5. */
  download: 'M12 3.5V14M7.5 9.5L12 14L16.5 9.5M4.5 17v1.5a2 2 0 0 0 2 2h11a2 2 0 0 0 2-2V17',

  /** Solid right-pointing triangle; its centroid sits on x=11.3, y=12. */
  play: 'M7.5 4.5L19 12L7.5 19.5Z',

  /** The brandmark: four 7×7 cells with a 2-unit gutter, spanning 4–20. */
  grid: 'M4 4h7v7h-7zM13 4h7v7h-7zM4 13h7v7h-7zM13 13h7v7h-7z',
} as const;

export type IconName = keyof typeof ICONS;

/**
 * The solid marks. Their path takes `fill="currentColor" stroke="none"`;
 * every other icon takes the root's 1.5-unit `stroke="currentColor"`. A
 * filled path must not also take the stroke, or the shape thickens by half a
 * unit on every edge and stops matching the outline icons beside it.
 */
export const FILLED_ICONS: readonly IconName[] = ['star', 'play'];
```

- [ ] **Step 4: Run** `npx vitest run src/lib/icons.test.ts` — green.

- [ ] **Step 5: Create `app/src/lib/Icon.svelte`:**

```svelte
<script lang="ts">
  import { FILLED_ICONS, ICONS, type IconName } from './icons';

  // The app's one icon. Modelled on `downloads/Sparkline.svelte`, which is
  // the only other SVG in the app and already gets this right: an explicit
  // `viewBox` plus explicit `width`/`height`, `display: block` so the mark
  // is a block box that cannot drift on a text baseline the way a glyph
  // does, and `currentColor` so the colour is always the caller's token.
  //
  // `label` decides the ARIA shape. Absent (the icon sits beside visible
  // text, or its button already has an `aria-label`): the SVG is hidden from
  // the accessibility tree so it cannot be announced twice. Present (the
  // icon IS the label): `role="img"` plus the name.
  let {
    name,
    size = 16,
    label = undefined,
  }: {
    name: IconName;
    size?: number;
    label?: string;
  } = $props();

  let filled = $derived(FILLED_ICONS.includes(name));
</script>

<svg
  class="icon"
  viewBox="0 0 24 24"
  width={size}
  height={size}
  fill="none"
  stroke="currentColor"
  stroke-width="1.5"
  stroke-linecap="round"
  stroke-linejoin="round"
  role={label === undefined ? undefined : 'img'}
  aria-label={label}
  aria-hidden={label === undefined ? 'true' : undefined}
  focusable="false"
>
  <path
    d={ICONS[name]}
    fill={filled ? 'currentColor' : 'none'}
    stroke={filled ? 'none' : 'currentColor'}
  />
</svg>

<style>
  .icon {
    display: block;
    flex: none;
    /* The icon never becomes the event target. Every icon in the app sits
       inside a button whose id the E2E suite clicks, and letting the click
       land on the button itself keeps `elementFromPoint`-style hit tests
       (and any future tooltip) pointing at the control, not the artwork. */
    pointer-events: none;
  }
</style>
```

- [ ] **Step 6: Add the shared icon-button class** — append to the end of `app/src/app.css`, after the `.view-content` block:

```css
/* The one icon-only button. Four components had hand-rolled copies of this
   box (18×18, 22×22 and two 28×28) and they had already drifted — one used
   `var(--r-chip)` where its twin used a literal `6px`, and none of the four
   set `font: inherit`. 28×28 is the plan's minimum pointer target; the
   `Icon` inside is 14 or 20. A component adds only what genuinely differs:
   `position`, `color`, and its own hover/focus background. */
.icon-btn {
  width: 28px;
  height: 28px;
  padding: 0;
  display: grid;
  place-items: center;
  font: inherit;
  line-height: 1;
  border: none;
  border-radius: var(--r-chip);
  background: transparent;
  color: inherit;
  cursor: pointer;
}
```

- [ ] **Step 7: Run** from `app/`: `npx vitest run` (all green) and `npm run check` (no new warnings against the recorded baseline).

- [ ] **Step 8: Commit**

```bash
git add app/src/lib/icons.ts app/src/lib/icons.test.ts app/src/lib/Icon.svelte app/src/app.css
git commit -m "rewrite: add the inline-SVG icon set, Icon component and shared icon-button class"
```

---

### Task 2: Close, dismiss and remove buttons — Details, NativeSettings, CloudPanel

Covers audit rows #4, #5, #6, #11 and the colour-literal fix in §4.1. **MediaViewer's close (#7) is in Task 3 instead**, so that file is opened once for its close button, its two chevrons and its `transform` bug together.

**Files:**
- Modify: `app/src/lib/Details.svelte` (the `details-close` button and its `.close` CSS; the `details-warning-dismiss` button and its `.dismiss` CSS)
- Modify: `app/src/lib/details/NativeSettings.svelte` (the `native-settings-close` button, its `.close` CSS, and the two `#e5484d` literals)
- Modify: `app/src/lib/details/CloudPanel.svelte` (the `cloud-native-path-remove-*` button, its `.remove` CSS, and the four `#e5484d` plus one `#e5a53a` literals)

**Interfaces:**
- Consumes: `Icon.svelte` and the `.icon-btn` class from Task 1.
- No test id, no `aria-label`, no prop and no exported function changes. The rendered *text* of every element in this task becomes empty, and no spec reads it (audit §4.5 verified this for all four ids).

- [ ] **Step 1: Re-read both parity-2 files.** Parity-2 edits `CloudPanel.svelte` and `NativeSettings.svelte` before this plan runs. `cat` both files in full and locate each target by `data-testid` / class. The line numbers below are from before parity-2 and are a hint only. Then prove no spec reads these buttons' text:

```bash
grep -rn "details-close\|native-settings-close\|details-warning-dismiss\|cloud-native-path-remove" e2e/specs
```

Expected: only `.click()`, `.waitForExist()`, `.toExist()` and `.toBeDisplayed()` calls. If any `getText()` or `toHaveText` appears on one of them, stop and report NEEDS_CONTEXT.

- [ ] **Step 2: `Details.svelte` — import the component.** Add to the import block at the top of `<script lang="ts">`, next to the other local component imports:

```ts
  import Icon from './Icon.svelte';
```

- [ ] **Step 3: `Details.svelte` — the close button** (~`:493`).

Before:

```svelte
    <button data-testid="details-close" class="close" onclick={onClose} aria-label="Close">×</button>
```

After:

```svelte
    <button data-testid="details-close" class="close icon-btn" onclick={onClose} aria-label="Close">
      <Icon name="close" size={20} />
    </button>
```

- [ ] **Step 4: `Details.svelte` — the close CSS** (~`:759-772`).

Before:

```css
  .close {
    position: absolute;
    top: 8px;
    right: 8px;
    width: 28px;
    height: 28px;
    line-height: 1;
    font-size: 20px;
    border: none;
    border-radius: var(--r-chip);
    background: transparent;
    color: var(--text);
    cursor: pointer;
  }
```

After:

```css
  /* Box, radius and reset come from `.icon-btn` in app.css. Only the
     placement and the colour are this dialog's own. */
  .close {
    position: absolute;
    top: 8px;
    right: 8px;
    color: var(--text);
  }
```

Leave the `.close:hover, .close:focus-visible { background: var(--border); }` rule exactly as it is.

- [ ] **Step 5: `Details.svelte` — the warning dismiss button** (~`:673`).

Before:

```svelte
            <button data-testid="details-warning-dismiss" class="dismiss" onclick={() => sessions.dismissWarning()} aria-label="Dismiss warning">×</button>
```

After:

```svelte
            <button data-testid="details-warning-dismiss" class="dismiss icon-btn" onclick={() => sessions.dismissWarning()} aria-label="Dismiss warning">
              <Icon name="close" size={14} />
            </button>
```

- [ ] **Step 6: `Details.svelte` — the dismiss CSS** (~`:953-965`). The button grows from 18×18 to the 28×28 minimum target.

Before:

```css
  .dismiss {
    flex: none;
    width: 18px;
    height: 18px;
    line-height: 1;
    padding: 0;
    font-size: 14px;
    border: none;
    border-radius: var(--r-control);
    background: transparent;
    color: var(--danger);
    cursor: pointer;
  }
```

After:

```css
  /* Was 18×18 — below the minimum pointer target. `.icon-btn` makes it
     28×28; the 14px icon inside keeps it visually small next to the 13px
     warning text. */
  .dismiss {
    flex: none;
    color: var(--danger);
  }
```

Leave the `.dismiss:hover, .dismiss:focus-visible` rule as it is.

- [ ] **Step 7: `NativeSettings.svelte` — import and close button.** Add `import Icon from '../Icon.svelte';` to the import block.

Before (~`:122`):

```svelte
    <button data-testid="native-settings-close" class="close" onclick={onClose} aria-label="Close">×</button>
```

After:

```svelte
    <button data-testid="native-settings-close" class="close icon-btn" onclick={onClose} aria-label="Close">
      <Icon name="close" size={20} />
    </button>
```

- [ ] **Step 8: `NativeSettings.svelte` — the close CSS** (~`:213-226`). This is the copy that had drifted to a literal `6px`; the shared class ends the drift.

Before:

```css
  .close {
    position: absolute;
    top: 8px;
    right: 8px;
    width: 28px;
    height: 28px;
    line-height: 1;
    font-size: 20px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: var(--text);
    cursor: pointer;
  }
```

After:

```css
  /* Same shape as Details.svelte's close, and now literally the same rules:
     `.icon-btn` in app.css owns the box (this copy had drifted to a literal
     `6px` radius where its twin used `var(--r-chip)`). */
  .close {
    position: absolute;
    top: 8px;
    right: 8px;
    color: var(--text);
  }
```

- [ ] **Step 9: `NativeSettings.svelte` — the two colour literals** (~`:272` `.error-hint`, ~`:296` `.error`). In both rules change `color: #e5484d;` to `color: var(--danger);`. Re-grep to confirm none is left:

```bash
grep -n "e5484d\|e5a53a" app/src/lib/details/NativeSettings.svelte
```

Expected: no output.

- [ ] **Step 10: `CloudPanel.svelte` — import and remove button.** Add `import Icon from '../Icon.svelte';` to the import block.

Before (~`:333-341`):

```svelte
                    <button
                      data-testid={`cloud-native-path-remove-${path}`}
                      class="remove"
                      disabled={manualPathPending}
                      onclick={() => handleRemoveManualPath(path)}
                      aria-label={`Remove ${path}`}
                    >
                      ×
                    </button>
```

After:

```svelte
                    <button
                      data-testid={`cloud-native-path-remove-${path}`}
                      class="remove icon-btn"
                      disabled={manualPathPending}
                      onclick={() => handleRemoveManualPath(path)}
                      aria-label={`Remove ${path}`}
                    >
                      <Icon name="close" size={14} />
                    </button>
```

- [ ] **Step 11: `CloudPanel.svelte` — the remove CSS** (~`:486-497`). It was the worst control in the audit: a 13px glyph in a 22px box in an off-palette literal colour.

Before:

```css
  .remove {
    flex: none;
    width: 22px;
    height: 22px;
    line-height: 1;
    padding: 0;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: #e5484d;
    cursor: pointer;
  }
```

After:

```css
  /* Was 22×22 with a raw `#e5484d` that did not track the theme. `.icon-btn`
     gives it the 28×28 minimum target and `var(--r-chip)`; the colour is now
     the danger token. */
  .remove {
    flex: none;
    color: var(--danger);
  }
```

- [ ] **Step 12: `CloudPanel.svelte` — the remaining colour literals.** Change `color: #e5484d;` to `color: var(--danger);` in `.error` (~`:436`), and in `.record-actions button.danger` (~`:591-592`) change both `color: #e5484d;` → `color: var(--danger);` and `border-color: #e5484d;` → `border-color: var(--danger);`. Change `.message.warn`'s `color: #e5a53a;` (~`:447`) to `color: var(--warning);`. Re-grep:

```bash
grep -n "e5484d\|e5a53a" app/src/lib/details/CloudPanel.svelte
```

Expected: no output. (`app/src/lib/Connect.svelte:96` keeps its literal — it is out of this plan's scope by ruling 10.)

- [ ] **Step 13: Run** from `app/`: `npm run check` (no new warnings) and `npx vitest run` (green).

- [ ] **Step 14: Commit**

```bash
git add app/src/lib/Details.svelte app/src/lib/details/NativeSettings.svelte app/src/lib/details/CloudPanel.svelte
git commit -m "rewrite: draw every close, dismiss and remove button with the close icon on a 28px target"
```

---

### Task 3: Media viewer — close, chevrons and the off-centre nav bug; media tab play badge

Covers audit rows #7, #8, #9 (media viewer) and #3 (media tab), plus the live `transform` defect in §4.4.

**Files:**
- Modify: `app/src/lib/details/MediaViewer.svelte` (the three buttons at `:94`, `:97-112`; the `.icon`, `.prev`, `.next` CSS at `:193-226`)
- Modify: `app/src/lib/details/MediaTab.svelte` (the video tile at `:26`; the `.video-tile` CSS at `:60-66`)

**Interfaces:**
- Consumes: `Icon.svelte` and `.icon-btn` from Task 1.
- No id or prop change. `MediaTab`'s tile button's accessible name changes from `"▶ Trailer"` to `"Trailer"` — the caption alone — because the icon becomes `aria-hidden`.

- [ ] **Step 1: Prove no spec asserts the media buttons' text:**

```bash
grep -rn "media-viewer-close\|media-viewer-prev\|media-viewer-next\|details-media-" e2e/specs
```

Expected: `.click()` / `.toExist()` / `.waitForExist()` only. If any assertion reads their text, stop and report NEEDS_CONTEXT.

- [ ] **Step 2: `MediaViewer.svelte` — import.** Add `import Icon from '../Icon.svelte';` next to `import Image from '../Image.svelte';`.

- [ ] **Step 3: `MediaViewer.svelte` — the three buttons** (`:94-113`).

Before:

```svelte
    <button data-testid="media-viewer-close" class="icon close" onclick={onClose} aria-label="Close">×</button>

    {#if items.length > 1}
      <button
        data-testid="media-viewer-prev"
        class="icon prev"
        onclick={() => go(prevIndex(index, items.length))}
        aria-label="Previous"
      >
        ‹
      </button>
      <button
        data-testid="media-viewer-next"
        class="icon next"
        onclick={() => go(nextIndex(index, items.length))}
        aria-label="Next"
      >
        ›
      </button>
    {/if}
```

After:

```svelte
    <button data-testid="media-viewer-close" class="icon-btn icon close" onclick={onClose} aria-label="Close">
      <Icon name="close" size={20} />
    </button>

    {#if items.length > 1}
      <button
        data-testid="media-viewer-prev"
        class="icon-btn icon prev"
        onclick={() => go(prevIndex(index, items.length))}
        aria-label="Previous"
      >
        <Icon name="chevronLeft" size={20} />
      </button>
      <button
        data-testid="media-viewer-next"
        class="icon-btn icon next"
        onclick={() => go(nextIndex(index, items.length))}
        aria-label="Next"
      >
        <Icon name="chevronRight" size={20} />
      </button>
    {/if}
```

- [ ] **Step 4: `MediaViewer.svelte` — the button CSS** (`:193-226`). The viewer's buttons stay 44px (they float over artwork, not in a dense panel), so `.icon` overrides the shared box; scoped component rules out-specify `.icon-btn`.

Before:

```css
  .icon {
    position: absolute;
    font: inherit;
    font-size: 28px;
    line-height: 1;
    width: 44px;
    height: 44px;
    border: none;
    border-radius: var(--r-pill);
    background: rgba(255, 255, 255, 0.12);
    color: #fff;
    cursor: pointer;
    transition: background var(--m-fast) ease;
  }

  .icon:hover,
  .icon:focus-visible {
    background: rgba(255, 255, 255, 0.24);
  }

  .close {
    top: 16px;
    right: 16px;
  }

  .prev {
    left: 16px;
    top: 50%;
  }

  .next {
    right: 16px;
    top: 50%;
  }
```

After:

```css
  /* `.icon-btn` (app.css) supplies the reset; the viewer keeps its own 44px
     circle on a scrim, because these three float over artwork rather than
     sitting in a panel. `#fff` is deliberate: the viewer is always a dark
     overlay, so its controls do not track the theme. */
  .icon {
    position: absolute;
    width: 44px;
    height: 44px;
    border-radius: var(--r-pill);
    background: rgba(255, 255, 255, 0.12);
    color: #fff;
    transition: background var(--m-fast) ease;
  }

  .icon:hover,
  .icon:focus-visible {
    background: rgba(255, 255, 255, 0.24);
  }

  .close {
    top: 16px;
    right: 16px;
  }

  /* `top: 50%` alone put the button's TOP edge on the centre line, so both
     nav buttons rendered 22px low. The translate is what actually centres
     them. */
  .prev {
    left: 16px;
    top: 50%;
    transform: translateY(-50%);
  }

  .next {
    right: 16px;
    top: 50%;
    transform: translateY(-50%);
  }
```

- [ ] **Step 5: `MediaTab.svelte` — import.** Add `import Icon from '../Icon.svelte';` next to `import Image from '../Image.svelte';`.

- [ ] **Step 6: `MediaTab.svelte` — the video tile** (`:26`). The icon takes no `label`, so it is `aria-hidden` and the button's accessible name is the caption alone (it was `"▶ Trailer"`).

Before:

```svelte
          <div class="video-tile">▶ {item.kind === 'youtube' ? 'Trailer' : 'Video'}</div>
```

After:

```svelte
          <div class="video-tile">
            <Icon name="play" size={20} />
            <span>{item.kind === 'youtube' ? 'Trailer' : 'Video'}</span>
          </div>
```

- [ ] **Step 7: `MediaTab.svelte` — the tile CSS** (`:60-66`). `place-items: center` on a grid stacks the icon over the caption; a centred flex row puts them side by side with a real gap.

Before:

```css
  .video-tile {
    display: grid;
    place-items: center;
    height: 100%;
    color: var(--text);
    font-size: 14px;
  }
```

After:

```css
  .video-tile {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    height: 100%;
    color: var(--text);
    font-size: 14px;
  }
```

- [ ] **Step 8: Run** from `app/`: `npm run check` (no new warnings) and `npx vitest run` (green).

- [ ] **Step 9: Commit**

```bash
git add app/src/lib/details/MediaViewer.svelte app/src/lib/details/MediaTab.svelte
git commit -m "rewrite: give the media viewer real chevrons, centre its nav buttons, and badge video tiles with a play icon"
```

---

### Task 4: Back arrow, brandmark and card cloud badge

Covers audit rows #10 (CloudPanel back), #1 (Shell brandmark) and #2 (GameCard cloud badge).

**Files:**
- Modify: `app/src/lib/details/CloudPanel.svelte` (the `cloud-back` button and its `.back` CSS)
- Modify: `app/src/lib/Shell.svelte` (the `.brand` block at `:100-103`; the `.logo` CSS at `:231-234`)
- Modify: `app/src/lib/GameCard.svelte` (the `card-cloud-badge-*` span at `:84`; the `.cloud-badge` CSS at `:324-333`)

**Interfaces:**
- Consumes: `Icon.svelte` from Task 1. `CloudPanel.svelte` already imports it after Task 2 — do not add a second import.
- The `cloud-back` button **loses its `aria-label`**. Its accessible name becomes the visible word "Back", so voice control matches what is on screen.
- `card-cloud-badge-*` keeps `role="img"`, `aria-label` and `title` on the **span** (the element that carries the test id), and the `<Icon>` inside stays `aria-hidden` so the name is not doubled.

- [ ] **Step 1: Re-read `CloudPanel.svelte`** (parity-2 may have moved things) and confirm no spec depends on the removed label:

```bash
grep -rn "cloud-back\|Back to details" e2e/specs app/src
```

Expected: the button in `CloudPanel.svelte` and, at most, `.click()` calls in specs. If a spec matches on the accessible name "Back to details", stop and report NEEDS_CONTEXT.

- [ ] **Step 2: `CloudPanel.svelte` — the back button** (~`:242`).

Before:

```svelte
    <button data-testid="cloud-back" class="back" onclick={onBack} aria-label="Back to details">← Back</button>
```

After:

```svelte
    <!-- No `aria-label`: it duplicated the visible word and overrode it, so
         voice control ("click Back") did not match the button. The icon is
         `aria-hidden`, so the visible text is the accessible name. -->
    <button data-testid="cloud-back" class="back" onclick={onBack}>
      <Icon name="arrowLeft" size={16} />
      Back
    </button>
```

- [ ] **Step 3: `CloudPanel.svelte` — the back CSS** (~`:401-409`). Add the flex row so the arrow and the word sit on one baseline-free centre line, and use the radius token instead of the literal.

Before:

```css
  .back {
    font: inherit;
    padding: 4px 8px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text);
    cursor: pointer;
  }
```

After:

```css
  .back {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font: inherit;
    padding: 4px 8px;
    border-radius: var(--r-chip);
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text);
    cursor: pointer;
  }
```

- [ ] **Step 4: `Shell.svelte` — import.** Add `import Icon from './Icon.svelte';` next to `import BackgroundArt from './BackgroundArt.svelte';`.

- [ ] **Step 5: `Shell.svelte` — the brandmark** (`:100-103`). The `▦` glyph resolved to a different typeface than the wordmark 8px beside it; the `grid` icon is drawn on the same 24 grid as every other icon and takes the primary token.

Before:

```svelte
  <div class="brand">
    <span class="logo" aria-hidden="true">▦</span>
    <span class="wordmark">GRID</span>
  </div>
```

After:

```svelte
  <div class="brand">
    <span class="logo"><Icon name="grid" size={20} /></span>
    <span class="wordmark">GRID</span>
  </div>
```

(The `aria-hidden` moves onto the `<svg>` itself, which `Icon` emits because no `label` is passed. The `.logo` span survives so the primary colour has somewhere to live — `Icon`'s own CSS is scoped and cannot be reached from here.)

- [ ] **Step 6: `Shell.svelte` — the logo CSS** (`:231-234`).

Before:

```css
  .logo {
    color: var(--primary);
    font-size: 18px;
  }
```

After:

```css
  /* Just the colour carrier for the brandmark: the `Icon` inside paints with
     `currentColor`, and `display: flex` keeps the 20px mark from sitting on
     the wordmark's text baseline. */
  .logo {
    display: flex;
    color: var(--primary);
  }
```

- [ ] **Step 7: `GameCard.svelte` — import.** Add `import Icon from './Icon.svelte';` next to `import Image from './Image.svelte';`.

- [ ] **Step 8: `GameCard.svelte` — the cloud badge** (`:84`). The glyph was the smallest in the app at 11px and, on a machine with Noto Color Emoji, would render as a coloured bitmap that ignores `var(--info)` and breaks the dark chip's contrast.

Before:

```svelte
      <span data-testid={`card-cloud-badge-${badgeId}`} class="cloud-badge" role="img" aria-label="Cloud saves enabled" title="Cloud saves enabled">☁</span>
```

After:

```svelte
      <span data-testid={`card-cloud-badge-${badgeId}`} class="cloud-badge" role="img" aria-label="Cloud saves enabled" title="Cloud saves enabled"><Icon name="cloud" size={14} /></span>
```

- [ ] **Step 9: `GameCard.svelte` — the badge CSS** (`:324-333`). The box was sized by the glyph's advance width, so it changed shape per machine; it is now a fixed 3px frame around a 14px mark.

Before:

```css
  .cloud-badge {
    position: absolute;
    bottom: 6px;
    right: 6px;
    font-size: 11px;
    line-height: 1;
    padding: 3px 5px;
    border-radius: var(--r-chip);
    background: rgba(0, 0, 0, 0.65);
    color: var(--info);
  }
```

After:

```css
  .cloud-badge {
    position: absolute;
    bottom: 6px;
    right: 6px;
    display: grid;
    place-items: center;
    padding: 3px;
    border-radius: var(--r-chip);
    background: rgba(0, 0, 0, 0.65);
    color: var(--info);
  }
```

- [ ] **Step 10: Run** from `app/`: `npm run check` (no new warnings) and `npx vitest run` (green).

- [ ] **Step 11: Commit**

```bash
git add app/src/lib/details/CloudPanel.svelte app/src/lib/Shell.svelte app/src/lib/GameCard.svelte
git commit -m "rewrite: draw the back arrow, the brandmark and the card cloud badge as icons"
```

---

### Task 5: The header rating star leaves the string

Covers audit row #13 and the first blocking E2E assertion in §4.5. This is one atomic change across a pure module, its test, the component and the spec — split it and the suite is red in between.

**Files:**
- Modify: `app/src/lib/details/header.ts` (`ratingText` → `ratingValue`; `headerLine`; `HeaderInput`)
- Modify: `app/src/lib/details/header.test.ts` (the `ratingText` describe block; the two `headerLine` cases that pass a rating; the import list)
- Modify: `app/src/lib/Details.svelte` (the `headerLine` call, the header import, the `.header-line` markup and its CSS)
- Modify: `e2e/specs/images-a.spec.ts:106-109`

**Interfaces:**
- Produces: `export function ratingValue(rating: string): string` — the trimmed rating, `''` when the server has none. No star.
- Removes: `export function ratingText(rating: string): string`.
- Changes: `HeaderInput` drops its `rating` field; `headerLine` joins platform, year, developer and genres only.
- Consumer contract: `details-header-line`'s text becomes `Super Nintendo Entertainment System · 1990 · Nintendo · Platformer · 9.2` — the number is still in the element (rendered by the `.rating` span), only the `★` character leaves, because `getText()` does not see an SVG.

- [ ] **Step 1: Update the failing tests** in `app/src/lib/details/header.test.ts`.

Replace `ratingText` with `ratingValue` in the import list at the top of the file. Then replace this describe block:

```ts
describe('ratingText', () => {
  it('stars a rating', () => {
    expect(ratingText('9.2')).toBe('★ 9.2');
  });

  it('is blank for no rating', () => {
    expect(ratingText('   ')).toBe('');
  });
});
```

with:

```ts
describe('ratingValue', () => {
  // The star is now an <Icon> in Details.svelte, not a character in this
  // string: a glyph resolved to a different typeface than the line around
  // it and could not be given the accent colour the old app used.
  it('is the trimmed number', () => {
    expect(ratingValue('9.2')).toBe('9.2');
    expect(ratingValue('  9.2  ')).toBe('9.2');
  });

  it('is blank for no rating', () => {
    expect(ratingValue('   ')).toBe('');
    expect(ratingValue('')).toBe('');
  });
});
```

Then fix the three `headerLine` cases. The first becomes:

```ts
  it('joins platform, year, developer and genres with the middot', () => {
    expect(
      headerLine({
        platformName: 'SNES',
        firstReleaseDate: '631152000',
        companies: 'Nintendo',
        genres: 'Platformer',
      })
    ).toBe('SNES · 1990 · Nintendo · Platformer');
  });
```

The second and third drop their `rating: ''` property (their expectations, `'SNES'` and `''`, are unchanged):

```ts
  it('drops every part the server has nothing for, with no dangling separator', () => {
    expect(
      headerLine({
        platformName: 'SNES',
        firstReleaseDate: '',
        companies: '',
        genres: '',
      })
    ).toBe('SNES');
  });

  it('is blank when the server knows nothing at all', () => {
    expect(headerLine({ platformName: '', firstReleaseDate: '', companies: '', genres: '' })).toBe(
      ''
    );
  });
```

- [ ] **Step 2: Run** `npx vitest run src/lib/details/header.test.ts` from `app/` — expect failures (`ratingValue` is not exported; `headerLine` still appends the star).

- [ ] **Step 3: Implement in `app/src/lib/details/header.ts`.**

Before (`:32-36`):

```ts
/** The header's rating chip, or `''` when the server has no rating. */
export function ratingText(rating: string): string {
  const trimmed = rating.trim();
  return trimmed === '' ? '' : `★ ${trimmed}`;
}
```

After:

```ts
/**
 * The header's rating number, or `''` when the server has no rating.
 *
 * The star is NOT here. It is an `<Icon name="star">` in `Details.svelte`,
 * because (a) `★` resolved to a different typeface than the rest of the
 * line and (b) the old app drew this star in the accent colour
 * (`grid_launcher/ui/game_views.py:582`), which a character inside a joined
 * string cannot be given.
 */
export function ratingValue(rating: string): string {
  return rating.trim();
}
```

Before (`:51-73`):

```ts
export type HeaderInput = {
  platformName: string;
  firstReleaseDate: string;
  companies: string;
  genres: string;
  rating: string;
};

/**
 * The one line under the title. Every part the server has nothing for is
 * dropped, so the separator never dangles on a sparse rom.
 */
export function headerLine(input: HeaderInput): string {
  return [
    input.platformName.trim(),
    releaseYear(input.firstReleaseDate),
    developerOf(input.companies),
    input.genres.trim(),
    ratingText(input.rating),
  ]
    .filter((part) => part !== '')
    .join(' · ');
}
```

After:

```ts
export type HeaderInput = {
  platformName: string;
  firstReleaseDate: string;
  companies: string;
  genres: string;
};

/**
 * The one line under the title, up to and including the genres. Every part
 * the server has nothing for is dropped, so the separator never dangles on a
 * sparse rom. The rating is appended by `Details.svelte` instead, because it
 * carries an icon and its own colour — see [`ratingValue`].
 */
export function headerLine(input: HeaderInput): string {
  return [
    input.platformName.trim(),
    releaseYear(input.firstReleaseDate),
    developerOf(input.companies),
    input.genres.trim(),
  ]
    .filter((part) => part !== '')
    .join(' · ');
}
```

- [ ] **Step 4: Run** `npx vitest run src/lib/details/header.test.ts` — green.

- [ ] **Step 5: `Details.svelte` — the import.** In the `./details/header` import block, replace nothing and **add** `ratingValue` in alphabetical position:

```ts
  import {
    cloudStatusLabel,
    epochDate,
    flagList,
    headerLine,
    lastPlayedText,
    launchTargetLine,
    ratingValue,
    verificationLabel,
  } from './details/header';
```

- [ ] **Step 6: `Details.svelte` — the derived values** (~`:465-473`).

Before:

```ts
  let header = $derived(
    headerLine({
      platformName: subject.platformName,
      firstReleaseDate: detail?.first_release_date ?? '',
      companies: detail?.companies ?? '',
      genres,
      rating,
    })
  );
```

After:

```ts
  let header = $derived(
    headerLine({
      platformName: subject.platformName,
      firstReleaseDate: detail?.first_release_date ?? '',
      companies: detail?.companies ?? '',
      genres,
    })
  );
  let ratingNumber = $derived(ratingValue(rating));
```

(`let rating = $derived(merged.rating)` at ~`:121` stays — it is now `ratingNumber`'s only consumer.)

- [ ] **Step 7: `Details.svelte` — the header line markup** (~`:602`). Keep it on one line: the `<p>`'s text content must come out of `getText()` with single spaces and no stray break.

Before:

```svelte
          <p class="header-line" data-testid="details-header-line">{header}</p>
```

After:

```svelte
          <p class="header-line" data-testid="details-header-line">{header}{#if ratingNumber !== ''} · <span class="rating"><span class="star"><Icon name="star" size={14} /></span>{ratingNumber}</span>{/if}</p>
```

- [ ] **Step 8: `Details.svelte` — the rating CSS.** Add immediately after the existing `.header-line` rule (~`:799-804`):

```css
  /* The rating is the one part of the header line that is not muted: the old
     app drew it in the accent colour so it stood out from the metadata
     around it. The star takes `--primary` (the accent the old app used for
     the rating); the NUMBER takes `--text-h`, because `--warning` is the same
     amber in both themes and would fall below a readable contrast on the
     light background. */
  .rating {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    color: var(--text-h);
  }

  .star {
    display: flex;
    color: var(--primary);
  }
```

- [ ] **Step 9: Update the E2E assertion** in `e2e/specs/images-a.spec.ts:106-109`.

Before:

```ts
    // Design §7's right header: platform · year · developer · genres · rating.
    await expect($(testId('details-header-line'))).toHaveText(
      'Super Nintendo Entertainment System · 1990 · Nintendo · Platformer · ★ 9.2',
    );
```

After:

```ts
    // Design §7's right header: platform · year · developer · genres · rating.
    // The rating's star is an inline SVG (`Icon name="star"`), so it does not
    // appear in the element's text — only the number does.
    await expect($(testId('details-header-line'))).toHaveText(
      'Super Nintendo Entertainment System · 1990 · Nintendo · Platformer · 9.2',
    );
```

- [ ] **Step 10: Prove no other reference to the old export or the old text survives:**

```bash
grep -rn "ratingText" app/src e2e
grep -rn "★" app/src e2e
```

Both must return no output.

- [ ] **Step 11: Run** from `app/`: `npx vitest run` (green) and `npm run check` (no new warnings).

- [ ] **Step 12: Commit**

```bash
git add app/src/lib/details/header.ts app/src/lib/details/header.test.ts app/src/lib/Details.svelte e2e/specs/images-a.spec.ts
git commit -m "rewrite: render the details rating star as an icon in the accent colour"
```

---

### Task 6: The downloads footer arrow leaves the string

Covers audit row #12 and the second blocking E2E assertion in §4.5. Same atomic shape as Task 5.

**Files:**
- Modify: `app/src/lib/downloads/format.ts` (`footerLine` and its doc comment, `:152-187`)
- Modify: `app/src/lib/downloads/format.test.ts` (`:308`, `:315`, `:323`, `:329`, `:330`, `:331`, `:354`)
- Modify: `app/src/lib/DownloadsFooter.svelte` (the strip markup and its CSS)
- Modify: `e2e/specs/downloads.spec.ts:162-168`

**Interfaces:**
- Changes: `footerLine(entries)` returns `<title> · <percent> · <speed>` — no `⬇` prefix. It still returns `null` when nothing is live.
- Consumer contract: `downloads-aggregate`'s text loses the `⬇ ` prefix. The arrow becomes an `<Icon name="download" size={14} />` rendered as a **sibling** of `.line`, not inside it, because `.line` is a single-line ellipsis box and a `display: block` SVG inside it would take its own row.

- [ ] **Step 1: Update the failing tests** in `app/src/lib/downloads/format.test.ts` — remove the `⬇ ` prefix from all seven expectations:

```ts
    expect(line).toBe('Chrono Trigger · 50% · 2.0 KB/s');
```
```ts
    expect(line).toBe('Chrono Trigger · — · 0 B/s');
```
```ts
    expect(line).toBe('Downloading One · 25% · 1.0 KB/s');
```
```ts
    ).toBe('A · 75% · Installing');
    expect(footerLine([entry({ title: 'A', status: 'queued' })])).toBe('A · — · Queued');
    expect(footerLine([entry({ title: 'A', status: 'cancelling' })])).toBe('A · — · Cancelling');
```
```ts
    expect(footerLine(entries)).toContain('Two ·');
```

- [ ] **Step 2: Run** `npx vitest run src/lib/downloads/format.test.ts` from `app/` — expect seven failures.

- [ ] **Step 3: Implement in `app/src/lib/downloads/format.ts`.**

Before (`:152-158`, the doc comment):

```ts
/**
 * The 28px status strip's one line (design §3):
 * `⬇ <title> · <percent> · <speed>`, or `null` when nothing is live and the
 * strip hides itself.
```

After:

```ts
/**
 * The 28px status strip's one line (design §3):
 * `<title> · <percent> · <speed>`, or `null` when nothing is live and the
 * strip hides itself. The download arrow that used to prefix this string is
 * an `<Icon name="download">` in `DownloadsFooter.svelte`: `⬇` fell back to
 * a different typeface mid-sentence, and as an emoji-defaulted character it
 * could render as a coloured bitmap inside a `var(--text-h)` status line.
```

Before (`:187`):

```ts
  return `⬇ ${current.title} · ${pct} · ${speed}`;
```

After:

```ts
  return `${current.title} · ${pct} · ${speed}`;
```

- [ ] **Step 4: Run** `npx vitest run src/lib/downloads/format.test.ts` — green.

- [ ] **Step 5: `DownloadsFooter.svelte` — import.** Add `import Icon from './Icon.svelte';` next to `import Sparkline from './downloads/Sparkline.svelte';`.

- [ ] **Step 6: `DownloadsFooter.svelte` — the markup** (`:30`).

Before:

```svelte
  <span data-testid="downloads-aggregate" class="line">{line ?? ''}</span>
```

After:

```svelte
  {#if line !== null}
    <span class="lead"><Icon name="download" size={14} /></span>
  {/if}
  <span data-testid="downloads-aggregate" class="line">{line ?? ''}</span>
```

- [ ] **Step 7: `DownloadsFooter.svelte` — the CSS.** Add immediately before the existing `.line` rule (~`:73`):

```css
  /* The download mark, outside `.line` so the ellipsis box stays a single
     text run, and in `.line`'s colour rather than the strip's muted one so
     the two read as one unit. */
  .lead {
    display: flex;
    flex: none;
    color: var(--text-h);
  }
```

- [ ] **Step 8: Update the E2E assertion** in `e2e/specs/downloads.spec.ts:162-168`.

Before:

```ts
    // `⬇ <title> · <percent> · <speed>` (design §3). The percent is a
    // number while the total is known and an em dash otherwise; the last
    // slot is a byte rate while downloading and the phase word once the
    // entry has moved on to installing.
    expect(await $(testId('downloads-aggregate')).getText()).toMatch(
      /^⬇ Big Arcade Game · (\d{1,3}%|—) · ([\d.]+ [KMGT]?B\/s|Installing)$/,
    );
```

After:

```ts
    // `<title> · <percent> · <speed>` (design §3). The percent is a number
    // while the total is known and an em dash otherwise; the last slot is a
    // byte rate while downloading and the phase word once the entry has
    // moved on to installing. The download arrow is an inline SVG in a
    // sibling span, so it is not part of this element's text.
    expect(await $(testId('downloads-aggregate')).getText()).toMatch(
      /^Big Arcade Game · (\d{1,3}%|—) · ([\d.]+ [KMGT]?B\/s|Installing)$/,
    );
```

- [ ] **Step 9: Prove the glyph is gone:**

```bash
grep -rn "⬇" app/src e2e
```

Expected: no output.

- [ ] **Step 10: Run** from `app/`: `npx vitest run` (green) and `npm run check` (no new warnings).

- [ ] **Step 11: Commit**

```bash
git add app/src/lib/downloads/format.ts app/src/lib/downloads/format.test.ts app/src/lib/DownloadsFooter.svelte e2e/specs/downloads.spec.ts
git commit -m "rewrite: render the downloads footer's download mark as an icon"
```

---

### Task 7: Record the icon system in the design spec

**Files:**
- Modify: `docs/superpowers/specs/2026-09-04-desktop-ui-redesign-design.md` (§4 Theme tokens, §11 Test ids and E2E)

- [ ] **Step 1: Add an "Iconography" group to §4**, after the "Spacing on a 4px scale…" bullet and before the "Theme resolution" bullet:

```markdown
- Iconography: one `Icon.svelte` component over `lib/icons.ts`, a pure module of nine
  hand-authored paths on a fixed `0 0 24 24` grid (`close`, `chevronLeft`, `chevronRight`,
  `arrowLeft`, `cloud`, `star`, `download`, `play`, `grid`). Outline icons use
  `stroke="currentColor"` at 1.5 with round caps and joins; `star` and `play` are the only
  solid marks (`fill="currentColor"`). No colour literal appears in an icon — colour is
  always the caller's token. Sizes are 14 (inline with 12–13px text), 16 (default) and 20
  (standalone icon buttons, brandmark); no other value. Icon-only buttons use the global
  `.icon-btn` class in `app.css` and are at least 28×28. An icon paired with visible text
  is `aria-hidden="true" focusable="false"`; an icon that IS the label takes `role="img"`
  plus `aria-label`. No Unicode character is used as an icon anywhere in the UI (added
  2026-09-05, parity-3).
```

- [ ] **Step 2: Add a paragraph to §11**, after the "New ids" bullet:

```markdown
- Icons and E2E text: an inline SVG contributes nothing to `getText()`, so any mark moved
  out of a returned string and into an `<Icon>` changes the element's text. Two did:
  `details-header-line` now reads `… · Platformer · 9.2` (the `★` left `ratingText`, which
  became `ratingValue`), and `downloads-aggregate` now reads `<title> · <percent> ·
  <speed>` (the `⬇` left `footerLine`). Each move landed in one commit with its unit tests
  and its spec assertion. No test id changed. New icon markup must never be given a test
  id of its own — the id stays on the control (parity-3, 2026-09-05).
```

- [ ] **Step 3: Run** from `app/`: `npm run check` and `npx vitest run` — both unaffected and green.

- [ ] **Step 4: Commit**

```bash
git add ../docs/superpowers/specs/2026-09-04-desktop-ui-redesign-design.md
git commit -m "rewrite: document the icon system and its two E2E text changes in the design spec"
```

---

### Task 8: E2E gate

**Files:** none modified unless a group fails.

- [ ] **Step 1: Prove every glyph-as-icon is gone.** From `rewrite/`:

```bash
grep -rn "▦\|☁\|▶\|×\|‹\|›\|←\|⬇\|★" app/src e2e/specs
```

Expected: **no output**. (The typographic characters the audit ruled out of scope — `·`, `•`, `…`, `—`, `“ ”` — are not in this pattern and are unaffected.)

- [ ] **Step 2: Prove every audited test id still exists:**

```bash
grep -rn "details-close\|media-viewer-close\|media-viewer-prev\|media-viewer-next\|native-settings-close\|cloud-back\|details-warning-dismiss\|card-cloud-badge-\|cloud-native-path-remove-\|installed-badge-\|details-header-line\|downloads-aggregate" app/src | wc -l
```

Every one of the twelve must appear in `app/src`. If any is missing, stop and report.

- [ ] **Step 3: Run the suite.** From `rewrite/`, detached, logging to a file:

```bash
nohup scripts/e2e.sh images downloads library install cloud-saves native > /tmp/claude-1000/-home-six-Documents-Programming-grid-launcher/d527a4be-8a2d-487c-bc02-e067fbdcf4ce/scratchpad/e2e-parity3.log 2>&1 &
```

Then poll the log until the summary line appears. Group coverage: `images` reads the details header line and drives the media viewer; `downloads` reads the aggregate line; `library` renders the cards and the cloud badge; `install` opens the popup and its close button; `cloud-saves` drives the back button and the manual-path remove button; `native` opens the Native Settings dialog and its close button.

- [ ] **Step 4:** All six groups green. If one fails, read the failure, fix the cause within this plan's scope, re-run that group, and commit the fix with a `rewrite: ` subject.

- [ ] **Step 5:** Report the per-group result lines verbatim.

---

## Self-review notes

**Coverage of all thirteen audit rows.** Every row in audit §1 is assigned to exactly one task:

| Audit row | Glyph | Task | Icon / size |
|---|---|---|---|
| #1 Shell brandmark | `▦` | 4 | `grid` @ 20 |
| #2 GameCard cloud badge | `☁` | 4 | `cloud` @ 14 |
| #3 MediaTab video tile | `▶` | 3 | `play` @ 20, `aria-hidden` |
| #4 Details close | `×` | 2 | `close` @ 20 |
| #5 Details warning dismiss | `×` | 2 | `close` @ 14, target 18→28 |
| #6 NativeSettings close | `×` | 2 | `close` @ 20 |
| #7 MediaViewer close | `×` | 3 | `close` @ 20 |
| #8 MediaViewer prev | `‹` | 3 | `chevronLeft` @ 20 |
| #9 MediaViewer next | `›` | 3 | `chevronRight` @ 20 |
| #10 CloudPanel back | `←` | 4 | `arrowLeft` @ 16, `aria-label` dropped |
| #11 CloudPanel remove path | `×` | 2 | `close` @ 14, target 22→28, `--danger` |
| #12 Downloads footer | `⬇` | 6 | `download` @ 14 |
| #13 Details rating | `★` | 5 | `star` @ 14, `--primary` |

Structural fixes from §4.4 all placed: `translateY(-50%)` (Task 3 Step 4), MediaTab `aria-hidden` (Task 3 Step 6), CloudPanel `aria-label` dropped (Task 4 Step 2), 28×28 growth (Task 2 Steps 6 and 11), `font: inherit` (via `.icon-btn`, Task 1 Step 6), colour literals (Task 2 Steps 9 and 12). The four CSS-drawn dots and every typographic separator are untouched, as the audit requires.

**Deviations from the brief, and why.** (a) MediaViewer's close button is in Task 3, not Task 2, so the file is opened once for all three of its controls plus its CSS bug rather than twice. (b) `cloud` is an outline, not a solid — recorded as ruling 3 with its reason. (c) `Connect.svelte:96` carries the same `#e5484d` literal but is outside the audit's scope and is deliberately left alone (ruling 10). (d) The header rating's *number* is `--text-h`, not `--warning`; only the star takes `--warning` — see the open question.

**Placeholder scan.** No step says "similar to", "and so on", "etc.", or "…". Every path string, every CSS block, every markup replacement and every changed test expectation is written out in full. Both before→after snippets in the two parity-2 files are accompanied by a re-read instruction (rulings 11, Task 2 Step 1, Task 4 Step 1) because their line numbers will move.

**Type consistency.** `ICONS` is `as const`, so `IconName` is the nine string literals; `Icon.svelte`'s `name` prop is typed `IconName`, so a typo at a call site is a `npm run check` error rather than an empty `<path>`. `FILLED_ICONS` is `readonly IconName[]`, and `icons.test.ts` asserts both that it names only real icons and that it is exactly `star` and `play`, which is what keeps `Icon.svelte`'s fill/stroke switch honest without a component harness. `HeaderInput` loses `rating` in Task 5, so any caller that still passes it is a compile error caught by `npm run check` in the same task — `Details.svelte` is the only caller and is updated in Step 6. `footerLine`'s signature is unchanged in Task 6; only its returned string changes, which is why its seven unit expectations and the one spec regex move with it.

**Sequencing.** Tasks 2–4 have zero spec impact (audit §4.5 verified no spec reads any of those controls' text), so they can land before the two text-changing tasks. Tasks 5 and 6 are each self-contained: module + unit test + component + spec in one commit, so the suite is never red between commits.

## Open question

**The rating star's colour on the light theme — RESOLVED: `var(--primary)` (ruling 8).** The original draft set the star to `var(--warning)` to match the old app. `--warning` is `#fbbf24` in *both* themes (`app.css` does not override it under `data-theme="light"`), and `#fbbf24` on the light `--bg` `#f5f5fa` is about **1.5:1** — below the 3:1 that WCAG asks of a meaningful graphic. The plan therefore colours only the star with `--warning` and keeps the number at `--text-h`, which is legible in both themes, so nothing *unreadable* ships. But on the light theme the star itself will be faint. Two one-line alternatives, both inside Task 5 Step 8: use `var(--primary)` for `.star` (it darkens to `#553e98` on light and is legible in both), or add a light-theme override for `--warning` in `app.css`. This needs the user's call — it is a palette decision, not an implementation one.
