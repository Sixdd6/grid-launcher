# Round 4 — upload feedback, media tiles, server fade, background art

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the four round-4 review findings: an auto cloud upload finishes silently, a dead screenshot tile stays on screen forever (and a *loading* tile looks exactly like a dead one), the Server grid dims every not-installed card, and the shell's background art is one un-cycled full-resolution cover blurred by the compositor on every frame.

**Architecture:** Four independent seams, sequenced so each lands with its own tests.
(1) **Feedback seam** — one backend event, `cloud-upload-finished`, emitted by `CloudService` after every auto upload AND every manual upload; one listener in `Shell.svelte` calling `pushToast`. The payload text is not re-derived: it is the `upload_completion_message` text `ops::upload` already produced, so the toast and the panel line can never disagree.
(2) **Media seam** — `Image.svelte` gains a real tri-state (`loading | error | ready`) with a shimmer skeleton, so a loading tile is never mistaken for a dead one; failed screenshot tiles disappear from the Media tab and are never shown as a dead frame in the viewer; the trailer/video tile gets artwork with a play badge instead of a bare icon.
(3) **Server fade** — a two-rule CSS deletion plus a spec line.
(4) **Background seam**, split in two: *selection* (a `BackgroundSubject` of fanart → screenshots → cover, cycling every 5 s, fed by details-open, keyboard/gamepad focus, hover dwell and a startup seed) and *performance* (a backend-built, pre-blurred 960 px JPEG variant, so the webview composites a 0.3 MP still instead of blurring 2.4 MP per layer per frame).

**Tech Stack:** Rust (grid-core + the Tauri `app` crate), Svelte 5 runes + TypeScript + vitest, WebdriverIO E2E against the mock RomM server.

**Spec:** `docs/superpowers/specs/2026-09-04-desktop-ui-redesign-design.md` (D-UI-3 and §3 "Background art") and `docs/porting/07-covers-images.md` ("Fanart background (TV)", "Rust port deviations (milestone 7)") are the behaviour specs, and are updated by Task 1 and Task 10 where the rewrite now deliberately differs. The diagnosis is the 2026-09-05 round-4 research pass.

All paths below are relative to `rewrite/` unless they start with `docs/`.

## User decisions / rulings (binding)

