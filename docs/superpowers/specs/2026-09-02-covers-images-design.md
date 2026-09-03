# Covers, Images, and the Offline Library — Rust Rewrite Design (Milestone 7)

Date: 2026-09-02
Status: draft for user review
Behavior contract: `docs/porting/07-covers-images.md` (doc 07); the offline
shell touches doc 02 (startup/connection) and the installed grid touches
doc 03 (installed records) and doc 10 (identity).

## Goal

Port the desktop image behavior of doc 07 to the Rust rewrite at behavioral
parity: which cover URL is chosen, which screenshots are accepted, how images
are cached, how missing covers are back-filled, and how the Details view
presents them. Ship the spec's Library section (installed games, usable with
no server connection) and the top-bar shell it needs, because doc 07's
persisted cover data only matters once that view exists.

User rulings that shape this design (2026-09-02):

- Behavioral parity only. "The specifics of the machinery are up to you as
  the programmer." Machinery is free to differ; the result must not.
- The offline installed library is IN scope (option 2), together with the
  spec's top bar, offline-first (option 1).
- Details shows every screenshot in a vertical scrollable column; no cap.
- The image cache is bounded with a startup sweep (no parity with Python's
  never-evict behavior).
- Images reach the webview through the existing "ensure file, return path"
  pipeline, generalized to three variants (approach A). No custom URI scheme.
- TV mode, fanart, and the TV `CoverLoader` are OUT of scope (TV mode is a
  later full redesign). Discover is out of scope (not yet in the rewrite).

## Scope

In scope:

- `grid-core` `images` module: URL rules, screenshot extraction, disk cache
  with in-flight dedup, negative cache, concurrency limit, content gate,
  pinning, and the startup sweep.
- Registry migration adding `cover_small_path`, `cover_large_path`, and
  `screenshot_urls` to `installed_games`; install fills them.
- Replenish job: after each successful connect, back-fill missing image
  fields and missing small-cover files for installed rows; emit an event.
- Tauri: `ensure_image` command (replacing `ensure_cover`), three-way
  session restore, `images-replenished` event, `retry_connect` command.
- Frontend: top-bar shell with Library / Server / Downloads / Emulators
  sections and a connection chip; Library section (installed grid); Server
  section (the current grid, renamed) with a not-connected state; `Image`
  component (replacing `Cover`); Details fed by a subject built from either
  a server summary or an installed row; Details three-column layout with the
  large cover and the screenshot strip.
- Tests: grid-core unit + wiremock integration, vitest, and an E2E group
  `images`.
- Doc updates: deviations in docs 07, 02, and 10.

Out of scope:

- TV mode, fanart background, TV cover loader, platform logo images.
- Discover section and Settings section (placeholders only, not rendered).
- Update detection, "Update Available" / "NEW" badges, rating badges on
  cards (doc 10).
- Any change to how Play / Uninstall are keyed (rom id). See D10.
- A lightbox for screenshots (the Python desktop has none).

## Authority and porting rules

- Doc 07 is normative for what the user sees: URL selection, screenshot
  filtering, host filtering, replenish semantics, placeholder behavior.
- Python's `tests/test_screenshot_urls.py`, `tests/test_cover_manager.py`
  (where meaningful without Qt), and `tests/test_background_workers.py`
  (replenish cases) are the oracle for the pure functions and the job.
- Where machinery differs, the deviation is numbered below and recorded in
  the relevant porting doc on merge, same as milestone 6.

## Architecture

### grid-core: `images/` module (replaces `covers.rs`)

```
crates/grid-core/src/images/
  mod.rs        ImageVariant, ImageRef, re-exports
  urls.rs       resolve_image_url, filter_to_server_host, cover_url_from_payload,
                screenshot_urls_from_payload, screenshot_urls_from_stored,
                looks_like_screenshot_url (regex tokens), extension_for
  cache.rs      ImageCache: ensure(), key(), find_existing(), touch on hit,
                Semaphore(6), in-flight Notify map, session negative map
  sweep.rs      sweep(dir, cap_bytes, pinned: &HashSet<String>) -> SweepReport
  replenish.rs  plan_replenish(rows) -> Vec<ReplenishItem>; run_replenish()
```

**`ImageVariant`**: `CoverSmall | CoverLarge | Screenshot`. Variants exist
for pinning and for the frontend's placeholder choice; the cache key does
not depend on the variant.

**URL rules** (`urls.rs`) — Python parity, oracle-tested:

