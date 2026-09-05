# 07 — Cover art, screenshots, and image handling

## Purpose

This document describes how GRID Launcher turns a server ROM payload into a picture on
screen, and how those pictures are persisted. It covers:

- how a cover URL is derived from a ROM payload and then rewritten/filtered,
- how screenshot URL lists are extracted and how non-screenshot art is rejected,
- the on-disk image cache (directory, filename scheme, extension detection, deletion),
- the in-memory pixmap cache and the asynchronous desktop load pipeline (dedup of
  in-flight requests, waiter lists),
- the blocking synchronous cache write performed at install time,
- the background worker that back-fills covers for library entries whose cached file is
  missing,
- the separate TV-mode image provider (`CoverLoader`), its own cache key scheme, batch
  cancellation, and the fanart background that consumes it,
- how the details view sizes cover and screenshot widgets.

Out of scope (cross-references):

- The endpoints that produce ROM payloads (`/api/roms`, `/api/roms/{id}`) and the auth
  header construction — see doc 01.
- The rest of `grid_launcher/background/workers.py` (install/download/discover workers)
  — see doc 08. Only `MissingCoverReplenishWorker` is documented here
  (grid_launcher/background/workers.py:836).
- Config file location and `installed_games` normalization in general — see doc 02.
  Only the `cover_url` / `cached_cover_path` / `screenshot_urls` fields are described here
  (grid_launcher/core/config.py:157).
- Install/uninstall flow — see doc 03. Only the two cover hooks it calls are described
  here (grid_launcher/ui/mixins/install_mixin.py:132,
  grid_launcher/library/install_cleanup.py:119).

## External surfaces

### Cache directory layout

| Path | Contents | Anchor |
|---|---|---|
| `~/.grid-launcher/` | config root | grid-launcher.py:2387 |
| `~/.grid-launcher/imagecache/` | every cached image file, flat (no subdirectories) | grid-launcher.py:2389 |

The directory is created lazily with `mkdir(parents=True, exist_ok=True)` immediately
before each write, never at startup (grid_launcher/cover/cache.py:53,
grid_launcher/cover/cache.py:81, grid_launcher/background/workers.py:876,
grid_launcher/tv/widgets/cover_loader.py:186).

Both the desktop pipeline and the TV pipeline write into the same directory, but with
**two different and non-interoperable filename schemes** (see "Filename/key schemes").

### Filename/key schemes

There are three distinct key spaces. A port must keep them distinct.

**1. Desktop on-disk filename — game identity based**
(grid_launcher/cover/utils.py:205):

```
<safe_title[:48]>-<sha1(basis)[:12]><extension>
```

- `basis` = `rom_id` if non-empty, otherwise `"<title>|<platform>"`
  (grid_launcher/cover/utils.py:214). All three inputs are stripped; non-string values
  become `""` (grid_launcher/cover/utils.py:210).
- `sha1` is hex, truncated to 12 characters (grid_launcher/cover/utils.py:215).
- `safe_title` replaces every run of characters outside `[A-Za-z0-9._-]` with a single
  `_`, then strips leading/trailing `_`, `.` and `-`; the empty result becomes the
  literal `"game"` (grid_launcher/cover/utils.py:216). It is then truncated to 48
  characters (grid_launcher/cover/utils.py:217).
- `<extension>` is chosen by `cover_cache_extension_from_payload` (below), including the
  leading dot.

Used by the install-time writer (grid_launcher/cover/cache.py:49), the PNG fallback
writer (which hardcodes `.png`, grid_launcher/cover/cache.py:78), and the replenish
worker (grid_launcher/background/workers.py:873).

**2. TV on-disk filename — URL hash based**
(grid_launcher/tv/widgets/cover_loader.py:152):

```
<sha1(cover_url, full 40 hex chars)><extension>
```

Lookup probes the fixed extension list `.jpg, .jpeg, .png, .webp, .gif` in that order
(grid_launcher/tv/widgets/cover_loader.py:153). Writes pick the extension from magic
bytes, falling back to the URL suffix if it is in `_VALID_IMAGE_EXTS`, else `.png`
(grid_launcher/tv/widgets/cover_loader.py:187). An existing file is never overwritten
(grid_launcher/tv/widgets/cover_loader.py:192).

**3. In-memory pixmap cache keys (desktop only)** — `window.cover_cache` is a single
dict keyed by strings in three flavours (grid-launcher.py:400):

| Key form | Meaning | Anchor |
|---|---|---|
| the resolved remote URL string, e.g. `https://host/api/roms/1/cover` | remote fetch result | grid_launcher/cover/loader.py:71 |
| `file://<absolute path>` (a `QUrl.fromLocalFile(...).toString()`) | local cached file fetched through the same async loader | grid_launcher/cover/manager.py:36 |
| `file:<path_key(path)>` — note the single colon, no slashes, path lower-cased and resolved | canonical alias for a cached file | grid_launcher/cover/utils.py:201 |

`path_key` = `expanduser()` then `resolve(strict=False)` then `casefold()`, falling back
to the unresolved string on `OSError` (grid_launcher/core/path.py:15).

### Endpoints and image sources

GRID Launcher never calls a dedicated "cover" endpoint. Cover and screenshot URLs are
*fields of the ROM payload* returned by the ROM listing/detail endpoints described in
doc 01, and are then fetched as plain GETs.

| Payload field consumed | Schema | Anchor |
|---|---|---|
| `url_cover`, `path_cover_large`, `path_cover_small` | `SimpleRomSchema` / `DetailedRomSchema` | grid_launcher/cover/utils.py:76 |
| `cover_url`, `cover_image`, `cover_path`, `image_url` | tolerated aliases, not in the RomM schema | grid_launcher/cover/utils.py:76 |
| `merged_screenshots` (list of strings) | `SimpleRomSchema` / `DetailedRomSchema` | grid_launcher/cover/utils.py:119 |
| `user_screenshots[].download_path\|file_path\|full_path` | `DetailedRomSchema` | grid_launcher/cover/utils.py:124 |
| `gamelist_metadata.screenshot_url`, `.title_screen_url` | metadata block | grid_launcher/cover/utils.py:132 |
| `ss_metadata.screenshot_url`, `.title_screen_url` | metadata block | grid_launcher/cover/utils.py:137 |
| `launchbox_metadata.images[]` with a screenshot-ish `type` | metadata block | grid_launcher/cover/utils.py:142 |
| `url_screenshots`, `path_screenshots`, `screenshots`, `images` | tolerated aliases | grid_launcher/cover/utils.py:156 |
| `url_screenshot`, `path_screenshot` | tolerated aliases | grid_launcher/cover/utils.py:177 |

Outgoing image requests:

| Caller | Method | Headers | Timeout | Anchor |
|---|---|---|---|---|
| Install-time cache write | blocking `urlopen` | `Accept: image/*`, plus `Authorization` if `_auth_headers()` yields a non-blank one | 30 s | grid_launcher/cover/cache.py:100 |
| Desktop async loader | Qt `QNetworkAccessManager.get` | `Accept: image/*` **only — no Authorization** | none set | grid_launcher/cover/loader.py:58 |
| Replenish worker | blocking `urlopen` | full `auth_headers` dict, with `Accept: image/*` added via `setdefault` | 30 s | grid_launcher/background/workers.py:862 |
| TV `CoverLoader` | blocking `urlopen` on a worker thread | `Accept: image/*`, `User-Agent: grid-launcher/1.0`, plus `Authorization: Bearer <token>` when hosts match | 15 s | grid_launcher/tv/widgets/cover_loader.py:162 |

Platform logo images (TV platform grid) come from a bundled asset directory converted to
a `file://` URI, not from the server: `assets/retroarch-assets/<logo file>`
(grid_launcher/server/catalog.py:11, grid_launcher/server/catalog.py:134). The TV
`CoverLoader` has an explicit `file://` branch to read them
(grid_launcher/tv/widgets/cover_loader.py:140).

## Data model

### Game-dict image fields

Every game dict (server catalog entry, installed record, Discover card) carries:

| Field | Type | Produced by | Anchor |
|---|---|---|---|
| `cover_url` | single resolved absolute URL string, or `""` | `cover_url_from_rom_payload` via the catalog builder | grid_launcher/server/catalog.py:303, grid_launcher/server/catalog.py:352 |
| `screenshot_urls` | newline-joined list of URLs in one string | `screenshot_urls_from_rom_payload` joined with `"\n"` | grid_launcher/server/catalog.py:353 |
| `cached_cover_path` | absolute filesystem path string, or `""` | set at install time, updated by the replenish worker | grid_launcher/library/install_registry.py:32, grid-launcher.py:2884 |

Discover cards set `cover_url` from the first non-empty of
`path_cover_large, path_cover_small, url_cover, cover_url` **without** running it through
`resolve_cover_url`/host filtering, and always set `screenshot_urls` to `""`
(grid_launcher/server/discover.py:196, grid_launcher/server/discover.py:219).

Config normalization coerces all three fields to stripped strings and drops non-strings
to `""` (grid_launcher/core/config.py:157).

TV game dicts may carry `screenshot_urls` as either a newline string **or** a real list;
both TV readers accept both shapes (grid_launcher/tv/widgets/views/details_view.py:979,
grid_launcher/tv/widgets/views/home_view.py:305).

