# Desktop UI redesign 3 — Game details popup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the details popup from one long scrolling column into the redesign's cover-left dialog: a fixed 240px action column, a right header, and four tabs (Overview, Media, Saves, Files) with a fullscreen media viewer and the D-UI-10 version rule.

**Architecture:** `Details.svelte` becomes a shell — left column, header, tab bar, one tab component each — and every rule it needs lives in a pure module under `app/src/lib/details/` that vitest covers (`tabs.ts`, `header.ts`, `related.ts`, `media.ts`, plus a `version.ts` extension). The backend grows only what the new tabs read: seven `#[serde(default)]` fields on `RomDetail` (`franchises`, `game_modes`, `player_count`, `youtube_video_id`, `video_path`, `is_identified`, `related`), `last_modified` on `RomFile`, and one `ensure_video` command that pulls a server-hosted trailer through the session client into the existing image cache directory so no token ever reaches a frontend URL.

**Tech Stack:** Rust (grid-core `romm` + `images`, Tauri 2 `app` crate), Svelte 5 runes + TypeScript + vitest, WebdriverIO E2E against the mock RomM server.

**Spec:** `docs/superpowers/specs/2026-09-04-desktop-ui-redesign-design.md` — binding. This plan implements **delivery item 3 only** (§12.3): §7 Game details popup, D-UI-4, D-UI-10, and the §11 new ids `details-tab-<name>` and `media-viewer`. Plans 4 (Downloads segments and sparklines) and 5 (Emulators/Settings rails, removal of the old modal ids) are explicitly NOT implemented here.

All paths below are relative to `rewrite/` unless they start with `docs/`.

## Deliberate deviations from §7, and why

Each of these is a decision the plan makes against the spec text. They are listed here so a reviewer can reject the decision, not discover it buried in a task.

- **No gear popover.** §7 asks for a "gear menu (native game settings, emulator override, remove)". Hiding those behind a click would break the direct `details-game-settings` click in `native.spec.ts` and the direct `details-uninstall` two-click in `install-b.spec.ts`, and there is no per-game emulator override command in the backend at all (only `save_default_emulator`, which is per platform and belongs to plan 5's Emulators view). The left column therefore renders **Game Settings** and **Uninstall** as an always-visible secondary stack under the primary button, with their existing ids. Nothing is lost; one click is saved.
- **Cloud status button says "Cloud saves" / "Not configured", not "Synced 2h ago".** The relative time comes from `cloud_records`, a per-game IPC round trip the popup otherwise never makes before the user opens the Saves tab. The Saves tab shows a real per-record relative time on every row; the left button routes to it.
- **Rating stays `metadatum.average_rating`.** §7 names `igdb_metadata.total_rating`. `average_rating` is RomM's own merged rating, is already deserialised into `RomDetail::rating`, and is already asserted by `romm_detail.rs`. `total_rating` is a second value on a different scale (0–100 vs 0–10 in the fixtures), so showing whichever happened to be present would make the number meaningless. One source, kept.
- **`user_saves` / `user_states` are read through the existing cloud commands, not added to `RomDetail`.** `cloud_records(game, saveType)` already returns exactly what §7's Saves tab lists — file name, emulator, size text, absolute and relative time — from those same RomM endpoints, with the restore/delete rules and their verbatim copy already ported and E2E-covered. Adding a second, thinner copy of the same list to `RomDetail` would give the tab two sources that can disagree.
- **Related games carry no cover art.** `igdb_metadata.similar_games[].cover_url` points at `images.igdb.com`; `filter_to_server_host` (doc 07) drops every non-server host, so those covers can never load. The Related row is a row of title chips.
- **Video E2E asserts the embed's `src` attribute, not playback.** The mock server has no real video bytes and the E2E host has no network, so a spec that waits for a playing `<video>`/YouTube frame would be a false green. The `ensure_video` path is covered by Rust unit tests instead.

## Global Constraints

- **Token secrecy (hard):** tokens live only in the OS keyring and the redacting in-memory type; never in files, logs, errors, IPC, or console output. **Every image, video and save byte fetched from the server goes through the session client into the local cache — no frontend URL ever carries the token.** Covers, screenshots and trailers reach the DOM as `convertFileSrc(<cached path>)` only.
- **Popup:** dialog **1040×680 max**, centred over a **dimmed, blurred** shell; **Esc and ✕ close**; test id **`details-panel` kept**.
- **Left column, 240px:** cover (`path_cover_large` via `Image.svelte`), Play / Install primary (existing ids `details-play`, `details-install`, `details-stop`, `details-playing-chip` kept), Update (`details-update`, **existing label rules and native confirm text kept verbatim**), cloud status button (existing cloud panel behaviour, opened by `initialCloudMode`), the secondary stack (native game settings → the existing `NativeSettings`, remove), play time and **the emulator + core that will launch** (from `get_launch_defaults` + the installed row).
- **Right header:** title, platform, first release date, developer, genres, rating, region / language flags, verification state (`is_identified`).
- **Tabs `details-tab-overview` / `-media` / `-saves` / `-files`**, last tab remembered per session.
  - **Overview:** `summary`, metadata grid, screenshot strip (**first six**), Related row **filtered to titles present on the server** (client-side against the platform's game list).
  - **Media:** screenshot gallery + video; click opens a fullscreen viewer **`media-viewer`** with arrows, Esc, and a caption.
  - **Saves:** `user_saves` / `user_states` with timestamps and sizes, last cloud sync, Upload / Download / Sync now (existing cloud panel actions), **the existing cloud scope warning**.
  - **Files:** `files[]` name / size / `last_modified`, installed vs server version **per D-UI-10** (parsed version tag when present, else the file's `last_modified` date) with the Update button, PS4 / Xbox 360 content rows (existing content flow), firmware row.
- **Every string E2E asserts today stays verbatim**: the update toast `Updated '<title>' successfully.`, the native confirm `Saves and configuration will be preserved — confirm update`, the launch errors, and the `details-error` / `details-warning` texts.
- **Only `app.css` tokens for colours**; views and the popup use the `--m-*` motion tokens.
- **Every task ends with**, from `rewrite/`: `cargo fmt`; `cargo clippy --workspace --all-targets -- -D warnings` and `cargo clippy -p app --all-targets --features e2e -- -D warnings` clean; `cargo test --workspace` green **when Rust changed**; and from `rewrite/app`: `npm run check` and `npx vitest run` green. Then a commit whose subject starts `rewrite: `. **The final task runs every E2E group (`scripts/e2e.sh` with no argument) and must be green.**
- **Never** run `git checkout`, `git restore`, `git reset`, or `git stash`. Commit with explicit pathspecs.
- **No component test harness exists** in this repo (no `@testing-library/svelte`, no jsdom). Every `.svelte` change is verified by an extracted, unit-tested pure module plus `npm run check` and E2E — never by a fabricated component test.

---

## File map

| File | Responsibility |
|---|---|
| `crates/grid-core/src/romm/mod.rs` | `RomDetail` + `RomFile` new fields, `RawIgdbMetadata`, `RelatedGame` |
| `crates/grid-core/tests/romm_detail.rs` | deserialisation coverage for the new fields |
| `crates/grid-core/src/images/video.rs` (+ `mod.rs`) | `video_extension_for`, `ensure_video` |
| `crates/grid-core/tests/images_video.rs` | video cache coverage |
| `app/src-tauri/src/commands.rs` | `ensure_video` command |
| `app/src-tauri/src/lib.rs` | handler registration for `ensure_video` |
| `app/src-tauri/tauri.conf.json` | CSP `frame-src` + `media-src` |
| `app/src/lib/api.ts` | `RomDetail` / `RomFile` / `RelatedGame` types, `ensureVideo` |
| `app/src/lib/details/tabs.ts` (+ test) | the four tabs and the remembered tab |
| `app/src/lib/details/header.ts` (+ test) | header line, developer, release year, rating, flags, verification |
| `app/src/lib/details/related.ts` (+ test) | filter related titles to the platform's server list |
| `app/src/lib/details/media.ts` (+ test) | gallery items, viewer navigation, YouTube embed URL |
| `app/src/lib/details/version.ts` (+ test) | `fileVersionLabel` — D-UI-10 |
| `app/src/lib/details/files.ts` (+ test) | file rows: size text, content-category rows |
| `app/src/lib/Details.svelte` | the shell: left column, header, tab bar |
| `app/src/lib/details/OverviewTab.svelte` | summary, metadata grid, screenshot strip, Related |
| `app/src/lib/details/MediaTab.svelte` | gallery tiles |
| `app/src/lib/details/MediaViewer.svelte` | fullscreen viewer, arrows, Esc, caption |
| `app/src/lib/details/SavesTab.svelte` | cloud toggles + `CloudPanel` |
| `app/src/lib/details/FilesTab.svelte` | file rows, content rows, firmware row |
| `e2e/fixtures/rom-details.json`, `e2e/fixtures-updates/rom-details.json` | new metadata for the specs |
| `e2e/specs/images-a.spec.ts`, `e2e/specs/updates.spec.ts`, `e2e/specs/cloud-saves.spec.ts` | tab, header, viewer and version cases; the Saves-tab click the cloud helper now needs |
| `SPEC.md`, `rewrite/README.md`, `docs/porting/07-covers-images.md` | documentation |

---

### Task 1: The detail fields the new tabs read

**Files:**
- Modify: `crates/grid-core/src/romm/mod.rs:332-348` (`RomFile`), `:356-384` (`RomDetail`), `:389-398` (`RawRomMetadata`), `:404-444` (`RawRomDetail`), `:445-486` (`into_detail`)
- Modify: `crates/grid-core/tests/romm_detail.rs` (append tests at the tail; extend the two existing payload tests)
- Modify: `app/src/lib/api.ts:33-39` (`RomFile`), `:41-61` (`RomDetail`)

**Interfaces:**
- Consumes: `grid_core::romm::{RommClient, RomDetail, RomFile}`; `crate::images::urls::server_resolver`.
- Produces, used by Tasks 2, 4, 5, 7, 8 and 9:
  - `RomFile` gains `pub last_modified: String` (`""` when absent).
  - `RomDetail` gains `pub franchises: String`, `pub game_modes: String`, `pub player_count: String`, `pub youtube_video_id: String`, `pub video_path: String`, `pub is_identified: bool`, `pub related: Vec<RelatedGame>`.
  - `pub struct RelatedGame { pub name: String, pub kind: String }` where `kind` is one of `"similar"`, `"remake"`, `"remaster"`, `"dlc"`, `"expansion"`, in that list order.
  - TS mirrors, exactly: `RomFile` gains `last_modified: string`; `RomDetail` gains `franchises: string; game_modes: string; player_count: string; youtube_video_id: string; video_path: string; is_identified: boolean; related: RelatedGame[]`; `export type RelatedGame = { name: string; kind: string }`.

- [ ] **Step 1: Write the failing deserialisation tests**

Append to `crates/grid-core/tests/romm_detail.rs`:

```rust
#[tokio::test]
async fn rom_detail_maps_the_igdb_block_and_the_media_fields() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/roms/77"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 77,
            "name": "Super Game",
            "fs_name_no_ext": "super_game",
            "platform_id": 7,
            "platform_display_name": "SNES",
            "fs_name": "super_game.zip",
            "summary": "A great game.",
            "regions": ["USA"],
            "languages": ["en"],
            "tags": [],
            "revision": null,
            "fs_size_bytes": 0,
            "updated_at": "2026-01-01T00:00:00",
            "is_identified": true,
            "youtube_video_id": "dQw4w9WgXcQ",
            "path_video": "/assets/romm/resources/roms/77/video.mp4",
            "files": [{
                "id": 1,
                "file_name": "super_game.sfc",
                "file_size_bytes": 10,
                "is_top_level": true,
                "last_modified": "2026-02-03T11:22:33"
            }],
            "metadatum": {
                "average_rating": 87.34,
                "genres": ["Platformer"],
                "companies": ["Nintendo"],
                "first_release_date": 631152000,
                "franchises": ["Mario", "Super Mario"],
                "game_modes": ["Single player"],
                "player_count": "1"
            },
            "igdb_metadata": {
                "similar_games": [{"id": 1, "name": "Sim One", "slug": "s1", "type": "game", "cover_url": "https://images.igdb.com/a.jpg"}],
                "remakes": [{"id": 2, "name": "Remake One", "slug": "r1", "type": "game", "cover_url": ""}],
                "remasters": [{"id": 3, "name": "Remaster One", "slug": "rr1", "type": "game", "cover_url": ""}],
                "dlcs": [{"id": 4, "name": "DLC One", "slug": "d1", "type": "dlc", "cover_url": ""}],
                "expansions": [{"id": 5, "name": "Expansion One", "slug": "e1", "type": "expansion", "cover_url": ""}]
            }
        })))
        .mount(&server)
        .await;
    let client = RommClient::new(&server.uri(), token_cred()).unwrap();
    let detail = client.rom_detail(77).await.unwrap();

    assert_eq!(detail.franchises, "Mario, Super Mario");
    assert_eq!(detail.game_modes, "Single player");
    assert_eq!(detail.player_count, "1");
    assert_eq!(detail.youtube_video_id, "dQw4w9WgXcQ");
    assert_eq!(detail.video_path, "/assets/romm/resources/roms/77/video.mp4");
    assert!(detail.is_identified);
    assert_eq!(detail.files[0].last_modified, "2026-02-03T11:22:33");

    // Source order is fixed by `into_detail`: similar, remake, remaster,
    // dlc, expansion — so the Overview row cannot reshuffle between builds.
    let related: Vec<(&str, &str)> = detail
        .related
        .iter()
        .map(|r| (r.name.as_str(), r.kind.as_str()))
        .collect();
    assert_eq!(
        related,
        vec![
            ("Sim One", "similar"),
            ("Remake One", "remake"),
            ("Remaster One", "remaster"),
            ("DLC One", "dlc"),
            ("Expansion One", "expansion"),
        ]
    );
}

#[tokio::test]
async fn rom_detail_without_an_igdb_block_reports_empty_media_and_no_related() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/roms/78"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 78,
            "fs_name": "g.zip",
            "fs_name_no_ext": "g",
            "platform_id": 2,
            "platform_display_name": "SNES",
            "fs_size_bytes": 0,
            "updated_at": "",
            "regions": [],
            "languages": [],
            "tags": [],
            "files": [{"id": 1, "file_name": "g.sfc", "file_size_bytes": 0, "is_top_level": true}],
            "name": null,
            "summary": null,
            "revision": null,
            "metadatum": null,
            "igdb_metadata": null,
            "youtube_video_id": null,
            "path_video": null
        })))
        .mount(&server)
        .await;
    let client = RommClient::new(&server.uri(), token_cred()).unwrap();
    let detail = client.rom_detail(78).await.unwrap();

    assert_eq!(detail.franchises, "");
    assert_eq!(detail.game_modes, "");
    assert_eq!(detail.player_count, "");
    assert_eq!(detail.youtube_video_id, "");
    assert_eq!(detail.video_path, "");
    // `is_identified` is absent from this payload entirely: a server that
    // never sends the flag must read as "not identified", not fail the decode.
    assert!(!detail.is_identified);
    assert!(detail.related.is_empty());
    assert_eq!(detail.files[0].last_modified, "");
}

#[tokio::test]
async fn rom_detail_still_reads_merged_screenshots_now_that_igdb_is_a_named_field() {
    // Regression guard: `merged_screenshots` is read out of RawRomDetail's
    // `#[serde(flatten)] extra` map. Naming any new field removes it from
    // that map, so this pins that the screenshot source survived Task 1.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/roms/79"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 79,
            "fs_name": "g.zip",
            "fs_name_no_ext": "g",
            "platform_id": 2,
            "platform_display_name": "SNES",
            "fs_size_bytes": 0,
            "updated_at": "",
            "regions": [],
            "languages": [],
            "tags": [],
            "files": [],
            "name": null,
            "summary": null,
            "revision": null,
            "metadatum": null,
            "igdb_metadata": {"similar_games": []},
            "merged_screenshots": ["/assets/romm/resources/roms/79/screenshots/1.png"]
        })))
        .mount(&server)
        .await;
    let client = RommClient::new(&server.uri(), token_cred()).unwrap();
    let detail = client.rom_detail(79).await.unwrap();
    assert_eq!(
        detail.screenshot_urls,
        vec![format!(
            "{}/assets/romm/resources/roms/79/screenshots/1.png",
            server.uri()
        )]
    );
}
```

- [ ] **Step 2: Run them to verify they fail**

Run from `rewrite/`: `cargo test -p grid-core --test romm_detail`
Expected: FAIL to compile — `no field 'franchises' on type 'RomDetail'`.

- [ ] **Step 3: Add the fields to grid-core**

In `crates/grid-core/src/romm/mod.rs`, add to `RomFile` (after `is_top_level`, before `category`):

```rust
    /// The file's own last-modified timestamp as the server states it
    /// (ISO 8601, e.g. `2026-02-03T11:22:33`). `""` when the server does
    /// not send one. D-UI-10 falls back to this when a file name carries
    /// no version tag, so it is kept verbatim and formatted in the UI.
    #[serde(default, deserialize_with = "null_to_empty")]
    pub last_modified: String,
```

Add above `RomDetail`:

```rust
/// One entry of the details Overview "Related" row. IGDB's own cover URLs
/// live on `images.igdb.com`, which `filter_to_server_host` (doc 07) drops,
/// so only the title and which list it came from are carried.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct RelatedGame {
    pub name: String,
    /// `"similar"`, `"remake"`, `"remaster"`, `"dlc"` or `"expansion"`.
    pub kind: String,
}
```

Add to `RomDetail`, after `first_release_date`:

```rust
    /// `metadatum.franchises`, comma-joined — same convention as `genres`.
    pub franchises: String,
    /// `metadatum.game_modes`, comma-joined.
    pub game_modes: String,
    /// `metadatum.player_count`, verbatim (the server sends a free-form
    /// string such as `"1"` or `"1-4"`).
    pub player_count: String,
```

and after `screenshot_urls`:

```rust
    /// `youtube_video_id`, `""` when the server has none. The frontend
    /// embeds it; it is never resolved to a URL here.
    pub youtube_video_id: String,
    /// `path_video`, verbatim from the server (server-relative) — resolved
    /// lazily against the server URL by `ensure_video`, exactly as the
    /// cover paths are by `ensure_image`. Never a token-bearing URL.
    pub video_path: String,
    /// RomM's `is_identified`: the game was matched against a metadata
    /// provider. `false` when the server omits the flag.
    pub is_identified: bool,
    /// The Overview "Related" row, in source-list order.
    pub related: Vec<RelatedGame>,
