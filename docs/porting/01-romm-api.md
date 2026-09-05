# RomM API and external HTTP services — porting behavior

## Purpose

This document describes how GRID Launcher talks to its RomM server and to two secondary
external services (RetroAchievements, PCGamingWiki), so the behavior can be reimplemented
in another language without reading the Python source.

Scope covered here:

- The low-level request helpers (`grid_launcher/core/api.py`) — auth headers, URL/query
  building, timeouts, multipart encoding, error message formatting, TLS trust store.
- The server layer (`grid_launcher/server/`) — connection/auth state, platform and ROM
  catalog fetching and normalization, ROM detail caching, metadata merging, the Discover
  data flow with its caching layers and on-disk files, and the static platform metadata
  tables.
- The secondary clients `grid_launcher/server/retroachievements.py` and
  `grid_launcher/server/pcgamingwiki.py`.
- Every RomM endpoint the client actually calls, cross-checked against the checked-in
  server description `openapi.json` (RomM API 5.2.0, `openapi.json:1`). Every cross-check below
  was re-run against that spec revision.

Out of scope, noted only so a porter knows it exists: emulator-source updates fetch GitHub
/Gitea release metadata and binaries over HTTP from `grid_launcher/background/workers.py:197`
and `grid_launcher/background/workers.py:479`; PS3 firmware resolution fetches Sony's update
manifest (`grid_launcher/library/firmware_install.py:22`). Widget rendering is out of scope;
only the server-data contracts of `grid_launcher/server/view.py` are described.

## External surfaces

### RomM endpoints used by the client

All paths are appended to the configured base URL (trailing slashes stripped,
`grid_launcher/server/state.py:18`). All carry the bearer auth header
(`grid_launcher/core/api.py:15`). "Spec" column records the cross-check against
`openapi.json`.

| # | Method | Path | Query params sent | Client use of the response | Spec |
|---|--------|------|-------------------|----------------------------|------|
| 1 | GET | `/api/users/me` | none | Reads `username` (string, trimmed); stored as the display identity. Non-object payload or non-string username yields empty and is ignored (`grid_launcher/server/catalog.py:14`, `grid_launcher/server/orchestrator.py:7`) | present, 200 only |
| 2 | GET | `/api/platforms` | none | Expects a JSON array. Reads `id`, `slug`, `name`/`display_name`, `rom_count`, `url_logo` per entry (`grid_launcher/server/catalog.py:37`, `:67`, `:98`) | present; spec also allows `updated_after`, unused |
| 3 | GET | `/api/roms` | `platform_ids[]`, `limit`, `offset`, `with_char_index=false`, `with_filter_values=false` (`grid_launcher/server/catalog.py:163`) | Paged catalog load; reads `items`, `total` (`grid_launcher/server/catalog.py:175`, `:183`) | present, all params in spec |
| 3b | GET | `/api/roms` | `limit`, `with_char_index=false`, `with_filter_values=true` (`grid_launcher/server/discover.py:256`) | Discover "all games" + genre list from `filter_values.genres` | present |
| 3c | GET | `/api/roms` | `limit=100`, `with_char_index=false`, `with_filter_values=true`, `metadata_providers=["hltb"]` (`grid_launcher/server/discover.py:280`) | Discover "Short But Fun"; reads `hltb_metadata.main_story` per item | present |
| 3d | GET | `/api/roms` | `genres=[<genre>]`, `limit` (+ the two `with_*=false` defaults) (`grid_launcher/server/discover.py:321`, `:157`) | Discover genre carousels and recommendation candidates | present |
| 3e | GET | `/api/roms` | `genres=[<genre>]`, `limit=1`, `with_char_index=false`, `with_filter_values=false` (`grid_launcher/server/discover.py:382`) | Reads only `total` to label genre pills | present |
| 3f | GET | `/api/roms` | `order_by=average_rating`, `order_dir=desc`, `limit` (`grid_launcher/server/discover.py:480`) | Discover "Highly Rated"; client re-filters to rating ≥ 4.0 | params present; value of `order_by` not enumerated in spec |
| 3g | GET | `/api/roms` | `order_by=created_at`, `order_dir=desc`, `limit` (`grid_launcher/server/discover.py:631`) | Discover "New on Server" | as above |
| 3h | GET | `/api/roms` | `platform_ids=[id]`, `order_by=average_rating`, `order_dir=desc`, `limit=8` (`grid_launcher/server/discover.py:507`) | Discover per-platform carousels | present |
| 4 | GET | `/api/roms/{id}` | none | Full ROM detail. Used to resolve the real download filename (`grid_launcher/server/details_cache.py:75`) and to backfill missing detail metadata (`grid_launcher/background/workers.py:771`) | present; 200/404/422 |
| 5 | GET | `/api/roms/{id}/content/{file_name}` | optional `file_ids=<id>` | Raw ROM/archive bytes written to disk (`grid_launcher/ui/mixins/install_mixin.py:1012`, `:1019`, `:1023`); also used with literal `game.json` name plus `file_ids` (`grid_launcher/ui/mixins/install_mixin.py:931`) | present; `file_ids` is a spec query param |
| 6 | GET | `/api/saves` | `rom_id` (`grid_launcher/ui/mixins/cloud_mixin.py:1593`); or none for a bulk sweep (`grid_launcher/tv/bridge/workers.py:185`) | Array of save records; reads `id`, `rom_id`, `file_name`, `download_path`, timestamps | present |
| 7 | POST | `/api/saves` | `rom_id` (required), `emulator`, `overwrite=true`, optional `slot` (`grid_launcher/ui/mixins/cloud_mixin.py:2607`) | multipart body field `saveFile` (+ optional `screenshotFile`); response parsed as JSON and otherwise unused | present; field names match `Body_add_save_api_saves_post` |
| 8 | GET | `/api/saves/{id}/content` | none | Raw save bytes (`grid_launcher/ui/mixins/cloud_mixin.py:1775`) | present |
| 9 | POST | `/api/saves/delete` | none | JSON body `{"saves": [<int id>, ...]}`; 404/410 treated as success (`grid_launcher/ui/mixins/cloud_mixin.py:1745`) | present; body shape matches spec |
| 10 | GET | `/api/states` | `rom_id` (`grid_launcher/ui/mixins/cloud_mixin.py:1651`) | Array of state records; entries whose `file_name` ends in an image extension are dropped client-side | present |
| 11 | GET | `/api/states/{id}` | none | Single state record; the client derives candidate content URLs from it (`grid_launcher/ui/mixins/cloud_mixin.py:1779`) | present |
| 12 | POST | `/api/states` | `rom_id`, `emulator` only (`grid_launcher/ui/mixins/cloud_mixin.py:2478`, `:2606`) | multipart field `stateFile` (+ optional `screenshotFile`) | present; spec accepts exactly `rom_id`+`emulator` |
| 13 | POST | `/api/states/delete` | none | JSON body `{"states": [ids]}` (`grid_launcher/ui/mixins/details_view_mixin.py:1303`) | present |
| 14 | GET | `/api/firmware` | `platform_id` (`grid_launcher/library/firmware_install.py:28`) | Expects an array; non-array yields empty list. Reads `id`, `file_name` | present |
| 15 | GET | `/api/firmware/{id}/content/{file_name}` | none | Raw firmware bytes (`grid_launcher/library/firmware_install.py:35`) | present |
| 16 | GET | `/api/collections` | none | Expects an array; picks the first entry with `is_favorite == true`, reads `id` and `rom_ids` (`grid_launcher/tv/bridge/workers.py:206`) | present |
| 17 | POST | `/api/collections` | `is_favorite=true` | multipart text fields `name="Favorites"`, `rom_ids=<JSON array string>` (`grid_launcher/tv/bridge/workers.py:268`) | present; `rom_ids` is specified as a JSON-array-in-a-string |
| 18 | PUT | `/api/collections/{id}` | none | multipart text field `rom_ids=<JSON array string>` (`grid_launcher/tv/bridge/workers.py:246`) | present |

Endpoint count: 18 distinct RomM method+path pairs (`/api/roms` GET is one path used with
eight different parameter shapes).

The server also advertises HTTP Basic as an alternative security scheme on these operations
(for example `/api/roms` GET lists `OAuth2PasswordBearer` and `HTTPBasic`), but the client
only ever emits a bearer header (`grid_launcher/core/api.py:16`). There is no Basic-auth code
path anywhere in the client.

### Secondary external services