### In-memory state (desktop window)

| Field | Type | Purpose | Anchor |
|---|---|---|---|
| `cover_cache` | `dict[str, QPixmap \| None]` | decoded images and negative results, never evicted | grid-launcher.py:400 |
| `cover_waiters` | `dict[str, list[QLabel]]` | labels waiting on an in-flight URL | grid-launcher.py:401 |
| `cover_loading` | `set[str]` | URLs with a request in flight | grid-launcher.py:402 |
| `cover_network` | one shared network manager | issues all async GETs | grid-launcher.py:403 |
| `_cover_replenish_thread` / `_cover_replenish_worker` | single-slot handles | replenish job in progress | grid-launcher.py:420 |

A `None` value in `cover_cache` is a **negative cache entry**: it means "this URL was
tried and produced no usable image" (grid_launcher/cover/loader.py:71).

### In-memory state (TV `CoverLoader`)

| Field | Type | Purpose | Anchor |
|---|---|---|---|
| `_cache` | `dict[url -> cached file path]`; `""` value means known-bad | maps URL to a file on disk, not to a decoded image | grid_launcher/tv/widgets/cover_loader.py:56 |
| `_next_batch_id` | monotonically increasing int | batch handles | grid_launcher/tv/widgets/cover_loader.py:61 |
| `_cancelled_batches` | `set[int]`, grow-only | suppresses delivery of stale callbacks | grid_launcher/tv/widgets/cover_loader.py:62 |
| `_lock` | mutex | guards all three of the above | grid_launcher/tv/widgets/cover_loader.py:55 |

`_cache` is seeded at construction from a `cover_url -> cached_cover_path` mapping built
by snapshotting `installed_games` (grid-launcher.py:714), filtered to entries whose key is
a string and whose value is a non-empty string — the key's emptiness is not checked
(grid_launcher/tv/widgets/cover_loader.py:56).

### Constant

`MAX_CACHED_COVER_BYTES = 20 * 1024 * 1024` is declared in
grid_launcher/cover/manager.py:11. Production code never reads it; the only reference is
a unit test that writes a file larger than it (tests/test_cover_manager.py:57). It is
effectively dead — see "Open questions".

## Behavior

### URL resolution rules

`resolve_cover_url(value, base_url)` (grid_launcher/cover/utils.py:28):

1. Non-string or blank-after-strip → `""` (grid_launcher/cover/utils.py:29).
2. If the stripped candidate does not start with `http://` or `https://`, it is treated
   as server-relative. With an empty `base_url` the result is `""`; otherwise a `/`-
   prefixed candidate is appended to `base_url` verbatim, and any other candidate is
   joined with an inserted `/` (grid_launcher/cover/utils.py:32).
3. The result is normalized: the path is percent-encoded with the safe set
   `/%._-~`, the query is round-tripped through parse/urlencode with blank values kept,
   and the fragment is preserved (grid_launcher/cover/utils.py:40).

`filter_to_server_host(url, base_url)` (grid_launcher/cover/utils.py:47):

- Empty `url` or empty `base_url` → returned unchanged (permissive).
- `base_url` with no parseable netloc → returned unchanged (permissive).
- Otherwise, if the candidate has a netloc and it differs from the base netloc → `""`.
  Comparison is on the whole netloc string, so a port mismatch rejects
  (tests/test_screenshot_urls.py:135) and no case-folding or default-port normalization
  is applied.

The desktop window always composes both, in this order:
`filter_to_server_host(resolve_cover_url(value, base), base)` (grid-launcher.py:2894).
Consequence: third-party scraper hosts (ScreenScraper, LaunchBox CDN, …) are dropped
whenever a server URL is configured (tests/test_screenshot_urls.py:114), but are allowed
through when no server URL is set (tests/test_screenshot_urls.py:119).

**Cover selection from a payload** — `cover_url_from_rom_payload`
(grid_launcher/cover/utils.py:63) walks the key list
`url_cover, path_cover_large, path_cover_small, cover_url, cover_image, cover_path,
image_url` in order and returns the first key that resolves to a non-empty URL
(grid_launcher/cover/utils.py:76). A key's value may be a string, or a dict, in which
case the sub-keys `url, path, image, src, download_path, file_path, full_path` are tried
in order (grid_launcher/cover/utils.py:68).

**Cover selection for a game** — `resolved_cover_url_for_game`
(grid_launcher/cover/details.py:15):

1. Resolve `game["cover_url"]`; if non-empty, return it.
2. Otherwise take `game["rom_id"]` (stripped); empty → `""`.
3. Look up `server_rom_payloads[rom_id]` and, if it is a dict, run
   `cover_url_from_rom_payload` on it (grid_launcher/cover/details.py:33).

**Screenshot extraction from a payload** — `screenshot_urls_from_rom_payload`
(grid_launcher/cover/utils.py:93). Sources are appended in a fixed order, with
de-duplication against the already-collected list at every append
(grid_launcher/cover/utils.py:104):

1. `merged_screenshots` list (grid_launcher/cover/utils.py:119).
2. `user_screenshots` — dict items only, trying `download_path`, `file_path`,
   `full_path` (grid_launcher/cover/utils.py:124).
3. `gamelist_metadata.screenshot_url` then `.title_screen_url`
   (grid_launcher/cover/utils.py:132).
4. `ss_metadata.screenshot_url` then `.title_screen_url`
   (grid_launcher/cover/utils.py:137).
5. `launchbox_metadata.images[]` — only entries whose `type`, case-folded, *contains* one
   of `screenshot, title screen, titlescreen, gameplay, in-game, ingame`
   (grid_launcher/cover/utils.py:95, grid_launcher/cover/utils.py:153).
6. `url_screenshots, path_screenshots, screenshots, images`. For `images` specifically,
   dict entries with a string `type` are kept only if the type matches the same token
   list; dict entries without a string `type`, and non-dict entries, are appended
   unconditionally. For the other three keys every list item is appended. A non-list
   value at any of these keys is appended as a single item
   (grid_launcher/cover/utils.py:156).
7. `url_screenshot`, `path_screenshot` (grid_launcher/cover/utils.py:177).

The whole collected list is then filtered by `_looks_like_screenshot_url`
(grid_launcher/cover/utils.py:180).

**Screenshot heuristic** — `_looks_like_screenshot_url` (grid_launcher/cover/utils.py:20)
builds a haystack of `"<path>?<query>"` (or the raw value when both are empty) and:

- returns `True` if the positive pattern matches — any of `screenshot`, `screen_shot`/
  `screen-shot`, `gameplay`, `in_game`/`in-game`, `title_screen`/`title-screen`,
  `titlescreen`, each bounded by non-alphanumerics or string edges, case-insensitive
  (grid_launcher/cover/utils.py:10);
- otherwise returns `True` only if the negative pattern does **not** match — `box`,
  `box_art`/`box-art`, `cover`, `cover_art`, `fanart`/`fan_art`, `logo`, `clear_logo`,
  `clear_art`, `banner`, `poster`, `marquee`, `cartridge`, `disc`
  (grid_launcher/cover/utils.py:14).

So the default for an unlabelled URL is "treat as screenshot".

**Screenshot list on a stored game** — `screenshot_urls_from_game(raw)` splits the stored
newline string, strips each line, applies the same heuristic, and de-duplicates, so stale
box-art URLs saved by older versions are filtered out at read time
(grid_launcher/cover/utils.py:183, tests/test_screenshot_urls.py:92).

### Cache lookup, store, and eviction

**Cached path from a game dict** — `cached_cover_path_from_game` returns
`Path(game["cached_cover_path"].strip()).expanduser()`, or `None` when the field is
missing/blank/non-string (grid_launcher/cover/utils.py:194). It does **not** touch the
filesystem.

**Memory lookup** — `cached_cover_for_game` (grid_launcher/cover/manager.py:40):

1. No cached path → `None`.
2. If the `file:<path_key>` alias key is present in `cover_cache`, return its value
   (possibly `None`, i.e. a negative entry).
3. Else if the `file://…` URL key is present, copy it into the alias key and return it
   (grid_launcher/cover/manager.py:49).
4. Else `None`.

No disk read and no decode occurs here — this is asserted by
tests/test_cover_manager.py:39, which patches `QPixmap` inside the manager module to
raise if constructed.

**Extension detection** — `cover_cache_extension_from_payload(cover_url, payload,
content_type)` (grid_launcher/cover/utils.py:220), in order:

1. `Content-Type`, lower-cased and truncated at the first `;`, mapped through a fixed
   table: `image/jpeg` and `image/jpg` → `.jpg`, `image/png` → `.png`, `image/webp` →
   `.webp`, `image/gif` → `.gif`, `image/bmp` and `image/x-ms-bmp` → `.bmp`,
   `image/tiff` → `.tiff`, `image/x-icon` and `image/vnd.microsoft.icon` → `.ico`,
   `image/svg+xml` → `.svg` (grid_launcher/cover/utils.py:222).
2. Magic bytes: PNG signature, `FF D8 FF` (JPEG), `GIF87a`/`GIF89a`, `BM`, `II*\0`/
   `MM\0*` (TIFF), `\0\0\1\0` (ICO), and `RIFF….WEBP` with a length ≥ 12 check
   (grid_launcher/cover/utils.py:239).
