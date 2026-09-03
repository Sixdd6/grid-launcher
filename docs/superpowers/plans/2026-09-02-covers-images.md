# Covers, Images, and the Offline Library Implementation Plan (rewrite milestone 7)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port doc 07's desktop image behavior (URL selection, screenshot
filtering, disk cache, replenish) to grid-core/Tauri, add the spec's
top-bar shell with an offline-capable installed Library section, and give
Details a large cover plus a scrollable screenshot strip.

**Architecture:** `covers.rs` becomes `grid-core/src/images/` (pure URL
rules → disk cache with dedup/semaphore/content gate → startup sweep →
replenish job). The registry migrates to schema v2 with three image
columns filled at install. The Tauri layer exposes `ensure_image`,
`get_rom_detail`, `retry_connect`, a three-way `restore_session`, and an
`images-replenished` event. The frontend gains a `Shell` (top bar + chip),
a `Server` section (the old grid), a `Library` section (installed grid),
one `Image` component, and a Details subject model.

**Tech Stack:** Rust (reqwest, sha2, regex, percent-encoding, rusqlite,
tokio sync primitives, wiremock), Tauri 2, Svelte 5, vitest, WebdriverIO.

**Spec:** `docs/superpowers/specs/2026-09-02-covers-images-design.md`
(binding). Behavior contract: `docs/porting/07-covers-images.md`; where the
spec is silent doc 07 wins; where both are silent the Python source wins:
`grid_launcher/cover/utils.py` (URL rules, verbatim port),
`grid_launcher/ui/game_views.py:297-311` (library sort/hide),
`grid_launcher/background/workers.py:836-884` (replenish). Oracle tests:
`tests/test_screenshot_urls.py`.

## Global Constraints

- **Behavioral parity, free machinery** (user ruling). What the user sees
  matches Python; how it is produced is this plan's choice. The pure URL
  functions in Task 1 are the exception: they port `cover/utils.py`
  line-for-line, including quirks (permissive screenshot default,
  whole-netloc host comparison, `append_url` continuing to the next dict
  key when a resolved URL is already present).
- **Deviations D1–D10** (spec) are the only intended behavior changes. New
  one found while planning, record as **D11**: Python's SVG sniff calls
  `bytes.casefold()` (nonexistent) and would raise on an `<?xml` body; the
  port lowercases.
- **Tokens.** No new `expose_secret()` call sites. `ImageError`, logs and
  events carry at most a URL path, never a header or query string. All
  fetches go through `RommClient`. `scripts/check_secret_hygiene.sh`
  passes after every task.
- **grid-core never imports Tauri.** The app layer owns the AppHandle,
  events, and the replenish trigger.
- **Cache key = lowercase hex SHA-256 of the resolved absolute URL.** One
  scheme everywhere (D1). Filename `<key>.<ext>`; the lookup extension
  list is exactly what `extension_for` can produce.
- **Constants:** `MAX_CONCURRENT_DOWNLOADS = 6`, `IMAGE_CACHE_CAP_BYTES =
  512 * 1024 * 1024`, `STALE_PART_AGE = 1 hour`, per-fetch timeout is the
  existing 30 s reqwest client timeout.
- **Registry:** `LATEST_USER_VERSION = 2`; columns `cover_small_path`,
  `cover_large_path`, `screenshot_urls` (`TEXT NOT NULL DEFAULT ''`).
  `screenshot_urls` stores newline-joined, already resolved and
  host-filtered absolute URLs; cover paths store what the server sent.
- **User-facing strings (verbatim):** "No games installed", "No cover",
  "No screenshots available", "Not connected", "Retry", "Disconnect",
  "This entry has no server id", "not connected", "filtered".
- **Event:** `images-replenished`, payload `ReplenishReport { updated_rows,
  fetched_files, skipped }`.
- **Every task ends green**, run from `rewrite/`:
  - `cargo test --workspace`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo fmt --check`
  - `bash scripts/check_secret_hygiene.sh`
  - `npm run check` + `npm test` in `rewrite/app` when the frontend is touched
  - `rewrite/scripts/e2e.sh` (every group) gates the milestone at Task 12.
- **Existing E2E test ids stay.** `platform-btn-*`, `game-card-*`,
  `details-*`, `downloads-footer`, `downloads-drawer`, `emulators-open`,
  `emulators-panel`, `library-path-*`, `installed-badge-*`, `connect-*`
  keep their names and meaning. Specs wait for `platform-btn-1` right after
  connecting, so the Server section must be the active section when the
  shell first renders connected.

## Plan-level rulings (surface to the user at finish)

- **R1 — Downloads and Emulators stay as the footer drawer and the
  overlay.** The top bar gets buttons for them (`nav-downloads` toggles
  the drawer, `nav-emulators` opens the overlay). The spec's section 1
  said they "become sections"; converting them would rewrite two
  components plus five E2E specs that drive `downloads-footer` /
  `emulators-open`, for no functional gain. Cost if wrong: one later UI
  task.
- **R2 — Default section:** Server when the shell renders connected,
  Library when it renders offline.
- **R3 — The startup sweep runs synchronously inside Tauri `setup`**
  (milliseconds for thousands of files) instead of the spec's
  spawn_blocking + OnceCell gate, which existed only to order the sweep
  before the first fetch. Synchronous ordering makes the gate unnecessary.
- **R4 — `ensure_image` takes only `url`.** The variant is a frontend
  concern (placeholder choice); the backend key is the URL.
- **R5 — E2E offline scenario** is driven by a mock toggle
  (`POST /__e2e__/offline`) set at the end of spec A so spec B's app
  starts unreachable, then cleared before Retry.

## File Structure

```
rewrite/crates/grid-core/src/images/mod.rs        NEW  ImageVariant, ImageFields, pub mods
rewrite/crates/grid-core/src/images/urls.rs       NEW  urlsplit/unsplit, resolve, host filter, cover/screenshot extraction, extension_for
rewrite/crates/grid-core/src/images/cache.rs      NEW  ImageCache (URL-hash key, dedup, semaphore, gate, touch), ImageError
rewrite/crates/grid-core/src/images/sweep.rs      NEW  pinned_keys, sweep
rewrite/crates/grid-core/src/images/replenish.rs  NEW  plan + run + ReplenishReport
rewrite/crates/grid-core/src/covers.rs            DEL  (Task 2)
rewrite/crates/grid-core/src/lib.rs               MOD  pub mod images; drop covers
rewrite/crates/grid-core/src/romm/mod.rs          MOD  get_bytes_with_type; RomDetail/GameSummary image fields
rewrite/crates/grid-core/src/session.rs           MOD  ImageCache, server_url, RestoreOutcome, retry
rewrite/crates/grid-core/src/library/registry.rs  MOD  v2 migration, fields, update_images
rewrite/crates/grid-core/src/library/mod.rs       MOD  new_record fills fields; registry(); image hook
rewrite/crates/grid-core/tests/covers.rs          DEL  → tests/images_cache.rs
rewrite/crates/grid-core/tests/images_cache.rs    NEW
rewrite/crates/grid-core/tests/images_sweep.rs    NEW
rewrite/crates/grid-core/tests/images_replenish.rs NEW
rewrite/crates/grid-core/tests/registry.rs        MOD  v2 + migration tests
rewrite/crates/grid-core/tests/romm_detail.rs     MOD  image fields test
rewrite/crates/grid-core/tests/session.rs         MOD  RestoreOutcome tests
rewrite/app/src-tauri/src/images.rs               NEW  ImageService: sweep_at_startup, replenish trigger, prefetch hook
rewrite/app/src-tauri/src/commands.rs             MOD  ensure_image, get_rom_detail, retry_connect, restore_session
rewrite/app/src-tauri/src/lib.rs                  MOD  sweep, hooks, command registration
rewrite/app/src/lib/api.ts                        MOD  types + invokes
rewrite/app/src/lib/stores/session.svelte.ts      MOD  three-way restore, retry, disconnect
rewrite/app/src/lib/stores/installed.svelte.ts    MOD  replenish listener
rewrite/app/src/lib/shell.ts                      NEW  pure shell helpers + shell.test.ts
rewrite/app/src/lib/Shell.svelte                  NEW  top bar, chip, sections
rewrite/app/src/lib/Server.svelte                 NEW  git mv Library.svelte → Server.svelte (+ offline notice, active prop)
rewrite/app/src/lib/Library.svelte                NEW  installed grid (new meaning)
rewrite/app/src/lib/library.ts                    NEW  visibleLibraryGames + library.test.ts
rewrite/app/src/lib/Image.svelte                  NEW  replaces Cover.svelte
rewrite/app/src/lib/details/subject.ts            NEW  DetailsSubject + subject.test.ts
rewrite/app/src/lib/Details.svelte                MOD  subject prop, three-column layout, screenshots
rewrite/app/src/App.svelte                        MOD  phase routing
rewrite/e2e/mock-romm/server.mjs                  MOD  offline toggle, large cover + screenshot routes
rewrite/e2e/mock-romm/server.test.mjs             MOD
rewrite/e2e/fixtures/roms.json, rom-details.json  MOD  rom 101 large cover + screenshots
rewrite/e2e/seed/images-seed.mjs                  NEW
rewrite/e2e/specs/images-a.spec.ts, images-b.spec.ts NEW
rewrite/scripts/e2e.sh                            MOD  images group
rewrite/README.md                                 MOD  coverage row, milestone 7 checklist
docs/porting/07-covers-images.md, 02-…, 10-…      MOD  deviations (Task 12)
```

**Shared types** (Task 1 defines; later tasks consume):

```rust
// images/mod.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageVariant { CoverSmall, CoverLarge, Screenshot }