| Service | Method | URL | Auth | Response use |
|---------|--------|-----|------|--------------|
| RetroAchievements login | GET | `https://retroachievements.org/dorequest.php?r=login&u=<user>&p=<password>` | credentials in query string | Reads `Success`, `User`, `Token` (`grid_launcher/server/retroachievements.py:75`). Rewrite: `crates/grid-core/src/retroachievements.rs` (`ra_login`), reached by the `retroachievements_login` command; the token goes to the keyring only and the password is never persisted |
| RetroAchievements, authenticated | GET | `https://retroachievements.org/API/API_GetGameInfoAndUserProgress.php?u=<user>&y=<api_key>&g=<game_id>` | username + web API key in query | Reads the `Achievements` map, including per-achievement earned dates (`grid_launcher/server/retroachievements.py:99`) |
| RetroAchievements, anonymous | GET | `https://retroachievements.org/API/API_GetGameExtended.php?g=<game_id>` | none | Same `Achievements` map, without earned dates (`grid_launcher/server/retroachievements.py:106`) |
| PCGamingWiki page lookup | GET | `https://www.pcgamingwiki.com/w/api.php?action=query&titles=<title>&prop=info&format=json` | none | Extracts the first non-missing page id (`grid_launcher/server/pcgamingwiki.py:185`, `:190`) |
| PCGamingWiki search fallback | GET | `…/api.php?action=opensearch&search=<title>&namespace=0&limit=3&format=json` | none | Reads element 3 of the OpenSearch array (list of URLs), takes the first (`grid_launcher/server/pcgamingwiki.py:229`) |
| PCGamingWiki wikitext | GET | `…/api.php?action=parse&pageid=<id>&prop=wikitext&format=json` | none | Reads `parse.wikitext.*` as a string (`grid_launcher/server/pcgamingwiki.py:250`) |

Both secondary clients send a `User-Agent` of `grid-launcher/1.0 (retroachievements-client)`
(`grid_launcher/server/retroachievements.py:25`) or `grid-launcher/1.0 (pcgamingwiki-client)`
(`grid_launcher/server/pcgamingwiki.py:137`), and no auth headers.

### Files read and written

All under a per-user config directory: `<home>/.grid-launcher` (`grid-launcher.py:2387`).

| File | Format | Written when | Read when |
|------|--------|--------------|-----------|
| `config.json` | JSON object; holds `server_url`, `username`, `details_rom_id_cache`, `retroachievements_username`. The secret fields `api_token`, `retroachievements_token`, and `retroachievements_api_key` are forced to `""` on every write (`grid_launcher/core/config.py:253-258`) — secrets never reach this file; see doc 02 | on settings save (`grid-launcher.py:3155`) | at startup |
| `token.bin` | Legacy secret file: DPAPI-encrypted blob on Windows, base64 text elsewhere (`grid_launcher/core/token_store.py:112`) | only as a Windows fallback when the OS keyring refuses (`grid_launcher/core/token_store.py:147`) | migrated into the keyring on first read, then deleted (`grid_launcher/core/token_store.py:159`) |
| `ra_token.bin`, `ra_api_key.bin` | same legacy format, different keyring accounts (`grid_launcher/core/token_store.py:13`) | as above | as above |
| `discover_cache.json` | JSON object: `{section_id: {"data": <object>, "timestamp": <epoch seconds>}}` (`grid_launcher/server/discover.py:78`, `:119`) | after a successful live Discover render (`grid-launcher.py:1412`) | at startup with a 7-day max age (`grid-launcher.py:501`) |
| `watchlist.json` | JSON object `{rom_id: <game record>}`; a bare JSON array of rom-id strings is the accepted legacy form (`grid_launcher/server/discover.py:596`) | on every watchlist toggle and on hydration (`grid-launcher.py:2419`, `:2444`) | at startup (`grid-launcher.py:511`) |
| `discover_events.jsonl` | Newline-delimited JSON, one object per line: `{"event", "section_id", "rom_id", "ts"}` (`grid_launcher/server/discover.py:575`) | on Discover interactions; writing stops once the file exceeds 1 MiB (`grid_launcher/server/discover.py:573`) | never read by the client |
| `discover_ui.json` | JSON object with `hidden_sections`, `section_order`, `preferred_platforms` (`grid-launcher.py:2455`, `:1279`) | on preference change (`grid-launcher.py:2466`) | on every Discover render |
| `imagecache/` | Binary image files named `<cover cache key><extension>` | cover replenish worker (`grid_launcher/background/workers.py:874`) | cover display |

Secrets are stored in the OS keyring under service `GRIDLauncher` with accounts `api_token`,
`retroachievements_token`, `retroachievements_api_key` (`grid_launcher/core/token_store.py:11`).

Static data read from the application bundle: platform logo images resolved from
`<repo>/assets/retroarch-assets/<file>.png` and exposed as a `file://` URI
(`grid_launcher/server/catalog.py:11`, `:134`).

## Data model

### Connection/config values

- `server_url` — user-entered base URL. The canonical form is trimmed of surrounding
  whitespace and of trailing `/` (`grid_launcher/server/state.py:14`).
- `api_token` — bearer token string; trimmed at header-construction time
  (`grid_launcher/core/api.py:16`).
- "credentials present" is true only when both `server_url` and `api_token` are strings with
  non-whitespace content (`grid_launcher/server/state.py:6`).
- Account status text is `"Logged in as: <username>"` when a non-empty username exists *and*
  the session is connected, else `"Offline"` (`grid_launcher/server/state.py:21`).

### `ConnectionFailure`

An immutable record with four fields (`grid_launcher/server/connection.py:7`):

- `status_text` — short text for a status line.
- `dialog_text` — longer text for a modal; empty means "show no modal".
- `token_expired` — boolean flag that triggers the re-authentication flow.
- `access_denied` — boolean flag for permission failures.

### Platform records

Three different projections of the `/api/platforms` array exist:

- **Label → numeric id map** (`grid_launcher/server/catalog.py:37`). Entries lacking an integer
  `id` or a usable label are skipped; entries whose `rom_count` is an integer ≤ 0 are skipped.
  Duplicate labels get a ` (2)`, ` (3)` … suffix.
- **Label → slug map** (`grid_launcher/server/catalog.py:67`) with the same skip and
  de-duplication rules, keyed on a non-empty trimmed `slug`.
- **Detail records** (`grid_launcher/server/catalog.py:98`) — objects with `slug`, `name`
  (de-duplicated display name), `rom_count`, `manufacturer`, `release_year`, `player_count`,
  `local_logo_path` (a `file://` URI or empty), `url_logo` (server value or empty). The list is
  sorted case-insensitively by `name` (`grid_launcher/server/catalog.py:149`).

The display label for any of the three is the first non-empty of `display_name`, `name`, `slug`,
trimmed; if all are empty but an integer id exists the label is `Platform <id>`
(`grid_launcher/server/catalog.py:23`).

### Game record (catalog projection)

`games_from_rom_items` converts raw ROM items into flat records whose values are **all
strings** — numbers and structures are stringified so the record can be stored and compared
uniformly (`grid_launcher/server/catalog.py:336`). Fields:

- `title` — `name`, falling back to `fs_name_no_ext`; items with neither are dropped entirely
  (`grid_launcher/server/catalog.py:284`).
- `platform` — item's `platform_display_name` when non-empty, else the label of the platform
  the fetch was issued for (`grid_launcher/server/catalog.py:294`).
- `rating`, `description`, `genres`, `regions`, `release_year`, `filesize_bytes`, `revision`,
  `languages`, `tags`, `fanart_url`, `companies`, `first_release_date` — all from the metadata
  merge described under Behavior.
- `cover_url` — resolved absolute cover URL (empty when unresolvable).
- `screenshot_urls` — newline-joined list of absolute URLs.
- `rom_id` — stringified item `id`.
- `server_updated_at` — trimmed `updated_at` string, used for update detection.
- `rom_file_name` — download filename: `"<fs_name>.<fs_extension>"` when `fs_name` has no
  suffix of its own, otherwise plain `fs_name` (`grid_launcher/server/catalog.py:310`).
- `rom_nested_file_name` — set only for folder-backed ROMs (no `fs_extension`): the
  `files[0].file_name` when that name has a real suffix (`grid_launcher/server/catalog.py:314`).
- `rom_base_file_id` — for multi-file ROMs, the stringified `id` of the first file entry whose
  `category` is empty or `"game"` (`grid_launcher/server/catalog.py:323`).
- `ra_id` — stringified RetroAchievements game id, empty when absent.
- `ps4_has_update` / `ps4_has_dlc` / `ps4_file_ids_by_category` and the three `xbox360_*`
  equivalents — see Platform differences.

### Discover game record

`normalize_discover_item` produces the same shape from a leaner source, with different field
sources (`grid_launcher/server/discover.py:187`):

- `cover_url` — first non-empty of `path_cover_large`, `path_cover_small`, `url_cover`,
  `cover_url`, used **as returned** (no base-URL resolution at this layer)
  (`grid_launcher/server/discover.py:196`).
- `genres` — comma-joined `name` values from the item's `genres` array, or the raw string when
  `genres` is already a string.
- `rating` — `rating`, else `average_rating`, stringified; falsy values become empty string.
- `description` — `summary`, else `description`.
- `release_year` and `first_release_date` both carry the raw `first_release_date` value.
- `created_at` — passed through verbatim (used by the "New on Server" badge).
- All the console-specific flags default to `"false"` / `"{}"`, and `update_available` defaults
  to `"false"`.

### Discover cache entry