3. SVG sniff on the first 256 bytes after left-strip (grid_launcher/cover/utils.py:254).
4. The URL path suffix, lower-cased, if it is in the allowed set
   `.jpg .jpeg .png .webp .gif .bmp .tif .tiff .ico .svg .avif .heic .heif`
   (grid_launcher/cover/utils.py:260).
5. Fallback `.img` (grid_launcher/cover/utils.py:277).

**Install-time synchronous store** — `cache_cover_image_for_game`
(grid_launcher/cover/cache.py:91), called once per install to compute the record's
`cached_cover_path` (grid_launcher/ui/mixins/install_mixin.py:132):

1. If `cached_cover_path` already points at an existing regular file, return it as-is
   (grid_launcher/cover/cache.py:92).
2. Resolve the cover URL; empty → return `""` (grid_launcher/cover/cache.py:96).
3. Blocking GET, 30 s timeout, `Accept: image/*` plus `Authorization` when
   `_auth_headers()["Authorization"]` is non-blank after strip
   (grid_launcher/cover/cache.py:100).
4. `_write_cover_payload` (grid_launcher/cover/cache.py:40): empty payload → `""`;
   payload that fails to decode → `""` (so non-image bodies are never written); choose
   extension; write bytes; on `OSError` return `""`. On success, insert the decoded
   pixmap into `cover_cache` under **both** the remote URL key and the
   `file:<path_key>` alias, then return the file path
   (grid_launcher/cover/cache.py:58).
5. On any of `HTTPError, URLError, OSError, ValueError, OverflowError`, or when the write
   returned `""`, fall through to `_save_fallback_cached_pixmap`
   (grid_launcher/cover/cache.py:113).

**Fallback store** — `_save_fallback_cached_pixmap` (grid_launcher/cover/cache.py:63)
re-encodes whatever pixmap is already available:

1. Take `cover_cache[cover_url]`. If missing or null, and the currently open details game
   matches this game by identity and a details cover label exists, steal that label's
   pixmap and memoize it under the URL key (grid_launcher/cover/cache.py:67). Identity
   matching is `rom_id` equality when both sides have one, else `(title, platform)`
   equality (grid_launcher/library/identity.py:15).
2. Still null → `""`.
3. Save as PNG under `<installed_cover_cache_key>.png`; a false return from the save or
   an `OSError` yields `""` (grid_launcher/cover/cache.py:82).
4. On success, memoize under the `file:<path_key>` alias and return the path.

**Deletion (the only eviction that exists)** — `cleanup_cached_cover_for_game`
(grid_launcher/cover/manager.py:84):

1. No cached path → `(True, None)`.
2. Unconditionally drop both the `file:<path_key>` alias and the `file://…` URL key from
   `cover_cache` (grid_launcher/cover/manager.py:93). The remote-URL key is **not**
   dropped.
3. If `protected_cache_paths` contains this path's `path_key`, stop without unlinking —
   another installed game shares the file (grid_launcher/cover/manager.py:96).
4. If the path is not an existing regular file, stop.
5. `unlink()`. `OSError` propagates to the caller (grid_launcher/cover/manager.py:102).

The protected set is built by `cached_cover_path_keys_for_games` over all *surviving*
library entries (grid_launcher/cover/manager.py:74,
grid_launcher/library/install_cleanup.py:113). The desktop wrapper catches `OSError`,
shows an "Uninstall Error" message box, and returns `False`, which aborts the uninstall
(grid-launcher.py:2937, grid_launcher/library/install_cleanup.py:119).

There is **no size-based, count-based, or age-based eviction** anywhere. Files are
removed only on uninstall, and `cover_cache` in memory is never cleared or trimmed for
the lifetime of the process.

### Async load lifecycle (desktop)

`queue_cover_load(window, cover_url, label)` (grid_launcher/cover/loader.py:43):

1. Strip the URL; blank → return.
2. **Cache hit (including negative entries)** → apply immediately and return
   (grid_launcher/cover/loader.py:48). Note `in` is used, so a cached `None` short-
   circuits and no request is issued.
3. Append the label to `cover_waiters[url]` (grid_launcher/cover/loader.py:52).
4. **Dedup**: if the URL is already in `cover_loading`, return — the existing reply will
   serve every waiter (grid_launcher/cover/loader.py:54).
5. Mark in-flight, issue the GET with `Accept: image/*`, and connect the reply's
   `finished` to the completion handler, binding both the URL and the reply into the
   closure (grid_launcher/cover/loader.py:57).

`on_cover_reply(window, cover_url, reply)` (grid_launcher/cover/loader.py:64):

1. Only on `NoError`, read the body and try to decode; anything else leaves the pixmap
   `None` (grid_launcher/cover/loader.py:66).
2. Store the result (pixmap or `None`) in `cover_cache` — negative caching
   (grid_launcher/cover/loader.py:71).
3. Discard from `cover_loading`, pop the waiter list, apply to every waiter
   (grid_launcher/cover/loader.py:72).
4. `reply.deleteLater()` (grid_launcher/cover/loader.py:76).

`apply_cover_to_label(label, pixmap)` (grid_launcher/cover/loader.py:24):

- `None` or null pixmap → no-op, so placeholder text stays visible.
- If the label exposes `set_source_pixmap`, delegate to it and return — this is the
  aspect-preserving screenshot label (grid_launcher/cover/loader.py:27,
  grid_launcher/ui/game_views.py:104).
- Otherwise clear the label text and set a `KeepAspectRatio` + `SmoothTransformation`
  scale of the pixmap to the label's current size, swallowing `RuntimeError` (deleted
  widget) (grid_launcher/cover/loader.py:31).

`queue_game_cover_load(window, game, label)` (grid_launcher/cover/manager.py:58) is the
entry point every card and the details cover uses (grid_launcher/ui/game_views.py:336,
grid_launcher/ui/game_views.py:483):

1. If a decoded pixmap is already in memory, deliver it via a zero-delay timer rather
   than inline, so the caller never re-enters painting during widget construction
   (grid_launcher/cover/manager.py:62).
2. Otherwise, if the game has a `cached_cover_path`, queue a load of its `file://…` URL
   (grid_launcher/cover/manager.py:65). Existence is not checked; a missing file simply
   produces a negative cache entry.
3. **In addition**, if a remote cover URL resolves, queue that too
   (grid_launcher/cover/manager.py:69). Both requests target the same label; whichever
   completes last wins. tests/test_cover_manager.py:54 pins exactly this: two queued
   loads, `file:`-prefixed first, remote second, nothing applied synchronously.

### Details view media

`update_details_screenshots(window, game)` (grid_launcher/cover/details.py:74): if there
are no screenshot label slots, return. Otherwise, for each of the fixed 5 slots
(grid-launcher.py:2117), clear it, then either show it and queue the URL at that index,
or hide it (grid_launcher/cover/details.py:79). Extra URLs beyond 5 are dropped.

`rescale_details_media_for_current_sizes(window)` (grid_launcher/cover/details.py:166) is
the re-fit path after a resize: re-queue the game cover through
`_queue_game_cover_load`, and for each screenshot slot apply the cached pixmap directly
when present, else queue the load (grid_launcher/cover/details.py:181). Because
`apply_cover_to_label` re-scales from the source pixmap, this is what makes covers sharp
after a window resize instead of stretched.

`update_details_layout_metrics(window)` (grid_launcher/cover/details.py:96) computes
widget geometry:

- Content width falls back to `window.width() - 64` when the frame reports ≤ 0, then is
  clamped to a minimum of 640; height falls back to `window.height() - 180` with a
  minimum of 420 (grid_launcher/cover/details.py:100).
- Cover aspect ratio is fixed at 1.35 (height/width). `cover_max_height =
  clamp(int(content_height * 0.78), 320, 680)`; `cover_width = clamp(min(int(content_width
  * 0.32), int(cover_max_height / 1.35)), 220, 720)`; `cover_height =
  clamp(int(cover_width * 1.35), 300, cover_max_height)` (grid_launcher/cover/details.py:110).
  The cover label gets a **fixed** size (grid_launcher/cover/details.py:122).
- Screenshot width = `clamp(int(content_width * 0.19), 160, 420)`
  (grid_launcher/cover/details.py:124). Screenshot labels get only a **maximum** width —
  never a fixed size (grid_launcher/cover/details.py:154,
  tests/test_cover_details_layout.py:177). The scroll area gets a fixed width of
  `screenshot_width + 28` (grid_launcher/cover/details.py:151).
- `compact_cloud_layout` is true when the details view is *not* in `"overview"` cloud
  mode and any of: content width < 1360, content height < 640, window width ≤ 1280,
  window height ≤ 720 (grid_launcher/cover/details.py:132).
- Screenshots are shown only when not compact **and**
  `content_width >= cover_width + minimum_center_width + (screenshot_width + 84)`, where
  `minimum_center_width` is 620 outside overview mode and 420 in overview mode
  (grid_launcher/cover/details.py:138). Visibility is applied to the panel when it
  exists, else to the scroll area (grid_launcher/cover/details.py:144).