- `resolve_image_url(value, base_url) -> String`: blank/non-http candidate
  handling, relative join (leading `/` appended verbatim, otherwise joined
  with an inserted `/`), then normalization: path percent-encoded with safe
  set `/%._-~`, query round-tripped keeping blank values, fragment kept.
  Empty `base_url` with a relative candidate → `""`.
- `filter_to_server_host(url, base_url) -> String`: permissive on empty
  `url`, empty `base_url`, or unparseable base netloc; otherwise a candidate
  with a netloc different from the base netloc (whole string, no case fold,
  no default-port normalization) → `""`.
- `cover_url_from_payload(payload: &serde_json::Value, base) -> String`
  walks `url_cover, path_cover_large, path_cover_small, cover_url,
  cover_image, cover_path, image_url`; a dict value tries
  `url, path, image, src, download_path, file_path, full_path`. First
  candidate that resolves AND passes the host filter wins. (The rewrite
  composes resolve+filter at this point, matching the desktop window's
  composition; the Discover bypass does not apply — Discover is out of
  scope.)
- Small/large cover paths for the registry and the grid come from the typed
  `path_cover_small` / `path_cover_large` fields, resolved and filtered the
  same way. `cover_url_from_payload` is the fallback when both are empty.
- `screenshot_urls_from_payload(payload, base) -> Vec<String>`: the seven
  sources in doc 07's order with de-dup at every append, then the
  `looks_like_screenshot_url` filter, then resolve + host filter per URL
  (empty results dropped). Sources: `merged_screenshots`;
  `user_screenshots[]` dicts via `download_path|file_path|full_path`;
  `gamelist_metadata.screenshot_url|title_screen_url`;
  `ss_metadata.screenshot_url|title_screen_url`; `launchbox_metadata.images[]`
  by type token; `url_screenshots|path_screenshots|screenshots|images` with
  the `images`-only type rule and the non-list-as-single-item rule;
  `url_screenshot|path_screenshot`.
- `looks_like_screenshot_url`: haystack `"<path>?<query>"` (raw value when
  both empty); positive tokens `screenshot, screen[_-]shot, gameplay,
  in[_-]game, title[_-]screen, titlescreen`; negative tokens `box,
  box[_-]art, cover, cover[_-]art, fanart, fan[_-]art, logo, clear[_-]logo,
  clear[_-]art, banner, poster, marquee, cartridge, disc`; each bounded by
  non-alphanumerics or string edges, case-insensitive. Positive → true;
  else true iff negative does not match. Permissive default kept (doc 07
  open question ruled: reproduce).
- `screenshot_urls_from_stored(text) -> Vec<String>`: split on `\n`, strip,
  filter with the heuristic, de-dup. Applied on every read of the registry
  column.
- `extension_for(url, body, content_type) -> &'static str`: Content-Type
  table (`image/jpeg|jpg`→jpg, png, webp, gif, `bmp|x-ms-bmp`→bmp, tiff,
  `x-icon|vnd.microsoft.icon`→ico, `svg+xml`→svg), then magic bytes (PNG,
  JPEG, GIF87a/89a, BM, TIFF II*/MM*, ICO, RIFF…WEBP with len ≥ 12), then
  SVG sniff on the first 256 bytes after left-strip, then URL suffix from
  the allowed set `.jpg .jpeg .png .webp .gif .bmp .tif .tiff .ico .svg
  .avif .heic .heif`, else `img`.

**Cache** (`cache.rs`):

- Directory: unchanged (`<cache dir>/covers`, or `<data dir>/covers` under
  `GRID_LAUNCHER_DATA_DIR`). The asset-protocol scope extension in `lib.rs`
  stays.
- Key: lowercase hex SHA-256 of the resolved absolute URL string. Filename
  `<key>.<ext>`. Lookup probes `png, jpg, webp, gif, bmp, tiff, ico, svg,
  img` (the set `extension_for` can produce).
- `ensure(&self, client: Option<&RommClient>, url: &str) -> Result<PathBuf,
  ImageError>`:
  1. Session negative map hit → `Err(recorded)`.
  2. Existing file → touch its mtime (best effort, ignore errors) → `Ok`.
  3. No client → `Err(ImageError::Offline)` (not recorded as negative).
  4. In-flight dedup via the existing Notify pattern (kept verbatim,
     including the enable-before-drop ordering comment).
  5. Owner acquires the `Semaphore(6)` permit, fetches with a 30 s timeout
     through `RommClient::get_bytes_with_type` (new: returns body +
     Content-Type), applies the content gate, writes `<key>.part` then
     renames. Errors are recorded in the negative map and replayed.