`{section_id: {"data": <section object>, "timestamp": <epoch seconds>}}`. Known section ids:
`short_games`, `new_games`, `highly_rated`, `recommendations`, `platforms_list`,
`all_platforms`, `platform:<platform id>`, `genres`, `genre_totals`
(`grid_launcher/background/workers.py:912`, `:924`, `:936`, `:959`, `:965`, `:973`, `:1010`,
`:1101`).

Section payload shapes: carousels store `{"games": [...]}`; `short_games` additionally stores
`{"genres": [...]}`; `platforms_list` stores `{"platforms": [{"id", "display_name", "games"}]}`;
`genres` stores `{"genres": [...], "games_by_genre": {genre: [...]}}`; `genre_totals` stores
`{"totals": {genre: int}}`.

### Achievement record

Normalized from the RetroAchievements payload (`grid_launcher/server/retroachievements.py:59`):
`id` (integer, from `ID`, else `AchievementID`, else the map key), `title`, `description`,
`points` (integer, 0 when absent), `badge_name`, `date_earned` (string; empty in anonymous
mode).

## Behavior

### TLS trust store

At package import the default HTTPS context is replaced with one built from the bundled
`certifi` CA file, if `certifi` is importable; otherwise the platform default stands
(`grid_launcher/__init__.py:5`). A port targeting frozen/self-contained binaries needs the same
bundled-CA behavior, because system CA discovery is unreliable there.

### Auth flow

1. Secrets are loaded from the OS keyring first. On a miss, the legacy file is decoded; if it
   yields a value, the value is written to the keyring and the file is deleted
   (`grid_launcher/core/token_store.py:159`).
2. Saving an empty value clears both keyring entry and legacy file and reports success
   (`grid_launcher/core/token_store.py:138`).
3. Saving a non-empty value tries the keyring; on failure Windows falls back to the encrypted
   file and every other platform **refuses to store** and reports failure
   (`grid_launcher/core/token_store.py:143`).
4. Every request carries `Accept: application/json` and `Authorization: Bearer <trimmed token>`
   (`grid_launcher/core/api.py:15`). Binary requests replace `Accept` with
   `application/octet-stream, */*;q=0.9` and keep the same Authorization
   (`grid_launcher/core/api.py:19`).
5. Connecting: if credentials are absent, clear all cached server data, set the status to
   "Missing server URL or API token" and stop — no request is made (`grid-launcher.py:3024`).
6. Otherwise issue `/api/users/me` then `/api/platforms`, strictly in that order and
   sequentially (`grid_launcher/server/orchestrator.py:7`). Success sets the connected flag,
   stores the username, populates the platform list, and starts the missing-cover replenish
   pass (`grid-launcher.py:3034`).
7. Any HTTP error, network error, value error or JSON decode error aborts the pair, clears
   server data, and classifies the failure (`grid-launcher.py:3041`).

### Failure classification

Given the error from the connect attempt (`grid_launcher/server/connection.py:15`):

- HTTP 401 → `status_text="Token expired"`, empty `dialog_text`, `token_expired=true`. The
  caller runs the re-authentication dialog and closes the window if the user declines
  (`grid-launcher.py:3050`).
- HTTP 403 → `status_text="Access denied (403)"`, a dialog explaining that the account or token
  lacks API permission, `access_denied=true`.
- Any other HTTP status → both texts become `"Connection failed (<code>)"`.
- Network-level error → both texts become `"Connection failed (network error)"`.
- Anything else (including no error object) → both texts become `"Failed to connect"`.

### Request lifecycle

URL construction: concatenate base URL and path verbatim, then append `?` plus a
percent-encoded query when parameters exist. List-valued parameters are expanded by
**repeating the key** (`grid_launcher/core/api.py:59`). The path is *not* encoded by the
helper — callers percent-encode path segments themselves before calling
(`grid_launcher/server/details_cache.py:73`, `grid_launcher/ui/mixins/install_mixin.py:1012`).

Empty base URL raises an explicit "Server URL is required" error before any I/O in every
helper (`grid_launcher/core/api.py:70`, `:80`, `:133`, `:153`, `:173`, `:193`, `:213`).

Timeouts, by helper:

- JSON GET: 10 seconds (`grid_launcher/core/api.py:74`).
- Binary GET: 60 seconds (`grid_launcher/core/api.py:84`).
- All POST/PUT variants: 60 seconds (`grid_launcher/core/api.py:140`, `:160`, `:180`, `:200`,
  `:220`).
- RetroAchievements and PCGamingWiki: 10 seconds each
  (`grid_launcher/server/retroachievements.py:26`, `grid_launcher/server/pcgamingwiki.py:138`).

Response handling: JSON helpers decode the body as UTF-8 and parse it. `api_post_json` is the
only tolerant one — it trims the body, returns an empty object for an empty body, and returns
an empty object rather than raising when the body is not valid JSON
(`grid_launcher/core/api.py:221`).

Multipart encoding, file variant (`grid_launcher/core/api.py:88`):

- Boundary is `----GRIDLauncherBoundary<epoch milliseconds>`.
- Each part: `--<boundary>` CRLF, then
  `Content-Disposition: form-data; name="<field>"; filename="<basename>"` CRLF,
  `Content-Type: <guessed mime or application/octet-stream>` CRLF CRLF, then the raw file
  bytes, then CRLF.
- Files that do not exist or are not regular files are silently skipped, which can produce a
  body with no parts at all.
- Terminator is `--<boundary>--` CRLF.

Multipart encoding, text variant (`grid_launcher/core/api.py:111`): boundary is
`----GRIDLauncherBoundary<random hex>`; each part has only the `Content-Disposition` line and
no `Content-Type`; values are UTF-8 encoded.

Error text formatting (`grid_launcher/core/api.py:25`) builds a single-line diagnostic:

1. Title: `HTTP <status>` when a positive status exists, else `HTTP error`; the reason phrase
   is appended when present.
2. `url=<url>` is appended when the error carries a URL.
3. Up to `body_limit + 1` bytes of the response body are read (default limit 240). The bytes
   are decoded as UTF-8 with replacement, all whitespace runs are collapsed to single spaces,
   the text is cut to `body_limit`, and `...` is appended when the collapsed text was longer.
   The result is appended as `body="<snippet>"`. Body-read failures are swallowed.
4. Parts are joined with ` | `.

### Catalog pagination

`fetch_platform_rom_items` walks `/api/roms` with a fixed page size of 200
(`grid_launcher/server/catalog.py:153`):

1. Start at offset 0.
2. Request the page; if the payload is not an object, stop.
3. If `items` is missing, not an array, or empty, stop.
4. Append every object element of `items` to the accumulator (non-objects are skipped).
5. If `total` is an integer and the accumulated count has reached or passed it, stop.
6. If the page returned fewer than 200 elements, stop.
7. Otherwise advance the offset by 200 and repeat.

There is no page limit and no cancellation check inside the loop. The offset advances by the
requested page size, not by the number of accepted items, so non-object elements do not shift
the window.

The single-shot variant `fetch_rom_items_by_params` issues one request with caller-supplied
parameters and returns the object elements of `items`, or an empty list when the payload is not
an object or `items` is not an array (`grid_launcher/server/catalog.py:195`).

### Metadata merge

`details_metadata_from_item` merges four provider blocks in a fixed priority order —
`launchbox_metadata`, `ss_metadata`, `igdb_metadata`, `moby_metadata`
(`grid_launcher/server/metadata.py:8`). Missing or non-object blocks become empty objects
(`grid_launcher/server/metadata.py:232`).

Per-field rules (`grid_launcher/server/metadata.py:245`):

- **description** — first non-empty of keys `description`, `summary`, `overview`, `synopsis`,
  `plot`, scanning providers in priority order; first hit wins and its provider name is recorded
  as `description_source`. Fallback: the item's top-level `summary`, recorded as source
  `summary`.
- **genres** — union across all providers, seeded by the highest-priority provider that has
  any, then extended with case-insensitively new values from lower-priority providers.
  Fallback when still empty: `metadatum.genres`.
- **regions** — same union rule over keys `regions`, `region`, `countries`, `country`.
  Fallback: the same keys at the item's top level.
- **rating** — first parseable value among keys `rating`, `ratings`, `user_rating`,
  `community_rating`, `avg_rating`, `score`, `total_rating`, `aggregated_rating`, `moby_score`,
  scanning providers in priority order. Fallback: the same keys at the item's top level, with
  source recorded as `rom`.
- **release_year** — first year extracted from `first_release_date` of launchbox, screenscraper,
  igdb, then `flashpoint_metadata`; fallback `metadatum.first_release_date`.
- **first_release_date** — same source order, formatted as a date.
- **filesize_bytes** — `fs_size_bytes` stringified, but only when it is a positive integer.
- **companies** — first non-empty array among launchbox, igdb, screenscraper, comma-joined;
  fallback `metadatum.companies` (array joined, or a trimmed string).