- Description max width = `clamp(int(content_width * 0.42), 280, 1200)`
  (grid_launcher/cover/details.py:156).
- Font scale = `clamp(window.height() / 1080, 720/1080, 2.5)`, applied by rewriting every
  `font-size: Npx` occurrence in each registered label's *base* stylesheet to
  `max(8, round(N * scale))` (grid_launcher/cover/details.py:88,
  grid_launcher/cover/details.py:159). Because the base stylesheet is stored alongside the
  label, rescaling is idempotent.
- Finally a zero-delay timer schedules the media rescale
  (grid_launcher/cover/details.py:163).

### Replenishment of missing covers

Trigger: immediately after a successful server connection, inside the connect routine
(grid-launcher.py:3039).

Job assembly — `_start_missing_cover_replenish` (grid-launcher.py:2846):

1. If a replenish thread exists and is running, return (single job at a time)
   (grid-launcher.py:2847).
2. For each library game: skip non-dicts; skip when `cached_cover_path` points at an
   existing regular file; skip when no cover URL resolves; else collect
   `(game_key, dict(game), cover_url)` — a **snapshot copy** of the game dict
   (grid-launcher.py:2850).
3. Empty list → return without starting a thread (grid-launcher.py:2860).
4. Build the worker with the auth headers and image cache dir, move it to a fresh thread,
   wire `started -> run`, `game_cover_cached -> _on_cover_replenish_game_cached`,
   `finished -> _on_cover_replenish_finished`, `finished -> thread.quit`,
   `finished -> worker.deleteLater`, `thread.finished -> thread.deleteLater`, and start
   (grid-launcher.py:2864).

Worker body — `MissingCoverReplenishWorker.run` (grid_launcher/background/workers.py:852),
sequentially per game:

1. Empty cover URL → skip (grid_launcher/background/workers.py:856).
2. Re-check the snapshot's `cached_cover_path` for an existing regular file → skip
   (grid_launcher/background/workers.py:858, tests/test_background_workers.py:1628 asserts
   no network call happens in this case).
3. Blocking GET, 30 s, headers = copy of the auth headers with `Accept: image/*` added by
   `setdefault`. Any of `HTTPError, URLError, OSError, ValueError, OverflowError` → skip
   this game and continue (grid_launcher/background/workers.py:861,
   tests/test_background_workers.py:1694).
4. Empty payload → skip. **No decode check here** — unlike the install-time writer, the
   worker writes whatever bytes came back (grid_launcher/background/workers.py:870).
5. Compute extension and filename with the same identity-based scheme, mkdir, write
   bytes; `OSError` → skip (grid_launcher/background/workers.py:872).
6. Emit `{"game_key": …, "path": str(cache_file)}` (grid_launcher/background/workers.py:880).
7. After the loop, emit `finished` exactly once, regardless of failures
   (grid_launcher/background/workers.py:882).

Main-thread handling — `_on_cover_replenish_game_cached` ignores payloads with an empty
`path`, then finds the first library game whose key matches and writes the new
`cached_cover_path` onto the live dict (grid-launcher.py:2877).
`_on_cover_replenish_finished` saves the config and clears both handles
(grid-launcher.py:2887). Note the UI is not refreshed and `cover_cache` is not
invalidated by this — the new file is picked up on the next queue.

### TV-mode variants

The TV image provider `CoverLoader` (grid_launcher/tv/widgets/cover_loader.py:44) is an
independent implementation. tests/test_tv_image_provider.py:113 pins that importing it
does **not** import `grid_launcher.cover.loader`.

Differences from the desktop pipeline:

| Aspect | Desktop | TV |
|---|---|---|
| Transport | Qt network manager on the main thread | blocking `urlopen` on a fresh daemon `threading.Thread` per request | (grid_launcher/cover/loader.py:60 vs grid_launcher/tv/widgets/cover_loader.py:120) |
| Memory cache | URL → decoded pixmap | URL → cached **file path** (or `""` for known-bad) | (grid-launcher.py:400 vs grid_launcher/tv/widgets/cover_loader.py:56) |
| Disk key | game identity hash | full SHA-1 of the URL | (grid_launcher/cover/utils.py:205 vs grid_launcher/tv/widgets/cover_loader.py:152) |
| Auth | not sent on async loads | `Bearer` sent when the cover host equals the server host, or when a token exists and no server URL is configured | (grid_launcher/cover/loader.py:59 vs grid_launcher/tv/widgets/cover_loader.py:163) |
| Dedup of in-flight requests | yes, via `cover_loading` + waiter lists | **no** — two concurrent `load_async` calls for the same URL each fetch | (grid_launcher/cover/loader.py:54 vs grid_launcher/tv/widgets/cover_loader.py:85) |
| Cancellation | none | batch IDs | (grid_launcher/tv/widgets/cover_loader.py:64) |
| Timeout | none / 30 s | 15 s | (grid_launcher/tv/widgets/cover_loader.py:173) |
| Payload validation | `QPixmap.loadFromData` | `QImage.loadFromData`, applied to both network payloads and files read from disk | (grid_launcher/tv/widgets/cover_loader.py:213) |

`_load_bytes` order (grid_launcher/tv/widgets/cover_loader.py:123):

1. Empty URL → `None`.
2. Memory cache hit: `""` → `None` (known-bad). A path → read and validate it; if that
   fails, the entry is **evicted** and the lookup continues, so a deleted file self-heals
   (grid_launcher/tv/widgets/cover_loader.py:130, tests/test_tv_image_provider.py:87).
3. `file://` URLs are converted with `url2pathname` and read directly; no caching, no
   network fallback (grid_launcher/tv/widgets/cover_loader.py:140).
4. Probe `<sha1>.jpg|.jpeg|.png|.webp|.gif` in the shared image cache directory; the first
   readable, valid image populates the memory cache and is returned
   (grid_launcher/tv/widgets/cover_loader.py:153).
5. Network GET. Failure → memoize `""` and return `None`
   (grid_launcher/tv/widgets/cover_loader.py:175). A payload that fails `QImage` decode →
   also memoize `""` and return `None` (grid_launcher/tv/widgets/cover_loader.py:180).
6. Write to disk (skipping the write when the file already exists) and memoize the path.
   Write failures are swallowed; the bytes are still returned
   (grid_launcher/tv/widgets/cover_loader.py:185).

`load_async(url, callback, batch_id=None)` (grid_launcher/tv/widgets/cover_loader.py:85)
fetches on the worker thread, then marshals delivery back through a zero-delay timer
parented to the application object; with no running application it calls back inline
(grid_launcher/tv/widgets/cover_loader.py:114). Before delivering, a non-`None`
`batch_id` present in `_cancelled_batches` suppresses the callback entirely
(grid_launcher/tv/widgets/cover_loader.py:93). Delivery is wrapped so that a
`RuntimeError` whose message contains `"already deleted"` is swallowed and any other
`RuntimeError` re-raises (grid_launcher/tv/widgets/cover_loader.py:109).

**Batching.** Only the server-view game wall uses batches. `set_games` cancels the
previous batch and allocates a new one before repopulating
(grid_launcher/tv/widgets/components/game_wall.py:66), then only rows intersecting the
visible viewport are populated, each card's load tagged with the current batch
(grid_launcher/tv/widgets/components/game_wall.py:88,
grid_launcher/tv/widgets/components/game_wall.py:112). Already-populated indices are
tracked so scrolling never re-requests; when everything is populated the scroll handler
disconnects itself (grid_launcher/tv/widgets/components/game_wall.py:118).
tests/test_game_wall_batching.py:53 pins the cancel-on-second-`set_games` behavior.

Other TV consumers pass **no** batch id and therefore cannot be cancelled: home rows
(grid_launcher/tv/widgets/components/game_row.py:87), the library carousel's pool bind
and recycle-on-nav (grid_launcher/tv/widgets/views/library_view.py:295,
grid_launcher/tv/widgets/views/library_view.py:563), platform cards
(grid_launcher/tv/widgets/components/platform_card.py:34), the details cover
(grid_launcher/tv/widgets/views/details_view.py:421), details screenshots
(grid_launcher/tv/widgets/views/details_view.py:875), and fanart
(grid_launcher/tv/widgets/components/fanart_background.py:76).

**Placeholder behavior (TV).** Cards store `None` on a failed load and repaint, so their
painted placeholder shows; every card guards against being orphaned from the widget tree
before calling `update()`, because updating an orphan would turn it into a top-level
window (grid_launcher/tv/widgets/components/game_card.py:48,
grid_launcher/tv/widgets/components/home_card.py:45,
grid_launcher/tv/widgets/components/platform_card.py:36). The TV details cover replaces
its content with the literal text `"No Cover"` plus a placeholder stylesheet
(grid_launcher/tv/widgets/views/details_view.py:755). The TV details screenshot column
renders a single `"No screenshots available"` label when the URL list is empty
(grid_launcher/tv/widgets/views/details_view.py:830) and sizes each loaded shot's card
height from the pixmap's aspect ratio (grid_launcher/tv/widgets/views/details_view.py:866,
tests/test_tv_details_view.py:86).

### Fanart background (TV)

`FanartBackground` is driven entirely by the cover system — it has no separate fetcher
(grid_launcher/tv/widgets/components/fanart_background.py:38).