- Content gate: a body is accepted iff `extension_for` chose an extension
  from Content-Type or magic bytes or SVG sniff, OR the Content-Type starts
  with `image/`. A body that only matched the URL-suffix rule, or nothing,
  is rejected with `ImageError::NotAnImage` and never written. Empty bodies
  are rejected.
- `ImageError`: `Offline`, `NotAnImage`, `Http(RommError)`, `Io(String)`.
  Cloneable (for replay). Never carries the URL's query string or any
  header (token secrecy).

**Pinning and sweep** (`sweep.rs`):

- `pinned_keys(rows: &[InstalledGame], base_url) -> HashSet<String>`: keys
  of every non-empty `cover_small_path` and `cover_large_path` (resolved
  against `base_url`). Screenshots never pin.
- `sweep(dir, cap_bytes = 512 MiB, pinned) -> SweepReport { total_before,
  total_after, deleted: usize }`: list regular files (skip `.part` older
  than 1 h → delete them too; younger `.part` left alone), sum sizes; if
  ≤ cap return; else sort unpinned by mtime ascending and delete until the
  total ≤ cap. Pinned files are never deleted even if the pinned set alone
  exceeds the cap. Any single delete error is logged at debug (path only)
  and skipped.
- Runs once at startup on `spawn_blocking`, after the registry opens, before
  the first `ensure` (ordering: `ensure` waits on a `OnceCell` set by the
  sweep so a concurrent early fetch cannot be deleted mid-write; in
  practice the sweep is milliseconds).

**Replenish** (`replenish.rs`):

- `plan_replenish(rows) -> Vec<ReplenishItem>`: for each row with a rom id,
  in registry order: `NeedsFields` when all three image columns are empty;
  else `NeedsFile` when `cover_small_path` is non-empty and the cache has no
  file for it; else skipped. Rows without a rom id are skipped.
- `run_replenish(client, cache, registry, items) -> ReplenishReport`:
  sequential; `NeedsFields` fetches `rom_detail`, computes the three
  fields, updates the row (`registry.update_images(rom_id, ...)`), then
  ensures the small cover; `NeedsFile` ensures the small cover. Any error
  skips the item (`RommError`, `ImageError`, registry error) and is counted.
  Never throws. Report: `{ updated_rows, fetched_files, skipped }`.
- Trigger: after every successful `connect` and successful `restore`, and
  after a successful `retry_connect`. One job at a time (an `AtomicBool`
  guard; a trigger while running is dropped, matching Python's
  `isRunning()` check).

### grid-core: registry

- `LATEST_USER_VERSION` → 2. Migration 1→2: `ALTER TABLE installed_games ADD
  COLUMN cover_small_path TEXT NOT NULL DEFAULT ''`, same for
  `cover_large_path`, `screenshot_urls`. `open()` gains a `match` arm per
  version step (`1 => migrate_1_to_2`), then sets `user_version`.
- `InstalledGame` gains the three `String` fields (server-relative paths as
  the server sent them, resolved lazily; `screenshot_urls` newline-joined,
  already resolved+filtered absolute URLs).
- `Registry::update_images(rom_id, small, large, screenshots)`.
- Install (`library/mod.rs`): the install record builder fills the three
  fields from the `RomDetail` it already has. `RomDetail` gains
  `cover_small_path: String`, `cover_large_path: String`,
  `screenshot_urls: Vec<String>` (from `path_cover_small`,
  `path_cover_large`, and the payload run through
  `screenshot_urls_from_payload`; `RawRomDetail` keeps the raw
  `serde_json::Value` for the metadata blocks). `GameSummary` adds
  `path_cover_large` (optional) beside the existing small path.
- After a successful install, the app layer fires small + large cover
  `ensure` calls on a background task (never awaited by the install
  future). Failures are ignored (logged at debug, no URL query).

### App layer (Tauri)

- `SessionManager::restore` returns `RestoreOutcome`: `NoSession`,
  `Connected(SessionState)`, or `Unreachable { server_url, username, error:
  String }`. The error text is the existing `SessionError` display (already
  secret-free). The command `restore_session` returns a tagged JSON enum.
- New command `retry_connect() -> SessionState`: re-probes with stored
  credentials; on success installs the client and fires replenish.
- `ensure_image(variant, url) -> String` replaces `ensure_cover`. `url` is
  server-relative or absolute (already resolved by the caller through the
  typed fields; the command resolves+filters again defensively and returns
  `Err("filtered")` for a foreign host). With no live client the command
  returns the cached path if present, else `Err("offline")`.