- **fanart_url** — `ss_metadata.fanart_url`, else `gamelist_metadata.fanart_url`, else empty.
- **tags** — comma-joined `name` values (or stringified elements) of the item's `tags` array.
- **languages** — comma-joined only when the item's `languages` is an array.
- **revision** — stringified item `revision`.

Scalar text values from a provider may be strings, objects, or arrays; objects contribute the
first present of `name`, `value`, `title`, `label`; arrays are flattened recursively
(`grid_launcher/server/metadata.py:56`). Duplicates are removed case-insensitively, preserving
first-seen casing (`grid_launcher/server/metadata.py:79`).

**Rating normalization** (`grid_launcher/server/metadata.py:107`) infers the source scale:

- Numeric (non-boolean) input: negative → no rating; ≤ 5 → scale 5; ≤ 10 → scale 10; otherwise
  scale 100.
- String input: an `a / b` pattern uses `a` over `b` (rejected when `a` is negative or `b` ≤ 0);
  otherwise an `n%` pattern uses scale 100; otherwise the first number in the string is taken
  and the same ≤5 / ≤10 / else-100 inference applies.
- The result is `value / scale * 5`, clamped to at most 5, rounded to one decimal
  (`grid_launcher/server/metadata.py:155`), and formatted as `"<x.x>/5"`
  (`grid_launcher/server/metadata.py:172`). Unparseable input yields an empty string.

**Year extraction** (`grid_launcher/server/metadata.py:179`): booleans are rejected; integers
above 10000 are treated as UTC epoch seconds and converted to a year; integers in 1900–2100 are
taken literally; strings contribute their first 4-digit run. Any year outside 1900–2100 is
discarded.

**Date formatting** (`grid_launcher/server/metadata.py:204`): integers above 10000 (and, oddly,
integers in 0–1899) are formatted as `YYYY-MM-DD` from UTC epoch seconds; integers in 1900–2200
are returned as-is; strings are parsed as ISO-8601 and reformatted, falling back to their first
4-digit run.

### ROM filename resolution

Downloading requires the server's real filename. `rom_file_name_from_payload` builds an ordered
candidate list (`grid_launcher/server/details_cache.py:10`):

1. Highest priority: `"<fs_name>.<fs_extension>"` when both are non-empty (the leading dot of
   the extension is stripped first).
2. If `fs_name` exists but `fs_extension` is empty (folder-backed ROM): `files[0].file_name`,
   but only when that name has a suffix.
3. Then, in order, the raw values of `fs_name`, `file_name`, `filename`, `rom_file_name`,
   `download_path`, `file_path`, `full_path`, `path`, `url`. Backslashes are converted to
   forward slashes and leading slashes are stripped; for `url` only the URL path component is
   kept.

The first candidate whose **last path segment has a suffix** wins; if none has a suffix, the
first candidate wins; if there are no candidates the result is empty
(`grid_launcher/server/details_cache.py:54`).

`resolved_rom_file_name_for_game` layers retries on top (`grid_launcher/server/details_cache.py:161`):

1. If the game record already has a name whose last segment has a suffix, use it unchanged.
2. Otherwise take the cached ROM payload; if absent, fetch it without forcing a refresh.
   Derive a name; if it has a suffix, return it; else remember it as a fallback.
3. Otherwise force-refetch the ROM payload and repeat step 2's derivation.
4. Return the best suffix-less fallback (possibly empty).

### ROM payload cache

`fetch_server_rom_payload` (`grid_launcher/server/details_cache.py:58`):

1. Trim the id; empty id yields nothing.
2. Return the in-memory cached object unless a refresh is forced.
3. Percent-encode the id (encoding `/` too) and GET `/api/roms/{id}`. HTTP, network, value and
   JSON-decode errors all return nothing — they are not propagated.
4. Accept the payload directly when it contains any of `fs_name`, `file_name`, `filename`,
   `rom_file_name`; otherwise unwrap the first present of the nested keys `item`, `rom`, `data`
   when that nested value is an object.
5. Store the accepted object in the in-memory map keyed by id and return it.

### ROM id resolution and the persisted id cache

To act on a game the client needs its ROM id (`grid_launcher/server/details_cache.py:197`):

1. Use the record's own `rom_id` when non-empty.
2. Otherwise look up the persisted cache under the key `"<title>::<platform>"`
   (`grid_launcher/server/details_cache.py:97`; an empty title or platform yields an empty key,
   which never matches).
3. Otherwise scan every loaded server platform's game list for a record with the same
   (title, platform) key and take its `rom_id`.
4. Otherwise return empty.

The persisted cache lives inside `config.json` under `details_rom_id_cache`. Loading normalizes
it: only string-to-string pairs with non-blank key and value survive, each trimmed
(`grid_launcher/server/details_cache.py:110`). Storing writes the trimmed id under the computed
key (`grid_launcher/server/details_cache.py:123`). Clearing removes the key and, when the map
becomes empty, removes the whole `details_rom_id_cache` entry from the config
(`grid_launcher/server/details_cache.py:140`).

### Detail backfill

When a details view opens for a game that is missing any of `genres`, `regions`,
`filesize_bytes`, `rating`, `companies` — with the literal text `n/a` for rating treated as
missing — and a rom id, base URL and token are available, a background GET of `/api/roms/{id}`
is started (`grid_launcher/ui/mixins/details_view_mixin.py:185`, `:199`).

The response is merged in (`grid_launcher/ui/mixins/details_view_mixin.py:228`):

- A result whose rom id no longer matches the currently displayed game is discarded.
- Errors are silently ignored.
- Fields are filled **only where the current value is empty**; the placeholder strings `n/a`
  (rating) and `no description available.` (description) count as empty.

### Discover data flow

Trigger points:

- Opening the Discover tab renders from cache first (if no sections are built yet), then
  refreshes in the background only when the `short_games` section is stale
  (`grid-launcher.py:791`).
- An hourly timer force-refreshes when the oldest cache entry is older than a week
  (`grid-launcher.py:520`, `:1173`).
- Explicit refresh forces a full reload (`grid-launcher.py:1154`).
- When not connected, the offline path renders whatever cache exists and shows an offline
  notice (`grid-launcher.py:1226`).

Cache-only render ignores TTL entirely and collects `short_games`, `new_games`, `highly_rated`,
`recommendations`, `platforms_list`, `genres`, `genre_totals`, dropping empty sections
(`grid-launcher.py:1183`).

The live load (`grid_launcher/background/workers.py:1019`) runs as:

1. Build the installed-title set (lowercased names) and the installed-platform-name set
   (lowercased, trimmed) before starting (`grid-launcher.py:1166`,
   `grid_launcher/server/discover.py:28`, `:39`).
2. **Sequential, first**: the `short_games` section. On a cache hit take its `games` and
   `genres`; otherwise fetch and store both. Any exception here is recorded but does not abort.
3. Fan out the remaining sections onto a worker pool of at most 12 concurrent tasks:
   - `new_games` and `highly_rated` always.
   - `recommendations` only when the local library has at least 20 games
     (`grid_launcher/background/workers.py:1054`).
   - `platforms_list` (which itself may issue one request per unexplored platform).
   - Up to the first 6 available genres, one request each, but only when the `genres` section
     is not already cached (`grid_launcher/background/workers.py:1061`).
   - `genre_totals` for up to the first 15 genres, one request per genre
     (`grid_launcher/background/workers.py:1065`).
4. Collect results; each future's failure is swallowed individually so siblings still land
   (`grid_launcher/background/workers.py:1073`, `:1080`, `:1092`, `:1107`).
5. If the result is completely empty *and* step 2 raised, emit an error event; otherwise emit
   the assembled result (`grid_launcher/background/workers.py:1112`).

Per-section fetch rules:

- **Short But Fun** (`grid_launcher/server/discover.py:274`): always requests 100 items with
  the `hltb` metadata provider, regardless of the caller's `limit`. Items are split into
  "short" (`hltb_metadata.main_story` numeric, greater than 0 and at most 1200) and "other";
  each group is shuffled independently, short items are placed first, the concatenation is
  truncated to `3 × limit`, normalized, and truncated again to `limit`. Genres are extracted
  from the same response.
- **Genre list extraction** (`grid_launcher/server/discover.py:168`): read `filter_values.genres`;
  accept both plain strings and objects with a `name`; sort ascending; keep the first 15.
- **New on Server** (`grid_launcher/server/discover.py:621`) and **Highly Rated**
  (`grid_launcher/server/discover.py:467`): ordered server-side. Highly Rated additionally
  applies a client-side cut: parse the normalized `rating` as a number and keep only ≥ 4.0;
  unparseable or missing ratings are dropped.
- **Genre totals** (`grid_launcher/server/discover.py:366`): one request per genre with
  `limit=1`, reading only `total`. A failing or malformed genre response is omitted rather than
  raising.