Image sources, all *screenshot* URLs rather than a dedicated fanart field:

| View | Source | Anchor |
|---|---|---|
| Home | active row card's `screenshot_urls`, falling back to `url_screenshots`; accepts list or newline string | grid_launcher/tv/widgets/views/home_view.py:305 |
| Library | current carousel game's `screenshot_urls` / `url_screenshots`, behind a 500 ms single-shot debounce timer | grid_launcher/tv/widgets/views/library_view.py:84, grid_launcher/tv/widgets/views/library_view.py:324 |
| Details | the same list the screenshot column uses | grid_launcher/tv/widgets/views/details_view.py:813 |

Note the game dict's `fanart_url` field is carried through the catalog and merged in TV
metadata (grid_launcher/tv/bridge/app_backend.py:170) but is **not** what the fanart
widget displays.

Cycle behavior (grid_launcher/tv/widgets/components/fanart_background.py:65):

1. `set_urls` normalizes to non-blank strings, resets index and back buffer, stops any
   running animation and the cycle timer.
2. Empty list → repaint (leaving the last front image in place) and stop.
3. Load the first URL; start a 5000 ms repeating timer only when there is more than one
   URL (grid_launcher/tv/widgets/components/fanart_background.py:53).
4. Each tick advances the index modulo the list length and loads that URL
   (grid_launcher/tv/widgets/components/fanart_background.py:80).

On arrival (grid_launcher/tv/widgets/components/fanart_background.py:87): orphan check
first; `None`/null pixmaps are ignored; the image is blurred at radius 4 through an
offscreen graphics scene (grid_launcher/tv/widgets/components/fanart_background.py:16);
the first image becomes the front buffer immediately, later ones become the back buffer
and cross-fade in over 1000 ms with an `InOutQuad` curve, swapping to front on finish
(grid_launcher/tv/widgets/components/fanart_background.py:100).

Painting (grid_launcher/tv/widgets/components/fanart_background.py:120): fill with the
theme background, draw the front image centred and scaled with
`KeepAspectRatioByExpanding`, draw the back image on top at the animated opacity, then
overlay a flat `rgba(0,0,0,178)` scrim over the whole rect
(grid_launcher/tv/widgets/components/fanart_background.py:150).

#### Rust port (round 4)

The Python desktop window never had a background — the rewrite's shell (`BackgroundArt.svelte`)
adds one where none existed. It follows the TV widget's own cycle timing and its "more than one
URL" gate: `BACKGROUND_CYCLE_MS = 5000` and `shouldCycle(urls, fade) = urls.length > 1 && fade > 0`
mirror `fanart_background.py:52-53,80-84` almost exactly, the only difference being that the fade
slider (not just the URL count) can hold the rotation.

Unlike TV, the rewrite has a real fanart source instead of overloading the screenshot list:
`RomSSMetadata.fanart_path` and `RomGamelistMetadata.fanart_path`, read by
`fanart_urls_from_payload` (`crates/grid-core/src/images/urls.rs`) off both `SimpleRomSchema` and
`DetailedRomSchema` payloads — so the Server grid and the Library both carry fanart with no
per-card detail fetch, closing the gap this doc's own note above flags (TV carried `fanart_url`
through the catalog but never displayed it).

`NON_SCREENSHOT_ART_RE` is deliberately NOT applied to fanart URLs — that regex exists to keep
fanart, box art and logos OUT of a *screenshot* list; applying it to the fanart list would reject
every fanart by name (`looks_like_screenshot_url("/art/fanart.jpg")` is `false`).

`fanart_url` (usually an external host, since it points at ScreenScraper/LaunchBox art) is dropped
by `filter_to_server_host` like any other foreign URL, and that is intended: apart from the
YouTube trailer thumbnail (below), nothing may leave the server host. `fanart_path` (server
relative) is what actually survives in practice.

The priority used to choose the shell's background is fanart → screenshots → cover
(`backgroundUrls` in `app/src/lib/background.ts`). The cover is the last resort of the three: a
portrait cover stretched across a landscape window is worse than a screenshot or fanart image
that was already shot in a roughly landscape aspect.

## Invariants and error handling

- **No exception from image handling ever aborts a user flow, except uninstall.** Every
  fetch/decode/write path returns `""`/`None` on failure
  (grid_launcher/cover/cache.py:113, grid_launcher/background/workers.py:868,
  grid_launcher/tv/widgets/cover_loader.py:175). The single exception is unlink failure
  during uninstall, which surfaces a message box and returns `False`
  (grid-launcher.py:2947).
- **Only decodable payloads are written by the install-time path**
  (grid_launcher/cover/cache.py:44); the replenish worker does not enforce this
  (grid_launcher/background/workers.py:870), so a cached file may be undecodable and will
  simply produce a negative cache entry when later loaded.
- **Negative caching is permanent for the process lifetime** on desktop: a `None` entry
  is never retried (grid_launcher/cover/loader.py:48). The TV loader's `""` entries are
  likewise never retried, but its *path* entries self-heal when the file disappears
  (grid_launcher/tv/widgets/cover_loader.py:136).
- **A cached-file path is never validated before it is queued**
  (grid_launcher/cover/manager.py:65); staleness is discovered as a failed load.
- **A cover file can be shared by several library entries** whenever they share a
  `rom_id` (or title+platform); the `protected_cache_paths` set is what stops one
  uninstall from breaking another entry (grid_launcher/cover/manager.py:96).
- **Widget-lifetime guards** are pervasive: `RuntimeError` around label mutation on
  desktop (grid_launcher/cover/loader.py:39), orphan checks in every TV painted card and
  the fanart widget (grid_launcher/tv/widgets/components/fanart_background.py:89),
  and the `"already deleted"` filter in TV delivery
  (grid_launcher/tv/widgets/cover_loader.py:109). A port that keeps strong handles from
  callbacks to widgets must reproduce equivalent guards.
- **Host filtering is fail-open**: with no configured server URL, any host is fetched
  (grid_launcher/cover/utils.py:52). A port should not tighten this silently — the
  Discover path already bypasses filtering entirely
  (grid_launcher/server/discover.py:196).
- **Screenshot slots are capped at 5 on desktop** (grid-launcher.py:2117); TV renders all
  of them (grid_launcher/tv/widgets/views/details_view.py:840).

## Platform differences

The cover subsystem is almost entirely platform-neutral. The only platform-sensitive
points:

- The cache root is `Path.home() / ".grid-launcher" / "imagecache"` on every OS — there is
  no XDG or `%APPDATA%` branch (grid-launcher.py:2387, grid-launcher.py:2389), even though
  XDG helpers exist elsewhere in the codebase (grid_launcher/core/path.py:33).
- `path_key` case-folds paths, which matches Windows/macOS case-insensitive semantics but
  makes two case-differing paths collide on case-sensitive Linux filesystems
  (grid_launcher/core/path.py:15).
- Filenames are constrained to `[A-Za-z0-9._-]` plus a hash, so they are safe on every
  filesystem without further sanitizing (grid_launcher/cover/utils.py:216).
- `file://` URL construction and parsing go through Qt's `QUrl.fromLocalFile`
  (grid_launcher/cover/manager.py:36) on desktop and `url2pathname` in TV
  (grid_launcher/tv/widgets/cover_loader.py:142), both of which handle drive letters and
  percent-encoding; a port must use an equivalent, not naive string concatenation.
- Bundled platform logos are located relative to the package directory
  (grid_launcher/server/catalog.py:11) and exposed as `as_uri()` file URLs
  (grid_launcher/server/catalog.py:134), which percent-encodes spaces
  (tests/test_tv_server_platform_details.py:92).

## Concurrency

**Desktop.**

- All cover state (`cover_cache`, `cover_waiters`, `cover_loading`) is touched only from
  the UI thread; the network manager delivers `finished` on that thread
  (grid_launcher/cover/loader.py:61). No locking is used or needed.
- The in-flight set gives at-most-one request per URL; N labels waiting on the same URL
  share one response (grid_launcher/cover/loader.py:54).
- `queue_game_cover_load` intentionally defers delivery of an already-cached pixmap to
  the next event-loop turn (grid_launcher/cover/manager.py:62).
- `cache_cover_image_for_game` is **blocking on the UI thread** and runs inside the
  install flow with a 30 s timeout (grid_launcher/cover/cache.py:107).
- Exactly one replenish job may exist at a time, guarded by an `isRunning()` check
  (grid-launcher.py:2847). The worker runs on its own thread and communicates only via
  signals; it operates on snapshot copies of the game dicts, never the live ones
  (grid-launcher.py:2859).

**TV.**

- One unbounded daemon thread is spawned per `load_async` call
  (grid_launcher/tv/widgets/cover_loader.py:120). There is no pool, no queue, and no
  concurrency limit; a wall of visible cards produces that many simultaneous sockets.
- `_cache`, `_next_batch_id`, and `_cancelled_batches` are guarded by one mutex; each
  critical section is a single dict/set operation
  (grid_launcher/tv/widgets/cover_loader.py:127,
  grid_launcher/tv/widgets/cover_loader.py:158,
  grid_launcher/tv/widgets/cover_loader.py:194).