1. **Background art priority is fanart → screenshots → cover** (user ruling 2026-09-05). Fanart wins whenever the server supplies one; otherwise the game's own screenshots, cycling every 5000 ms when there is more than one; the cover only as a last resort. Python's TV `FanartBackground` used screenshots only and had no fanart source at all (`grid_launcher/tv/widgets/components/fanart_background.py:38`, doc 07 "Fanart background (TV)"); the rewrite reads the real `ss_metadata.fanart_path` / `gamelist_metadata.fanart_path` fields RomM exposes.
2. **Cycling stops when the fade slider is 0.** Fade 0 means the art is invisible (`BackgroundArt.svelte`'s `--art-opacity`); a timer that swaps invisible images is pure cost.
3. **The YouTube thumbnail is the ONE allowed foreign host** (user ruling 2026-09-05). `https://img.youtube.com/vi/<id>/hqdefault.jpg` is a static CDN path: no API key, no quota, no token, no cookie (`referrerpolicy="no-referrer"`). It is loaded as a plain `<img>`, **never** through `ensure_image` — routing it through the cache would send it via `RommClient`, which attaches the RomM Authorization header. On error, or when the rom has no id, the tile falls back to the server-hosted large cover with the same play badge. `img-src` in the CSP gains `https://img.youtube.com` and nothing else.
4. **Remove the Server view's not-installed dimming (D-UI-3).** The installed dot and the Install/Play button already carry that state; dimming the cover reads as "broken image". The spec line is corrected rather than deleted, so the change is recorded.
5. **`NON_SCREENSHOT_ART_RE` must NOT be applied to fanart.** That regex (`crates/grid-core/src/images/urls.rs:32-37`) explicitly rejects `fanart`, `banner`, `logo`… from *screenshot* lists. A fanart URL is read through its own extractor that does not call `looks_like_screenshot_url`, or every fanart would be dropped.
6. **A failed screenshot in the fullscreen viewer is NOT auto-advanced.** The Media tab drops the tile (mirroring `OverviewTab.svelte:70-77`); the viewer, which cannot drop the item it is showing without shifting every index under the user, renders an explicit "This screenshot could not be loaded" line instead. Auto-advancing would loop forever when every item fails.
7. **No raw-image fallback for the background.** When `ensure_background_variant` fails, the previous art stays. `BackgroundArt.svelte` no longer applies `filter: blur(40px)`, so falling back to the un-blurred full-resolution source would show a *different* effect, not a degraded one.
8. **Deferred, NOT in this plan:** the unconditional `WEBKIT_DISABLE_DMABUF_RENDERER=1` on Linux (`app/src-tauri/src/lib.rs:25-34`); a `last_played` column for the startup seed (the seed still uses the newest `installed_at`, as `startupCover` does today); any second background variant size; user screenshots as a fanart source.

## Global Constraints

- **Token secrecy (hard):** tokens live only in the OS keyring and the redacting in-memory type; never in files, logs, errors, IPC, or console output. The `cloud-upload-finished` payload carries a game title and a completion message and nothing else. The YouTube thumbnail URL carries no token and is the only foreign host anything in this plan may reach; every other image still goes through `ensure_image` → `RommClient`.
- **Only `app.css` tokens for colours**; `--m-*` motion tokens for animation. `app/src/lib/Image.svelte:66-67` still carries `#2a2d34` / `#aab`; Task 2 edits that block, so both become tokens (`var(--surface-2)`, `var(--text-muted)` — declared once at `app/src/app.css:10,13` and re-declared per theme, so they are correct in light and dark).
- **Every test id E2E asserts today stays:** `details-cover`, `details-screenshot-<n>`, `details-media-<n>`, `details-no-media`, `media-viewer`, `media-viewer-image`, `media-viewer-youtube`, `media-viewer-video`, `media-viewer-caption`, `media-viewer-next`, `media-viewer-prev`, `media-viewer-close`, `background-art`, `background-art-toggle`, `background-fade`, `cloud-upload`, `cloud-upload-error`, `toast`, `toast-region`, every `game-card-*` / `library-card-*` id.
- **Every task ends with**, from `rewrite/`: `cargo fmt`; `cargo clippy --workspace --all-targets -- -D warnings` and `cargo clippy -p app --all-targets --features e2e -- -D warnings` clean; `cargo test --workspace` green **when Rust changed**; and from `rewrite/app`: `npm run check` (baseline 3 warnings — re-record the exact count in Task 1 Step 0; no NEW warnings after that) and `npx vitest run` green. Then a commit whose subject starts `rewrite: `.
- **Never** run `git checkout`, `git restore`, `git reset`, or `git stash`. Commit with explicit pathspecs.
- **No component test harness exists** (no `@testing-library/svelte`, no jsdom); the only Svelte test in the repo is an SSR `svelte/server` render test (`app/src/lib/Icon.svelte.test.ts`). Every other `.svelte` change is verified by an extracted pure module with vitest tests, plus `npm run check`, plus E2E. Never fabricate a component test.
- The final task runs the E2E groups `cloud-saves`, `images`, `library`, `install`, `launch` (`rewrite/scripts/e2e.sh cloud-saves images library install launch`, detached, log to a file) and they must be green.

---

## File map

| File | Responsibility |
|---|---|
| `app/src/lib/GameCard.svelte` | `.card.dim` rules and `class:dim` deleted (D-UI-3 ruling) |
| `app/src/lib/Image.svelte` | tri-state `loading \| error \| ready`, shimmer skeleton, colour tokens |
| `app/src/lib/details/MediaTab.svelte` | failed screenshot tiles removed; trailer/video poster + play badge |
| `app/src/lib/details/MediaViewer.svelte` | failed screenshot renders an explicit error line |
| `app/src/lib/details/media.ts` (+ `.test.ts`) | `youtubeThumbnailUrl`, `trailerPoster` |
| `app/src-tauri/tauri.conf.json` | `img-src` gains `https://img.youtube.com` |
| `app/src-tauri/src/cloud_service.rs` | `cloud-upload-finished` event, payload, `upload_finished_payload` + tests |
| `app/src-tauri/src/commands/cloud.rs`, `app/src-tauri/src/lib.rs` | `AppHandle` threaded into the manual upload command and the session-finished hook |
| `app/src/lib/Shell.svelte` | one `listen` → `pushToast` |
| `crates/grid-core/src/images/urls.rs` | `fanart_urls_from_payload` (no screenshot filter) |
| `crates/grid-core/src/romm/mod.rs` | `RomDetail.fanart_urls`; `GameSummary.screenshot_urls`/`fanart_urls`; `RawGameSummary` flatten + `into_summary(base)` |
| `crates/grid-core/src/library/registry.rs` (+ `tests/registry.rs`, `e2e/seed/registry-schema.mjs`) | `fanart_urls` column, migration v4 → v5 |
| `crates/grid-core/src/images/mod.rs` | `ImageFields.fanart_urls` |
| `app/src/lib/background.ts` (+ `.test.ts`) | `BackgroundSubject`, priority, cycle, subject constructors, startup seed |
| `app/src/lib/stores/lastViewed.svelte.ts`, `app/src/lib/lastViewedHover.ts` (+ tests) | subject-shaped store, dwell + 150 ms prefetch |
| `app/src/lib/backgroundPrefetch.ts` | the one `ensureBackgroundVariant` fire-and-forget call the dwell timer makes |
| `app/src/lib/Library.svelte`, `app/src/lib/Server.svelte`, `app/src/lib/Details.svelte`, `app/src/lib/Shell.svelte` | subject writers (details open, focused card, hover, startup seed) |
| `crates/grid-core/src/images/background.rs` (new) | `ensure_background_variant`: decode → 960 px → `fast_blur` σ12 → JPEG q80 → `<key>.bg.jpg` |
| `crates/grid-core/src/images/sweep.rs` | pin by key prefix so a variant is pinned with its source |
| `crates/grid-core/src/images/replenish.rs`, `app/src-tauri/src/images.rs` | plan/prefetch the variant |
| `app/src/lib/BackgroundArt.svelte` | no CSS blur, `will-change: opacity`, captured timeout, URL→path memo, cycling |
| `docs/superpowers/specs/2026-09-04-desktop-ui-redesign-design.md`, `docs/porting/07-covers-images.md` | D-UI-3 and §3 rewritten; doc 07 gains the fanart + variant sections |
| `e2e/specs/images-a.spec.ts`, `e2e/specs/cloud-saves.spec.ts`, `e2e/specs/library.spec.ts`, `e2e/fixtures/rom-details.json` | new cases |

---

### Task 1: Remove the Server view's not-installed dimming (D-UI-3)

**Files:**
- Modify: `app/src/lib/GameCard.svelte:65` (`class:dim={!installed}`) and `:149-157` (the `/* D-UI-3 … */` comment and the `.card.dim` rules)
- Modify: `docs/superpowers/specs/2026-09-04-desktop-ui-redesign-design.md:27` (D-UI-3)

**Interfaces:**
- Changes: nothing in the component's props. `installed` is still consumed by `cardBadges` (`app/src/lib/cards/badges.ts`) for the installed dot and by the `Play`/`Install` label at `GameCard.svelte:88`.
- Produces: no new exports.

- [ ] **Step 0 (baseline):** from `app/`, run `npm run check` and record the exact warning count and the files they name in the commit message body. That count is the "no new warnings" baseline for every later task.

- [ ] **Step 1: Prove no spec depends on the class:** run `grep -rn "dim" e2e/specs app/src/lib` — expected hits are only `app/src/lib/GameCard.svelte` (the three lines this task removes plus the `blurred, dimmed copy` comment at `:168`) and `app/src/lib/Details.svelte:739` (`dimmed AND blurred shell`, an unrelated rule). If any file under `e2e/specs` matches, stop and report NEEDS_CONTEXT.

- [ ] **Step 2: Delete the class binding.** In `app/src/lib/GameCard.svelte`, remove the whole line `  class:dim={!installed}` from the card root's attribute list (currently between `class:focused` and `onclick={onOpen}`).

- [ ] **Step 3: Delete the rules.** Remove this entire block from the `<style>` (currently directly under the `.card.focused .cover` rule):

```css
  /* D-UI-3: not-installed cards render at 60% until hover. */
  .card.dim .cover {
    opacity: 0.6;
    transition: opacity var(--m-fast) ease;
  }
  .card.dim:hover .cover,
  .card.dim.focused .cover {
    opacity: 1;
  }
```

- [ ] **Step 4: Record the ruling in the spec.** In `docs/superpowers/specs/2026-09-04-desktop-ui-redesign-design.md`, replace the D-UI-3 row's text with:

```
| D-UI-3 | Server mirrors Library: rail of server platforms with counts, platform header (name, counts, firmware status), grid with Installed / Update badges. Not-installed cards render at full opacity — the 60%-until-hover dimming this row originally called for was removed on 2026-09-05 (user ruling): the installed dot and the Play/Install button already state that, and a dimmed cover reads as a failed image. |
```

- [ ] **Step 5: Run** `npx vitest run` and, from `app/`, `npm run check` — green, no new warnings. (`cards/badges.test.ts` and `cards/size.test.ts` cover this component's only logic and are untouched.)

- [ ] **Step 6: Commit**

```bash
git add app/src/lib/GameCard.svelte ../docs/superpowers/specs/2026-09-04-desktop-ui-redesign-design.md
git commit -m "rewrite: stop dimming not-installed cards on the Server grid"
```

---

### Task 2: `Image.svelte` tri-state + failed screenshot tiles

**Files:**
- Modify: `app/src/lib/Image.svelte:20-45` (state + effect), `:48-59` (markup), `:61-72` (style)
- Modify: `app/src/lib/details/MediaTab.svelte:6-12` (props), `:17-33` (the tile loop)
- Modify: `app/src/lib/details/MediaViewer.svelte:8-18` (props), `:118-143` (the stage)
- Modify: `app/src/lib/Details.svelte:112-121` (near `mediaItems`), `:674` (`<MediaTab …>`), `:725-733` (`<MediaViewer …>`)

**Interfaces:**
- Produces (in `Image.svelte`): no new props. Internally `let status = $state<'loading' | 'error' | 'ready'>('loading')`. `onerror` keeps its exact current contract — called once per failure, after `src` has been cleared.
- Produces (in `MediaTab.svelte`): two new props — `failed: Record<string, true>` and `onScreenshotError: (url: string) => void`.
- Produces (in `MediaViewer.svelte`): the same two props.
- Consumes: `MediaItem` from `details/media.ts:5-8` (unchanged this task).

Reference: `OverviewTab.svelte:49-52,70-77` is the pattern being mirrored — a `Record<string, true>` keyed by URL, an `onerror` that writes into it, and an `{#if}` **inside** the `{#each}` so the surviving tiles keep their original indices.

- [ ] **Step 1: Lift the failure map into `Details.svelte`.** Both the tab and the viewer must agree about which URL is dead, and the viewer is rendered outside the tab. Add, directly after the `viewerIndex` declaration (`app/src/lib/Details.svelte:122`):

```svelte
  // One failure map for the Media tab AND the fullscreen viewer: the viewer
  // is rendered outside the tab (above the whole dialog), so a map owned by
  // either one would let the two disagree about which screenshot is dead.
  // Keyed by URL, exactly like OverviewTab's own `failedScreenshots`.
  let failedMedia = $state<Record<string, true>>({});
  function markMediaFailed(url: string) {
    failedMedia = { ...failedMedia, [url]: true };
  }
```

- [ ] **Step 2: Implement the tri-state** in `app/src/lib/Image.svelte`. Replace `:20-45` with:

```svelte
  let src = $state<string | null>(null);
  /**
   * Which of the three things this component can be showing:
   * - `loading` — a URL was given and `ensureImage` has not answered yet.
   *   Renders the shimmer skeleton, NOT the text placeholder: before this,
   *   a tile still being fetched and a tile whose image is gone looked
   *   identical, so a slow cache miss read as a permanent failure.
   * - `error` — there is no URL, or the fetch/decode failed. The text
   *   placeholder is the honest answer here, and it is the state `onerror`
   *   is reported in.
   * - `ready` — `src` is set and the <img> is in the DOM.
   */
  let status = $state<'loading' | 'error' | 'ready'>('loading');

  $effect(() => {
    let cancelled = false;
    src = null;
    // A null/blank url has nothing in flight, so it is `error` (the caller's
    // placeholder text), never a skeleton that would shimmer forever.
    status = url ? 'loading' : 'error';
    if (url) {
      api
        .ensureImage(url)
        .then((path) => {
          if (cancelled) return;
          src = convertFileSrc(path);
          status = 'ready';
        })
        .catch(() => {
          // offline/missing image: the caller decides whether to keep showing
          // the placeholder (covers) or drop the tile entirely (screenshots)
          if (cancelled) return;
          status = 'error';
          onerror?.();
        });
    }
    return () => {
      cancelled = true;
    };
  });

  function handleImgError() {
    src = null;
    status = 'error';
    onerror?.();
  }
```

- [ ] **Step 3: Implement the markup.** Replace `Image.svelte:48-59` with:

```svelte
{#if status === 'ready' && src}
  <!-- A decode failure drops back to the placeholder before telling the
       caller: without clearing `src` first, a caller that passes no
       `onerror` (the Library and Server cards) is left with the browser's
       broken-image glyph in the card. -->
  {#if backdrop}
    <img class="backdrop" src={src} alt="" aria-hidden="true" loading="lazy" draggable="false" />
  {/if}
  <img {src} {alt} loading="lazy" draggable="false" onerror={handleImgError} {...rest} />
{:else if status === 'loading'}
  <div class="skeleton" aria-hidden="true" {...rest}></div>
{:else}
  <div class="placeholder" {...rest}>{placeholder}</div>
{/if}
```

`{...rest}` still carries the caller's `data-testid` in all three branches, so `images-a.spec.ts`'s `waitForLoadedImage` keeps working unchanged: it reads `naturalWidth` off whatever element carries the id, and a `<div>` reports 0.

- [ ] **Step 4: Implement the styles.** Replace `Image.svelte:61-72` with:

```svelte
<style>
  .placeholder {
    display: grid;
    place-items: center;
    height: 100%;
    background: var(--surface-2);
    color: var(--text-muted);
    font-size: 0.8rem;
    text-align: center;
    padding: 8px;
  }

  /* The loading state, and the whole point of the tri-state: a shimmer
     says "still coming", a flat placeholder says "there is nothing here".
     Tokens only — the gradient is two existing surface tokens, so it
     tracks the theme. */
  .skeleton {
    height: 100%;
    width: 100%;
    border-radius: inherit;
    background: linear-gradient(
      90deg,
      var(--surface) 25%,
      var(--surface-2) 37%,
      var(--surface) 63%
    );
    background-size: 400% 100%;
    animation: image-shimmer calc(var(--m-slow) * 4) linear infinite;
  }

  @keyframes image-shimmer {
    from {
      background-position: 100% 0;
    }
    to {
      background-position: 0 0;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .skeleton {
      animation: none;
    }
  }
</style>
```

- [ ] **Step 5: Drop failed tiles in `MediaTab.svelte`.** Add the two props to `:6-12`:

```svelte
  let {
    items,
    onOpen,
    failed,
    onScreenshotError,
  }: {
    items: MediaItem[];
    onOpen: (index: number) => void;
    /** URLs whose image failed to load, keyed by URL (owned by Details.svelte). */
    failed: Record<string, true>;
    onScreenshotError: (url: string) => void;
  } = $props();
```

and wrap the tile body (`:17-33`) so a dead screenshot's whole `<button>` disappears while every surviving tile keeps its index:

```svelte
    {#each items as item, i (item.caption)}
      {#if !(item.kind === 'screenshot' && failed[item.url])}
        <button
          class="tile"
          data-testid={`details-media-${i}`}
          title={item.caption}
          onclick={() => onOpen(i)}
        >
          {#if item.kind === 'screenshot'}
            <Image
              url={item.url}
              alt={item.caption}
              placeholder="Screenshot"
              onerror={() => onScreenshotError(item.url)}
            />
          {:else}
            <div class="video-tile">
              <Icon name="play" size={20} />
              <span>{item.kind === 'youtube' ? 'Trailer' : 'Video'}</span>
            </div>
          {/if}
        </button>
      {/if}
    {/each}
```

(The `{:else}` branch is replaced wholesale by Task 3; this step only lands the failure handling and keeps the file compiling.)

- [ ] **Step 6: Never show a dead frame in `MediaViewer.svelte`.** Add the same two props to `:8-18`:

```svelte
    failed,
    onScreenshotError,
  }: {
    items: MediaItem[];
    index: number;
    onIndex: (index: number) => void;
    onClose: () => void;
    failed: Record<string, true>;
    onScreenshotError: (url: string) => void;
  } = $props();
```

and replace the screenshot branch of the stage (`:119-125`) with:

```svelte
      {#if current.kind === 'screenshot' && failed[current.url]}
        <!-- User ruling 2026-09-05: the viewer does NOT auto-advance past a
             dead screenshot. Dropping the item would shift every index under
             the user, and advancing would loop forever when every item
             fails; an explicit line is the honest answer. The tile itself is
             already gone from the Media tab behind this. -->
        <p class="pending" data-testid="media-viewer-image-error">
          This screenshot could not be loaded
        </p>
      {:else if current.kind === 'screenshot'}
        <Image
          url={current.url}
          alt={current.caption}
          placeholder="Screenshot"
          data-testid="media-viewer-image"
          onerror={() => onScreenshotError(current.url)}
        />
```

- [ ] **Step 7: Wire both call sites** in `app/src/lib/Details.svelte`. `:674` becomes:

```svelte
            <MediaTab
              items={mediaItems}
              onOpen={(i) => (viewerIndex = i)}
              failed={failedMedia}
              onScreenshotError={markMediaFailed}
            />
```

and the `<MediaViewer` opening at `:725-728` gains the same two props after `onIndex`:

```svelte
    failed={failedMedia}
    onScreenshotError={markMediaFailed}
```

- [ ] **Step 8: Run** `npx vitest run` and, from `app/`, `npm run check` — green, no new warnings.

- [ ] **Step 9: Commit**

```bash
git add app/src/lib/Image.svelte app/src/lib/details/MediaTab.svelte app/src/lib/details/MediaViewer.svelte app/src/lib/Details.svelte
git commit -m "rewrite: tell a loading image apart from a dead one, and drop failed screenshot tiles"
```

---

### Task 3: The trailer tile shows artwork, not a bare icon

**Files:**
- Modify: `app/src/lib/details/media.ts` (append after `youtubeEmbedUrl`, `:29-32`)
- Modify: `app/src/lib/details/media.test.ts` (new describes)
- Modify: `app/src/lib/details/MediaTab.svelte` (the `{:else}` branch from Task 2 Step 5, and the `<style>`)
- Modify: `app/src/lib/Details.svelte:674` (one new prop)
- Modify: `app/src-tauri/tauri.conf.json:30` (`img-src`)

**Interfaces:**
- Produces, in `details/media.ts`:
  - `export const YOUTUBE_THUMBNAIL_BASE = 'https://img.youtube.com/vi';`
  - `export function youtubeThumbnailUrl(videoId: string): string` — `''` for anything that is not an 11-character id.
  - `export type TilePoster = { kind: 'youtube'; url: string } | { kind: 'cover'; url: string | null };`
  - `export function trailerPoster(videoId: string, coverUrl: string | null, thumbnailFailed: boolean): TilePoster`
- Produces, in `MediaTab.svelte`: a new prop `coverUrl: string | null` (the game's large cover, the fallback poster and the hosted-video poster).
- Consumes: `isYoutubeId` (`details/media.ts:25-27`), `Image.svelte` (for the cover poster only — the YouTube thumbnail is a plain `<img>`).

- [ ] **Step 1: Write the failing tests** in `app/src/lib/details/media.test.ts`. Add `trailerPoster`, `youtubeThumbnailUrl` and `YOUTUBE_THUMBNAIL_BASE` to the import list at the top, then append:

```ts
describe('youtubeThumbnailUrl', () => {
  it('builds the static CDN path for a valid id', () => {
    expect(youtubeThumbnailUrl('dQw4w9WgXcQ')).toBe(
      'https://img.youtube.com/vi/dQw4w9WgXcQ/hqdefault.jpg'
    );
  });

  it('trims before building', () => {
    expect(youtubeThumbnailUrl('  dQw4w9WgXcQ  ')).toBe(
      'https://img.youtube.com/vi/dQw4w9WgXcQ/hqdefault.jpg'
    );
  });

  // The id is interpolated into a URL the page loads directly, so anything
  // that is not exactly an id must produce nothing at all.
  it('is blank for anything that is not an 11-character id', () => {
    expect(youtubeThumbnailUrl('')).toBe('');
    expect(youtubeThumbnailUrl('short')).toBe('');
    expect(youtubeThumbnailUrl('https://youtu.be/dQw4w9WgXcQ')).toBe('');
    expect(youtubeThumbnailUrl('../../etc/passwd')).toBe('');
  });

  it('never leaves the one allowed foreign host', () => {
    expect(youtubeThumbnailUrl('dQw4w9WgXcQ').startsWith(`${YOUTUBE_THUMBNAIL_BASE}/`)).toBe(true);
  });
});

describe('trailerPoster', () => {
  it("prefers YouTube's own thumbnail for a valid id", () => {
    expect(trailerPoster('dQw4w9WgXcQ', 'https://romm/cover.png', false)).toEqual({
      kind: 'youtube',
      url: 'https://img.youtube.com/vi/dQw4w9WgXcQ/hqdefault.jpg',
    });
  });

  it('falls back to the server-hosted cover once the thumbnail has failed', () => {
    expect(trailerPoster('dQw4w9WgXcQ', 'https://romm/cover.png', true)).toEqual({
      kind: 'cover',
      url: 'https://romm/cover.png',
    });
  });

  it('falls back to the cover when there is no usable id', () => {
    expect(trailerPoster('', 'https://romm/cover.png', false)).toEqual({
      kind: 'cover',
      url: 'https://romm/cover.png',
    });
    expect(trailerPoster('not-an-id', 'https://romm/cover.png', false)).toEqual({
      kind: 'cover',
      url: 'https://romm/cover.png',
    });
  });

  it('reports a cover poster with no cover, which the tile renders as its placeholder', () => {
    expect(trailerPoster('', null, false)).toEqual({ kind: 'cover', url: null });
  });
});
```

- [ ] **Step 2: Run** `npx vitest run media` — the new cases fail (nothing exported).

- [ ] **Step 3: Implement** in `app/src/lib/details/media.ts`, directly under `youtubeEmbedUrl`:

```ts
/**
 * YouTube's static thumbnail CDN. User ruling 2026-09-05: this is the ONE
 * foreign host anything in this app may load, because `/vi/<id>/hqdefault.jpg`
 * needs no API key, no quota and no cookie. It is loaded as a plain `<img>`
 * with `referrerpolicy="no-referrer"` — NEVER through `ensure_image`, which
 * would fetch it via `RommClient` and attach the RomM Authorization header
 * to a request leaving the server's host.
 */
export const YOUTUBE_THUMBNAIL_BASE = 'https://img.youtube.com/vi';

/** The thumbnail URL for `videoId`, or `''` when it is not an 11-character id. */
export function youtubeThumbnailUrl(videoId: string): string {
  const id = videoId.trim();
  if (!isYoutubeId(id)) return '';
  return `${YOUTUBE_THUMBNAIL_BASE}/${id}/hqdefault.jpg`;
}

/** What a trailer/video tile paints behind its play badge. */
export type TilePoster =
  | { kind: 'youtube'; url: string }
  | { kind: 'cover'; url: string | null };

/**
 * The trailer tile's poster. YouTube's thumbnail when there is a real id and
 * it has not already failed to load (offline, or a video with no thumbnail);
 * the game's own server-hosted cover otherwise. `{ kind: 'cover', url: null }`
 * means "no artwork at all" — the tile renders its placeholder, which is
 * still better than the bare play icon this replaces.
 */
export function trailerPoster(
  videoId: string,
  coverUrl: string | null,
  thumbnailFailed: boolean
): TilePoster {
  const thumbnail = youtubeThumbnailUrl(videoId);
  if (thumbnail !== '' && !thumbnailFailed) return { kind: 'youtube', url: thumbnail };
  return { kind: 'cover', url: coverUrl };
}
```

- [ ] **Step 4: Run** `npx vitest run media` — green.

- [ ] **Step 5: Render the poster** in `app/src/lib/details/MediaTab.svelte`. Add `coverUrl` to the props block:

```svelte
    /** The game's large cover — the poster for a hosted video, and the
     *  fallback poster when YouTube's thumbnail cannot be reached. */
    coverUrl: string | null;
```

Add to the imports and the script body:

```svelte
  import { trailerPoster, type MediaItem } from './media';
```

```svelte
  // Which YouTube thumbnails have failed, keyed by video id: an offline
  // launcher must fall back to the cover once, not retry on every re-render.
  let thumbnailFailed = $state<Record<string, true>>({});
  function markThumbnailFailed(videoId: string) {
    thumbnailFailed = { ...thumbnailFailed, [videoId]: true };
  }
```

Replace the `{:else}` branch of the tile loop (Task 2 Step 5) with:

```svelte
          {:else if item.kind === 'youtube'}
            {@const poster = trailerPoster(item.videoId, coverUrl, thumbnailFailed[item.videoId] === true)}
            <div class="video-tile">
              {#if poster.kind === 'youtube'}
                <!-- Plain <img>, deliberately NOT `Image.svelte`: that
                     component fetches through `ensure_image` -> RommClient,
                     which would attach the RomM token to a foreign host.
                     `no-referrer` keeps this app's URL out of the request. -->
                <img
                  data-testid={`details-media-thumb-${i}`}
                  class="poster"
                  src={poster.url}
                  alt=""
                  aria-hidden="true"
                  loading="lazy"
                  referrerpolicy="no-referrer"
                  draggable="false"
                  onerror={() => markThumbnailFailed(item.videoId)}
                />
              {:else}
                <Image
                  url={poster.url}
                  alt=""
                  placeholder="Trailer"
                  data-testid={`details-media-poster-${i}`}
                />
              {/if}
              <span class="play-badge" data-testid={`details-media-play-${i}`}>
                <Icon name="play" size={20} />
              </span>
              <span class="video-label">Trailer</span>
            </div>
          {:else}
            <div class="video-tile">
              <Image
                url={coverUrl}
                alt=""
                placeholder="Video"
                data-testid={`details-media-poster-${i}`}
              />
              <span class="play-badge" data-testid={`details-media-play-${i}`}>
                <Icon name="play" size={20} />
              </span>
              <span class="video-label">Video</span>
            </div>
          {/if}
```

- [ ] **Step 6: Styles.** Replace the `.video-tile` rule in `MediaTab.svelte`'s `<style>` with:

```css
  .video-tile {
    position: relative;
    height: 100%;
    width: 100%;
    display: block;
    color: var(--text);
    font-size: 14px;
  }

  /* Both posters fill the tile the same way the screenshot tiles do; the
     `.tile :global(img)` rule above already sizes the <Image> branch. */
  .video-tile .poster {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .play-badge {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border-radius: var(--r-pill);
    background: rgba(0, 0, 0, 0.55);
    color: #fff;
  }

  .video-label {
    position: absolute;
    left: 8px;
    bottom: 6px;
    padding: 2px 8px;
    border-radius: var(--r-chip);
    background: rgba(0, 0, 0, 0.65);
    color: #fff;
    font-size: 11px;
    font-weight: 600;
  }
```

(The two `rgba(0, 0, 0, …)` scrims and the `#fff` on them are the same literals `GameCard.svelte`'s `.tag` / `.actions button` already use for text over artwork; they are scrim values, not theme colours, and there is no token for them.)

- [ ] **Step 7: Pass the cover** in `app/src/lib/Details.svelte` — add `coverUrl={coverLarge ?? coverSmall}` to the `<MediaTab …>` call Task 2 Step 7 rewrote. (`coverLarge`/`coverSmall` are the derived values at `:108-109`, the same pair `details-cover` renders at `:528`.)

- [ ] **Step 8: CSP.** In `app/src-tauri/tauri.conf.json`, change the `img-src` line (`:30`) to:

```json
        "img-src": "'self' asset: http://asset.localhost https://img.youtube.com",
```

Nothing else in `security.csp` changes — `frame-src` already allows `https://www.youtube-nocookie.com` for the viewer's iframe.

- [ ] **Step 9: Run** `npx vitest run` and, from `app/`, `npm run check` — green, no new warnings.

- [ ] **Step 10: Commit**

```bash
git add app/src/lib/details/media.ts app/src/lib/details/media.test.ts app/src/lib/details/MediaTab.svelte app/src/lib/Details.svelte app/src-tauri/tauri.conf.json
git commit -m "rewrite: give the trailer tile YouTube's thumbnail with a cover fallback"
```

---

### Task 4: `cloud-upload-finished` — the backend event

**Files:**
- Modify: `app/src-tauri/src/cloud_service.rs` — the `use` block (`:29-58`), a new event const + payload + mapper next to the DTOs (`:1455+`), `CloudService::upload` (`:323-362`), `install_session_finished_hook` (`:771-790`), `handle_session_finished` (`:798-900`), `run_auto_upload` (`:924-1029`), the test module
- Modify: `app/src-tauri/src/commands/cloud.rs:52-71` (`cloud_upload`)
- Modify: `app/src-tauri/src/lib.rs:259-264` (the hook call site)

**Interfaces:**
- Produces, in `cloud_service.rs`:
  ```rust
  pub const CLOUD_UPLOAD_FINISHED_EVENT: &str = "cloud-upload-finished";

  #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
  pub struct CloudUploadFinished {
      pub title: String,
      pub message: String,
      pub failed: bool,
  }

  fn upload_finished_payload(title: &str, messages: &[CloudMessage]) -> Option<CloudUploadFinished>;
  fn emit_upload_finished(app: &AppHandle, title: &str, messages: &[CloudMessage]);
  ```
- Changes: `CloudService::upload(&self, app: &AppHandle, session, install, launch, config_path, game, save_type)` — new FIRST argument.
- Changes: `CloudService::install_session_finished_hook(self: &Arc<Self>, app: AppHandle, launch, session_mgr, install, config_path)` — new FIRST argument, threaded into `handle_session_finished` and then `run_auto_upload`.
- Consumes: `grid_core::cloud::ops::CloudMessage` (`crates/grid-core/src/cloud/ops/mod.rs:67-71`) and `grid_core::cloud::transfer::MessageSeverity` (`transfer.rs:826-830`), both already imported by this file (`:40`, `:46`). `tauri::{AppHandle, Emitter}` follow `firmware_service.rs:69`.

Why the message text is not re-derived: `ops::upload::upload_cloud_files_for_game` already builds it from `upload_completion_message` and returns it on `UploadReport.messages` (`crates/grid-core/src/cloud/ops/upload.rs:212-221`). `run_auto_upload` throws those away today (`:993-1009`); this task keeps them. One source of text, so the toast and the panel's inline line can never disagree.

- [ ] **Step 1: Write the failing Rust tests** in `app/src-tauri/src/cloud_service.rs`'s test module, next to the existing `record_dtos_*` tests:

```rust
    fn info(text: &str) -> CloudMessage {
        CloudMessage {
            text: text.to_string(),
            severity: MessageSeverity::Info,
        }
    }

    fn warn(text: &str) -> CloudMessage {
        CloudMessage {
            text: text.to_string(),
            severity: MessageSeverity::Warning,
        }
    }

    #[test]
    fn upload_finished_payload_reports_a_clean_upload_as_a_success() {
        let payload = upload_finished_payload("Chrono Trigger", &[info("Uploaded 2 save files.")])
            .expect("a completed upload always has a message");
        assert_eq!(payload.title, "Chrono Trigger");
        assert_eq!(payload.message, "Uploaded 2 save files.");
        assert!(!payload.failed);
    }

    #[test]
    fn upload_finished_payload_reports_a_partial_upload_as_a_failure() {
        // `upload_completion_message` marks a partial upload Warning
        // (transfer.rs:870-885); the toast follows that severity rather than
        // guessing from counts it was never given.
        let payload =
            upload_finished_payload("Chrono Trigger", &[warn("Uploaded 1 save files. Failed: b.sav")])
                .expect("a completed upload always has a message");
        assert_eq!(payload.message, "Uploaded 1 save files. Failed: b.sav");
        assert!(payload.failed);
    }

    #[test]
    fn upload_finished_payload_reports_a_total_failure() {
        let payload =
            upload_finished_payload("Chrono Trigger", &[warn("Cloud upload failed for all matching files.")])
                .expect("a completed upload always has a message");
        assert_eq!(payload.message, "Cloud upload failed for all matching files.");
        assert!(payload.failed);
    }

    #[test]
    fn upload_finished_payload_joins_both_save_types_and_fails_if_either_did() {
        // An auto upload can run Save AND State in one pass
        // (`auto_cloud_upload_plan`), producing one message each.
        let payload = upload_finished_payload(
            "Chrono Trigger",
            &[info("Uploaded 2 save files."), warn("Cloud upload failed for all matching files.")],
        )
        .expect("a completed upload always has a message");
        assert_eq!(
            payload.message,
            "Uploaded 2 save files. Cloud upload failed for all matching files."
        );
        assert!(payload.failed);
    }

    #[test]
    fn upload_finished_payload_is_none_when_nothing_ran() {
        // A plan with no save types, or a stop-with-no-message: there is
        // nothing to tell the user, so no event and no toast.
        assert!(upload_finished_payload("Chrono Trigger", &[]).is_none());
        assert!(upload_finished_payload("Chrono Trigger", &[info("   ")]).is_none());
    }

    #[test]
    fn upload_finished_payload_keeps_a_blank_title_out_of_the_message() {
        let payload = upload_finished_payload("  ", &[info("Uploaded 1 save files.")])
            .expect("a completed upload always has a message");
        assert_eq!(payload.title, "");
    }
```

If `MessageSeverity` / `CloudMessage` are not already in scope inside the test module, add `use super::*;`-visible imports at the top of the module rather than re-importing them per test.

- [ ] **Step 2: Run** `cargo test -p app cloud_service::` — the new tests fail to compile (`upload_finished_payload` does not exist).

- [ ] **Step 3: Implement the event, payload and mapper** in `cloud_service.rs`, immediately above `CloudMessageDto` (`:1462`):

```rust
/// Emitted after EVERY cloud upload that actually ran — the manual button in
/// the details cloud panel and the D5 auto-upload after a session exits
/// alike. Before this, an auto upload reported only into `tracing::debug!`
/// (`run_auto_upload`), so a user who left a game had no way to know whether
/// their save reached the server.
pub const CLOUD_UPLOAD_FINISHED_EVENT: &str = "cloud-upload-finished";

/// The [`CLOUD_UPLOAD_FINISHED_EVENT`] payload. Token secrecy: a game title
/// and the completion message `upload_completion_message` already produced
/// (`crates/grid-core/src/cloud/transfer.rs:855-901`) — no path, no URL, no
/// header, nothing derived from a credential.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CloudUploadFinished {
    /// The game's title, trimmed. `""` when the caller has none.
    pub title: String,
    /// Every completion message from this run, space-joined in save-type
    /// order (Save then State).
    pub message: String,
    /// `true` when ANY message was a `Warning` — a total failure and a
    /// partial upload alike. The frontend raises an error-level toast for it.
    pub failed: bool,
}

/// Maps one upload run's messages onto the toast payload, or `None` when the
/// run produced nothing worth reporting (an empty plan, or only blank text).
/// `failed` follows `MessageSeverity`, NOT a count: `upload_completion_message`
/// already decided that a partial upload is a warning, and re-deriving it
/// from `uploaded`/`total` here would let the toast and the panel's own line
/// disagree about the same run.
fn upload_finished_payload(title: &str, messages: &[CloudMessage]) -> Option<CloudUploadFinished> {
    let text: Vec<&str> = messages
        .iter()
        .map(|m| m.text.trim())
        .filter(|t| !t.is_empty())
        .collect();
    if text.is_empty() {
        return None;
    }
    Some(CloudUploadFinished {
        title: title.trim().to_string(),
        message: text.join(" "),
        failed: messages
            .iter()
            .any(|m| m.severity == MessageSeverity::Warning),
    })
}

/// Emits [`CLOUD_UPLOAD_FINISHED_EVENT`]. A webview that has gone away is not
/// an error: the upload itself already happened. Mirrors
/// `firmware_service::emit_pass_finished`.
fn emit_upload_finished(app: &AppHandle, title: &str, messages: &[CloudMessage]) {
    if let Some(payload) = upload_finished_payload(title, messages) {
        let _ = app.emit(CLOUD_UPLOAD_FINISHED_EVENT, payload);
    }
}
```

Add `use tauri::{AppHandle, Emitter};` to the file's `use` block (after the `serde` imports at `:56-57`).

- [ ] **Step 4: Emit from the manual path.** In `CloudService::upload` (`:323-362`), add `app: &AppHandle,` as the FIRST parameter (before `session`), and replace the final two lines of the body with:

```rust
        let report = ops::upload::upload_cloud_files_for_game(
            &client,
            &ctx,
            &mut caches,
            &cloud_game,
            save_type,
        )
        .await;
        // The panel already renders `report.messages` inline; the toast is
        // the same text, so a user who has scrolled the panel away still
        // learns the result. Both come from one `upload_completion_message`.
        emit_upload_finished(app, &cloud_game.title, &report.messages);
        Ok(UploadReportDto::from(report))
```

The early "not installed" return at `:343-348` stays silent: nothing was attempted, so there is nothing to report.

- [ ] **Step 5: Thread the handle into the auto path.** In `install_session_finished_hook` (`:771-790`), add `app: AppHandle,` as the FIRST parameter, clone it into the closure alongside `cloud`/`session_mgr`/`install`/`config_path`, and pass it into `handle_session_finished`. `handle_session_finished` (`:798`) takes `app: AppHandle` as its first parameter after `self`, and passes it to `run_auto_upload` (add `app: AppHandle` to that signature after `key: String`; it already carries `#[allow(clippy::too_many_arguments)]`). Move it into the `pool.trigger` closure the same way `cloud_for_task` is.

- [ ] **Step 6: Emit from the auto path.** In `run_auto_upload`, collect the messages while uploading — replace the `for save_type in plan.types.clone()` loop body's tail so the report's messages are kept:

```rust
        let mut per_type: BTreeMap<SaveType, PerTypeResult> = BTreeMap::new();
        // Kept, not dropped: these are the SAME `upload_completion_message`
        // strings the manual panel shows, and they are what the toast says.
        let mut messages: Vec<CloudMessage> = Vec::new();
        for save_type in plan.types.clone() {
            let report = ops::upload::upload_cloud_files_for_game(
                &client,
                &ctx,
                &mut caches,
                &game,
                save_type,
            )
            .await;
            messages.extend(report.messages.iter().cloned());
            per_type.insert(
                save_type,
                PerTypeResult {
                    uploaded: report.uploaded as i64,
                    total: report.total as i64,
                    failed: report.failed,
                },
            );
        }
        emit_upload_finished(&app, &game.title, &messages);
```

The `if plan.types.is_empty() { return; }` guard above stays: an exit that had nothing new to upload must not toast.

- [ ] **Step 7: Update the command.** In `app/src-tauri/src/commands/cloud.rs`, `cloud_upload` takes the handle Tauri already offers commands:

```rust
#[tauri::command]
pub async fn cloud_upload(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    game: CloudGameInput,
    save_type: SaveType,
) -> Result<UploadReportDto, String> {
    let install = state.install.as_ref().map_err(Clone::clone)?.clone();
    let launch = state.launch.as_ref().map_err(Clone::clone)?.clone();
    state
        .cloud
        .upload(
            &app,
            &state.session,
            install,
            launch,
            &Config::default_path(),
            game,
            save_type,
        )
        .await
}
```

The frontend's `invoke('cloud_upload', { game, saveType })` is unchanged: Tauri injects `AppHandle` itself and never expects it in the JS argument object.

- [ ] **Step 8: Update the hook call site** in `app/src-tauri/src/lib.rs:259-264`:

```rust
                    state.cloud.install_session_finished_hook(
                        app.handle().clone(),
                        launch,
                        state.session.clone(),
                        install.clone(),
                        Config::default_path(),
                    );
```

`app.handle().clone()` is exactly what the downloads notify (`:176-178`), the firmware/updates game-finalized hook (`:209`) and the compat-tools hook (`:245`) already do a few lines above.

- [ ] **Step 9: Run** `cargo test -p app cloud_service::` (green), then `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo clippy -p app --all-targets --features e2e -- -D warnings`. The `e2e` clippy run matters here: the `e2e` feature only gates the embedded WebDriver server (`lib.rs:95-115`), but this task changes a `setup()` call site, so both feature shapes must still build.

- [ ] **Step 10: Run** `cargo test --workspace` — green.

- [ ] **Step 11: Commit**

```bash
git add app/src-tauri/src/cloud_service.rs app/src-tauri/src/commands/cloud.rs app/src-tauri/src/lib.rs
git commit -m "rewrite: emit cloud-upload-finished after every manual and auto upload"
```

---

### Task 5: The upload toast, and its E2E

**Files:**
- Modify: `app/src/lib/api.ts` (a new type + const next to `FIRMWARE_PASS_FINISHED_EVENT`, `:311-320`)
- Modify: `app/src/lib/Shell.svelte:1-22` (imports), `:79-94` (the effects block)
- Modify: `e2e/specs/cloud-saves.spec.ts:157-181` (the manual-upload case) and `:208-257` (the exit auto-upload case)

**Interfaces:**
- Produces, in `api.ts`:
  ```ts
  export type CloudUploadFinished = { title: string; message: string; failed: boolean };
  export const CLOUD_UPLOAD_FINISHED_EVENT = 'cloud-upload-finished';
  ```
- Consumes: `listen` from `@tauri-apps/api/event` (the pattern at `Details.svelte:197-205`), and `pushToast` from `stores/toasts.svelte.ts` (`pushToast(text, level: 'success' | 'error' = 'success')`).

The Shell is the right listener because it is mounted exactly once and is never `hidden` (the same reason `Toast.svelte` is mounted there, `Shell.svelte:211-213`). A listener inside `Details.svelte` would miss every auto upload that happens while the popup is closed — which is all of them.

- [ ] **Step 1: Add the type and the event name** in `app/src/lib/api.ts`, directly under the `FIRMWARE_PASS_FINISHED_EVENT` block (`:311-320`):

```ts
/// The `cloud-upload-finished` payload (`app/src-tauri/src/cloud_service.rs`'s
/// `CloudUploadFinished`). `message` is the completion text
/// `upload_completion_message` produced for this run; `failed` is true for a
/// partial upload as well as a total one. Carries no path, URL or token.
export type CloudUploadFinished = { title: string; message: string; failed: boolean };

/// Emitted after every cloud upload that ran — the manual panel button and
/// the auto upload after a game exits. The Shell turns it into one toast.
export const CLOUD_UPLOAD_FINISHED_EVENT = 'cloud-upload-finished';
```

- [ ] **Step 2: Listen once** in `app/src/lib/Shell.svelte`. Add to the imports:

```svelte
  import { listen } from '@tauri-apps/api/event';
  import { CLOUD_UPLOAD_FINISHED_EVENT, type CloudUploadFinished } from './api';
  import { pushToast } from './stores/toasts.svelte';
```

(`api` itself is already imported at `:11`; extend that import rather than adding a second one from the same module.)

Add, after the `seedLastViewed` effect (`:92-94`):

```svelte
  // The only report an auto upload has: it runs after the game has exited,
  // with no command in flight and usually no popup open. Mounted here, not
  // in Details.svelte, because the Shell is mounted exactly once and is
  // never `hidden` — the same reason `Toast.svelte` lives here.
  $effect(() => {
    const unlisten = listen<CloudUploadFinished>(CLOUD_UPLOAD_FINISHED_EVENT, (e) => {
      const { title, message, failed } = e.payload;
      pushToast(title === '' ? message : `${title} — ${message}`, failed ? 'error' : 'success');
    });
    return () => {
      void unlisten.then((off) => off());
    };
  });
```

- [ ] **Step 3: Run** `npx vitest run` and, from `app/`, `npm run check` — green, no new warnings. (`stores/toasts.test.ts` already covers `appendToast`/`removeToast`; this task adds no new pure logic to test — the mapping it consumes is Task 4's Rust unit tests, and the wiring is proved by the E2E below.)

- [ ] **Step 4: Assert the toast on the manual upload** in `e2e/specs/cloud-saves.spec.ts`. In the `'manual upload: the panel Upload button POSTs one overwrite=true multipart request'` case, insert the toast assertion immediately after the `cloud-upload` click and before the request polling — a toast lives for `TOAST_DURATION_MS` (4000 ms, `stores/toasts.svelte.ts`), so it must be read first:

```ts
    const before = (await mockRequests()).length;
    await $(testId('cloud-upload')).click();

    // Round 4: every upload now reports through the shell toast, so a user
    // who has scrolled the panel away still learns the result. rom 601 has
    // exactly one local save file (cloud-saves-seed.mjs), so the text is the
    // Info branch of `upload_completion_message` (transfer.rs:897-900).
    await $(testId('toast')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'no toast appeared after the manual upload',
    });
    await expect($(testId('toast'))).toHaveText('SaveSyncManual — Uploaded 1 save files.');

    await browser.waitUntil(
      async () => (await mockRequests()).length > before,
      { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the mock never received the manual upload' },
    );
```

- [ ] **Step 5: Assert the toast on the exit auto upload** in the `'exit: the auto upload fires after the (zeroed) delay with the stub-written content'` case. The auto upload is what this whole feature exists for, and the fixture already drives it. Add, immediately after the existing `await browser.waitUntil(... uploads.length > 0 ...)` block:

```ts
    // The auto upload's own toast — the round-4 gap this closes. The wait
    // above returns as soon as the POST reaches the mock, and the event is
    // emitted on the same task right after that POST resolves, so the toast
    // is still inside its 4 s window here.
    await $(testId('toast')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'no toast appeared after the auto upload on exit',
    });
    await expect($(testId('toast'))).toHaveText('SaveSyncLaunch — Uploaded 1 save files.');
```

- [ ] **Step 6: Reconcile both texts with a real run.** The two `toHaveText` strings assume one uploaded file each and the Info branch. Before committing, run the group once (`scripts/e2e.sh cloud-saves`, detached, log to a file) and read the actual toast text from the failure message if either assertion fails. If the real text differs (a second file in the seed, or a retention warning), replace that single assertion with `expect(await $(testId('toast')).getText()).toContain('SaveSyncManual')` plus `toMatch(/Uploaded \d+ save files\./)` and note why in a comment. Do NOT change the toast's own wording to fit the test — it is `upload_completion_message`'s, and Task 4's Rust tests pin it.

- [ ] **Step 7: Commit**

```bash
git add app/src/lib/api.ts app/src/lib/Shell.svelte e2e/specs/cloud-saves.spec.ts
git commit -m "rewrite: toast the result of every cloud upload from the shell"
```

---

### Task 6: Fanart and screenshots on the summary, the detail and the registry row

**Files:**
- Modify: `crates/grid-core/src/images/urls.rs` (new `fanart_urls_from_payload` after `screenshot_urls_from_payload`, `:277-380`) and its test module
- Modify: `crates/grid-core/src/romm/mod.rs:194-241` (`GameSummary`, `RawGameSummary`, the `From` impl), `:265-284` (`games`), `:388-434` (`RomDetail`), `:520-611` (`RawRomDetail::into_detail`), new test module
- Modify: `crates/grid-core/src/images/mod.rs:22-39` (`ImageFields`)
- Modify: `crates/grid-core/src/library/registry.rs:12-53` (`SCHEMA_SQL`), `:58` (`LATEST_USER_VERSION`), `:80-84` (new column const), `:145-168` (new migration), `:338-350` (the migration match), `:173-178` (`SELECT_COLUMNS`), `:184-238` (`InstalledGame`), `:240-278` (`from_row`), `:365-470` (`upsert`), `:475-490` (`update_images`)
- Modify: `crates/grid-core/src/library/mod.rs:1590-1594` (the `ImageFields` literal)
- Modify: `crates/grid-core/tests/registry.rs` (ten `assert_eq!(version, 4)` sites at `:69, :83, :258, :306, :363, :399, :440, :558, :594, :621`; the two `ImageFields` literals at `:266` and `:326`; two test names)
- Modify: `e2e/seed/registry-schema.mjs:22` (`USER_VERSION`) and `:49` (the schema text)
- Modify: `app/src/lib/api.ts:26-32` (`GameSummary`), `:48-75` (`RomDetail`), `:176-200` (`InstalledGame`)
- Modify: `app/src/lib/details/subject.ts` (`DetailsSubject`, `fromSummary`, `fromInstalled`, `summaryOf`, `mergeDetail`) and `subject.test.ts`

**Interfaces:**
- Produces: `pub fn fanart_urls_from_payload(payload: &Value, resolver: &dyn Fn(&str) -> String) -> Vec<String>` in `images/urls.rs`.
- Produces: `RomDetail.fanart_urls: Vec<String>`; `GameSummary.screenshot_urls: Vec<String>` and `GameSummary.fanart_urls: Vec<String>` (both `#[serde(default)]`); `ImageFields.fanart_urls: String`; `InstalledGame.fanart_urls: String`.
- Changes: `RawGameSummary` gains `#[serde(flatten)] extra: serde_json::Map<String, serde_json::Value>` and the `From<RawGameSummary> for GameSummary` impl is replaced by `fn into_summary(self, base_url: &str) -> GameSummary`, because resolving a screenshot or fanart path needs the server URL and `From` cannot take one.
- Changes: `Registry` `LATEST_USER_VERSION` 4 → 5.
- Consumes: `crate::images::urls::server_resolver` (`urls.rs:214-218`) — the same resolver `into_detail` already builds (`romm/mod.rs:571`).

Wire evidence (verified against `openapi.json`): both `SimpleRomSchema` (the `/api/roms` list) and `DetailedRomSchema` carry `merged_screenshots`, `ss_metadata` and `gamelist_metadata`; `RomSSMetadata` and `RomGamelistMetadata` both carry `fanart_url` and `fanart_path`. `fanart_path` is server-relative and survives `filter_to_server_host`; `fanart_url` is usually an external host and is dropped by that filter, which is correct — nothing but the YouTube thumbnail may leave the server host.

- [ ] **Step 1: Write the failing extractor tests** in `crates/grid-core/src/images/urls.rs`'s test module, next to the existing `screenshot_urls_from_payload` cases:

```rust
    #[test]
    fn fanart_is_read_from_both_metadata_blocks_and_resolved_against_the_server() {
        let resolver = server_resolver("https://romm.example");
        let payload = serde_json::json!({
            "ss_metadata": { "fanart_path": "/assets/romm/resources/roms/1/fanart.jpg" },
            "gamelist_metadata": { "fanart_path": "/assets/romm/resources/roms/1/gl-fanart.jpg" }
        });
        assert_eq!(
            fanart_urls_from_payload(&payload, &resolver),
            vec![
                "https://romm.example/assets/romm/resources/roms/1/fanart.jpg".to_string(),
                "https://romm.example/assets/romm/resources/roms/1/gl-fanart.jpg".to_string(),
            ]
        );
    }

    /// The whole reason this is a separate function: `NON_SCREENSHOT_ART_RE`
    /// (`urls.rs:32-37`) rejects any URL containing "fanart", so routing
    /// fanart through `screenshot_urls_from_payload`'s own
    /// `looks_like_screenshot_url` filter would drop every one of them.
    #[test]
    fn fanart_is_not_filtered_by_the_screenshot_art_regex() {
        let resolver = server_resolver("https://romm.example");
        let payload = serde_json::json!({ "ss_metadata": { "fanart_path": "/art/fanart.jpg" } });
        assert!(!looks_like_screenshot_url("/art/fanart.jpg"));
        assert_eq!(
            fanart_urls_from_payload(&payload, &resolver),
            vec!["https://romm.example/art/fanart.jpg".to_string()]
        );
    }

    #[test]
    fn a_foreign_fanart_url_is_dropped_by_the_host_filter() {
        let resolver = server_resolver("https://romm.example");
        let payload = serde_json::json!({
            "ss_metadata": { "fanart_url": "https://cdn.elsewhere/fanart.jpg" }
        });
        assert!(fanart_urls_from_payload(&payload, &resolver).is_empty());
    }

    #[test]
    fn fanart_de_duplicates_and_ignores_blanks_and_missing_blocks() {
        let resolver = server_resolver("https://romm.example");
        let payload = serde_json::json!({
            "ss_metadata": { "fanart_path": "/a/fanart.jpg", "fanart_url": "/a/fanart.jpg" },
            "gamelist_metadata": { "fanart_path": "" }
        });
        assert_eq!(
            fanart_urls_from_payload(&payload, &resolver),
            vec!["https://romm.example/a/fanart.jpg".to_string()]
        );
        assert!(fanart_urls_from_payload(&serde_json::json!({}), &resolver).is_empty());
    }
```

- [ ] **Step 2: Run** `cargo test -p grid-core fanart` — fails to compile.

- [ ] **Step 3: Implement the extractor** in `crates/grid-core/src/images/urls.rs`, directly after `screenshot_urls_from_payload`:

```rust
/// The rom's fanart URLs, resolved and host-filtered, in a stable order:
/// `ss_metadata` before `gamelist_metadata`, and `fanart_path` (server
/// relative) before `fanart_url` (usually an external host, and therefore
/// usually dropped by the host filter — which is the desired outcome).
///
/// Deliberately NOT routed through `looks_like_screenshot_url`:
/// `NON_SCREENSHOT_ART_RE` exists to keep fanart, box art and logos OUT of a
/// screenshot list, so applying it here would reject every fanart by name.
/// Both metadata blocks are present on `SimpleRomSchema` and
/// `DetailedRomSchema`, so the grid payload and the detail payload feed the
/// same function.
pub fn fanart_urls_from_payload(
    payload: &Value,
    resolver: &dyn Fn(&str) -> String,
) -> Vec<String> {
    let mut urls: Vec<String> = Vec::new();
    for block in ["ss_metadata", "gamelist_metadata"] {
        let Some(Value::Object(map)) = payload.get(block) else {
            continue;
        };
        for key in ["fanart_path", "fanart_url"] {
            let Some(Value::String(candidate)) = map.get(key) else {
                continue;
            };
            let resolved = resolver(candidate);
            if !resolved.is_empty() && !urls.contains(&resolved) {
                urls.push(resolved);
            }
        }
    }
    urls
}
```

- [ ] **Step 4: Run** `cargo test -p grid-core fanart` — green.

- [ ] **Step 5: Write the failing romm tests.** Append a new module at the end of `crates/grid-core/src/romm/mod.rs` (beside `release_date_tests`):

```rust
#[cfg(test)]
mod summary_tests {
    use super::{GameSummary, RawGameSummary};

    fn parse(value: serde_json::Value) -> GameSummary {
        let raw: RawGameSummary = serde_json::from_value(value).expect("summary decodes");
        raw.into_summary("https://romm.example")
    }

    #[test]
    fn a_summary_carries_its_screenshots_and_fanart_resolved_and_filtered() {
        let summary = parse(serde_json::json!({
            "id": 101,
            "name": "Super Mario World",
            "platform_id": 1,
            "path_cover_small": "/assets/small.png",
            "path_cover_large": "/assets/large.png",
            "merged_screenshots": [
                "/assets/shots/1.png",
                "https://img.elsewhere/box-front.jpg"
            ],
            "ss_metadata": { "fanart_path": "/assets/art/fanart.jpg" }
        }));
        assert_eq!(summary.id, 101);
        assert_eq!(
            summary.screenshot_urls,
            vec!["https://romm.example/assets/shots/1.png".to_string()]
        );
        assert_eq!(
            summary.fanart_urls,
            vec!["https://romm.example/assets/art/fanart.jpg".to_string()]
        );
    }

    /// The pinned public contract from before this change: a null `name`
    /// still falls back to `fs_name_no_ext`, and a payload with none of the
    /// new fields still decodes.
    #[test]
    fn an_older_payload_still_decodes_with_empty_lists() {
        let summary = parse(serde_json::json!({
            "id": 102,
            "name": null,
            "fs_name_no_ext": "Chrono Trigger (USA)",
            "platform_id": 1,
            "path_cover_small": "/assets/small.png"
        }));
        assert_eq!(summary.name, "Chrono Trigger (USA)");
        assert!(summary.screenshot_urls.is_empty());
        assert!(summary.fanart_urls.is_empty());
        assert_eq!(summary.cover_large_path, None);
    }
}

#[cfg(test)]
mod detail_fanart_tests {
    use super::RawRomDetail;

    #[test]
    fn a_detail_carries_its_fanart() {
        let raw: RawRomDetail = serde_json::from_value(serde_json::json!({
            "id": 101,
            "fs_name_no_ext": "Super Mario World",
            "platform_id": 1,
            "gamelist_metadata": { "fanart_path": "/assets/art/fanart.jpg" }
        }))
        .expect("detail decodes");
        let detail = raw.into_detail("https://romm.example");
        assert_eq!(
            detail.fanart_urls,
            vec!["https://romm.example/assets/art/fanart.jpg".to_string()]
        );
    }
}
```

- [ ] **Step 6: Run** `cargo test -p grid-core summary_tests detail_fanart_tests` — fails to compile.

- [ ] **Step 7: Implement the summary changes** in `crates/grid-core/src/romm/mod.rs`. Add to `GameSummary` after `cover_large_path`:

```rust
    /// Already resolved + host-filtered absolute screenshot URLs, in source
    /// order — the same `screenshot_urls_from_payload` output `RomDetail`
    /// carries, read from the LIST payload so the Server grid's background
    /// art has screenshots without a per-card detail fetch.
    #[serde(default)]
    pub screenshot_urls: Vec<String>,
    /// Already resolved + host-filtered absolute fanart URLs
    /// (`fanart_urls_from_payload`). Usually empty: most servers have no
    /// fanart, which is why the background falls back to screenshots.
    #[serde(default)]
    pub fanart_urls: Vec<String>,
```

Add to `RawGameSummary`, after `cover_large_path`:

```rust
    /// Every field not named above — the screenshot and fanart sources
    /// (`merged_screenshots`, `ss_metadata`, `gamelist_metadata`, …) are read
    /// from here, exactly as `RawRomDetail` does it (`:556-559`).
    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
```

Replace the whole `impl From<RawGameSummary> for GameSummary` block with:

```rust
impl RawGameSummary {
    /// `base_url` is needed because a screenshot/fanart path is server
    /// relative and must be resolved and host-filtered before it leaves this
    /// crate — which is why this is a method rather than the `From` impl it
    /// replaces.
    fn into_summary(self, base_url: &str) -> GameSummary {
        let name = self
            .name
            .filter(|n| !n.is_empty())
            .or(self.fs_name_no_ext)
            .unwrap_or_default();
        let resolver = crate::images::urls::server_resolver(base_url);
        let extra = serde_json::Value::Object(self.extra);
        GameSummary {
            id: self.id,
            name,
            platform_id: self.platform_id,
            cover_path: self.cover_path,
            cover_large_path: self.cover_large_path,
            screenshot_urls: crate::images::urls::screenshot_urls_from_payload(&extra, &resolver),
            fanart_urls: crate::images::urls::fanart_urls_from_payload(&extra, &resolver),
        }
    }
}
```

and in `games` (`:282`) replace `.map(GameSummary::from)` with `.map(|raw| raw.into_summary(&self.base))`.

- [ ] **Step 8: Implement the detail change.** Add to `RomDetail` after `screenshot_urls` (`:421`):

```rust
    /// Already resolved + host-filtered absolute fanart URLs
    /// (`images::urls::fanart_urls_from_payload`). The shell's background art
    /// prefers these over screenshots (user ruling 2026-09-05).
    pub fanart_urls: Vec<String>,
```

In `into_detail`, the `extra` map is consumed by `screenshot_urls_from_payload` today (`:571-574`); bind it once and use it twice:

```rust
        let resolver = crate::images::urls::server_resolver(base_url);
        let extra = serde_json::Value::Object(self.extra);
        let screenshot_urls =
            crate::images::urls::screenshot_urls_from_payload(&extra, &resolver);
        let fanart_urls = crate::images::urls::fanart_urls_from_payload(&extra, &resolver);
```

and add `fanart_urls,` to the `RomDetail { … }` literal, right after `screenshot_urls,`.

- [ ] **Step 9: Run** `cargo test -p grid-core summary_tests detail_fanart_tests` — green.

- [ ] **Step 10: Write the failing registry test** in `crates/grid-core/tests/registry.rs`, modelled on `migrates_v3_to_v4_keeping_rows_and_defaulting_last_played_to_zero` (`:574`):

```rust
#[test]
fn migrates_v4_to_v5_keeping_rows_and_defaulting_fanart_to_blank() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("grid-launcher.db");
    {
        // A v4 database is exactly what `Registry::open` writes today, so
        // creating one and stepping the version back is the honest fixture.
        let registry = Registry::open(&path).unwrap();
        drop(registry);
        let conn = Connection::open(&path).unwrap();
        conn.execute(
            "INSERT INTO installed_games (title, platform, title_key, platform_key, rom_id, installed_at)
             VALUES ('Four', 'SNES', 'four', 'snes', 7, 1)",
            [],
        )
        .unwrap();
        conn.execute_batch("ALTER TABLE installed_games DROP COLUMN fanart_urls;")
            .unwrap();
        conn.pragma_update(None, "user_version", 4).unwrap();
    }

    let registry = Registry::open(&path).unwrap();
    let conn = Connection::open(&path).unwrap();
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, 5);

    let rows = registry.all().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].title, "Four");
    assert_eq!(rows[0].fanart_urls, "");
}

#[test]
fn v4_to_v5_migration_is_idempotent_when_the_column_preexists() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("grid-launcher.db");
    {
        let registry = Registry::open(&path).unwrap();
        drop(registry);
        // The column is already there (a database torn by an interrupted
        // migration); only the version is behind.
        let conn = Connection::open(&path).unwrap();
        conn.pragma_update(None, "user_version", 4).unwrap();
    }

    let registry = Registry::open(&path).unwrap();
    let conn = Connection::open(&path).unwrap();
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, 5);
    assert!(registry.all().unwrap().is_empty());
}
```

`ALTER TABLE … DROP COLUMN` needs SQLite 3.35+; `rusqlite`'s `bundled` feature (Cargo.toml) ships a much newer one, so this is safe. If it errors anyway, build the v4 table by hand the way `v3_schema()` (`:336`) already does for its own version and say so in the test's comment.

- [ ] **Step 11: Run** `cargo test -p grid-core --test registry` — the two new tests fail, and the ten `assert_eq!(version, 4)` assertions still pass (nothing has changed yet).

- [ ] **Step 12: Implement the registry column.** In `crates/grid-core/src/library/registry.rs`:
  1. `SCHEMA_SQL`: add `    fanart_urls         TEXT NOT NULL DEFAULT '',` directly after the `screenshot_urls` line.
  2. `const LATEST_USER_VERSION: i64 = 5;`
  3. after `V4_COLUMN` (`:84`):

```rust
/// The column v4 -> v5 (round 4's background art) adds: the row's fanart
/// URLs, newline-joined exactly like `screenshot_urls`. A TEXT with a blank
/// default, so an existing row simply has no fanart and the background falls
/// back to its screenshots.
const V5_COLUMN: &str = "fanart_urls";
```

  4. after `migrate_3_to_4` (`:168`):

```rust
/// v4 -> v5 (round 4): adds `fanart_urls`. Same transaction +
/// idempotent-`ADD COLUMN` shape as every migration above it, for the same
/// reasons — one commit for the schema change and the `user_version` bump,
/// and a column already present is skipped rather than erroring.
fn migrate_4_to_5(conn: &mut Connection) -> Result<(), LibraryError> {
    let tx = conn.transaction().map_err(registry_err)?;
    let existing = installed_games_columns(&tx)?;
    if !existing.iter().any(|name| name == V5_COLUMN) {
        tx.execute_batch(&format!(
            "ALTER TABLE installed_games ADD COLUMN {V5_COLUMN} TEXT NOT NULL DEFAULT '';"
        ))
        .map_err(registry_err)?;
    }
    tx.pragma_update(None, "user_version", 5)
        .map_err(registry_err)?;
    tx.commit().map_err(registry_err)
}
```

  5. add `4 => migrate_4_to_5(&mut conn)?,` to the migration match (after the `3 => …` arm).
  6. `SELECT_COLUMNS`: append `, fanart_urls` at the very END of the list (after `last_played_at`), so no existing `row.get(n)` index moves.
  7. `InstalledGame`: add after `last_played_at`:

```rust
    /// Newline-joined fanart URLs, already resolved + host-filtered, exactly
    /// like `screenshot_urls`. `""` for a row installed before v5 or for a
    /// game the server has no fanart for.
    #[serde(default)]
    pub fanart_urls: String,
```

  8. `from_row`: add `fanart_urls: row.get(36)?,` as the last field.
  9. `upsert`: add `fanart_urls` to the INSERT column list (after `screenshot_urls`), add `?38` to the VALUES list, add `fanart_urls = excluded.fanart_urls,` to the `DO UPDATE SET` block (after the `screenshot_urls` line), and add `rec.fanart_urls,` to `params![…]` in the matching position (after `rec.screenshot_urls,`). Named columns and `params!` are positional against each other — count both lists after editing.
  10. `update_images`: `"UPDATE installed_games SET cover_small_path = ?1, cover_large_path = ?2, screenshot_urls = ?3, fanart_urls = ?4 WHERE rom_id = ?5"` with `fields.fanart_urls` as the fourth param and `rom_id` as the fifth.

- [ ] **Step 13: Implement `ImageFields`.** In `crates/grid-core/src/images/mod.rs`, add to the struct after `screenshot_urls`:

```rust
    /// Fanart URLs, newline-joined — same convention as `screenshot_urls`.
    pub fanart_urls: String,
```

and to `from_detail`: `fanart_urls: detail.fanart_urls.join("\n"),`. Then fix the three struct literals this breaks: `crates/grid-core/src/library/mod.rs:1590-1594` (`fanart_urls: record.fanart_urls.clone(),`) and `crates/grid-core/tests/registry.rs:266` and `:326` (`fanart_urls: "https://h/fanart.jpg".into(),`).

- [ ] **Step 14: Update the version assertions.** In `crates/grid-core/tests/registry.rs`, change every `assert_eq!(version, 4);` to `assert_eq!(version, 5);` (ten sites: `:69, :83, :258, :306, :363, :399, :440, :558, :594, :621`), rename `open_creates_file_and_sets_user_version_4` → `open_creates_file_and_sets_user_version_5` and `fresh_db_is_v4_and_has_the_twelve_columns` → `fresh_db_is_v5_and_has_the_twelve_columns`, and in the `migrates_v1_to_v4_transactionally` / `migrates_v2_to_v4` / `migrates_v3_to_v4_*` doc comments and names leave the *source* version alone — they now migrate all the way to 5, so append `_to_v5` only where the name states the target. Run `grep -n "v4\|version_4" crates/grid-core/tests/registry.rs` afterwards and make sure no name still claims 4 is the latest.

- [ ] **Step 15: Update the E2E seed schema.** In `e2e/seed/registry-schema.mjs`: `export const USER_VERSION = 5;` and add `    fanart_urls         TEXT NOT NULL DEFAULT '',` to `SCHEMA_SQL` directly after the `screenshot_urls` line. That file's own doc comment says the pairing is load-bearing — `Registry::open` trusts a database already at `LATEST_USER_VERSION` — so the `firmware`, `native`, `content`, `ps3-install` and `updates` groups break silently without this. The three v1-seeding scripts (`images-seed.mjs`, `launch-seed.mjs`, `cloud-saves-seed.mjs`, `emulator-catalog-seed.mjs`) are deliberately left alone: they exercise the migration path.

- [ ] **Step 16: Run** `cargo test --workspace` — green. Then `cargo fmt` and both clippy commands.

- [ ] **Step 17: Mirror the shapes in TypeScript.** In `app/src/lib/api.ts`:
  - `GameSummary` gains `screenshot_urls: string[];` and `fanart_urls: string[];` (with the comment `/** Resolved + host-filtered; the Server grid's background art reads these. */`).
  - `RomDetail` gains `fanart_urls: string[];` after `screenshot_urls`.
  - `InstalledGame` gains `fanart_urls: string;` after `screenshot_urls` (`/** Newline-joined, like screenshot_urls; '' for a row installed before schema v5. */`).

- [ ] **Step 18: Carry fanart through the details subject.** In `app/src/lib/details/subject.ts`:
  - `DetailsSubject` gains `fanartUrls: string[];` after `screenshotUrls`.
  - `fromSummary` now has real data to carry: `screenshotUrls: game.screenshot_urls, fanartUrls: game.fanart_urls,` (replacing `screenshotUrls: []`).
  - `fromInstalled` gains `fanartUrls: splitStored(row.fanart_urls)` using the same split the existing `screenshot_urls` line uses; extract that expression into a local helper in this module so the two agree:

```ts
/** The registry stores these columns as newline-joined text; blanks are
 *  dropped defensively even though the backend already filters them. */
function splitStored(stored: string): string[] {
  return stored
    .split('\n')
    .map((url) => url.trim())
    .filter((url) => url.length > 0);
}
```

  - `summaryOf` gains `screenshot_urls: subject.screenshotUrls, fanart_urls: subject.fanartUrls,` so the shim still satisfies `GameSummary`.
  - `mergeDetail` gains `fanartUrls: detail.fanart_urls.length > 0 ? detail.fanart_urls : subject.fanartUrls,` — the same "empty means absent" rule the line above it uses for screenshots.

- [ ] **Step 19: Update `subject.test.ts`.** Every `GameSummary` / `InstalledGame` / `RomDetail` fixture in that file gains the new fields, and add:

```ts
  it('carries the summary\'s own screenshots and fanart', () => {
    const subject = fromSummary(
      summary({ screenshot_urls: ['https://romm/s1.png'], fanart_urls: ['https://romm/f1.jpg'] }),
      'SNES'
    );
    expect(subject.screenshotUrls).toEqual(['https://romm/s1.png']);
    expect(subject.fanartUrls).toEqual(['https://romm/f1.jpg']);
  });

  it('splits the registry row\'s newline-joined fanart column', () => {
    const subject = fromInstalled(row({ fanart_urls: 'https://romm/f1.jpg\n\n https://romm/f2.jpg ' }));
    expect(subject.fanartUrls).toEqual(['https://romm/f1.jpg', 'https://romm/f2.jpg']);
  });

  it('keeps the subject\'s fanart when the detail has none', () => {
    const merged = mergeDetail(
      { ...base, fanartUrls: ['https://romm/f1.jpg'] },
      detail({ fanart_urls: [] })
    );
    expect(merged.fanartUrls).toEqual(['https://romm/f1.jpg']);
  });
```

Use whatever fixture-builder names that file already defines (`summary`, `row`, `detail`, `base` above are placeholders for them — read the file first and reuse its own helpers; do not add new ones).

- [ ] **Step 20: Run** `npx vitest run` and, from `app/`, `npm run check` — green, no new warnings.

- [ ] **Step 21: Commit**

```bash
git add crates/grid-core/src/images/urls.rs crates/grid-core/src/images/mod.rs crates/grid-core/src/romm/mod.rs crates/grid-core/src/library/registry.rs crates/grid-core/src/library/mod.rs crates/grid-core/tests/registry.rs e2e/seed/registry-schema.mjs app/src/lib/api.ts app/src/lib/details/subject.ts app/src/lib/details/subject.test.ts
git commit -m "rewrite: carry fanart and grid screenshots through the summary, detail and registry"
```

---

### Task 7: Background selection — subject, priority, cycling inputs

**Files:**
- Modify: `app/src/lib/background.ts` (whole file)
- Modify: `app/src/lib/background.test.ts` (whole file)
- Modify: `app/src/lib/backgroundSlots.test.ts` (one new case)
- Modify: `app/src/lib/stores/lastViewed.svelte.ts` (whole file)
- Modify: `app/src/lib/lastViewedHover.ts` and `lastViewedHover.test.ts`
- Modify: `app/src/lib/Library.svelte:20-21` (imports), `:118-122` (`openDetails`), `:113-116` (near the focus-clamp effect), `:141` (`createHoverViewed`), `:272-273` (the card's hover props)
- Modify: `app/src/lib/Server.svelte:15-16` (imports), `:247-251` (`openDetails`), `:135-138` (near the focus-clamp effect), `:274`, `:480-481`
- Modify: `app/src/lib/Details.svelte` (one effect that reports the merged subject)
- Modify: `app/src/lib/BackgroundArt.svelte:16-18` (read the subject's first URL — cycling and the variant land in Task 9)

**Interfaces:**
- Produces, in `background.ts`:
  ```ts
  export const HOVER_DELAY_MS = 500;            // unchanged
  export const CROSS_FADE_MS = 360;             // unchanged
  export const PREFETCH_DELAY_MS = 150;
  export const BACKGROUND_CYCLE_MS = 5000;
  export type BackgroundSubject = { fanart: string[]; screenshots: string[]; cover: string | null };
  export const EMPTY_BACKGROUND: BackgroundSubject;
  export function backgroundUrls(subject: BackgroundSubject): string[];
  export function shouldCycle(urls: string[], fade: number): boolean;
  export function cycleIndex(current: number, count: number): number;
  export function subjectFromInstalled(row: InstalledGame): BackgroundSubject;
  export function subjectFromSummary(game: GameSummary): BackgroundSubject;
  export function subjectFromDetails(subject: DetailsSubject): BackgroundSubject;
  export function isEmptySubject(subject: BackgroundSubject): boolean;
  export function startupSubject(rows: InstalledGame[]): BackgroundSubject | null;
  ```
  `startupCover` is REPLACED by `startupSubject` (same rule — newest `installed_at` — but it now accepts a row that has screenshots or fanart even with no large cover).
- Produces, in `stores/lastViewed.svelte.ts`: `lastViewed.subject`, `lastViewed.urls`, `noteViewed(subject: BackgroundSubject): void`, `seedLastViewed(rows: InstalledGame[]): void`.
- Changes: `createHoverViewed(delayMs?, prefetchMs?)`'s `start` now takes a `BackgroundSubject`, not a cover string.
- Consumes: `InstalledGame`, `GameSummary` types from `./api`; `DetailsSubject` type from `./details/subject` (type-only import — `details/subject.ts` imports only from `../api`, so there is no cycle).

- [ ] **Step 1: Write the failing selection tests.** Replace `app/src/lib/background.test.ts`'s `startupCover` describe and add the new ones (keep the file's existing `row()` helper, extending it with `fanart_urls: ''`):

```ts
describe('backgroundUrls', () => {
  it('prefers fanart over everything else', () => {
    expect(
      backgroundUrls({
        fanart: ['https://romm/f1.jpg', 'https://romm/f2.jpg'],
        screenshots: ['https://romm/s1.png'],
        cover: 'https://romm/cover.png',
      })
    ).toEqual(['https://romm/f1.jpg', 'https://romm/f2.jpg']);
  });

  it('falls back to the screenshots when there is no fanart', () => {
    expect(
      backgroundUrls({ fanart: [], screenshots: ['https://romm/s1.png'], cover: 'https://romm/c.png' })
    ).toEqual(['https://romm/s1.png']);
  });

  it('uses the cover only as a last resort', () => {
    expect(backgroundUrls({ fanart: [], screenshots: [], cover: 'https://romm/c.png' })).toEqual([
      'https://romm/c.png',
    ]);
  });

  it('is empty when the game has no art at all', () => {
    expect(backgroundUrls({ fanart: [], screenshots: [], cover: null })).toEqual([]);
    expect(backgroundUrls({ fanart: ['  '], screenshots: [''], cover: '   ' })).toEqual([]);
  });

  it('trims and de-duplicates within the chosen tier', () => {
    expect(
      backgroundUrls({ fanart: [], screenshots: [' https://romm/s1.png ', 'https://romm/s1.png'], cover: null })
    ).toEqual(['https://romm/s1.png']);
  });
});

describe('shouldCycle', () => {
  it('cycles only with more than one image', () => {
    expect(shouldCycle(['a', 'b'], 25)).toBe(true);
    expect(shouldCycle(['a'], 25)).toBe(false);
    expect(shouldCycle([], 25)).toBe(false);
  });

  // User ruling 2026-09-05: fade 0 means the art is invisible, so a timer
  // swapping invisible images is pure cost.
  it('does not cycle while the fade slider is at 0', () => {
    expect(shouldCycle(['a', 'b'], 0)).toBe(false);
  });
});

describe('cycleIndex', () => {
  it('advances and wraps', () => {
    expect(cycleIndex(0, 3)).toBe(1);
    expect(cycleIndex(2, 3)).toBe(0);
  });

  it('is 0 for an empty list, never NaN', () => {
    expect(cycleIndex(4, 0)).toBe(0);
  });

  it('recovers from an index past the end (the list shrank mid-cycle)', () => {
    expect(cycleIndex(9, 2)).toBe(0);
  });
});

describe('startupSubject', () => {
  it('is null when nothing is installed', () => {
    expect(startupSubject([])).toBeNull();
  });

  it('picks the newest row that has any art', () => {
    const subject = startupSubject([
      row({ installed_at: 100, cover_large_path: 'https://romm/old.png' }),
      row({ installed_at: 300, cover_large_path: 'https://romm/newest.png' }),
      row({ installed_at: 200, cover_large_path: '' }),
    ]);
    expect(subject).toEqual({ fanart: [], screenshots: [], cover: 'https://romm/newest.png' });
  });

  it('accepts a row with screenshots but no cover', () => {
    expect(
      startupSubject([row({ installed_at: 1, cover_large_path: '', screenshot_urls: 'https://romm/s1.png' })])
    ).toEqual({ fanart: [], screenshots: ['https://romm/s1.png'], cover: null });
  });

  it('skips rows with no art at all', () => {
    expect(startupSubject([row({ installed_at: 9, cover_large_path: '' })])).toBeNull();
  });
});

describe('subjectFromInstalled / subjectFromSummary', () => {
  it('splits the registry row\'s newline-joined columns', () => {
    expect(
      subjectFromInstalled(
        row({
          fanart_urls: 'https://romm/f1.jpg',
          screenshot_urls: 'https://romm/s1.png\nhttps://romm/s2.png',
          cover_large_path: 'https://romm/c.png',
        })
      )
    ).toEqual({
      fanart: ['https://romm/f1.jpg'],
      screenshots: ['https://romm/s1.png', 'https://romm/s2.png'],
      cover: 'https://romm/c.png',
    });
  });

  it('reads the server summary\'s own arrays', () => {
    expect(
      subjectFromSummary({
        id: 1,
        name: 'x',
        platform_id: 1,
        path_cover_small: 'https://romm/s.png',
        path_cover_large: 'https://romm/l.png',
        screenshot_urls: ['https://romm/s1.png'],
        fanart_urls: [],
      })
    ).toEqual({ fanart: [], screenshots: ['https://romm/s1.png'], cover: 'https://romm/l.png' });
  });
});
```

- [ ] **Step 2: Run** `npx vitest run background` — fails (nothing exported).

- [ ] **Step 3: Implement** `app/src/lib/background.ts`. Keep `HOVER_DELAY_MS` and `CROSS_FADE_MS` exactly as they are (docs included), delete `startupCover`, and add:

```ts
import type { GameSummary, InstalledGame } from './api';
import type { DetailsSubject } from './details/subject';

/**
 * How long a card must be dwelled on before its art is fetched — 150ms,
 * well under `HOVER_DELAY_MS`. The swap still happens at 500ms; this only
 * starts the (potentially slow: network + decode + blur) variant build early,
 * so the image is usually ready by the time the swap is allowed.
 */
export const PREFETCH_DELAY_MS = 150;

/**
 * How long each background image is held before the next one
 * (`FanartBackground`'s own 5000ms timer,
 * `grid_launcher/tv/widgets/components/fanart_background.py:52-53`).
 */
export const BACKGROUND_CYCLE_MS = 5000;

/**
 * Everything the background art may show for ONE game, in priority order.
 * User ruling 2026-09-05: fanart wins, then the game's own screenshots
 * (cycling), and the cover only as a last resort — a portrait cover stretched
 * across a landscape window is the worst of the three, not the first choice.
 */
export type BackgroundSubject = {
  /** Resolved + host-filtered fanart URLs; usually empty. */
  fanart: string[];
  /** Resolved + host-filtered screenshot URLs, in source order. */
  screenshots: string[];
  /** The large cover, or `null`. */
  cover: string | null;
};

export const EMPTY_BACKGROUND: BackgroundSubject = { fanart: [], screenshots: [], cover: null };

function clean(urls: readonly (string | null | undefined)[]): string[] {
  const out: string[] = [];
  for (const url of urls) {
    if (typeof url !== 'string') continue;
    const trimmed = url.trim();
    if (trimmed === '' || out.includes(trimmed)) continue;
    out.push(trimmed);
  }
  return out;
}

/** The URLs to show for `subject`, in order: the FIRST non-empty tier wins. */
export function backgroundUrls(subject: BackgroundSubject): string[] {
  const fanart = clean(subject.fanart);
  if (fanart.length > 0) return fanart;
  const screenshots = clean(subject.screenshots);
  if (screenshots.length > 0) return screenshots;
  return clean([subject.cover]);
}

export function isEmptySubject(subject: BackgroundSubject): boolean {
  return backgroundUrls(subject).length === 0;
}

/** Cycle only with something to cycle to, and only while the art is visible. */
export function shouldCycle(urls: string[], fade: number): boolean {
  return urls.length > 1 && fade > 0;
}

/** The next index, wrapping. `0` for an empty list — never `NaN`. */
export function cycleIndex(current: number, count: number): number {
  if (count <= 0) return 0;
  return (current + 1) % count;
}

/** The registry stores these columns as newline-joined text. */
function splitStored(stored: string | null | undefined): string[] {
  return clean((stored ?? '').split('\n'));
}

export function subjectFromInstalled(row: InstalledGame): BackgroundSubject {
  return {
    fanart: splitStored(row.fanart_urls),
    screenshots: splitStored(row.screenshot_urls),
    cover: row.cover_large_path.trim() === '' ? null : row.cover_large_path.trim(),
  };
}

export function subjectFromSummary(game: GameSummary): BackgroundSubject {
  return {
    fanart: clean(game.fanart_urls),
    screenshots: clean(game.screenshot_urls),
    cover: clean([game.path_cover_large])[0] ?? null,
  };
}

/** The merged detail the popup is showing — the richest subject there is. */
export function subjectFromDetails(subject: DetailsSubject): BackgroundSubject {
  return {
    fanart: clean(subject.fanartUrls),
    screenshots: clean(subject.screenshotUrls),
    cover: clean([subject.coverLarge])[0] ?? null,
  };
}

/**
 * The subject the shell starts with, before the user has viewed anything.
 *
 * The design asks for "the most recently played installed game". The registry
 * records no play timestamp, so the newest `installed_at` stands in for it:
 * the game a user just added is the one they are about to play. Revisit this
 * when a play-time column exists.
 *
 * Rows with no art at all are skipped rather than returned blank: the caller
 * would otherwise render an empty layer over a perfectly good candidate
 * further down the list.
 */
export function startupSubject(rows: InstalledGame[]): BackgroundSubject | null {
  let best: InstalledGame | null = null;
  for (const row of rows) {
    if (isEmptySubject(subjectFromInstalled(row))) continue;
    if (best === null || row.installed_at > best.installed_at) best = row;
  }
  return best === null ? null : subjectFromInstalled(best);
}
```

- [ ] **Step 4: Run** `npx vitest run background` — green.

- [ ] **Step 5: Add the slot regression case** to `app/src/lib/backgroundSlots.test.ts` (the module is unchanged; this pins that cycling — repeated `withNextCover` calls for the SAME subject — keeps alternating rather than writing into the visible slot):

```ts
  it('keeps alternating across a 5s cycle through three images', () => {
    let state = initialSlotState;
    state = withNextCover(state, 'one');
    expect(state.top).toBe('b');
    state = clearIfBottom(state, outgoingSlot(state));
    state = withNextCover(state, 'two');
    expect(state.top).toBe('a');
    expect(state.a).toBe('two');
    // The outgoing image is still on screen for the fade.
    expect(state.b).toBe('one');
    state = withNextCover(state, 'three');
    expect(state.top).toBe('b');
    expect(state.b).toBe('three');
  });
```

- [ ] **Step 6: Implement the store.** Replace `app/src/lib/stores/lastViewed.svelte.ts` with:

```ts
// What the background art is showing. Module scoped so it survives a Shell
// remount, like `appUpdate.svelte.ts`.
import type { InstalledGame } from '../api';
import {
  backgroundUrls,
  EMPTY_BACKGROUND,
  isEmptySubject,
  startupSubject,
  type BackgroundSubject,
} from '../background';

const state = $state<{ subject: BackgroundSubject; seeded: boolean }>({
  subject: EMPTY_BACKGROUND,
  seeded: false,
});

export const lastViewed = {
  get subject(): BackgroundSubject {
    return state.subject;
  },
  /** The chosen tier's URLs, in cycle order. `[]` when there is no art. */
  get urls(): string[] {
    return backgroundUrls(state.subject);
  },
};

/** A details popup opened, a card was focused, or a card was hovered past the
 *  dwell. A subject with no art at all is ignored: keeping the previous art
 *  beats a blank frame. */
export function noteViewed(subject: BackgroundSubject): void {
  if (isEmptySubject(subject)) return;
  state.subject = subject;
  state.seeded = true;
}

/** The startup fallback. Runs once, and never overwrites a real view. */
export function seedLastViewed(rows: InstalledGame[]): void {
  if (state.seeded) return;
  const subject = startupSubject(rows);
  if (subject === null) return;
  state.subject = subject;
  state.seeded = true;
}
```

- [ ] **Step 7: Implement the dwell timer.** Replace the body of `app/src/lib/lastViewedHover.ts` with a two-timer version (the 150 ms prefetch call is wired in Task 9 — this step only changes the argument type, so the file compiles against the new store):

```ts
// Shared dwell-timer factory for feeding `lastViewed` from a card grid's
// hover events and from keyboard/gamepad focus (design §3). Library.svelte
// and Server.svelte each mount one per input rather than duplicating the
// timer bookkeeping.
import { HOVER_DELAY_MS } from './background';
import type { BackgroundSubject } from './background';
import { noteViewed } from './stores/lastViewed.svelte';

/**
 * Design §3: a card becomes the background only after the pointer (or the
 * selection) has rested on it for more than `delayMs` (500ms by default).
 * Shorter dwells are pointer travel, or a held arrow key, not interest.
 */
export function createHoverViewed(delayMs: number = HOVER_DELAY_MS): {
  start: (subject: BackgroundSubject) => void;
  end: () => void;
} {
  let timer: ReturnType<typeof setTimeout> | null = null;

  function start(subject: BackgroundSubject): void {
    if (timer !== null) clearTimeout(timer);
    timer = setTimeout(() => {
      timer = null;
      noteViewed(subject);
    }, delayMs);
  }

  function end(): void {
    if (timer === null) return;
    clearTimeout(timer);
    timer = null;
  }

  return { start, end };
}
```

Update `lastViewedHover.test.ts`: every `hover.start('https://romm/cover.png')` becomes `hover.start(subject('https://romm/cover.png'))` with a local helper

```ts
const subject = (cover: string) => ({ fanart: [], screenshots: [], cover });
```

and every `expect(noteViewed).toHaveBeenCalledExactlyOnceWith('https://romm/cover.png')` becomes `…With(subject('https://romm/cover.png'))`.

- [ ] **Step 8: Wire the Library view.** In `app/src/lib/Library.svelte`, import `subjectFromInstalled` from `./background`. `openDetails` (`:118-122`) becomes:

```svelte
  function openDetails(row: InstalledGame, mode: CloudMode = 'overview') {
    detailsCloudMode = mode;
    subject = fromInstalled(row);
    noteViewed(subjectFromInstalled(row));
  }
```

Add a second dwell timer next to `const hover = createHoverViewed();` (`:141`):

```svelte
  // Keyboard/gamepad selection feeds the background through the SAME 500ms
  // dwell as the pointer, so holding an arrow key across the grid does not
  // start a fetch per card. A separate timer from `hover`: sharing one would
  // let a mouse move cancel a keyboard selection's pending swap.
  const focusDwell = createHoverViewed();

  $effect(() => {
    const row = rows[focusIndex];
    if (!active || row === undefined) return;
    focusDwell.start(subjectFromInstalled(row));
    return () => focusDwell.end();
  });
```

and the card's hover prop (`:272`) becomes `onHoverStart={() => hover.start(subjectFromInstalled(row))}`.

- [ ] **Step 9: Wire the Server view.** In `app/src/lib/Server.svelte`, import `subjectFromSummary` from `./background`. `openDetails` (`:247-251`) becomes:

```svelte
  function openDetails(game: GameSummary, mode: CloudMode = 'overview') {
    detailsCloudMode = mode;
    detailsGame = game;
    noteViewed(subjectFromSummary(game));
  }
```

Add the same focus effect next to `const hover = createHoverViewed();` (`:274`), reading `visible[focusIndex]` and `subjectFromSummary`, and change the card prop (`:480`) to `onHoverStart={() => hover.start(subjectFromSummary(game))}`.

- [ ] **Step 10: Report the merged detail.** In `app/src/lib/Details.svelte`, import `subjectFromDetails` from `./background` and `noteViewed` from `./stores/lastViewed.svelte`, and add after the `merged` derived (`:107`):

```svelte
  // The grid already reported what IT knew when the popup opened (a summary
  // has a cover and, since round 4, the list payload's screenshots); once the
  // detail lands, the merged subject is strictly richer — it is the only
  // place fanart is ever known. Reporting again is idempotent when nothing
  // changed, because `noteViewed` just replaces the subject.
  $effect(() => {
    noteViewed(subjectFromDetails(merged));
  });
```

- [ ] **Step 11: Keep `BackgroundArt.svelte` compiling.** Replace the two lines at `:16-18`:

```svelte
  $effect(() => {
    const url = lastViewed.urls[0];
    if (url === undefined) return;
```

(the rest of that effect, and the CSS, are Task 9's.)

- [ ] **Step 12: Run** `npx vitest run` and, from `app/`, `npm run check` — green, no new warnings.

- [ ] **Step 13: Commit**

```bash
git add app/src/lib/background.ts app/src/lib/background.test.ts app/src/lib/backgroundSlots.test.ts app/src/lib/stores/lastViewed.svelte.ts app/src/lib/lastViewedHover.ts app/src/lib/lastViewedHover.test.ts app/src/lib/Library.svelte app/src/lib/Server.svelte app/src/lib/Details.svelte app/src/lib/BackgroundArt.svelte
git commit -m "rewrite: choose background art by fanart, then screenshots, then cover"
```

---

### Task 8: `ensure_background_variant` — a pre-blurred 960px JPEG

**Files:**
- Create: `crates/grid-core/src/images/background.rs`
- Modify: `crates/grid-core/src/images/mod.rs:5-9` (module list)
- Modify: `crates/grid-core/src/images/cache.rs:17-32` (one new `ImageError` variant)
- Modify: `crates/grid-core/src/images/sweep.rs:65-71` (pin by key prefix) and its test module
- Modify: `crates/grid-core/src/images/replenish.rs:12-49` (a new item kind) and `:51-95` (`run`)
- Modify: `crates/grid-core/Cargo.toml` (the `image` dependency)
- Modify: `app/src-tauri/src/images.rs:92-105` (`spawn_prefetch`)
- Modify: `app/src-tauri/src/commands.rs:244-259` (a new command beside `ensure_video`)
- Modify: `app/src-tauri/src/lib.rs:289-290` (the invoke handler list)
- Modify: `app/src/lib/api.ts:356-357` (the wrapper)

**Interfaces:**
- Produces, in `crates/grid-core/src/images/background.rs`:
  ```rust
  pub const BACKGROUND_VARIANT_EXT: &str = "bg.jpg";
  pub const BACKGROUND_WIDTH: u32 = 960;
  pub const BACKGROUND_BLUR_SIGMA: f32 = 12.0;
  pub const BACKGROUND_JPEG_QUALITY: u8 = 80;

  pub fn build_background_variant(source: &Path, dir: &Path, key: &str) -> Result<PathBuf, ImageError>;
  pub async fn ensure_background_variant(cache: &ImageCache, client: Option<&RommClient>, url: &str) -> Result<PathBuf, ImageError>;
  ```
- Produces: `ImageError::Decode`.
- Produces: `ReplenishItem::NeedsVariant { rom_id: i64, url: String }`.
- Produces: `#[tauri::command] pub async fn ensure_background_variant(state: State<'_, AppState>, url: String) -> Result<String, String>` and `api.ensureBackgroundVariant(url: string): Promise<string>`.
- Consumes: `ImageCache::{ensure, find_with_extension, dir}` (`cache.rs:76-85, 101-144`) and `image_key` (`:35-40`) — the same pattern `images/video.rs:67-92` uses.

Why: the shell paints two `inset: -60px` layers with `filter: blur(40px)` over a `background-size: cover` full-resolution cover (up to 850×1122). That is ~2.4 Mpx of blur per layer per frame for the whole 360 ms fade, redone whenever the window resizes. Blurring once, at 960 px, in Rust, turns that into compositing a ~0.3 Mpx JPEG. Python's TV background did exactly this — `_blur_pixmap` ran ONCE on arrival (`fanart_background.py`, doc 07 "Fanart background (TV)").

- [ ] **Step 1: Add the dependency.** From `rewrite/`, run:

```bash
cargo add image@0.25 --no-default-features --features jpeg,png,webp,gif,bmp -p grid-core
```

This needs the network. If it cannot fetch, stop and report NEEDS_CONTEXT — do not hand-write a version into `Cargo.toml`. Confirm afterwards that `crates/grid-core/Cargo.toml` gained a line of the shape `image = { version = "0.25.x", default-features = false, features = ["jpeg", "png", "webp", "gif", "bmp"] }` and that `Cargo.lock` changed. The five decoders are exactly the formats `LOOKUP_EXTENSIONS` (`urls.rs:47-52`) can produce that the `image` crate supports; anything else (svg, avif, heic, ico, tiff) fails to decode and is reported as `ImageError::Decode`, which the frontend treats as "keep the current art".

- [ ] **Step 2: Write the failing variant tests.** Create the test module at the end of the new `crates/grid-core/src/images/background.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::images::cache::image_key;

    /// A 1200x800 PNG, wider than `BACKGROUND_WIDTH`, so the resize branch
    /// actually runs.
    fn write_source(dir: &Path, key: &str) -> PathBuf {
        let mut buf = image::RgbImage::new(1200, 800);
        for (x, y, pixel) in buf.enumerate_pixels_mut() {
            *pixel = image::Rgb([(x % 256) as u8, (y % 256) as u8, 128]);
        }
        let path = dir.join(format!("{key}.png"));
        buf.save(&path).expect("the fixture PNG writes");
        path
    }

    #[test]
    fn the_variant_is_written_beside_the_source_and_is_960_wide() {
        let dir = tempfile::tempdir().unwrap();
        let key = image_key("https://romm.example/cover.png");
        let source = write_source(dir.path(), &key);

        let out = build_background_variant(&source, dir.path(), &key).unwrap();

        assert_eq!(out, dir.path().join(format!("{key}.{BACKGROUND_VARIANT_EXT}")));
        assert!(out.is_file());
        let decoded = image::open(&out).unwrap();
        assert_eq!(decoded.width(), BACKGROUND_WIDTH);
        // 1200x800 scaled to 960 wide keeps its 3:2 ratio.
        assert_eq!(decoded.height(), 640);
        // No `.part` file is left behind.
        assert!(!dir.path().join(format!("{key}.bg.part")).exists());
    }

    #[test]
    fn a_source_narrower_than_the_target_is_not_upscaled() {
        let dir = tempfile::tempdir().unwrap();
        let key = image_key("https://romm.example/small.png");
        let mut buf = image::RgbImage::new(320, 240);
        for pixel in buf.pixels_mut() {
            *pixel = image::Rgb([10, 20, 30]);
        }
        let source = dir.path().join(format!("{key}.png"));
        buf.save(&source).unwrap();

        let out = build_background_variant(&source, dir.path(), &key).unwrap();
        let decoded = image::open(&out).unwrap();
        assert_eq!(decoded.width(), 320);
        assert_eq!(decoded.height(), 240);
    }

    #[test]
    fn a_body_that_is_not_an_image_reports_decode_rather_than_panicking() {
        let dir = tempfile::tempdir().unwrap();
        let key = image_key("https://romm.example/broken.png");
        let source = dir.path().join(format!("{key}.png"));
        std::fs::write(&source, b"<html>not an image</html>").unwrap();
        assert!(matches!(
            build_background_variant(&source, dir.path(), &key),
            Err(ImageError::Decode)
        ));
    }

    #[tokio::test]
    async fn a_second_call_is_a_cache_hit_and_does_not_rebuild() {
        let dir = tempfile::tempdir().unwrap();
        let cache = ImageCache::new(dir.path().to_path_buf());
        let url = "https://romm.example/cover.png";
        let key = image_key(url);
        write_source(dir.path(), &key);

        // No client: `ImageCache::ensure` finds the source already cached, so
        // the whole path runs offline.
        let first = ensure_background_variant(&cache, None, url).await.unwrap();
        let stamp = std::fs::metadata(&first).unwrap().modified().unwrap();
        let second = ensure_background_variant(&cache, None, url).await.unwrap();
        assert_eq!(first, second);
        assert_eq!(std::fs::metadata(&second).unwrap().modified().unwrap(), stamp);
    }
}
```

`find_with_extension` refreshes the file's mtime on a hit, so the last assertion compares the CONTENT's modified stamp taken immediately after the first build against the one after the second call — if that proves flaky on a filesystem with coarse timestamps, assert on the file's length instead and say why in the comment.

- [ ] **Step 3: Run** `cargo test -p grid-core images::background` — fails (no module).

- [ ] **Step 4: Implement** `crates/grid-core/src/images/background.rs`:

```rust
//! The shell's background art variant: one pre-scaled, pre-blurred JPEG per
//! source image, built once and cached beside it.
//!
//! `BackgroundArt.svelte` used to hand the raw large cover (up to 850x1122)
//! to two `filter: blur(40px)` layers, so the compositor blurred ~2.4 Mpx per
//! layer per frame for the whole 360ms cross-fade. Python's TV background
//! blurred ONCE on arrival instead (`_blur_pixmap`,
//! `grid_launcher/tv/widgets/components/fanart_background.py`); this is that,
//! moved into Rust so the webview only ever composites a ~0.3 Mpx still.
//!
//! Shares the image cache's directory and key scheme, like `video.rs`: the
//! variant for `<key>` is `<key>.bg.jpg`, which keeps it with its source for
//! the sweep (`sweep::pinned_keys` pins by key PREFIX for exactly this).
//!
//! Token secrecy: the source bytes come from `ImageCache::ensure` (the
//! session's `RommClient`); nothing here builds a URL or logs one.

use super::cache::{image_key, ImageCache, ImageError};
use crate::romm::RommClient;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::{fast_blur, FilterType};
use std::path::{Path, PathBuf};

/// The variant's extension. Not in `LOOKUP_EXTENSIONS`, so `find_existing`
/// never mistakes a variant for its own source.
pub const BACKGROUND_VARIANT_EXT: &str = "bg.jpg";
/// Wide enough for a 1080p window's background, small enough to blur once and
/// composite for free.
pub const BACKGROUND_WIDTH: u32 = 960;
/// The blur radius, chosen so a 960px-wide variant looks like the 40px CSS
/// blur did on a full-resolution cover.
pub const BACKGROUND_BLUR_SIGMA: f32 = 12.0;
pub const BACKGROUND_JPEG_QUALITY: u8 = 80;

/// Decodes `source`, scales it to at most [`BACKGROUND_WIDTH`], blurs it and
/// writes `<key>.bg.jpg` into `dir` through a `.part` + rename, so a killed
/// process never leaves a half-written JPEG that a later run would serve.
///
/// Blocking: the caller runs it on `spawn_blocking`.
pub fn build_background_variant(
    source: &Path,
    dir: &Path,
    key: &str,
) -> Result<PathBuf, ImageError> {
    let io = |e: std::io::Error| ImageError::Io(e.to_string());
    let bytes = std::fs::read(source).map_err(io)?;
    let decoded = image::load_from_memory(&bytes).map_err(|_| ImageError::Decode)?;

    // Never upscale: a small source blurred and blown up is worse than the
    // small source blurred.
    let scaled = if decoded.width() > BACKGROUND_WIDTH {
        let height = ((decoded.height() as u64 * BACKGROUND_WIDTH as u64)
            / decoded.width().max(1) as u64)
            .max(1) as u32;
        decoded.resize_exact(BACKGROUND_WIDTH, height, FilterType::Triangle)
    } else {
        decoded
    };

    // RGB8: the background is opaque behind the whole shell, and JPEG has no
    // alpha channel anyway.
    let blurred = fast_blur(&scaled.to_rgb8(), BACKGROUND_BLUR_SIGMA);

    let mut encoded: Vec<u8> = Vec::new();
    JpegEncoder::new_with_quality(&mut encoded, BACKGROUND_JPEG_QUALITY)
        .encode_image(&blurred)
        .map_err(|_| ImageError::Decode)?;

    std::fs::create_dir_all(dir).map_err(io)?;
    let target = dir.join(format!("{key}.{BACKGROUND_VARIANT_EXT}"));
    // `.bg.part`, NOT `.part`: `ImageCache::fetch_and_store` uses `<key>.part`
    // for the source, and a concurrent fetch of the same image would
    // otherwise rename our half-written JPEG over the source.
    let tmp = dir.join(format!("{key}.bg.part"));
    std::fs::write(&tmp, &encoded).map_err(io)?;
    std::fs::rename(&tmp, &target).map_err(io)?;
    Ok(target)
}

/// The local path of `url`'s background variant, building it (and fetching
/// the source through `ImageCache::ensure`, with its dedup, negative cache and
/// download semaphore) on a miss.
pub async fn ensure_background_variant(
    cache: &ImageCache,
    client: Option<&RommClient>,
    url: &str,
) -> Result<PathBuf, ImageError> {
    let key = image_key(url);
    if let Some(path) = cache.find_with_extension(&key, BACKGROUND_VARIANT_EXT) {
        return Ok(path);
    }
    let source = cache.ensure(client, url).await?;
    let dir = cache.dir().to_path_buf();
    tokio::task::spawn_blocking(move || build_background_variant(&source, &dir, &key))
        .await
        .map_err(|e| ImageError::Io(format!("background variant did not finish: {e}")))?
}
```

Add `pub mod background;` to `crates/grid-core/src/images/mod.rs`'s module list (alphabetically first, before `pub mod cache;`).

- [ ] **Step 5: Add the error variant** in `crates/grid-core/src/images/cache.rs`, after `NotAVideo`:

```rust
    /// The cached bytes are not a format this build can decode, or the
    /// re-encode failed. Distinct from [`ImageError::NotAnImage`]: the server
    /// DID return an image (an SVG, an AVIF), it just cannot be turned into a
    /// background variant.
    #[error("the image could not be decoded")]
    Decode,
```

- [ ] **Step 6: Fix the sweep so a variant is pinned with its source.** In `crates/grid-core/src/images/sweep.rs`, replace `:65-71` with:

```rust
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        // `<key>.bg.jpg`'s file stem is `<key>.bg`, so a whole-stem compare
        // would leave every background variant unpinned and let the sweep
        // evict an installed game's art while keeping its source. Keys are
        // hex SHA-256 and never contain a dot, so the prefix is the key.
        let key = stem.split('.').next().unwrap_or(stem);
        entries.push(Entry {
            pinned: pinned.contains(key),
            path,
            size: meta.len(),
            mtime,
        });
```

and add to that file's test module:

```rust
    #[test]
    fn a_background_variant_is_pinned_with_its_source() {
        let dir = tempfile::tempdir().unwrap();
        let key = crate::images::cache::image_key("https://romm.example/cover.png");
        std::fs::write(dir.path().join(format!("{key}.png")), vec![0u8; 4096]).unwrap();
        std::fs::write(dir.path().join(format!("{key}.bg.jpg")), vec![0u8; 4096]).unwrap();
        let victim = crate::images::cache::image_key("https://romm.example/other.png");
        std::fs::write(dir.path().join(format!("{victim}.png")), vec![0u8; 8192]).unwrap();

        let mut pinned = std::collections::HashSet::new();
        pinned.insert(key.clone());
        // A cap below the total forces the sweep to delete something.
        let report = sweep(dir.path(), 4096, &pinned);

        assert!(dir.path().join(format!("{key}.png")).exists());
        assert!(dir.path().join(format!("{key}.bg.jpg")).exists());
        assert!(!dir.path().join(format!("{victim}.png")).exists());
        assert_eq!(report.deleted, 1);
    }
```

- [ ] **Step 7: Plan the variant in replenish.** In `crates/grid-core/src/images/replenish.rs`, add the item kind:

```rust
    /// The row's background source (its first screenshot, else its large
    /// cover) has no `<key>.bg.jpg` yet. Planned LAST, after every
    /// `NeedsFields`/`NeedsFile` item, so building variants never delays the
    /// grid covers the user is actually looking at.
    NeedsVariant { rom_id: i64, url: String },
```

and, in `plan`, collect variants in a second vector appended at the end:

```rust
/// The URL the background art would show for `row`: its first screenshot
/// (already resolved + host-filtered when it was stored), else its large
/// cover. Mirrors `backgroundUrls`' priority on the frontend, minus fanart —
/// a fanart URL is stored resolved too, so the same rule applies; see the
/// caller.
fn background_source_url(row: &InstalledGame, base_url: &str) -> String {
    for stored in [&row.fanart_urls, &row.screenshot_urls] {
        if let Some(first) = stored.lines().map(str::trim).find(|u| !u.is_empty()) {
            return filter_to_server_host(&resolve_image_url(first, base_url), base_url);
        }
    }
    filter_to_server_host(
        &resolve_image_url(&row.cover_large_path, base_url),
        base_url,
    )
}

pub fn plan(rows: &[InstalledGame], cache: &ImageCache, base_url: &str) -> Vec<ReplenishItem> {
    let mut items = Vec::new();
    let mut variants = Vec::new();
    for row in rows {
        let Some(rom_id) = row.rom_id else { continue };
        if row.cover_small_path.is_empty()
            && row.cover_large_path.is_empty()
            && row.screenshot_urls.is_empty()
        {
            items.push(ReplenishItem::NeedsFields { rom_id });
            continue;
        }
        let url = small_cover_url(row, base_url);
        if !url.is_empty() && cache.find_existing(&image_key(&url)).is_none() {
            items.push(ReplenishItem::NeedsFile { rom_id, url });
        }
        let background = background_source_url(row, base_url);
        if !background.is_empty()
            && cache
                .find_with_extension(&image_key(&background), BACKGROUND_VARIANT_EXT)
                .is_none()
        {
            variants.push(ReplenishItem::NeedsVariant {
                rom_id,
                url: background,
            });
        }
    }
    items.extend(variants);
    items
}
```

and handle it in `run`'s match:

```rust
            ReplenishItem::NeedsVariant { url, .. } => {
                match ensure_background_variant(cache, Some(client), &url).await {
                    Ok(_) => report.fetched_files += 1,
                    Err(_) => report.skipped += 1,
                }
            }
```

with `use super::background::{ensure_background_variant, BACKGROUND_VARIANT_EXT};` at the top. Extend that module's existing `plan` tests with one case proving a row whose variant already exists plans no `NeedsVariant`, and one proving the variant items come after every other item.

- [ ] **Step 8: Prefetch the variant at install time.** In `app/src-tauri/src/images.rs`, replace `spawn_prefetch`'s loop body (`:98-103`) with:

```rust
            let mut background_source = String::new();
            for path in [&fields.cover_small_path, &fields.cover_large_path] {
                let url = filter_to_server_host(&resolve_image_url(path, &base), &base);
                if !url.is_empty() {
                    let _ = session.cache().ensure(Some(&client), &url).await;
                    background_source = url;
                }
            }
            // The background art's own source, preferred in the same order
            // `backgroundUrls` uses on the frontend: fanart, then the first
            // screenshot, then the large cover (already fetched above).
            for stored in [&fields.fanart_urls, &fields.screenshot_urls] {
                if let Some(first) = stored.lines().map(str::trim).find(|u| !u.is_empty()) {
                    let url = filter_to_server_host(&resolve_image_url(first, &base), &base);
                    if !url.is_empty() {
                        background_source = url;
                        break;
                    }
                }
            }
            if !background_source.is_empty() {
                // Built here so the first time this game becomes the
                // background there is nothing to wait for.
                let _ = grid_core::images::background::ensure_background_variant(
                    session.cache(),
                    Some(&client),
                    &background_source,
                )
                .await;
            }
```

(The `for` loop above sets `background_source` to the large cover as its fallback because that is the last URL it visits.)

- [ ] **Step 9: Add the command.** In `app/src-tauri/src/commands.rs`, after `ensure_video` (`:259`):

```rust
/// The local path of the shell background's pre-scaled, pre-blurred variant
/// of `url`, building it on a miss. Mirrors [`ensure_image`]'s resolution and
/// host filter exactly, so a URL pointing anywhere but the configured server
/// is refused rather than fetched.
#[tauri::command]
pub async fn ensure_background_variant(
    state: State<'_, AppState>,
    url: String,
) -> Result<String, String> {
    let base = state.session.server_url();
    let resolved = filter_to_server_host(&resolve_image_url(&url, &base), &base);
    if resolved.is_empty() {
        return Err("filtered".to_string());
    }
    let client = state.session.client();
    let path = grid_core::images::background::ensure_background_variant(
        state.session.cache(),
        client.as_deref(),
        &resolved,
    )
    .await
    .map_err(err)?;
    Ok(path.to_string_lossy().into_owned())
}
```

and register `commands::ensure_background_variant,` in `app/src-tauri/src/lib.rs`'s invoke handler, directly after `commands::ensure_video,` (`:290`).

- [ ] **Step 10: Add the wrapper** in `app/src/lib/api.ts`, after `ensureVideo` (`:357`):

```ts
  /** The blurred, 960px-wide background variant of `url`, built on demand.
   *  Same host filter as `ensureImage`; a failure means "keep the current art". */
  ensureBackgroundVariant: (url: string) =>
    invoke<string>('ensure_background_variant', { url }),
```

- [ ] **Step 11: Run** `cargo test --workspace` — green (the new variant, sweep and replenish tests included). Then `cargo fmt` and both clippy commands. From `app/`: `npx vitest run` and `npm run check`.

- [ ] **Step 12: Commit**

```bash
git add crates/grid-core/Cargo.toml crates/grid-core/src/images/background.rs crates/grid-core/src/images/mod.rs crates/grid-core/src/images/cache.rs crates/grid-core/src/images/sweep.rs crates/grid-core/src/images/replenish.rs app/src-tauri/src/images.rs app/src-tauri/src/commands.rs app/src-tauri/src/lib.rs app/src/lib/api.ts Cargo.lock
git commit -m "rewrite: build the background art variant once in Rust instead of blurring every frame"
```

(If `Cargo.lock` is at the repo root rather than `rewrite/Cargo.lock`, adjust the pathspec — `ls rewrite/Cargo.lock` confirms it is in `rewrite/`.)

---

### Task 9: `BackgroundArt.svelte` — cycling, memoised paths, no CSS blur

**Files:**
- Create: `app/src/lib/backgroundPrefetch.ts`
- Modify: `app/src/lib/BackgroundArt.svelte` (whole file)
- Modify: `app/src/lib/lastViewedHover.ts` (the 150 ms prefetch timer) and `lastViewedHover.test.ts`

**Interfaces:**
- Produces: `export function prefetchBackground(subject: BackgroundSubject): void` in `backgroundPrefetch.ts` — fire-and-forget, never throws, never awaited.
- Changes: `createHoverViewed(delayMs?: number, prefetchMs?: number)` — the second timer fires at `PREFETCH_DELAY_MS` and calls `prefetchBackground`; the swap timer is unchanged at `HOVER_DELAY_MS`.
- Consumes: `api.ensureBackgroundVariant` (Task 8), `backgroundUrls` / `BACKGROUND_CYCLE_MS` / `shouldCycle` / `cycleIndex` / `CROSS_FADE_MS` (Task 7), `initialSlotState` / `withNextCover` / `outgoingSlot` / `clearIfBottom` (`backgroundSlots.ts`, unchanged).

- [ ] **Step 1: Create the prefetch seam** — `app/src/lib/backgroundPrefetch.ts`:

```ts
// The one place a dwell timer touches the backend. Split out of
// `lastViewedHover.ts` so that module stays trivially mockable in vitest (it
// already mocks the store the same way) and so nothing else can start a
// background fetch by accident.
import { api } from './api';
import { backgroundUrls, type BackgroundSubject } from './background';

/**
 * Starts building `subject`'s first background image, without waiting for it.
 * Called at 150ms of dwell while the actual swap still waits for 500ms, so
 * the fetch + decode + blur has a head start and the swap usually has
 * something ready. A failure is silent: the swap path asks again and, if that
 * also fails, the current art simply stays.
 */
export function prefetchBackground(subject: BackgroundSubject): void {
  const url = backgroundUrls(subject)[0];
  if (url === undefined) return;
  void api.ensureBackgroundVariant(url).catch(() => {});
}
```

- [ ] **Step 2: Write the failing dwell test.** In `app/src/lib/lastViewedHover.test.ts`, add the prefetch mock beside the existing store mock at the top:

```ts
const { prefetchBackground } = vi.hoisted(() => ({ prefetchBackground: vi.fn() }));
vi.mock('./backgroundPrefetch', () => ({ prefetchBackground }));
```

add `prefetchBackground.mockClear();` to the existing `beforeEach`, and append:

```ts
  it('starts the fetch at 150ms but does not swap until 500ms', () => {
    const hover = createHoverViewed(500, 150);
    hover.start(subject('https://romm/cover.png'));

    vi.advanceTimersByTime(150);
    expect(prefetchBackground).toHaveBeenCalledExactlyOnceWith(subject('https://romm/cover.png'));
    expect(noteViewed).not.toHaveBeenCalled();

    vi.advanceTimersByTime(350);
    expect(noteViewed).toHaveBeenCalledExactlyOnceWith(subject('https://romm/cover.png'));
  });

  it('cancels the prefetch too when the pointer leaves early', () => {
    const hover = createHoverViewed(500, 150);
    hover.start(subject('https://romm/cover.png'));
    vi.advanceTimersByTime(100);
    hover.end();
    vi.advanceTimersByTime(1000);
    expect(prefetchBackground).not.toHaveBeenCalled();
    expect(noteViewed).not.toHaveBeenCalled();
  });
```

- [ ] **Step 3: Run** `npx vitest run lastViewedHover` — the two new cases fail.

- [ ] **Step 4: Implement the second timer** in `app/src/lib/lastViewedHover.ts`:

```ts
import { HOVER_DELAY_MS, PREFETCH_DELAY_MS } from './background';
import type { BackgroundSubject } from './background';
import { prefetchBackground } from './backgroundPrefetch';
import { noteViewed } from './stores/lastViewed.svelte';

export function createHoverViewed(
  delayMs: number = HOVER_DELAY_MS,
  prefetchMs: number = PREFETCH_DELAY_MS
): {
  start: (subject: BackgroundSubject) => void;
  end: () => void;
} {
  let timer: ReturnType<typeof setTimeout> | null = null;
  let prefetchTimer: ReturnType<typeof setTimeout> | null = null;

  function clearBoth(): void {
    if (timer !== null) clearTimeout(timer);
    if (prefetchTimer !== null) clearTimeout(prefetchTimer);
    timer = null;
    prefetchTimer = null;
  }

  function start(subject: BackgroundSubject): void {
    clearBoth();
    // Two timers, not one: the fetch is the slow part (network, decode,
    // blur), so it starts a third of the way into the dwell, while the visual
    // swap still waits the full 500ms design §3 asks for. A dwell abandoned
    // before 150ms costs nothing at all.
    prefetchTimer = setTimeout(() => {
      prefetchTimer = null;
      prefetchBackground(subject);
    }, prefetchMs);
    timer = setTimeout(() => {
      timer = null;
      noteViewed(subject);
    }, delayMs);
  }

  function end(): void {
    clearBoth();
  }

  return { start, end };
}
```

- [ ] **Step 5: Run** `npx vitest run lastViewedHover` — green.

- [ ] **Step 6: Rewrite `app/src/lib/BackgroundArt.svelte`** in full:

```svelte
<script lang="ts">
  import { convertFileSrc } from '@tauri-apps/api/core';
  import { api } from './api';
  import { BACKGROUND_CYCLE_MS, CROSS_FADE_MS, cycleIndex, shouldCycle } from './background';
  import { clearIfBottom, initialSlotState, outgoingSlot, withNextCover } from './backgroundSlots';
  import { lastViewed } from './stores/lastViewed.svelte';
  import { uiSettings } from './stores/uiSettings.svelte';

  // Two layers so a change cross-fades rather than popping (design §3:
  // 360ms). A new image is written only into the slot about to become
  // visible; the outgoing slot's image is left in place until the fade has
  // had time to finish, so both images sit on screen together while the
  // opacity transitions — see `backgroundSlots.ts` for the sequencing.
  let slots = $state(initialSlotState);

  // Resolved URL -> local variant path, for the process lifetime. The path is
  // stable (the cache is content-addressed by URL) and the answer costs an
  // IPC round trip, so re-hovering a card the user has already dwelled on
  // must not pay for it again. Module scoped, like `lastViewed` itself, so it
  // survives a Shell remount.
  const variantPaths = new Map<string, string>();

  // Which of the subject's images is showing. Reset whenever the subject
  // changes, so a new game always starts at its first image.
  let index = $state(0);
  let urls = $derived(lastViewed.urls);
  $effect(() => {
    // Referencing `urls` is what subscribes this effect to a subject change.
    urls;
    index = 0;
  });

  let current = $derived(urls[index % Math.max(urls.length, 1)] ?? null);

  // 0-60 in the config, 0-0.6 as an opacity.
  let opacity = $derived(uiSettings.backgroundFade / 100);

  // The 5s rotation (fanart_background.py:52-53, 80-84). Only with more than
  // one image, and only while the art is visible — user ruling 2026-09-05.
  $effect(() => {
    if (!shouldCycle(urls, uiSettings.backgroundFade)) return;
    const count = urls.length;
    const timer = setInterval(() => {
      index = cycleIndex(index, count);
    }, BACKGROUND_CYCLE_MS);
    return () => clearInterval(timer);
  });

  $effect(() => {
    const url = current;
    if (url === null) return;
    let cancelled = false;
    // The timeout handle is captured now and cleared on teardown: before
    // this, a rapid sequence of subjects left one pending `clearIfBottom` per
    // change, each firing after the component may already have gone.
    let clearTimer: ReturnType<typeof setTimeout> | null = null;

    function show(path: string) {
      const src = convertFileSrc(path);
      if (slots[slots.top] === src) return; // already showing this image
      slots = withNextCover(slots, src);
      const toClear = outgoingSlot(slots);
      clearTimer = setTimeout(() => {
        clearTimer = null;
        slots = clearIfBottom(slots, toClear);
      }, CROSS_FADE_MS);
    }

    const memoised = variantPaths.get(url);
    if (memoised !== undefined) {
      show(memoised);
    } else {
      api
        .ensureBackgroundVariant(url)
        .then((path) => {
          variantPaths.set(url, path);
          if (!cancelled) show(path);
        })
        .catch(() => {
          // Offline, missing, or a format this build cannot decode. User
          // ruling 2026-09-05: no raw-image fallback — the CSS blur is gone,
          // so the raw source would be a different effect, not a degraded
          // one. Keep whatever is already showing.
        });
    }

    return () => {
      cancelled = true;
      if (clearTimer !== null) clearTimeout(clearTimer);
    };
  });
</script>

<div data-testid="background-art" class="art" aria-hidden="true" style={`--art-opacity: ${opacity}`}>
  <div class="layer" class:visible={slots.top === 'a'} style={slots.a ? `background-image: url("${slots.a}")` : ''}></div>
  <div class="layer" class:visible={slots.top === 'b'} style={slots.b ? `background-image: url("${slots.b}")` : ''}></div>
</div>

<style>
  .art {
    position: fixed;
    inset: 0;
    /* Behind every positioned view wrapper and the unpositioned shell
       chrome (the `session-error` line), which paint at z 0. */
    z-index: -1;
    pointer-events: none;
    overflow: hidden;
  }

  .layer {
    position: absolute;
    /* `inset: 0`, not the old `-60px` overscan: the image arrives already
       blurred (`ensure_background_variant`), so nothing samples past the
       element's edges any more and there is nothing to hide. */
    inset: 0;
    background-position: center;
    background-size: cover;
    /* No `filter: blur(40px)`. The blur is baked into the 960px JPEG the
       backend builds once, so the compositor uploads one small texture
       instead of re-blurring ~2.4 Mpx per layer per frame for the whole
       fade. */
    opacity: 0;
    transition: opacity var(--m-slow) ease;
    /* Promotes each layer to its own compositor layer for the fade — the
       only property that animates here. */
    will-change: opacity;
  }

  .layer.visible {
    opacity: var(--art-opacity);
  }
</style>
```

- [ ] **Step 7: Run** `npx vitest run` and, from `app/`, `npm run check` — green, no new warnings. Confirm `grep -n "blur(40px)" app/src/lib/BackgroundArt.svelte` returns nothing and that `CROSS_FADE_MS` (`background.ts`) still equals `--m-slow` (`app/src/app.css:48`, 360ms) — the two must agree for the fade to look right.

- [ ] **Step 8: Commit**

```bash
git add app/src/lib/backgroundPrefetch.ts app/src/lib/BackgroundArt.svelte app/src/lib/lastViewedHover.ts app/src/lib/lastViewedHover.test.ts
git commit -m "rewrite: cycle the background art and composite a pre-blurred variant"
```

---

### Task 10: Behaviour docs

**Files:**
- Modify: `docs/superpowers/specs/2026-09-04-desktop-ui-redesign-design.md:42-45` (§3's background bullet)
- Modify: `docs/porting/07-covers-images.md` — the "Fanart background (TV)" section (`:598`) and the "Rulings" list (`:989-1012`)

(D-UI-3 was already corrected by Task 1.)

- [ ] **Step 1: Rewrite the design spec's §3 background bullet.** Replace the four lines at `:42-45` with:

```
- Background art: the art of the last game the user viewed (opened in the details
  popup, selected with the keyboard/gamepad, or hovered for more than 500ms), chosen
  fanart → screenshots → large cover (user ruling 2026-09-05); more than one image
  rotates every 5000ms, and the rotation stops while the fade is 0. Falls back on
  startup to the most recently installed game that has any art. The image is scaled to
  960px and blurred by the backend once (`ensure_background_variant`) rather than by the
  compositor every frame; opacity comes from the Settings › Appearance fade slider
  (0–60%, default 25%, stored as `ui.background_fade`); cross-fades over 360ms.
```

- [ ] **Step 2: Record the rewrite's own fanart source** in `docs/porting/07-covers-images.md`. At the end of the "Fanart background (TV)" section (after the cycle-behaviour list), add a short subsection "Rust port (round 4)" stating:
  - the desktop rewrite now has a background where Python's desktop had none, and it follows the TV widget's 5000 ms cycle and its "more than one URL" gate (`fanart_background.py:52-53, 80-84`);
  - unlike TV, it has a real fanart source: `RomSSMetadata.fanart_path` and `RomGamelistMetadata.fanart_path`, read by `fanart_urls_from_payload` (`crates/grid-core/src/images/urls.rs`) off both `SimpleRomSchema` and `DetailedRomSchema`, so the Server grid and the Library both have it without a per-card detail fetch. The doc's own note that TV carried `fanart_url` through the catalog but never displayed it is what this closes;
  - `NON_SCREENSHOT_ART_RE` is deliberately NOT applied to fanart — it exists to keep fanart out of *screenshot* lists;
  - `fanart_url` (usually an external host) is dropped by `filter_to_server_host`, and that is intended: apart from the YouTube trailer thumbnail, nothing may leave the server host;
  - the priority is fanart → screenshots → cover, and the cover is a last resort because a portrait cover stretched across a landscape window is the worst of the three.

- [ ] **Step 3: Add the variant to the "Game videos (rewrite only)" neighbourhood.** Add a new section after it, "Background variant (rewrite only)", stating:
  - `images::background::ensure_background_variant` reuses `ImageCache::ensure` for the source and stores `<key>.bg.jpg` beside it, the same directory + key scheme `video.rs` uses;
  - 960px wide (Triangle), `fast_blur` σ 12, JPEG quality 80, written through a `.bg.part` + rename (a distinct temp name from `ImageCache`'s own `<key>.part`);
  - `sweep::pinned_keys` pins by key PREFIX so a variant is never evicted while its source is pinned;
  - it is built ahead of time by `spawn_prefetch` (install) and by `replenish::plan`'s `NeedsVariant` items (connect), and on demand at 150 ms of hover dwell;
  - the reason: `BackgroundArt.svelte` used to hand a full-resolution cover to two `filter: blur(40px)` layers, which is Python's TV `_blur_pixmap` done per frame instead of once on arrival.

- [ ] **Step 4: Append to the "Rulings" list** in the same file:
  - **"The background art has a variant; the cover pipeline does not."** `ensure_image` still returns the raw cached bytes for every card and screenshot; only the shell background asks for `ensure_background_variant`. One extra variant per background source, not per image.
  - **"A failed variant keeps the current art."** There is no raw-image fallback: the CSS blur is gone, so the raw source would be a different effect rather than a degraded one.
  - **"The YouTube trailer thumbnail is the only foreign host."** `https://img.youtube.com/vi/<id>/hqdefault.jpg`, a plain `<img>` with `referrerpolicy="no-referrer"`, allowed by `img-src` in `app/src-tauri/tauri.conf.json`. It is deliberately NOT routed through `ensure_image`, which would fetch it via `RommClient` and attach the RomM Authorization header to a foreign request. On error it falls back to the server-hosted cover with the same play badge.

- [ ] **Step 5: Commit**

```bash
git add ../docs/superpowers/specs/2026-09-04-desktop-ui-redesign-design.md ../docs/porting/07-covers-images.md
git commit -m "rewrite: document the background art selection, its variant and the trailer thumbnail"
```

---

### Task 11: E2E coverage and gate

**Files:**
- Modify: `e2e/fixtures/rom-details.json` (rom 103 gains `merged_screenshots`)
- Modify: `e2e/specs/images-a.spec.ts` (two new cases)
- Modify: `e2e/specs/library.spec.ts` (one new case)

**Interfaces:**
- Consumes: `mockUrl`, `TRANSITION_TIMEOUT`, `APP_START_TIMEOUT` from `e2e/helpers/env.ts` — already imported by both specs.

**Mock-server rulings (verified before writing):**
1. **The mock CAN serve a 404 screenshot.** `server.mjs`'s `SCREENSHOT_PATH_RE` is `/^\/assets\/romm\/resources\/roms\/\d+\/screenshots\/\d+\.png$/` (`:437`); anything else under `/assets/` falls through to `sendJson(res, 404, …)` (`:594`). `RommClient::get_bytes_with_type` turns a non-2xx into `RommError::Http` (`romm/mod.rs:148-153`), so `ensure_image` rejects and `Image.svelte` reports `onerror` — exactly the path the failed-tile removal is built on. A URL ending `/screenshots/missing.png` also survives `looks_like_screenshot_url` (it matches neither `SCREENSHOT_HINT_RE` — "screenshots" is not "screenshot" followed by a non-alphanumeric — nor `NON_SCREENSHOT_ART_RE`), so it does reach `screenshot_urls`.
2. **The 404 screenshot goes on rom 103, not rom 101.** `images-a.spec.ts` asserts `details-screenshot-2` does NOT exist and that `details-media-2` DOES (the trailer tile); adding a fourth entry to rom 101 would make both assertions race a tile that appears and then disappears.
3. **The YouTube thumbnail's own load is NOT asserted.** The runner's network is unknown: with internet the thumbnail loads, without it the tile falls back to the cover. Asserting either branch would be flaky, so E2E asserts what is true both ways — the tile paints an `<img>` and a play badge instead of the bare icon it used to — and the fallback RULE is covered by `media.test.ts` (Task 3).
4. **No E2E asserts the background layer's `background-image` today** (`grep -rn "background-image\|background-art" e2e/specs` matches only `updates.spec.ts:387`'s `background-art-toggle` checkbox). So there is nothing to keep true; this task ADDS the assertion instead, on the `.bg.jpg` suffix.

- [ ] **Step 1: Add the 404 screenshot fixture.** In `e2e/fixtures/rom-details.json`, add to rom `"103"` (which has none today):

```json
  "merged_screenshots": [
    "/assets/romm/resources/roms/103/screenshots/1.png",
    "/assets/romm/resources/roms/103/screenshots/missing.png"
  ],
```

The first path matches the mock's screenshot route and serves a real PNG; the second does not match (`missing` is not `\d+`) and 404s. Rom 103 is the multi-disc fixture used by `install`/`library`; neither group asserts anything about its screenshots.

- [ ] **Step 2: Add the failed-tile case** to `e2e/specs/images-a.spec.ts`, after the `'renders the redesigned popup: …'` case and before `'installs rom 101'`:

```ts
  it('drops a screenshot tile whose image 404s, and keeps the one that loads', async () => {
    // rom 103's second merged_screenshots entry does not match the mock's
    // screenshot route, so it 404s (see the mock-server ruling in this
    // spec's header). The tile must disappear rather than sit there as a
    // permanent placeholder — the round-4 bug this closes.
    await $(testId('game-card-103')).click();
    await $(testId('details-panel')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the details overlay never opened for rom 103',
    });

    await waitForLoadedImage(
      testId('details-screenshot-0'),
      TRANSITION_TIMEOUT,
      "rom 103's first screenshot",
    );
    await $(testId('details-screenshot-1')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      reverse: true,
      timeoutMsg: 'the 404 screenshot tile never went away on the Overview strip',
    });

    await $(testId('details-tab-media')).click();
    await $(testId('details-media-0')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    // Indices are stable (`{#if}` inside `{#each}`), so the dead tile is
    // simply absent rather than renumbering the surviving ones.
    await $(testId('details-media-1')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      reverse: true,
      timeoutMsg: 'the 404 screenshot tile never went away on the Media tab',
    });

    await $(testId('details-tab-overview')).click();
    await $(testId('details-close')).click();
    await $(testId('details-panel')).waitForExist({ timeout: TRANSITION_TIMEOUT, reverse: true });
  });
```

- [ ] **Step 3: Add the trailer-poster case** to the same spec, right after the case above:

```ts
  it('paints artwork and a play badge on the trailer tile', async () => {
    await $(testId('game-card-101')).click();
    await $(testId('details-panel')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await $(testId('details-tab-media')).click();
    await $(testId('details-media-2')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: "rom 101's trailer tile never rendered",
    });

    // Which poster wins depends on whether the runner can reach
    // img.youtube.com, so this asserts what is true either way: the tile is
    // artwork with a play badge, not the bare icon it used to be. The
    // fallback RULE is pinned by media.test.ts's `trailerPoster` cases.
    await browser.waitUntil(
      async () =>
        (await $(testId('details-media-thumb-2')).isExisting()) ||
        (await $(testId('details-media-poster-2')).isExisting()),
      {
        timeout: TRANSITION_TIMEOUT,
        timeoutMsg: 'the trailer tile rendered neither the thumbnail nor the cover poster',
      },
    );
    if (await $(testId('details-media-thumb-2')).isExisting()) {
      await expect($(testId('details-media-thumb-2'))).toHaveAttribute(
        'src',
        'https://img.youtube.com/vi/dQw4w9WgXcQ/hqdefault.jpg',
      );
    }
    await expect($(testId('details-media-play-2'))).toExist();

    await $(testId('details-tab-overview')).click();
    await $(testId('details-close')).click();
    await $(testId('details-panel')).waitForExist({ timeout: TRANSITION_TIMEOUT, reverse: true });
  });
```

- [ ] **Step 4: Assert the background variant** in `e2e/specs/library.spec.ts`, as the last case:

```ts
  // Round 4: the shell background is no longer the raw cover blurred by the
  // compositor — the backend builds one 960px, pre-blurred `<key>.bg.jpg`
  // (`ensure_background_variant`) and the layer composites that. Opening rom
  // 101's details reports its subject synchronously (`noteViewed`), so no
  // hover dwell has to be simulated here.
  it('paints the pre-blurred background variant after a game is viewed', async () => {
    await $(testId('platform-btn-1')).click();
    await $(testId('game-card-101')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await $(testId('game-card-101')).click();
    await $(testId('details-panel')).waitForExist({ timeout: TRANSITION_TIMEOUT });

    await browser.waitUntil(
      async () => {
        const images = await browser.execute(() =>
          Array.from(document.querySelectorAll('[data-testid="background-art"] .layer'))
            .map((el) => (el as HTMLElement).style.backgroundImage)
            .join(' '),
        );
        return images.includes('.bg.jpg');
      },
      {
        timeout: TRANSITION_TIMEOUT,
        timeoutMsg: 'the background layer never got a .bg.jpg variant path',
      },
    );

    await $(testId('details-close')).click();
    await $(testId('details-panel')).waitForExist({ timeout: TRANSITION_TIMEOUT, reverse: true });
  });
```

Rom 101 is the one platform-1 fixture with a `path_cover_large` AND (after Task 6) `merged_screenshots` on the list payload, so the subject is non-empty either way. The assertion is on the SUFFIX only, deliberately: whether the chosen tier is the first screenshot or the cover is Task 7's business, and both end in `.bg.jpg`.

- [ ] **Step 5: Run the gate.** From `rewrite/`, detached with a log:

```bash
nohup scripts/e2e.sh cloud-saves images library install launch > /tmp/claude-1000/-home-six-Documents-Programming-grid-launcher/d527a4be-8a2d-487c-bc02-e067fbdcf4ce/scratchpad/e2e-round4.log 2>&1 &
```

then poll the log until the summary line appears. `cloud-saves` covers both upload toasts; `images` covers the failed tile, the trailer poster and the whole cover pipeline after the schema bump; `library` covers the background variant and the un-dimmed cards; `install` and `launch` cover the registry v5 migration on the two groups that write rows through the real pipeline.

- [ ] **Step 6:** All five groups green. If one fails, read the failure, fix the cause within this plan's scope, re-run that group, and commit the fix with a `rewrite: ` subject. The two likeliest failures and their causes: a stale `e2e/seed/registry-schema.mjs` (Task 6 Step 15 — it must say 5 and carry `fanart_urls`), and a toast text that does not match the seed (Task 5 Step 6's reconciliation).

- [ ] **Step 7:** Report the per-group result lines verbatim.

- [ ] **Step 8: Commit**

```bash
git add e2e/fixtures/rom-details.json e2e/specs/images-a.spec.ts e2e/specs/library.spec.ts
git commit -m "rewrite: cover the dropped screenshot tile, the trailer poster and the background variant in E2E"
```

---

## Self-review notes

**Spec coverage.** Every item of the stated scope maps to a task. **A (upload feedback):** the event, its payload, the summary→message mapping and its Rust tests → Task 4; the `AppHandle` threading (hook + command, `e2e` feature build kept green by Task 4 Step 9's second clippy run) → Task 4 Steps 5-8; the Shell listener and `pushToast` with an error level on failure → Task 5; the manual-upload E2E assertion → Task 5 Step 4, with the exit auto-upload also asserted at Step 5 (both extend cases the fixture already drives; no new flow). **B (media):** failed tiles in `MediaTab`/`MediaViewer` → Task 2; the `Image.svelte` tri-state with a token-only shimmer → Task 2 Steps 2-4; the YouTube thumbnail, the cover+badge fallback, hosted-video posters and the pure-module tests → Task 3; the CSP `img-src` (one host, nothing else) → Task 3 Step 8; the E2E → Task 11 Steps 1-3. **C (server fade):** → Task 1, spec line included. **D1 (selection):** `BackgroundSubject`, priority, 5000 ms cycle gated on the fade, `BACKGROUND_CYCLE_MS` in `background.ts` → Task 7 (+ the cycle's render in Task 9); the four writers (details open from the merged detail, keyboard/gamepad focus, hover dwell, startup seed) → Task 7 Steps 8-10 and the store; `RomDetail.fanart_urls`, `GameSummary` gaining both lists, the registry column + migration, the Rust deserialisation tests and the `NON_SCREENSHOT_ART_RE` exclusion → Task 6; the pure-module tests → Task 7 Steps 1 and 5; the spec §background line → Task 10 Step 1. **D2 (performance):** `ensure_background_variant` following `video.rs`, the `image` dependency added with `cargo add`, the sweep prefix pin, `spawn_prefetch`, `replenish::plan`, the command and the `api.ts` wrapper, and the three Rust tests → Task 8; the `BackgroundArt` blur removal, `will-change`, captured timeout, memo Map and 150 ms dwell prefetch → Task 9; the E2E → Task 11 Step 4.

**Placeholder scan.** No step says "similar to", "add appropriate" or "TODO". Four steps are deliberately conditional and each states exactly what to do in every branch: Task 5 Step 6 (reconcile the toast text against a real run rather than reword the toast), Task 6 Step 10 (`ALTER TABLE … DROP COLUMN` needs SQLite 3.35+; build the v4 table by hand if it errors), Task 8 Step 1 (`cargo add` needs the network — stop rather than invent a version), Task 8 Step 2 (compare file length instead of mtime if the filesystem's timestamps are too coarse). Task 6 Step 19 says explicitly to read `subject.test.ts` and reuse its own fixture builders rather than invent names, because that file's helpers were not read while writing this plan — the only named-but-unverified identifiers anywhere here.

**Type consistency across tasks.** `BackgroundSubject { fanart, screenshots, cover }` is produced by `background.ts` (Task 7) and consumed unchanged by the store, `lastViewedHover.ts`, `backgroundPrefetch.ts` (Task 9), `Library.svelte`, `Server.svelte` and `Details.svelte`; `noteViewed`'s argument changes from `string | null | undefined` to `BackgroundSubject` in ONE commit together with all four call sites and the hover module's test. `startupCover` is deleted and `startupSubject` replaces it in the same commit as its only caller (`seedLastViewed`). `GameSummary` gains `screenshot_urls`/`fanart_urls` in Rust (Task 6 Step 7), in `api.ts` (Step 17) and in `subject.ts`'s `summaryOf` shim (Step 18) in one commit, so the shim never fails to satisfy the type. `ImageFields.fanart_urls` is added with all three of its struct literals in the same step (Task 6 Step 13). `InstalledGame.fanart_urls` lands with the migration, the `SELECT_COLUMNS` append (index 36, so no existing `row.get(n)` moves), `upsert`'s `?38` and `update_images`' `?4`. `ReplenishItem::NeedsVariant` is added and matched in the same file in Task 8 Step 7 — `run`'s match is exhaustive, so a missed arm is a compile error, not a silent skip. `CloudUploadFinished { title, message, failed }` (Rust, Task 4) is mirrored field-for-field in `api.ts` (Task 5 Step 1) and read only by the Shell listener. `MediaTab`/`MediaViewer`'s two new props are added and passed in the same task (Task 2 Steps 5-7); `MediaTab`'s third prop, `coverUrl`, is added and passed in Task 3 (Steps 5 and 7), so the file never compiles with an unsatisfied required prop.

**Three corrections made during review, folded into the tasks above.**
1. The brief said `images-b.spec.ts` and `library.spec.ts` "currently assert the background layer's `background-image`". They do not — `grep -rn "background-image\|background-art" e2e/specs` matches only `updates.spec.ts:387`'s `background-art-toggle` checkbox. There is therefore nothing to keep true; Task 11 Step 4 adds the assertion to `library.spec.ts` instead, and Task 11's header records the check.
2. The brief's E2E for the trailer tile assumed the mock has no network and the thumbnail always falls back to the cover. The mock has no network, but the *runner's host* may well have one, and `https://img.youtube.com` is not proxied through the mock — so the fallback branch is not deterministic. Task 11 Step 3 asserts the invariant that holds either way (artwork + play badge, with the thumbnail's `src` checked only when that branch is the live one) and leaves the fallback rule to `media.test.ts`.
3. `e2e/seed/registry-schema.mjs` is not mentioned in the brief but MUST change with the registry migration: its own doc comment says `Registry::open` trusts a database already at `LATEST_USER_VERSION`, so a stale copy would silently give the `firmware`, `native`, `content`, `ps3-install` and `updates` groups a table with no `fanart_urls` column and fail every `SELECT`. Task 6 Step 15 covers it.

**One thing this plan changes that no test previously covered:** `sweep`'s stem comparison. Every cached file used to have a single-dot name, so `file_stem()` was the key; `<key>.bg.jpg` breaks that, and without Task 8 Step 6 the sweep would evict an installed game's background variant while keeping the source it was built from. It is covered by a new Rust unit test in `sweep.rs`.