- **Platform sections** (`grid_launcher/background/workers.py:950`): fetch all platforms, cache
  them whole under `all_platforms`, then choose up to 3 "unexplored" ones — entries with a
  truthy `rom_count` whose display name and name are both absent from the installed-platform
  set, sorted by `rom_count` descending (`grid_launcher/server/discover.py:446`). Each chosen
  platform gets its own cached sub-section keyed `platform:<id>`; platforms whose game list ends
  up empty are dropped from the result.
- **Recommendations** (`grid_launcher/server/discover.py:517`): count genre occurrences across
  the local library, take the top 3 genres, fetch each genre's games, de-duplicate by `rom_id`
  (first occurrence wins), optionally keep only preferred platforms (case-insensitive), remove
  installed titles, truncate to the limit. An empty genre histogram short-circuits to an empty
  list, and any exception yields an empty list.
- **Installed filter** (`grid_launcher/server/discover.py:325`): a game is excluded when its
  lowercased `title` (or `name`) exactly equals an installed title.
- **Client-side filter panel** (`grid_launcher/server/discover.py:341`): genre matching is a
  case-insensitive **substring** test against the joined genre string; platform matching is a
  case-insensitive **exact** match. With no filters selected the input list is returned
  unchanged.
- **Genre statistics** (`grid_launcher/server/discover.py:400`): counts are accumulated by
  splitting each record's `genres` on commas and trimming. Server-provided totals override the
  sampled totals for genres they cover, and add genres that never appeared in the sample.

Section assembly for display (`grid-launcher.py:1271`) applies the preferred-platform filter to
every carousel, orders sections by the persisted `section_order` (unknown ids appended in
insertion order), skips ids listed in `hidden_sections`, and always appends a `watchlist`
section built from local state. The Discover cache is written to disk only when at least one
section was added **and** the render came from a live fetch (`grid-launcher.py:1411`).

Watchlist behavior (`grid-launcher.py:2410`): toggling stores a copy of the whole game record
under its rom id, or deletes the entry, then rewrites the file immediately. Records without a
rom id are ignored. Entries loaded from the legacy id-only format have empty bodies; they are
hidden from the watchlist section (`grid-launcher.py:2428`) until the same rom id appears in
another section, at which point the record is copied in and the file is rewritten
(`grid-launcher.py:2435`).

Watchlist persistence (`grid_launcher/server/discover.py:582`, `:608`): reading accepts an array
(legacy: ids map to empty objects) or an object (ids with object values only); anything else,
or any error, yields an empty map. Writing is atomic — serialize to a sibling whose extension
is REPLACED by `.tmp` (`watchlist.json` → `watchlist.tmp`, not `watchlist.json.tmp`), then
rename over the target — with parent directories created first and every error swallowed.

Cache persistence uses the same tmp-file-then-rename scheme
(`grid_launcher/server/discover.py:111`). Loading validates each entry has both `data` and a
numeric `timestamp`, optionally skips entries older than a max age, and **never overwrites an
entry already in memory** (`grid_launcher/server/discover.py:124`).

Analytics logging (`grid_launcher/server/discover.py:561`) appends one JSON line per event and
stops writing once the file exceeds 1 MiB; it is never rotated or read back.

### RetroAchievements flow

1. Resolve the game's RA id from the record's `ra_id`: absent → none; blank → none;
   all-digits → integer; anything else → none (`grid_launcher/server/retroachievements.py:126`).
   The achievements button is shown only when this resolves.
2. Validate the id is a positive integer, raising otherwise
   (`grid_launcher/server/retroachievements.py:16`).
3. Choose the endpoint by credential presence: **both** a non-empty username and a non-empty API
   key select the user-progress endpoint (query `u`, `y`, `g`); otherwise the anonymous extended
   endpoint (query `g` only) (`grid_launcher/server/retroachievements.py:97`).
4. Fetch and validate: non-object payloads are rejected; a payload with `Success` present and
   falsy, or with a truthy `Error`, is rejected with the message taken from `Error`, else
   `Message`, else a generic string (`grid_launcher/server/retroachievements.py:39`).
5. Read the `Achievements` map. A missing, non-object or empty map yields an empty list. Each
   entry that is an object is normalized; non-object entries are skipped
   (`grid_launcher/server/retroachievements.py:114`).
6. Earned date, in authenticated mode only: prefer `DateEarned`, fall back to
   `DateEarnedHardcore`. The strings `""`, `"0"`, `"null"`, `"None"` count as not earned
   (`grid_launcher/server/retroachievements.py:52`).

Login (`grid_launcher/server/retroachievements.py:69`): username and password must be non-empty
strings, else a validation error is raised before any request. On `Success == true` the response
must carry non-empty string `User` and `Token`, else an error is raised. On any other outcome the
error message is `Error`, else `Message`, else `"Invalid credentials"`.

HTTP failures from either RA call are converted to `RetroAchievements HTTP <code>: <body>` with
the body truncated to 300 characters; network, decode and value errors become
`RetroAchievements request failed: <detail>` (`grid_launcher/server/retroachievements.py:29`).

### PCGamingWiki flow

`fetch_windows_save_paths(title)` (`grid_launcher/server/pcgamingwiki.py:264`):

1. Resolve a page id (`grid_launcher/server/pcgamingwiki.py:218`):
   a. Trim the title; empty → no result.
   b. Query the MediaWiki `query` action with the URL-encoded title. Scan the `query.pages`
      object and return the first key that is neither `"-1"` nor an object containing `missing`,
      parsed as an integer (`grid_launcher/server/pcgamingwiki.py:190`).
   c. On no hit, run an OpenSearch query limited to 3 results in namespace 0. The response is a
      JSON array; element index 3 holds the result URLs.
   d. Take the first URL, extract the segment after `/wiki/`, percent-decode it and replace
      underscores with spaces (`grid_launcher/server/pcgamingwiki.py:208`).
   e. Re-run the `query` lookup with that title and return its page id.
2. Fetch the wikitext with the `parse` action. Missing `parse.wikitext.*`, or a non-string
   value, raises (`grid_launcher/server/pcgamingwiki.py:250`).
3. Parse Windows save paths out of the wikitext (`grid_launcher/server/pcgamingwiki.py:96`):
   - Scan for occurrences of a `{{Game data/saves|` template header (whitespace-tolerant,
     case-insensitive).
   - Extract the full balanced `{{ … }}` block by brace-depth counting; an unbalanced block
     terminates the scan (`grid_launcher/server/pcgamingwiki.py:76`).
   - Split the block's interior on `|` at brace depth 0 only, so nested templates stay intact
     (`grid_launcher/server/pcgamingwiki.py:153`).
   - Require at least 3 arguments and require argument index 1 to be exactly `windows`
     (case-insensitive); otherwise skip the block.
   - Every remaining argument is expanded to a path; duplicates are dropped, order is preserved.
4. Path expansion (`grid_launcher/server/pcgamingwiki.py:56`): the argument must **start** with a
   `{{P|<var>}}` template. The variable is looked up case-insensitively in a fixed table
   (`grid_launcher/server/pcgamingwiki.py:15`) which maps wiki variables to Windows
   environment-style prefixes — for example `userprofile\documents`, `userdocuments` and
   `savedgames` all map to `%USERPROFILE%\Documents`; `appdata` → `%APPDATA%`; `game` →
   `%GAME_DIR%`. Store-specific and registry variables (`steam`, `uplay`, `epicgames`, `gog`,
   `origin`, `battlenet`, `itchapp`, `registry`) map to nothing and cause the argument to be
   dropped. After substitution, all remaining `{{…}}` annotations are stripped, a trailing
   path segment containing a wildcard is removed, and trailing separators are trimmed. An empty
   result is dropped.

HTTP failures become `PCGamingWiki HTTP <code>: <body>` (body truncated to 300 characters);
other failures become `PCGamingWiki request failed: <detail>`
(`grid_launcher/server/pcgamingwiki.py:141`).

### Server view data contracts

Only the data-shaping parts matter for a port:

- Clearing the connection resets the connected flag and empties the platform id map, slug map,
  per-platform game lists and the ROM payload cache (`grid_launcher/server/view.py:61`).
- Populating platforms replaces the id map and slug map, discards previously loaded games and
  ROM payloads, lists the labels in map order, and selects the first entry
  (`grid_launcher/server/view.py:76`). An empty id map clears everything and returns early.
- Selecting a platform starts a load only when that platform has no cached game list, then
  renders (`grid_launcher/server/view.py:131`).
- Search filtering is case-insensitive substring matching against title, platform, or the
  genres value; genres may be a list/tuple (joined with `, `) or a string. An empty query
  returns the input unchanged (`grid_launcher/server/view.py:107`).

### Platform static metadata

Two lookup tables keyed by RomM platform slug live in
`grid_launcher/server/platform_metadata.py`: `PLATFORM_METADATA` (slug →
manufacturer / release year / player count, `grid_launcher/server/platform_metadata.py:3`) and
`PLATFORM_LOGO_FILES` (slug → RetroArch asset filename,
`grid_launcher/server/platform_metadata.py:226`). Their contents are display-only; a port can
carry them as data files.

Logo resolution is name-first, slug-second (`grid_launcher/server/platform_metadata.py:368`):