- Because there is no in-flight dedup, two threads may fetch and write the same file
  concurrently; the `if not cache_file.exists()` guard narrows but does not eliminate the
  race (grid_launcher/tv/widgets/cover_loader.py:192).
- All callbacks are marshalled to the GUI thread through a zero-delay timer parented to
  the application object (grid_launcher/tv/widgets/cover_loader.py:116).
- `_cancelled_batches` only ever grows for the life of the loader
  (grid_launcher/tv/widgets/cover_loader.py:73).

## Test oracle

| Test file | What it pins |
|---|---|
| tests/test_cover_manager.py:39 | `queue_game_cover_load` performs no synchronous `QPixmap` decode — the manager module's `QPixmap` is patched to raise |
| tests/test_cover_manager.py:54 | Exactly two queued loads in order: the `file:`-prefixed local URL, then the remote URL; nothing applied synchronously; file size is irrelevant |
| tests/test_screenshot_urls.py:13 | LaunchBox typed images keep only screenshot/title-screen types, in payload order |
| tests/test_screenshot_urls.py:36 | `gamelist_metadata` / `ss_metadata` contribute only `screenshot_url` and `title_screen_url`; `image_url` and `fanart_url` are excluded |
| tests/test_screenshot_urls.py:62 | Source ordering: `merged_screenshots`, then `url_screenshots`, then screenshot-typed `images` entries; box/fanart entries dropped |
| tests/test_screenshot_urls.py:92 | `screenshot_urls_from_game` filters stale box-art/fanart lines out of a stored newline blob |
| tests/test_screenshot_urls.py:114 | Host filtering: foreign host rejected with a base URL, allowed with an empty base URL, allowed with an unparseable base URL, port mismatch rejected, relative URL resolves and passes |
| tests/test_cover_details_layout.py:120 | Cover height stays ≤ 440 for 1280×720 windows |
| tests/test_cover_details_layout.py:128 | Cover width and description width shrink below the old floors on small windows |
| tests/test_cover_details_layout.py:138 | Screenshots panel hidden for tight non-overview cloud layouts, including the 1280×720 case |
| tests/test_cover_details_layout.py:164 | Screenshots panel visible when space allows |
| tests/test_cover_details_layout.py:177 | Screenshot labels get a max width, never a fixed size |
| tests/test_cover_details_layout.py:191 | Font scale: ≤ 24 px at 720p, exactly the base 30 px at 1080p, > 30 px and ≤ 75 px at 1440p |
| tests/test_tv_image_provider.py:42 | `load_pixmap` returns the image from a seeded cached path |
| tests/test_tv_image_provider.py:53 | Empty and `None` URLs return `None` without touching the network |
| tests/test_tv_image_provider.py:61 | HTTP fetch happens exactly once on a cache miss |
| tests/test_tv_image_provider.py:77 | Fetch failure returns `None` |
| tests/test_tv_image_provider.py:87 | A stale cached path falls back to HTTP |
| tests/test_tv_image_provider.py:113 | Importing the TV loader must not import `grid_launcher.cover.loader` |
| tests/test_background_workers.py:1616 | Replenish worker skips empty cover URLs and still emits `finished` |
| tests/test_background_workers.py:1628 | Replenish worker makes no network call when the cached file exists |
| tests/test_background_workers.py:1645 | Replenish worker writes the file and emits `{game_key, path}` |
| tests/test_background_workers.py:1668 | `finished` is emitted exactly once for a mixed batch |
| tests/test_background_workers.py:1694 | `HTTPError` during fetch skips the game and still emits `finished` |
| tests/test_game_wall_batching.py:28 | All cards enter the grid immediately; only visible rows populate |
| tests/test_game_wall_batching.py:53 | A second `set_games` cancels the previous batch id |
| tests/test_tv_details_view.py:86 | A loaded screenshot sets its card height from the pixmap aspect ratio |
| tests/test_tv_server_platform_details.py:77 | Known platform slugs produce a percent-encoded local logo file URL; unknown slugs produce `""` |

## Open questions

- `OPEN QUESTION:` `MAX_CACHED_COVER_BYTES` (grid_launcher/cover/manager.py:11) is
  referenced only by a test (tests/test_cover_manager.py:8). Was a size guard intended,
  and should a port implement one, or drop the constant?
  **RULED (milestone 7): dropped — see "Rust port deviations (milestone 7)" D9.**
- `OPEN QUESTION:` There is no cache eviction of any kind — the `imagecache` directory
  grows without bound and files orphaned by a failed uninstall or by a changed
  `installed_cover_cache_key` basis are never collected
  (grid_launcher/cover/manager.py:84 is the only deletion path). Should a port add a
  sweep, and if so keyed on what?
  **RULED (milestone 7): yes — a bounded 512 MiB cache with an oldest-unpinned-first
  startup sweep — see "Rust port deviations (milestone 7)" D3.**
- `OPEN QUESTION:` The desktop async loader sends no `Authorization` header
  (grid_launcher/cover/loader.py:58) while the install-time path and the TV loader both do
  (grid_launcher/cover/cache.py:101, grid_launcher/tv/widgets/cover_loader.py:167). Is
  the RomM asset route expected to be unauthenticated (cookie/session only), or is this a
  latent failure for token-only deployments?
  **RULED (milestone 7): latent failure — every image fetch is authenticated through the
  RomM client — see "Rust port deviations (milestone 7)" D2.**
- `OPEN QUESTION:` The desktop pipeline writes files named by game identity while the TV
  pipeline writes files named by URL hash, both into the same directory
  (grid_launcher/cover/utils.py:205 vs grid_launcher/tv/widgets/cover_loader.py:152). The
  same image is therefore stored twice. Should a port unify the schemes, and if so which
  one wins?
  **RULED (milestone 7): unified on the URL-hash scheme — see "Rust port deviations
  (milestone 7)" D1.**
- `OPEN QUESTION:` `queue_game_cover_load` issues both a local and a remote request for
  the same label whenever a cached path exists, with no ordering guarantee about which
  one paints last (grid_launcher/cover/manager.py:65). Is the remote fetch meant to win
  (freshness) or the local one (speed/offline)?
  **RULED (milestone 7): no equivalent — the webview owns decoded images, so there is no
  local/remote race to resolve — see "Rust port deviations (milestone 7)" D9.**
- `OPEN QUESTION:` `cleanup_cached_cover_for_game` drops the two `file`-flavoured cache
  keys but leaves the remote-URL entry in `cover_cache`
  (grid_launcher/cover/manager.py:93). Intentional (the remote image is still valid) or
  an oversight?
  **RULED (milestone 7): moot — see "Rust port deviations (milestone 7)" D9; there is no
  in-memory `cover_cache` structure to leave a stale entry in.**
- `OPEN QUESTION:` `MissingCoverReplenishWorker` writes payloads without a decode check
  (grid_launcher/background/workers.py:870) whereas the install path requires a successful
  decode (grid_launcher/cover/cache.py:44). Should the worker validate too?
  **RULED (milestone 7): both paths get the same content gate instead of a decode check —
  see "Rust port deviations (milestone 7)" D8 and D6.**
- `OPEN QUESTION:` After replenishment updates `cached_cover_path` and saves the config,
  no view refresh or cache invalidation is triggered (grid-launcher.py:2887). Should the
  library grid re-render so the newly fetched covers appear without a restart?
  **RULED (milestone 7): yes — replenish emits an event the Library listens for and
  re-renders on — see "Rust port deviations (milestone 7)" D6.**
- `OPEN QUESTION:` `PlatformCard.set_platform` falls back to a `logo_url` key
  (grid_launcher/tv/widgets/components/platform_card.py:32), but the catalog emits
  `url_logo` (grid_launcher/server/catalog.py:144). The remote-logo fallback therefore
  never fires. Is the key name a bug?
  **DEFERRED (milestone 7): TV mode is not yet in the rewrite; revisit with the TV mode
  redesign.**
- `OPEN QUESTION:` `AppBackend` stores `image_cache_dir`
  (grid_launcher/tv/bridge/app_backend.py:82) but never reads it. Dead parameter, or a
  hook a port is expected to keep?
  **DEFERRED (milestone 7): TV mode is not yet in the rewrite; revisit with the TV mode
  redesign.**
- `OPEN QUESTION:` The TV `CoverLoader`'s URL→path map is a one-time snapshot taken when
  the TV window is first constructed (grid-launcher.py:714) and is never re-synced on
  `syncConfig`. Games installed while in TV mode fall back to network fetches. Intended?
  **DEFERRED (milestone 7): TV mode is not yet in the rewrite; revisit with the TV mode
  redesign.**
- `OPEN QUESTION:` TV spawns one unbounded thread per image request
  (grid_launcher/tv/widgets/cover_loader.py:120) with no dedup. Should a port bound
  concurrency, and what limit preserves the current scroll responsiveness?
  **DEFERRED (milestone 7): TV mode is not yet in the rewrite; revisit with the TV mode
  redesign.**
- `OPEN QUESTION:` The fanart widget is fed screenshot URLs while a `fanart_url` field
  exists on the game dict and is merged into TV metadata
  (grid_launcher/tv/bridge/app_backend.py:170). Should fanart prefer `fanart_url` when
  present?
  **DEFERRED (milestone 7): TV mode is not yet in the rewrite; revisit with the TV mode
  redesign.**