/// The three registry image columns, as stored.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct ImageFields {
    pub cover_small_path: String,
    pub cover_large_path: String,
    /// Newline-joined resolved + host-filtered absolute URLs.
    pub screenshot_urls: String,
}
```

---

### Task 1: `images::urls` — URL rules, screenshot extraction, extension sniff

**Files:**
- Create: `rewrite/crates/grid-core/src/images/mod.rs`
- Create: `rewrite/crates/grid-core/src/images/urls.rs`
- Modify: `rewrite/crates/grid-core/src/lib.rs` (add `pub mod images;`)

**Interfaces:**
- Produces:
  - `pub fn urlsplit(&str) -> SplitUrl`, `pub fn urlunsplit(&SplitUrl) -> String`
  - `pub fn resolve_image_url(value: &str, base_url: &str) -> String`
  - `pub fn filter_to_server_host(url: &str, base_url: &str) -> String`
  - `pub fn server_resolver(base_url: &str) -> impl Fn(&str) -> String`
  - `pub fn cover_url_from_payload(payload: &Value, resolver: &dyn Fn(&str) -> String) -> String`
  - `pub fn screenshot_urls_from_payload(payload: &Value, resolver: &dyn Fn(&str) -> String) -> Vec<String>`
  - `pub fn screenshot_urls_from_stored(raw: &str) -> Vec<String>`
  - `pub fn looks_like_screenshot_url(&str) -> bool`
  - `pub struct Sniff { pub ext: String, pub identified: bool }` and
    `pub fn extension_for(url: &str, body: &[u8], content_type: &str) -> Sniff`
  - `pub const LOOKUP_EXTENSIONS: [&str; 14]`

- [ ] **Step 1: Write the failing tests** at the bottom of `urls.rs` in a `#[cfg(test)] mod tests`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn identity(v: &str) -> String {
        v.to_string()
    }

    // tests/test_screenshot_urls.py:13
    #[test]
    fn launchbox_typed_images_keep_only_screenshot_types() {
        let payload = json!({"launchbox_metadata": {"images": [
            {"type": "Box - Front", "url": "https://img.example/box-front.jpg"},
            {"type": "Fanart - Background", "url": "https://img.example/fanart.jpg"},
            {"type": "Clear Logo", "url": "https://img.example/logo.png"},
            {"type": "Screenshot - Gameplay", "url": "https://img.example/shot-gameplay.jpg"},
            {"type": "Screenshot - Game Title", "url": "https://img.example/shot-title.jpg"}
        ]}});
        assert_eq!(
            screenshot_urls_from_payload(&payload, &identity),
            vec!["https://img.example/shot-gameplay.jpg", "https://img.example/shot-title.jpg"]
        );
    }

    // tests/test_screenshot_urls.py:36
    #[test]
    fn metadata_blocks_exclude_non_screenshot_fields() {
        let payload = json!({
            "gamelist_metadata": {"screenshot_url": "https://img.example/gamelist-shot.jpg",
                "title_screen_url": "https://img.example/gamelist-title.jpg",
                "image_url": "https://img.example/gamelist-box-art.jpg"},
            "ss_metadata": {"screenshot_url": "https://img.example/ss-shot.jpg",
                "title_screen_url": "https://img.example/ss-title.jpg",
                "fanart_url": "https://img.example/ss-fanart.jpg"}
        });
        assert_eq!(
            screenshot_urls_from_payload(&payload, &identity),
            vec![
                "https://img.example/gamelist-shot.jpg",
                "https://img.example/gamelist-title.jpg",
                "https://img.example/ss-shot.jpg",
                "https://img.example/ss-title.jpg"
            ]
        );
    }

    // tests/test_screenshot_urls.py:62
    #[test]
    fn screenshot_source_order_and_images_block_filtering() {
        let payload = json!({
            "merged_screenshots": ["https://img.example/merged-shot-1.jpg", "https://img.example/merged-shot-2.jpg"],
            "url_screenshots": ["https://img.example/list-shot-1.jpg", "https://img.example/list-shot-2.jpg"],
            "images": [
                {"type": "Screenshot - Gameplay", "url": "https://img.example/images-shot-1.jpg"},
                {"type": "Box - Front", "url": "https://img.example/box-front.jpg"},
                {"type": "Fanart - Background", "url": "https://img.example/fanart.jpg"}
            ]
        });
        assert_eq!(
            screenshot_urls_from_payload(&payload, &identity),
            vec![
                "https://img.example/merged-shot-1.jpg",
                "https://img.example/merged-shot-2.jpg",
                "https://img.example/list-shot-1.jpg",
                "https://img.example/list-shot-2.jpg",
                "https://img.example/images-shot-1.jpg"
            ]
        );
    }

    #[test]
    fn user_screenshots_take_every_path_key_and_dedupe() {
        let payload = json!({"user_screenshots": [
            {"download_path": "/assets/roms/1/screenshots/a.png", "file_path": "/assets/roms/1/screenshots/a.png", "full_path": "/x/b.png"},
            "not-a-dict"
        ]});
        assert_eq!(
            screenshot_urls_from_payload(&payload, &identity),
            vec!["/assets/roms/1/screenshots/a.png", "/x/b.png"]
        );
    }

    #[test]
    fn non_list_screenshot_value_is_appended_as_single_item() {
        let payload = json!({"screenshots": "https://img.example/single-shot.jpg",
                             "url_screenshot": "https://img.example/only.jpg"});
        assert_eq!(
            screenshot_urls_from_payload(&payload, &identity),
            vec!["https://img.example/single-shot.jpg", "https://img.example/only.jpg"]
        );
    }

    // tests/test_screenshot_urls.py:92
    #[test]
    fn stored_list_filters_stale_non_screenshot_lines() {
        let raw = "https://img.example/box-front.jpg\nhttps://img.example/screenshot-gameplay.jpg\n\
                   https://img.example/fanart-background.jpg\nhttps://img.example/title-screen.jpg\n\
                   https://img.example/screenshot-gameplay.jpg\n  \n";
        assert_eq!(
            screenshot_urls_from_stored(raw),
            vec!["https://img.example/screenshot-gameplay.jpg", "https://img.example/title-screen.jpg"]
        );
        assert!(screenshot_urls_from_stored("   ").is_empty());
    }

    #[test]
    fn screenshot_heuristic_is_permissive_by_default() {
        assert!(looks_like_screenshot_url("https://h/assets/1234.png"));
        assert!(looks_like_screenshot_url("https://h/x/screenshot-1.jpg"));
        assert!(looks_like_screenshot_url("https://h/x/a.jpg?kind=title_screen"));
        assert!(!looks_like_screenshot_url("https://h/x/box-front.jpg"));
        assert!(!looks_like_screenshot_url("https://h/x/BoxArt.jpg"));
        assert!(!looks_like_screenshot_url("https://h/x/clear_logo.png"));
        // positive beats negative
        assert!(looks_like_screenshot_url("https://h/x/cover-screenshot.jpg"));
        // "boxes" is not "box" (token bounded by non-alphanumerics)
        assert!(looks_like_screenshot_url("https://h/x/boxes.jpg"));
    }

    // tests/test_screenshot_urls.py:114-150
    #[test]
    fn host_filter_cases() {
        let ext = "https://neoclone.screenscraper.fr/img/123.jpg";
        assert_eq!(filter_to_server_host(ext, "https://my-romm-server"), "");
        assert_eq!(filter_to_server_host(ext, ""), ext);
        assert_eq!(filter_to_server_host(ext, "not-a-url"), ext);
        assert_eq!(
            filter_to_server_host("https://my-romm-server/api/roms/123/cover", "https://my-romm-server"),
            "https://my-romm-server/api/roms/123/cover"
        );
        assert_eq!(
            filter_to_server_host("https://my-romm-server:9090/img/cover.jpg", "https://my-romm-server:8080"),
            ""
        );
        assert_eq!(filter_to_server_host("", "https://my-romm-server"), "");
    }

    #[test]
    fn resolve_relative_and_normalize() {
        assert_eq!(
            resolve_image_url("/api/roms/123/cover", "https://my-romm-server"),
            "https://my-romm-server/api/roms/123/cover"
        );
        assert_eq!(resolve_image_url("api/x.png", "https://h"), "https://h/api/x.png");
        assert_eq!(resolve_image_url("/api/x.png", ""), "");
        assert_eq!(resolve_image_url("   ", "https://h"), "");
        assert_eq!(
            resolve_image_url("/assets/cover art.png", "https://h"),
            "https://h/assets/cover%20art.png"
        );
        // already-encoded stays encoded (% is safe)
        assert_eq!(resolve_image_url("/a%20b.png", "https://h"), "https://h/a%20b.png");
        // query round-trip keeps blank values, encodes spaces as '+'
        assert_eq!(
            resolve_image_url("https://h/x.png?a=1&b=&c=x y#frag", ""),
            "https://h/x.png?a=1&b=&c=x+y#frag"
        );
        // absolute foreign URL untouched by resolve (filter drops it)
        let r = server_resolver("https://h");
        assert_eq!(r("https://other/x.png"), "");
        assert_eq!(r("/x.png"), "https://h/x.png");
    }

    #[test]
    fn cover_key_walk_and_dict_values() {
        let r = server_resolver("https://h");
        let p = json!({"path_cover_small": "/small.png", "path_cover_large": "/large.png"});
        assert_eq!(cover_url_from_payload(&p, &r), "https://h/large.png");
        let p = json!({"url_cover": "https://other/c.png", "path_cover_small": "/small.png"});
        assert_eq!(cover_url_from_payload(&p, &r), "https://h/small.png");
        let p = json!({"cover_image": {"src": "/dict.png"}});
        assert_eq!(cover_url_from_payload(&p, &r), "https://h/dict.png");
        assert_eq!(cover_url_from_payload(&json!({}), &r), "");
    }

    #[test]
    fn extension_for_precedence() {
        let png = b"\x89PNG\r\n\x1a\n....";
        assert_eq!(extension_for("/x", png, "").ext, "png");
        assert!(extension_for("/x", png, "").identified);
        assert_eq!(extension_for("/x", png, "image/jpeg; charset=x").ext, "jpg");
        assert_eq!(extension_for("/x", b"\xff\xd8\xff\xe0", "").ext, "jpg");
        assert_eq!(extension_for("/x", b"GIF89a", "").ext, "gif");
        assert_eq!(extension_for("/x", b"BM....", "").ext, "bmp");
        assert_eq!(extension_for("/x", b"II*\0", "").ext, "tiff");
        assert_eq!(extension_for("/x", b"\0\0\x01\0", "").ext, "ico");
        assert_eq!(extension_for("/x", b"RIFF\0\0\0\0WEBPVP8 ", "").ext, "webp");
        assert_eq!(extension_for("/x", b"RIFF\0\0\0\0WEB", "").ext, "img");
        assert_eq!(extension_for("/x", b"  <svg xmlns", "").ext, "svg");
        assert_eq!(extension_for("/x", b"<?xml version='1'?><SVG>", "").ext, "svg");
        let s = extension_for("https://h/a/b.JPEG?x=1", b"zzzz", "text/html");
        assert_eq!(s.ext, "jpeg");
        assert!(!s.identified);
        assert_eq!(extension_for("https://h/a/b.exe", b"zzzz", "").ext, "img");
        assert_eq!(extension_for("https://h/a/.hidden", b"zzzz", "").ext, "img");
        assert_eq!(extension_for("/x", b"", "image/webp").ext, "webp");
    }

    #[test]
    fn urlsplit_matches_python_shapes() {
        let s = urlsplit("https://host:8080/p/a th?q=1#f");
        assert_eq!(s.scheme, "https");
        assert_eq!(s.netloc, "host:8080");
        assert_eq!(s.path, "/p/a th");
        assert_eq!(s.query, "q=1");
        assert_eq!(s.fragment, "f");
        assert_eq!(urlsplit("not-a-url").netloc, "");
        assert_eq!(urlunsplit(&urlsplit("https://host")), "https://host");
        assert_eq!(urlunsplit(&urlsplit("https://host/x?a=1#f")), "https://host/x?a=1#f");
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run (from `rewrite/`): `cargo test -p grid-core images::urls`
Expected: compile error, module missing.

- [ ] **Step 3: Write `images/mod.rs`**

```rust
//! Image pipeline: URL rules (doc 07 "URL resolution rules"), the disk
//! cache, the startup sweep, and the replenish job. Spec:
//! docs/superpowers/specs/2026-09-02-covers-images-design.md.

pub mod urls;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageVariant {
    CoverSmall,
    CoverLarge,
    Screenshot,
}

/// The three registry image columns, as stored: cover paths verbatim from
/// the server (resolved lazily against the server URL), screenshots as a
/// newline-joined list of already resolved + host-filtered absolute URLs.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct ImageFields {
    pub cover_small_path: String,
    pub cover_large_path: String,
    pub screenshot_urls: String,
}
```

- [ ] **Step 4: Write `images/urls.rs`** (verbatim port of `grid_launcher/cover/utils.py`)

```rust
//! Verbatim port of `grid_launcher/cover/utils.py`: Python `urllib.parse`
//! semantics are reproduced by hand (`urlsplit`, `urlunsplit`, `quote`,
//! `parse_qsl`/`urlencode`) because the `url` crate normalizes differently
//! (lowercases hosts, adds a trailing slash to bare origins).

use percent_encoding::{percent_decode_str, utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use regex::Regex;
use serde_json::Value;
use std::sync::LazyLock;

/// Python `quote(path, safe="/%._-~")`: letters, digits, `_.-~` always safe.
const PATH_SAFE: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'/')
    .remove(b'%')
    .remove(b'.')
    .remove(b'_')
    .remove(b'-')
    .remove(b'~');
/// Python `quote_plus(s, safe="")`.
const QUERY_SAFE: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'.')
    .remove(b'_')
    .remove(b'-')
    .remove(b'~');
const USES_NETLOC: [&str; 6] = ["http", "https", "ftp", "file", "ws", "wss"];

static SCREENSHOT_HINT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(?:^|[^a-z0-9])(?:screenshot|screen[_-]?shot|gameplay|in[_-]?game|title[_-]?screen|titlescreen)(?:[^a-z0-9]|$)",
    )
    .expect("static regex")
});
static NON_SCREENSHOT_ART_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(?:^|[^a-z0-9])(?:box(?:[_-]?art)?|cover(?:[_-]?art)?|fan(?:[_-]?art)?|logo|clear[_-]?logo|clear[_-]?art|banner|poster|marquee|cartridge|disc)(?:[^a-z0-9]|$)",
    )
    .expect("static regex")
});

const LAUNCHBOX_SCREENSHOT_TYPE_TOKENS: [&str; 6] =
    ["screenshot", "title screen", "titlescreen", "gameplay", "in-game", "ingame"];

/// Every extension `extension_for` can return; the cache probes these.
pub const LOOKUP_EXTENSIONS: [&str; 14] = [
    "png", "jpg", "jpeg", "webp", "gif", "bmp", "tif", "tiff", "ico", "svg", "avif", "heic",
    "heif", "img",
];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SplitUrl {
    pub scheme: String,
    pub netloc: String,
    pub path: String,
    pub query: String,
    pub fragment: String,
}

fn is_scheme(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
}

/// Python `urllib.parse.urlsplit`.
pub fn urlsplit(value: &str) -> SplitUrl {
    let (rest, fragment) = match value.split_once('#') {
        Some((a, b)) => (a, b.to_string()),
        None => (value, String::new()),
    };
    let (rest, query) = match rest.split_once('?') {
        Some((a, b)) => (a, b.to_string()),
        None => (rest, String::new()),
    };
    let (scheme, rest) = match rest.split_once(':') {
        Some((s, r)) if is_scheme(s) => (s.to_ascii_lowercase(), r),
        _ => (String::new(), rest),
    };
    let (netloc, path) = match rest.strip_prefix("//") {
        Some(after) => {
            let end = after.find(['/', '?', '#']).unwrap_or(after.len());
            (after[..end].to_string(), after[end..].to_string())
        }
        None => (String::new(), rest.to_string()),
    };
    SplitUrl { scheme, netloc, path, query, fragment }
}

/// Python `urllib.parse.urlunsplit`.
pub fn urlunsplit(s: &SplitUrl) -> String {
    let mut url = s.path.clone();
    if !s.netloc.is_empty()
        || (!s.scheme.is_empty() && USES_NETLOC.contains(&s.scheme.as_str()) && !url.starts_with("//"))
    {
        if !url.is_empty() && !url.starts_with('/') {
            url.insert(0, '/');
        }
        url = format!("//{}{url}", s.netloc);
    }
    if !s.scheme.is_empty() {
        url = format!("{}:{url}", s.scheme);
    }
    if !s.query.is_empty() {
        url = format!("{url}?{}", s.query);
    }
    if !s.fragment.is_empty() {
        url = format!("{url}#{}", s.fragment);
    }
    url
}

fn unquote_plus(s: &str) -> String {
    percent_decode_str(&s.replace('+', " ")).decode_utf8_lossy().into_owned()
}

fn quote_plus(s: &str) -> String {
    utf8_percent_encode(s, QUERY_SAFE).to_string().replace("%20", "+")
}

/// Python `parse_qsl(query, keep_blank_values=True)`.
fn parse_qsl(query: &str) -> Vec<(String, String)> {
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
            (unquote_plus(name), unquote_plus(value))
        })
        .collect()
}

/// Python `urlencode(pairs, doseq=True)` over string pairs.
fn urlencode(pairs: &[(String, String)]) -> String {
    pairs
        .iter()
        .map(|(k, v)| format!("{}={}", quote_plus(k), quote_plus(v)))
        .collect::<Vec<_>>()
        .join("&")
}

/// `_looks_like_screenshot_url` (cover/utils.py:20).
pub fn looks_like_screenshot_url(value: &str) -> bool {
    let parsed = urlsplit(value);
    let haystack = if !parsed.path.is_empty() || !parsed.query.is_empty() {
        format!("{}?{}", parsed.path, parsed.query)
    } else {
        value.to_string()
    };
    if SCREENSHOT_HINT_RE.is_match(&haystack) {
        return true;
    }
    !NON_SCREENSHOT_ART_RE.is_match(&haystack)
}

/// `resolve_cover_url` (cover/utils.py:28).
pub fn resolve_image_url(value: &str, base_url: &str) -> String {
    let candidate = value.trim();
    if candidate.is_empty() {
        return String::new();
    }
    let candidate = if candidate.starts_with("http://") || candidate.starts_with("https://") {
        candidate.to_string()
    } else if base_url.is_empty() {
        return String::new();
    } else if candidate.starts_with('/') {
        format!("{base_url}{candidate}")
    } else {
        format!("{base_url}/{candidate}")
    };
    let split = urlsplit(&candidate);
    urlunsplit(&SplitUrl {
        scheme: split.scheme,
        netloc: split.netloc,
        path: utf8_percent_encode(&split.path, PATH_SAFE).to_string(),
        query: urlencode(&parse_qsl(&split.query)),
        fragment: split.fragment,
    })
}

/// `filter_to_server_host` (cover/utils.py:47). Permissive on an empty
/// `url`/`base_url` or a base with no netloc; whole-netloc comparison.
pub fn filter_to_server_host(url: &str, base_url: &str) -> String {
    if url.is_empty() || base_url.is_empty() {
        return url.to_string();
    }
    let base_netloc = urlsplit(base_url).netloc;
    if base_netloc.is_empty() {
        return url.to_string();
    }
    let candidate_netloc = urlsplit(url).netloc;
    if !candidate_netloc.is_empty() && candidate_netloc != base_netloc {
        return String::new();
    }
    url.to_string()
}

/// The desktop window's composition (grid-launcher.py:2894):
/// `filter_to_server_host(resolve_cover_url(value, base), base)`.
pub fn server_resolver(base_url: &str) -> impl Fn(&str) -> String {
    let base = base_url.to_string();
    move |value: &str| filter_to_server_host(&resolve_image_url(value, &base), &base)
}

fn resolve_cover_value(value: &Value, resolver: &dyn Fn(&str) -> String) -> String {
    match value {
        Value::String(s) => resolver(s),
        Value::Object(map) => {
            for key in ["url", "path", "image", "src", "download_path", "file_path", "full_path"] {
                if let Some(Value::String(candidate)) = map.get(key) {
                    let resolved = resolver(candidate);
                    if !resolved.is_empty() {
                        return resolved;
                    }
                }
            }
            String::new()
        }
        _ => String::new(),
    }
}

/// `cover_url_from_rom_payload` (cover/utils.py:63).
pub fn cover_url_from_payload(payload: &Value, resolver: &dyn Fn(&str) -> String) -> String {
    for key in [
        "url_cover", "path_cover_large", "path_cover_small", "cover_url", "cover_image",
        "cover_path", "image_url",
    ] {
        if let Some(value) = payload.get(key) {
            let resolved = resolve_cover_value(value, resolver);
            if !resolved.is_empty() {
                return resolved;
            }
        }
    }
    String::new()
}

fn is_launchbox_screenshot_type(image_type: &str) -> bool {
    let normalized = image_type.trim().to_lowercase();
    LAUNCHBOX_SCREENSHOT_TYPE_TOKENS.iter().any(|t| normalized.contains(t))
}

/// `screenshot_urls_from_rom_payload` (cover/utils.py:93), source order and
/// the per-append de-dup preserved exactly.
pub fn screenshot_urls_from_payload(payload: &Value, resolver: &dyn Fn(&str) -> String) -> Vec<String> {
    let mut urls: Vec<String> = Vec::new();

    fn append_url(urls: &mut Vec<String>, value: Option<&Value>, resolver: &dyn Fn(&str) -> String) {
        match value {
            Some(Value::String(s)) => {
                let resolved = resolver(s);
                if !resolved.is_empty() && !urls.contains(&resolved) {
                    urls.push(resolved);
                }
            }
            Some(Value::Object(map)) => {
                for key in ["url", "path", "image", "src"] {
                    if let Some(Value::String(candidate)) = map.get(key) {
                        let resolved = resolver(candidate);
                        if !resolved.is_empty() && !urls.contains(&resolved) {
                            urls.push(resolved);
                            return;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    if let Some(Value::Array(items)) = payload.get("merged_screenshots") {
        for item in items {
            append_url(&mut urls, Some(item), resolver);
        }
    }
    if let Some(Value::Array(items)) = payload.get("user_screenshots") {
        for item in items {
            let Value::Object(map) = item else { continue };
            for key in ["download_path", "file_path", "full_path"] {
                append_url(&mut urls, map.get(key), resolver);
            }
        }
    }
    for block in ["gamelist_metadata", "ss_metadata"] {
        if let Some(Value::Object(map)) = payload.get(block) {
            for key in ["screenshot_url", "title_screen_url"] {
                append_url(&mut urls, map.get(key), resolver);
            }
        }
    }
    if let Some(Value::Object(launchbox)) = payload.get("launchbox_metadata") {
        if let Some(Value::Array(images)) = launchbox.get("images") {
            for image in images {
                let Value::Object(map) = image else { continue };
                let Some(Value::String(image_type)) = map.get("type") else { continue };
                if is_launchbox_screenshot_type(image_type) {
                    append_url(&mut urls, map.get("url"), resolver);
                }
            }
        }
    }
    for key in ["url_screenshots", "path_screenshots", "screenshots", "images"] {
        match payload.get(key) {
            Some(Value::Array(items)) if key == "images" => {
                for item in items {
                    let Value::Object(map) = item else {
                        append_url(&mut urls, Some(item), resolver);
                        continue;
                    };
                    if let Some(Value::String(image_type)) = map.get("type") {
                        if is_launchbox_screenshot_type(image_type) {
                            append_url(&mut urls, Some(item), resolver);
                        }
                        continue;
                    }
                    append_url(&mut urls, Some(item), resolver);
                }
            }
            Some(Value::Array(items)) => {
                for item in items {
                    append_url(&mut urls, Some(item), resolver);
                }
            }
            other => append_url(&mut urls, other, resolver),
        }
    }
    for key in ["url_screenshot", "path_screenshot"] {
        append_url(&mut urls, payload.get(key), resolver);
    }

    urls.into_iter().filter(|u| looks_like_screenshot_url(u)).collect()
}

/// `screenshot_urls_from_game` (cover/utils.py:183): re-filter on read.
pub fn screenshot_urls_from_stored(raw: &str) -> Vec<String> {
    if raw.trim().is_empty() {
        return Vec::new();
    }
    let mut unique: Vec<String> = Vec::new();
    for line in raw.lines() {
        let value = line.trim();
        if !value.is_empty() && looks_like_screenshot_url(value) && !unique.iter().any(|u| u == value) {
            unique.push(value.to_string());
        }
    }
    unique
}

/// Result of `extension_for`. `identified` is true when Content-Type,
/// magic bytes or the SVG sniff recognized an image (the content gate);
/// false when only the URL suffix or the `img` fallback chose it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sniff {
    pub ext: String,
    pub identified: bool,
}

fn sniff(ext: &str, identified: bool) -> Sniff {
    Sniff { ext: ext.to_string(), identified }
}

/// `cover_cache_extension_from_payload` (cover/utils.py:220), without the
/// leading dot. D11: the SVG `<?xml` branch lowercases (Python's
/// `bytes.casefold()` does not exist).
pub fn extension_for(url: &str, body: &[u8], content_type: &str) -> Sniff {
    let normalized = content_type.trim().to_lowercase();
    let normalized = normalized.split(';').next().unwrap_or("");
    let mapped = match normalized {
        "image/jpeg" | "image/jpg" => Some("jpg"),
        "image/png" => Some("png"),
        "image/webp" => Some("webp"),
        "image/gif" => Some("gif"),
        "image/bmp" | "image/x-ms-bmp" => Some("bmp"),
        "image/tiff" => Some("tiff"),
        "image/x-icon" | "image/vnd.microsoft.icon" => Some("ico"),
        "image/svg+xml" => Some("svg"),
        _ => None,
    };
    if let Some(ext) = mapped {
        return sniff(ext, true);
    }
    if body.starts_with(b"\x89PNG\r\n\x1a\n") {
        return sniff("png", true);
    }
    if body.starts_with(b"\xff\xd8\xff") {
        return sniff("jpg", true);
    }
    if body.starts_with(b"GIF87a") || body.starts_with(b"GIF89a") {
        return sniff("gif", true);
    }
    if body.starts_with(b"BM") {
        return sniff("bmp", true);
    }
    if body.starts_with(b"II*\0") || body.starts_with(b"MM\0*") {
        return sniff("tiff", true);
    }
    if body.starts_with(b"\0\0\x01\0") {
        return sniff("ico", true);
    }
    if body.len() >= 12 && body.starts_with(b"RIFF") && &body[8..12] == b"WEBP" {
        return sniff("webp", true);
    }
    let preview = &body[..body.len().min(256)];
    let start = preview.iter().position(|b| !b.is_ascii_whitespace()).unwrap_or(preview.len());
    let preview = &preview[start..];
    let lowered = preview.to_ascii_lowercase();
    if preview.starts_with(b"<svg")
        || (preview.starts_with(b"<?xml") && lowered.windows(4).any(|w| w == b"<svg"))
    {
        return sniff("svg", true);
    }
    let path = urlsplit(url).path;
    let last = path.rsplit('/').next().unwrap_or("");
    if let Some(idx) = last.rfind('.') {
        if idx > 0 && idx + 1 < last.len() {
            let suffix = last[idx + 1..].to_lowercase();
            if [
                "jpg", "jpeg", "png", "webp", "gif", "bmp", "tif", "tiff", "ico", "svg", "avif",
                "heic", "heif",
            ]
            .contains(&suffix.as_str())
            {
                return sniff(&suffix, false);
            }
        }
    }
    sniff("img", false)
}
```

Add `pub mod images;` to `lib.rs` (keep `pub mod covers;` until Task 2).

- [ ] **Step 5: Run tests and lints**

Run: `cargo test -p grid-core images::urls && cargo clippy -p grid-core --all-targets -- -D warnings && cargo fmt --check`
Expected: all pass. If `LazyLock` is unavailable on the toolchain, use `std::sync::OnceLock` with a `fn` accessor instead.

- [ ] **Step 6: Commit**

```bash
git add rewrite/crates/grid-core/src/images rewrite/crates/grid-core/src/lib.rs
git commit -m "rewrite: images::urls — URL rules, screenshot extraction, extension sniff"
```

---

### Task 2: `images::cache` replaces `covers.rs`; `ensure_image` command

**Files:**
- Create: `rewrite/crates/grid-core/src/images/cache.rs`
- Delete: `rewrite/crates/grid-core/src/covers.rs`, `rewrite/crates/grid-core/tests/covers.rs`
- Create: `rewrite/crates/grid-core/tests/images_cache.rs`
- Modify: `rewrite/crates/grid-core/src/images/mod.rs` (`pub mod cache;`), `src/lib.rs` (drop `covers`), `src/romm/mod.rs` (`get_bytes_with_type`), `src/session.rs` (ImageCache + `server_url`)
- Modify: `rewrite/app/src-tauri/src/commands.rs` (`ensure_image`), `lib.rs` (registration), `rewrite/app/src/lib/api.ts`, `rewrite/app/src/lib/Cover.svelte`

**Interfaces:**
- Consumes: Task 1 `extension_for`, `resolve_image_url`, `filter_to_server_host`, `LOOKUP_EXTENSIONS`.
- Produces:
  - `pub fn image_key(url: &str) -> String`
  - `pub enum ImageError { Offline, NotAnImage, Http(RommError), Io(String) }` (Clone, Display)
  - `pub struct ImageCache` with `new(dir)`, `dir()`, `find_existing(&self, key) -> Option<PathBuf>`, `cached_path(&self, url) -> Option<PathBuf>`, `async fn ensure(&self, client: Option<&RommClient>, url: &str) -> Result<PathBuf, ImageError>`
  - `pub const MAX_CONCURRENT_DOWNLOADS: usize = 6`
  - `RommClient::get_bytes_with_type(&self, path_or_url: &str) -> Result<(Vec<u8>, String), RommError>` — accepts `/relative` or absolute `http(s)://`
  - `SessionManager::server_url(&self) -> String` (stored server URL, set on connect/restore even when the probe fails)
  - Tauri `ensure_image(url: String) -> Result<String, String>`; errors `"filtered"` / `"not connected"` / `ImageError` text
  - `api.ensureImage(url: string): Promise<string>`

- [ ] **Step 1: Write the failing integration tests** `tests/images_cache.rs` (port the three tests from `tests/covers.rs` to the new API — `ensure(Some(&client), &format!("{}/assets/cover.png", server.uri()))` — then add):

```rust
#[tokio::test]
async fn offline_miss_is_not_recorded_as_failure() {
    let server = MockServer::start().await;
    let mock = Mock::given(method("GET")).and(path("/assets/c.png"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(PNG_MAGIC)).expect(1)
        .mount_as_scoped(&server).await;
    let dir = tempfile::tempdir().unwrap();
    let cache = ImageCache::new(dir.path().to_path_buf());
    let url = format!("{}/assets/c.png", server.uri());
    let offline = cache.ensure(None, &url).await;
    assert!(matches!(offline, Err(ImageError::Offline)));
    let client = client_for(&server);
    let got = cache.ensure(Some(&client), &url).await.unwrap();
    assert!(got.exists());
    assert_eq!(cache.ensure(None, &url).await.unwrap(), got); // offline hit
    drop(mock);
}

#[tokio::test]
async fn content_gate_rejects_non_images_and_writes_nothing() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/assets/login"))
        .respond_with(ResponseTemplate::new(200).set_body_string("<html>login</html>")
            .insert_header("content-type", "text/html"))
        .mount(&server).await;
    let dir = tempfile::tempdir().unwrap();
    let cache = ImageCache::new(dir.path().to_path_buf());
    let client = client_for(&server);
    let err = cache.ensure(Some(&client), &format!("{}/assets/login", server.uri())).await.unwrap_err();
    assert!(matches!(err, ImageError::NotAnImage));
    assert_eq!(std::fs::read_dir(dir.path()).map(|d| d.count()).unwrap_or(0), 0);
}

#[tokio::test]
async fn image_content_type_alone_is_accepted() {
    // An `image/*` body the sniffers don't recognize is still written (suffix rule picks the ext).
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/assets/x.avif"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"not-a-known-magic".to_vec())
            .insert_header("content-type", "image/avif"))
        .mount(&server).await;
    let dir = tempfile::tempdir().unwrap();
    let cache = ImageCache::new(dir.path().to_path_buf());
    let client = client_for(&server);
    let p = cache.ensure(Some(&client), &format!("{}/assets/x.avif", server.uri())).await.unwrap();
    assert_eq!(p.extension().unwrap(), "avif");
}

#[tokio::test]
async fn cache_hit_refreshes_mtime() {
    let dir = tempfile::tempdir().unwrap();
    let cache = ImageCache::new(dir.path().to_path_buf());
    let url = "https://h/assets/old.png";
    let file = dir.path().join(format!("{}.png", image_key(url)));
    std::fs::write(&file, PNG_MAGIC).unwrap();
    let old = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
    std::fs::File::options().write(true).open(&file).unwrap().set_modified(old).unwrap();
    cache.ensure(None, url).await.unwrap();
    let mtime = std::fs::metadata(&file).unwrap().modified().unwrap();
    assert!(mtime > old + std::time::Duration::from_secs(1800));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn downloads_are_limited_to_six_in_flight() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(PNG_MAGIC).set_delay(Duration::from_millis(300)))
        .mount(&server).await;
    let dir = tempfile::tempdir().unwrap();
    let cache = Arc::new(ImageCache::new(dir.path().to_path_buf()));
    let client = Arc::new(client_for(&server));
    let started = std::time::Instant::now();
    let tasks: Vec<_> = (0..12).map(|i| {
        let cache = cache.clone(); let client = client.clone(); let base = server.uri();
        tokio::spawn(async move { cache.ensure(Some(&client), &format!("{base}/assets/{i}.png")).await })
    }).collect();
    for t in futures::future::join_all(tasks).await { t.unwrap().unwrap(); }
    // 12 fetches at 300 ms each through 6 permits: two waves, so >= ~600 ms.
    assert!(started.elapsed() >= Duration::from_millis(550), "elapsed {:?}", started.elapsed());
}
```

with `fn client_for(server: &MockServer) -> RommClient` building a token client as the old tests did.

- [ ] **Step 2: Run to verify it fails** — `cargo test -p grid-core --test images_cache` → compile error.

- [ ] **Step 3: Add `get_bytes_with_type` to `RommClient`** (romm/mod.rs). Refactor `get_bytes` to call it:

```rust
    fn target(&self, path_or_url: &str) -> Result<url::Url, RommError> {
        if path_or_url.starts_with("http://") || path_or_url.starts_with("https://") {
            url::Url::parse(path_or_url).map_err(|_| RommError::InvalidUrl)
        } else {
            self.endpoint(path_or_url)
        }
    }

    /// Bytes plus the response Content-Type (empty when absent). Accepts a
    /// server-relative `/path` or an absolute same-host URL (host filtering
    /// happens before this is called — see images::urls). Same 401/403 →
    /// Unauthorized mapping as `get_bytes`.
    pub async fn get_bytes_with_type(&self, path_or_url: &str) -> Result<(Vec<u8>, String), RommError> {
        let resp = self.http.get(self.target(path_or_url)?)
            .header(reqwest::header::AUTHORIZATION, self.auth.clone())
            .send().await
            .map_err(|e| RommError::Connection(e.without_url().to_string()))?;
        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(RommError::Unauthorized);
        }
        if !status.is_success() {
            return Err(RommError::Http { status: status.as_u16(), excerpt: String::new() });
        }
        let content_type = resp.headers().get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
        let bytes = resp.bytes().await
            .map_err(|e| RommError::Connection(e.without_url().to_string()))?.to_vec();
        Ok((bytes, content_type))
    }

    pub async fn get_bytes(&self, path: &str) -> Result<Vec<u8>, RommError> {
        self.get_bytes_with_type(path).await.map(|(b, _)| b)
    }
```

Keep the "Task 11 fix" comment on the new function.

- [ ] **Step 4: Write `images/cache.rs`**

```rust
//! Disk cache for covers and screenshots (spec "Cache"). One filename
//! scheme (D1): `<sha256(resolved url)>.<ext>`. In-flight dedup, a
//! per-session negative map, a download semaphore (D4) and the content
//! gate (D8).

use super::urls::{extension_for, LOOKUP_EXTENSIONS};
use crate::romm::{RommClient, RommError};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::{Mutex, Notify, Semaphore};

pub const MAX_CONCURRENT_DOWNLOADS: usize = 6;

#[derive(Debug, Clone, thiserror::Error)]
pub enum ImageError {
    #[error("not connected")]
    Offline,
    #[error("the server did not return an image")]
    NotAnImage,
    #[error(transparent)]
    Http(#[from] RommError),
    #[error("file error: {0}")]
    Io(String),
}

/// Lowercase hex SHA-256 of the resolved absolute URL.
pub fn image_key(url: &str) -> String {
    Sha256::digest(url.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

pub struct ImageCache {
    dir: PathBuf,
    downloads: Semaphore,
    in_flight: Mutex<HashMap<String, Arc<Notify>>>,
    /// Session-only negative cache: a URL that failed once replays its error.
    failed: Mutex<HashMap<String, ImageError>>,
}

impl ImageCache {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            downloads: Semaphore::new(MAX_CONCURRENT_DOWNLOADS),
            in_flight: Mutex::new(HashMap::new()),
            failed: Mutex::new(HashMap::new()),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn find_existing(&self, key: &str) -> Option<PathBuf> {
        LOOKUP_EXTENSIONS
            .iter()
            .map(|ext| self.dir.join(format!("{key}.{ext}")))
            .find(|p| p.is_file())
    }

    /// The cached file for `url`, refreshing its mtime so the sweep treats
    /// it as recently used. No network.
    pub fn cached_path(&self, url: &str) -> Option<PathBuf> {
        let path = self.find_existing(&image_key(url))?;
        // Best effort: a read-only cache dir must not turn a hit into a miss.
        if let Ok(f) = std::fs::File::options().write(true).open(&path) {
            let _ = f.set_modified(SystemTime::now());
        }
        Some(path)
    }

    /// Cache hit → path. Miss with no client → `Offline` (not recorded).
    /// Miss with a client → fetch (deduplicated per URL, at most
    /// `MAX_CONCURRENT_DOWNLOADS` in flight), gate, write atomically.
    pub async fn ensure(&self, client: Option<&RommClient>, url: &str) -> Result<PathBuf, ImageError> {
        let key = image_key(url);
        loop {
            if let Some(e) = self.failed.lock().await.get(&key) {
                return Err(e.clone());
            }
            if let Some(p) = self.cached_path(url) {
                return Ok(p);
            }
            let Some(client) = client else {
                return Err(ImageError::Offline);
            };

            let mut map = self.in_flight.lock().await;
            if let Some(existing) = map.get(&key).cloned() {
                // Register interest while still holding the map lock: the
                // owner takes this same lock before notify_waiters(), so
                // enable() happens-before any notification and no wakeup
                // is lost (regression-tested by
                // ensure_dedups_concurrent_callers_for_same_url).
                let notified = existing.notified();
                tokio::pin!(notified);
                notified.as_mut().enable();
                drop(map);
                notified.await;
                continue;
            }
            map.insert(key.clone(), Arc::new(Notify::new()));
            drop(map);

            let result = self.fetch_and_store(client, &key, url).await;
            if let Err(e) = &result {
                self.failed.lock().await.insert(key.clone(), e.clone());
            }
            if let Some(n) = self.in_flight.lock().await.remove(&key) {
                n.notify_waiters();
            }
            return result;
        }
    }

    async fn fetch_and_store(&self, client: &RommClient, key: &str, url: &str) -> Result<PathBuf, ImageError> {
        let (bytes, content_type) = {
            let _permit = self.downloads.acquire().await.expect("image semaphore is never closed");
            client.get_bytes_with_type(url).await?
        };
        if bytes.is_empty() {
            return Err(ImageError::NotAnImage);
        }
        let sniff = extension_for(url, &bytes, &content_type);
        if !sniff.identified && !content_type.trim().to_ascii_lowercase().starts_with("image/") {
            return Err(ImageError::NotAnImage);
        }
        let io = |e: std::io::Error| ImageError::Io(e.to_string());
        std::fs::create_dir_all(&self.dir).map_err(io)?;
        let target = self.dir.join(format!("{key}.{}", sniff.ext));
        let tmp = self.dir.join(format!("{key}.part"));
        std::fs::write(&tmp, &bytes).map_err(io)?;
        std::fs::rename(&tmp, &target).map_err(io)?;
        Ok(target)
    }
}
```

- [ ] **Step 5: Rewire `session.rs`**: replace `CoverCache` with `ImageCache`; add `server_url: Mutex<String>` set in `connect` (after persistence) and at the top of `restore` (right after `cfg.server_url` is known non-empty, before the probe); add `pub fn server_url(&self) -> String`. Delete `covers.rs`, remove `pub mod covers;`, add `pub mod cache;` to `images/mod.rs`.

- [ ] **Step 6: Replace the command** in `commands.rs`:

```rust
#[tauri::command]
pub async fn ensure_image(state: State<'_, AppState>, url: String) -> Result<String, String> {
    let base = state.session.server_url();
    let resolved = filter_to_server_host(&resolve_image_url(&url, &base), &base);
    if resolved.is_empty() {
        return Err("filtered".to_string());
    }
    let client = state.session.client();
    let path = state
        .session
        .cache()
        .ensure(client.as_deref(), &resolved)
        .await
        .map_err(err)?;
    Ok(path.to_string_lossy().into_owned())
}
```

Register `commands::ensure_image` in place of `ensure_cover`. In `api.ts` replace `ensureCover` with `ensureImage: (url: string) => invoke<string>('ensure_image', { url })`. In `Cover.svelte` call `api.ensureImage(game.path_cover_small)`.

- [ ] **Step 7: Run everything**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --check && bash scripts/check_secret_hygiene.sh && (cd app && npm run check && npm test)`
Expected: green. `tests/session.rs` still compiles (no API change yet).

- [ ] **Step 8: Commit**

```bash
git add -A rewrite/crates/grid-core/src/images rewrite/crates/grid-core/src/covers.rs rewrite/crates/grid-core/tests/covers.rs rewrite/crates/grid-core/tests/images_cache.rs rewrite/crates/grid-core/src/lib.rs rewrite/crates/grid-core/src/romm/mod.rs rewrite/crates/grid-core/src/session.rs rewrite/app/src-tauri/src/commands.rs rewrite/app/src-tauri/src/lib.rs rewrite/app/src/lib/api.ts rewrite/app/src/lib/Cover.svelte
git commit -m "rewrite: images::cache replaces covers (URL-hash key, semaphore, content gate); ensure_image"
```

(`git add -A <paths>` stages the deletions of the two removed files.)

---

### Task 3: `images::sweep` — pinning and the bounded cache

**Files:**
- Create: `rewrite/crates/grid-core/src/images/sweep.rs`, `rewrite/crates/grid-core/tests/images_sweep.rs`
- Modify: `images/mod.rs` (`pub mod sweep;`)

**Interfaces:**
- Consumes: `image_key`, `resolve_image_url`, `filter_to_server_host`, `InstalledGame` (fields `cover_small_path`, `cover_large_path` arrive in Task 4 — see note).
- Produces:
  - `pub const IMAGE_CACHE_CAP_BYTES: u64`, `pub const STALE_PART_AGE: Duration`
  - `pub fn pinned_keys<'a>(cover_paths: impl IntoIterator<Item = &'a str>, base_url: &str) -> HashSet<String>` — takes cover path strings, so this task does not depend on Task 4's struct fields
  - `pub struct SweepReport { total_before: u64, total_after: u64, deleted: usize, stale_parts: usize }`
  - `pub fn sweep(dir: &Path, cap_bytes: u64, pinned: &HashSet<String>) -> SweepReport`

- [ ] **Step 1: Write failing tests** `tests/images_sweep.rs`:

```rust
use grid_core::images::cache::image_key;
use grid_core::images::sweep::{pinned_keys, sweep, SweepReport};
use std::collections::HashSet;
use std::fs;
use std::time::{Duration, SystemTime};

fn write(dir: &std::path::Path, name: &str, size: usize, age_secs: u64) {
    let p = dir.join(name);
    fs::write(&p, vec![0u8; size]).unwrap();
    let t = SystemTime::now() - Duration::from_secs(age_secs);
    fs::File::options().write(true).open(&p).unwrap().set_modified(t).unwrap();
}

#[test]
fn under_cap_deletes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a.png", 100, 10);
    write(dir.path(), "b.jpg", 100, 20);
    let r = sweep(dir.path(), 1000, &HashSet::new());
    assert_eq!(r, SweepReport { total_before: 200, total_after: 200, deleted: 0, stale_parts: 0 });
}

#[test]
fn over_cap_deletes_oldest_unpinned_until_under_cap() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "old.png", 100, 300);
    write(dir.path(), "mid.png", 100, 200);
    write(dir.path(), "new.png", 100, 100);
    let r = sweep(dir.path(), 150, &HashSet::new());
    assert_eq!(r.deleted, 2);
    assert_eq!(r.total_after, 100);
    assert!(!dir.path().join("old.png").exists());
    assert!(!dir.path().join("mid.png").exists());
    assert!(dir.path().join("new.png").exists());
}

#[test]
fn pinned_files_survive_even_above_cap() {
    let dir = tempfile::tempdir().unwrap();
    let key = image_key("https://h/assets/pinned.png");
    write(dir.path(), &format!("{key}.png"), 100, 300);
    write(dir.path(), "loose.png", 100, 100);
    let pinned = pinned_keys(["/assets/pinned.png"], "https://h");
    let r = sweep(dir.path(), 50, &pinned);
    assert_eq!(r.deleted, 1);
    assert!(dir.path().join(format!("{key}.png")).exists());
    assert!(!dir.path().join("loose.png").exists());
}

#[test]
fn stale_part_files_are_removed_and_fresh_ones_kept() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "stale.part", 10, 7200);
    write(dir.path(), "fresh.part", 10, 10);
    let r = sweep(dir.path(), 1000, &HashSet::new());
    assert_eq!(r.stale_parts, 1);
    assert!(!dir.path().join("stale.part").exists());
    assert!(dir.path().join("fresh.part").exists());
    assert_eq!(r.total_before, 0); // .part files never count toward the total
}

#[test]
fn pinned_keys_skips_empty_and_foreign_hosts() {
    let pinned = pinned_keys(["", "/a.png", "https://other/b.png"], "https://h");
    assert_eq!(pinned.len(), 1);
    assert!(pinned.contains(&image_key("https://h/a.png")));
}

#[test]
fn missing_dir_is_a_noop() {
    let dir = tempfile::tempdir().unwrap();
    let r = sweep(&dir.path().join("nope"), 10, &HashSet::new());
    assert_eq!(r, SweepReport::default());
}
```

- [ ] **Step 2: Run** `cargo test -p grid-core --test images_sweep` → compile error.

- [ ] **Step 3: Write `sweep.rs`**

```rust
//! Bounded cache (D3): a startup sweep deletes the least-recently-modified
//! unpinned files until the directory is under the cap. Installed rows'
//! covers are pinned; screenshots never are.

use super::cache::image_key;
use super::urls::{filter_to_server_host, resolve_image_url};
use std::collections::HashSet;
use std::path::Path;
use std::time::{Duration, SystemTime};

pub const IMAGE_CACHE_CAP_BYTES: u64 = 512 * 1024 * 1024;
pub const STALE_PART_AGE: Duration = Duration::from_secs(3600);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SweepReport {
    pub total_before: u64,
    pub total_after: u64,
    pub deleted: usize,
    pub stale_parts: usize,
}

/// Keys of every non-empty cover path that resolves to the server host.
pub fn pinned_keys<'a>(cover_paths: impl IntoIterator<Item = &'a str>, base_url: &str) -> HashSet<String> {
    cover_paths
        .into_iter()
        .filter(|p| !p.trim().is_empty())
        .map(|p| filter_to_server_host(&resolve_image_url(p, base_url), base_url))
        .filter(|u| !u.is_empty())
        .map(|u| image_key(&u))
        .collect()
}

struct Entry {
    path: std::path::PathBuf,
    size: u64,
    mtime: SystemTime,
    pinned: bool,
}

pub fn sweep(dir: &Path, cap_bytes: u64, pinned: &HashSet<String>) -> SweepReport {
    let mut report = SweepReport::default();
    let Ok(read) = std::fs::read_dir(dir) else {
        return report;
    };
    let now = SystemTime::now();
    let mut entries = Vec::new();
    for entry in read.flatten() {
        let path = entry.path();
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        let mtime = meta.modified().unwrap_or(now);
        if path.extension().is_some_and(|e| e == "part") {
            if now.duration_since(mtime).unwrap_or_default() > STALE_PART_AGE
                && std::fs::remove_file(&path).is_ok()
            {
                report.stale_parts += 1;
            }
            continue;
        }
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        entries.push(Entry { pinned: pinned.contains(stem), path, size: meta.len(), mtime });
    }
    report.total_before = entries.iter().map(|e| e.size).sum();
    report.total_after = report.total_before;
    if report.total_before <= cap_bytes {
        return report;
    }
    let mut victims: Vec<&Entry> = entries.iter().filter(|e| !e.pinned).collect();
    victims.sort_by_key(|e| e.mtime);
    for victim in victims {
        if report.total_after <= cap_bytes {
            break;
        }
        match std::fs::remove_file(&victim.path) {
            Ok(()) => {
                report.total_after -= victim.size;
                report.deleted += 1;
            }
            Err(e) => tracing::debug!("image sweep: could not delete {}: {e}", victim.path.display()),
        }
    }
    report
}
```

If `tracing` is not a grid-core dependency, drop the `debug!` and ignore the error silently (check `Cargo.toml`; do not add a dependency for one log line).

- [ ] **Step 4: Run** `cargo test -p grid-core --test images_sweep && cargo clippy -p grid-core --all-targets -- -D warnings && cargo fmt --check` → green.

- [ ] **Step 5: Commit**

```bash
git add rewrite/crates/grid-core/src/images/sweep.rs rewrite/crates/grid-core/src/images/mod.rs rewrite/crates/grid-core/tests/images_sweep.rs
git commit -m "rewrite: images::sweep — pinned keys and the bounded cache sweep"
```

---

### Task 4: Registry v2, RomDetail/GameSummary image fields, install fills them

**Files:**
- Modify: `rewrite/crates/grid-core/src/library/registry.rs`, `src/library/mod.rs` (`new_record`, `registry()`), `src/romm/mod.rs` (RomDetail, RawRomDetail, GameSummary, RawGameSummary), `src/images/mod.rs` (`ImageFields::from_detail`)
- Modify tests: `tests/registry.rs`, `tests/romm_detail.rs`, `tests/romm_catalog.rs` (if it asserts on GameSummary shape)
- Modify: `rewrite/app/src/lib/api.ts`, `details/cloud.ts` (`syntheticCloudGame`), `stores/installed.test.ts`, `details/cloud.test.ts` (any `InstalledGame`/`GameSummary` literal)

**Interfaces:**
- Produces:
  - `InstalledGame { …, cover_small_path: String, cover_large_path: String, screenshot_urls: String }`
  - `Registry::update_images(&self, rom_id: i64, fields: &ImageFields) -> Result<bool, LibraryError>`
  - `InstallService::registry(&self) -> Arc<Registry>`
  - `RomDetail { …, cover_small_path: String, cover_large_path: String, screenshot_urls: Vec<String> }` + `#[derive(serde::Serialize)]`
  - `GameSummary { …, #[serde(rename = "path_cover_large")] cover_large_path: Option<String> }`
  - `ImageFields::from_detail(&RomDetail) -> ImageFields`
  - TS: `InstalledGame` + 3 fields; `GameSummary.path_cover_large: string | null`

- [ ] **Step 1: Failing tests.** In `tests/registry.rs`: rename `open_creates_file_and_sets_user_version_1` → `…_2` asserting `2`; extend `sample()` with the three fields (`"/assets/s.png"`, `"/assets/l.png"`, `"https://h/a.png\nhttps://h/b.png"`); add:

```rust
const V1_SCHEMA: &str = "CREATE TABLE installed_games (
    id INTEGER PRIMARY KEY, title TEXT NOT NULL, platform TEXT NOT NULL,
    title_key TEXT NOT NULL, platform_key TEXT NOT NULL, rom_id INTEGER,
    rom_file_name TEXT NOT NULL DEFAULT '', archive_path TEXT NOT NULL DEFAULT '',
    extracted_path TEXT NOT NULL DEFAULT '', extracted_dir TEXT NOT NULL DEFAULT '',
    multi_file_game_dir TEXT NOT NULL DEFAULT '', description TEXT NOT NULL DEFAULT '',
    rating TEXT NOT NULL DEFAULT '', genres TEXT NOT NULL DEFAULT '', regions TEXT NOT NULL DEFAULT '',
    languages TEXT NOT NULL DEFAULT '', tags TEXT NOT NULL DEFAULT '', revision TEXT NOT NULL DEFAULT '',
    companies TEXT NOT NULL DEFAULT '', first_release_date TEXT NOT NULL DEFAULT '',
    filesize_bytes INTEGER NOT NULL DEFAULT 0, server_updated_at TEXT NOT NULL DEFAULT '',
    installed_at INTEGER NOT NULL, UNIQUE (title_key, platform_key));";

#[test]
fn open_migrates_a_v1_database_and_update_images_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("grid-launcher.db");
    {
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(V1_SCHEMA).unwrap();
        conn.execute("INSERT INTO installed_games (title, platform, title_key, platform_key, rom_id, installed_at)
                      VALUES ('Old', 'SNES', 'old', 'snes', 7, 1)", []).unwrap();
        conn.pragma_update(None, "user_version", 1).unwrap();
    }
    let registry = Registry::open(&path).unwrap();
    let conn = Connection::open(&path).unwrap();
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
    assert_eq!(version, 2);
    let rows = registry.all().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].cover_small_path, "");
    assert_eq!(rows[0].screenshot_urls, "");

    let fields = ImageFields {
        cover_small_path: "/s.png".into(),
        cover_large_path: "/l.png".into(),
        screenshot_urls: "https://h/x.png".into(),
    };
    assert!(registry.update_images(7, &fields).unwrap());
    assert!(!registry.update_images(999, &fields).unwrap());
    let row = &registry.all().unwrap()[0];
    assert_eq!(row.cover_small_path, "/s.png");
    assert_eq!(row.cover_large_path, "/l.png");
    assert_eq!(row.screenshot_urls, "https://h/x.png");
}

#[test]
fn open_refuses_a_newer_database() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("grid-launcher.db");
    { Connection::open(&path).unwrap().pragma_update(None, "user_version", 99).unwrap(); }
    assert!(Registry::open(&path).is_err());
}
```

In `tests/romm_detail.rs` add a test that a payload with `"path_cover_small": "/assets/s.png"`, `"path_cover_large": "/assets/l.png"`, `"merged_screenshots": ["/assets/roms/1/screenshots/a.png", "https://other/b.png"]`, `"launchbox_metadata": {"images": [{"type": "Box - Front", "url": "/box.png"}]}` yields `cover_small_path == "/assets/s.png"`, `cover_large_path == "/assets/l.png"`, and `screenshot_urls == vec![format!("{}/assets/roms/1/screenshots/a.png", server.uri())]` (foreign host dropped, box art dropped). Also a test that a payload without any of these fields decodes with empty values.

- [ ] **Step 2: Run** `cargo test -p grid-core --test registry --test romm_detail` → fails.

- [ ] **Step 3: Registry migration** (registry.rs):

```rust
const LATEST_USER_VERSION: i64 = 2;

/// v1 → v2 (milestone 7): the three image columns.
const MIGRATE_1_TO_2_SQL: &str = "
ALTER TABLE installed_games ADD COLUMN cover_small_path TEXT NOT NULL DEFAULT '';
ALTER TABLE installed_games ADD COLUMN cover_large_path TEXT NOT NULL DEFAULT '';
ALTER TABLE installed_games ADD COLUMN screenshot_urls  TEXT NOT NULL DEFAULT '';
";
```

Add the three columns to `SCHEMA_SQL` (before `installed_at`'s UNIQUE line, after `server_updated_at`), to `SELECT_COLUMNS` (after `installed_at`: `…, installed_at, cover_small_path, cover_large_path, screenshot_urls`), to `InstalledGame` and `from_row` (indices 20, 21, 22), and to `upsert` (`?23, ?24, ?25` + three `excluded.` lines + params). Replace the `match version` in `open` with:

```rust
        let mut version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0)).map_err(registry_err)?;
        if version > LATEST_USER_VERSION {
            return Err(LibraryError::Registry(format!(
                "this database (user_version {version}) is from a newer app version; update the app to open it"
            )));
        }
        if version == 0 {
            conn.execute_batch(SCHEMA_SQL).map_err(registry_err)?;
            version = LATEST_USER_VERSION;
        }
        while version < LATEST_USER_VERSION {
            let sql = match version {
                1 => MIGRATE_1_TO_2_SQL,
                v => return Err(LibraryError::Registry(format!("no migration from user_version {v}"))),
            };
            conn.execute_batch(sql).map_err(registry_err)?;
            version += 1;
        }
        conn.pragma_update(None, "user_version", LATEST_USER_VERSION).map_err(registry_err)?;
```

Add:

```rust
    /// Sets the three image columns on the row for `rom_id`. Returns whether
    /// a row matched.
    pub fn update_images(&self, rom_id: i64, fields: &ImageFields) -> Result<bool, LibraryError> {
        let conn = self.conn.lock().unwrap();
        let affected = conn
            .execute(
                "UPDATE installed_games SET cover_small_path = ?1, cover_large_path = ?2, \
                 screenshot_urls = ?3 WHERE rom_id = ?4",
                params![fields.cover_small_path, fields.cover_large_path, fields.screenshot_urls, rom_id],
            )
            .map_err(registry_err)?;
        Ok(affected > 0)
    }
```

with `use crate::images::ImageFields;`.

- [ ] **Step 4: RomDetail** (romm/mod.rs): add `#[derive(serde::Serialize)]` to `RomDetail` and the three fields. `RawRomDetail` gains:

```rust
    #[serde(default)]
    path_cover_small: Option<String>,
    #[serde(default)]
    path_cover_large: Option<String>,
    /// Every field not named above — the screenshot sources
    /// (`merged_screenshots`, `user_screenshots`, metadata blocks…) are read
    /// from here by `screenshot_urls_from_payload`.
    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
```

Replace `impl From<RawRomDetail> for RomDetail` with `impl RawRomDetail { fn into_detail(self, base_url: &str) -> RomDetail { … } }` that computes:

```rust
        let resolver = crate::images::urls::server_resolver(base_url);
        let screenshot_urls =
            crate::images::urls::screenshot_urls_from_payload(&serde_json::Value::Object(self.extra), &resolver);
        // then the existing field mapping plus:
        cover_small_path: self.path_cover_small.unwrap_or_default(),
        cover_large_path: self.path_cover_large.unwrap_or_default(),
        screenshot_urls,
```

and `rom_detail` calls `Ok(raw.into_detail(&self.base))`. `GameSummary`/`RawGameSummary` gain `#[serde(rename = "path_cover_large")] pub cover_large_path: Option<String>` (with `#[serde(default)]` on the raw side) copied through in `From`.

- [ ] **Step 5: `ImageFields::from_detail`** (images/mod.rs):

```rust
impl ImageFields {
    pub fn from_detail(detail: &crate::romm::RomDetail) -> Self {
        Self {
            cover_small_path: detail.cover_small_path.clone(),
            cover_large_path: detail.cover_large_path.clone(),
            screenshot_urls: detail.screenshot_urls.join("\n"),
        }
    }
}
```

`new_record` in library/mod.rs adds `cover_small_path: detail.cover_small_path.clone(), cover_large_path: detail.cover_large_path.clone(), screenshot_urls: detail.screenshot_urls.join("\n"),`. Add `pub fn registry(&self) -> Arc<Registry> { self.registry.clone() }` to `InstallService`.

- [ ] **Step 6: Frontend types.** `api.ts`: `GameSummary` adds `path_cover_large: string | null`; `InstalledGame` adds `cover_small_path: string; cover_large_path: string; screenshot_urls: string;`. `syntheticCloudGame` returns the three as `''`. Every `InstalledGame`/`GameSummary` literal in `*.test.ts` gets the new fields (`grep -rn "installed_at:" app/src` and `"path_cover_small:"` to find them).

- [ ] **Step 7: Run** the full gate (workspace tests, clippy, fmt, hygiene, `npm run check`, `npm test`). Also `grep -rn "cover_path" crates/grid-core/src rewrite/app/src-tauri/src` to check nothing else constructs `GameSummary` positionally.

- [ ] **Step 8: Commit**

```bash
git add rewrite/crates/grid-core/src/library rewrite/crates/grid-core/src/romm/mod.rs rewrite/crates/grid-core/src/images/mod.rs rewrite/crates/grid-core/tests/registry.rs rewrite/crates/grid-core/tests/romm_detail.rs rewrite/app/src/lib/api.ts rewrite/app/src/lib/details/cloud.ts rewrite/app/src/lib/stores/installed.test.ts rewrite/app/src/lib/details/cloud.test.ts
git commit -m "rewrite: registry v2 image columns; RomDetail/GameSummary image fields; install fills them"
```

---

### Task 5: `images::replenish`

**Files:**
- Create: `rewrite/crates/grid-core/src/images/replenish.rs`, `rewrite/crates/grid-core/tests/images_replenish.rs`
- Modify: `images/mod.rs` (`pub mod replenish;`)

**Interfaces:**
- Consumes: `ImageCache::ensure/find_existing`, `image_key`, `server_resolver`, `Registry::update_images/all`, `RommClient::rom_detail`, `ImageFields::from_detail`.
- Produces:
  - `pub enum ReplenishItem { NeedsFields { rom_id: i64 }, NeedsFile { rom_id: i64, url: String } }` (Debug, PartialEq)
  - `pub fn plan(rows: &[InstalledGame], cache: &ImageCache, base_url: &str) -> Vec<ReplenishItem>`
  - `#[derive(Serialize, Clone, Default, Debug, PartialEq)] pub struct ReplenishReport { pub updated_rows: usize, pub fetched_files: usize, pub skipped: usize }`
  - `pub async fn run(client: &RommClient, cache: &ImageCache, registry: &Registry, base_url: &str, items: Vec<ReplenishItem>) -> ReplenishReport`

- [ ] **Step 1: Failing tests** `tests/images_replenish.rs` (wiremock; helper `row(rom_id, small, large, shots)` building an `InstalledGame` with `title = format!("G{rom_id}")`, `platform = "SNES"`):

```rust
#[tokio::test]
async fn plan_classifies_rows() {
    let dir = tempfile::tempdir().unwrap();
    let cache = ImageCache::new(dir.path().to_path_buf());
    let base = "https://h";
    // rom 3 already has its file on disk
    let key = image_key("https://h/assets/3.png");
    std::fs::write(dir.path().join(format!("{key}.png")), b"\x89PNG\r\n\x1a\n").unwrap();
    let rows = vec![
        row(Some(1), "", "", ""),
        row(Some(2), "/assets/2.png", "", ""),
        row(Some(3), "/assets/3.png", "/assets/3l.png", ""),
        row(None, "", "", ""),
        row(Some(5), "https://other/5.png", "", ""), // foreign host: no fetch target
    ];
    assert_eq!(
        plan(&rows, &cache, base),
        vec![
            ReplenishItem::NeedsFields { rom_id: 1 },
            ReplenishItem::NeedsFile { rom_id: 2, url: "https://h/assets/2.png".into() },
        ]
    );
}

#[tokio::test]
async fn run_backfills_fields_fetches_files_and_counts_skips() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/api/roms/1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 1, "name": "G1", "platform_id": 1, "path_cover_small": "/assets/1.png",
            "path_cover_large": "/assets/1l.png",
            "merged_screenshots": ["/assets/roms/1/screenshots/a.png"]
        }))).mount(&server).await;
    Mock::given(method("GET")).and(path("/assets/1.png"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(PNG_MAGIC)).mount(&server).await;
    Mock::given(method("GET")).and(path("/assets/2.png"))
        .respond_with(ResponseTemplate::new(404)).mount(&server).await;
    Mock::given(method("GET")).and(path("/api/roms/9"))
        .respond_with(ResponseTemplate::new(404)).mount(&server).await;

    let dir = tempfile::tempdir().unwrap();
    let cache = ImageCache::new(dir.path().join("covers"));
    let registry = Registry::open(&dir.path().join("db.sqlite")).unwrap();
    registry.upsert(&row(Some(1), "", "", "")).unwrap();
    registry.upsert(&row(Some(2), "/assets/2.png", "", "")).unwrap();
    registry.upsert(&row(Some(9), "", "", "")).unwrap();
    let client = client_for(&server);
    let items = plan(&registry.all().unwrap(), &cache, &server.uri());
    let report = run(&client, &cache, &registry, &server.uri(), items).await;

    assert_eq!(report, ReplenishReport { updated_rows: 1, fetched_files: 1, skipped: 2 });
    let rows = registry.all().unwrap();
    let r1 = rows.iter().find(|r| r.rom_id == Some(1)).unwrap();
    assert_eq!(r1.cover_small_path, "/assets/1.png");
    assert_eq!(r1.cover_large_path, "/assets/1l.png");
    assert_eq!(r1.screenshot_urls, format!("{}/assets/roms/1/screenshots/a.png", server.uri()));
    assert!(cache.find_existing(&image_key(&format!("{}/assets/1.png", server.uri()))).is_some());
}
```

- [ ] **Step 2: Run** → compile error.

- [ ] **Step 3: Write `replenish.rs`**

```rust
//! Replenish (doc 07 "Replenishment of missing covers", D6): after a
//! successful connect, back-fill image fields for rows that lack them and
//! fetch missing small-cover files. Sequential, never fails; errors skip
//! the item.

use super::cache::{image_key, ImageCache};
use super::urls::{filter_to_server_host, resolve_image_url};
use super::ImageFields;
use crate::library::registry::{InstalledGame, Registry};
use crate::romm::RommClient;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplenishItem {
    NeedsFields { rom_id: i64 },
    NeedsFile { rom_id: i64, url: String },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct ReplenishReport {
    pub updated_rows: usize,
    pub fetched_files: usize,
    pub skipped: usize,
}

fn small_cover_url(row: &InstalledGame, base_url: &str) -> String {
    filter_to_server_host(&resolve_image_url(&row.cover_small_path, base_url), base_url)
}

pub fn plan(rows: &[InstalledGame], cache: &ImageCache, base_url: &str) -> Vec<ReplenishItem> {
    let mut items = Vec::new();
    for row in rows {
        let Some(rom_id) = row.rom_id else { continue };
        if row.cover_small_path.is_empty() && row.cover_large_path.is_empty() && row.screenshot_urls.is_empty() {
            items.push(ReplenishItem::NeedsFields { rom_id });
            continue;
        }
        let url = small_cover_url(row, base_url);
        if !url.is_empty() && cache.find_existing(&image_key(&url)).is_none() {
            items.push(ReplenishItem::NeedsFile { rom_id, url });
        }
    }
    items
}

pub async fn run(
    client: &RommClient,
    cache: &ImageCache,
    registry: &Registry,
    base_url: &str,
    items: Vec<ReplenishItem>,
) -> ReplenishReport {
    let mut report = ReplenishReport::default();
    for item in items {
        match item {
            ReplenishItem::NeedsFields { rom_id } => {
                let detail = match client.rom_detail(rom_id).await {
                    Ok(d) => d,
                    Err(_) => {
                        report.skipped += 1;
                        continue;
                    }
                };
                let fields = ImageFields::from_detail(&detail);
                match registry.update_images(rom_id, &fields) {
                    Ok(true) => report.updated_rows += 1,
                    _ => {
                        report.skipped += 1;
                        continue;
                    }
                }
                let url = filter_to_server_host(&resolve_image_url(&fields.cover_small_path, base_url), base_url);
                if !url.is_empty() {
                    match cache.ensure(Some(client), &url).await {
                        Ok(_) => report.fetched_files += 1,
                        Err(_) => report.skipped += 1,
                    }
                }
            }
            ReplenishItem::NeedsFile { url, .. } => match cache.ensure(Some(client), &url).await {
                Ok(_) => report.fetched_files += 1,
                Err(_) => report.skipped += 1,
            },
        }
    }
    report
}
```

- [ ] **Step 4: Run** `cargo test -p grid-core --test images_replenish` + clippy + fmt → green. (In the run test the expected `skipped: 2` = rom 2's 404 cover + rom 9's 404 detail; rom 1 fetched 1 file. If `update_images` for rom 1 returns `Ok(true)` the count is as stated.)

- [ ] **Step 5: Commit**

```bash
git add rewrite/crates/grid-core/src/images/replenish.rs rewrite/crates/grid-core/src/images/mod.rs rewrite/crates/grid-core/tests/images_replenish.rs
git commit -m "rewrite: images::replenish — plan and run the post-connect back-fill"
```

---

### Task 6: Three-way restore and retry (core + commands + api types)

**Files:**
- Modify: `rewrite/crates/grid-core/src/session.rs`, `tests/session.rs`
- Modify: `rewrite/app/src-tauri/src/commands.rs` (`restore_session`, `retry_connect`), `lib.rs` (register), `rewrite/app/src/lib/api.ts`, `rewrite/app/src/lib/stores/session.svelte.ts` (map outcome; UI unchanged until Task 8)

**Interfaces:**
- Produces:
  - `SessionError::NoStoredSession` (`"no stored session"`)
  - `#[serde(tag = "kind", rename_all = "snake_case")] pub enum RestoreOutcome { NoSession, Connected { state: SessionState }, Unreachable { server_url: String, username: String, error: String } }`
  - `SessionManager::restore(&self) -> Result<RestoreOutcome, SessionError>` (Err only for config/secret load failures)
  - `SessionManager::retry(&self) -> Result<SessionState, SessionError>`
  - Tauri `restore_session -> Result<RestoreOutcome, String>`, `retry_connect -> Result<SessionState, String>`
  - TS `RestoreOutcome = { kind: 'no_session' } | { kind: 'connected'; state: SessionState } | { kind: 'unreachable'; server_url: string; username: string; error: string }`; `api.restoreSession(): Promise<RestoreOutcome>`, `api.retryConnect(): Promise<SessionState>`

- [ ] **Step 1: Failing tests** in `tests/session.rs`: update the existing restore assertion to

```rust
    let restored = mgr2.restore().await.expect("restore should not error");
    let RestoreOutcome::Connected { state } = restored else { panic!("expected Connected, got {restored:?}") };
    assert!(state.connected);
    assert_eq!(state.server_url, server.uri());
```

and add:

```rust
#[tokio::test]
async fn restore_reports_no_session_without_stored_server() {
    let dir = tempfile::tempdir().unwrap();
    let mgr = SessionManager::new(dir.path().join("config.toml"), dir.path().join("covers"), Arc::new(MemoryStore::default()));
    assert!(matches!(mgr.restore().await.unwrap(), RestoreOutcome::NoSession));
}

#[tokio::test]
async fn restore_reports_unreachable_and_retry_reconnects() {
    // connect against a live mock, then drop the mock and restore from a
    // fresh manager: Unreachable with the stored server url; bring a mock
    // back on the same address is not possible, so retry is asserted
    // against a manager whose stored url points at a live server instead.
    let server = MockServer::start().await;
    mount_users_me(&server).await;
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(MemoryStore::default());
    let mgr = SessionManager::new(dir.path().join("config.toml"), dir.path().join("covers"), store.clone());
    mgr.connect(server.uri(), String::new(), SecretString::from("FAKE-TEST-TOKEN-not-real"), true).await.unwrap();
    let uri = server.uri();
    drop(server);

    let mgr2 = SessionManager::new(dir.path().join("config.toml"), dir.path().join("covers"), store.clone());
    match mgr2.restore().await.unwrap() {
        RestoreOutcome::Unreachable { server_url, error, .. } => {
            assert_eq!(server_url, uri);
            assert!(!error.is_empty());
        }
        other => panic!("expected Unreachable, got {other:?}"),
    }
    assert!(mgr2.client().is_none());
    assert_eq!(mgr2.server_url(), uri);
    assert!(mgr2.retry().await.is_err());
}
```

(Use whatever helper the file already has for the `/api/users/me` mock; `MemoryStore` is `grid_core::secrets::MemoryStore` if public, else the file's own `FakeStore`.)

- [ ] **Step 2: Run** `cargo test -p grid-core --test session` → fails.

- [ ] **Step 3: Implement** in `session.rs`:

```rust
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RestoreOutcome {
    NoSession,
    Connected { state: SessionState },
    Unreachable { server_url: String, username: String, error: String },
}

    /// Three-way restore (spec "App layer"): no stored session, connected,
    /// or stored-but-unreachable with the probe error's text (SessionError
    /// Display is secret-free by construction). Only config/secret load
    /// failures are `Err`.
    pub async fn restore(&self) -> Result<RestoreOutcome, SessionError> {
        let cfg = Config::load(&self.config_path)?;
        if cfg.server_url.is_empty() {
            return Ok(RestoreOutcome::NoSession);
        }
        let Some(cred) = self.secrets.load()? else {
            return Ok(RestoreOutcome::NoSession);
        };
        *self.server_url.lock().unwrap() = cfg.server_url.clone();
        match self.probe(&cfg.server_url, &cfg.username, cred).await {
            Ok((client, state)) => {
                *self.client.lock().unwrap() = Some(Arc::new(client));
                Ok(RestoreOutcome::Connected { state })
            }
            Err(e) => Ok(RestoreOutcome::Unreachable {
                server_url: cfg.server_url,
                username: cfg.username,
                error: e.to_string(),
            }),
        }
    }

    /// Re-probes with the stored credentials (the chip's Retry).
    pub async fn retry(&self) -> Result<SessionState, SessionError> {
        let cfg = Config::load(&self.config_path)?;
        let Some(cred) = self.secrets.load()? else {
            return Err(SessionError::NoStoredSession);
        };
        if cfg.server_url.is_empty() {
            return Err(SessionError::NoStoredSession);
        }
        let (client, state) = self.probe(&cfg.server_url, &cfg.username, cred).await?;
        *self.client.lock().unwrap() = Some(Arc::new(client));
        Ok(state)
    }
```

Commands:

```rust
#[tauri::command]
pub async fn restore_session(state: State<'_, AppState>) -> Result<RestoreOutcome, String> {
    state.session.restore().await.map_err(err)
}

#[tauri::command]
pub async fn retry_connect(state: State<'_, AppState>) -> Result<SessionState, String> {
    state.session.retry().await.map_err(err)
}
```

Register `retry_connect`. `api.ts` gets the `RestoreOutcome` type and both invokes. `session.svelte.ts` `restore()` becomes:

```ts
export async function restore() {
  try {
    const outcome = await api.restoreSession();
    session.state = outcome.kind === 'connected' ? outcome.state : null;
  } catch {
    session.state = null;
  }
}
```

(Behavior identical to today until Task 8 builds the shell.)

- [ ] **Step 4: Run** full gate (workspace tests, clippy, fmt, hygiene, `npm run check`, `npm test`).

- [ ] **Step 5: Commit**

```bash
git add rewrite/crates/grid-core/src/session.rs rewrite/crates/grid-core/tests/session.rs rewrite/app/src-tauri/src/commands.rs rewrite/app/src-tauri/src/lib.rs rewrite/app/src/lib/api.ts rewrite/app/src/lib/stores/session.svelte.ts
git commit -m "rewrite: three-way session restore and retry_connect"
```

---

### Task 7: App image service — startup sweep, replenish trigger + event, install prefetch hook, `get_rom_detail`

**Files:**
- Create: `rewrite/app/src-tauri/src/images.rs`
- Modify: `rewrite/app/src-tauri/src/lib.rs`, `commands.rs`, `rewrite/crates/grid-core/src/library/mod.rs` (image hook), `rewrite/app/src/lib/api.ts`

**Interfaces:**
- Consumes: `sweep`, `pinned_keys`, `replenish::{plan, run}`, `SessionManager::{client, cache, server_url}`, `InstallService::registry`.
- Produces:
  - grid-core: `pub type ImageHook = Arc<dyn Fn(ImageFields) + Send + Sync>; InstallService::set_image_hook(&self, f: ImageHook)`; called in `finalize_inner` right after `self.registry.upsert(&record)?` with `ImageFields` built from the record.
  - app: `pub struct ImageService { replenish_running: AtomicBool }` with `new() -> Arc<Self>`, `pub fn try_begin_replenish(&self) -> bool`, `pub fn end_replenish(&self)`, `pub fn sweep_at_startup(cache: &ImageCache, rows: &[InstalledGame], base_url: &str) -> SweepReport`, `pub fn spawn_replenish(self: &Arc<Self>, app: AppHandle, session: Arc<SessionManager>, install: Arc<InstallService>)`, `pub fn spawn_prefetch(session: Arc<SessionManager>, fields: ImageFields)`
  - `AppState.images: Arc<ImageService>`
  - Event `images-replenished` (`ReplenishReport`)
  - Tauri `get_rom_detail(rom_id: i64) -> Result<RomDetail, String>`; TS `RomDetail` type (id, name, platform_id, platform_name, fs_name, description, regions, languages, tags, revision, rating, genres, companies, first_release_date, filesize_bytes, server_updated_at, files, cover_small_path, cover_large_path, screenshot_urls: string[]) and `api.getRomDetail(romId)`
  - `connect`, `restore_session` (on `Connected`), `retry_connect` call `state.images.spawn_replenish(...)` on success (commands take `app: tauri::AppHandle`).

- [ ] **Step 1: Failing unit test** in `images.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::ImageService;
    #[test]
    fn only_one_replenish_runs_at_a_time() {
        let svc = ImageService::new();
        assert!(svc.try_begin_replenish());
        assert!(!svc.try_begin_replenish());
        svc.end_replenish();
        assert!(svc.try_begin_replenish());
    }
}
```

- [ ] **Step 2: Write `images.rs`**

```rust
//! App-layer glue for grid-core's `images` module: the startup sweep, the
//! one-at-a-time replenish job with its `images-replenished` event, and the
//! post-install cover prefetch.

use grid_core::images::cache::ImageCache;
use grid_core::images::replenish::{self, ReplenishReport};
use grid_core::images::sweep::{pinned_keys, sweep, SweepReport, IMAGE_CACHE_CAP_BYTES};
use grid_core::images::urls::{filter_to_server_host, resolve_image_url};
use grid_core::images::ImageFields;
use grid_core::library::registry::InstalledGame;
use grid_core::library::InstallService;
use grid_core::session::SessionManager;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

pub const REPLENISHED_EVENT: &str = "images-replenished";

pub struct ImageService {
    replenish_running: AtomicBool,
}

impl ImageService {
    pub fn new() -> Arc<Self> {
        Arc::new(Self { replenish_running: AtomicBool::new(false) })
    }

    pub fn try_begin_replenish(&self) -> bool {
        self.replenish_running.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire).is_ok()
    }

    pub fn end_replenish(&self) {
        self.replenish_running.store(false, Ordering::Release);
    }

    /// R3: synchronous, called from Tauri `setup` before any command runs.
    pub fn sweep_at_startup(cache: &ImageCache, rows: &[InstalledGame], base_url: &str) -> SweepReport {
        let paths = rows
            .iter()
            .flat_map(|r| [r.cover_small_path.as_str(), r.cover_large_path.as_str()]);
        let pinned = pinned_keys(paths, base_url);
        let report = sweep(cache.dir(), IMAGE_CACHE_CAP_BYTES, &pinned);
        if report.deleted > 0 || report.stale_parts > 0 {
            tracing::info!(
                "image cache sweep: {} -> {} bytes, {} deleted, {} stale parts",
                report.total_before, report.total_after, report.deleted, report.stale_parts
            );
        }
        report
    }

    /// One job at a time (Python's `isRunning()` guard): a trigger while a
    /// job runs is dropped. Emits `images-replenished` when done, even with
    /// nothing to do, so the UI can clear any busy state.
    pub fn spawn_replenish(self: &Arc<Self>, app: AppHandle, session: Arc<SessionManager>, install: Arc<InstallService>) {
        if !self.try_begin_replenish() {
            return;
        }
        let svc = self.clone();
        tauri::async_runtime::spawn(async move {
            let report = replenish_once(&session, &install).await;
            svc.end_replenish();
            let _ = app.emit(REPLENISHED_EVENT, report);
        });
    }

    /// Post-install (D5): fetch the small and large covers without blocking
    /// the install. Errors are ignored — the Library's own load and the next
    /// replenish are the fallbacks.
    pub fn spawn_prefetch(session: Arc<SessionManager>, fields: ImageFields) {
        tauri::async_runtime::spawn(async move {
            let base = session.server_url();
            let Some(client) = session.client() else { return };
            for path in [&fields.cover_small_path, &fields.cover_large_path] {
                let url = filter_to_server_host(&resolve_image_url(path, &base), &base);
                if !url.is_empty() {
                    let _ = session.cache().ensure(Some(&client), &url).await;
                }
            }
        });
    }
}

async fn replenish_once(session: &SessionManager, install: &InstallService) -> ReplenishReport {
    let Some(client) = session.client() else {
        return ReplenishReport::default();
    };
    let base = session.server_url();
    let registry = install.registry();
    let rows = {
        let registry = registry.clone();
        tokio::task::spawn_blocking(move || registry.all())
            .await
            .ok()
            .and_then(Result::ok)
            .unwrap_or_default()
    };
    let items = replenish::plan(&rows, session.cache(), &base);
    replenish::run(&client, session.cache(), &registry, &base, items).await
}
```

- [ ] **Step 3: grid-core hook.** In `library/mod.rs`, next to `RaProvider`: `pub type ImageHook = Arc<dyn Fn(ImageFields) + Send + Sync>;`, a `image_hook: RwLock<Option<ImageHook>>` field (initialized `None` in `new`), `pub fn set_image_hook(&self, f: ImageHook)`, and in `finalize_inner` immediately after `self.registry.upsert(&record)?;`:

```rust
        if let Some(hook) = self.image_hook.read().unwrap().clone() {
            hook(ImageFields {
                cover_small_path: record.cover_small_path.clone(),
                cover_large_path: record.cover_large_path.clone(),
                screenshot_urls: record.screenshot_urls.clone(),
            });
        }
```

Follow the existing `set_ra_provider` storage pattern exactly (same lock type it uses).

- [ ] **Step 4: Wire `lib.rs`:** `mod images;`, `AppState { …, images: images::ImageService::new() }`. In `setup`, after the asset-scope block:

```rust
            // R3: sweep synchronously before any image command can run.
            if let Ok(install) = &state.install {
                let rows = install.registry().all().unwrap_or_default();
                let base = Config::load(&Config::default_path()).map(|c| c.server_url).unwrap_or_default();
                images::ImageService::sweep_at_startup(state.session.cache(), &rows, &base);
                let session = state.session.clone();
                install.set_image_hook(Arc::new(move |fields| {
                    images::ImageService::spawn_prefetch(session.clone(), fields);
                }));
            }
```

(`state` is already bound in `setup`; move this block after `let state = app.state::<AppState>();`.) Register `commands::get_rom_detail`.

- [ ] **Step 5: Commands.** `connect`, `restore_session`, `retry_connect` gain `app: tauri::AppHandle` and, on success (`restore_session` only for `RestoreOutcome::Connected`), call:

```rust
    if let Ok(install) = state.install.as_ref() {
        state.images.spawn_replenish(app, state.session.clone(), install.clone());
    }
```

Add:

```rust
#[tauri::command]
pub async fn get_rom_detail(state: State<'_, AppState>, rom_id: i64) -> Result<RomDetail, String> {
    let client = state.session.client().ok_or("not connected")?;
    client.rom_detail(rom_id).await.map_err(err)
}
```

`api.ts`: `RomDetail` type + `getRomDetail: (romId: number) => invoke<RomDetail>('get_rom_detail', { romId })`.

- [ ] **Step 6: Run** full gate. Also `cargo test -p app` for the new unit test.

- [ ] **Step 7: Commit**

```bash
git add rewrite/app/src-tauri/src/images.rs rewrite/app/src-tauri/src/lib.rs rewrite/app/src-tauri/src/commands.rs rewrite/crates/grid-core/src/library/mod.rs rewrite/app/src/lib/api.ts
git commit -m "rewrite: image service — startup sweep, replenish trigger + event, install prefetch, get_rom_detail"
```

---

### Task 8: Shell — top bar, connection chip, Server section, offline-first routing

**Files:**
- Create: `rewrite/app/src/lib/Shell.svelte`, `rewrite/app/src/lib/shell.ts`, `rewrite/app/src/lib/shell.test.ts`
- Rename: `rewrite/app/src/lib/Library.svelte` → `Server.svelte` (`git mv`), then edit (offline notice, `active` prop)
- Create: `rewrite/app/src/lib/Library.svelte` (placeholder section: `<section data-testid="library-section">` with the text "No games installed"; Task 9 replaces it)
- Modify: `rewrite/app/src/App.svelte`, `rewrite/app/src/lib/stores/session.svelte.ts`, `rewrite/app/src/lib/Downloads.svelte` (`export function toggle`), `rewrite/app/src/lib/Connect.svelte` (unchanged unless `session` field names change)

**Interfaces:**
- `shell.ts`:
  ```ts
  export type Section = 'library' | 'server';
  export type SessionPhase = 'loading' | 'none' | 'shell';
  export type ShellSession = { phase: SessionPhase; connected: boolean; serverUrl: string; username: string; lastError: string | null };
  export function applyRestore(outcome: RestoreOutcome): ShellSession
  export function initialSection(connected: boolean): Section   // R2
  export function chipLabel(s: ShellSession): string             // `${username} @ ${host}` | 'Not connected'
  export function hostOf(serverUrl: string): string              // URL host (with port) or the raw string
  ```
- `session.svelte.ts` store shape: `{ phase, connected, serverUrl, username, lastError, error, busy }` plus `restore()`, `connect(...)`, `retry()`, `disconnect()`.
- Test ids: `shell-topbar`, `nav-library`, `nav-server`, `nav-downloads`, `nav-emulators`, `session-chip`, `session-retry`, `session-disconnect`, `session-error`, `server-offline`, `server-retry`, `library-section`, `server-section`.
- `Server.svelte` props: `{ active: boolean }`; exports `handleNav`. Its `<svelte:window onkeydown>` handler returns early when `!active`. When `!session.connected` it renders `<div data-testid="server-offline">Not connected to {host} <button data-testid="server-retry">Retry</button></div>` instead of the platform nav + grid, and its platform fetch `$effect` runs only when connected (re-runs on reconnect).

- [ ] **Step 1: Failing vitest** `shell.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import { applyRestore, chipLabel, hostOf, initialSection } from './shell';

describe('applyRestore', () => {
  it('maps no_session to the connect screen', () => {
    expect(applyRestore({ kind: 'no_session' }).phase).toBe('none');
  });
  it('maps connected to the shell, connected', () => {
    const s = applyRestore({ kind: 'connected', state: { connected: true, username: 'u', server_url: 'https://h:1' } });
    expect(s).toEqual({ phase: 'shell', connected: true, serverUrl: 'https://h:1', username: 'u', lastError: null });
  });
  it('maps unreachable to the shell, offline, with the error', () => {
    const s = applyRestore({ kind: 'unreachable', server_url: 'https://h', username: 'u', error: 'boom' });
    expect(s.phase).toBe('shell');
    expect(s.connected).toBe(false);
    expect(s.lastError).toBe('boom');
  });
});

describe('initialSection / chipLabel / hostOf', () => {
  it('opens Server when connected and Library when offline (R2)', () => {
    expect(initialSection(true)).toBe('server');
    expect(initialSection(false)).toBe('library');
  });
  it('labels the chip', () => {
    expect(chipLabel({ phase: 'shell', connected: true, serverUrl: 'https://romm.example:8080/base', username: 'six', lastError: null })).toBe('six @ romm.example:8080');
    expect(chipLabel({ phase: 'shell', connected: false, serverUrl: 'https://x', username: 'six', lastError: 'e' })).toBe('Not connected');
  });
  it('hostOf falls back to the raw string', () => {
    expect(hostOf('not a url')).toBe('not a url');
  });
});
```

- [ ] **Step 2: Run** `cd app && npm test` → fails (module missing).

- [ ] **Step 3: `shell.ts`**

```ts
import type { RestoreOutcome } from './api';

export type Section = 'library' | 'server';
export type SessionPhase = 'loading' | 'none' | 'shell';
export type ShellSession = {
  phase: SessionPhase;
  connected: boolean;
  serverUrl: string;
  username: string;
  lastError: string | null;
};

export function applyRestore(outcome: RestoreOutcome): ShellSession {
  switch (outcome.kind) {
    case 'no_session':
      return { phase: 'none', connected: false, serverUrl: '', username: '', lastError: null };
    case 'connected':
      return { phase: 'shell', connected: true, serverUrl: outcome.state.server_url, username: outcome.state.username, lastError: null };
    case 'unreachable':
      return { phase: 'shell', connected: false, serverUrl: outcome.server_url, username: outcome.username, lastError: outcome.error };
  }
}

/** R2: Server first when connected (E2E specs wait for platform-btn-1 after connecting), Library when offline. */
export function initialSection(connected: boolean): Section {
  return connected ? 'server' : 'library';
}

export function hostOf(serverUrl: string): string {
  try {
    return new URL(serverUrl).host || serverUrl;
  } catch {
    return serverUrl;
  }
}

export function chipLabel(s: ShellSession): string {
  return s.connected ? `${s.username} @ ${hostOf(s.serverUrl)}` : 'Not connected';
}
```

- [ ] **Step 4: Session store**

```ts
import { api } from '../api';
import { applyRestore, type ShellSession } from '../shell';

export const session = $state<ShellSession & { error: string | null; busy: boolean }>({
  phase: 'loading', connected: false, serverUrl: '', username: '', lastError: null, error: null, busy: false,
});

function assign(next: ShellSession) {
  session.phase = next.phase; session.connected = next.connected; session.serverUrl = next.serverUrl;
  session.username = next.username; session.lastError = next.lastError;
}

export async function restore() {
  try { assign(applyRestore(await api.restoreSession())); }
  catch { assign({ phase: 'none', connected: false, serverUrl: '', username: '', lastError: null }); }
}

export async function connect(serverUrl: string, username: string, secret: string, useToken: boolean) {
  session.busy = true; session.error = null;
  try {
    const state = await api.connect(serverUrl, username, secret, useToken);
    assign({ phase: 'shell', connected: true, serverUrl: state.server_url, username: state.username, lastError: null });
  } catch (e) { session.error = String(e); }
  finally { session.busy = false; }
}

export async function retry() {
  session.busy = true;
  try {
    const state = await api.retryConnect();
    assign({ phase: 'shell', connected: true, serverUrl: state.server_url, username: state.username, lastError: null });
  } catch (e) { session.lastError = String(e); }
  finally { session.busy = false; }
}

export async function disconnect() {
  try { await api.disconnect(); } finally {
    assign({ phase: 'none', connected: false, serverUrl: '', username: '', lastError: null });
  }
}
```

(`Connect.svelte` keeps using `session.busy` / `session.error`.)

- [ ] **Step 5: `Shell.svelte`** — layout: a sticky top bar (`shell-topbar`) with the four nav buttons on the left (`class:active`), the chip on the right (`session-chip` text = `chipLabel`, then `session-retry` when offline, `session-disconnect` always; `session-error` shows `lastError` as a `title` tooltip and a small line under the chip when offline). Below: `<Library active={section==='library'} bind:this={library} hidden={section!=='library'} />` and `<Server active={section==='server'} bind:this={server} hidden={section!=='server'} />` — wrap each in a `<div hidden={…}>` since components cannot take `hidden` directly. `Downloads` renders as today (footer + drawer); `nav-downloads` calls `downloads?.toggle()`; `nav-emulators` sets `showEmulators = true` and renders `<Emulators onClose=…/>` as App.svelte does today. `section` initializes from `initialSection(session.connected)` once on mount. Export `handleNav(action)` that forwards to the active section's component. `Downloads.svelte`: add `export` to its existing `toggle` function.

- [ ] **Step 6: `App.svelte`**

```svelte
{#if session.phase === 'shell'}
  <Shell bind:this={shell} />
{:else if session.phase === 'none'}
  <Connect />
{/if}
```

The `nav` listener forwards to `shell?.handleNav`. `initDownloads()` / `initSessions()` run when `session.phase === 'shell'` (they need no client).

- [ ] **Step 7: `Server.svelte`** — `git mv Library.svelte Server.svelte`; wrap the template in `<section data-testid="server-section">`; add the `active` prop and the offline branch described above; `server-retry` calls `retry()` from the store; the platforms `$effect` depends on `session.connected`. The placeholder `Library.svelte` renders `<section data-testid="library-section"><p>No games installed</p></section>` and exports a no-op `handleNav`.

- [ ] **Step 8: Run** `npm run check && npm test`, then the Rust gate (unchanged), then `bash scripts/e2e.sh connect connect-restore library` to confirm the existing flows still pass with the shell (specs wait for `platform-btn-1` → Server is the initial section when connected).

- [ ] **Step 9: Commit**

```bash
git add rewrite/app/src rewrite/app/src/lib
git commit -m "rewrite: shell with top bar, connection chip, Server section, offline-first routing"
```

---

### Task 9: Library section (installed grid), `Image` component, Details subject

**Files:**
- Create: `rewrite/app/src/lib/library.ts`, `library.test.ts`, `Image.svelte`, `details/subject.ts`, `details/subject.test.ts`
- Replace: `rewrite/app/src/lib/Library.svelte` (real section)
- Delete: `rewrite/app/src/lib/Cover.svelte`
- Modify: `Server.svelte` (use `Image`, pass a subject to Details), `Details.svelte` (subject prop; no layout change yet), `stores/installed.svelte.ts` (`initReplenishListener`), `Shell.svelte` (call it)

**Interfaces:**
- `library.ts`:
  ```ts
  export function isHiddenLibraryPlatform(platform: string): boolean   // trim().toLowerCase() in {'emulator','emulators'}
  export function visibleLibraryGames(rows: InstalledGame[]): InstalledGame[] // filter + sort by (title, platform) lowercased/trimmed
  ```
- `details/subject.ts`:
  ```ts
  export type DetailsSubject = { romId: number | null; name: string; platformName: string; coverSmall: string | null; coverLarge: string | null; screenshotUrls: string[]; description: string; rating: string; genres: string; source: 'server' | 'installed' };
  export function fromSummary(game: GameSummary, platformName: string): DetailsSubject
  export function fromInstalled(row: InstalledGame): DetailsSubject   // screenshotUrls = stored text split on '\n', trimmed, non-empty (the backend already filtered)
  export function summaryOf(s: DetailsSubject): GameSummary            // { id: romId ?? 0, name, platform_id: 0, path_cover_small: coverSmall, path_cover_large: coverLarge } — for isInstalled/syntheticCloudGame
  ```
- `Image.svelte` props: `{ url: string | null; alt: string; placeholder?: string }` (default placeholder = alt). Calls `api.ensureImage(url)`; on error/offline renders the placeholder. Test ids passed through with `...rest`.
- `installed.svelte.ts`: `export function initReplenishListener(): Promise<UnlistenFn>` — `listen('images-replenished', () => refresh())`.
- Library test ids: `library-section`, `library-card-<romId|'x'-index>`, `library-empty`.
- Details: prop `subject: DetailsSubject` replaces `game`; internal uses become `subject.romId` (guarded: actions hidden when `null`, with `<p data-testid="details-no-id">This entry has no server id</p>`), `subject.name`; `isInstalled(summaryOf(subject), subject.platformName)`; `syntheticCloudGame(summaryOf(subject), subject.platformName)`.

- [ ] **Step 1: Failing vitest** `library.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import { isHiddenLibraryPlatform, visibleLibraryGames } from './library';
import type { InstalledGame } from './api';

const row = (title: string, platform: string): InstalledGame => ({
  title, platform, rom_id: 1, rom_file_name: '', archive_path: '', extracted_path: '', extracted_dir: '',
  multi_file_game_dir: '', description: '', rating: '', genres: '', regions: '', languages: '', tags: '',
  revision: '', companies: '', first_release_date: '', filesize_bytes: 0, server_updated_at: '', installed_at: 0,
  cover_small_path: '', cover_large_path: '', screenshot_urls: '',
});

describe('library visibility (game_views.py:297-311)', () => {
  it('hides the synthetic emulator platform, case- and space-insensitively', () => {
    expect(isHiddenLibraryPlatform(' Emulators ')).toBe(true);
    expect(isHiddenLibraryPlatform('emulator')).toBe(true);
    expect(isHiddenLibraryPlatform('SNES')).toBe(false);
  });
  it('sorts by title then platform, case-folded and trimmed', () => {
    const out = visibleLibraryGames([row('zelda', 'SNES'), row(' Alpha', 'PS2'), row('alpha', 'GBA'), row('Redream', 'Emulators')]);
    expect(out.map((r) => `${r.title}|${r.platform}`)).toEqual(['alpha|GBA', ' Alpha|PS2', 'zelda|SNES']);
  });
});
```

and `details/subject.test.ts` covering `fromInstalled` splitting `screenshot_urls` on newlines (ignoring blank lines), `fromSummary` mapping `path_cover_large`, and `summaryOf` giving `id: 0` for a null rom id.

- [ ] **Step 2: Run** `npm test` → fails.

- [ ] **Step 3: Implement** `library.ts`, `subject.ts`, `Image.svelte` (port `Cover.svelte`'s effect to a `url` prop; keep `loading="lazy"`, `draggable="false"`), `Library.svelte`:

```svelte
<script lang="ts">
  import { installed } from './stores/installed.svelte';
  import { visibleLibraryGames } from './library';
  import { fromInstalled, type DetailsSubject } from './details/subject';
  import Image from './Image.svelte';
  import Details from './Details.svelte';
  import { moveFocus, type NavDirection } from './focus/grid';

  let { active }: { active: boolean } = $props();
  const COLUMNS = 6;
  let rows = $derived(visibleLibraryGames(installed.list));
  let focusIndex = $state(0);
  let gridEl = $state<HTMLElement | null>(null);
  let subject = $state<DetailsSubject | null>(null);

  export function handleNav(action: NavDirection | 'accept' | 'back') { /* same shape as Server.svelte's, over `rows` */ }
  function onKey(e: KeyboardEvent) { if (!active) return; /* arrow map as Server.svelte */ }
</script>

<svelte:window onkeydown={onKey} />
<section data-testid="library-section">
  {#if rows.length === 0}
    <p data-testid="library-empty" class="empty">No games installed</p>
  {:else}
    <div class="grid" bind:this={gridEl} style="--columns: {COLUMNS}">
      {#each rows as row, i (row.rom_id ?? `x-${i}`)}
        <div data-testid={`library-card-${row.rom_id ?? `x-${i}`}`} class="card" class:focused={i === focusIndex}
             onclick={() => (subject = fromInstalled(row))} role="presentation">
          <Image url={row.cover_small_path || null} alt={row.title} placeholder="No cover" />
          <div class="caption"><span class="title">{row.title}</span><span class="platform">{row.platform}</span></div>
        </div>
      {/each}
    </div>
  {/if}
</section>
{#if subject}
  {#key subject.romId}
    <Details {subject} onClose={() => (subject = null)} onLibraryPathUnset={() => {}} />
  {/key}
{/if}
```

Reuse `Server.svelte`'s `.grid`/`.card` CSS (copy; a shared stylesheet is optional). `Server.svelte` switches to `<Image url={game.path_cover_small} alt={game.name} />` and passes `subject={fromSummary(detailsGame, activePlatformName)}`. `Details.svelte` switches to the subject prop as listed under Interfaces; `platformName` prop is dropped (it lives on the subject). `Shell.svelte` calls `initReplenishListener()` in its effect and unlistens on teardown.

- [ ] **Step 4: Run** `npm run check && npm test`; then `bash scripts/e2e.sh library install launch cloud-saves` (Details ids unchanged; `game-card-*` stays on the Server grid).

- [ ] **Step 5: Commit**

```bash
git add -A rewrite/app/src/lib
git commit -m "rewrite: Library section (installed grid), Image component, Details subject"
```

---

### Task 10: Details layout — large cover, metadata, screenshot strip

**Files:**
- Modify: `rewrite/app/src/lib/Details.svelte`

**Interfaces:**
- Consumes: `DetailsSubject`, `api.getRomDetail`, `session.connected`, `Image`.
- Produces test ids: `details-cover` (the `<img>` or placeholder container), `details-description`, `details-rating`, `details-genres`, `details-screenshots` (the strip container), `details-screenshot-<n>` (each `<img>`), `details-no-screenshots`.

Design (spec section 4): panel widens to `min(1100px, calc(100vw - 48px))`; CSS grid `grid-template-columns: 240px 1fr 220px` at panel width ≥ 900 px (`@container` query on the panel, or a `ResizeObserver` → class toggle if container queries are unavailable in the WebKitGTK build; check `npm run check` still passes), collapsing to a single column with the strip as a horizontal scroller (`overflow-x: auto; display: flex; gap: 8px`) below the description. Cover box: `Image url={subject.coverLarge ?? subject.coverSmall} placeholder="No cover"`, `aspect-ratio: 3/4`, `object-fit: cover`. Strip: vertical `overflow-y: auto`, each thumbnail `width: 100%; height: auto` (natural aspect), `data-testid="details-screenshot-{i}"`; empty → `<p data-testid="details-no-screenshots">No screenshots available</p>`. Center column keeps h2, platform, chips, rating (`x.x` when non-empty), genres (comma list), description, the action bar, the cloud toggle/panel, errors — in that order. The existing `.wide` cloud-mode width rule is replaced by the new fixed layout.

Data: on open, `detail = subject` fields; when `session.connected && subject.source === 'server'` (or an installed subject whose `screenshotUrls` is empty), call `api.getRomDetail(subject.romId)` once and overlay `coverLarge`, `screenshotUrls`, `description`, `rating`, `genres` from the response (`cover_large_path`, `screenshot_urls`, …). Errors are ignored (the subject's own data stands). Offline installed rows show stored screenshots; `Image` renders nothing (not a placeholder) for a screenshot that errors — wrap each strip item in `{#if}` on a per-item `loaded` flag from `Image`'s `onerror` callback prop (add `onerror?: () => void` to `Image`).

- [ ] **Step 1: Implement** per the design above.
- [ ] **Step 2: Run** `npm run check && npm test`, then `bash scripts/e2e.sh library install launch cloud-saves`.
- [ ] **Step 3: Commit**

```bash
git add rewrite/app/src/lib/Details.svelte rewrite/app/src/lib/Image.svelte
git commit -m "rewrite: Details three-column layout with large cover, metadata and screenshot strip"
```

---

### Task 11: E2E group `images` — mock offline toggle, fixtures, seed, two specs

**Files:**
- Modify: `rewrite/e2e/mock-romm/server.mjs`, `server.test.mjs`, `rewrite/e2e/fixtures/roms.json`, `rom-details.json`, `rewrite/scripts/e2e.sh`, `rewrite/README.md` (coverage table)
- Create: `rewrite/e2e/seed/images-seed.mjs`, `rewrite/e2e/specs/images-a.spec.ts`, `images-b.spec.ts`

**Mock changes:**
- `COVER_PATH_RE` → `/^\/assets\/romm\/resources\/roms\/\d+\/cover\/(small|large)\.png$/`; new `SCREENSHOT_PATH_RE = /^\/assets\/romm\/resources\/roms\/\d+\/screenshots\/\d+\.png$/` served the same way (`state.pngBytes`, `image/png`).
- `state.offline = false`; `POST /__e2e__/offline` with JSON body `{ "offline": true|false }` sets it and replies `{ offline }`; `GET /__e2e__/offline` reads it. Both live next to `/__e2e__/requests` (outside `/api/`, no auth, not logged).
- When `state.offline` is true, every request whose path starts with `/api/` or `/assets/` is answered by `req.socket.destroy()` (a connection error, which is what an unreachable server looks like to reqwest). The `/__e2e__/` routes always work.
- `server.test.mjs`: a test that the large cover and a screenshot route return PNG bytes; a test that after `POST /__e2e__/offline {offline:true}` a `GET /api/users/me` rejects with a socket error and after `{offline:false}` it succeeds again.

**Fixtures:** rom 101 in `roms.json` gains `"path_cover_large": "/assets/romm/resources/roms/101/cover/large.png"`; in `rom-details.json` rom 101 gains `path_cover_small`, `path_cover_large` (same values) and `"merged_screenshots": ["/assets/romm/resources/roms/101/screenshots/1.png", "/assets/romm/resources/roms/101/screenshots/2.png", "https://img.example/box-front.jpg"]`. The other groups' assertions about rom 101 (name, files) are untouched; `server.test.mjs`'s existing fixture assertions must still pass — extend rather than replace.

**Seed `images-seed.mjs`** (`node images-seed.mjs <data-dir>`): `config.toml` with `schema_version = 1` and `library_path = <data-dir>/library` only (no server url — the mock port is unknown at seed time; spec A connects through the UI). `grid-launcher.db` written with the **v1** schema text and `PRAGMA user_version = 1` (copy `cloud-saves-seed.mjs`'s approach) holding one row: rom 102, title `Chrono Trigger (USA)`, platform `Super Nintendo Entertainment System`, `rom_file_name = 'game.rom'`, plus the file `<library>/Super Nintendo Entertainment System/Chrono Trigger (USA)/game.rom`. This row is the migration + replenish subject: it has no image columns until the app migrates it, and empty ones until replenish fills them.

**`images-a.spec.ts`** (connected):
1. Connect through the form (as `library.spec.ts`), wait for `platform-btn-1`.
2. Click `platform-btn-1`, open `game-card-101`; assert `details-cover` img `naturalWidth > 0`; assert `details-screenshot-0` and `details-screenshot-1` exist with `naturalWidth > 0`, and no `details-screenshot-2` (the foreign box-art URL was filtered server-side into the detail). Assert `details-description` has text "A classic platformer.".
3. Set the library path via the banner (as `install-a`), install rom 101, wait for `installed-badge-101`.
4. Click `nav-library`; assert `library-card-101` exists with an `img` whose `naturalWidth > 0`, and `library-card-102` exists (seeded row; its image may or may not be loaded yet — do not assert on it here).
5. `await fetch(`${mockUrl()}/__e2e__/offline`, { method: 'POST', body: JSON.stringify({ offline: true }) })` as the last step, so spec B's app start finds the server unreachable.

**`images-b.spec.ts`** (starts unreachable):
1. Wait for `session-chip` (APP_START_TIMEOUT); assert text "Not connected"; assert `library-section` is displayed (R2) and `library-card-101`'s img has `naturalWidth > 0` (served from the cache with no client).
2. Open `library-card-101`: `details-panel` shows `details-play` enabled and no `details-install`; close.
3. Click `nav-server`; assert `server-offline` exists.
4. `POST /__e2e__/offline {offline:false}`; click `session-retry`; wait until `session-chip` text starts with `e2euser @`; wait for `platform-btn-1`.
5. Click `nav-library`; `browser.waitUntil` that `library-card-102 img` exists with `naturalWidth > 0` (replenish back-filled the migrated row and fetched its cover; timeout `INSTALL_TIMEOUT`).

**`e2e.sh`:** add `"images:specs/images-a.spec.ts specs/images-b.spec.ts"` to `STAGE_GROUPS` and `images) printf '%s' "$E2E_DIR/seed/images-seed.mjs" ;;` to `seed_script_for_group`. README coverage table gets an `images` row describing the above.

- [ ] **Step 1: Mock + fixtures + `server.test.mjs`**; run `node --test e2e/mock-romm/` → green.
- [ ] **Step 2: Seed + specs + e2e.sh + README.**
- [ ] **Step 3: Run** `bash scripts/e2e.sh images` until green, then the full `bash scripts/e2e.sh`.
- [ ] **Step 4: Commit**

```bash
git add rewrite/e2e rewrite/scripts/e2e.sh rewrite/README.md
git commit -m "rewrite: E2E images group — offline start, retry, large cover, screenshots, replenish"
```

---

### Task 12: Docs — deviations and rulings; milestone gate

**Files:**
- Modify: `docs/porting/07-covers-images.md` (new section "Rust port deviations (milestone 7)" with D1–D11 verbatim from the spec plus D11 above; update every Open question with a ruling line: `MAX_CACHED_COVER_BYTES` dropped (D9); eviction → D3; auth → D2; two schemes → D1; double queue → D9; remote key on cleanup → moot (D9); replenish decode → D8/D6; refresh after replenish → D6; `logo_url` and `image_cache_dir` and TV snapshot and TV threads and fanart questions → deferred with TV mode; Discover → out of scope; permissive heuristic → reproduced; 5-slot cap → D7)
- Modify: `docs/porting/02-config-and-secrets.md` (deviation "D-02-a offline-first shell" as worded in the spec), `docs/porting/10-identity-updates.md` ("D-10-a rows without a rom id")
- Modify: `rewrite/README.md` ("Manual test checklist — Milestone 7": the E2E `images` group automates the exit gate; residual manual items: a real RomM server with LaunchBox/ScreenScraper screenshots, a cache over 512 MiB to see the sweep log line)

- [ ] **Step 1: Write the doc sections.** Follow doc 06's "Rust port deviations (milestone 6)" formatting exactly.
- [ ] **Step 2: Milestone gate**, from `rewrite/`: `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --check && bash scripts/check_secret_hygiene.sh && (cd app && npm run check && npm test) && bash scripts/e2e.sh` — all green.
- [ ] **Step 3: Commit**

```bash
git add docs/porting/07-covers-images.md docs/porting/02-config-and-secrets.md docs/porting/10-identity-updates.md rewrite/README.md
git commit -m "rewrite: milestone 7 deviations in docs 07/02/10; README checklist"
```

---

## Self-review notes

- Spec coverage: URL rules (T1), cache/gate/semaphore (T2), pinning + sweep (T3), registry + install fill (T4), replenish (T5), three-way restore + retry (T6), sweep-at-startup + event + prefetch + `get_rom_detail` (T7), shell/chip/Server offline (T8), Library + Image + subject + rom-less rows (T9), Details layout (T10), E2E (T11), deviations (T12). Discover, TV, fanart: out of scope by spec.
- Type consistency: `ImageFields` (T1 mod.rs) is what T4 stores, T5 writes, T7's hook receives. `image_key` (T2) is what T3 pins by and T5 probes with. `RestoreOutcome` (T6) is what T8's `applyRestore` consumes; its serde tag `kind` and snake_case variant names match the TS union.
- Known dependency inversion: T3 (`pinned_keys`) takes plain strings so it does not need T4's fields; T7 adapts the struct to strings.