```

Add the three metadata fields to `RawRomMetadata`:

```rust
    #[serde(default)]
    franchises: Vec<String>,
    #[serde(default)]
    game_modes: Vec<String>,
    #[serde(default)]
    player_count: String,
```

Add, directly below `RawRomMetadata`:

```rust
/// One `IGDBRelatedGame`. Only `name` is used; the rest of the wire shape
/// (`id`, `slug`, `type`, `cover_url`) is ignored by omission.
#[derive(Deserialize, Default)]
struct RawRelatedGame {
    #[serde(default)]
    name: String,
}

/// Wire shape of the `RomIGDBMetadata` lists §7's Related row reads. Every
/// field defaulted: a null `igdb_metadata`, or one with only some lists,
/// must never fail the outer decode.
#[derive(Deserialize, Default)]
struct RawIgdbMetadata {
    #[serde(default)]
    similar_games: Vec<RawRelatedGame>,
    #[serde(default)]
    remakes: Vec<RawRelatedGame>,
    #[serde(default)]
    remasters: Vec<RawRelatedGame>,
    #[serde(default)]
    dlcs: Vec<RawRelatedGame>,
    #[serde(default)]
    expansions: Vec<RawRelatedGame>,
}

impl RawIgdbMetadata {
    /// Flattens the five lists into one row, in a FIXED order, dropping
    /// blank names and any title already present (IGDB repeats a title
    /// across lists often enough that the row would otherwise stutter).
    fn into_related(self) -> Vec<RelatedGame> {
        let lists = [
            ("similar", self.similar_games),
            ("remake", self.remakes),
            ("remaster", self.remasters),
            ("dlc", self.dlcs),
            ("expansion", self.expansions),
        ];
        let mut out: Vec<RelatedGame> = Vec::new();
        for (kind, list) in lists {
            for entry in list {
                let name = entry.name.trim().to_string();
                if name.is_empty() || out.iter().any(|r| r.name == name) {
                    continue;
                }
                out.push(RelatedGame {
                    name,
                    kind: kind.to_string(),
                });
            }
        }
        out
    }
}
```

Add to `RawRomDetail`, directly above the `#[serde(flatten)] extra` field:

```rust
    #[serde(default)]
    igdb_metadata: Option<RawIgdbMetadata>,
    #[serde(default)]
    youtube_video_id: Option<String>,
    #[serde(default)]
    path_video: Option<String>,
    #[serde(default)]
    is_identified: bool,
```

In `into_detail`, bind the IGDB block beside the existing `metadatum` line:

```rust
        let metadatum = self.metadatum.unwrap_or_default();
        let igdb = self.igdb_metadata.unwrap_or_default();
```

and add these to the returned `RomDetail`, keeping the field order of the struct:

```rust
            franchises: metadatum.franchises.join(", "),
            game_modes: metadatum.game_modes.join(", "),
            player_count: metadatum.player_count,
            youtube_video_id: self.youtube_video_id.unwrap_or_default(),
            video_path: self.path_video.unwrap_or_default(),
            is_identified: self.is_identified,
            related: igdb.into_related(),
```

- [ ] **Step 4: Run the tests to verify they pass**

Run from `rewrite/`: `cargo test -p grid-core --test romm_detail`
Expected: PASS — the three new tests and all five pre-existing ones.

- [ ] **Step 5: Mirror the fields in `api.ts`**

In `app/src/lib/api.ts`, add to `RomFile` after `is_top_level`:

```ts
  /** ISO 8601 as the server states it, or `''`. D-UI-10's fallback. */
  last_modified: string;
```

Add above `RomDetail`:

```ts
/** One Overview "Related" chip. Mirrors `grid_core::romm::RelatedGame`. */
export type RelatedGame = { name: string; kind: string };
```

Add to `RomDetail` after `first_release_date`:

```ts
  franchises: string;
  game_modes: string;
  player_count: string;
```

and after `screenshot_urls`:

```ts
  youtube_video_id: string;
  video_path: string;
  is_identified: boolean;
  related: RelatedGame[];
```

- [ ] **Step 6: Run the frontend gates**

Run from `rewrite/app`: `npm run check` then `npx vitest run`
Expected: both green. `svelte-check` reports 0 errors — `Details.svelte` reads none of the new fields yet, so nothing else has to change.

- [ ] **Step 7: Full gate and commit**

Run from `rewrite/`: `cargo fmt`, then `cargo clippy --workspace --all-targets -- -D warnings`, then `cargo clippy -p app --all-targets --features e2e -- -D warnings`, then `cargo test --workspace`.
Expected: all clean/green.

```bash
cd /home/six/Documents/Programming/grid-launcher
git add rewrite/crates/grid-core/src/romm/mod.rs rewrite/crates/grid-core/tests/romm_detail.rs rewrite/app/src/lib/api.ts
git commit -m "rewrite: deserialise the rom detail fields the details tabs need"
```

---

### Task 2: The pure helpers behind the header, the tabs, Related, media and D-UI-10

Everything the redesigned popup decides that does not need the DOM. No `.svelte`, store or `api` import in any of these modules, so vitest reaches every rule directly.

**Files:**
- Create: `app/src/lib/details/tabs.ts`, `app/src/lib/details/tabs.test.ts`
- Create: `app/src/lib/details/header.ts`, `app/src/lib/details/header.test.ts`
- Create: `app/src/lib/details/related.ts`, `app/src/lib/details/related.test.ts`
- Create: `app/src/lib/details/media.ts`, `app/src/lib/details/media.test.ts`
- Create: `app/src/lib/details/files.ts`, `app/src/lib/details/files.test.ts`
- Modify: `app/src/lib/details/version.ts:56` (append), `app/src/lib/details/version.test.ts` (append)

**Interfaces:**
- Consumes: `RelatedGame` and `RomFile` from `../api` (Task 1) — types only.
- Produces, used by Tasks 4–9:
  - `tabs.ts`: `type DetailsTab = 'overview' | 'media' | 'saves' | 'files'`; `DETAILS_TABS: readonly DetailsTab[]`; `DETAILS_TAB_LABELS: Record<DetailsTab, string>`; `tabTestId(tab: DetailsTab): string`; `isDetailsTab(value: string): value is DetailsTab`; `rememberedTab(): DetailsTab`; `rememberTab(tab: DetailsTab): void`; `resetRememberedTab(): void`.
  - `header.ts`: `releaseYear(firstReleaseDate: string): string`; `developerOf(companies: string): string`; `ratingText(rating: string): string`; `flagList(value: string): string[]`; `verificationLabel(isIdentified: boolean): string`; `type HeaderInput`; `headerLine(input: HeaderInput): string`.
  - `related.ts`: `normalizeTitle(title: string): string`; `RELATED_KIND_LABELS: Record<string, string>`; `relatedKindLabel(kind: string): string`; `relatedOnServer(related: RelatedGame[], serverTitles: Iterable<string>): RelatedGame[]`.
  - `media.ts`: `type MediaItem`; `galleryItems(input: MediaGalleryInput): MediaItem[]`; `type MediaGalleryInput`; `youtubeEmbedUrl(videoId: string): string`; `isYoutubeId(value: string): boolean`; `nextIndex(current: number, count: number): number`; `prevIndex(current: number, count: number): number`; `OVERVIEW_STRIP_LIMIT: 6`; `overviewStrip(urls: string[]): string[]`.
  - `files.ts`: `fileSizeText(bytes: number): string`; `type FileRow`; `fileRows(files: RomFile[]): FileRow[]`; `contentRows(files: RomFile[]): FileRow[]`.
  - `version.ts`: `isoDate(value: string): string`; `fileVersionLabel(fileName: string, lastModified: string): string`.

- [ ] **Step 1: Write the failing tab tests**

Create `app/src/lib/details/tabs.test.ts`:

```ts
import { beforeEach, describe, expect, it } from 'vitest';
import {
  DETAILS_TABS,
  DETAILS_TAB_LABELS,
  isDetailsTab,
  rememberTab,
  rememberedTab,
  resetRememberedTab,
  tabTestId,
} from './tabs';

beforeEach(() => resetRememberedTab());

describe('the tab set', () => {
  it('is exactly design §7 four tabs, in order', () => {
    expect(DETAILS_TABS).toEqual(['overview', 'media', 'saves', 'files']);
  });

  it('labels every tab', () => {
    expect(DETAILS_TABS.map((t) => DETAILS_TAB_LABELS[t])).toEqual([
      'Overview',
      'Media',
      'Saves',
      'Files',
    ]);
  });

  it('builds the design §11 test id', () => {
    expect(tabTestId('media')).toBe('details-tab-media');
  });

  it('recognizes only the four names', () => {
    expect(isDetailsTab('files')).toBe(true);
    expect(isDetailsTab('metadata')).toBe(false);
  });
});

describe('the remembered tab', () => {
  it('starts on Overview', () => {
    expect(rememberedTab()).toBe('overview');
  });

  it('remembers the last tab across popup opens within the session', () => {
    rememberTab('saves');
    expect(rememberedTab()).toBe('saves');
  });

  it('is module scoped, so a later read sees the last write', () => {
    rememberTab('files');
    rememberTab('media');
    expect(rememberedTab()).toBe('media');
  });
});
```

- [ ] **Step 2: Run it to verify it fails**

Run from `rewrite/app`: `npx vitest run src/lib/details/tabs.test.ts`
Expected: FAIL — `Failed to resolve import "./tabs"`.

- [ ] **Step 3: Write `tabs.ts`**

Create `app/src/lib/details/tabs.ts`:

```ts
// The details popup's four tabs (design §7) and the session's remembered
// choice. Module scoped rather than stored in config: §7 says "last tab
// remembered per session", so it must survive closing and reopening the
// popup but not survive a restart.

export type DetailsTab = 'overview' | 'media' | 'saves' | 'files';

export const DETAILS_TABS: readonly DetailsTab[] = ['overview', 'media', 'saves', 'files'] as const;

export const DETAILS_TAB_LABELS: Record<DetailsTab, string> = {
  overview: 'Overview',
  media: 'Media',
  saves: 'Saves',
  files: 'Files',
};

/** Design §11's new id for a tab button. */
export function tabTestId(tab: DetailsTab): string {
  return `details-tab-${tab}`;
}

export function isDetailsTab(value: string): value is DetailsTab {
  return (DETAILS_TABS as readonly string[]).includes(value);
}

let remembered: DetailsTab = 'overview';

export function rememberedTab(): DetailsTab {
  return remembered;
}

export function rememberTab(tab: DetailsTab): void {
  remembered = tab;
}

/** Test-only reset, so one spec's choice cannot leak into the next. */
export function resetRememberedTab(): void {
  remembered = 'overview';
}
```

- [ ] **Step 4: Run it to verify it passes**

Run from `rewrite/app`: `npx vitest run src/lib/details/tabs.test.ts`
Expected: PASS, 7 tests.

- [ ] **Step 5: Write the failing header tests**

Create `app/src/lib/details/header.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import {
  developerOf,
  flagList,
  headerLine,
  ratingText,
  releaseYear,
  verificationLabel,
} from './header';

describe('releaseYear', () => {
  it('reads the year out of the epoch-seconds string the backend sends', () => {
    // 631152000 = 1990-01-01T00:00:00Z
    expect(releaseYear('631152000')).toBe('1990');
  });

  it('is blank for a blank date', () => {
    expect(releaseYear('')).toBe('');
  });

  it('is blank for a non-numeric date rather than rendering NaN', () => {
    expect(releaseYear('sometime in 1990')).toBe('');
  });
});

describe('developerOf', () => {
  it('takes the first company as the developer', () => {
    expect(developerOf('Nintendo, Nintendo EAD')).toBe('Nintendo');
  });

  it('trims', () => {
    expect(developerOf('  Konami  ')).toBe('Konami');
  });

  it('is blank when there are no companies', () => {
    expect(developerOf('')).toBe('');
  });
});

describe('ratingText', () => {
  it('stars a rating', () => {
    expect(ratingText('9.2')).toBe('★ 9.2');
  });

  it('is blank for no rating', () => {
    expect(ratingText('   ')).toBe('');
  });
});

describe('flagList', () => {
  it('splits the comma-joined backend string', () => {
    expect(flagList('USA, Europe')).toEqual(['USA', 'Europe']);
  });

  it('drops blanks', () => {
    expect(flagList('USA, , ')).toEqual(['USA']);
  });

  it('is empty for a blank field', () => {
    expect(flagList('')).toEqual([]);
  });
});

describe('verificationLabel', () => {
  it('names both states', () => {
    expect(verificationLabel(true)).toBe('Identified');
    expect(verificationLabel(false)).toBe('Unidentified');
  });
});

describe('headerLine', () => {
  it('joins platform, year, developer, genres and rating with the middot', () => {
    expect(
      headerLine({
        platformName: 'SNES',
        firstReleaseDate: '631152000',
        companies: 'Nintendo',
        genres: 'Platformer',
        rating: '9.2',
      })
    ).toBe('SNES · 1990 · Nintendo · Platformer · ★ 9.2');
  });

  it('drops every part the server has nothing for, with no dangling separator', () => {
    expect(
      headerLine({
        platformName: 'SNES',
        firstReleaseDate: '',
        companies: '',
        genres: '',
        rating: '',
      })
    ).toBe('SNES');
  });

  it('is blank when the server knows nothing at all', () => {
    expect(
      headerLine({ platformName: '', firstReleaseDate: '', companies: '', genres: '', rating: '' })
    ).toBe('');
  });
});
```

- [ ] **Step 6: Run it to verify it fails**

Run from `rewrite/app`: `npx vitest run src/lib/details/header.test.ts`
Expected: FAIL — `Failed to resolve import "./header"`.

- [ ] **Step 7: Write `header.ts`**

Create `app/src/lib/details/header.ts`:

```ts
// The details popup's right-hand header (design §7): title, platform, first
// release date, developer, genres, rating, region/language flags and the
// verification state. Pure — the `.svelte` shell only renders what these
// return.

/**
 * The four-digit year of `first_release_date`. The backend sends IGDB's
 * epoch SECONDS as a string (`romm/mod.rs`'s `into_detail`), so the year is
 * read in UTC: the value is a release date, not a local timestamp, and
 * rendering it in the viewer's zone would move it a day either way.
 */
export function releaseYear(firstReleaseDate: string): string {
  const trimmed = firstReleaseDate.trim();
  if (trimmed === '') return '';
  const epoch = Number(trimmed);
  if (!Number.isFinite(epoch)) return '';
  return String(new Date(epoch * 1000).getUTCFullYear());
}

/**
 * The developer: the first entry of the comma-joined `companies` field.
 * RomM does not separate developer from publisher in `metadatum.companies`
 * — it lists the developer first — so the header names the first and the
 * Overview metadata grid lists all of them.
 */
export function developerOf(companies: string): string {
  return companies.split(',')[0]?.trim() ?? '';
}

/** The header's rating chip, or `''` when the server has no rating. */
export function ratingText(rating: string): string {
  const trimmed = rating.trim();
  return trimmed === '' ? '' : `★ ${trimmed}`;
}

/** Splits a comma-joined backend field (`regions`, `languages`) into flags. */
export function flagList(value: string): string[] {
  return value
    .split(',')
    .map((part) => part.trim())
    .filter((part) => part.length > 0);
}

/** RomM's `is_identified`, in words. */
export function verificationLabel(isIdentified: boolean): string {
  return isIdentified ? 'Identified' : 'Unidentified';
}

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

- [ ] **Step 8: Run it to verify it passes**

Run from `rewrite/app`: `npx vitest run src/lib/details/header.test.ts`
Expected: PASS, 13 tests.

- [ ] **Step 9: Write the failing Related tests**

Create `app/src/lib/details/related.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import { normalizeTitle, relatedKindLabel, relatedOnServer } from './related';

describe('normalizeTitle', () => {
  it('folds case, trims and collapses whitespace', () => {
    expect(normalizeTitle('  Super   Mario  World ')).toBe('super mario world');
  });

  it('drops a trailing region/tag parenthetical, which server file names carry', () => {
    expect(normalizeTitle('Chrono Trigger (USA)')).toBe('chrono trigger');
  });
});

describe('relatedKindLabel', () => {
  it('names each list', () => {
    expect(relatedKindLabel('similar')).toBe('Similar');
    expect(relatedKindLabel('remake')).toBe('Remake');
    expect(relatedKindLabel('remaster')).toBe('Remaster');
    expect(relatedKindLabel('dlc')).toBe('DLC');
    expect(relatedKindLabel('expansion')).toBe('Expansion');
  });

  it('falls back for a kind a newer backend adds', () => {
    expect(relatedKindLabel('port')).toBe('Related');
  });
});

describe('relatedOnServer', () => {
  const related = [
    { name: 'Super Mario World', kind: 'similar' },
    { name: 'Chrono Trigger', kind: 'remake' },
    { name: 'A Game Nobody Owns', kind: 'similar' },
  ];

  it('keeps only titles the platform list actually holds', () => {
    expect(relatedOnServer(related, ['Super Mario World', 'Chrono Trigger (USA)'])).toEqual([
      { name: 'Super Mario World', kind: 'similar' },
      { name: 'Chrono Trigger', kind: 'remake' },
    ]);
  });

  it('keeps the backend order', () => {
    const out = relatedOnServer(related, ['Chrono Trigger', 'Super Mario World']);
    expect(out.map((r) => r.name)).toEqual(['Super Mario World', 'Chrono Trigger']);
  });

  it('is empty when the platform list has not loaded yet', () => {
    expect(relatedOnServer(related, [])).toEqual([]);
  });

  it('drops a duplicate title the two lists both name', () => {
    const dupes = [
      { name: 'Super Mario World', kind: 'similar' },
      { name: 'super mario world', kind: 'remaster' },
    ];
    expect(relatedOnServer(dupes, ['Super Mario World'])).toEqual([
      { name: 'Super Mario World', kind: 'similar' },
    ]);
  });
});
```

- [ ] **Step 10: Run it to verify it fails**

Run from `rewrite/app`: `npx vitest run src/lib/details/related.test.ts`
Expected: FAIL — `Failed to resolve import "./related"`.

- [ ] **Step 11: Write `related.ts`**

Create `app/src/lib/details/related.ts`:

```ts
// Design §7 Overview: the Related row is "filtered to titles present on the
// server". The filter is client-side against the platform's already-loaded
// game list — RomM's search endpoint is out of scope (design §13) — so this
// module owns the title match and nothing else.
import type { RelatedGame } from '../api';