- `OPEN QUESTION:` `FanartBackground.set_urls([])` stops the timer but leaves the last
  front pixmap painted (grid_launcher/tv/widgets/components/fanart_background.py:73).
  Should an empty list clear the background instead?
  **DEFERRED (milestone 7): TV mode is not yet in the rewrite; revisit with the TV mode
  redesign.**
- `OPEN QUESTION:` Discover cards bypass `resolve_cover_url` and
  `filter_to_server_host` entirely (grid_launcher/server/discover.py:196), so a relative
  `path_cover_large` is handed to the loader unresolved and a foreign host is not
  filtered. Is Discover meant to use the same rules as the catalog?
  **RULED (milestone 7): out of scope — Discover is not yet in the rewrite.**
- `OPEN QUESTION:` `_looks_like_screenshot_url` defaults to "screenshot" for any URL with
  no recognizable token (grid_launcher/cover/utils.py:25), so opaque asset URLs
  (`/assets/…/1234.png`) always pass. Is permissive-by-default the desired bias?
  **RULED (milestone 7): reproduced as-is — see "Rust port deviations (milestone 7)",
  Follow-the-code quirks.**
- `OPEN QUESTION:` The desktop details view is hard-limited to 5 screenshot slots
  (grid-launcher.py:2117) while TV shows all. Should the desktop limit be configurable?
  **RULED (milestone 7): dropped — no cap and no width gate — see "Rust port deviations
  (milestone 7)" D7.**

## Source map

| Path | Role |
|---|---|
| grid_launcher/cover/utils.py | Pure helpers: screenshot/non-screenshot regexes, `resolve_cover_url`, `filter_to_server_host`, `cover_url_from_rom_payload`, `screenshot_urls_from_rom_payload`, `screenshot_urls_from_game`, `cached_cover_path_from_game`, `cached_cover_cache_key`, `installed_cover_cache_key`, `cover_cache_extension_from_payload` |
| grid_launcher/cover/cache.py | Blocking install-time fetch + disk write, decode gate, PNG re-encode fallback from an on-screen pixmap |
| grid_launcher/cover/loader.py | Desktop async pipeline: `apply_cover_to_label`, `queue_cover_load` (dedup + waiter lists), `on_cover_reply` (negative caching, fan-out) |
| grid_launcher/cover/manager.py | `MAX_CACHED_COVER_BYTES`, local-file URL construction, memory lookup, `queue_game_cover_load` (local + remote double queue), protected-path set, cache-entry drop and unlink |
| grid_launcher/cover/details.py | `resolved_cover_url_for_game`, screenshot slot binding, details layout metrics (cover/screenshot/description sizing, font scaling), media rescale on resize |
| grid_launcher/cover/__init__.py | Public re-export surface of the package |
| grid_launcher/background/workers.py | `MissingCoverReplenishWorker` only (rest belongs to doc 08) |
| grid_launcher/tv/widgets/cover_loader.py | Independent TV image provider: URL-hash disk cache, `file://` branch, host-scoped bearer auth, per-request threads, batch create/cancel, `QImage` payload validation |
| grid_launcher/tv/widgets/components/fanart_background.py | Blurred cross-fading background driven by screenshot URLs and the TV loader |
| grid_launcher/tv/widgets/components/game_wall.py | The only batched TV consumer: viewport-driven population, batch cancel on re-populate |
| grid_launcher/tv/widgets/components/game_row.py, home_card.py, game_card.py, platform_card.py | Unbatched TV consumers; orphan-guarded `set_pixmap` placeholder behavior |
| grid_launcher/tv/widgets/views/home_view.py, library_view.py, details_view.py | Fanart URL sources, library carousel pool binding/recycling, TV details cover + screenshot column + lightbox |
| grid_launcher/ui/game_views.py | `AspectRatioLabel` (`set_source_pixmap`, height-for-width, resize rescale), game card cover widget, details-open cover queue |
| grid_launcher/ui/mixins/install_mixin.py | Calls `_cache_cover_image_for_game` at install; provides `_path_key` |
| grid_launcher/library/install_registry.py | Persists `cover_url`, `cached_cover_path`, `screenshot_urls` onto the installed record |
| grid_launcher/library/install_cleanup.py | Builds the protected-path set and calls the cover cleanup during uninstall |
| grid_launcher/library/identity.py | `games_match_identity`, used by the details-pixmap fallback |
| grid_launcher/core/path.py | `path_key` (expanduser + resolve + casefold) used for cache-key canonicalization |
| grid_launcher/core/config.py | Normalizes `cover_url` / `cached_cover_path` / `screenshot_urls` on load |
| grid_launcher/server/catalog.py | Produces `cover_url` and newline-joined `screenshot_urls` per game; builds bundled platform logo file URIs |
| grid_launcher/server/discover.py | Discover-card cover selection (unresolved, unfiltered) and empty screenshot list |
| grid-launcher.py | `MainWindow` glue: cache dir, cover cache/waiter/loading state, network manager, `_resolve_cover_url` composition, replenish job lifecycle, 5 screenshot label slots, TV `CoverLoader` construction and URL→path snapshot |

## Rust port deviations (milestone 7)

Deliberate deviations, and rulings on ambiguous or defective reference behavior, made while
porting covers and screenshots (URL resolution, the disk cache, the startup sweep, install-time
prefetch, replenishment, and the Details screenshot strip) to Rust (grid-core's `images/` module
— `urls.rs`, `cache.rs`, `sweep.rs`, `replenish.rs` — the Tauri `app/src-tauri/src/images.rs`
glue and `commands.rs`, and the `app/src/lib/` Image, Library, Details, and Shell components).
Rust paths are relative to `rewrite/`. D1-D10 restate the deviations already declared by the
covers/images design task (`docs/superpowers/specs/2026-09-02-covers-images-design.md`,
"Deviations" §D1-D10) for completeness; D11 and D12 are new to this milestone's review.

1. **D1 — One filename scheme for every image: SHA-256 of the resolved URL.** Python used an
   identity hash on desktop and a URL hash on TV, into one directory. Nothing reads the Python
   cache, so no compatibility is lost. `image_key` (`crates/grid-core/src/images/cache.rs:30`) is
   the single scheme; every cached file is named `<sha256(resolved url)>.<ext>`
   (`crates/grid-core/src/images/cache.rs:153`).
2. **D2 — Every image fetch is authenticated through the RomM client.** Python's desktop async
   loader sent no `Authorization` header (doc 07 open question ruled: token-only servers must
   work). `fetch_and_store` fetches through `RommClient::get_bytes_with_type`
   (`crates/grid-core/src/images/cache.rs:137`, `crates/grid-core/src/romm/mod.rs:133`), which
   attaches the same bearer token as every other authenticated request; there is no unauthenticated
   image path.
3. **D3 — Bounded cache: 512 MiB cap, startup sweep, oldest-unpinned-first, installed rows' small
   and large covers pinned.** Uninstall deletes no files and cannot fail on image cleanup. Python
   never evicted and unlinked on uninstall with a protected-path set. `sweep`
   (`crates/grid-core/src/images/sweep.rs:43`) deletes the least-recently-modified unpinned files
   under `IMAGE_CACHE_CAP_BYTES` (`crates/grid-core/src/images/sweep.rs:11`, 512 * 1024 * 1024);
   `pinned_keys` (`crates/grid-core/src/images/sweep.rs:23`) pins every installed row's cover
   paths. The sweep runs once, at startup — see the Rulings below.
4. **D4 — At most 6 concurrent image downloads; 30 s per-fetch timeout on every path.** Python:
   unbounded async loads, 30 s only on blocking paths. `MAX_CONCURRENT_DOWNLOADS = 6`
   (`crates/grid-core/src/images/cache.rs:15`) bounds a `Semaphore` acquired before every fetch
   (`crates/grid-core/src/images/cache.rs:132-137`); the RomM HTTP client's 30 s timeout
   (`crates/grid-core/src/romm/mod.rs:49`) applies uniformly, install-time and replenish alike.
5. **D5 — The install-time cover fetch is non-blocking.** Python blocked the UI thread for up to
   30 s; the PNG re-encode fallback from an on-screen pixmap is dropped (no pixmap exists outside
   the webview). `ImageService::spawn_prefetch` (`app/src-tauri/src/images.rs:89-103`) fetches the
   small and large covers on a spawned async task; install itself never waits on it.
6. **D6 — Replenish also back-fills the three image fields from `rom_detail` for rows that lack
   them, and emits an event that re-renders the Library.** Python only refetched files and
   triggered no refresh. `replenish::plan`/`run` (`crates/grid-core/src/images/replenish.rs:32`,
   `:51`) back-fill `ImageFields` via `Registry::update_images`
   (`crates/grid-core/src/images/replenish.rs:70`) before fetching a file; the Tauri glue emits
   `images-replenished` (`app/src-tauri/src/images.rs:17`, `:85`), which the Library's frontend
   listener re-renders on.