1. A derived index maps both the full filename stem (e.g. `sony - playstation 2`) and the part
   after the last ` - ` (e.g. `playstation 2`) — both lowercased — to the filename
   (`grid_launcher/server/platform_metadata.py:345`).
2. Try the lowercased display name; then the lowercased display name with all spaces removed.
3. Fall back to an exact slug match in `PLATFORM_LOGO_FILES`.
4. Return an empty string when nothing matches.

## Invariants and error handling

- A request is never issued with an empty base URL; the helpers raise first
  (`grid_launcher/core/api.py:70`).
- The token is trimmed at header-construction time, so surrounding whitespace in storage is
  harmless (`grid_launcher/core/api.py:16`).
- Every response is treated as untrusted: the code type-checks objects, arrays and strings at
  each access and degrades to empty values instead of raising
  (`grid_launcher/server/catalog.py:37`, `:99`, `:175`; `grid_launcher/server/discover.py:162`).
- Discover fetch helpers catch **all** exceptions and return empty results
  (`grid_launcher/server/discover.py:158`, `:261`, `:310`, `:390`, `:437`, `:557`). Failures are
  therefore invisible to the caller and only surface as missing sections.
- Discover disk I/O — cache save/load, watchlist save/load, analytics append — swallows all
  errors (`grid_launcher/server/discover.py:121`, `:145`, `:578`, `:604`, `:617`).
- Both disk writes that matter (Discover cache, watchlist) are atomic via a temp sibling plus
  rename; the temp name replaces the extension with `.tmp` (`discover_cache.tmp`)
  (`grid_launcher/server/discover.py:118`, `:614`).
- `fetch_server_rom_payload` never raises; unreachable or malformed details resolve to nothing
  (`grid_launcher/server/details_cache.py:76`).
- Connect-time errors are narrowed to HTTP, network, value and JSON-decode errors; anything else
  propagates (`grid-launcher.py:3041`).
- Retention deletes treat HTTP 404 and 410 as success, since the record being gone is the
  desired end state (`grid_launcher/ui/mixins/cloud_mixin.py:1753`).
- Platform entries with an integer `rom_count` ≤ 0 are excluded from all three platform
  projections; a missing or non-integer `rom_count` does **not** exclude an entry
  (`grid_launcher/server/catalog.py:52`, `:82`, `:119`).
- Display-label collisions are resolved deterministically with an ascending numeric suffix, and
  the id map and slug map use the same algorithm so their labels stay aligned
  (`grid_launcher/server/catalog.py:58`, `:88`).
- Catalog records drop items without a usable title rather than emitting a placeholder
  (`grid_launcher/server/catalog.py:284`).
- Ratings are always normalized to a 0–5 scale and clamped, never passed through raw
  (`grid_launcher/server/metadata.py:155`).
- The persisted rom-id cache stores only trimmed non-empty string pairs
  (`grid_launcher/server/details_cache.py:110`).
- Non-Windows platforms refuse to persist a secret at all when the OS keyring is unavailable,
  rather than falling back to weaker storage (`grid_launcher/core/token_store.py:156`).
- Cover and screenshot URLs are re-encoded: the path is percent-encoded with `/%._-~` left
  intact and the query is re-serialized (`grid_launcher/cover/utils.py:40`). A separate host
  filter can reject any URL whose host differs from the server's, and is permissive when the
  base URL has no parseable host (`grid_launcher/cover/utils.py:47`).

## Platform differences

Operating-system differences:

- **Secret storage.** All platforms prefer the OS keyring. Only Windows has a fallback, using
  the OS data-protection API to write an encrypted file; every other platform reports failure
  and stores nothing (`grid_launcher/core/token_store.py:147`).
- **Legacy secret decoding.** A legacy file is decrypted with the Windows data-protection API on
  Windows and base64-decoded (strict) everywhere else
  (`grid_launcher/core/token_store.py:112`).
- **Path separators.** Filename candidates from the server have backslashes rewritten to forward
  slashes before use, so Windows-style server paths behave identically on all clients
  (`grid_launcher/server/details_cache.py:44`, `grid_launcher/server/details_cache.py:170`).
- **Local logo paths** are emitted as `file://` URIs rather than native paths
  (`grid_launcher/server/catalog.py:134`).
- **Certificate trust** comes from the bundled CA file when available, which matters most for
  packaged builds (`grid_launcher/__init__.py:5`).

Console-platform (game platform) differences in the catalog projection:

- **PlayStation 4.** An item counts as PS4 when any of `platform_fs_slug`, `platform_slug`,
  `platform_display_name`, or the fetch's platform label normalizes (lowercased,
  non-alphanumerics removed) to `ps4`, `playstation4` or `sonyplaystation4`
  (`grid_launcher/server/catalog.py:208`). For such items the `files` array is grouped by
  `category` (trimmed and lowercased; missing or blank category becomes `game`) into a
  category → list-of-file-ids map, serialized as compact JSON with sorted keys. Two boolean
  strings record whether an `update` or `dlc` category exists
  (`grid_launcher/server/catalog.py:246`, `:360`).
- **Xbox 360.** Identical treatment with the normalized labels `xbox360`, `microsoftxbox360`,
  `xb360`, using the same grouping routine and separate output fields
  (`grid_launcher/server/catalog.py:227`, `:299`).
- Non-matching platforms get `"false"` flags and `"{}"` maps, so the fields are always present.
- **Multi-file ROMs** of any platform expose `rom_base_file_id`, the id of the first `game`-
  category (or category-less) file, used to download the base game rather than an add-on
  (`grid_launcher/server/catalog.py:323`).
- **Folder-backed ROMs** (no `fs_extension`) expose `rom_nested_file_name`
  (`grid_launcher/server/catalog.py:314`).

## Concurrency

- **Catalog loads run off the UI thread.** Base URL and token are captured on the UI thread
  before the worker starts, so the worker never reads mutable shared config
  (`grid-launcher.py:3094`). The worker writes its result into a per-platform result map and
  emits a completion event carrying only the platform label; the UI thread pops the result
  (`grid-launcher.py:3117`, `:3124`).
- **Duplicate-load guard.** A platform already present in the "loading" set is not fetched again
  (`grid-launcher.py:3085`); the entry is removed when the result is consumed
  (`grid-launcher.py:3126`).
- **Stale-render guard.** The result is only re-rendered when the platform is still the selected
  one (`grid-launcher.py:3142`).
- **Discover.** A single background load runs at a time; a new request is dropped while one is
  running (`grid-launcher.py:1239`). Inside it, sections fan out over a pool of at most 12
  concurrent tasks and every task's failure is isolated
  (`grid_launcher/background/workers.py:1049`).
- **Discover cache locking.** Read, write, invalidate and staleness checks take a mutex, because
  sections are written from pool threads (`grid_launcher/server/discover.py:24`, `:60`, `:77`,
  `:89`, `:101`).
- **Detail, achievements and wiki lookups** each run on their own background worker and report
  through a completion event (`grid_launcher/background/workers.py:769`, `:789`, `:827`).
- **Request-id correlation instead of cancellation.** Achievements and wiki lookups carry a
  request id; a result whose id does not match the pending id is discarded
  (`grid_launcher/ui/mixins/details_view_mixin.py:1917`, `:1953`). ROM detail results are correlated by
  rom id instead (`grid_launcher/ui/mixins/details_view_mixin.py:239`). Starting a new ROM
  detail lookup asks the previous thread to quit but does not interrupt an in-flight request
  (`grid_launcher/ui/mixins/details_view_mixin.py:202`).
- **No request-level cancellation exists** for RomM catalog paging, Discover, RA or PCGW calls;
  the only cooperative cancel flag in the codebase belongs to the emulator-source download
  worker (`grid_launcher/background/workers.py:113`).
- **Rendering is chunked, not concurrent.** The server game grid first fills with cheap
  placeholders and upgrades only the rows inside the viewport, guarded by a monotonically
  increasing render generation so a superseded render stops upgrading
  (`grid_launcher/server/view.py:190`, `:229`).

## Test oracle