/**
 * A title reduced to what two sources can be compared on: case-folded,
 * whitespace-collapsed, and without the trailing `(USA)`/`(Rev 1)` style
 * parenthetical that server titles derived from file names carry. A
 * parenthetical in the MIDDLE of a title is left alone — it is part of the
 * name there, not a tag.
 */
export function normalizeTitle(title: string): string {
  return title
    .replace(/\s*\([^()]*\)\s*$/g, '')
    .trim()
    .toLowerCase()
    .replace(/\s+/g, ' ');
}

export const RELATED_KIND_LABELS: Record<string, string> = {
  similar: 'Similar',
  remake: 'Remake',
  remaster: 'Remaster',
  dlc: 'DLC',
  expansion: 'Expansion',
};

/** The chip's kind label; a kind a newer backend adds still renders. */
export function relatedKindLabel(kind: string): string {
  return RELATED_KIND_LABELS[kind] ?? 'Related';
}

/**
 * `related` filtered to the entries whose title appears in `serverTitles`,
 * in backend order, with duplicates (IGDB repeats a title across its lists,
 * and normalization can collide two spellings) reduced to the first hit.
 * An empty `serverTitles` yields an empty row: before the platform list
 * loads, the honest answer is "nothing to show", not "everything".
 */
export function relatedOnServer(
  related: RelatedGame[],
  serverTitles: Iterable<string>
): RelatedGame[] {
  const present = new Set<string>();
  for (const title of serverTitles) present.add(normalizeTitle(title));
  const seen = new Set<string>();
  const out: RelatedGame[] = [];
  for (const entry of related) {
    const key = normalizeTitle(entry.name);
    if (key === '' || !present.has(key) || seen.has(key)) continue;
    seen.add(key);
    out.push(entry);
  }
  return out;
}
```

- [ ] **Step 12: Run it to verify it passes**

Run from `rewrite/app`: `npx vitest run src/lib/details/related.test.ts`
Expected: PASS, 8 tests.

- [ ] **Step 13: Write the failing media tests**

Create `app/src/lib/details/media.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import {
  OVERVIEW_STRIP_LIMIT,
  galleryItems,
  isYoutubeId,
  nextIndex,
  overviewStrip,
  prevIndex,
  youtubeEmbedUrl,
} from './media';

describe('galleryItems', () => {
  it('lists every screenshot, numbered from one, then the videos', () => {
    expect(
      galleryItems({
        title: 'Super Mario World',
        screenshotUrls: ['http://s/1.png', 'http://s/2.png'],
        youtubeVideoId: 'dQw4w9WgXcQ',
        videoPath: '/assets/romm/resources/roms/101/video.mp4',
      })
    ).toEqual([
      { kind: 'screenshot', url: 'http://s/1.png', caption: 'Super Mario World — screenshot 1' },
      { kind: 'screenshot', url: 'http://s/2.png', caption: 'Super Mario World — screenshot 2' },
      { kind: 'youtube', videoId: 'dQw4w9WgXcQ', caption: 'Super Mario World — trailer' },
      {
        kind: 'video',
        url: '/assets/romm/resources/roms/101/video.mp4',
        caption: 'Super Mario World — video',
      },
    ]);
  });

  it('omits the YouTube tile when the id is not a YouTube id', () => {
    const items = galleryItems({
      title: 'G',
      screenshotUrls: [],
      youtubeVideoId: 'not an id',
      videoPath: '',
    });
    expect(items).toEqual([]);
  });

  it('is empty when the server has no media at all', () => {
    expect(
      galleryItems({ title: 'G', screenshotUrls: [], youtubeVideoId: '', videoPath: '' })
    ).toEqual([]);
  });
});

describe('youtubeEmbedUrl', () => {
  it('uses the no-cookie host', () => {
    expect(youtubeEmbedUrl('dQw4w9WgXcQ')).toBe(
      'https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ'
    );
  });
});

describe('isYoutubeId', () => {
  it('accepts an 11-character id', () => {
    expect(isYoutubeId('dQw4w9WgXcQ')).toBe(true);
  });

  it('rejects anything else, so no arbitrary string reaches the iframe src', () => {
    expect(isYoutubeId('')).toBe(false);
    expect(isYoutubeId('short')).toBe(false);
    expect(isYoutubeId('../../evil/path')).toBe(false);
    expect(isYoutubeId('dQw4w9WgXcQextra')).toBe(false);
  });
});

describe('viewer navigation', () => {
  it('advances', () => {
    expect(nextIndex(0, 3)).toBe(1);
  });

  it('wraps forward off the end', () => {
    expect(nextIndex(2, 3)).toBe(0);
  });

  it('wraps backward off the start', () => {
    expect(prevIndex(0, 3)).toBe(2);
  });

  it('stays put with a single item', () => {
    expect(nextIndex(0, 1)).toBe(0);
    expect(prevIndex(0, 1)).toBe(0);
  });

  it('never divides by zero on an empty gallery', () => {
    expect(nextIndex(0, 0)).toBe(0);
    expect(prevIndex(0, 0)).toBe(0);
  });
});

describe('overviewStrip', () => {
  it('caps at design §7 first six', () => {
    const urls = Array.from({ length: 9 }, (_, i) => `http://s/${i}.png`);
    expect(overviewStrip(urls)).toHaveLength(OVERVIEW_STRIP_LIMIT);
    expect(overviewStrip(urls)[5]).toBe('http://s/5.png');
  });

  it('passes a shorter list through', () => {
    expect(overviewStrip(['a', 'b'])).toEqual(['a', 'b']);
  });
});
```

- [ ] **Step 14: Run it to verify it fails**

Run from `rewrite/app`: `npx vitest run src/lib/details/media.test.ts`
Expected: FAIL — `Failed to resolve import "./media"`.

- [ ] **Step 15: Write `media.ts`**

Create `app/src/lib/details/media.ts`:

```ts
// The Media tab's gallery and the fullscreen viewer's navigation (design
// §7). Pure: `MediaTab.svelte` renders these items and `MediaViewer.svelte`
// walks them, but neither decides what is in the list or what comes next.

export type MediaItem =
  | { kind: 'screenshot'; url: string; caption: string }
  | { kind: 'youtube'; videoId: string; caption: string }
  | { kind: 'video'; url: string; caption: string };

export type MediaGalleryInput = {
  title: string;
  /** Already resolved + host-filtered absolute URLs (`RomDetail.screenshot_urls`). */
  screenshotUrls: string[];
  /** `RomDetail.youtube_video_id`. */
  youtubeVideoId: string;
  /** `RomDetail.video_path`, server-relative and NOT yet cached. */
  videoPath: string;
};

/**
 * An 11-character YouTube id and nothing else. The id is interpolated into
 * an iframe `src`, so anything that is not exactly an id — a path, a full
 * URL, an empty string — must not reach it.
 */
export function isYoutubeId(value: string): boolean {
  return /^[A-Za-z0-9_-]{11}$/.test(value.trim());
}

/** The privacy-preserving embed host; the only frame origin the CSP allows. */
export function youtubeEmbedUrl(videoId: string): string {
  return `https://www.youtube-nocookie.com/embed/${videoId}`;
}

/** Screenshots first (source order), then the trailer, then a hosted video. */
export function galleryItems(input: MediaGalleryInput): MediaItem[] {
  const items: MediaItem[] = input.screenshotUrls.map((url, i) => ({
    kind: 'screenshot' as const,
    url,
    caption: `${input.title} — screenshot ${i + 1}`,
  }));
  const videoId = input.youtubeVideoId.trim();
  if (isYoutubeId(videoId)) {
    items.push({ kind: 'youtube', videoId, caption: `${input.title} — trailer` });
  }
  const videoPath = input.videoPath.trim();
  if (videoPath !== '') {
    items.push({ kind: 'video', url: videoPath, caption: `${input.title} — video` });
  }
  return items;
}

/** The next item, wrapping. `0` for an empty gallery — never `NaN`. */
export function nextIndex(current: number, count: number): number {
  if (count <= 0) return 0;
  return (current + 1) % count;
}

/** The previous item, wrapping. `0` for an empty gallery. */
export function prevIndex(current: number, count: number): number {
  if (count <= 0) return 0;
  return (current - 1 + count) % count;
}

/** Design §7 Overview: "screenshot strip (first six of `merged_screenshots`)". */
export const OVERVIEW_STRIP_LIMIT = 6;

export function overviewStrip(urls: string[]): string[] {
  return urls.slice(0, OVERVIEW_STRIP_LIMIT);
}
```

- [ ] **Step 16: Run it to verify it passes**

Run from `rewrite/app`: `npx vitest run src/lib/details/media.test.ts`
Expected: PASS, 13 tests.

- [ ] **Step 17: Write the failing D-UI-10 tests**

Append to `app/src/lib/details/version.test.ts`:

```ts
describe('isoDate', () => {
  it('takes the date out of a server timestamp', () => {
    expect(isoDate('2026-02-03T11:22:33')).toBe('2026-02-03');
  });

  it('takes the date out of a Z-suffixed timestamp', () => {
    expect(isoDate('2026-02-03T11:22:33Z')).toBe('2026-02-03');
  });

  it('is blank when the server sends nothing', () => {
    expect(isoDate('')).toBe('');
  });

  it('is blank for a value that is not a date, rather than a truncated string', () => {
    expect(isoDate('last Tuesday')).toBe('');
  });
});

describe('fileVersionLabel (D-UI-10)', () => {
  it('names the parsed version tag when the file name carries one', () => {
    expect(fileVersionLabel('mygame (v1.1.0).zip', '2026-02-03T11:22:33')).toBe('v1.1.0');
  });

  it('names the numeric tag in its padded form', () => {
    expect(fileVersionLabel('Game (v00042).zip', '2026-02-03T11:22:33')).toBe('v00042');
  });

  it('falls back to the last_modified date when there is no tag', () => {
    expect(fileVersionLabel('Super Mario World.zip', '2026-02-03T11:22:33')).toBe('2026-02-03');
  });

  it('is blank when the file has neither a tag nor a timestamp', () => {
    expect(fileVersionLabel('Super Mario World.zip', '')).toBe('');
  });
});
```

Extend the file's existing import at line 2 to:

```ts
import {
  fileVersionLabel,
  formatVersionTag,
  isoDate,
  parseVersionTag,
  romFileNamesFor,
  versionLabel,
} from './version';
```

- [ ] **Step 18: Run it to verify it fails**

Run from `rewrite/app`: `npx vitest run src/lib/details/version.test.ts`
Expected: FAIL — `fileVersionLabel is not a function`.

- [ ] **Step 19: Extend `version.ts`**

Append to `app/src/lib/details/version.ts`:

```ts
/**
 * The `YYYY-MM-DD` head of an ISO 8601 timestamp, or `''` when the value is
 * not one. Sliced rather than parsed through `Date`: the server sends the
 * file's own stated timestamp and D-UI-10 shows the date it states, not
 * that instant re-rendered in the viewer's time zone.
 */
export function isoDate(value: string): string {
  const match = /^(\d{4}-\d{2}-\d{2})/.exec(value.trim());
  return match ? match[1] : '';
}

/**
 * D-UI-10: one file's version — "the parsed version tag when the file name
 * carries one, else the file's `last_modified` date". Unlike
 * [`versionLabel`], which is the header's whole-game row and is
 * platform-gated, this is per file and applies on every platform: the Files
 * tab states what each file IS, and a tagged file name is as informative on
 * a PS2 rom as on a PC one. `''` when the server offers neither.
 */
export function fileVersionLabel(fileName: string, lastModified: string): string {
  const tag = parseVersionTag(fileName);
  if (tag) return formatVersionTag(tag);
  return isoDate(lastModified);
}
```

- [ ] **Step 20: Run it to verify it passes**

Run from `rewrite/app`: `npx vitest run src/lib/details/version.test.ts`
Expected: PASS — the eight new tests plus every pre-existing one.

- [ ] **Step 21: Write the failing file-row tests**

Create `app/src/lib/details/files.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import { contentRows, fileRows, fileSizeText } from './files';
import type { RomFile } from '../api';

function file(overrides: Partial<RomFile> & { id: number; file_name: string }): RomFile {
  return {
    file_size_bytes: 0,
    is_top_level: true,
    category: '',
    last_modified: '',
    ...overrides,
  };
}

describe('fileSizeText', () => {
  it('reports plain bytes below a kibibyte', () => {
    expect(fileSizeText(512)).toBe('512 B');
  });

  it('reports one decimal from a kibibyte up', () => {
    expect(fileSizeText(1536)).toBe('1.5 KB');
    expect(fileSizeText(5 * 1024 * 1024)).toBe('5.0 MB');
    expect(fileSizeText(3 * 1024 * 1024 * 1024)).toBe('3.0 GB');
  });

  it('reports an unknown size as an em dash rather than "0 B"', () => {
    expect(fileSizeText(0)).toBe('—');
  });

  it('never reports a negative size', () => {
    expect(fileSizeText(-1)).toBe('—');
  });
});

describe('fileRows', () => {
  it('carries the name, size and D-UI-10 version of every file', () => {
    expect(
      fileRows([
        file({ id: 1, file_name: 'mygame (v1.1.0).zip', file_size_bytes: 2048 }),
        file({ id: 2, file_name: 'game.json', last_modified: '2026-02-03T11:22:33' }),
      ])
    ).toEqual([
      { id: 1, name: 'mygame (v1.1.0).zip', sizeText: '2.0 KB', version: 'v1.1.0', category: '' },
      { id: 2, name: 'game.json', sizeText: '—', version: '2026-02-03', category: '' },
    ]);
  });

  it('is empty for a rom with no listed files', () => {
    expect(fileRows([])).toEqual([]);
  });
});

describe('contentRows', () => {
  it('picks out the update and dlc category files', () => {
    const rows = contentRows([
      file({ id: 1, file_name: 'ps4-base.zip', category: 'game' }),
      file({ id: 2, file_name: 'ps4-update.zip', category: 'update' }),
      file({ id: 3, file_name: 'ps4-dlc.zip', category: 'dlc' }),
    ]);
    expect(rows.map((r) => r.name)).toEqual(['ps4-update.zip', 'ps4-dlc.zip']);
    expect(rows.map((r) => r.category)).toEqual(['update', 'dlc']);
  });

  it('folds the category case, which the server does not guarantee', () => {
    const rows = contentRows([file({ id: 1, file_name: 'u.zip', category: 'UPDATE' })]);
    expect(rows).toHaveLength(1);
  });

  it('is empty when every file is an ordinary game file', () => {
    expect(contentRows([file({ id: 1, file_name: 'g.zip', category: 'game' })])).toEqual([]);
  });
});
```

- [ ] **Step 22: Run it to verify it fails**

Run from `rewrite/app`: `npx vitest run src/lib/details/files.test.ts`
Expected: FAIL — `Failed to resolve import "./files"`.

- [ ] **Step 23: Write `files.ts`**

Create `app/src/lib/details/files.ts`:

```ts
// The Files tab's rows (design §7): `files[]` with name, size and the
// D-UI-10 version, plus the PS4 / Xbox 360 content rows. Pure.
import type { RomFile } from '../api';
import { fileVersionLabel } from './version';

const KIB = 1024;

/**
 * A file size for display. `0` is "the server did not state a size" — every
 * E2E fixture and plenty of real RomM rows send `0` for files it has not
 * measured — so it reads as an em dash rather than a confident "0 B".
 */
export function fileSizeText(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return '—';
  if (bytes < KIB) return `${bytes} B`;
  const units = ['KB', 'MB', 'GB', 'TB'];
  let value = bytes / KIB;
  let unit = 0;
  while (value >= KIB && unit < units.length - 1) {
    value /= KIB;
    unit += 1;
  }
  return `${value.toFixed(1)} ${units[unit]}`;
}

export type FileRow = {
  id: number;
  name: string;
  sizeText: string;
  /** D-UI-10: the parsed tag, else the `last_modified` date, else `''`. */
  version: string;
  /** The server's file category, lowercased; `''` for an ordinary file. */
  category: string;
};

export function fileRows(files: RomFile[]): FileRow[] {
  return files.map((f) => ({
    id: f.id,
    name: f.file_name,
    sizeText: fileSizeText(f.file_size_bytes),
    version: fileVersionLabel(f.file_name, f.last_modified),
    category: f.category.trim().toLowerCase(),
  }));
}

/**
 * The PS4 update / Xbox 360 content files the server lists for this rom.
 * These are the same `category` values `content_availability` reads on the
 * backend (`app/src-tauri/src/commands/specials.rs`), so the rows and the
 * left column's Install Update / Install DLC buttons agree by construction.
 */
export function contentRows(files: RomFile[]): FileRow[] {
  return fileRows(files).filter((row) => row.category === 'update' || row.category === 'dlc');
}
```

- [ ] **Step 24: Run it to verify it passes**

Run from `rewrite/app`: `npx vitest run src/lib/details/files.test.ts`
Expected: PASS, 9 tests.

- [ ] **Step 25: Full gate and commit**

Run from `rewrite/app`: `npm run check` and `npx vitest run`.
Expected: both green. No Rust changed, so the Rust gates are not required here — but `cargo fmt` is harmless and keeps the habit.

```bash
cd /home/six/Documents/Programming/grid-launcher
git add rewrite/app/src/lib/details/tabs.ts rewrite/app/src/lib/details/tabs.test.ts \
  rewrite/app/src/lib/details/header.ts rewrite/app/src/lib/details/header.test.ts \
  rewrite/app/src/lib/details/related.ts rewrite/app/src/lib/details/related.test.ts \
  rewrite/app/src/lib/details/media.ts rewrite/app/src/lib/details/media.test.ts \
  rewrite/app/src/lib/details/files.ts rewrite/app/src/lib/details/files.test.ts \
  rewrite/app/src/lib/details/version.ts rewrite/app/src/lib/details/version.test.ts