7. **D7 — No screenshot cap and no width gate that hides screenshots; the strip collapses under
   the description on narrow panels.** Python capped at 5 and hid the column below 1360 px.
   `Details.svelte` renders every URL in `screenshotUrls` with no slice
   (`app/src/lib/Details.svelte:229-243`); a container query
   (`app/src/lib/Details.svelte:577`) reflows the layout under the description instead of hiding
   the strip. The three-column threshold is a 900 px query on the *panel* container
   (`app/src/lib/Details.svelte:362`, `:577`), not the ≥ 1100 px panel width the design spec
   named: the panel's content box is 1052 px at its full 1100 px width, so a 1100 px threshold
   would never fire.
8. **D8 — Content gate replaces the decode gate: a body is written only if Content-Type or magic
   bytes identify an image.** Python required a successful `QPixmap` decode on the install path
   and nothing on the replenish path. `fetch_and_store`'s gate
   (`crates/grid-core/src/images/cache.rs:142-150`) applies to every caller — install prefetch and
   replenish alike — and rejects on neither successful nor failed image decode, only on
   unidentifiable content; the write itself is atomic (`.part` file, then rename,
   `crates/grid-core/src/images/cache.rs:154-156`).
9. **D9 — `MAX_CACHED_COVER_BYTES` is dropped (dead in Python).** The in-memory pixmap cache,
   waiter lists, local-then-remote double queue, `file:` alias keys, and `path_key` case-folding
   have no equivalent: the webview owns decoded images. `ImageCache`'s only state is the in-flight
   map, the per-session negative-result map, and the disk itself
   (`crates/grid-core/src/images/cache.rs:30-49`) — there is no in-memory image cache to bound,
   race, or key by anything but the URL.
10. **D10 — The details layout metrics (fixed 1.35 aspect math, font scaling by window height) are
    not ported; CSS handles sizing.** `Details.svelte`'s `<style>` block
    (`app/src/lib/Details.svelte:330-630`) uses container queries (`:362`, `:577`) and CSS
    `aspect-ratio` (`:392`) instead of JS-computed pixel metrics recalculated on resize.

11. **D11 — the SVG sniff lowercases instead of calling a nonexistent `bytes.casefold()`.**
    Python's `cover_cache_extension_from_payload` calls `preview.casefold()` on a `bytes` object
    (`grid_launcher/cover/utils.py:255`) — `bytes` has no `casefold` method in Python, so this
    branch raises `AttributeError` the moment an `<?xml`-prefixed SVG body is actually sniffed; no
    test exercises it. `extension_for`'s SVG branch
    (`crates/grid-core/src/images/urls.rs:457-467`) lowercases with `to_ascii_lowercase()` instead,
    so an `<?xml version="1.0"?><SVG>` body is correctly identified rather than crashing.

12. **D12 — `cover_url_from_payload` is not wired into `into_detail`.** The design spec's cover
    fallback for a rom row whose `path_cover_small` and `path_cover_large` are both empty (walk
    the remaining RomM cover keys) is implemented and unit-tested
    (`crates/grid-core/src/images/urls.rs:248`) but no production caller uses it: `into_detail`
    (`crates/grid-core/src/romm/mod.rs:387`) takes the two path fields directly. The keys the
    fallback would reach are foreign-host IGDB/SteamGridDB URLs, which `filter_to_server_host`
    discards anyway, so wiring it in would change no rendered cover. The function is kept for a
    future server that serves those keys from its own host.

### Follow-the-code quirks (ported as-is)

Reproduced verbatim from the design spec's "Follow-the-code quirks" — where the reference's own
behavior is internally inconsistent or arguably wrong, the port follows the CODE:

- Host filtering compares whole netlocs: a port mismatch rejects, no case folding.
- Screenshot heuristic defaults to "screenshot" for unlabelled URLs.
- Screenshot source order and the `images`-only type rule, including "non-list value appended as
  a single item".
- Stored screenshot lists are re-filtered on read.
- Negative image results are never retried within a process.
- Replenish runs one job at a time; a second trigger while running is dropped.
- Library sort and the hidden `emulator|emulators` platform.

### Rulings

Additional decisions made during execution, not individually numbered as deviations because they
resolve implementation questions the design left open rather than diverging from a stated Python
behavior:

- `ensure_image` (`app/src-tauri/src/commands.rs:176`) takes only a URL — the cache key is the
  URL; which cover variant (small/large) it is stays a frontend concern.
- The startup sweep runs synchronously inside Tauri `setup`, before any command runs
  (`ImageService::sweep_at_startup`, `app/src-tauri/src/images.rs:40-41`, called from
  `app/src-tauri/src/lib.rs:132`) — no async gate delays the shell on it.
- Downloads and Emulators keep the existing footer drawer and overlay, reachable from the new top
  bar (`nav-downloads`, `nav-emulators`, `app/src/lib/Shell.svelte:48-49`); they were not
  converted into sections alongside Library and Server.
- The default section is Server when the shell renders connected and Library when it renders
  offline (`initialSection`, `app/src/lib/shell.ts:26`).
- The E2E offline scenario uses a mock toggle (`POST /__e2e__/offline`,
  `e2e/mock-romm/server.mjs:318-322`) rather than an actual network failure.
- The mock's rom 102 detail fixture carries `path_cover_small`
  (`e2e/fixtures/rom-details.json:41`) because replenish only fetches a cover for a row
  whose back-filled detail has one.
- Replenish also treats a row that vanished between planning and running
  (`Registry::update_images` returning no matching row) as skipped, without attempting a fetch
  (`crates/grid-core/src/images/replenish.rs:69-72`).
- **The background art has a variant; the cover pipeline does not.** `ensure_image` still returns
  the raw cached bytes for every card and screenshot; only the shell background asks for
  `ensure_background_variant`. One extra variant per background source, not per image.
- **A failed variant keeps the current art.** There is no raw-image fallback: the CSS blur is
  gone, so the raw source would be a different effect rather than a degraded one.
- **The YouTube trailer thumbnail is the only foreign host.**
  `https://img.youtube.com/vi/<id>/hqdefault.jpg`, a plain `<img>` with
  `referrerpolicy="no-referrer"`, allowed by `img-src` in `app/src-tauri/tauri.conf.json`. It is
  deliberately NOT routed through `ensure_image`, which would fetch it via `RommClient` and attach
  the RomM Authorization header to a foreign request. On error it falls back to the server-hosted
  cover with the same play badge.
- **`noteViewed` gates on subject equality.** Landing alongside this documentation pass: reporting
  the same `BackgroundSubject` twice — for example `Details.svelte`'s effect re-firing because an
  unrelated field on `merged` changed while the popup stays open on the same game — no longer
  overwrites `state.subject`, so `BackgroundArt.svelte`'s cycle index and its 5000ms interval are
  not reset by a report that carries no new art.

## Game videos (rewrite only)

`DetailedRomSchema.path_video` is a file on the RomM server, not an image, so it cannot
go through `ImageCache::ensure` — that gate rejects any body that is not an image, which
is the correct behaviour for covers and the wrong one here. `images::video::ensure_video`
reuses the same directory and the same `sha256(resolved url)` key scheme with its own
content gate (Content-Type, then the `ftyp` / EBML magic bytes), storing the file as
`<key>.mp4` / `.webm` / `.mov`. The startup sweep keys off the file stem, so a cached
video is an ordinary unpinned entry: evictable, and refetched on the next view.

The bytes are fetched through the session's `RommClient`, exactly like a cover, and the
frontend only ever receives the resulting local path. No video URL in the UI carries a
token. `youtube_video_id` is a different case entirely — it is embedded, touches no
server bytes, and needs the `frame-src https://www.youtube-nocookie.com` CSP entry to
render at all.

## Background variant (rewrite only)

`images::background::ensure_background_variant` (`crates/grid-core/src/images/background.rs`)
reuses `ImageCache::ensure` to fetch the source and stores `<key>.bg.jpg` beside it, the same
directory and the same `sha256(resolved url)` key scheme `video.rs` uses (`image_key`).

The variant is 960px wide (never upscaled, `FilterType::Triangle`), blurred once with
`fast_blur` at sigma 12.0, and encoded as JPEG at quality 80. It is written through a `.bg.part` +
rename, a temp name distinct from `ImageCache`'s own `<key>.part` so a concurrent fetch of the
same source cannot rename its half-written JPEG over the variant, or vice versa.

`sweep::pinned_keys` pins by key PREFIX, so `<key>.bg.jpg` is pinned whenever its source `<key>`
is — a variant is never evicted out from under an installed game's art.

It is built ahead of time in two places — `spawn_prefetch` (install, `app/src-tauri/src/images.rs`)
and `replenish::plan`'s `NeedsVariant` items (a game connected to the library, planned last, after
every cover item) — and on demand when the frontend hovers a card for 150ms
(`PREFETCH_DELAY_MS` in `app/src/lib/background.ts`, before the 500ms `noteViewed` swap).

The reason it exists at all: `BackgroundArt.svelte` used to hand a full-resolution cover to two
`filter: blur(40px)` layers, blurring the same ~2.4 Mpx image on the compositor every frame of the
360ms cross-fade. That is Python's TV `_blur_pixmap` (`fanart_background.py:16`) done per frame
instead of once on arrival — the variant moves the blur to Rust, once, so the webview only ever
composites a small pre-blurred still.