| Behavior | Test |
|----------|------|
| Binary vs JSON `Accept` header, bearer format | `tests/test_core_api.py:11` |
| HTTP error text: title, url, body truncation with ellipsis | `tests/test_core_api.py:20` |
| `/api/roms` GET still accepts the eight params the client sends | `tests/test_openapi_contract.py:38` |
| Rom schemas still expose `platform_*`, `fs_name`, `fs_extension`, `files`, `updated_at`, `ra_id` | `tests/test_openapi_contract.py:57`, `:72` |
| Platform schema still exposes `display_name`, `name`, `slug`, `rom_count`, `url_logo` | `tests/test_openapi_contract.py:87` |
| Save/state/firmware schemas still expose the fields the client reads | `tests/test_openapi_contract.py:99`, `:110`, `:121` |
| Filename from `fs_name` + `fs_extension`, folder-backed nested file, empty-`files` fallback | `tests/test_server_catalog.py:12`–`:46` |
| Catalog record filename, nested filename, base file id selection | `tests/test_server_catalog.py:55`–`:167` |
| Provider priority, fallbacks, release year from epoch, extended metadata fields | `tests/test_server_catalog.py:167`–`:311` |
| Rating scale inference and clamping | `tests/test_server_catalog.py:313`, `tests/test_metadata_merge.py:13`–`:36` |
| Single-shot rom fetch: non-object payload, missing/empty `items`, non-object elements, param pass-through | `tests/test_server_catalog.py:325` |
| Slug map extraction, skips, de-duplication parity with the id map | `tests/test_server_catalog.py:358` |
| PS4 category grouping, flags, and safe defaults for other platforms | `tests/test_server_catalog_ps4_metadata.py:10`, `:56` |
| Metadata merge per-field rules (description/genres/regions/rating/filesize/tags/companies/dates) | `tests/test_metadata_merge.py:38`–`:262` |
| Platform detail records: exclusions, static metadata fill, logo resolution by name and slug, sorting | `tests/test_tv_server_platform_details.py:9`–`:374` |
| Discover cache: set/get, TTL expiry, force refresh, invalidate, staleness, concurrent writes | `tests/test_discover.py:38`–`:87` |
| Discover cache disk round-trip, corrupt file, max-age skip, no-overwrite-fresh rule | `tests/test_discover.py:251`–`:314` |
| Installed-title filtering, installed-platform-name set | `tests/test_discover.py:106`, `:332` |
| `normalize_discover_item` field mapping, cover fallback order, defaults | `tests/test_discover.py:131`–`:174` |
| Genre extraction from `filter_values` (strings and objects), API error paths | `tests/test_discover.py:182`–`:232` |
| New-games and highly-rated fetch, rating threshold exclusion | `tests/test_discover.py:357`–`:418` |
| Platform fetch, unexplored-platform selection, sorting and cap | `tests/test_discover.py:424`–`:503` |
| Recommendations: empty library, dedupe by rom id, installed filter, preferred platforms | `tests/test_discover.py:525`–`:592` |
| Short-games ordering, threshold, missing HLTB data, limit | `tests/test_discover.py:1098`–`:1179` |
| Genre totals: per-genre request params, failure omission, empty input | `tests/test_discover.py:783`–`:812` |
| Genre statistics and server-total override | `tests/test_discover.py:744`–`:777` |
| Client-side filter panel semantics (substring genre, exact platform) | `tests/test_discover.py:700`–`:740` |
| Watchlist persistence: missing file, round-trip, corrupt file, legacy array format | `tests/test_discover.py:826`–`:866` |
| Watchlist store behavior: toggle, id-only entries hidden, hydration | `tests/test_discover.py:1538`–`:1581` |
| Analytics JSONL append and 1 MiB cut-off | `tests/test_discover.py:880`–`:912` |
| Discover worker: full result, isolated section failure, error only when everything fails | `tests/test_discover.py:1282`, `:1334`, `:1365` |
| Cache-only render result shape, stale entries still shown, empty sections omitted | `tests/test_discover.py:1463`–`:1491` |
| Server search filter including genres as list or string | `tests/test_discover.py:1398`–`:1427` |
| RA login success/failure, empty credential validation | `tests/test_retroachievements_client.py:28`–`:53` |
| RA authenticated vs anonymous endpoint selection, HTTP error, invalid id, id resolution | `tests/test_retroachievements_client.py:55`–`:126` |
| PCGW wikitext parsing: typical, no Windows entry, wildcard stripping, DRM path exclusion, multiple paths, annotations | `tests/test_pcgamingwiki.py:15`–`:143` |
| PCGW page-id lookup, OpenSearch fallback, full round trip, HTTP error propagation | `tests/test_pcgamingwiki.py:59`–`:149` |
| ROM detail worker success/error emission and merge-only-empty-fields behavior | `tests/test_rom_detail_worker.py:37`–`:96` |
| Cover/screenshot URL resolution and host filtering | `tests/test_screenshot_urls.py:13`–`:143` |
| Secret storage keyring/legacy behavior | `tests/test_token_store.py` |
| Firmware routing after `/api/firmware` fetch | `tests/test_firmware_install.py:37` |
| Cloud save/state transfer against the saves and states endpoints | `tests/test_cloud_transfer.py`, `tests/test_cloud_restore.py` |

## Open questions

- **OPEN QUESTION:** `fetch_platform_rom_items` never sends `with_files=true`, yet the records
  it produces read `item["files"]` for PS4/Xbox 360 categories, nested filenames and base file
  ids (`grid_launcher/server/catalog.py:163` vs `:298`, `:317`, `:325`). The spec documents
  `with_files` as defaulting to false (`openapi.json`, `/api/roms` GET parameter `with_files`)
  while also marking `files` as a required property of `SimpleRomSchema`. Whether the array is
  populated by default, or whether these fields silently never populate from the list endpoint,
  is not determinable from the code.
- **OPEN QUESTION:** The client sends `order_by=average_rating` and `order_by=created_at`
  (`grid_launcher/server/discover.py:481`, `:631`), but the spec declares `order_by` as a free
  string with no enumerated values. Whether unknown field names are rejected, ignored, or
  silently fall back to name ordering is unspecified.
- **OPEN QUESTION:** The "short game" threshold is the bare number 1200 compared against
  `hltb_metadata.main_story` (`grid_launcher/server/discover.py:293`). The spec types
  `main_story` as an integer without units. If the unit is minutes the threshold is 20 hours; if
  seconds it is 20 minutes. The code carries no unit annotation.
- **OPEN QUESTION:** `resolve_ra_game_id` accepts `username` and `api_key` parameters and uses
  neither (`grid_launcher/server/retroachievements.py:126`). It is unclear whether a
  credential-based lookup (e.g. a title search) was intended and dropped, or whether the
  parameters are vestigial.
- **OPEN QUESTION:** `_normalize_achievement` converts the achievement id with an integer
  conversion that will raise on a non-numeric map key or `ID`
  (`grid_launcher/server/retroachievements.py:60`). The surrounding loop has no guard, so one
  malformed entry aborts the whole list. Whether this is intended strictness or an oversight is
  unclear.
- **OPEN QUESTION:** `DiscoverCache.clear` mutates the cache without taking the mutex that every
  other method takes (`grid_launcher/server/discover.py:107` vs `:60`, `:77`, `:89`, `:101`).
  `save_to_disk` also serializes the map unlocked (`grid_launcher/server/discover.py:119`).
  Whether these are intentional (called only from the UI thread) or latent races is not stated.
- **OPEN QUESTION:** `_fetch_roms` mutates the caller's parameter object in place when applying
  its `with_char_index` / `with_filter_values` defaults
  (`grid_launcher/server/discover.py:156`). All current callers pass a fresh literal, so the
  side effect is unobservable, but a port must decide whether to preserve it.
- **OPEN QUESTION:** `fetch_short_games` accepts a `limit` argument but always requests 100
  items, using `limit` only for post-fetch truncation
  (`grid_launcher/server/discover.py:281` vs `:308`). Whether the fixed 100 is a deliberate
  sampling window or a leftover is not documented.
- **OPEN QUESTION:** The Discover worker is constructed with `config["server_url"].rstrip("/")`
  (`grid-launcher.py:1243`), which does not trim surrounding whitespace, while every other
  caller uses the canonical form that trims first (`grid_launcher/server/state.py:18`). A
  configured URL with trailing whitespace would behave differently in Discover than elsewhere.
- **OPEN QUESTION:** `_upgrade_visible_server_cards` builds a `remaining` list that is never
  appended to, so the branch that would keep a row pending is dead
  (`grid_launcher/server/view.py:250`, `:264`). Whether partial-row upgrades were intended is
  unclear; behaviorally every touched row is marked complete.
- **OPEN QUESTION:** Xbox 360 file categorization reuses the function named for PS4
  (`grid_launcher/server/catalog.py:301` calls `_ps4_file_ids_by_category`). The behavior is
  identical for both, but it is not stated whether the two are meant to stay coupled.
- **OPEN QUESTION:** `_format_release_date` treats integers in the range 0–1899 as epoch seconds
  and formats them as full dates in 1970 (`grid_launcher/server/metadata.py:210`), while
  `_extract_year` rejects the same range (`grid_launcher/server/metadata.py:190`). The two
  functions disagree on the same input.
- **OPEN QUESTION:** `format_http_error_details` consumes the error body
  (`grid_launcher/core/api.py:40`), so any caller that later tries to read the same error's body
  gets nothing. No caller currently does, but the ordering constraint is undocumented.
- **OPEN QUESTION:** `multipart_payload` silently skips files that are missing at encode time
  (`grid_launcher/core/api.py:93`), which can produce a request body containing only the
  terminator. Whether the server treats that as an empty upload or an error is not covered by
  any test or by the code.
- **OPEN QUESTION:** The 401 branch of failure classification returns an empty `dialog_text`
  (`grid_launcher/server/connection.py:21`) and relies entirely on the caller running a
  re-authentication dialog. A port that ignores the `token_expired` flag would show no feedback
  at all.