git commit -m "rewrite: pure helpers for the redesigned details popup"
```

---

### Task 3: The popup shell — left column, header, tab bar, and the four panels

The layout change itself. Every action button keeps its behaviour, its label and its test id; they move from the centre column into the fixed left column. The centre becomes a header plus four tab panels.

**Files:**
- Modify: `app/src/lib/details/header.ts` (append), `app/src/lib/details/header.test.ts` (append)
- Rewrite: `app/src/lib/Details.svelte` (all 844 lines)
- Modify: `e2e/specs/cloud-saves.spec.ts:98-108` (`openSavePanel` selects the Saves tab first — the id moved in this task, so its spec is updated in this task)
- Create: `app/src/lib/details/OverviewTab.svelte`
- Create: `app/src/lib/details/MediaTab.svelte`
- Create: `app/src/lib/details/SavesTab.svelte`
- Create: `app/src/lib/details/FilesTab.svelte`

**Interfaces:**
- Consumes: Task 1's `RomDetail` fields; Task 2's `tabs.ts`, `header.ts`, `media.ts`, `files.ts`, `version.ts`; the existing `details/subject.ts`, `details/cloud.ts`, `details/actions.ts`, `details/CloudPanel.svelte`, `details/NativeSettings.svelte`, `Image.svelte`; `stores/installed.svelte`, `stores/sessions.svelte`, `stores/downloads.svelte`, `stores/updates.svelte`, `stores/session.svelte`; `api.getLaunchDefaults`, `api.getRomDetail`.
- Produces, used by Tasks 4, 5 and 6:
  - `header.ts` gains `epochDate(seconds: number): string`, `lastPlayedText(lastPlayedAt: number): string`, `launchTargetLine(defaults: LaunchDefaults | null, platformName: string): string`, `cloudStatusLabel(saveSupported: boolean, stateSupported: boolean): string`.
  - `OverviewTab.svelte` props: `{ name: string; description: string; screenshotUrls: string[]; detail: RomDetail | null }`.
  - `MediaTab.svelte` props: `{ name: string; screenshotUrls: string[]; detail: RomDetail | null }`.
  - `SavesTab.svelte` props: `{ gameTitle: string; cloudGame: InstalledGame; isNative: boolean; savePanelInfo: CloudPanelInfo | null; statePanelInfo: CloudPanelInfo | null; cloudMode: CloudMode; infoError: string | null; onToggle: (saveType: 'save' | 'state') => void; onBack: () => void }`.
  - `FilesTab.svelte` props: `{ files: RomFile[]; installedVersion: string; serverVersion: string; installedNow: boolean }`.
  - New test ids: `details-tab-overview`, `details-tab-media`, `details-tab-saves`, `details-tab-files`, `details-header-line`, `details-verification`, `details-flags`, `details-emulator`, `details-last-played`, `details-cloud-status`, `details-meta-<key>`, `details-media-<i>`, `details-file-<id>`, `details-file-version-<id>`, `details-files-empty`, `details-files-version`.
  - Unchanged test ids, all still directly clickable: `details-panel`, `details-close`, `details-cover`, `details-playing-chip`, `details-rating`, `details-version`, `details-genres`, `details-description`, `details-screenshots`, `details-screenshot-<i>`, `details-no-screenshots`, `details-no-id`, `details-play`, `details-stop`, `details-install`, `details-uninstall`, `details-update`, `details-update-toast`, `details-cancel`, `details-install-update`, `details-install-dlc`, `details-game-settings`, `details-cloud-save-toggle`, `details-cloud-state-toggle`, `cloud-panel-info-error`, `details-error`, `details-warning`, `details-warning-dismiss`.

- [ ] **Step 1: Write the failing tests for the left column's four text rules**

Append to `app/src/lib/details/header.test.ts`:

```ts
describe('epochDate', () => {
  it('formats an epoch as a UTC date', () => {
    expect(epochDate(1_800_000_000)).toBe('2027-01-15');
  });

  it('is blank for never', () => {
    expect(epochDate(0)).toBe('');
  });
});

describe('lastPlayedText', () => {
  it('names the date of the last launch', () => {
    expect(lastPlayedText(1_800_000_000)).toBe('Last played 2027-01-15');
  });

  it('says so when the game has never been launched through GRID', () => {
    expect(lastPlayedText(0)).toBe('Never played');
  });
});

describe('launchTargetLine', () => {
  const defaults = (
    emulators: Record<string, string>,
    cores: Record<string, string> = {}
  ) => ({ default_emulators: emulators, retroarch_cores: cores, launch_args: '' });

  it('names the platform default emulator', () => {
    expect(launchTargetLine(defaults({ snes: 'Snes9x' }), 'SNES')).toBe('Snes9x');
  });

  it('names the core too when the default is a RetroArch build', () => {
    expect(
      launchTargetLine(defaults({ snes: 'RetroArch' }, { snes: 'snes9x_libretro' }), 'SNES')
    ).toBe('RetroArch · snes9x_libretro');
  });

  it('says a RetroArch default has no core rather than naming half a target', () => {
    expect(launchTargetLine(defaults({ snes: 'RetroArch' }), 'SNES')).toBe('RetroArch · no core');
  });

  it('reads a remembered "(none)" the same as an absent default', () => {
    expect(launchTargetLine(defaults({ snes: '<none>' }), 'SNES')).toBe('No default emulator');
  });

  it('says so when nothing is configured at all', () => {
    expect(launchTargetLine(null, 'SNES')).toBe('No default emulator');
  });
});

describe('cloudStatusLabel', () => {
  it('offers the panel when either kind is supported', () => {
    expect(cloudStatusLabel(true, false)).toBe('Cloud saves');
    expect(cloudStatusLabel(false, true)).toBe('Cloud saves');
  });

  it('says so when neither is', () => {
    expect(cloudStatusLabel(false, false)).toBe('Not configured');
  });
});
```

Extend the file's import to:

```ts
import {
  cloudStatusLabel,
  developerOf,
  epochDate,
  flagList,
  headerLine,
  lastPlayedText,
  launchTargetLine,
  ratingText,
  releaseYear,
  verificationLabel,
} from './header';
```

- [ ] **Step 2: Run it to verify it fails**

Run from `rewrite/app`: `npx vitest run src/lib/details/header.test.ts`
Expected: FAIL — `epochDate is not a function`.

- [ ] **Step 3: Extend `header.ts`**

Add this import at the top of `app/src/lib/details/header.ts`, under the file comment:

```ts
import type { LaunchDefaults } from '../api';
import { NO_EMULATOR_MARKER, isRetroarchName, savedDefaultFor } from '../emulators/defaults';
```

Append:

```ts
/**
 * An epoch-seconds stamp as `YYYY-MM-DD`, `''` for 0/never. UTC for the
 * same reason as [`releaseYear`]: this is a date the app states, and
 * re-rendering it per time zone would move it a day for some users.
 */
export function epochDate(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) return '';
  return new Date(seconds * 1000).toISOString().slice(0, 10);
}

/** The left column's play-time row, from the registry's `last_played_at`. */
export function lastPlayedText(lastPlayedAt: number): string {
  const date = epochDate(lastPlayedAt);
  return date === '' ? 'Never played' : `Last played ${date}`;
}

/**
 * Design §7's "the emulator + core that will launch". The emulator is the
 * platform's saved default (case-folded lookup, same as everywhere else);
 * the core is only meaningful for a RetroArch build, and a RetroArch
 * default with no core mapped is named as such rather than silently
 * reading like a complete target.
 */
export function launchTargetLine(defaults: LaunchDefaults | null, platformName: string): string {
  const name = savedDefaultFor(defaults?.default_emulators, platformName).trim();
  if (name === '' || name === NO_EMULATOR_MARKER) return 'No default emulator';
  if (!isRetroarchName(name)) return name;
  const cores = defaults?.retroarch_cores ?? {};
  const folded = platformName.trim().toLowerCase();
  const key = Object.keys(cores).find((k) => k.toLowerCase() === folded);
  const core = (key ? cores[key] : '').trim();
  return core === '' ? `${name} · no core` : `${name} · ${core}`;
}

/**
 * The left column's cloud button. It routes to the Saves tab, which shows a
 * real per-record relative time; it deliberately does not claim one itself,
 * because the popup does not fetch cloud records until that tab opens.
 */
export function cloudStatusLabel(saveSupported: boolean, stateSupported: boolean): string {
  return saveSupported || stateSupported ? 'Cloud saves' : 'Not configured';
}
```

- [ ] **Step 4: Run it to verify it passes**

Run from `rewrite/app`: `npx vitest run src/lib/details/header.test.ts`
Expected: PASS — the ten new tests plus the thirteen from Task 2.

- [ ] **Step 5: Create `OverviewTab.svelte`**

Create `app/src/lib/details/OverviewTab.svelte`:

```svelte
<script lang="ts">
  import type { RomDetail } from '../api';
  import Image from '../Image.svelte';
  import { overviewStrip } from './media';

  let {
    name,
    description,
    screenshotUrls,
    detail,
  }: {
    name: string;
    description: string;
    screenshotUrls: string[];
    detail: RomDetail | null;
  } = $props();

  let strip = $derived(overviewStrip(screenshotUrls));

  // `details-meta-<key>` rows, built from whatever the server actually
  // knows. A row with no value is dropped rather than rendered blank: an
  // empty grid cell reads as a failure, an absent row reads as "the server
  // has nothing", which is the truth.
  let metaRows = $derived(
    (
      [
        ['developer', 'Developer', detail?.companies.split(',')[0]?.trim() ?? ''],
        ['companies', 'Companies', detail?.companies ?? ''],
        ['release', 'Release', detail?.first_release_date ?? ''],
        ['genres', 'Genres', detail?.genres ?? ''],
        ['modes', 'Game modes', detail?.game_modes ?? ''],
        ['players', 'Players', detail?.player_count ?? ''],
        ['franchises', 'Franchises', detail?.franchises ?? ''],
      ] as const
    ).filter(([, , value]) => value.trim() !== '')
  );

  let failedScreenshots = $state<Record<string, true>>({});
  function markScreenshotFailed(url: string) {
    failedScreenshots = { ...failedScreenshots, [url]: true };
  }
</script>