- Event `images-replenished` with the `ReplenishReport` payload when a job
  finishes (even with zero changes, so the UI can clear a busy state).
- Startup: sweep on `spawn_blocking` after `Registry::open`.
- `list_installed` returns the three new fields.

### Frontend

- `App.svelte`: three-way restore. `NoSession` → `Connect`. Otherwise
  `Shell` with the session store carrying `{ connected, serverUrl,
  username, lastError }`.
- `Shell.svelte`: top bar (Library, Server, Downloads, Emulators; a
  connection chip on the right: `user @ host` or "Not connected" + Retry;
  Disconnect on the chip). Sections are mounted once and toggled with
  `hidden`, so state survives switching. Gamepad `nav` events route to the
  active section's `handleNav`.
- `Library.svelte` (new meaning): installed grid from the installed store,
  sorted by `(title.casefold, platform.casefold)`, hiding platform
  `emulator|emulators` (case-folded, trimmed). Cards: `Image` (small cover
  via the row's `cover_small_path`), title, platform. Empty: "No games
  installed". Re-renders on `installed` store refresh and on
  `images-replenished`.
- `Server.svelte`: the current `Library.svelte` renamed; when
  `!connected` it renders a notice ("Not connected to <host>" + Retry)
  instead of the platform nav and grid.
- `Image.svelte` (replaces `Cover.svelte`): props `variant`, `url`,
  `alt`; on `url` change calls `ensure_image`, sets `src` via
  `convertFileSrc`, renders the placeholder slot on empty url, error, or
  offline miss. `loading="lazy"` kept.
- `details/subject.ts`: `DetailsSubject = { romId: number | null, name,
  platformName, coverSmall: string | null, coverLarge: string | null,
  screenshotUrls: string[], description, rating, genres }` with
  `fromSummary(GameSummary, platformName)` and `fromInstalled(InstalledGame)`.
  Details takes a subject. When connected and the subject came from a
  summary, Details fetches `rom_detail` once to fill `coverLarge`,
  `screenshotUrls`, `description`, `rating`, `genres` through a new command
  `get_rom_detail(rom_id) -> RomDetail` (no such command exists today; the
  install pipeline fetches the detail internally).
- Details layout: three columns at ≥ 1100 px panel width (cover | center |
  screenshot strip, strip 220 px wide, vertical scroll); below that the
  strip moves under the description as a horizontal scroller. Cover box
  uses `coverLarge ?? coverSmall`, placeholder text "No cover". Strip
  empty state: "No screenshots available". Thumbnails keep their natural
  aspect (`object-fit: contain`, width-bound). Rating shown as `x.x` when
  non-empty; genres comma-joined.
- Rows without a rom id: card renders with placeholder; Details opens with
  no action buttons and a one-line note "This entry has no server id".

## Deviations (numbered; recorded on merge)

Doc 07:

- **D1** One filename scheme for every image: SHA-256 of the resolved URL.
  Python used an identity hash on desktop and a URL hash on TV, into one
  directory. Nothing reads the Python cache, so no compatibility is lost.
- **D2** Every image fetch is authenticated through the RomM client. Python's
  desktop async loader sent no Authorization header (doc 07 open question
  ruled: token-only servers must work).
- **D3** Bounded cache: 512 MiB cap, startup sweep, oldest-unpinned-first,
  installed rows' small and large covers pinned. Uninstall deletes no
  files and cannot fail on image cleanup. Python never evicted and
  unlinked on uninstall with a protected-path set.
- **D4** At most 6 concurrent image downloads; 30 s per-fetch timeout on
  every path. Python: unbounded async loads, 30 s only on blocking paths.
- **D5** The install-time cover fetch is non-blocking. Python blocked the UI
  thread for up to 30 s; the PNG re-encode fallback from an on-screen
  pixmap is dropped (no pixmap exists outside the webview).
- **D6** Replenish also back-fills the three image fields from `rom_detail`
  for rows that lack them, and emits an event that re-renders the Library.
  Python only refetched files and triggered no refresh.
- **D7** No screenshot cap and no width gate that hides screenshots; the
  strip collapses under the description on narrow panels. Python capped at
  5 and hid the column below 1360 px.
- **D8** Content gate replaces the decode gate: a body is written only if
  Content-Type or magic bytes identify an image. Python required a
  successful `QPixmap` decode on the install path and nothing on the
  replenish path.
- **D9** `MAX_CACHED_COVER_BYTES` is dropped (dead in Python). The
  in-memory pixmap cache, waiter lists, local-then-remote double queue,
  `file:` alias keys, and `path_key` case-folding have no equivalent: the
  webview owns decoded images.
- **D10** The details layout metrics (fixed 1.35 aspect math, font scaling
  by window height) are not ported; CSS handles sizing.

Doc 02:

- **D-02-a** Offline-first shell: with a stored server URL and credentials
  the main window renders before and regardless of the probe result;
  "Not connected" state with Retry replaces Python's status label +
  auto-reconnect. Python's `server_auto_reconnect` flag maps to: probe on
  startup, never automatically again; Retry is manual.

Doc 10:

- **D-10-a** Installed rows without a rom id render in the Library but
  expose no actions (Play/Uninstall are rom-id keyed). Python identity
  falls back to (title, platform) for these. Revisit in the identity
  milestone.

## Follow-the-code quirks (ported as-is)

- Host filtering compares whole netlocs: a port mismatch rejects, no case
  folding.
- Screenshot heuristic defaults to "screenshot" for unlabelled URLs.
- Screenshot source order and the `images`-only type rule, including
  "non-list value appended as a single item".
- Stored screenshot lists are re-filtered on read.
- Negative image results are never retried within a process.
- Replenish runs one job at a time; a second trigger while running is
  dropped.
- Library sort and the hidden `emulator|emulators` platform.

## Configuration

No new config keys. The cap is a constant (`IMAGE_CACHE_CAP_BYTES = 512 *
1024 * 1024`). Cache directory resolution is unchanged.

## Security

- Token secrecy unchanged: `ImageError` and all logs carry at most the URL
  path, never headers or query strings. `RestoreOutcome::Unreachable.error`
  is the existing secret-free `SessionError` text.
- Cached filenames are hex digests; no user-controlled path components.
- The asset-protocol scope stays limited to the covers directory.

## Testing

grid-core unit tests (`images/`):

- `urls.rs`: ports of `tests/test_screenshot_urls.py` cases 13, 36, 62, 92,
  114 (LaunchBox typed images; metadata blocks contribute only the two
  keys; source ordering with box/fanart dropped; stored-list re-filter;
  host filtering incl. port mismatch, empty base, unparseable base,
  relative resolve). Cover key walk with dict values. `extension_for`
  table, magic bytes, SVG sniff, suffix, `img` fallback.
- `cache.rs`: dedup (two concurrent ensures, one fetch), negative replay,
  offline miss vs hit, content gate rejects `text/html` bodies and writes
  nothing, `.part` never left behind on success, mtime touched on hit,
  semaphore (7 concurrent ensures, at most 6 in flight, asserted with a
  wiremock delay).
- `sweep.rs`: under cap deletes nothing; over cap deletes oldest unpinned
  first and stops at the cap; pinned survive even above cap; stale `.part`
  removed.
- `replenish.rs`: plan skips rows with files present and rows without rom
  id; run back-fills fields, fetches files, counts skips on 404, never
  errors.
- Registry: 1→2 migration on a v1 fixture db; new fields round-trip.

App integration (wiremock, `--features e2e` where the app harness needs it):

- `ensure_image` sends Authorization; returns cached path offline; errors
  `offline` on a miss.
- Replenish fires after connect and emits `images-replenished`.
- `restore_session` three-way outcomes.

vitest:

- Library sort/hide rules; `fromSummary`/`fromInstalled`; screenshot strip
  renders N thumbnails and the empty text; shell routes restore outcomes.

E2E group `images` (mock RomM gains large covers + screenshots for rom 101,
and an asset route that 401s without a token):

1. Connected: Details for rom 101 shows the large cover and two
   screenshots (`naturalWidth > 0` each).
2. Install rom 101, quit, start with the mock server DOWN: shell renders,
   Library shows rom 101 with its cover from cache, Details opens, Play is
   enabled, Server section shows "Not connected".
3. Start the mock server, click Retry: chip shows connected, Server grid
   loads.
4. Seed an installed row (pre-migration shape: empty image columns) and
   start connected: after `images-replenished`, the Library card shows a
   cover.

## Milestone exit

- Every plan task reviewed clean; final whole-branch review; one fix wave.
- `cargo test --workspace`, `npm test` (vitest), and every E2E stage group
  green: the existing `connect`, `connect-restore`, `library`, `install`,
  `launch`, `downloads`, `emulators`, `emulator-catalog`, `cloud-saves`
  (all touched by the shell change) plus the new `images`.
- Docs 07, 02, 10 deviations recorded; doc 07 open questions updated with
  rulings.
- `cargo clean --profile dev` from `rewrite/` at the end.