## Source map

| Topic | Location |
|-------|----------|
| Bundled CA / TLS context override | `grid_launcher/__init__.py:5` |
| Bearer header construction | `grid_launcher/core/api.py:15` |
| Binary-download header override | `grid_launcher/core/api.py:19` |
| HTTP error message formatting | `grid_launcher/core/api.py:25` |
| URL + query construction (repeated keys for lists) | `grid_launcher/core/api.py:59` |
| JSON GET (10 s timeout) | `grid_launcher/core/api.py:69` |
| Binary GET (60 s timeout) | `grid_launcher/core/api.py:79` |
| Multipart file encoding | `grid_launcher/core/api.py:88` |
| Multipart text-field encoding | `grid_launcher/core/api.py:111` |
| PUT / POST multipart-text JSON | `grid_launcher/core/api.py:125`, `:145` |
| POST / PUT multipart-file JSON | `grid_launcher/core/api.py:165`, `:185` |
| POST JSON with tolerant response parsing | `grid_launcher/core/api.py:205` |
| Secret storage: keyring accounts, Windows fallback, legacy migration | `grid_launcher/core/token_store.py:11`, `:129`, `:159` |
| Credentials-present / base URL / account status | `grid_launcher/server/state.py:6`, `:14`, `:21` |
| Connection failure classification | `grid_launcher/server/connection.py:15` |
| Connect request pair ordering | `grid_launcher/server/orchestrator.py:6` |
| Status-label application | `grid_launcher/server/status.py:14` |
| Username extraction | `grid_launcher/server/catalog.py:14` |
| Platform display-label derivation | `grid_launcher/server/catalog.py:23` |
| Platform id map / slug map / detail records | `grid_launcher/server/catalog.py:37`, `:67`, `:98` |
| ROM list pagination | `grid_launcher/server/catalog.py:153` |
| Single-shot ROM fetch by params | `grid_launcher/server/catalog.py:195` |
| PS4 / Xbox 360 platform detection | `grid_launcher/server/catalog.py:208`, `:227` |
| File-category grouping | `grid_launcher/server/catalog.py:246` |
| Catalog game-record projection | `grid_launcher/server/catalog.py:274` |
| Filename candidate resolution | `grid_launcher/server/details_cache.py:10` |
| ROM detail fetch + payload unwrapping | `grid_launcher/server/details_cache.py:58` |
| Persisted rom-id cache (key, normalize, store, clear) | `grid_launcher/server/details_cache.py:97`, `:110`, `:123`, `:140` |
| Filename resolution with retry | `grid_launcher/server/details_cache.py:161` |
| Rom-id resolution order | `grid_launcher/server/details_cache.py:197` |
| Provider priority list | `grid_launcher/server/metadata.py:8` |
| Rating scale inference / normalization / formatting | `grid_launcher/server/metadata.py:107`, `:155`, `:172` |
| Year extraction / date formatting | `grid_launcher/server/metadata.py:179`, `:204` |
| Full metadata merge | `grid_launcher/server/metadata.py:231` |
| Discover in-memory cache with TTL and mutex | `grid_launcher/server/discover.py:12` |
| Discover cache disk persistence | `grid_launcher/server/discover.py:111`, `:124` |
| Shared `/api/roms` fetch wrapper | `grid_launcher/server/discover.py:149` |
| Genre extraction from `filter_values` | `grid_launcher/server/discover.py:168` |
| Discover item normalization | `grid_launcher/server/discover.py:187` |
| All-games + genres single call | `grid_launcher/server/discover.py:250` |
| Short-games selection | `grid_launcher/server/discover.py:274` |
| Games by genre | `grid_launcher/server/discover.py:314` |
| Installed filter / client filter | `grid_launcher/server/discover.py:325`, `:341` |
| Genre totals | `grid_launcher/server/discover.py:366` |
| Genre statistics with server-total override | `grid_launcher/server/discover.py:400` |
| Platform fetch / unexplored selection | `grid_launcher/server/discover.py:435`, `:446` |
| Highly rated (client-side threshold) | `grid_launcher/server/discover.py:467` |
| Games by platform | `grid_launcher/server/discover.py:497` |
| Recommendations | `grid_launcher/server/discover.py:517` |
| Analytics event append | `grid_launcher/server/discover.py:561` |
| Watchlist load / save | `grid_launcher/server/discover.py:582`, `:608` |
| New games | `grid_launcher/server/discover.py:621` |
| Static platform metadata table | `grid_launcher/server/platform_metadata.py:3` |
| Platform logo filename table | `grid_launcher/server/platform_metadata.py:226` |
| Normalized-name logo index build | `grid_launcher/server/platform_metadata.py:345` |
| Logo resolution (name first, slug fallback) | `grid_launcher/server/platform_metadata.py:368` |
| RA base URLs | `grid_launcher/server/retroachievements.py:8` |
| RA id validation | `grid_launcher/server/retroachievements.py:16` |
| RA fetch + payload error detection | `grid_launcher/server/retroachievements.py:23` |
| RA achievement normalization | `grid_launcher/server/retroachievements.py:49` |
| RA login | `grid_launcher/server/retroachievements.py:69` |
| RA endpoint selection by credentials | `grid_launcher/server/retroachievements.py:94` |
| RA id resolution from a game record | `grid_launcher/server/retroachievements.py:126` |
| PCGW API base | `grid_launcher/server/pcgamingwiki.py:12` |
| PCGW path-variable table | `grid_launcher/server/pcgamingwiki.py:15` |
| PCGW path expansion | `grid_launcher/server/pcgamingwiki.py:52` |
| PCGW balanced-template extraction | `grid_launcher/server/pcgamingwiki.py:76` |
| PCGW Windows save-path parsing | `grid_launcher/server/pcgamingwiki.py:96` |
| PCGW fetch + error mapping | `grid_launcher/server/pcgamingwiki.py:128` |
| PCGW template argument splitting | `grid_launcher/server/pcgamingwiki.py:153` |
| PCGW page-id query URL / extraction | `grid_launcher/server/pcgamingwiki.py:185`, `:190` |
| PCGW title-from-URL extraction | `grid_launcher/server/pcgamingwiki.py:208` |
| PCGW page-id resolution with OpenSearch fallback | `grid_launcher/server/pcgamingwiki.py:218` |
| PCGW wikitext fetch | `grid_launcher/server/pcgamingwiki.py:250` |
| PCGW end-to-end save-path lookup | `grid_launcher/server/pcgamingwiki.py:264` |
| Connection-data reset | `grid_launcher/server/view.py:61` |
| Platform list population | `grid_launcher/server/view.py:76` |
| Server search filter | `grid_launcher/server/view.py:107` |
| Lazy grid render with generation guard | `grid_launcher/server/view.py:156`, `:220` |
| Client API wrappers (base URL + token injection) | `grid-launcher.py:2981`–`:3017` |
| Connect flow and failure handling | `grid-launcher.py:3022` |
| Background platform game load | `grid-launcher.py:3077`, `:3124` |
| Config/cache file paths | `grid-launcher.py:2386`–`:2453` |
| Watchlist toggle / section / hydration | `grid-launcher.py:2410`, `:2428`, `:2435` |
| Discover initialization and auto-refresh timer | `grid-launcher.py:499`–`:524` |
| Discover open / refresh / offline paths | `grid-launcher.py:791`, `:1154`, `:1226` |
| Discover cache-only result assembly | `grid-launcher.py:1183` |
| Discover thread start and completion | `grid-launcher.py:1237`, `:1258` |
| Discover section assembly and disk write | `grid-launcher.py:1271`, `:1411` |
| ROM detail worker | `grid_launcher/background/workers.py:760` |
| RA achievements / login workers | `grid_launcher/background/workers.py:779`, `:799` |
| PCGW worker | `grid_launcher/background/workers.py:819` |
| Cover replenish worker | `grid_launcher/background/workers.py:836` |
| Discover load worker (sections, pool, error rule) | `grid_launcher/background/workers.py:885`, `:1019` |
| Detail-backfill gating and merge | `grid_launcher/ui/mixins/details_view_mixin.py:185`, `:199`, `:228` |
| Achievements panel credential sourcing | `grid_launcher/ui/mixins/details_view_mixin.py:1885`, `:1912` |
| Save/state listing, download, delete, upload | `grid_launcher/ui/mixins/cloud_mixin.py:1592`, `:1650`, `:1745`, `:1775`, `:2478` |
| ROM content download paths | `grid_launcher/ui/mixins/install_mixin.py:931`, `:1012`, `:1277` |
| Firmware listing and download | `grid_launcher/library/firmware_install.py:27`, `:34` |
| Collections fetch/create/update | `grid_launcher/tv/bridge/workers.py:206`, `:246`, `:268` |
| Cover/screenshot URL resolution and host filter | `grid_launcher/cover/utils.py:28`, `:47`, `:63`, `:93` |