<div class="overview">
  <p data-testid="details-description" class="description">{description}</p>

  {#if metaRows.length}
    <dl class="meta">
      {#each metaRows as [key, label, value] (key)}
        <dt>{label}</dt>
        <dd data-testid={`details-meta-${key}`}>{value}</dd>
      {/each}
    </dl>
  {/if}

  {#if strip.length}
    <div class="shots" data-testid="details-screenshots">
      {#each strip as url, i (url)}
        {#if !failedScreenshots[url]}
          <Image
            {url}
            alt={`${name} screenshot ${i + 1}`}
            data-testid={`details-screenshot-${i}`}
            onerror={() => markScreenshotFailed(url)}
          />
        {/if}
      {/each}
    </div>
  {:else}
    <p class="empty" data-testid="details-no-screenshots">No screenshots available</p>
  {/if}
</div>

<style>
  .overview {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .description {
    margin: 0;
    color: var(--text);
    font-size: 14px;
    line-height: 1.5;
  }

  .meta {
    display: grid;
    grid-template-columns: 140px 1fr;
    gap: 6px 12px;
    margin: 0;
  }

  .meta dt {
    color: var(--text-muted);
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .meta dd {
    margin: 0;
    color: var(--text);
    font-size: 13px;
  }

  .shots {
    display: flex;
    gap: 8px;
    overflow-x: auto;
    padding-bottom: 4px;
  }

  .shots :global(img) {
    height: 110px;
    width: auto;
    flex: none;
    border-radius: var(--r-chip);
    object-fit: cover;
  }

  .empty {
    margin: 0;
    color: var(--text-muted);
    font-size: 13px;
  }
</style>
```

- [ ] **Step 6: Create `MediaTab.svelte`**

Create `app/src/lib/details/MediaTab.svelte`. The tiles are the gallery; Task 4 gives them the fullscreen viewer.

```svelte
<script lang="ts">
  import type { RomDetail } from '../api';
  import Image from '../Image.svelte';
  import { galleryItems } from './media';

  let {
    name,
    screenshotUrls,
    detail,
  }: {
    name: string;
    screenshotUrls: string[];
    detail: RomDetail | null;
  } = $props();

  let items = $derived(
    galleryItems({
      title: name,
      screenshotUrls,
      youtubeVideoId: detail?.youtube_video_id ?? '',
      videoPath: detail?.video_path ?? '',
    })
  );
</script>

{#if items.length}
  <div class="gallery">
    {#each items as item, i (item.caption)}
      <div class="tile" data-testid={`details-media-${i}`} title={item.caption}>
        {#if item.kind === 'screenshot'}
          <Image url={item.url} alt={item.caption} placeholder="Screenshot" />
        {:else}
          <div class="video-tile">▶ {item.kind === 'youtube' ? 'Trailer' : 'Video'}</div>
        {/if}
      </div>
    {/each}
  </div>
{:else}
  <p class="empty" data-testid="details-no-media">No media available</p>
{/if}

<style>
  .gallery {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: 10px;
  }

  .tile {
    aspect-ratio: 16 / 9;
    border-radius: var(--r-row);
    overflow: hidden;
    background: var(--surface);
    border: 1px solid var(--border);
  }

  .tile :global(img) {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .video-tile {
    display: grid;
    place-items: center;
    height: 100%;
    color: var(--text);
    font-size: 14px;
  }

  .empty {
    margin: 0;
    color: var(--text-muted);
    font-size: 13px;
  }
</style>
```

- [ ] **Step 7: Create `SavesTab.svelte`**

Create `app/src/lib/details/SavesTab.svelte`. Every element here is moved verbatim out of today's `Details.svelte:454-490` — same ids, same labels, same `CloudPanel` props.

```svelte
<script lang="ts">
  import type { CloudPanelInfo, InstalledGame } from '../api';
  import CloudPanel from './CloudPanel.svelte';
  import { cloudButtonLabel, type CloudMode } from './cloud';

  let {
    gameTitle,
    cloudGame,
    isNative,
    savePanelInfo,
    statePanelInfo,
    cloudMode,
    infoError,
    onToggle,
    onBack,
  }: {
    gameTitle: string;
    cloudGame: InstalledGame;
    isNative: boolean;
    savePanelInfo: CloudPanelInfo | null;
    statePanelInfo: CloudPanelInfo | null;
    cloudMode: CloudMode;
    infoError: string | null;
    onToggle: (saveType: 'save' | 'state') => void;
    onBack: () => void;
  } = $props();

  let activePanelInfo = $derived(
    cloudMode === 'save' ? savePanelInfo : cloudMode === 'state' ? statePanelInfo : null
  );
  let anySupported = $derived(savePanelInfo?.supported === true || statePanelInfo?.supported === true);
</script>

<div class="saves">
  {#if anySupported}
    <div class="cloud-toggle">
      {#if savePanelInfo?.supported}
        <button
          data-testid="details-cloud-save-toggle"
          class:active={cloudMode === 'save'}
          onclick={() => onToggle('save')}
        >
          {cloudButtonLabel('save', savePanelInfo.scope)}
        </button>
      {/if}
      {#if statePanelInfo?.supported}
        <button
          data-testid="details-cloud-state-toggle"
          class:active={cloudMode === 'state'}
          onclick={() => onToggle('state')}
        >
          {cloudButtonLabel('state', statePanelInfo.scope)}
        </button>
      {/if}
    </div>
  {:else}
    <p class="empty" data-testid="details-cloud-unsupported">
      {savePanelInfo?.block_reason || 'Cloud saves are not configured for this game.'}
    </p>
  {/if}

  {#if infoError}
    <p data-testid="cloud-panel-info-error" class="error" role="alert">{infoError}</p>
  {/if}

  {#if cloudMode !== 'overview' && activePanelInfo}
    <CloudPanel
      game={cloudGame}
      {gameTitle}
      saveType={cloudMode}
      panelInfo={activePanelInfo}
      {isNative}
      {onBack}
    />
  {/if}
</div>

<style>
  .saves {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .cloud-toggle {
    display: flex;
    gap: 8px;
  }

  .cloud-toggle button {
    flex: 1;
    font: inherit;
    padding: 8px 12px;
    border-radius: var(--r-control);
    background: transparent;
    color: var(--text);
    border: 1px solid var(--border);
    cursor: pointer;
    transition: background var(--m-fast) ease;
  }

  .cloud-toggle button.active {
    background: var(--primary);
    color: #fff;
    border-color: var(--primary);
  }

  .empty {
    margin: 0;
    color: var(--text-muted);
    font-size: 13px;
  }

  .error {
    margin: 0;
    color: var(--danger);
    font-size: 13px;
  }
</style>
```

- [ ] **Step 8: Create `FilesTab.svelte`**

Create `app/src/lib/details/FilesTab.svelte`. Task 5 adds the firmware row to this file.

```svelte
<script lang="ts">
  import type { RomFile } from '../api';
  import { contentRows, fileRows } from './files';

  let {
    files,
    installedVersion,
    serverVersion,
    installedNow,
  }: {
    files: RomFile[];
    installedVersion: string;
    serverVersion: string;
    installedNow: boolean;
  } = $props();

  let rows = $derived(fileRows(files));
  let content = $derived(contentRows(files));

  // D-UI-10's comparison line. Only meaningful once the game is installed:
  // for a server-only game there is no installed side to compare against,
  // and the left column's Install button is the whole story.
  let versionLine = $derived(
    installedNow && (installedVersion !== '' || serverVersion !== '')
      ? `Installed ${installedVersion || 'unknown'} · Server ${serverVersion || 'unknown'}`
      : ''
  );
</script>

<div class="files">
  {#if versionLine}
    <p class="version-line" data-testid="details-files-version">{versionLine}</p>
  {/if}

  {#if rows.length}
    <ul class="rows">
      {#each rows as row (row.id)}
        <li class="row" data-testid={`details-file-${row.id}`}>
          <span class="name">{row.name}</span>
          <span class="size">{row.sizeText}</span>
          <span class="version" data-testid={`details-file-version-${row.id}`}>{row.version}</span>
        </li>
      {/each}
    </ul>
  {:else}
    <p class="empty" data-testid="details-files-empty">The server lists no files for this game</p>
  {/if}

  {#if content.length}
    <h3>Extra content</h3>
    <ul class="rows">
      {#each content as row (row.id)}
        <li class="row" data-testid={`details-content-${row.id}`}>
          <span class="name">{row.name}</span>
          <span class="size">{row.sizeText}</span>
          <span class="version">{row.category === 'update' ? 'Update' : 'DLC'}</span>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .files {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .version-line {
    margin: 0;
    color: var(--text-muted);
    font-size: 13px;
  }

  h3 {
    margin: 4px 0 0;
    font-size: 13px;
    font-weight: 600;
    color: var(--text-h);
  }

  .rows {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .row {
    display: grid;
    grid-template-columns: 1fr auto auto;
    gap: 12px;
    align-items: baseline;
    padding: 8px 10px;
    border-radius: var(--r-row);
    background: var(--surface);
  }

  .name {
    color: var(--text);
    font-size: 13px;
    overflow-wrap: anywhere;
  }

  .size,
  .version {
    color: var(--text-muted);
    font-size: 12px;
    white-space: nowrap;
  }

  .empty {
    margin: 0;
    color: var(--text-muted);
    font-size: 13px;
  }
</style>
```

- [ ] **Step 9: Rewrite `Details.svelte`**

Replace the whole of `app/src/lib/Details.svelte` with:

```svelte
<script lang="ts">
  import {
    api,
    type CloudPanelInfo,
    type ContentAvailability,
    type ContentKind,
    type DownloadStatus,
    type LaunchDefaults,
    type RomDetail,
  } from './api';
  import { downloads } from './stores/downloads.svelte';
  import { updates } from './stores/updates.svelte';
  import { isInstalled, installed, matchesInstalled, refresh as refreshInstalled } from './stores/installed.svelte';
  import { session } from './stores/session.svelte';
  import { sessions } from './stores/sessions.svelte';
  import Image from './Image.svelte';
  import NativeSettings from './details/NativeSettings.svelte';
  import OverviewTab from './details/OverviewTab.svelte';
  import MediaTab from './details/MediaTab.svelte';
  import SavesTab from './details/SavesTab.svelte';
  import FilesTab from './details/FilesTab.svelte';
  import { mergeDetail, summaryOf, type DetailsSubject } from './details/subject';
  import { isNativeExecutablePlatform, syntheticCloudGame, toggleCloudMode, type CloudMode } from './details/cloud';
  import { contentButtons, installLabel, isContentPlatform, isNativePlatform } from './details/actions';
  import { fileVersionLabel, romFileNamesFor, versionLabel } from './details/version';
  import {
    cloudStatusLabel,
    epochDate,
    flagList,
    headerLine,
    lastPlayedText,
    launchTargetLine,
    verificationLabel,
  } from './details/header';
  import {
    DETAILS_TABS,
    DETAILS_TAB_LABELS,
    rememberTab,
    rememberedTab,
    tabTestId,
    type DetailsTab,
  } from './details/tabs';

  let {
    subject,
    onClose,
    onLibraryPathUnset,
    initialCloudMode = 'overview',
  }: {
    subject: DetailsSubject;
    onClose: () => void;
    onLibraryPathUnset: () => void;
    /** Which cloud panel the popup opens with. A card's "Cloud sync" action
     *  passes `'save'`, which also selects the Saves tab. */
    initialCloudMode?: CloudMode;
  } = $props();

  const LIVE_INSTALL_STATUSES: DownloadStatus[] = ['queued', 'downloading', 'installing', 'cancelling'];

  type PendingAction = 'install' | 'uninstall' | 'play' | 'stop' | null;

  let confirmingUninstall = $state(false);
  let pendingAction = $state<PendingAction>(null);
  let error = $state<string | null>(null);
  let panelEl = $state<HTMLElement | null>(null);

  // Install specials (task-16-brief.md): Cancel for a live install,
  // Install Update/DLC for installed PS4/Xbox 360 games, and the native
  // Game Settings dialog.
  let cancelPending = $state(false);
  let contentAvailability = $state<ContentAvailability | null>(null);
  let wasLive = $state(false);
  let contentActionKind = $state<ContentKind | null>(null);
  let showNativeSettings = $state(false);

  // Design §7: last tab remembered per session, not per game.
  let tab = $state<DetailsTab>(initialCloudMode === 'overview' ? rememberedTab() : 'saves');
  function selectTab(next: DetailsTab) {
    tab = next;
    rememberTab(next);
  }

  // Metadata overlay (task-10-brief.md): the subject carries whatever the
  // grid it opened from already had on hand; the full `RomDetail` fills in
  // the gaps. The redesigned tabs read `files`, `related`, the IGDB block
  // and the media fields off it, none of which the registry stores, so the
  // fetch now runs for every rom with a server id rather than only for a
  // subject with no screenshots. It stays a display overlay: `subject` is
  // the caller's data and is never mutated.
  let detail = $state<RomDetail | null>(null);
  let launchDefaults = $state<LaunchDefaults | null>(null);

  let merged = $derived(detail === null ? subject : mergeDetail(subject, detail));
  let coverSmall = $derived(merged.coverSmall);
  let coverLarge = $derived(merged.coverLarge);
  let screenshotUrls = $derived(merged.screenshotUrls);
  let description = $derived(merged.description);
  let rating = $derived(merged.rating);
  let genres = $derived(merged.genres);

  $effect(() => {
    if (subject.romId === null) return; // no server id: nothing to overlay
    if (!session.connected) return;
    api
      .getRomDetail(subject.romId)
      .then((fetched) => {
        detail = fetched;
      })
      .catch(() => {}); // offline/removed rom: the subject's own data stands
  });

  $effect(() => {
    api
      .getLaunchDefaults()
      .then((d) => (launchDefaults = d))
      .catch(() => {}); // unreadable config: the emulator row says "No default emulator"
  });

  let pending = $derived(pendingAction !== null);
  let summary = $derived(summaryOf(subject));
  let liveEntry = $derived(
    subject.romId !== null
      ? downloads.entries.find((e) => e.rom_id === subject.romId && LIVE_INSTALL_STATUSES.includes(e.status))
      : undefined
  );
  let installedNow = $derived(isInstalled(summary, subject.platformName));
  let liveSession = $derived(subject.romId !== null ? sessions.sessionFor(subject.romId) : undefined);

  let isContent = $derived(isContentPlatform(subject.platformName));
  let isNativeInstall = $derived(isNativePlatform(subject.platformName));
  let buttons = $derived(contentButtons(contentAvailability, installedNow, liveEntry !== undefined));

  // Fetched once the subject is installed-and-a-content-platform, and
  // re-fetched right after a live install for it finishes (`wasLive` tracks
  // the previous liveEntry-defined-ness across effect runs).
  $effect(() => {
    if (subject.romId === null || !installedNow || !isContent) {
      contentAvailability = null;
      wasLive = liveEntry !== undefined;
      return;
    }
    const live = liveEntry !== undefined;
    const justFinished = wasLive && !live;
    wasLive = live;
    if (live || (contentAvailability !== null && !justFinished)) return;
    api
      .contentAvailability(subject.romId)
      .then((avail) => (contentAvailability = avail))
      .catch(() => (contentAvailability = null));
  });

  // Cloud saves/states (task-19-brief.md). `cloudGame` is the InstalledGame
  // registry row when one exists, else a synthetic stand-in built from the
  // subject.
  let installedRow = $derived(installed.list.find((row) => matchesInstalled(row, summary, subject.platformName)) ?? null);
  let cloudGame = $derived(installedRow ?? syntheticCloudGame(summary, subject.platformName));
  let isNative = $derived(isNativeExecutablePlatform(subject.platformName));

  // Server-side game updates (doc 10). `updateLabel` is null when the rom
  // has no update, which also hides the button. `version` is the header row:
  // the version tag parsed out of the file name for Windows/PC, else the raw
  // revision.
  let updateLabel = $derived(installedNow ? updates.labelFor(subject.romId) : null);
  let version = $derived(
    versionLabel(
      subject.platformName,
      romFileNamesFor(subject.source, installedRow?.rom_file_name ?? '', detail?.fs_name ?? ''),
      detail?.revision || installedRow?.revision || ''
    )
  );

  // D-UI-10, per side. The server side reads the top-level file's own
  // timestamp; the installed side has no server timestamp to fall back on,
  // so it falls back to when the install landed.
  let topLevelFile = $derived(detail?.files.find((f) => f.is_top_level) ?? detail?.files[0] ?? null);
  let serverVersion = $derived(
    detail ? fileVersionLabel(detail.fs_name, topLevelFile?.last_modified ?? '') : ''
  );
  let installedVersion = $derived(
    installedRow
      ? fileVersionLabel(installedRow.rom_file_name, '') || epochDate(installedRow.installed_at)
      : ''
  );

  let confirmingUpdate = $state(false);
  let updateToast = $state<string | null>(null);
  let updatePending = $state(false);

  // Last seen status per update entry for this rom. Deliberately NOT `$state`.
  const seenUpdateStatus = new Map<number, DownloadStatus>();

  // Toast on completion, not on click.
  $effect(() => {
    for (const entry of downloads.entries) {
      if (entry.rom_id !== subject.romId) continue;
      if (entry.kind !== 'update' && entry.kind !== 'native_update') continue;
      const previous = seenUpdateStatus.get(entry.id);
      seenUpdateStatus.set(entry.id, entry.status);
      if (previous !== undefined && previous !== 'completed' && entry.status === 'completed') {
        updateToast = `Updated '${subject.name}' successfully.`;
      }
    }
  });

  let cloudMode = $state<CloudMode>(initialCloudMode);
  let savePanelInfo = $state<CloudPanelInfo | null>(null);
  let statePanelInfo = $state<CloudPanelInfo | null>(null);
  let cloudPanelInfoError = $state<string | null>(null);

  $effect(() => {
    panelEl?.focus();
  });

  $effect(() => {
    if (subject.romId === null) return; // no server id: nothing to manage cloud saves for
    api
      .cloudPanelInfo(cloudGame, 'save')
      .then((info) => (savePanelInfo = info))
      .catch((err) => (cloudPanelInfoError = errorMessage(err)));
    api
      .cloudPanelInfo(cloudGame, 'state')
      .then((info) => (statePanelInfo = info))
      .catch((err) => (cloudPanelInfoError = errorMessage(err)));
  });

  function handleCloudToggle(saveType: 'save' | 'state') {
    cloudMode = toggleCloudMode(cloudMode, saveType);
  }

  /** The left column's cloud button: go to the Saves tab and open a panel. */
  function openCloud() {
    selectTab('saves');
    if (cloudMode === 'overview') {
      cloudMode = savePanelInfo?.supported ? 'save' : statePanelInfo?.supported ? 'state' : 'overview';
    }
  }

  function errorMessage(err: unknown): string {
    return err instanceof Error ? err.message : String(err);
  }

  async function handleInstall() {
    if (subject.romId === null) return;
    error = null;
    pendingAction = 'install';
    try {
      await api.installGame(subject.romId);
    } catch (err) {
      const message = errorMessage(err);
      error = message;
      if (message.includes('library folder')) onLibraryPathUnset();
    } finally {
      pendingAction = null;
    }
  }

  async function handleUninstallClick() {
    if (subject.romId === null) return;
    if (!confirmingUninstall) {
      confirmingUninstall = true;
      return;
    }
    error = null;
    pendingAction = 'uninstall';
    try {
      await api.uninstallGame(subject.romId);
      await refreshInstalled();
      onClose();
    } catch (err) {
      error = errorMessage(err);
      confirmingUninstall = false;
    } finally {
      pendingAction = null;
    }
  }

  // Two-click confirm for native installs only (doc 10).
  async function handleUpdateClick() {
    if (subject.romId === null) return;
    if (isNativeInstall && !confirmingUpdate) {
      confirmingUpdate = true;
      return;
    }
    error = null;
    updatePending = true;
    try {
      await api.updateGame(subject.romId);
    } catch (err) {
      error = errorMessage(err);
    } finally {
      updatePending = false;
      confirmingUpdate = false;
    }
  }

  async function handleCancel() {
    if (subject.romId === null) return;
    error = null;
    cancelPending = true;
    try {
      await api.cancelDownloadForRom(subject.romId);
    } catch (err) {
      error = errorMessage(err);
    } finally {
      cancelPending = false;
    }
  }

  async function handleInstallContent(kind: ContentKind) {
    if (subject.romId === null) return;
    error = null;
    contentActionKind = kind;
    try {
      await api.installContent(subject.romId, kind);
    } catch (err) {
      error = errorMessage(err);
    } finally {
      contentActionKind = null;
    }
  }

  async function handlePlay() {
    if (subject.romId === null) return;
    error = null;
    pendingAction = 'play';
    try {
      await api.launchGame(subject.romId);
    } catch (err) {
      error = errorMessage(err);
    } finally {
      pendingAction = null;
    }
  }

  async function handleStop() {
    if (!liveSession) return;
    error = null;
    pendingAction = 'stop';
    try {
      await api.stopGame(liveSession.id);
    } catch (err) {
      error = errorMessage(err);
    } finally {
      pendingAction = null;
    }
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      onClose();
    }
  }

  function onBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) onClose();
  }

  let header = $derived(
    headerLine({
      platformName: subject.platformName,
      firstReleaseDate: detail?.first_release_date ?? '',
      companies: detail?.companies ?? '',
      genres,
      rating,
    })
  );
  let flags = $derived([
    ...flagList(detail?.regions ?? installedRow?.regions ?? ''),
    ...flagList(detail?.languages ?? installedRow?.languages ?? ''),
  ]);
</script>

<div class="backdrop" onclick={onBackdropClick} role="presentation">
  <div
    data-testid="details-panel"
    class="panel"
    bind:this={panelEl}
    role="dialog"
    aria-modal="true"
    aria-label={subject.name}
    tabindex="-1"
    onkeydown={onKey}
  >
    <button data-testid="details-close" class="close" onclick={onClose} aria-label="Close">×</button>

    <div class="layout">
      <aside class="left">
        <div class="cover">
          <Image url={coverLarge ?? coverSmall} alt={subject.name} placeholder="No cover" data-testid="details-cover" />
        </div>

        {#if subject.romId === null}
          <p data-testid="details-no-id">This entry has no server id</p>
        {:else}
          <div class="action">
            {#if liveEntry}
              <button disabled>Installing…</button>
              <!-- `cancel_for_rom` leaves a finalizing entry alone —
                   extraction is not cancellable — so the button is disabled
                   rather than silently doing nothing while it runs. -->
              <button
                data-testid="details-cancel"
                class="secondary"
                disabled={cancelPending || liveEntry.status === 'installing'}
                title={liveEntry.status === 'installing' ? 'Installing — this step cannot be cancelled' : undefined}
                onclick={handleCancel}
              >
                {cancelPending ? 'Cancelling…' : 'Cancel'}
              </button>
            {:else if liveSession}
              <button data-testid="details-stop" disabled={pending} onclick={handleStop}>
                {pendingAction === 'stop' ? 'Stopping…' : 'Stop'}
              </button>
            {:else if installedNow}
              <button data-testid="details-play" disabled={pending} onclick={handlePlay}>
                {pendingAction === 'play' ? 'Launching…' : 'Play'}
              </button>
              {#if updateLabel !== null}
                <button
                  data-testid="details-update"
                  class="update"
                  class:confirm={confirmingUpdate}
                  disabled={pending || updatePending || liveEntry !== undefined}
                  onclick={handleUpdateClick}
                >
                  {updatePending
                    ? 'Updating…'
                    : confirmingUpdate
                      ? 'Saves and configuration will be preserved — confirm update'
                      : updateLabel}
                </button>
              {/if}
              {#if buttons.update}
                <button
                  data-testid="details-install-update"
                  class="secondary"
                  disabled={contentActionKind !== null}
                  onclick={() => handleInstallContent('update')}
                >
                  {contentActionKind === 'update' ? 'Installing…' : 'Install Update'}
                </button>
              {/if}
              {#if buttons.dlc}
                <button
                  data-testid="details-install-dlc"
                  class="secondary"
                  disabled={contentActionKind !== null}
                  onclick={() => handleInstallContent('dlc')}
                >
                  {contentActionKind === 'dlc' ? 'Installing…' : 'Install DLC'}
                </button>
              {/if}
              {#if isNativeInstall}
                <button data-testid="details-game-settings" class="secondary" onclick={() => (showNativeSettings = true)}>
                  Game Settings
                </button>
              {/if}
              <button
                data-testid="details-uninstall"
                class="secondary"
                class:confirm={confirmingUninstall}
                disabled={pending}
                onclick={handleUninstallClick}
              >
                {confirmingUninstall ? 'Confirm uninstall' : 'Uninstall'}
              </button>
            {:else}
              <button data-testid="details-install" disabled={pending} onclick={handleInstall}>
                {pendingAction === 'install' ? 'Installing…' : installLabel(subject.platformName)}
              </button>
            {/if}
            <button data-testid="details-cloud-status" class="secondary" onclick={openCloud}>
              {cloudStatusLabel(savePanelInfo?.supported === true, statePanelInfo?.supported === true)}
            </button>
          </div>

          {#if updateToast}
            <p data-testid="details-update-toast" class="hint" role="status">{updateToast}</p>
          {/if}
        {/if}

        <p class="meta-line" data-testid="details-last-played">
          {lastPlayedText(installedRow?.last_played_at ?? 0)}
        </p>
        <p class="meta-line" data-testid="details-emulator">
          {launchTargetLine(launchDefaults, subject.platformName)}
        </p>
      </aside>

      <section class="right">
        <header class="head">
          <h2>{subject.name}</h2>
          <p class="header-line" data-testid="details-header-line">{header}</p>
          <div class="chips">
            {#if liveSession}
              <span data-testid="details-playing-chip" class="chip playing">Playing</span>
            {/if}
            <span class="chip" data-testid="details-verification">
              {verificationLabel(detail?.is_identified ?? false)}
            </span>
            {#if flags.length}
              <span class="chip" data-testid="details-flags">{flags.join(' · ')}</span>
            {/if}
            {#if rating}
              <span class="chip" data-testid="details-rating">{rating}</span>
            {/if}
            {#if version}
              <span class="chip" data-testid="details-version">{version}</span>
            {/if}
          </div>
          <p class="genres" data-testid="details-genres">{genres}</p>
        </header>

        <div class="tabs" role="tablist">
          {#each DETAILS_TABS as name (name)}
            <button
              role="tab"
              data-testid={tabTestId(name)}
              class:active={tab === name}
              aria-selected={tab === name}
              onclick={() => selectTab(name)}
            >
              {DETAILS_TAB_LABELS[name]}
            </button>
          {/each}
        </div>

        <div class="tabpanel" role="tabpanel">
          {#if tab === 'overview'}
            <OverviewTab name={subject.name} {description} {screenshotUrls} {detail} />
          {:else if tab === 'media'}
            <MediaTab name={subject.name} {screenshotUrls} {detail} />
          {:else if tab === 'saves'}
            <SavesTab
              gameTitle={subject.name}
              {cloudGame}
              {isNative}
              {savePanelInfo}
              {statePanelInfo}
              {cloudMode}
              infoError={cloudPanelInfoError}
              onToggle={handleCloudToggle}
              onBack={() => (cloudMode = 'overview')}
            />
          {:else}
            <FilesTab
              files={detail?.files ?? []}
              {installedVersion}
              {serverVersion}
              {installedNow}
            />
          {/if}
        </div>

        {#if error}
          <p data-testid="details-error" class="error" role="alert">{error}</p>
        {/if}
        {#if sessions.lastWarning}
          <p data-testid="details-warning" class="error warning" role="alert">
            {sessions.lastWarning}
            <button data-testid="details-warning-dismiss" class="dismiss" onclick={() => sessions.dismissWarning()} aria-label="Dismiss warning">×</button>
          </p>
        {/if}
      </section>
    </div>
  </div>
</div>

{#if showNativeSettings && subject.romId !== null}
  <NativeSettings
    romId={subject.romId}
    title={subject.name}
    onClose={() => (showNativeSettings = false)}
    onSaved={refreshInstalled}
  />
{/if}

<style>
  /* Design §7: dimmed AND blurred shell behind the dialog. */
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    backdrop-filter: blur(6px);
    display: grid;
    place-items: center;
    z-index: 20;
  }

  .panel {
    position: relative;
    width: min(1040px, calc(100vw - 48px));
    height: min(680px, calc(100vh - 48px));
    box-sizing: border-box;
    padding: 24px;
    border-radius: 12px;
    background: var(--bg);
    border: 1px solid var(--border);
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.35);
  }

  .panel:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: 2px;
  }

  /* The left column is fixed at design §7's 240px; only the right column's
     tab panel scrolls, so the cover and the actions never leave the view. */
  .layout {
    display: grid;
    grid-template-columns: 240px 1fr;
    gap: 24px;
    height: 100%;
    min-height: 0;
  }

  .left {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-height: 0;
    overflow-y: auto;
  }

  .right {
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-height: 0;
  }

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

  .close:hover,
  .close:focus-visible {
    background: var(--border);
  }

  .cover {
    width: 100%;
    aspect-ratio: 3 / 4;
    border-radius: var(--r-row);
    overflow: hidden;
    flex: none;
  }

  .cover :global(img) {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  h2 {
    margin: 0;
    color: var(--text-h);
    font-size: 20px;
  }

  .header-line,
  .genres {
    margin: 0;
    color: var(--text-muted);
    font-size: 13px;
  }

  .head {
    display: flex;
    flex-direction: column;
    gap: 6px;
    flex: none;
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .chip {
    font-size: 11px;
    font-weight: 600;
    padding: 2px 10px;
    border-radius: var(--r-pill);
    background: var(--surface);
    color: var(--text);
    border: 1px solid var(--border);
  }

  .chip.playing {
    background: var(--primary);
    border-color: var(--primary);
    color: #fff;
    text-transform: uppercase;
    letter-spacing: 0.03em;
  }

  .tabs {
    display: flex;
    gap: 4px;
    border-bottom: 1px solid var(--border);
    flex: none;
  }

  .tabs button {
    font: inherit;
    padding: 8px 14px;
    border: none;
    border-bottom: 2px solid transparent;
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
    transition:
      color var(--m-fast) ease,
      border-color var(--m-fast) ease;
  }

  .tabs button.active {
    color: var(--text-h);
    border-bottom-color: var(--primary);
  }

  .tabpanel {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding-right: 4px;
  }

  .action {
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .action button {
    width: 100%;
    font: inherit;
    padding: 10px 16px;
    border-radius: var(--r-control);
    border: none;
    background: var(--primary);
    color: #fff;
    cursor: pointer;
    transition: background var(--m-fast) ease;
  }

  .action button:hover:not(:disabled) {
    background: var(--primary-hover);
  }

  .action button.confirm {
    background: var(--danger);
  }

  .action button.secondary {
    padding: 8px 16px;
    background: transparent;
    color: var(--text);
    border: 1px solid var(--border);
  }

  .action button.secondary:hover:not(:disabled) {
    background: var(--surface);
  }

  .action button.secondary.confirm {
    background: transparent;
    color: var(--danger);
    border-color: var(--danger);
  }

  /* The update confirm is a caution, not a destruction: it keeps the
     two-click shape but takes the warning amber instead of `.confirm`'s
     red, which would contradict the label's "will be preserved". */
  .action button.update.confirm {
    background: var(--warning);
    color: #16171d;
  }

  .action button:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .meta-line {
    margin: 0;
    color: var(--text-muted);
    font-size: 12px;
  }

  .hint {
    margin: 0;
    color: var(--text);
    opacity: 0.75;
    font-size: 13px;
  }

  .error {
    margin: 0;
    color: var(--danger);
    font-size: 13px;
    flex: none;
  }

  .error.warning {
    display: flex;
    align-items: center;
    gap: 6px;
  }

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

  .dismiss:hover,
  .dismiss:focus-visible {
    background: var(--border);
  }
</style>
```

- [ ] **Step 10: Run the frontend gates**

Run from `rewrite/app`: `npm run check` then `npx vitest run`
Expected: both green. If `svelte-check` reports an unused import in `Details.svelte` (`cloudButtonLabel` moved to `SavesTab.svelte`, `CloudPanel` with it), delete that import line — the list in Step 9's script block is already the correct final set.

- [ ] **Step 11: Point the cloud-saves E2E helper at the Saves tab**

The two cloud toggles moved out of the popup's centre column into the Saves
tab, so the one helper that clicks them has to select the tab first. In
`e2e/specs/cloud-saves.spec.ts`, replace the body of `openSavePanel`
(lines 98-108) with:

```ts
  async function openSavePanel() {
    // The cloud toggles live on the Saves tab (design §7). The tab itself
    // is always mounted; the toggle only exists once the tab is showing.
    await $(testId('details-tab-saves')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the details popup never rendered its Saves tab',
    });
    await $(testId('details-tab-saves')).click();
    await $(testId('details-cloud-save-toggle')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the "Manage Saves" toggle never appeared',
    });
    await $(testId('details-cloud-save-toggle')).click();
    await $(testId('cloud-panel')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the cloud save panel never opened',
    });
  }
```

- [ ] **Step 12: Run the three E2E groups that hammer the popup**

Run from `rewrite/`: `scripts/e2e.sh launch`, then `scripts/e2e.sh images`, then `scripts/e2e.sh cloud-saves`
Expected: all three green — `details-play` / `details-stop` / `details-playing-chip` / `details-error` / `details-warning` in `launch`, and `details-cover` / `details-description` / `details-screenshot-0..1` on the default Overview tab in `images`.

- [ ] **Step 13: Full gate and commit**

Run from `rewrite/app`: `npm run check` and `npx vitest run`.
Expected: green. No Rust changed.

```bash
cd /home/six/Documents/Programming/grid-launcher
git add rewrite/app/src/lib/Details.svelte rewrite/app/src/lib/details/OverviewTab.svelte \
  rewrite/app/src/lib/details/MediaTab.svelte rewrite/app/src/lib/details/SavesTab.svelte \
  rewrite/app/src/lib/details/FilesTab.svelte rewrite/app/src/lib/details/header.ts \
  rewrite/app/src/lib/details/header.test.ts rewrite/e2e/specs/cloud-saves.spec.ts
git commit -m "rewrite: cover-left details popup with four tabs"
```

---

### Task 4: The server-hosted video path, and the CSP that lets the embed load

Design §7's Media tab needs two video sources. `youtube_video_id` is an
embed and touches no server bytes; `path_video` is a file on the RomM server
that must be fetched **through the session client** like a cover — its URL
can never reach the DOM, because reaching the DOM would mean either a
token-bearing URL or an unauthenticated request that 401s.

**Files:**
- Create: `crates/grid-core/src/images/video.rs`
- Modify: `crates/grid-core/src/images/mod.rs:5-8` (module list)
- Modify: `crates/grid-core/src/images/cache.rs:36-58` (one accessor)
- Create: `crates/grid-core/tests/images_video.rs`
- Modify: `app/src-tauri/src/commands.rs:225-239` (beside `ensure_image`)
- Modify: `app/src-tauri/src/lib.rs:274` (handler list)
- Modify: `app/src-tauri/tauri.conf.json:22-31` (CSP)
- Modify: `app/src/lib/api.ts:329` (wrapper)

**Interfaces:**
- Consumes: `grid_core::images::cache::{ImageCache, ImageError, image_key}`; `grid_core::images::urls::{filter_to_server_host, resolve_image_url, urlsplit}`; `grid_core::romm::RommClient`.
- Produces, used by Task 5:
  - `pub fn grid_core::images::video::video_extension_for(url: &str, body: &[u8], content_type: &str) -> Option<String>` — `None` when the bytes are not a video.
  - `pub async fn grid_core::images::video::ensure_video(cache: &ImageCache, client: Option<&RommClient>, url: &str) -> Result<PathBuf, ImageError>`.
  - `pub fn ImageCache::find_with_extension(&self, key: &str, ext: &str) -> Option<PathBuf>`.
  - Tauri command `ensure_video(url: String) -> Result<String, String>`, registered in `lib.rs`.
  - TS: `api.ensureVideo(url: string): Promise<string>` — an absolute local path, to be passed through `convertFileSrc`.
  - CSP gains `"frame-src": "https://www.youtube-nocookie.com"` and `"media-src": "'self' asset: http://asset.localhost"`.

- [ ] **Step 1: Write the failing video tests**

Create `crates/grid-core/tests/images_video.rs`:

```rust
use grid_core::images::cache::{ImageCache, ImageError};
use grid_core::images::video::{ensure_video, video_extension_for};
use grid_core::romm::RommClient;
use grid_core::secrets::Credential;
use secrecy::SecretString;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// An ISO base media file: 4 size bytes, then the `ftyp` box type.
const MP4_MAGIC: &[u8] = &[0, 0, 0, 0x18, b'f', b't', b'y', b'p', b'i', b's', b'o', b'm'];
const WEBM_MAGIC: &[u8] = &[0x1A, 0x45, 0xDF, 0xA3, 0x01, 0x00, 0x00, 0x00];

fn client_for(server: &MockServer) -> RommClient {
    RommClient::new(
        &server.uri(),
        Credential::Token(SecretString::from("FAKE-TEST-TOKEN-not-real")),
    )
    .unwrap()
}

#[test]
fn video_extension_reads_the_content_type_first() {
    assert_eq!(
        video_extension_for("http://h/clip", MP4_MAGIC, "video/mp4"),
        Some("mp4".to_string())
    );
    assert_eq!(
        video_extension_for("http://h/clip", WEBM_MAGIC, "video/webm; charset=binary"),
        Some("webm".to_string())
    );
}

#[test]
fn video_extension_falls_back_to_the_magic_bytes() {
    assert_eq!(
        video_extension_for("http://h/clip", MP4_MAGIC, "application/octet-stream"),
        Some("mp4".to_string())
    );
    assert_eq!(
        video_extension_for("http://h/clip", WEBM_MAGIC, ""),
        Some("webm".to_string())
    );
}

#[test]
fn video_extension_rejects_anything_that_is_not_a_video() {
    // An HTML error page served with a 200 is the failure mode that matters:
    // without the gate it would be cached and handed to a <video> element.
    assert_eq!(
        video_extension_for("http://h/clip.mp4", b"<!doctype html>", "text/html"),
        None
    );
    assert_eq!(video_extension_for("http://h/clip.mp4", b"", "video/mp4"), None);
}

#[tokio::test]
async fn ensure_video_fetches_once_then_hits_the_cache() {
    let server = MockServer::start().await;
    let mock = Mock::given(method("GET"))
        .and(path("/assets/romm/resources/roms/1/video.mp4"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(MP4_MAGIC)
                .insert_header("content-type", "video/mp4"),
        )
        .expect(1)
        .mount_as_scoped(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let cache = ImageCache::new(dir.path().to_path_buf());
    let client = client_for(&server);
    let url = format!("{}/assets/romm/resources/roms/1/video.mp4", server.uri());

    let first = ensure_video(&cache, Some(&client), &url).await.unwrap();
    let second = ensure_video(&cache, Some(&client), &url).await.unwrap();
    assert_eq!(first, second);
    assert_eq!(first.extension().unwrap(), "mp4");
    assert!(first.starts_with(dir.path()));
    drop(mock);
}

#[tokio::test]
async fn ensure_video_with_no_client_is_offline_rather_than_a_bare_url() {
    let dir = tempfile::tempdir().unwrap();
    let cache = ImageCache::new(dir.path().to_path_buf());
    match ensure_video(&cache, None, "http://server/video.mp4").await {
        Err(ImageError::Offline) => {}
        other => panic!("expected Offline, got {other:?}"),
    }
}

#[tokio::test]
async fn ensure_video_refuses_a_non_video_body() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/assets/not-a-video.mp4"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(b"<!doctype html>".as_slice())
                .insert_header("content-type", "text/html"),
        )
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let cache = ImageCache::new(dir.path().to_path_buf());
    let client = client_for(&server);
    let url = format!("{}/assets/not-a-video.mp4", server.uri());
    match ensure_video(&cache, Some(&client), &url).await {
        Err(ImageError::NotAnImage) => {}
        other => panic!("expected NotAnImage, got {other:?}"),
    }
}
```

- [ ] **Step 2: Run it to verify it fails**

Run from `rewrite/`: `cargo test -p grid-core --test images_video`
Expected: FAIL to compile — `could not find 'video' in 'images'`.

- [ ] **Step 3: Add the cache accessor**

In `crates/grid-core/src/images/cache.rs`, add this method to `impl ImageCache`, directly after `find_existing`:

```rust
    /// The cached file for `key` under one exact extension, refreshing its
    /// mtime so the startup sweep treats it as recently used.
    /// [`find_existing`](Self::find_existing) only looks at the image
    /// extensions; video files live in the same directory under the same
    /// key scheme and need their own lookup.
    pub fn find_with_extension(&self, key: &str, ext: &str) -> Option<PathBuf> {
        let path = self.dir.join(format!("{key}.{ext}"));
        if !path.is_file() {
            return None;
        }
        if let Ok(f) = std::fs::File::options().write(true).open(&path) {
            let _ = f.set_modified(SystemTime::now());
        }
        Some(path)
    }
```

- [ ] **Step 4: Write `video.rs`**

Create `crates/grid-core/src/images/video.rs`:

```rust
//! Server-hosted game videos (`DetailedRomSchema.path_video`, design §7
//! Media tab). They share the image cache's directory and key scheme, but
//! not its content gate: `ImageCache::ensure` refuses anything that is not
//! an image, which is exactly right for covers and exactly wrong here.
//!
//! Token secrecy: the bytes are fetched through the session's `RommClient`,
//! which carries the credential in a header, and the frontend only ever
//! sees the resulting local path. No video URL built here or in the UI
//! carries a token.

use super::cache::{image_key, ImageCache, ImageError};
use super::urls::urlsplit;
use crate::romm::RommClient;
use std::path::PathBuf;

/// The extensions a cached video can be stored under, in the order the
/// cache probes them on a hit.
pub const VIDEO_EXTENSIONS: [&str; 3] = ["mp4", "webm", "mov"];

/// The extension to store a fetched body under, or `None` when the body is
/// not a video. Content-Type first (RomM serves these off disk with a real
/// type), then magic bytes, then — only if both are silent — the URL
/// suffix. An empty body is never a video.
pub fn video_extension_for(url: &str, body: &[u8], content_type: &str) -> Option<String> {
    if body.is_empty() {
        return None;
    }
    let normalized = content_type.trim().to_lowercase();
    let normalized = normalized.split(';').next().unwrap_or("");
    match normalized {
        "video/mp4" => return Some("mp4".to_string()),
        "video/quicktime" => return Some("mov".to_string()),
        "video/webm" => return Some("webm".to_string()),
        _ => {}
    }
    // ISO base media: a 4-byte box size, then `ftyp`.
    if body.len() >= 12 && &body[4..8] == b"ftyp" {
        return Some("mp4".to_string());
    }
    // Matroska/WebM EBML header.
    if body.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
        return Some("webm".to_string());
    }
    if normalized.starts_with("video/") {
        // A video type this build does not name specifically still plays in
        // the webview more often than not; store it under the URL's own
        // extension when that is one we know, else mp4.
        let path = urlsplit(url).path.to_lowercase();
        for ext in VIDEO_EXTENSIONS {
            if path.ends_with(&format!(".{ext}")) {
                return Some(ext.to_string());
            }
        }
        return Some("mp4".to_string());
    }
    None
}

/// Cache hit → path. Miss with no client → [`ImageError::Offline`]. Miss
/// with a client → fetch through the session client, gate on the body, and
/// write atomically beside the covers.
///
/// Deliberately simpler than [`ImageCache::ensure`]: no in-flight dedup and
/// no negative cache. At most one video is on screen at a time, so there is
/// nothing to deduplicate, and a video that failed once should be
/// retryable by reopening the tab.
pub async fn ensure_video(
    cache: &ImageCache,
    client: Option<&RommClient>,
    url: &str,
) -> Result<PathBuf, ImageError> {
    let key = image_key(url);
    for ext in VIDEO_EXTENSIONS {
        if let Some(path) = cache.find_with_extension(&key, ext) {
            return Ok(path);
        }
    }
    let Some(client) = client else {
        return Err(ImageError::Offline);
    };
    let (bytes, content_type) = client.get_bytes_with_type(url).await?;
    let Some(ext) = video_extension_for(url, &bytes, &content_type) else {
        return Err(ImageError::NotAnImage);
    };
    let io = |e: std::io::Error| ImageError::Io(e.to_string());
    std::fs::create_dir_all(cache.dir()).map_err(io)?;
    let target = cache.dir().join(format!("{key}.{ext}"));
    let tmp = cache.dir().join(format!("{key}.part"));
    std::fs::write(&tmp, &bytes).map_err(io)?;
    std::fs::rename(&tmp, &target).map_err(io)?;
    Ok(target)
}
```

Add the module to `crates/grid-core/src/images/mod.rs`, keeping the list alphabetical:

```rust
pub mod cache;
pub mod replenish;
pub mod sweep;
pub mod urls;
pub mod video;
```

- [ ] **Step 5: Run the tests to verify they pass**

Run from `rewrite/`: `cargo test -p grid-core --test images_video`
Expected: PASS, 6 tests.

Then run `cargo test -p grid-core --test images_sweep`.
Expected: PASS unchanged — the sweep keys off the file stem and treats a `.mp4` beside the covers as one more unpinned entry, which is the wanted behaviour: a trailer is evictable and refetchable.

- [ ] **Step 6: Add the `ensure_video` command**

In `app/src-tauri/src/commands.rs`, add directly after `ensure_image` (which ends at line 239):

```rust
/// The local path of a server-hosted game video, fetching it through the
/// session client on a cache miss. Mirrors [`ensure_image`]'s resolution and
/// host filter exactly, so a `path_video` pointing anywhere but the
/// configured server is refused rather than fetched.
#[tauri::command]
pub async fn ensure_video(state: State<'_, AppState>, url: String) -> Result<String, String> {
    let base = state.session.server_url();
    let resolved = filter_to_server_host(&resolve_image_url(&url, &base), &base);
    if resolved.is_empty() {
        return Err("filtered".to_string());
    }
    let client = state.session.client();
    let path = grid_core::images::video::ensure_video(
        state.session.cache(),
        client.as_deref(),
        &resolved,
    )
    .await
    .map_err(err)?;
    Ok(path.to_string_lossy().into_owned())
}
```

In `app/src-tauri/src/lib.rs`, add to the `generate_handler!` list directly after `commands::ensure_image,`:

```rust
            commands::ensure_video,
```

- [ ] **Step 7: Widen the CSP**

In `app/src-tauri/tauri.conf.json`, replace the `"csp"` object with:

```json
      "csp": {
        "default-src": "'self'",
        "img-src": "'self' asset: http://asset.localhost",
        "media-src": "'self' asset: http://asset.localhost",
        "frame-src": "https://www.youtube-nocookie.com",
        "connect-src": "ipc: http://ipc.localhost",
        "style-src": "'unsafe-inline' 'self'"
      }
```

`media-src` lets a cached video file load over the asset protocol (`default-src 'self'` alone blocks it, silently). `frame-src` names the ONE origin the YouTube embed may come from — without it the iframe falls back to `default-src 'self'` and renders blank with no console error.

- [ ] **Step 8: Add the TS wrapper**

In `app/src/lib/api.ts`, add directly after the `ensureImage` line:

```ts
  ensureVideo: (url: string) => invoke<string>('ensure_video', { url }),
```

- [ ] **Step 9: Full gate and commit**

Run from `rewrite/`: `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo clippy -p app --all-targets --features e2e -- -D warnings`, `cargo test --workspace`. Then from `rewrite/app`: `npm run check`, `npx vitest run`.
Expected: all clean/green.

```bash
cd /home/six/Documents/Programming/grid-launcher
git add rewrite/crates/grid-core/src/images/video.rs rewrite/crates/grid-core/src/images/mod.rs \
  rewrite/crates/grid-core/src/images/cache.rs rewrite/crates/grid-core/tests/images_video.rs \
  rewrite/app/src-tauri/src/commands.rs rewrite/app/src-tauri/src/lib.rs \
  rewrite/app/src-tauri/tauri.conf.json rewrite/app/src/lib/api.ts
git commit -m "rewrite: cache server-hosted game videos through the session client"
```

---

### Task 5: The fullscreen media viewer

**Files:**
- Create: `app/src/lib/details/MediaViewer.svelte`
- Modify: `app/src/lib/details/MediaTab.svelte` (props change: it is handed the gallery instead of computing it, and reports clicks)
- Modify: `app/src/lib/Details.svelte` (own the gallery and the viewer index)

**Interfaces:**
- Consumes: Task 2's `media.ts` (`MediaItem`, `galleryItems`, `nextIndex`, `prevIndex`, `youtubeEmbedUrl`); Task 4's `api.ensureVideo`; `Image.svelte`.
- Produces:
  - `MediaViewer.svelte` props: `{ items: MediaItem[]; index: number; onIndex: (index: number) => void; onClose: () => void }`.
  - `MediaTab.svelte` props become `{ items: MediaItem[]; onOpen: (index: number) => void }`.
  - New test ids: `media-viewer`, `media-viewer-close`, `media-viewer-prev`, `media-viewer-next`, `media-viewer-caption`, `media-viewer-image`, `media-viewer-youtube`, `media-viewer-video`.

- [ ] **Step 1: Create `MediaViewer.svelte`**

```svelte
<script lang="ts">
  import { convertFileSrc } from '@tauri-apps/api/core';
  import { api } from '../api';
  import Image from '../Image.svelte';
  import { nextIndex, prevIndex, youtubeEmbedUrl, type MediaItem } from './media';

  let {
    items,
    index,
    onIndex,
    onClose,
  }: {
    items: MediaItem[];
    index: number;
    onIndex: (index: number) => void;
    onClose: () => void;
  } = $props();

  let viewerEl = $state<HTMLElement | null>(null);
  let current = $derived(items[index] ?? null);

  // A server-hosted video is fetched through the session client and played
  // from the local cache (`ensure_video`, Task 4). The server URL never
  // reaches the DOM, so no request from the page needs a token.
  let videoSrc = $state<string | null>(null);
  let videoError = $state(false);

  $effect(() => {
    const item = current;
    videoSrc = null;
    videoError = false;
    if (item === null || item.kind !== 'video') return;
    let cancelled = false;
    api
      .ensureVideo(item.url)
      .then((path) => {
        if (!cancelled) videoSrc = convertFileSrc(path);
      })
      .catch(() => {
        if (!cancelled) videoError = true;
      });
    return () => {
      cancelled = true;
    };
  });

  $effect(() => {
    viewerEl?.focus();
  });

  function go(next: number) {
    onIndex(next);
  }

  function onKey(e: KeyboardEvent) {
    // The popup behind this also closes on Escape; without stopping the
    // event, one press would shut both and the user would lose their place.
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      onClose();
      return;
    }
    if (e.key === 'ArrowRight') {
      e.preventDefault();
      e.stopPropagation();
      go(nextIndex(index, items.length));
      return;
    }
    if (e.key === 'ArrowLeft') {
      e.preventDefault();
      e.stopPropagation();
      go(prevIndex(index, items.length));
    }
  }

  function onBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) onClose();
  }
</script>

{#if current}
  <div
    data-testid="media-viewer"
    class="viewer"
    bind:this={viewerEl}
    role="dialog"
    aria-modal="true"
    aria-label={current.caption}
    tabindex="-1"
    onkeydown={onKey}
    onclick={onBackdropClick}
  >
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

    <div class="stage">
      {#if current.kind === 'screenshot'}
        <Image
          url={current.url}
          alt={current.caption}
          placeholder="Screenshot"
          data-testid="media-viewer-image"
        />
      {:else if current.kind === 'youtube'}
        <iframe
          data-testid="media-viewer-youtube"
          class="frame"
          src={youtubeEmbedUrl(current.videoId)}
          title={current.caption}
          allow="accelerometer; clipboard-write; encrypted-media; picture-in-picture"
          allowfullscreen
        ></iframe>
      {:else if videoSrc}
        <!-- svelte-ignore a11y_media_has_caption -->
        <video data-testid="media-viewer-video" class="frame" src={videoSrc} controls></video>
      {:else}
        <p class="pending" data-testid="media-viewer-video-pending">
          {videoError ? 'This video could not be loaded' : 'Loading video…'}
        </p>
      {/if}
    </div>

    <p class="caption" data-testid="media-viewer-caption">{current.caption}</p>
  </div>
{/if}

<style>
  .viewer {
    position: fixed;
    inset: 0;
    z-index: 30;
    background: rgba(0, 0, 0, 0.9);
    display: grid;
    grid-template-rows: 1fr auto;
    place-items: center;
    padding: 48px;
    box-sizing: border-box;
  }

  .stage {
    display: grid;
    place-items: center;
    width: 100%;
    height: 100%;
    min-height: 0;
  }

  .stage :global(img) {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
  }

  .frame {
    width: min(100%, 1280px);
    aspect-ratio: 16 / 9;
    border: none;
    background: #000;
  }

  .caption {
    margin: 12px 0 0;
    color: #fff;
    font-size: 13px;
    text-align: center;
  }

  .pending {
    margin: 0;
    color: #fff;
    font-size: 13px;
  }

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
</style>
```

- [ ] **Step 2: Make the tiles clickable in `MediaTab.svelte`**

Replace the whole `<script>` block of `app/src/lib/details/MediaTab.svelte` with:

```svelte
<script lang="ts">
  import Image from '../Image.svelte';
  import type { MediaItem } from './media';

  let {
    items,
    onOpen,
  }: {
    items: MediaItem[];
    onOpen: (index: number) => void;
  } = $props();
</script>
```

and replace the `{#each}` body's `<div class="tile" …>` element with a button, so the tile is keyboard reachable as well as clickable:

```svelte
    {#each items as item, i (item.caption)}
      <button
        class="tile"
        data-testid={`details-media-${i}`}
        title={item.caption}
        onclick={() => onOpen(i)}
      >
        {#if item.kind === 'screenshot'}
          <Image url={item.url} alt={item.caption} placeholder="Screenshot" />
        {:else}
          <div class="video-tile">▶ {item.kind === 'youtube' ? 'Trailer' : 'Video'}</div>
        {/if}
      </button>
    {/each}
```

Add to the `.tile` rule in the same file's `<style>`, so a `<button>` does not inherit the browser's chrome:

```css
  .tile {
    padding: 0;
    cursor: pointer;
    display: block;
    width: 100%;
  }
```

- [ ] **Step 3: Own the gallery and the viewer in `Details.svelte`**

Add to the imports:

```ts
  import MediaViewer from './details/MediaViewer.svelte';
  import { galleryItems } from './details/media';
```

Add beside the other `$derived`/`$state` declarations, after `screenshotUrls`:

```ts
  // The gallery lives here, not in MediaTab: the viewer is rendered above
  // the whole dialog, so both need the same list and the same indices.
  let mediaItems = $derived(
    galleryItems({
      title: subject.name,
      screenshotUrls,
      youtubeVideoId: detail?.youtube_video_id ?? '',
      videoPath: detail?.video_path ?? '',
    })
  );
  let viewerIndex = $state<number | null>(null);
```

Replace the Media branch of the tab panel with:

```svelte
          {:else if tab === 'media'}
            <MediaTab items={mediaItems} onOpen={(i) => (viewerIndex = i)} />
```

Add, directly after the `{#if showNativeSettings …}{/if}` block at the end of the markup:

```svelte
{#if viewerIndex !== null}
  <MediaViewer
    items={mediaItems}
    index={viewerIndex}
    onIndex={(i) => (viewerIndex = i)}
    onClose={() => (viewerIndex = null)}
  />
{/if}
```

- [ ] **Step 4: Run the frontend gates**

Run from `rewrite/app`: `npm run check` then `npx vitest run`
Expected: both green. `svelte-check` must report 0 errors — in particular, `MediaTab`'s old `name` / `screenshotUrls` / `detail` props are gone and `Details.svelte` no longer passes them.

- [ ] **Step 5: Full gate and commit**

Run from `rewrite/app`: `npm run check` and `npx vitest run`. No Rust changed.

```bash
cd /home/six/Documents/Programming/grid-launcher
git add rewrite/app/src/lib/details/MediaViewer.svelte rewrite/app/src/lib/details/MediaTab.svelte \
  rewrite/app/src/lib/Details.svelte
git commit -m "rewrite: fullscreen media viewer for the details Media tab"
```

---

### Task 6: The Related row and the Files tab's firmware row

The two pieces that need a second data source: Related needs the platform's
game list to filter against, and the firmware row needs the platform
firmware status that plan 2's Server header already added.

**Files:**
- Modify: `app/src/lib/details/OverviewTab.svelte` (Related row)
- Modify: `app/src/lib/details/FilesTab.svelte` (firmware row)
- Modify: `app/src/lib/Details.svelte` (fetch the platform list and the firmware status; pass both down)

**Interfaces:**
- Consumes: Task 2's `related.ts`; `api.listGames(platformId)`; `api.platformFirmwareStatus(platformId, platform)`, `api.installFirmwareForPlatform(platformId, platform)` and `FIRMWARE_PASS_FINISHED_EVENT` (plan 2, `api.ts:410-412` and `:296`); `server/header.ts`'s `firmwareChipLabel` / `firmwareInstallable` (plan 2).
- Produces:
  - `OverviewTab.svelte` gains the prop `serverTitles: string[]`.
  - `FilesTab.svelte` gains the props `firmware: FirmwareChipState` and `onInstallFirmware: (() => void) | null`.
  - New test ids: `details-related`, `details-related-<i>`, `details-firmware`, `details-firmware-install`.

- [ ] **Step 1: Add the Related row to `OverviewTab.svelte`**

Add to the imports:

```ts
  import { relatedKindLabel, relatedOnServer } from './related';
```

Add `serverTitles` to the props block, which becomes:

```ts
  let {
    name,
    description,
    screenshotUrls,
    detail,
    serverTitles,
  }: {
    name: string;
    description: string;
    screenshotUrls: string[];
    detail: RomDetail | null;
    /** Every title on this game's platform, for design §7's "filtered to
     *  titles present on the server". Empty until the list loads. */
    serverTitles: string[];
  } = $props();

  let related = $derived(relatedOnServer(detail?.related ?? [], serverTitles));
```

Add, directly after the screenshot strip block and before `</div>`:

```svelte
  {#if related.length}
    <div class="related" data-testid="details-related">
      <h3>Related</h3>
      <div class="chips">
        {#each related as entry, i (entry.name)}
          <span class="related-chip" data-testid={`details-related-${i}`}>
            {entry.name}
            <em>{relatedKindLabel(entry.kind)}</em>
          </span>
        {/each}
      </div>
    </div>
  {/if}
```

Add to the same file's `<style>`:

```css
  .related h3 {
    margin: 0 0 8px;
    font-size: 13px;
    font-weight: 600;
    color: var(--text-h);
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .related-chip {
    display: inline-flex;
    align-items: baseline;
    gap: 6px;
    font-size: 12px;
    padding: 4px 10px;
    border-radius: var(--r-pill);
    background: var(--surface);
    border: 1px solid var(--border);
    color: var(--text);
  }

  .related-chip em {
    font-style: normal;
    color: var(--text-muted);
    font-size: 11px;
  }
```

- [ ] **Step 2: Add the firmware row to `FilesTab.svelte`**

Add to the imports:

```ts
  import { firmwareChipLabel, firmwareInstallable, type FirmwareChipState } from '../server/header';
```

Extend the props block to:

```ts
  let {
    files,
    installedVersion,
    serverVersion,
    installedNow,
    firmware,
    onInstallFirmware,
  }: {
    files: RomFile[];
    installedVersion: string;
    serverVersion: string;
    installedNow: boolean;
    /** The platform's firmware status; `null` while it is in flight. */
    firmware: FirmwareChipState;
    /** `null` while a pass this popup started has not finished yet. */
    onInstallFirmware: (() => void) | null;
  } = $props();
```

Add, directly before the closing `</div>` of `.files`:

```svelte
  <h3>Firmware</h3>
  <div class="row" data-testid="details-firmware">
    <span class="name">{firmwareChipLabel(firmware)}</span>
    <span class="size"></span>
    {#if firmwareInstallable(firmware)}
      <button
        data-testid="details-firmware-install"
        class="firmware-install"
        disabled={onInstallFirmware === null}
        onclick={() => onInstallFirmware?.()}
      >
        {onInstallFirmware === null ? 'Requested…' : 'Install'}
      </button>
    {:else}
      <span class="version"></span>
    {/if}
  </div>
```

Add to the same file's `<style>`:

```css
  .firmware-install {
    font: inherit;
    font-size: 12px;
    padding: 4px 12px;
    border-radius: var(--r-control);
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text);
    cursor: pointer;
  }

  .firmware-install:disabled {
    opacity: 0.6;
    cursor: default;
  }
```

- [ ] **Step 3: Fetch both sources in `Details.svelte`**

Add to the imports:

```ts
  import { listen } from '@tauri-apps/api/event';
  import { FIRMWARE_PASS_FINISHED_EVENT } from './api';
  import type { FirmwareChipState } from './server/header';
```

Add beside the other state:

```ts
  // Design §7 Overview: Related is filtered against the platform's own game
  // list. Fetched once the detail names the platform id; a failure leaves
  // the list empty, which renders no Related row at all rather than a row
  // of titles the user may not have.
  let serverTitles = $state<string[]>([]);
  let firmware = $state<FirmwareChipState>(null);
  let firmwarePending = $state(false);

  $effect(() => {
    const platformId = detail?.platform_id ?? null;
    if (platformId === null || !session.connected) return;
    if (detail !== null && detail.related.length === 0) return; // nothing to filter
    let cancelled = false;
    api
      .listGames(platformId)
      .then((games) => {
        if (!cancelled) serverTitles = games.map((g) => g.name);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  });

  $effect(() => {
    const platformId = detail?.platform_id ?? null;
    if (platformId === null || !session.connected) return;
    let cancelled = false;
    api
      .platformFirmwareStatus(platformId, subject.platformName)
      .then((status) => {
        if (!cancelled) firmware = status;
      })
      .catch(() => {
        if (!cancelled) firmware = 'unavailable';
      });
    return () => {
      cancelled = true;
    };
  });

  // The pass runs in the background and answers with one event; the button
  // stays disabled until it lands, whether or not anything was fetched.
  $effect(() => {
    const unlisten = listen(FIRMWARE_PASS_FINISHED_EVENT, () => {
      firmwarePending = false;
    });
    return () => {
      void unlisten.then((off) => off());
    };
  });

  async function installFirmware() {
    const platformId = detail?.platform_id ?? null;
    if (platformId === null) return;
    error = null;
    firmwarePending = true;
    try {
      await api.installFirmwareForPlatform(platformId, subject.platformName);
    } catch (err) {
      error = errorMessage(err);
      firmwarePending = false;
    }
  }
```

Pass them to the two tabs:

```svelte
          {#if tab === 'overview'}
            <OverviewTab name={subject.name} {description} {screenshotUrls} {detail} {serverTitles} />
```

```svelte
          {:else}
            <FilesTab
              files={detail?.files ?? []}
              {installedVersion}
              {serverVersion}
              {installedNow}
              {firmware}
              onInstallFirmware={firmwarePending ? null : installFirmware}
            />
          {/if}
```

- [ ] **Step 4: Run the frontend gates**

Run from `rewrite/app`: `npm run check` then `npx vitest run`
Expected: both green.

- [ ] **Step 5: Run the firmware E2E group**

Run from `rewrite/`: `scripts/e2e.sh firmware`
Expected: green — the popup now asks for `platform_firmware_status` on open, which the fixture server answers, and the existing per-game firmware assertions are unchanged.

- [ ] **Step 6: Commit**

```bash
cd /home/six/Documents/Programming/grid-launcher
git add rewrite/app/src/lib/details/OverviewTab.svelte rewrite/app/src/lib/details/FilesTab.svelte \
  rewrite/app/src/lib/Details.svelte
git commit -m "rewrite: details Related row and Files firmware row"
```

---

### Task 7: E2E — the tabs, the header, the viewer, and the D-UI-10 version

**Files:**
- Modify: `e2e/fixtures/rom-details.json` (rom 101 grows the IGDB block, the media fields and a file timestamp)
- Modify: `e2e/fixtures-updates/rom-details.json` (rom 802's file grows a timestamp)
- Modify: `e2e/specs/images-a.spec.ts` (a new case, after the first)
- Modify: `e2e/specs/updates.spec.ts:255-295` (the rom 802 case gains the Files tab assertions)

**Interfaces:**
- Consumes: every id Tasks 3–6 produced. No production code changes here.
- Produces: nothing code reads.

The mock server hands `/api/roms/:id` the fixture object verbatim (`e2e/mock-romm/server.mjs:631-640`), so new fields need no server change.

- [ ] **Step 1: Extend rom 101's fixture**

In `e2e/fixtures/rom-details.json`, replace rom `"101"`'s `metadatum` and `files` entries and append the new fields, so the object reads:

```json
  "101": {
    "id": 101,
    "name": "Super Mario World",
    "fs_name_no_ext": "Super Mario World",
    "platform_id": 1,
    "platform_display_name": "Super Nintendo Entertainment System",
    "fs_name": "Super Mario World.zip",
    "summary": "A classic platformer.",
    "regions": ["USA"],
    "languages": ["en"],
    "tags": [],
    "revision": null,
    "fs_size_bytes": 0,
    "updated_at": "2026-01-01T00:00:00Z",
    "is_identified": true,
    "youtube_video_id": "dQw4w9WgXcQ",
    "files": [
      {
        "id": 1001,
        "file_name": "Super Mario World.zip",
        "file_size_bytes": 0,
        "is_top_level": true,
        "last_modified": "2026-01-02T03:04:05"
      }
    ],
    "metadatum": {
      "average_rating": 9.2,
      "genres": ["Platformer"],
      "companies": ["Nintendo"],
      "first_release_date": 653529600,
      "franchises": ["Super Mario"],
      "game_modes": ["Single player"],
      "player_count": "1"
    },
    "igdb_metadata": {
      "similar_games": [
        { "id": 900, "name": "Chrono Trigger", "slug": "ct", "type": "game", "cover_url": "" },
        { "id": 901, "name": "A Game Nobody Owns", "slug": "ango", "type": "game", "cover_url": "" }
      ]
    },
    "path_cover_small": "/assets/romm/resources/roms/101/cover/small.png",
    "path_cover_large": "/assets/romm/resources/roms/101/cover/large.png",
    "merged_screenshots": [
      "/assets/romm/resources/roms/101/screenshots/1.png",
      "/assets/romm/resources/roms/101/screenshots/2.png",
      "https://img.example/box-front.jpg"
    ]
  },
```

`Chrono Trigger` is on platform 1 as `Chrono Trigger (USA)` (`e2e/fixtures/roms.json`), so `normalizeTitle` matches it; `A Game Nobody Owns` is on no platform, so the Related row must drop it. That pair is the whole point of the fixture.

- [ ] **Step 2: Give rom 802's archive a timestamp**

In `e2e/fixtures-updates/rom-details.json`, replace rom `"802"`'s `files` array with:

```json
    "files": [
      {
        "id": 4802,
        "file_name": "mygame (v1.1.0).zip",
        "file_size_bytes": 0,
        "is_top_level": true,
        "last_modified": "2026-05-04T00:00:00"
      },
      { "id": 4803, "file_name": "game.json", "file_size_bytes": 0, "is_top_level": true }
    ],
```

- [ ] **Step 3: Write the failing details-popup case**

In `e2e/specs/images-a.spec.ts`, insert this `it` block directly after the existing
`renders the large cover and the filtered screenshot list for rom 101` case:

```ts
  it('renders the redesigned popup: header, tabs, related, media viewer and file version', async () => {
    await $(testId('game-card-101')).click();
    await $(testId('details-panel')).waitForExist({ timeout: TRANSITION_TIMEOUT });

    // Design §7's right header: platform · year · developer · genres · rating.
    await expect($(testId('details-header-line'))).toHaveText(
      'Super Nintendo Entertainment System · 1990 · Nintendo · Platformer · ★ 9.2',
    );
    await expect($(testId('details-verification'))).toHaveText('Identified');

    // All four §11 tab ids exist, and Overview is the one showing.
    for (const name of ['overview', 'media', 'saves', 'files']) {
      await expect($(testId(`details-tab-${name}`))).toExist();
    }
    await expect($(testId('details-description'))).toHaveText('A classic platformer.');
    await expect($(testId('details-meta-players'))).toHaveText('1');

    // Related is filtered to titles the platform actually holds: "Chrono
    // Trigger" is rom 102 (as "Chrono Trigger (USA)"), "A Game Nobody Owns"
    // is on no platform and must not appear.
    await $(testId('details-related-0')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the Related row never rendered after the platform list loaded',
    });
    // The chip renders the title and its kind label; `toHaveText`
    // normalizes the whitespace between the two spans to one space.
    await expect($(testId('details-related-0'))).toHaveText('Chrono Trigger Similar');
    await expect($(testId('details-related-1'))).not.toExist();

    // Media: two screenshots plus the YouTube trailer tile.
    await $(testId('details-tab-media')).click();
    await $(testId('details-media-0')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('details-media-2'))).toExist();

    await $(testId('details-media-0')).click();
    await $(testId('media-viewer')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the fullscreen media viewer never opened',
    });
    await expect($(testId('media-viewer-caption'))).toHaveText(
      'Super Mario World — screenshot 1',
    );
    await $(testId('media-viewer-next')).click();
    await expect($(testId('media-viewer-caption'))).toHaveText(
      'Super Mario World — screenshot 2',
    );
    // Wrapping past the last item returns to the first (media.ts nextIndex).
    await $(testId('media-viewer-next')).click();
    await expect($(testId('media-viewer-caption'))).toHaveText('Super Mario World — trailer');
    await expect($(testId('media-viewer-youtube'))).toHaveAttribute(
      'src',
      'https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ',
    );
    await $(testId('media-viewer-next')).click();
    await expect($(testId('media-viewer-caption'))).toHaveText(
      'Super Mario World — screenshot 1',
    );

    // Esc closes the viewer and leaves the popup open.
    await browser.keys(['Escape']);
    await $(testId('media-viewer')).waitForExist({ timeout: TRANSITION_TIMEOUT, reverse: true });
    await expect($(testId('details-panel'))).toExist();

    // Files: D-UI-10 with no version tag in the name falls back to the
    // file's own last_modified date.
    await $(testId('details-tab-files')).click();
    await $(testId('details-file-1001')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('details-file-version-1001'))).toHaveText('2026-01-02');

    // Leave the session's remembered tab on Overview: it is module state,
    // so it would otherwise decide which tab the next case's popup opens on.
    await $(testId('details-tab-overview')).click();
    await $(testId('details-close')).click();
    await $(testId('details-panel')).waitForExist({ timeout: TRANSITION_TIMEOUT, reverse: true });
  });
```

- [ ] **Step 4: Add the Files-tab assertions to the native update case**

In `e2e/specs/updates.spec.ts`, inside the rom 802 case, directly after the two
existing `details-version` / `details-update` assertions at lines 261-262, insert:

```ts
    // D-UI-10 on the Files tab: the tag parsed out of each file name, and
    // the installed-vs-server line above them.
    await $(testId('details-tab-files')).click();
    await expect($(testId('details-files-version'))).toHaveText(
      'Installed v1.0.0 · Server v1.1.0',
    );
    await expect($(testId('details-file-version-4802'))).toHaveText('v1.1.0');
    // `game.json` carries no tag, so it falls back to its last_modified —
    // which this fixture file does not have, leaving the cell blank.
    expect(await $(testId('details-file-version-4803')).getText()).toBe('');
    await $(testId('details-tab-overview')).click();
```

- [ ] **Step 5: Run the two affected groups**

Run from `rewrite/`: `scripts/e2e.sh images` then `scripts/e2e.sh updates`
Expected: both green. If the Related assertion times out, the platform list fetch is the suspect — check that `Details.svelte`'s `listGames` effect runs for a Server-opened subject (it keys off `detail.platform_id`, which only exists once `get_rom_detail` resolves).

- [ ] **Step 6: Run the full sweep**

Run from `rewrite/`: `scripts/e2e.sh`
Expected: every group green. This is the plan's gate; a failing group here blocks Task 8.

- [ ] **Step 7: Commit**

```bash
cd /home/six/Documents/Programming/grid-launcher
git add rewrite/e2e/fixtures/rom-details.json rewrite/e2e/fixtures-updates/rom-details.json \
  rewrite/e2e/specs/images-a.spec.ts rewrite/e2e/specs/updates.spec.ts
git commit -m "rewrite: E2E for the details tabs, media viewer and file versions"
```

---

### Task 8: Documentation

**Files:**
- Modify: `SPEC.md:73-81` (the Game Details View section)
- Modify: `rewrite/README.md` (the residual manual checklist)
- Modify: `docs/porting/07-covers-images.md` (a video-cache note)

**Interfaces:**
- Consumes: nothing. Documentation only.
- Produces: nothing code reads.

- [ ] **Step 1: Rewrite the SPEC.md details section**

In `SPEC.md`, replace the `### Game Details View` section — its heading and the
paragraphs and list through `- 'Manage States' when state sync is supported for that platform` — with:

```markdown
### Game Details View
Clicking a game in the Library or Server sections opens a popup, at most 1040×680, centred
over a dimmed and blurred shell. Esc and the ✕ close it.

The left column is fixed at 240px: the large cover, then the primary action (Play, Stop,
Install / Install App, or Cancel while an install is live), Update when the server holds a
newer copy, Install Update / Install DLC for PS4 and Xbox 360 games, Game Settings for
native games, Uninstall, and a cloud button that opens the Saves tab. Under them the column
states when the game was last played and which emulator — and, for RetroArch, which core —
would launch it.

The right side carries the title, one header line (platform · release year · developer ·
genres · rating), chips for the playing state, the identification state, region and language
flags and the version, then four tabs:

- **Overview** — the summary, a metadata grid (developer, companies, release, genres, game
  modes, players, franchises), the first six screenshots, and a Related row filtered to
  titles the server actually holds for that platform.
- **Media** — every screenshot plus the trailer and any server-hosted video. Clicking a tile
  opens a fullscreen viewer with arrows, Esc and a caption.
- **Saves** — the Manage Saves / Emulator Saves / Manage States panels described below.
- **Files** — every file the server lists with its size and its version (the version tag
  parsed out of the file name, else the file's last-modified date), the installed-versus-
  server version line, the PS4 / Xbox 360 content files, and the platform's firmware row
  with an Install action when the server offers firmware.

The last tab is remembered for the rest of the session.

The button bar area should include:
- `Manage Saves` for normal per-game cloud-save platforms
- `Emulator Saves` for shared/global save media such as Xemu and Redream VMUs
- `Manage States` when state sync is supported for that platform
```

- [ ] **Step 2: Add the manual checklist rows**

In `rewrite/README.md`, append to the "Residual manual checklist" list:

```markdown
- **Details video**: on a RomM server whose game carries a `path_video`, open Details ›
  Media and play it. Confirm it plays from the local cache (the file appears under the
  covers directory) and that the network request came from the app, not the webview.
- **YouTube trailer**: with a game whose `youtube_video_id` is set, open the trailer in the
  fullscreen viewer and confirm it plays. A blank frame means the `frame-src` CSP entry is
  missing — the webview reports nothing for a blocked frame.
- **Related row**: open a game whose IGDB metadata lists similar games and confirm only
  titles the server actually holds appear, and that clicking one is not offered (the row is
  informational until collections land).
```

- [ ] **Step 3: Note the video cache in the porting doc**

In `docs/porting/07-covers-images.md`, append:

```markdown
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
```

- [ ] **Step 4: Commit**

```bash
cd /home/six/Documents/Programming/grid-launcher
git add SPEC.md rewrite/README.md docs/porting/07-covers-images.md
git commit -m "rewrite: document the redesigned details popup"
```

---

## Self-review

**1. Spec coverage.**

| Spec requirement (§7 / D-UI-4 / D-UI-10 / §11) | Task |
|---|---|
| Dialog 1040×680 max, centred over a dimmed **and blurred** shell | 3 (`.backdrop`, `.panel`) |
| Esc and ✕ close; `details-panel` kept | 3 (`onKey`, `details-close`) |
| Left column 240px: cover from `path_cover_large` | 3 (`.layout` `240px 1fr`, `Image` on `coverLarge ?? coverSmall`) |
| Play / Install primary, Update, existing ids and label rules verbatim | 3 (the `.action` stack; `details-play`/`-stop`/`-install`/`-update`/`-playing-chip` unchanged) |
| Cloud status button opening the cloud panel, honouring `initialCloudMode` | 3 (`details-cloud-status`, `openCloud`, `tab` initialised from `initialCloudMode`) |
| Gear menu (native settings, emulator override, remove) | 3 — **deviated deliberately**, see "Deliberate deviations": a visible secondary stack instead of a popover; no per-game emulator override exists in the backend |
| Play time and the emulator + core that will launch | 3 (`details-last-played`, `details-emulator`; `lastPlayedText`, `launchTargetLine`) |
| Right header: title, platform, release date, developer, genres, rating, region/language flags, `is_identified` | 1 (`is_identified`), 2 (`headerLine`, `flagList`, `verificationLabel`), 3 (markup) |
| Tabs `details-tab-overview` / `-media` / `-saves` / `-files`, last tab remembered per session | 2 (`tabs.ts`), 3 (tab bar) |
| Overview: `summary`, metadata grid, screenshot strip (first six) | 2 (`overviewStrip`), 3 (`OverviewTab`) |
| Overview: Related filtered to titles on the server | 1 (`related`), 2 (`related.ts`), 6 (`serverTitles` + row) |
| Media: gallery + video; fullscreen `media-viewer` with arrows, Esc, caption | 2 (`media.ts`), 4 (`ensure_video`, CSP), 5 (`MediaViewer`) |
| Saves: records with timestamps and sizes, upload/restore, scope warning | 3 (`SavesTab` re-hosts the existing `CloudPanel`, which already carries all of it) |
| Files: `files[]` name/size/`last_modified` | 1 (`last_modified`), 2 (`files.ts`), 3 (`FilesTab`) |
| D-UI-10 version: parsed tag, else the file's `last_modified` date | 2 (`fileVersionLabel`), 3 (rows + the installed-vs-server line), 7 (asserted) |
| Files: PS4/Xbox 360 content rows | 2 (`contentRows`), 3 (`FilesTab`) |
| Files: firmware row | 6 |
| Every string E2E asserts today stays verbatim | 3 (update toast, native confirm, `details-error`, `details-warning` copied unchanged), 7 (full sweep) |
| Only `app.css` tokens for colour; `--m-*` for motion | 3, 5, 6 (every new rule uses `var(--…)`; the old literal `#e5484d` / `#e5a53a` became `--danger` / `--warning`) |

No §7 requirement is left without a task. The one requirement not implemented as written is the gear menu, called out at the top of the plan rather than quietly dropped.

**2. Placeholder scan.** No "TBD", no "add error handling", no "similar to Task N": every task repeats the code it needs, including the parts Task 3 relocates verbatim out of the current `Details.svelte`. Task 3's Step 9 is the whole file, not a diff, precisely so an engineer reading tasks out of order cannot assemble half of it.

**3. Type consistency.**

- `RelatedGame { name, kind }` is defined in Task 1 (Rust + TS) and consumed unchanged in Tasks 2 (`relatedOnServer`), 6 (`OverviewTab`) and 7 (fixture).
- `FirmwareChipState` comes from plan 2's `server/header.ts` and is the type of `FilesTab`'s `firmware` prop in Task 6 — including its `null` and `'unavailable'` cases, which `firmwareChipLabel` already names.
- `MediaItem` is one discriminated union (`screenshot | youtube | video`), produced by `galleryItems` in Task 2 and consumed in Tasks 3 (`MediaTab`) and 5 (`MediaTab`, `MediaViewer`). `MediaTab`'s props change once, in Task 5, and that change is stated in that task's Interfaces block.
- `fileVersionLabel(fileName, lastModified)` has that argument order in Task 2's definition, in `files.ts`'s `fileRows`, and in `Details.svelte`'s `serverVersion` / `installedVersion` — checked at all four call sites.
- `epochDate` is defined in Task 3's `header.ts` extension and used in Task 3's `installedVersion`; it is imported there in the same step.
- `ensure_video(cache, client, url)` has that argument order in `video.rs`, in the `images_video.rs` tests, and in the `ensure_video` command.
- Test ids: every id in the Task 7 spec (`details-header-line`, `details-verification`, `details-meta-players`, `details-related-0`, `details-media-0..2`, `media-viewer*`, `details-file-1001`, `details-file-version-1001`, `details-files-version`, `details-tab-*`) is produced by Task 3, 5 or 6, and each appears in that task's Interfaces block.

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-09-04-ui-redesign-3-details.md`. Two execution options:

1. **Subagent-Driven (recommended)** — a fresh subagent per task, review between tasks, fast iteration. REQUIRED SUB-SKILL: `superpowers:subagent-driven-development`.
2. **Inline Execution** — execute the tasks in one session with checkpoints. REQUIRED SUB-SKILL: `superpowers:executing-plans`.

Which approach?
