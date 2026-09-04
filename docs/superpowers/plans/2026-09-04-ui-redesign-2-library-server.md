# Desktop UI redesign 2 — Library and Server views Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn Library and Server from flat grids into the redesign's two rail-plus-grid views — a filter rail with counts, a toolbar with search / sort / card size, one shared card component with hover actions and badges, and the Server platform header with its firmware and default-emulator chips.

**Architecture:** One `GameCard.svelte` + `CardGrid.svelte` pair serves both views, driven by pure helper modules (`lib/cards/*`, `lib/library/*`, `lib/server/*`) that carry every rule vitest can reach. Two small backend additions feed the new UI: a `last_played_at` column on the installed-games registry (registry `user_version` 4), stamped when a launch spawns, and two `ui.card_size_*` config fields. The Server platform header adds two read-mostly Tauri commands for platform firmware.

**Tech Stack:** Rust (grid-core `config` + `library::registry`, Tauri 2 `app` crate), Svelte 5 runes + TypeScript + vitest, WebdriverIO E2E against the mock RomM server.

**Spec:** `docs/superpowers/specs/2026-09-04-desktop-ui-redesign-design.md` — binding. This plan implements **delivery item 2 only** (§12.2): §5 Library view, §6 Server view, D-UI-2, D-UI-3, D-UI-7 (the grid half — grids may run to 1920px), D-UI-9 (card treatment and badges), plus the `Ctrl+F` half of §3 that plan 1 deferred and the §11 new ids `library-rail-<key>` and `server-rail-<id>`. Plans 3–5 (the Details popup layout, the Downloads segments and sparklines, the Emulators/Settings rails) are explicitly NOT implemented here.

**Deferred from D-UI-9, deliberately:** the card action row ships **Details, Cloud sync, More** — **no Favourite**. There is no favourite backend: nothing in `config.toml`, nothing in the registry, and the RomM favourite/collection API is listed out of scope by design §13. A Favourite button with no store behind it would be a lie in the UI, so it is left out and picked up whenever collections land. Every other D-UI-9 element ships here.

**Actions with no dedicated target yet, also deliberate:**
- **Cloud sync** on a card opens the existing Details popup with its cloud panel already expanded (a new `initialCloudMode` prop). The redesigned Saves tab is plan 3's; this plan does not build a second cloud surface.
- **More** on a card opens the Details popup unchanged (same as a card-body click). It exists so the action row matches D-UI-9's shape; plan 3 gives it a menu.
- The Server header's **default emulator chip** switches to the Emulators **view**. Selecting its "Platform defaults" sub-page is plan 5's work, so the chip links to the view only.

All paths below are relative to `rewrite/` unless they start with `docs/`.

## Global Constraints

- **Token secrecy (hard):** tokens live only in the OS keyring and the redacting in-memory type; never in files, logs, errors, IPC, or console output. Cover URLs go through the existing `ensure_image` path (`Image.svelte` → `api.ensureImage`), never a token-bearing URL built in the frontend.
- **Library rail is 220px.** Entries in order: **All games** (count), **Recent** (played in the last 30 days, count), **Updates** (count), then a `PLATFORMS` heading with each installed platform and its count. Test ids `library-rail-all`, `library-rail-recent`, `library-rail-updates`, `library-rail-platform-<slug>`. Selection persists per session (module-scoped state, so it survives a Shell remount and a view switch).
- **Library toolbar:** search `library-search` (title contains, case-insensitive), sort `library-sort` (Recently played, Recently installed, Title, Platform), card size `library-size` (small / medium / large → 120 / 160 / 200px minimum column).
- **Grid:** `repeat(auto-fill, minmax(<size>, 1fr))`; cover ratio comes from the image with a **3:4 fallback**; the title sits **under** the card, one line, ellipsis.
- **Library empty states, verbatim:** `No games installed`, `Nothing played in the last 30 days`, `Everything is up to date`, `No games installed for <platform>`.
- **Server rail:** the server's platforms with ROM counts. Each rail entry keeps its existing test id `platform-btn-<id>` (dozens of specs click it) **and** carries the §11 id as `data-rail="server-rail-<id>"` on the same element — a `data-testid` attribute holds one value, so the new id rides on a second attribute rather than displacing a working one.
- **Server platform header** `server-platform-header`: display name, ROM count, installed count, a firmware chip `server-firmware-chip` with an Install action when the server offers firmware, and a default-emulator chip `server-emulator-chip` that switches to the Emulators view.
- **Server grid:** `game-card-<id>` kept; installed dot, UPDATE tag, not-installed cards render at 60% opacity until hover; the hover primary action is **Install** for not-installed and **Play** for installed. Search `server-search` filters the loaded platform list client-side.
- **Cards:** hover scales 1.05 with a gradient overlay, a centred Play or Install button, and a bottom action row (Details, Cloud sync, More). Badges: **installed dot top-right**, **UPDATE tag top-left** (text exactly `UPDATE`), **cloud icon bottom-right** when cloud sync is configured for that game's platform, **platform chip bottom-left** (short platform name). Card size is remembered per view in `ui.card_size_library` / `ui.card_size_server`.
- **`Ctrl+F` focuses the active view's search box** (Library or Server), and is ignored while a dialog is open.
- **Views stay mounted** and switch with `hidden`, as plan 1 left them. **Grids may use the full width up to 1920px** (`max-width: 1920px; margin: 0 auto`) — they do NOT take `.view-content`, which caps at 1100px. Rail panes use only the tokens plan 1 defined in `app.css`; this plan adds no new colour.
- **Every task ends with**, from `rewrite/`: `cargo fmt`; `cargo clippy --workspace --all-targets -- -D warnings` and `cargo clippy -p app --all-targets --features e2e -- -D warnings` clean; `cargo test --workspace` green **when Rust changed**; and from `rewrite/app`: `npm run check` and `npx vitest run` green. Then a commit whose subject starts `rewrite: `. The final E2E task runs **every** group (`scripts/e2e.sh` with no argument) and must be green.
- **Never** run `git checkout`, `git restore`, `git reset`, or `git stash`. Commit with explicit pathspecs.
- **No component test harness exists** in this repo (no `@testing-library/svelte`, no jsdom). Every `.svelte` change is verified by an extracted, unit-tested pure module plus `npm run check` and E2E — never by a fabricated component test.

---

## File map

| File | Responsibility |
|---|---|
| `crates/grid-core/src/config.rs` | `UiSettings.card_size_library` / `card_size_server` |
| `crates/grid-core/src/library/registry.rs` | `last_played_at` column, `migrate_3_to_4`, `Registry::touch_last_played` |
| `crates/grid-core/tests/registry.rs` | v4 migration coverage; the seven `user_version` assertions move to 4 |
| `app/src-tauri/src/commands.rs` | `normalize_ui_settings` extension; `last_played_at` stamp in `launch_game`; `platform_firmware_status`, `install_firmware_for_platform` |
| `app/src-tauri/src/firmware_service.rs` | `FirmwareService::spawn_for_platform` (the platform half of `spawn_for_game`) |
| `app/src-tauri/src/lib.rs` | handler registration for the two new commands |
| `app/src/lib/api.ts` | `UiSettings` fields, `CardSizeName`, `PlatformFirmwareStatus`, two wrappers |
| `app/src/lib/stores/uiSettings.svelte.ts` | card size per view: getters + `setCardSize` |
| `app/src/lib/cards/size.ts` (+ test) | `CardSize`, `cardMinPx`, `gridTemplate`, `columnsOf` |
| `app/src/lib/cards/badges.ts` (+ test) | which badges a card shows; `shortPlatformName`; `UPDATE_TAG_TEXT` |
| `app/src/lib/library/rail.ts` (+ test) | rail entries, counts, per-entry filter, empty text |
| `app/src/lib/library/sort.ts` (+ test) | sort modes and `titleContains` |
| `app/src/lib/server/header.ts` (+ test) | platform header counts line and the two chip labels |
| `app/src/lib/GameCard.svelte` | the one card: cover, overlay, action row, badges |
| `app/src/lib/CardGrid.svelte` | the one grid: `auto-fill` template, 1920px cap, focus plumbing |
| `app/src/lib/Library.svelte` | rail + toolbar + grid + empty states + `focusSearch()` |
| `app/src/lib/Server.svelte` | rail + platform header + toolbar + grid + `focusSearch()` |
| `app/src/lib/Details.svelte` | new optional `initialCloudMode` prop |
| `app/src/lib/Shell.svelte` | `Ctrl+F` dispatch; `onOpenEmulators` wired into Server |
| `e2e/specs/*.spec.ts` | new-structure updates and the new rail/sort/size/header cases |
| `SPEC.md`, `rewrite/README.md`, `docs/porting/03-library-install.md` | documentation |

---

### Task 1: `last_played_at` and the two card-size config fields

**Files:**
- Modify: `crates/grid-core/src/config.rs:71-100` (the `UiSettings` struct and its `Default`)
- Modify: `crates/grid-core/src/library/registry.rs:12-53` (`SCHEMA_SQL`), `:55-57` (`LATEST_USER_VERSION`), `:120-140` (after `migrate_2_to_3`), `:141-153` (`SELECT_COLUMNS`), `:156-205` (the struct), `:206-245` (`from_row`), `:303-315` (the migration loop), `:435-455` (a new method beside `update_images`)
- Modify: `crates/grid-core/tests/registry.rs:69`, `:83`, `:258`, `:306`, `:363`, `:399`, `:440` (the `user_version` assertions), and append new tests at the file tail
- Modify: `app/src-tauri/src/commands.rs:338-348` (`normalize_ui_settings`), `:474-491` (`launch_game`'s tail)
- Modify: `app/src/lib/api.ts:16-17` (the `UiSettings` type), `:152-191` (`InstalledGame`)

**Interfaces:**
- Consumes: `grid_core::config::UiSettings` and `Config` (plan 1); `grid_core::library::registry::{InstalledGame, Registry}`; `grid_core::library::InstallService::registry()`.
- Produces, used by Tasks 2, 4, 5 and 6:
  - `UiSettings` gains `pub card_size_library: String` and `pub card_size_server: String`, both defaulting to `"medium"`, both normalized to one of `"small" | "medium" | "large"` by `normalize_ui_settings`.
  - `InstalledGame` gains `pub last_played_at: i64` (0 = never played), serialized to the frontend as `last_played_at: number`.
  - `pub fn Registry::touch_last_played(&self, rom_id: i64, at: i64) -> Result<bool, LibraryError>`.
  - Registry `LATEST_USER_VERSION` is `4`.
  - TS: `UiSettings` gains `card_size_library: CardSizeName; card_size_server: CardSizeName` where `export type CardSizeName = 'small' | 'medium' | 'large'`; `InstalledGame` gains `last_played_at: number`.

- [ ] **Step 1: Write the failing config test**

Append to the existing `#[cfg(test)] mod tests` block in `crates/grid-core/src/config.rs` (it already has `use super::*;`):

```rust
    #[test]
    fn card_sizes_default_to_medium_for_both_views() {
        let ui = UiSettings::default();
        assert_eq!(ui.card_size_library, "medium");
        assert_eq!(ui.card_size_server, "medium");
    }

    #[test]
    fn a_ui_table_written_before_the_card_sizes_existed_loads_the_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            "schema_version = 1\n\n[ui]\ntheme = \"dark\"\nbackground_fade = 40\n",
        )
        .unwrap();
        let loaded = Config::load(&path).unwrap();
        assert_eq!(loaded.ui.theme, "dark");
        assert_eq!(loaded.ui.background_fade, 40);
        assert_eq!(loaded.ui.card_size_library, "medium");
        assert_eq!(loaded.ui.card_size_server, "medium");
    }

    #[test]
    fn card_sizes_round_trip_through_save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let cfg = Config {
            ui: UiSettings {
                theme: "system".to_string(),
                background_fade: 25,
                card_size_library: "large".to_string(),
                card_size_server: "small".to_string(),
            },
            ..Default::default()
        };
        cfg.save(&path).unwrap();
        let loaded = Config::load(&path).unwrap();
        assert_eq!(loaded.ui.card_size_library, "large");
        assert_eq!(loaded.ui.card_size_server, "small");
    }
```

- [ ] **Step 2: Run it to verify it fails**

Run from `rewrite/`: `cargo test -p grid-core config::tests::card_sizes config::tests::a_ui_table_written`
Expected: FAIL to compile — `struct 'UiSettings' has no field named 'card_size_library'`.

- [ ] **Step 3: Add the two fields**

In `crates/grid-core/src/config.rs`, add these two fields to `UiSettings` directly after `pub background_fade: u8,`:

```rust
    /// Library grid card size: `"small"`, `"medium"` or `"large"`
    /// (design §5, D-UI-9 "remembered per view"). Stored as a plain string
    /// for the same forward-compatibility reason as `theme`: an unknown
    /// value written by a newer build round-trips instead of failing the
    /// whole config load, and both the app layer and the frontend
    /// normalize it.
    #[serde(default = "default_card_size")]
    pub card_size_library: String,
    /// Server grid card size. Independent of `card_size_library`: the two
    /// grids are browsed differently and D-UI-9 remembers them per view.
    #[serde(default = "default_card_size")]
    pub card_size_server: String,
```

Add the default function beside `default_background_fade`:

```rust
fn default_card_size() -> String {
    "medium".to_string()
}
```

and extend the `Default` impl's body to:

```rust
        Self {
            theme: default_theme(),
            background_fade: default_background_fade(),
            card_size_library: default_card_size(),
            card_size_server: default_card_size(),
        }
```

- [ ] **Step 4: Run the config tests**

Run from `rewrite/`: `cargo test -p grid-core config::`
Expected: PASS, including the three new tests and every pre-existing `config::tests` case.

- [ ] **Step 5: Write the failing registry tests**

Append to `crates/grid-core/tests/registry.rs`:

```rust
/// The v3 schema: v2 plus the twelve native/PS3/PS4/RA columns. Built the
/// same way `v2_schema()` builds v2 — from the previous schema plus the
/// `ALTER`s that migration performs — so the fixture can never drift from
/// what the migration actually produces.
fn v3_schema() -> String {
    let mut sql = v2_schema();
    for column in V3_COLUMN_NAMES {
        sql.push_str(&format!(
            "\n        ALTER TABLE installed_games ADD COLUMN {column} TEXT NOT NULL DEFAULT '';"
        ));
    }
    sql
}

#[test]
fn fresh_db_is_v4_and_has_last_played_at() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("grid-launcher.db");
    let registry = Registry::open(&path).unwrap();

    let conn = Connection::open(&path).unwrap();
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, 4);
    let columns = table_columns(&conn);
    assert!(
        columns.iter().any(|c| c == "last_played_at"),
        "fresh schema is missing last_played_at: {columns:?}"
    );

    registry.upsert(&sample("Chrono Trigger", "SNES")).unwrap();
    let rows = registry.all().unwrap();
    assert_eq!(rows[0].last_played_at, 0, "a fresh install has never played");
}

#[test]
fn migrates_v3_to_v4_keeping_rows_and_defaulting_last_played_to_zero() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("grid-launcher.db");
    {
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(&v3_schema()).unwrap();
        conn.execute(
            "INSERT INTO installed_games (title, platform, title_key, platform_key, rom_id, installed_at)
             VALUES ('Three', 'SNES', 'three', 'snes', 7, 1)",
            [],
        )
        .unwrap();
        conn.pragma_update(None, "user_version", 3).unwrap();
    }

    let registry = Registry::open(&path).unwrap();
    let conn = Connection::open(&path).unwrap();
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, 4);

    let rows = registry.all().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].title, "Three");
    assert_eq!(rows[0].last_played_at, 0);
}

#[test]
fn v3_to_v4_migration_is_idempotent_when_the_column_preexists() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("grid-launcher.db");
    {
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(&v3_schema()).unwrap();
        conn.execute_batch(
            "ALTER TABLE installed_games ADD COLUMN last_played_at INTEGER NOT NULL DEFAULT 0;",
        )
        .unwrap();
        conn.pragma_update(None, "user_version", 3).unwrap();
    }

    let registry = Registry::open(&path).unwrap();
    let conn = Connection::open(&path).unwrap();
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, 4);
    assert!(registry.all().unwrap().is_empty());
}

#[test]
fn touch_last_played_stamps_only_the_matching_rom() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("grid-launcher.db");
    let registry = Registry::open(&path).unwrap();

    let mut other = sample("Pac-Man", "Arcade");
    other.rom_id = Some(201);
    registry.upsert(&sample("Chrono Trigger", "SNES")).unwrap(); // rom_id 42
    registry.upsert(&other).unwrap();

    assert!(registry.touch_last_played(42, 1_800_000_000).unwrap());

    let rows = registry.all().unwrap();
    let ct = rows.iter().find(|r| r.rom_id == Some(42)).unwrap();
    let pac = rows.iter().find(|r| r.rom_id == Some(201)).unwrap();
    assert_eq!(ct.last_played_at, 1_800_000_000);
    assert_eq!(pac.last_played_at, 0);
}

#[test]
fn touch_last_played_reports_false_for_an_unknown_rom() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("grid-launcher.db");
    let registry = Registry::open(&path).unwrap();
    assert!(!registry.touch_last_played(999, 1_800_000_000).unwrap());
}

#[test]
fn reinstalling_a_game_does_not_reset_its_last_played_stamp() {
    // The Library's "Recent" rail entry and its "Recently played" sort both
    // read this column, and an update or a reinstall runs `upsert` again.
    // `upsert` must therefore leave `last_played_at` alone.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("grid-launcher.db");
    let registry = Registry::open(&path).unwrap();

    registry.upsert(&sample("Chrono Trigger", "SNES")).unwrap();
    registry.touch_last_played(42, 1_800_000_000).unwrap();

    let mut again = sample("Chrono Trigger", "SNES");
    again.installed_at = 1_900_000_000;
    registry.upsert(&again).unwrap();

    let rows = registry.all().unwrap();
    assert_eq!(rows[0].installed_at, 1_900_000_000);
    assert_eq!(rows[0].last_played_at, 1_800_000_000);
}
```

- [ ] **Step 6: Run them to verify they fail**

Run from `rewrite/`: `cargo test -p grid-core --test registry`
Expected: FAIL to compile — `no method named 'touch_last_played'` and `no field 'last_played_at' on type 'InstalledGame'`.

- [ ] **Step 7: Add the column, the migration and the setter**

In `crates/grid-core/src/library/registry.rs`:

Add the column to `SCHEMA_SQL`, directly after the `installed_at        INTEGER NOT NULL,` line and before `UNIQUE (title_key, platform_key)`:

```sql
    last_played_at      INTEGER NOT NULL DEFAULT 0,
```

Bump the version constant:

```rust
/// The schema version this build understands. Bumped when a migration adds
/// columns (see spec: later milestones add native/PS3/PS4 fields).
const LATEST_USER_VERSION: i64 = 4;
```

Add the v4 column list beside `V3_COLUMNS`:

```rust
/// The column v3 -> v4 (the redesign's Library rail) adds. An INTEGER, not
/// a TEXT like every earlier migration's columns, so it gets its own
/// `ADD COLUMN` type rather than joining a loop over string columns.
const V4_COLUMN: &str = "last_played_at";
```

Add the migration directly after `migrate_2_to_3`:

```rust
/// v3 -> v4 (desktop UI redesign 2): adds `last_played_at`, the epoch
/// seconds of the last launch, `0` for a game never launched through GRID.
/// The Library rail's "Recent" entry and the "Recently played" sort are its
/// only readers; nothing else in the app depends on it, so a database that
/// cannot be migrated would be a far worse outcome than a column of zeroes.
///
/// Same transaction + idempotent-`ADD COLUMN` shape as [`migrate_1_to_2`]
/// and [`migrate_2_to_3`], for the same reasons.
fn migrate_3_to_4(conn: &mut Connection) -> Result<(), LibraryError> {
    let tx = conn.transaction().map_err(registry_err)?;
    let existing = installed_games_columns(&tx)?;
    if !existing.iter().any(|name| name == V4_COLUMN) {
        tx.execute_batch(&format!(
            "ALTER TABLE installed_games ADD COLUMN {V4_COLUMN} INTEGER NOT NULL DEFAULT 0;"
        ))
        .map_err(registry_err)?;
    }
    tx.pragma_update(None, "user_version", 4)
        .map_err(registry_err)?;
    tx.commit().map_err(registry_err)
}
```

Register it in `Registry::open`'s migration loop, after the `2 => migrate_2_to_3(&mut conn)?,` arm:

```rust
                3 => migrate_3_to_4(&mut conn)?,
```

Append the column to `SELECT_COLUMNS` (it is the last entry, so extend the final segment):

```rust
const SELECT_COLUMNS: &str = "title, platform, rom_id, rom_file_name, archive_path, \
     extracted_path, extracted_dir, multi_file_game_dir, description, rating, genres, \
     regions, languages, tags, revision, companies, first_release_date, filesize_bytes, \
     server_updated_at, installed_at, cover_small_path, cover_large_path, screenshot_urls, \
     native_executable_path, native_launch_parameters, native_compat_tool, native_wineprefix, \
     native_game_dir, included_dlc, ps3_trophy_paths, ps3_game_id, ps3_iso_path, ps4_game_id, \
     ps4_content, ra_id, last_played_at";
```

Add the field to `InstalledGame`, after `pub ra_id: String,`:

```rust
    /// Epoch seconds of the last launch, `0` when the game has never been
    /// launched through GRID. Written ONLY by
    /// [`Registry::touch_last_played`] — never by [`Registry::upsert`], so
    /// an update or a reinstall keeps the history the Library rail reads.
    #[serde(default)]
    pub last_played_at: i64,
```

and to `from_row`, after `ra_id: row.get(34)?,`:

```rust
            last_played_at: row.get(35)?,
```

Add the setter directly after `update_images`:

```rust
    /// Stamps `last_played_at` on the row for `rom_id`. Returns whether a
    /// row matched — a launch of something not in the registry (there is no
    /// such path today, but `launch_game` does not require one) stamps
    /// nothing and reports `false`.
    pub fn touch_last_played(&self, rom_id: i64, at: i64) -> Result<bool, LibraryError> {
        let conn = self.conn.lock().unwrap();
        let affected = conn
            .execute(
                "UPDATE installed_games SET last_played_at = ?1 WHERE rom_id = ?2",
                params![at, rom_id],
            )
            .map_err(registry_err)?;
        Ok(affected > 0)
    }
```

`upsert` is deliberately left untouched: `last_played_at` is not in its column list, so an insert takes the `DEFAULT 0` and an `ON CONFLICT` update never mentions it.

- [ ] **Step 8: Update the seven existing `user_version` assertions**

In `crates/grid-core/tests/registry.rs`, change `assert_eq!(version, 3);` to `assert_eq!(version, 4);` at lines **69, 83, 258, 306, 363, 399** and **440**. These assert "the database is fully migrated", not "the database is v3"; the schema fixtures and column lists in those tests stay as they are, because v1/v2/v3 remain valid starting points. Rename the two tests whose names carry the version:
- `open_creates_file_and_sets_user_version_3` → `open_creates_file_and_sets_user_version_4`
- `fresh_db_is_v3_and_has_the_twelve_columns` → `fresh_db_is_v4_and_has_the_twelve_columns`
- `migrates_v1_to_v3_transactionally` → `migrates_v1_to_v4_transactionally`
- `migrates_v2_to_v3` → `migrates_v2_to_v4`

- [ ] **Step 9: Run the registry tests**

Run from `rewrite/`: `cargo test -p grid-core --test registry`
Expected: PASS — the six new tests plus every renamed/retargeted existing one.

- [ ] **Step 10: Write the failing app-layer test**

Append to the existing `mod ui_settings_tests` block at the tail of `app/src-tauri/src/commands.rs`:

```rust
    #[test]
    fn normalize_ui_settings_clamps_both_card_sizes_to_the_three_names() {
        for (raw, expected) in [
            ("small", "small"),
            ("medium", "medium"),
            ("large", "large"),
            ("  large  ", "large"),
            ("Large", "medium"),
            ("enormous", "medium"),
            ("", "medium"),
        ] {
            let out = normalize_ui_settings(UiSettings {
                theme: "system".to_string(),
                background_fade: 25,
                card_size_library: raw.to_string(),
                card_size_server: raw.to_string(),
            });
            assert_eq!(out.card_size_library, expected, "library size for {raw:?}");
            assert_eq!(out.card_size_server, expected, "server size for {raw:?}");
        }
    }
```

- [ ] **Step 11: Run it to verify it fails**

Run from `rewrite/`: `cargo test -p app normalize_ui_settings_clamps_both_card_sizes`
Expected: FAIL to compile — `missing fields 'card_size_library' and 'card_size_server'`.

- [ ] **Step 12: Extend `normalize_ui_settings` and stamp the launch**

In `app/src-tauri/src/commands.rs`, replace the body of `normalize_ui_settings` with:

```rust
pub fn normalize_ui_settings(settings: UiSettings) -> UiSettings {
    let theme = match settings.theme.trim() {
        "dark" => "dark",
        "light" => "light",
        _ => "system",
    };
    UiSettings {
        theme: theme.to_string(),
        background_fade: settings.background_fade.min(MAX_BACKGROUND_FADE),
        card_size_library: normalize_card_size(&settings.card_size_library),
        card_size_server: normalize_card_size(&settings.card_size_server),
    }
}

/// One of `"small"`, `"medium"`, `"large"`; anything else becomes
/// `"medium"` (design §5's default). Case-sensitive on purpose: the three
/// names are written by this app, and a `"Large"` in `config.toml` is a
/// hand edit whose intent is not worth guessing at.
fn normalize_card_size(raw: &str) -> String {
    match raw.trim() {
        "small" => "small",
        "large" => "large",
        _ => "medium",
    }
    .to_string()
}
```

In the same file, extend `launch_game`: after the existing `stamp_session_started` block and before `Ok(session)`, insert

```rust
    // The Library rail's "Recent" entry and its "Recently played" sort read
    // `last_played_at` (design §5). Stamped once the process has actually
    // spawned, so a launch that failed to start never counts as played. A
    // registry write failure is swallowed: the game IS running, and losing
    // one ordering hint must never surface as a launch error.
    {
        let registry = install.registry();
        let at = session.started_at as i64;
        if let Err(e) =
            tokio::task::spawn_blocking(move || registry.touch_last_played(rom_id, at)).await
        {
            tracing::debug!("last_played_at stamp did not finish for rom {rom_id}: {e}");
        }
    }
```

`install` is still owned here — the `stamp_session_started` call above consumes a clone (`install` is moved into it), so change that call site's argument from `install` to `install.clone()` and `launch` to `launch.clone()` if the compiler reports a move error.

- [ ] **Step 13: Run the app tests**

Run from `rewrite/`: `cargo test -p app`
Expected: PASS.

- [ ] **Step 14: Mirror both shapes in TypeScript**

In `app/src/lib/api.ts`, replace the `UiSettings` type (line 16-17) with:

```ts
/** The three card sizes design §5 offers, in `ui.card_size_*`. */
export type CardSizeName = 'small' | 'medium' | 'large';

/** Desktop shell appearance, mirroring `grid_core::config::UiSettings`. */
export type UiSettings = {
  theme: 'system' | 'dark' | 'light';
  background_fade: number;
  card_size_library: CardSizeName;
  card_size_server: CardSizeName;
};
```

and add to `InstalledGame`, after `ra_id: string;`:

```ts
  /** Epoch seconds of the last launch; 0 when never launched through GRID. */
  last_played_at: number;
```

- [ ] **Step 15: Fix the two `setUiSettings` call sites the new fields break**

In `app/src/lib/stores/uiSettings.svelte.ts`, the two `api.setUiSettings({ theme, background_fade })` calls (lines 118 and 130) now miss two required fields. Add the card sizes to the store's state and send them. Replace the `state` declaration (lines 17-21) with:

```ts
const state = $state<{
  theme: ThemeChoice;
  backgroundFade: number;
  prefersDark: boolean;
  cardSizeLibrary: CardSize;
  cardSizeServer: CardSize;
}>({
  theme: 'system',
  backgroundFade: FADE_DEFAULT,
  prefersDark: false,
  cardSizeLibrary: 'medium',
  cardSizeServer: 'medium',
});

/** The one place the whole `UiSettings` payload is assembled, so no writer
 *  can drop a field another writer owns. */
function payload(): UiSettings {
  return {
    theme: state.theme,
    background_fade: state.backgroundFade,
    card_size_library: state.cardSizeLibrary,
    card_size_server: state.cardSizeServer,
  };
}
```

add `import type { UiSettings } from '../api';` beside the existing `import { api } from '../api';`, add `import { normalizeCardSize, type CardSize } from '../cards/size';` — **this import comes from Task 2, so Task 2 must land before `npm run check` passes; do the import in Task 2 instead and here just inline `'medium'` as above**. In `initUiSettings`, after `state.backgroundFade = clampFade(stored.background_fade);`, add:

```ts
    state.cardSizeLibrary = stored.card_size_library;
    state.cardSizeServer = stored.card_size_server;
```

and replace both `api.setUiSettings({...})` calls with `api.setUiSettings(payload())`.

- [ ] **Step 16: Run the frontend checks**

Run from `rewrite/app`: `npm run check && npx vitest run`
Expected: PASS. `uiSettings.test.ts` mocks `api.setUiSettings`; if it asserts the exact payload object, extend those expectations with `card_size_library: 'medium', card_size_server: 'medium'`.

- [ ] **Step 17: Format, lint and commit**

```bash
cd rewrite
cargo fmt
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p app --all-targets --features e2e -- -D warnings
cargo test --workspace
cd app && npm run check && npx vitest run && cd ..
git add crates/grid-core/src/config.rs crates/grid-core/src/library/registry.rs \
        crates/grid-core/tests/registry.rs app/src-tauri/src/commands.rs \
        app/src/lib/api.ts app/src/lib/stores/uiSettings.svelte.ts
git commit -m "rewrite: record last_played_at and the per-view card size"
```

---

### Task 2: The pure helpers behind the rails, sorting, sizing and badges

**Files:**
- Create: `app/src/lib/cards/size.ts`, `app/src/lib/cards/size.test.ts`
- Create: `app/src/lib/cards/badges.ts`, `app/src/lib/cards/badges.test.ts`
- Create: `app/src/lib/library/rail.ts`, `app/src/lib/library/rail.test.ts`
- Create: `app/src/lib/library/sort.ts`, `app/src/lib/library/sort.test.ts`
- Modify: `app/src/lib/stores/uiSettings.svelte.ts` (card-size getters and `setCardSize`)

**Interfaces:**
- Consumes: `InstalledGame`, `CardSizeName` from `api.ts` (Task 1); `isHiddenLibraryPlatform` from `lib/library.ts:10`; `uiSettings` from `stores/uiSettings.svelte.ts`.
- Produces, used by Tasks 3, 4, 5 and 6:
  - `lib/cards/size.ts`: `export const CARD_SIZES: readonly ['small','medium','large']`, `export type CardSize = 'small' | 'medium' | 'large'`, `export function normalizeCardSize(raw: string): CardSize`, `export function cardMinPx(size: CardSize): number`, `export function gridTemplate(size: CardSize): string`, `export function cardSizeLabel(size: CardSize): string`, `export function columnsOf(grid: HTMLElement | null): number`.
  - `lib/cards/badges.ts`: `export const UPDATE_TAG_TEXT = 'UPDATE'`, `export type CardBadges = { installed: boolean; update: boolean; cloud: boolean; platform: string }`, `export function cardBadges(input: BadgeInput): CardBadges` with `export type BadgeInput = { platform: string; installed: boolean; updateLabel: string | null; cloudPlatforms: ReadonlySet<string> }`, `export function shortPlatformName(name: string): string`, `export function cloudPlatformSet(defaultEmulators: Record<string, string>): Set<string>`.
  - `lib/library/rail.ts`: `export type RailKey`, `export type RailEntry = { key: RailKey; testId: string; label: string; count: number }`, `export const RECENT_WINDOW_SECONDS = 2_592_000`, `export function platformSlug(platform: string): string`, `export function railEntries(rows: InstalledGame[], updateRomIds: ReadonlySet<number>, nowSeconds: number): RailEntry[]`, `export function matchesRail(row: InstalledGame, key: RailKey, updateRomIds: ReadonlySet<number>, nowSeconds: number): boolean`, `export function emptyText(entry: RailEntry): string`, `export function entryForKey(entries: RailEntry[], key: RailKey): RailEntry`.
  - `lib/library/sort.ts`: `export const LIBRARY_SORTS: readonly ['played','installed','title','platform']`, `export type LibrarySort`, `export function normalizeSort(raw: string): LibrarySort`, `export function sortLabel(sort: LibrarySort): string`, `export function sortGames(rows: InstalledGame[], sort: LibrarySort): InstalledGame[]`, `export function titleContains(title: string, query: string): boolean`.
  - `stores/uiSettings.svelte.ts`: `uiSettings.cardSizeLibrary: CardSize`, `uiSettings.cardSizeServer: CardSize`, `export async function setCardSize(view: 'library' | 'server', size: CardSize): Promise<void>`.

- [ ] **Step 1: Write the failing size test**

Create `app/src/lib/cards/size.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import {
  CARD_SIZES,
  cardMinPx,
  cardSizeLabel,
  columnsOf,
  gridTemplate,
  normalizeCardSize,
} from './size';

describe('normalizeCardSize', () => {
  it('accepts the three stored spellings', () => {
    expect(normalizeCardSize('small')).toBe('small');
    expect(normalizeCardSize('medium')).toBe('medium');
    expect(normalizeCardSize('large')).toBe('large');
  });
  it('trims, because config.toml can be hand-edited', () => {
    expect(normalizeCardSize('  large  ')).toBe('large');
  });
  it('falls back to medium for anything else, case included', () => {
    expect(normalizeCardSize('Large')).toBe('medium');
    expect(normalizeCardSize('')).toBe('medium');
    expect(normalizeCardSize('enormous')).toBe('medium');
  });
});

describe('cardMinPx / gridTemplate', () => {
  it('holds design section 5s three minimum column widths', () => {
    expect(cardMinPx('small')).toBe(120);
    expect(cardMinPx('medium')).toBe(160);
    expect(cardMinPx('large')).toBe(200);
  });
  it('builds the auto-fill template design section 5 specifies', () => {
    expect(gridTemplate('medium')).toBe('repeat(auto-fill, minmax(160px, 1fr))');
  });
  it('labels each size for the toolbar control', () => {
    expect(CARD_SIZES.map(cardSizeLabel)).toEqual(['Small', 'Medium', 'Large']);
  });
});

describe('columnsOf', () => {
  it('is 1 for no element, so keyboard navigation degrades to a list', () => {
    expect(columnsOf(null)).toBe(1);
  });
  it('counts the tracks the browser resolved for an auto-fill grid', () => {
    // getComputedStyle resolves `repeat(auto-fill, ...)` to concrete track
    // sizes, so the column count is the number of space-separated entries.
    const fake = {
      ownerDocument: {
        defaultView: {
          getComputedStyle: () => ({ gridTemplateColumns: '188px 188px 188px 188px' }),
        },
      },
    } as unknown as HTMLElement;
    expect(columnsOf(fake)).toBe(4);
  });
  it('is 1 when the grid has not been laid out yet', () => {
    const fake = {
      ownerDocument: {
        defaultView: { getComputedStyle: () => ({ gridTemplateColumns: 'none' }) },
      },
    } as unknown as HTMLElement;
    expect(columnsOf(fake)).toBe(1);
  });
});
```

- [ ] **Step 2: Run it to verify it fails**

Run from `rewrite/app`: `npx vitest run src/lib/cards/size.test.ts`
Expected: FAIL — `Failed to resolve import "./size"`.

- [ ] **Step 3: Write `size.ts`**

Create `app/src/lib/cards/size.ts`:

```ts
// Card sizing for both grids (design §5, D-UI-9). Pure: the store owns the
// persisted value, the components own the markup, this module owns the
// three numbers and the template string they turn into.
import type { CardSizeName } from '../api';

export const CARD_SIZES = ['small', 'medium', 'large'] as const;
export type CardSize = CardSizeName;

/** Design §5: "sizes 120 / 160 / 200px" — the grid's minimum column width. */
const MIN_PX: Record<CardSize, number> = { small: 120, medium: 160, large: 200 };
const LABELS: Record<CardSize, string> = { small: 'Small', medium: 'Medium', large: 'Large' };

/**
 * The stored value, or `medium` for anything unrecognized. Matches the
 * Rust `normalize_card_size` exactly, including its case sensitivity, so
 * the two normalizers can never disagree about a config value.
 */
export function normalizeCardSize(raw: string): CardSize {
  const trimmed = raw.trim();
  return (CARD_SIZES as readonly string[]).includes(trimmed) ? (trimmed as CardSize) : 'medium';
}

export function cardMinPx(size: CardSize): number {
  return MIN_PX[size];
}

export function gridTemplate(size: CardSize): string {
  return `repeat(auto-fill, minmax(${MIN_PX[size]}px, 1fr))`;
}

export function cardSizeLabel(size: CardSize): string {
  return LABELS[size];
}

/**
 * How many columns the browser actually laid out. `auto-fill` means the
 * count depends on the window width, so keyboard focus movement cannot use
 * a constant the way the pre-redesign grids did: it reads the resolved
 * `grid-template-columns`, which is a space-separated list of concrete
 * track sizes once layout has run.
 *
 * Returns 1 — a single-column list — when there is no element, no view to
 * compute styles from, or no layout yet (`none`). One is the safe floor:
 * `moveFocus` treats every card as its own row, which navigates correctly
 * even if it navigates slowly.
 */
export function columnsOf(grid: HTMLElement | null): number {
  const view = grid?.ownerDocument?.defaultView;
  if (!grid || !view) return 1;
  const template = view.getComputedStyle(grid).gridTemplateColumns;
  if (!template || template === 'none') return 1;
  const tracks = template.trim().split(/\s+/).filter((t) => t.length > 0);
  return Math.max(1, tracks.length);
}
```

- [ ] **Step 4: Run it to verify it passes**

Run from `rewrite/app`: `npx vitest run src/lib/cards/size.test.ts`
Expected: PASS (11 assertions across 3 describes).

- [ ] **Step 5: Write the failing badges test**

Create `app/src/lib/cards/badges.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import { cardBadges, cloudPlatformSet, shortPlatformName, UPDATE_TAG_TEXT } from './badges';

describe('cardBadges (D-UI-9)', () => {
  const cloud = new Set(['SNES']);

  it('shows the installed dot and the platform chip for a plain installed game', () => {
    expect(
      cardBadges({ platform: 'SNES', installed: true, updateLabel: null, cloudPlatforms: cloud }),
    ).toEqual({ installed: true, update: false, cloud: true, platform: 'SNES' });
  });

  it('shows the UPDATE tag only when the updates store has a label for the rom', () => {
    expect(
      cardBadges({
        platform: 'SNES',
        installed: true,
        updateLabel: 'Update to v1.1.0',
        cloudPlatforms: cloud,
      }).update,
    ).toBe(true);
  });

  it('never shows the UPDATE tag for a game that is not installed', () => {
    // The server-side update set only ever covers installed rows, but the
    // Server grid renders both, and a tag on an uninstalled card would read
    // as "your copy is stale" about a copy that does not exist.
    expect(
      cardBadges({
        platform: 'SNES',
        installed: false,
        updateLabel: 'Update',
        cloudPlatforms: cloud,
      }).update,
    ).toBe(false);
  });

  it('drops the cloud icon for a platform with no cloud sync configured', () => {
    expect(
      cardBadges({ platform: 'Arcade', installed: true, updateLabel: null, cloudPlatforms: cloud }).cloud,
    ).toBe(false);
  });

  it('matches the cloud platform case- and space-insensitively', () => {
    expect(
      cardBadges({
        platform: '  snes ',
        installed: true,
        updateLabel: null,
        cloudPlatforms: cloud,
      }).cloud,
    ).toBe(true);
  });

  it('keeps the tag text fixed at UPDATE', () => {
    expect(UPDATE_TAG_TEXT).toBe('UPDATE');
  });
});

describe('shortPlatformName', () => {
  it('leaves a name that already fits alone', () => {
    expect(shortPlatformName('Arcade')).toBe('Arcade');
    expect(shortPlatformName('Nintendo 64')).toBe('Nintendo 64');
  });
  it('initialises a long name, keeping digit runs whole', () => {
    expect(shortPlatformName('Super Nintendo Entertainment System')).toBe('SNES');
    expect(shortPlatformName('PlayStation 3')).toBe('PS3');
    expect(shortPlatformName('Microsoft Xbox 360')).toBe('MX360');
  });
  it('falls back to a truncation when there is nothing to initialise', () => {
    expect(shortPlatformName('a very long lowercase platform')).toBe('a very long…');
  });
  it('is blank for a blank name rather than an ellipsis', () => {
    expect(shortPlatformName('   ')).toBe('');
  });
});

describe('cloudPlatformSet', () => {
  it('keeps only platforms that actually name a default emulator', () => {
    const set = cloudPlatformSet({ SNES: 'RetroArch', Arcade: '', 'PlayStation 3': 'RPCS3' });
    expect([...set].sort()).toEqual(['playstation 3', 'snes']);
  });
});
```

- [ ] **Step 6: Run it to verify it fails**

Run from `rewrite/app`: `npx vitest run src/lib/cards/badges.test.ts`
Expected: FAIL — `Failed to resolve import "./badges"`.

- [ ] **Step 7: Write `badges.ts`**

Create `app/src/lib/cards/badges.ts`:

```ts
// Which badges a card shows (D-UI-9): installed dot top-right, UPDATE tag
// top-left, cloud icon bottom-right, platform chip bottom-left. Pure so the
// rules are tested once and both grids obey the same ones.

/** The UPDATE tag's text. Fixed here so the two grids cannot disagree. */
export const UPDATE_TAG_TEXT = 'UPDATE';

/** Beyond this many characters a platform name is initialised for the chip. */
const CHIP_MAX_CHARS = 12;

export type BadgeInput = {
  platform: string;
  installed: boolean;
  updateLabel: string | null;
  cloudPlatforms: ReadonlySet<string>;
};

export type CardBadges = {
  installed: boolean;
  update: boolean;
  cloud: boolean;
  platform: string;
};

const key = (value: string) => value.trim().toLowerCase();

export function cardBadges(input: BadgeInput): CardBadges {
  return {
    installed: input.installed,
    // An update tag is a statement about the copy on disk, so it needs one.
    update: input.installed && input.updateLabel !== null,
    cloud: input.cloudPlatforms.has(key(input.platform)),
    platform: shortPlatformName(input.platform),
  };
}

/**
 * The platforms whose cloud sync is configured, keyed for
 * [`cardBadges`]'s lookup.
 *
 * "Configured" is read as "the platform has a default emulator", from
 * `api.getLaunchDefaults().default_emulators`. That is the signal a whole
 * grid can afford: the exact per-game answer is `cloud_panel_info`, one IPC
 * round trip per game, which a 200-card grid cannot pay. It is a sound
 * approximation in the direction that matters — cloud sync resolves its
 * save paths through the platform's emulator entry, so no default emulator
 * means cloud sync is definitely not configured, and the badge is a
 * pointer to the Details cloud panel, which still gives the precise answer.
 */
export function cloudPlatformSet(defaultEmulators: Record<string, string>): Set<string> {
  const set = new Set<string>();
  for (const [platform, emulator] of Object.entries(defaultEmulators)) {
    if (emulator.trim() === '') continue;
    set.add(key(platform));
  }
  return set;
}

/**
 * The platform chip's text. Short names pass through; a long one is
 * initialised by keeping every uppercase letter and every whole digit run
 * ("Super Nintendo Entertainment System" → "SNES", "PlayStation 3" → "PS3").
 * A long name with nothing to initialise (all lowercase) is truncated with
 * an ellipsis instead, so the chip never renders a single stray letter.
 */
export function shortPlatformName(name: string): string {
  const trimmed = name.trim();
  if (trimmed.length <= CHIP_MAX_CHARS) return trimmed;

  let initials = '';
  for (let i = 0; i < trimmed.length; i += 1) {
    const ch = trimmed[i];
    if (ch >= '0' && ch <= '9') {
      // A digit run is one token: "360" must not become "3".
      while (i < trimmed.length && trimmed[i] >= '0' && trimmed[i] <= '9') {
        initials += trimmed[i];
        i += 1;
      }
      i -= 1;
    } else if (ch >= 'A' && ch <= 'Z') {
      initials += ch;
    }
  }
  if (initials.length >= 2) return initials;
  return `${trimmed.slice(0, CHIP_MAX_CHARS - 1).trimEnd()}…`;
}
```

- [ ] **Step 8: Run it to verify it passes**

Run from `rewrite/app`: `npx vitest run src/lib/cards/badges.test.ts`
Expected: PASS.

- [ ] **Step 9: Write the failing rail test**

Create `app/src/lib/library/rail.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import type { InstalledGame } from '../api';
import {
  emptyText,
  entryForKey,
  matchesRail,
  platformSlug,
  railEntries,
  RECENT_WINDOW_SECONDS,
} from './rail';

const NOW = 1_800_000_000;

const row = (overrides: Partial<InstalledGame>): InstalledGame => ({
  title: 'Game', platform: 'SNES', rom_id: 1, rom_file_name: '', archive_path: '',
  extracted_path: '', extracted_dir: '', multi_file_game_dir: '', description: '', rating: '',
  genres: '', regions: '', languages: '', tags: '', revision: '', companies: '',
  first_release_date: '', filesize_bytes: 0, server_updated_at: '', installed_at: 0,
  cover_small_path: '', cover_large_path: '', screenshot_urls: '', native_executable_path: '',
  native_launch_parameters: '', native_compat_tool: '', native_wineprefix: '',
  native_game_dir: '', included_dlc: '', ps3_trophy_paths: '', ps3_game_id: '',
  ps3_iso_path: '', ps4_game_id: '', ps4_content: '', ra_id: '', last_played_at: 0,
  ...overrides,
});

describe('platformSlug', () => {
  it('lowercases, trims and joins runs of punctuation with a single dash', () => {
    expect(platformSlug('Super Nintendo Entertainment System')).toBe(
      'super-nintendo-entertainment-system',
    );
    expect(platformSlug('  PlayStation 3 ')).toBe('playstation-3');
    expect(platformSlug('Sega CD / Mega-CD')).toBe('sega-cd-mega-cd');
  });
  it('never yields a leading or trailing dash', () => {
    expect(platformSlug('!Arcade!')).toBe('arcade');
  });
});

describe('railEntries (design section 5)', () => {
  const rows = [
    row({ rom_id: 1, platform: 'SNES', last_played_at: NOW - 100 }),
    row({ rom_id: 2, platform: 'SNES', last_played_at: 0 }),
    row({ rom_id: 3, platform: 'Arcade', last_played_at: NOW - RECENT_WINDOW_SECONDS - 1 }),
    row({ rom_id: 4, platform: 'Emulators', last_played_at: NOW }),
  ];
  const updates = new Set([2, 3]);

  it('lists All games, Recent, Updates, then platforms sorted by name', () => {
    expect(railEntries(rows, updates, NOW).map((e) => e.key)).toEqual([
      'all',
      'recent',
      'updates',
      'platform:arcade',
      'platform:snes',
    ]);
  });

  it('hides the synthetic Emulators platform from the rail and every count', () => {
    const entries = railEntries(rows, updates, NOW);
    expect(entries.map((e) => e.key)).not.toContain('platform:emulators');
    expect(entryForKey(entries, 'all').count).toBe(3);
  });

  it('counts Recent as played inside the 30-day window, never a zero stamp', () => {
    expect(entryForKey(railEntries(rows, updates, NOW), 'recent').count).toBe(1);
  });

  it('counts Updates only for rows the update set names', () => {
    expect(entryForKey(railEntries(rows, updates, NOW), 'updates').count).toBe(2);
  });

  it('counts and labels each platform', () => {
    const snes = entryForKey(railEntries(rows, updates, NOW), 'platform:snes');
    expect(snes.label).toBe('SNES');
    expect(snes.count).toBe(2);
    expect(snes.testId).toBe('library-rail-platform-snes');
  });

  it('gives the three fixed entries their design section 11 ids', () => {
    expect(railEntries(rows, updates, NOW).slice(0, 3).map((e) => e.testId)).toEqual([
      'library-rail-all',
      'library-rail-recent',
      'library-rail-updates',
    ]);
  });

  it('still lists the three fixed entries, at zero, for an empty library', () => {
    expect(railEntries([], new Set(), NOW).map((e) => e.count)).toEqual([0, 0, 0]);
  });
});

describe('matchesRail', () => {
  const played = row({ rom_id: 1, platform: 'SNES', last_played_at: NOW - 10 });
  const stale = row({ rom_id: 2, platform: 'Arcade', last_played_at: 0 });
  const updates = new Set([2]);

  it('accepts everything for All games', () => {
    expect(matchesRail(played, 'all', updates, NOW)).toBe(true);
    expect(matchesRail(stale, 'all', updates, NOW)).toBe(true);
  });
  it('accepts only rows played inside the window for Recent', () => {
    expect(matchesRail(played, 'recent', updates, NOW)).toBe(true);
    expect(matchesRail(stale, 'recent', updates, NOW)).toBe(false);
  });
  it('treats a stamp exactly on the window edge as recent', () => {
    const edge = row({ rom_id: 3, last_played_at: NOW - RECENT_WINDOW_SECONDS });
    expect(matchesRail(edge, 'recent', updates, NOW)).toBe(true);
  });
  it('accepts only rows in the update set for Updates', () => {
    expect(matchesRail(stale, 'updates', updates, NOW)).toBe(true);
    expect(matchesRail(played, 'updates', updates, NOW)).toBe(false);
  });
  it('never matches Updates for a row with no rom id', () => {
    expect(matchesRail(row({ rom_id: null }), 'updates', updates, NOW)).toBe(false);
  });
  it('matches a platform entry case- and space-insensitively', () => {
    expect(matchesRail(row({ platform: ' snes ' }), 'platform:snes', updates, NOW)).toBe(true);
    expect(matchesRail(row({ platform: 'Arcade' }), 'platform:snes', updates, NOW)).toBe(false);
  });
});

describe('emptyText (design section 5, verbatim)', () => {
  it('reads differently for each rail entry', () => {
    expect(emptyText({ key: 'all', testId: 'library-rail-all', label: 'All games', count: 0 })).toBe(
      'No games installed',
    );
    expect(
      emptyText({ key: 'recent', testId: 'library-rail-recent', label: 'Recent', count: 0 }),
    ).toBe('Nothing played in the last 30 days');
    expect(
      emptyText({ key: 'updates', testId: 'library-rail-updates', label: 'Updates', count: 0 }),
    ).toBe('Everything is up to date');
    expect(
      emptyText({
        key: 'platform:snes',
        testId: 'library-rail-platform-snes',
        label: 'SNES',
        count: 0,
      }),
    ).toBe('No games installed for SNES');
  });
});
```

- [ ] **Step 10: Run it to verify it fails**

Run from `rewrite/app`: `npx vitest run src/lib/library/rail.test.ts`
Expected: FAIL — `Failed to resolve import "./rail"`.

- [ ] **Step 11: Write `rail.ts`**

Create `app/src/lib/library/rail.ts`:

```ts
// The Library rail (design §5, D-UI-2): All games, Recent, Updates, then
// the installed platforms with counts. Pure — Library.svelte renders these
// entries and asks `matchesRail` which rows an entry keeps.
import type { InstalledGame } from '../api';
import { isHiddenLibraryPlatform } from '../library';

/** Design §5: "Recent (played in the last 30 days)", in whole seconds. */
export const RECENT_WINDOW_SECONDS = 30 * 24 * 60 * 60;

export type RailKey = 'all' | 'recent' | 'updates' | `platform:${string}`;

export type RailEntry = {
  key: RailKey;
  /** The §11 `library-rail-<key>` test id for this entry. */
  testId: string;
  label: string;
  count: number;
};

/**
 * A platform name reduced to the id-safe token the rail's test id carries.
 * Runs of anything that is not a letter or digit collapse to one dash, and
 * leading/trailing dashes are dropped, so "Sega CD / Mega-CD" and
 * "Sega CD  Mega CD" cannot produce two different rail entries for one name.
 */
export function platformSlug(platform: string): string {
  return platform
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');
}

const platformKey = (platform: string): RailKey => `platform:${platformSlug(platform)}`;

function isRecent(row: InstalledGame, nowSeconds: number): boolean {
  // A zero stamp is "never launched", not "launched at the epoch".
  if (row.last_played_at <= 0) return false;
  return nowSeconds - row.last_played_at <= RECENT_WINDOW_SECONDS;
}

function hasUpdate(row: InstalledGame, updateRomIds: ReadonlySet<number>): boolean {
  return row.rom_id !== null && updateRomIds.has(row.rom_id);
}

/** Whether `row` belongs under `key`. `all` keeps everything. */
export function matchesRail(
  row: InstalledGame,
  key: RailKey,
  updateRomIds: ReadonlySet<number>,
  nowSeconds: number,
): boolean {
  if (key === 'all') return true;
  if (key === 'recent') return isRecent(row, nowSeconds);
  if (key === 'updates') return hasUpdate(row, updateRomIds);
  return platformKey(row.platform) === key;
}

/**
 * The rail, in design §5's order. The synthetic "Emulators" platform is
 * excluded everywhere, counts included, exactly as the grid excludes it
 * (`isHiddenLibraryPlatform`, ported from game_views.py:297-311).
 */
export function railEntries(
  rows: InstalledGame[],
  updateRomIds: ReadonlySet<number>,
  nowSeconds: number,
): RailEntry[] {
  const visible = rows.filter((row) => !isHiddenLibraryPlatform(row.platform));

  const platforms = new Map<RailKey, { label: string; count: number }>();
  for (const row of visible) {
    const key = platformKey(row.platform);
    const existing = platforms.get(key);
    if (existing) existing.count += 1;
    else platforms.set(key, { label: row.platform.trim(), count: 1 });
  }

  const entries: RailEntry[] = [
    { key: 'all', testId: 'library-rail-all', label: 'All games', count: visible.length },
    {
      key: 'recent',
      testId: 'library-rail-recent',
      label: 'Recent',
      count: visible.filter((row) => isRecent(row, nowSeconds)).length,
    },
    {
      key: 'updates',
      testId: 'library-rail-updates',
      label: 'Updates',
      count: visible.filter((row) => hasUpdate(row, updateRomIds)).length,
    },
  ];

  const sorted = [...platforms.entries()].sort((a, b) =>
    a[1].label.toLowerCase() < b[1].label.toLowerCase() ? -1 : 1,
  );
  for (const [key, value] of sorted) {
    entries.push({
      key,
      testId: `library-rail-platform-${key.slice('platform:'.length)}`,
      label: value.label,
      count: value.count,
    });
  }
  return entries;
}

/** The entry for `key`, falling back to `all` when the key has gone away
 *  (the last game on a platform was uninstalled while it was selected). */
export function entryForKey(entries: RailEntry[], key: RailKey): RailEntry {
  return entries.find((entry) => entry.key === key) ?? entries[0];
}

/** Design §5's empty-state text, one line per rail entry, verbatim. */
export function emptyText(entry: RailEntry): string {
  if (entry.key === 'recent') return 'Nothing played in the last 30 days';
  if (entry.key === 'updates') return 'Everything is up to date';
  if (entry.key === 'all') return 'No games installed';
  return `No games installed for ${entry.label}`;
}
```

- [ ] **Step 12: Run it to verify it passes**

Run from `rewrite/app`: `npx vitest run src/lib/library/rail.test.ts`
Expected: PASS.

- [ ] **Step 13: Write the failing sort test**

Create `app/src/lib/library/sort.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import type { InstalledGame } from '../api';
import { LIBRARY_SORTS, normalizeSort, sortGames, sortLabel, titleContains } from './sort';

const row = (overrides: Partial<InstalledGame>): InstalledGame => ({
  title: 'Game', platform: 'SNES', rom_id: 1, rom_file_name: '', archive_path: '',
  extracted_path: '', extracted_dir: '', multi_file_game_dir: '', description: '', rating: '',
  genres: '', regions: '', languages: '', tags: '', revision: '', companies: '',
  first_release_date: '', filesize_bytes: 0, server_updated_at: '', installed_at: 0,
  cover_small_path: '', cover_large_path: '', screenshot_urls: '', native_executable_path: '',
  native_launch_parameters: '', native_compat_tool: '', native_wineprefix: '',
  native_game_dir: '', included_dlc: '', ps3_trophy_paths: '', ps3_game_id: '',
  ps3_iso_path: '', ps4_game_id: '', ps4_content: '', ra_id: '', last_played_at: 0,
  ...overrides,
});

describe('normalizeSort / sortLabel', () => {
  it('accepts the four stored names and falls back to title', () => {
    expect(LIBRARY_SORTS).toEqual(['played', 'installed', 'title', 'platform']);
    expect(normalizeSort('played')).toBe('played');
    expect(normalizeSort('nonsense')).toBe('title');
  });
  it('labels each sort the way design section 5 names it', () => {
    expect(LIBRARY_SORTS.map(sortLabel)).toEqual([
      'Recently played',
      'Recently installed',
      'Title',
      'Platform',
    ]);
  });
});

describe('sortGames', () => {
  const a = row({ title: 'Alpha', platform: 'SNES', installed_at: 10, last_played_at: 0 });
  const b = row({ title: 'beta', platform: 'Arcade', installed_at: 30, last_played_at: 500 });
  const c = row({ title: ' Ceta', platform: 'GBA', installed_at: 20, last_played_at: 900 });

  it('does not mutate the input', () => {
    const input = [a, b, c];
    sortGames(input, 'title');
    expect(input).toEqual([a, b, c]);
  });

  it('sorts by title case- and space-insensitively', () => {
    expect(sortGames([c, b, a], 'title').map((r) => r.title)).toEqual(['Alpha', 'beta', ' Ceta']);
  });

  it('sorts by platform then title', () => {
    expect(sortGames([a, b, c], 'platform').map((r) => r.platform)).toEqual([
      'Arcade',
      'GBA',
      'SNES',
    ]);
  });

  it('sorts most recently installed first', () => {
    expect(sortGames([a, b, c], 'installed').map((r) => r.installed_at)).toEqual([30, 20, 10]);
  });

  it('sorts most recently played first, with never-played rows last by title', () => {
    const d = row({ title: 'Delta', installed_at: 99, last_played_at: 0 });
    expect(sortGames([a, b, c, d], 'played').map((r) => r.title)).toEqual([
      ' Ceta',
      'beta',
      'Alpha',
      'Delta',
    ]);
  });
});

describe('titleContains', () => {
  it('is a case-insensitive substring test', () => {
    expect(titleContains('Chrono Trigger', 'chrono')).toBe(true);
    expect(titleContains('Chrono Trigger', 'TRIG')).toBe(true);
    expect(titleContains('Chrono Trigger', 'zelda')).toBe(false);
  });
  it('accepts everything for a blank or whitespace query', () => {
    expect(titleContains('Chrono Trigger', '')).toBe(true);
    expect(titleContains('Chrono Trigger', '   ')).toBe(true);
  });
  it('trims the query, so a trailing space still matches', () => {
    expect(titleContains('Chrono Trigger', ' chrono ')).toBe(true);
  });
});
```

- [ ] **Step 14: Run it to verify it fails**

Run from `rewrite/app`: `npx vitest run src/lib/library/sort.test.ts`
Expected: FAIL — `Failed to resolve import "./sort"`.

- [ ] **Step 15: Write `sort.ts`**

Create `app/src/lib/library/sort.ts`:

```ts
// The Library toolbar's sort modes and the search predicate both grids
// share (design §5, §6). Pure.
import type { InstalledGame } from '../api';

export const LIBRARY_SORTS = ['played', 'installed', 'title', 'platform'] as const;
export type LibrarySort = (typeof LIBRARY_SORTS)[number];

const LABELS: Record<LibrarySort, string> = {
  played: 'Recently played',
  installed: 'Recently installed',
  title: 'Title',
  platform: 'Platform',
};

export function normalizeSort(raw: string): LibrarySort {
  const trimmed = raw.trim();
  return (LIBRARY_SORTS as readonly string[]).includes(trimmed)
    ? (trimmed as LibrarySort)
    : 'title';
}

export function sortLabel(sort: LibrarySort): string {
  return LABELS[sort];
}

const fold = (value: string) => value.trim().toLowerCase();

function byTitle(a: InstalledGame, b: InstalledGame): number {
  const ta = fold(a.title);
  const tb = fold(b.title);
  if (ta !== tb) return ta < tb ? -1 : 1;
  return 0;
}

/**
 * A stable, non-mutating sort. Every mode falls back to title so the grid
 * never reorders itself between renders over rows that tie — two games
 * installed in the same second, or two never played.
 */
export function sortGames(rows: InstalledGame[], sort: LibrarySort): InstalledGame[] {
  const out = rows.slice();
  if (sort === 'title') {
    out.sort(byTitle);
  } else if (sort === 'platform') {
    out.sort((a, b) => {
      const pa = fold(a.platform);
      const pb = fold(b.platform);
      if (pa !== pb) return pa < pb ? -1 : 1;
      return byTitle(a, b);
    });
  } else if (sort === 'installed') {
    out.sort((a, b) => b.installed_at - a.installed_at || byTitle(a, b));
  } else {
    out.sort((a, b) => b.last_played_at - a.last_played_at || byTitle(a, b));
  }
  return out;
}

/** Design §5 / §6: "search (title contains)", case-insensitive. A blank
 *  query matches everything so the grid is never empty just from focus. */
export function titleContains(title: string, query: string): boolean {
  const needle = fold(query);
  if (needle === '') return true;
  return title.toLowerCase().includes(needle);
}
```

- [ ] **Step 16: Run it to verify it passes**

Run from `rewrite/app`: `npx vitest run src/lib/library/sort.test.ts`
Expected: PASS.

- [ ] **Step 17: Add the card size to the settings store**

In `app/src/lib/stores/uiSettings.svelte.ts`, add the import beside the existing `../theme` import:

```ts
import { normalizeCardSize, type CardSize } from '../cards/size';
```

Change the two `state.cardSize*` assignments added in Task 1 Step 15 to normalize what the config returned:

```ts
    state.cardSizeLibrary = normalizeCardSize(stored.card_size_library);
    state.cardSizeServer = normalizeCardSize(stored.card_size_server);
```

Add two getters to the exported `uiSettings` object, after `resolved`:

```ts
  get cardSizeLibrary(): CardSize {
    return state.cardSizeLibrary;
  },
  get cardSizeServer(): CardSize {
    return state.cardSizeServer;
  },
```

and append the writer at the end of the file:

```ts
/**
 * The size control on a grid toolbar. Applies immediately — the grid
 * re-flows on the next frame — then persists. A failed save leaves the
 * grid at the new size for this session and the config at the old one;
 * that is the same trade `setTheme` makes, and reverting a grid under the
 * user's cursor would be worse than a setting that did not stick.
 */
export async function setCardSize(view: 'library' | 'server', size: CardSize): Promise<void> {
  if (view === 'library') state.cardSizeLibrary = size;
  else state.cardSizeServer = size;
  await api.setUiSettings(payload());
}
```

- [ ] **Step 18: Run the whole frontend suite**

Run from `rewrite/app`: `npm run check && npx vitest run`
Expected: PASS — the four new test files plus every existing one.

- [ ] **Step 19: Commit**

```bash
cd rewrite
git add app/src/lib/cards app/src/lib/library app/src/lib/stores/uiSettings.svelte.ts
git commit -m "rewrite: add the rail, sort, size and badge rules for the redesigned grids"
```

---

### Task 3: `GameCard.svelte` and `CardGrid.svelte`

**Files:**
- Create: `app/src/lib/GameCard.svelte`, `app/src/lib/CardGrid.svelte`
- Modify: `app/src/lib/Details.svelte:15-24` (props), `:161` (the `cloudMode` initializer)

**Interfaces:**
- Consumes: `Image.svelte`; `cardBadges`, `shortPlatformName`, `UPDATE_TAG_TEXT` from `lib/cards/badges`; `gridTemplate`, `columnsOf`, `type CardSize` from `lib/cards/size`; `moveFocus`, `type NavDirection` from `lib/focus/grid`.
- Produces, used by Tasks 4, 5 and 7:
  - `GameCard.svelte` props:
    ```ts
    {
      testId: string;              // `library-card-<id>` or `game-card-<id>`
      badgeId: string | number;    // the id the badge test ids carry
      title: string;
      platform: string;
      coverUrl: string | null;
      installed: boolean;
      updateLabel: string | null;
      cloudPlatforms: ReadonlySet<string>;
      focused: boolean;
      onOpen: () => void;
      onPrimary: () => void;       // Play when installed, Install when not
      onCloud: () => void;
      onHoverStart: () => void;
      onHoverEnd: () => void;
    }
    ```
  - Test ids the card renders: `{testId}` on the root, `installed-badge-<badgeId>` on the installed dot, `library-update-badge-<badgeId>` on the UPDATE tag, `card-primary-<badgeId>` on the centred action, `card-details-<badgeId>`, `card-cloud-<badgeId>`, `card-more-<badgeId>` in the action row, `card-platform-<badgeId>` on the platform chip, `card-cloud-badge-<badgeId>` on the cloud icon.
  - `CardGrid.svelte` props: `{ size: CardSize; gridId: string; children: import('svelte').Snippet }` and `export function columns(): number`, plus `bind:this` access to the underlying element through `export function element(): HTMLElement | null`.
  - `Details.svelte` gains `initialCloudMode?: 'overview' | 'save' | 'state'` (default `'overview'`).

- [ ] **Step 1: Write the card-geometry note into a test that guards it**

The one behaviour of these components that pure code can hold is the geometry contract the E2E suite depends on: **a click at the card root's centre must not hit an overlay control.** Encode the arithmetic in `app/src/lib/cards/size.test.ts` (append):

```ts
import { CARD_COVER_RATIO, PRIMARY_CENTRE_FRACTION, ACTION_ROW_HEIGHT_PX, TITLE_ROW_HEIGHT_PX } from './size';

describe('card hover geometry (E2E click safety)', () => {
  // Specs click `library-card-<id>` / `game-card-<id>` to open Details, and
  // WebdriverIO clicks an element's CENTRE — which also hovers it, raising
  // the overlay. If the centred Play/Install button or the action row sat
  // under that point, every such click would launch or install instead.
  // The card root is the cover plus a one-line title, so its centre is
  // always slightly BELOW the cover's centre; the overlay keeps that band
  // free.
  const heights = [160, 213, 267, 400]; // small, medium, large, a stretched large

  it('never puts the centred primary action under the card root centre', () => {
    for (const cover of heights) {
      const rootCentre = (cover + TITLE_ROW_HEIGHT_PX) / 2;
      const primaryBottom = cover * PRIMARY_CENTRE_FRACTION + 17;
      expect(primaryBottom).toBeLessThan(rootCentre);
    }
  });

  it('never puts the bottom action row under the card root centre', () => {
    for (const cover of heights) {
      const rootCentre = (cover + TITLE_ROW_HEIGHT_PX) / 2;
      const actionRowTop = cover - ACTION_ROW_HEIGHT_PX;
      expect(actionRowTop).toBeGreaterThan(rootCentre);
    }
  });

  it('states the 3:4 fallback ratio design section 5 requires', () => {
    expect(CARD_COVER_RATIO).toBe('3 / 4');
  });
});
```

- [ ] **Step 2: Run it to verify it fails**

Run from `rewrite/app`: `npx vitest run src/lib/cards/size.test.ts`
Expected: FAIL — `No "CARD_COVER_RATIO" export is defined on the "./size" module`.

- [ ] **Step 3: Add the geometry constants**

Append to `app/src/lib/cards/size.ts`:

```ts
/**
 * The cover's fallback aspect ratio (design §5: "cover ratio from the image
 * with a 3:4 fallback"). Applied to the cover box; a loaded image with a
 * different intrinsic ratio fills it with `object-fit: cover`.
 */
export const CARD_COVER_RATIO = '3 / 4';

/** The one-line title strip under the cover, in px. */
export const TITLE_ROW_HEIGHT_PX = 22;

/**
 * Where the hover overlay's centred Play/Install sits, as a fraction of the
 * cover height, and how tall the bottom action row's reserved strip is.
 *
 * These two numbers exist so the band around the CARD ROOT's centre stays
 * free of interactive controls. WebdriverIO clicks an element's centre, and
 * `library-card-<id>` / `game-card-<id>` are the ids the specs click to
 * open Details — if the primary action or the action row covered that
 * point, every spec click would launch or install instead. The card root is
 * the cover plus `TITLE_ROW_HEIGHT_PX`, so its centre lands at
 * `0.5 + TITLE_ROW_HEIGHT_PX / (2 * cover)` of the cover — between 52.7%
 * and 56.9% for every size this app renders. `size.test.ts` proves the
 * primary button (34% ± 17px) and the action row (the last 38px) both clear
 * it. The gradient overlay itself is `pointer-events: none`, so a click in
 * that band falls through to the card root and opens Details.
 */
export const PRIMARY_CENTRE_FRACTION = 0.34;
export const ACTION_ROW_HEIGHT_PX = 38;
```

- [ ] **Step 4: Run it to verify it passes**

Run from `rewrite/app`: `npx vitest run src/lib/cards/size.test.ts`
Expected: PASS.

- [ ] **Step 5: Write `GameCard.svelte`**

Create `app/src/lib/GameCard.svelte`:

```svelte
<script lang="ts">
  import Image from './Image.svelte';
  import { cardBadges, UPDATE_TAG_TEXT } from './cards/badges';
  import {
    ACTION_ROW_HEIGHT_PX,
    CARD_COVER_RATIO,
    PRIMARY_CENTRE_FRACTION,
    TITLE_ROW_HEIGHT_PX,
  } from './cards/size';

  let {
    testId,
    badgeId,
    title,
    platform,
    coverUrl,
    installed,
    updateLabel,
    cloudPlatforms,
    focused,
    onOpen,
    onPrimary,
    onCloud,
    onHoverStart,
    onHoverEnd,
  }: {
    testId: string;
    badgeId: string | number;
    title: string;
    platform: string;
    coverUrl: string | null;
    installed: boolean;
    updateLabel: string | null;
    cloudPlatforms: ReadonlySet<string>;
    focused: boolean;
    onOpen: () => void;
    onPrimary: () => void;
    onCloud: () => void;
    onHoverStart: () => void;
    onHoverEnd: () => void;
  } = $props();

  let badges = $derived(cardBadges({ platform, installed, updateLabel, cloudPlatforms }));

  /**
   * Every overlay control stops the click here: the card root's own handler
   * opens Details, and without this an action button would open Details as
   * well as doing its own job.
   */
  function act(handler: () => void) {
    return (e: MouseEvent) => {
      e.stopPropagation();
      handler();
    };
  }
</script>

<div
  data-testid={testId}
  class="card"
  class:focused
  class:dim={!installed}
  onclick={onOpen}
  onmouseenter={onHoverStart}
  onmouseleave={onHoverEnd}
  role="presentation"
  style="--cover-ratio: {CARD_COVER_RATIO}; --title-h: {TITLE_ROW_HEIGHT_PX}px; --primary-y: {PRIMARY_CENTRE_FRACTION * 100}%; --action-h: {ACTION_ROW_HEIGHT_PX}px"
>
  <div class="cover">
    <Image url={coverUrl} alt={title} placeholder="No cover" />

    {#if badges.update}
      <span data-testid={`library-update-badge-${badgeId}`} class="tag update">{UPDATE_TAG_TEXT}</span>
    {/if}
    {#if badges.installed}
      <span data-testid={`installed-badge-${badgeId}`} class="dot" title="Installed"></span>
    {/if}
    {#if badges.platform}
      <span data-testid={`card-platform-${badgeId}`} class="tag platform">{badges.platform}</span>
    {/if}
    {#if badges.cloud}
      <span data-testid={`card-cloud-badge-${badgeId}`} class="cloud-badge" title="Cloud sync available">☁</span>
    {/if}

    <!-- The gradient itself never takes a click: the band around the card
         root's centre must fall through to `onOpen` (see size.ts). -->
    <div class="overlay" aria-hidden="true"></div>

    <button
      data-testid={`card-primary-${badgeId}`}
      class="primary"
      onclick={act(onPrimary)}
      tabindex="-1"
    >
      {installed ? 'Play' : 'Install'}
    </button>

    <div class="actions">
      <button data-testid={`card-details-${badgeId}`} onclick={act(onOpen)} tabindex="-1">Details</button>
      <button data-testid={`card-cloud-${badgeId}`} onclick={act(onCloud)} tabindex="-1">Cloud sync</button>
      <button data-testid={`card-more-${badgeId}`} onclick={act(onOpen)} tabindex="-1">More</button>
    </div>
  </div>

  <span class="title">{title}</span>
</div>

<style>
  .card {
    display: flex;
    flex-direction: column;
    gap: 4px;
    cursor: pointer;
    transform: scale(1);
    transition: transform var(--m-fast) cubic-bezier(0.2, 0.9, 0.3, 1.2);
    will-change: transform;
    /* Off-screen cards skip layout/paint; the intrinsic size keeps the
       scrollbar stable at the fallback cover ratio. */
    content-visibility: auto;
    contain-intrinsic-size: auto 200px 289px;
  }

  /* D-UI-9: hover scales 1.05. Focus (gamepad/arrow keys) uses the same
     scale so the two selection models look identical. */
  .card:hover,
  .card.focused {
    transform: scale(1.05);
    z-index: 1;
  }

  .card.focused .cover {
    outline: 2px solid var(--primary);
    outline-offset: 1px;
  }

  /* D-UI-3: not-installed cards render at 60% until hover. */
  .card.dim .cover {
    opacity: 0.6;
    transition: opacity var(--m-fast) ease;
  }
  .card.dim:hover .cover,
  .card.dim.focused .cover {
    opacity: 1;
  }

  .cover {
    position: relative;
    aspect-ratio: var(--cover-ratio);
    border-radius: var(--r-card);
    overflow: hidden;
    background: var(--surface-2);
  }

  .cover :global(img) {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .title {
    height: var(--title-h);
    line-height: var(--title-h);
    font-size: 12px;
    color: var(--text-h);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .overlay {
    position: absolute;
    inset: 0;
    pointer-events: none;
    opacity: 0;
    transition: opacity var(--m-fast) ease;
    background: linear-gradient(to top, rgba(0, 0, 0, 0.85), rgba(0, 0, 0, 0.15) 55%, rgba(0, 0, 0, 0.35));
  }

  .card:hover .overlay,
  .card:focus-within .overlay {
    opacity: 1;
  }

  .primary,
  .actions {
    position: absolute;
    opacity: 0;
    visibility: hidden;
    transition: opacity var(--m-fast) ease, visibility var(--m-fast) ease;
  }

  .card:hover .primary,
  .card:hover .actions,
  .card:focus-within .primary,
  .card:focus-within .actions {
    opacity: 1;
    visibility: visible;
  }

  .primary {
    top: var(--primary-y);
    left: 50%;
    transform: translate(-50%, -50%);
    font: inherit;
    font-size: 12px;
    font-weight: 600;
    height: 30px;
    padding: 0 18px;
    border: none;
    border-radius: var(--r-pill);
    background: var(--primary);
    color: #fff;
    cursor: pointer;
    white-space: nowrap;
  }

  .primary:hover {
    background: var(--primary-hover);
  }

  .actions {
    left: 0;
    right: 0;
    bottom: 0;
    height: var(--action-h);
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 4px;
    padding: 0 4px;
    box-sizing: border-box;
  }

  .actions button {
    font: inherit;
    font-size: 10px;
    padding: 3px 6px;
    border: none;
    border-radius: var(--r-chip);
    background: rgba(0, 0, 0, 0.55);
    color: #fff;
    cursor: pointer;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .actions button:hover {
    background: var(--primary);
  }

  .tag {
    position: absolute;
    padding: 2px 6px;
    border-radius: var(--r-chip);
    font-size: 10px;
    font-weight: 600;
    line-height: 1.4;
    background: rgba(0, 0, 0, 0.65);
    color: #fff;
    max-width: calc(100% - 12px);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* D-UI-9 placements. */
  .tag.update {
    top: 6px;
    left: 6px;
    background: var(--warning);
    color: #1a1a12;
    letter-spacing: 0.06em;
  }

  .tag.platform {
    bottom: 6px;
    left: 6px;
  }

  .dot {
    position: absolute;
    top: 8px;
    right: 8px;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--success);
    box-shadow: 0 0 0 2px rgba(0, 0, 0, 0.55);
  }

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
</style>
```

- [ ] **Step 6: Write `CardGrid.svelte`**

Create `app/src/lib/CardGrid.svelte`:

```svelte
<script lang="ts">
  import { columnsOf, gridTemplate, type CardSize } from './cards/size';

  let {
    size,
    gridId,
    children,
  }: {
    size: CardSize;
    gridId: string;
    children: import('svelte').Snippet;
  } = $props();

  let el = $state<HTMLElement | null>(null);

  /** The number of columns the browser resolved, for arrow-key movement. */
  export function columns(): number {
    return columnsOf(el);
  }

  /** The grid element, so a view can scroll its focused child into view. */
  export function element(): HTMLElement | null {
    return el;
  }
</script>

<!-- D-UI-7: grids may run to the full window width, capped at 1920px. They
     deliberately do NOT take `.view-content` (1100px), which is for lists. -->
<div
  data-testid={gridId}
  class="grid"
  bind:this={el}
  style="--template: {gridTemplate(size)}"
>
  {@render children()}
</div>

<style>
  .grid {
    display: grid;
    grid-template-columns: var(--template);
    gap: 16px;
    padding: 16px 24px 24px;
    width: 100%;
    max-width: 1920px;
    margin: 0 auto;
    box-sizing: border-box;
  }
</style>
```

- [ ] **Step 7: Give `Details.svelte` its `initialCloudMode` prop**

In `app/src/lib/Details.svelte`, replace the props block (lines 15-24) with:

```svelte
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
     *  passes `'save'` so the panel is already expanded; every other opener
     *  leaves it at `'overview'`. The redesigned Saves tab is plan 3's. */
    initialCloudMode?: CloudMode;
  } = $props();
```

and change line 161 from `let cloudMode = $state<CloudMode>('overview');` to:

```svelte
  let cloudMode = $state<CloudMode>(initialCloudMode);
```

`CloudMode` is already imported on line 12.

- [ ] **Step 8: Type-check**

Run from `rewrite/app`: `npm run check && npx vitest run`
Expected: PASS. `GameCard.svelte` and `CardGrid.svelte` are unreferenced at this point — svelte-check reports no error for an unused component.

- [ ] **Step 9: Commit**

```bash
cd rewrite
git add app/src/lib/GameCard.svelte app/src/lib/CardGrid.svelte \
        app/src/lib/cards/size.ts app/src/lib/cards/size.test.ts app/src/lib/Details.svelte
git commit -m "rewrite: add the shared game card and card grid components"
```

---

### Task 4: The Library view — rail, toolbar, grid, empty states

**Files:**
- Rewrite: `app/src/lib/Library.svelte` (all 179 lines)
- Create: `app/src/lib/library/selection.svelte.ts`

**Interfaces:**
- Consumes: `railEntries`, `matchesRail`, `emptyText`, `entryForKey`, `type RailKey` (Task 2); `sortGames`, `titleContains`, `normalizeSort`, `sortLabel`, `LIBRARY_SORTS`, `type LibrarySort` (Task 2); `CARD_SIZES`, `cardSizeLabel`, `columnsOf`, `type CardSize` (Task 2/3); `cloudPlatformSet` (Task 2); `uiSettings.cardSizeLibrary`, `setCardSize` (Task 2); `GameCard.svelte`, `CardGrid.svelte` (Task 3); `installed`, `updates`, `sessions` stores; `api.getLaunchDefaults`, `api.launchGame`; `createHoverViewed`, `noteViewed`; `visibleLibraryGames` (`lib/library.ts`); `moveFocus`.
- Produces, used by Tasks 7 and 8:
  - `Library.svelte` keeps `export function handleNav(action)` and gains `export function focusSearch(): void`.
  - `app/src/lib/library/selection.svelte.ts`: `export const librarySelection` with getter `key: RailKey`; `export function selectRail(key: RailKey): void`. Module-scoped, so the rail choice persists for the session across view switches and Shell remounts.
  - Test ids: `library-section` (root, kept), `library-rail`, `library-rail-<key>` entries, `library-rail-count-<key>`, `library-search`, `library-sort`, `library-size`, `library-grid`, `library-empty` (kept), `library-card-<romId>` (kept).

- [ ] **Step 1: Write `selection.svelte.ts`**

Create `app/src/lib/library/selection.svelte.ts`:

```ts
// Which Library rail entry is selected. Module scoped, like
// `appUpdate.svelte.ts`, because design §5 says the selection "persists per
// session": the Library view unmounts nothing today, but a Shell remount
// (a reconnect) must not silently throw the user back to All games.
import type { RailKey } from './rail';

const state = $state<{ key: RailKey }>({ key: 'all' });

export const librarySelection = {
  get key(): RailKey {
    return state.key;
  },
};

export function selectRail(key: RailKey): void {
  state.key = key;
}
```

- [ ] **Step 2: Rewrite `Library.svelte`**

Replace the whole of `app/src/lib/Library.svelte` with:

```svelte
<script lang="ts">
  import { api, type InstalledGame } from './api';
  import { installed } from './stores/installed.svelte';
  import { updates } from './stores/updates.svelte';
  import { visibleLibraryGames } from './library';
  import { emptyText, entryForKey, matchesRail, railEntries, type RailKey } from './library/rail';
  import { librarySelection, selectRail } from './library/selection.svelte';
  import { LIBRARY_SORTS, normalizeSort, sortGames, sortLabel, titleContains, type LibrarySort } from './library/sort';
  import { cloudPlatformSet } from './cards/badges';
  import { CARD_SIZES, cardSizeLabel, type CardSize } from './cards/size';
  import { setCardSize, uiSettings } from './stores/uiSettings.svelte';
  import { fromInstalled, type DetailsSubject } from './details/subject';
  import type { CloudMode } from './details/cloud';
  import GameCard from './GameCard.svelte';
  import CardGrid from './CardGrid.svelte';
  import Details from './Details.svelte';
  import { moveFocus, type NavDirection } from './focus/grid';
  import { createHoverViewed } from './lastViewedHover';
  import { noteViewed } from './stores/lastViewed.svelte';

  let { active }: { active: boolean } = $props();

  let search = $state('');
  let sort = $state<LibrarySort>('title');
  let focusIndex = $state(0);
  let grid = $state<ReturnType<typeof CardGrid> | null>(null);
  let searchEl = $state<HTMLInputElement | null>(null);
  let subject = $state<DetailsSubject | null>(null);
  let detailsCloudMode = $state<CloudMode>('overview');
  let launchError = $state<string | null>(null);
  let cloudPlatforms = $state<ReadonlySet<string>>(new Set<string>());

  // `Date.now()` is read once per rail recompute rather than per row, so
  // every entry and every predicate in one render agrees on "now".
  let nowSeconds = $derived.by(() => {
    // Depend on the two inputs that can change the rail, so the timestamp
    // refreshes whenever the rail does instead of freezing at mount.
    void installed.list;
    void updates.rows;
    return Math.floor(Date.now() / 1000);
  });

  let updateRomIds = $derived(new Set(updates.rows.map((r) => r.rom_id)));
  let entries = $derived(railEntries(installed.list, updateRomIds, nowSeconds));
  let selected = $derived(entryForKey(entries, librarySelection.key));

  let rows = $derived(
    sortGames(
      visibleLibraryGames(installed.list).filter(
        (row) =>
          matchesRail(row, selected.key, updateRomIds, nowSeconds) && titleContains(row.title, search),
      ),
      sort,
    ),
  );

  // Which platforms have a default emulator, for the cards' cloud badge
  // (see `cloudPlatformSet`). One call per mount; the Emulators view is
  // where defaults change, and switching back re-runs this effect.
  $effect(() => {
    if (!active) return;
    api
      .getLaunchDefaults()
      .then((defaults) => (cloudPlatforms = cloudPlatformSet(defaults.default_emulators)))
      .catch(() => {
        // No defaults readable: no cloud badges. A missing hint badge is
        // not worth an error line over the grid.
      });
  });

  // A filter change can leave the focus index past the end of the new list.
  $effect(() => {
    if (focusIndex > rows.length - 1) focusIndex = Math.max(0, rows.length - 1);
  });

  function openDetails(row: InstalledGame, mode: CloudMode = 'overview') {
    detailsCloudMode = mode;
    subject = fromInstalled(row);
    noteViewed(row.cover_large_path);
  }

  function closeDetails() {
    subject = null;
    detailsCloudMode = 'overview';
  }

  async function play(row: InstalledGame) {
    if (row.rom_id === null) return;
    launchError = null;
    try {
      await api.launchGame(row.rom_id);
    } catch (err) {
      launchError = err instanceof Error ? err.message : String(err);
    }
  }

  // Design §3: a card becomes the background only after the pointer has
  // rested on it for more than half a second.
  const hover = createHoverViewed();

  /** Design §3: `Ctrl+F` focuses the current view's search box. */
  export function focusSearch() {
    searchEl?.focus();
    searchEl?.select();
  }

  export function handleNav(action: NavDirection | 'accept' | 'back') {
    if (action === 'back') {
      if (subject) closeDetails();
      return;
    }
    if (action === 'accept') {
      if (!subject) {
        const row = rows[focusIndex];
        if (row) openDetails(row);
      }
      return;
    }
    if (subject) return; // grid navigation is suspended while the overlay is open
    focusIndex = moveFocus(focusIndex, action, grid?.columns() ?? 1, rows.length);
    grid?.element()?.children[focusIndex]?.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
  }

  function onKey(e: KeyboardEvent) {
    if (!active) return;
    // The search box owns its own arrow keys.
    if (document.activeElement === searchEl) return;
    const map: Record<string, NavDirection> = {
      ArrowUp: 'up', ArrowDown: 'down', ArrowLeft: 'left', ArrowRight: 'right',
    };
    const action = map[e.key];
    if (action) {
      e.preventDefault();
      handleNav(action);
    }
  }

  function onSortChange(e: Event) {
    sort = normalizeSort((e.currentTarget as HTMLSelectElement).value);
  }

  function onSizeChange(e: Event) {
    const size = (e.currentTarget as HTMLSelectElement).value as CardSize;
    setCardSize('library', size).catch(() => {
      // Applied for this session; a failed save is not worth an error line.
    });
  }
</script>

<svelte:window onkeydown={onKey} />

<section data-testid="library-section" class="library">
  <nav data-testid="library-rail" class="rail" aria-label="Library filters">
    {#each entries as entry, i (entry.key)}
      {#if i === 3}
        <span class="rail-heading">PLATFORMS</span>
      {/if}
      <button
        data-testid={entry.testId}
        data-rail={entry.testId}
        class="rail-item"
        class:active={entry.key === selected.key}
        aria-current={entry.key === selected.key ? 'page' : undefined}
        onclick={() => selectRail(entry.key)}
      >
        <span class="rail-label">{entry.label}</span>
        <span data-testid={`library-rail-count-${entry.key}`} class="rail-count">{entry.count}</span>
      </button>
    {/each}
  </nav>

  <div class="body">
    <div class="toolbar">
      <input
        data-testid="library-search"
        class="search"
        type="search"
        placeholder="Search installed games"
        aria-label="Search installed games"
        bind:this={searchEl}
        bind:value={search}
      />
      <label class="control">
        <span>Sort</span>
        <select data-testid="library-sort" value={sort} onchange={onSortChange}>
          {#each LIBRARY_SORTS as option (option)}
            <option value={option}>{sortLabel(option)}</option>
          {/each}
        </select>
      </label>
      <label class="control">
        <span>Size</span>
        <select data-testid="library-size" value={uiSettings.cardSizeLibrary} onchange={onSizeChange}>
          {#each CARD_SIZES as option (option)}
            <option value={option}>{cardSizeLabel(option)}</option>
          {/each}
        </select>
      </label>
    </div>

    {#if launchError}
      <p data-testid="library-launch-error" class="error" role="alert">{launchError}</p>
    {/if}

    {#if rows.length === 0}
      <p data-testid="library-empty" class="empty">
        {search.trim() === '' ? emptyText(selected) : `No games match “${search.trim()}”`}
      </p>
    {:else}
      <CardGrid bind:this={grid} gridId="library-grid" size={uiSettings.cardSizeLibrary}>
        {#each rows as row, i (row.rom_id ?? `x-${i}`)}
          <GameCard
            testId={`library-card-${row.rom_id ?? `x-${i}`}`}
            badgeId={row.rom_id ?? `x-${i}`}
            title={row.title}
            platform={row.platform}
            coverUrl={row.cover_small_path || null}
            installed={true}
            updateLabel={updates.labelFor(row.rom_id)}
            {cloudPlatforms}
            focused={i === focusIndex}
            onOpen={() => openDetails(row)}
            onPrimary={() => play(row)}
            onCloud={() => openDetails(row, 'save')}
            onHoverStart={() => hover.start(row.cover_large_path)}
            onHoverEnd={hover.end}
          />
        {/each}
      </CardGrid>
    {/if}
  </div>
</section>

{#if subject}
  {#key subject.romId}
    <Details
      {subject}
      initialCloudMode={detailsCloudMode}
      onClose={closeDetails}
      onLibraryPathUnset={() => {}}
    />
  {/key}
{/if}

<style>
  .library {
    display: flex;
    align-items: stretch;
    height: 100%;
    min-height: 0;
  }

  /* Design §5: the rail is 220px. */
  .rail {
    flex: 0 0 220px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 16px 8px;
    box-sizing: border-box;
    border-right: 1px solid var(--border);
    overflow-y: auto;
  }

  .rail-heading {
    margin: 12px 10px 4px;
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.1em;
    color: var(--text-muted);
  }

  .rail-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    font: inherit;
    font-size: 13px;
    text-align: left;
    padding: 7px 10px;
    border: none;
    border-radius: var(--r-row);
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
  }

  .rail-item:hover {
    background: var(--surface);
    color: var(--text-h);
  }

  .rail-item.active {
    background: var(--surface);
    color: var(--text-h);
    font-weight: 600;
  }

  .rail-label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .rail-count {
    flex: none;
    font-size: 11px;
    color: var(--text-muted);
  }

  .body {
    flex: 1 1 auto;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .toolbar {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 24px 0;
    width: 100%;
    max-width: 1920px;
    margin: 0 auto;
    box-sizing: border-box;
  }

  .search {
    flex: 1 1 240px;
    min-width: 120px;
    font: inherit;
    padding: 6px 10px;
    border-radius: var(--r-control);
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text-h);
  }

  .control {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    color: var(--text-muted);
  }

  .control select {
    font: inherit;
    font-size: 12px;
    padding: 5px 8px;
    border-radius: var(--r-control);
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text-h);
  }

  .empty {
    padding: 40px 24px;
    color: var(--text-muted);
    font-size: 14px;
  }

  .error {
    margin: 8px 24px 0;
    color: var(--danger);
    font-size: 12px;
  }
</style>
```

- [ ] **Step 3: Type-check and run the unit suite**

Run from `rewrite/app`: `npm run check && npx vitest run`
Expected: PASS.

- [ ] **Step 4: Confirm the Library group still passes**

Run from `rewrite/`: `scripts/e2e.sh images`
Expected: PASS. This is the group that asserts `library-section`, `library-card-101`'s `<img>`, and a card click opening Details — the three things this rewrite is most likely to have broken. Fix the component (not the spec) if a cover no longer loads or a card click no longer opens Details.

- [ ] **Step 5: Commit**

```bash
cd rewrite
git add app/src/lib/Library.svelte app/src/lib/library/selection.svelte.ts
git commit -m "rewrite: rebuild the Library view as a rail, toolbar and card grid"
```

---

### Task 5: The Server view — rail, toolbar, grid

**Files:**
- Rewrite: `app/src/lib/Server.svelte:1-177` (script and markup; the styles are replaced too)
- Create: `app/src/lib/server/header.ts`, `app/src/lib/server/header.test.ts`

**Interfaces:**
- Consumes: everything Task 4 consumes, plus `api.listPlatforms`, `api.listGames`, `api.installGame`, `isInstalled`, `refresh as refreshInstalled`, `session`, `retry`, `hostOf`, `fromSummary`.
- Produces, used by Tasks 6, 7 and 8:
  - `Server.svelte` keeps `export function handleNav(action)` and gains `export function focusSearch(): void`; it gains an `onOpenEmulators: () => void` prop (used in Task 6, passed from Shell in Task 7 — declare it now with a no-op default so Task 6 only fills in the chip).
  - `app/src/lib/server/header.ts`: `export function platformCountsLine(romCount: number, installedCount: number): string`, `export function emulatorChipLabel(name: string): string` (Task 6 adds `firmwareChipLabel` to the same module).
  - Test ids: `server-section` (root, kept), `server-rail`, rail entries `platform-btn-<id>` (kept) carrying `data-rail="server-rail-<id>"`, `server-search`, `server-sort` is **not** added (design §6 lists no sort for Server), `server-size`, `server-grid`, `server-empty`, `game-card-<id>` (kept), `installed-badge-<id>` (kept), `library-path-banner`/`library-path-input`/`library-path-save` (kept), `server-offline`/`server-retry` (kept).

- [ ] **Step 1: Write the failing header test**

Create `app/src/lib/server/header.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import { emulatorChipLabel, platformCountsLine } from './header';

describe('platformCountsLine', () => {
  it('reads as a sentence for the ordinary case', () => {
    expect(platformCountsLine(42, 7)).toBe('42 games · 7 installed');
  });
  it('singularises one game', () => {
    expect(platformCountsLine(1, 0)).toBe('1 game · 0 installed');
  });
  it('handles an empty platform without a stray dash', () => {
    expect(platformCountsLine(0, 0)).toBe('0 games · 0 installed');
  });
});

describe('emulatorChipLabel', () => {
  it('names the default emulator', () => {
    expect(emulatorChipLabel('RetroArch')).toBe('Emulator: RetroArch');
  });
  it('says so plainly when there is none, blank or whitespace', () => {
    expect(emulatorChipLabel('')).toBe('No default emulator');
    expect(emulatorChipLabel('   ')).toBe('No default emulator');
  });
});
```

- [ ] **Step 2: Run it to verify it fails**

Run from `rewrite/app`: `npx vitest run src/lib/server/header.test.ts`
Expected: FAIL — `Failed to resolve import "./header"`.

- [ ] **Step 3: Write `header.ts`**

Create `app/src/lib/server/header.ts`:

```ts
// The Server platform header's text (design §6). Pure; Task 6 adds the
// firmware chip's label to this module.

/** "42 games · 7 installed" — the header's counts line. */
export function platformCountsLine(romCount: number, installedCount: number): string {
  const games = romCount === 1 ? '1 game' : `${romCount} games`;
  return `${games} · ${installedCount} installed`;
}

/** The default-emulator chip's text. */
export function emulatorChipLabel(name: string): string {
  const trimmed = name.trim();
  return trimmed === '' ? 'No default emulator' : `Emulator: ${trimmed}`;
}
```

- [ ] **Step 4: Run it to verify it passes**

Run from `rewrite/app`: `npx vitest run src/lib/server/header.test.ts`
Expected: PASS.

- [ ] **Step 5: Rewrite `Server.svelte`**

Replace the whole of `app/src/lib/Server.svelte` with:

```svelte
<script lang="ts">
  import { api, type GameSummary, type Platform } from './api';
  import Details from './Details.svelte';
  import GameCard from './GameCard.svelte';
  import CardGrid from './CardGrid.svelte';
  import { fromSummary } from './details/subject';
  import type { CloudMode } from './details/cloud';
  import { moveFocus, type NavDirection } from './focus/grid';
  import { isInstalled, installed, refresh as refreshInstalled } from './stores/installed.svelte';
  import { updates } from './stores/updates.svelte';
  import { hostOf } from './shell';
  import { session, retry } from './stores/session.svelte';
  import { createHoverViewed } from './lastViewedHover';
  import { noteViewed } from './stores/lastViewed.svelte';
  import { cloudPlatformSet } from './cards/badges';
  import { CARD_SIZES, cardSizeLabel, type CardSize } from './cards/size';
  import { setCardSize, uiSettings } from './stores/uiSettings.svelte';
  import { titleContains } from './library/sort';
  import { platformCountsLine } from './server/header';

  let {
    active,
    onOpenEmulators = () => {},
  }: { active: boolean; onOpenEmulators?: () => void } = $props();

  let platforms = $state<Platform[]>([]);
  let games = $state<GameSummary[]>([]);
  let activePlatform = $state<number | null>(null);
  let search = $state('');
  let focusIndex = $state(0);
  let grid = $state<ReturnType<typeof CardGrid> | null>(null);
  let searchEl = $state<HTMLInputElement | null>(null);
  let detailsGame = $state<GameSummary | null>(null);
  let detailsCloudMode = $state<CloudMode>('overview');
  let installError = $state<string | null>(null);
  let cloudPlatforms = $state<ReadonlySet<string>>(new Set<string>());
  let defaultEmulators = $state<Record<string, string>>({});

  let libraryPathInput = $state('');
  let showLibraryBanner = $state(false);
  let libraryPathSaving = $state(false);
  let libraryPathError = $state<string | null>(null);

  let activePlatformRow = $derived(platforms.find((p) => p.id === activePlatform) ?? null);
  let activePlatformName = $derived(activePlatformRow?.name ?? '');
  let visible = $derived(games.filter((game) => titleContains(game.name, search)));
  let installedCount = $derived(
    games.filter((game) => isInstalled(game, activePlatformName)).length,
  );

  $effect(() => {
    if (!session.connected) return; // re-runs on reconnect: session.connected is read above
    api.listPlatforms().then((p) => {
      platforms = p;
      if (p.length && activePlatform === null) selectPlatform(p[0].id);
    });
    refreshInstalled();
    checkLibraryPath();
    api
      .getLaunchDefaults()
      .then((defaults) => {
        defaultEmulators = defaults.default_emulators;
        cloudPlatforms = cloudPlatformSet(defaults.default_emulators);
      })
      .catch(() => {
        // No defaults readable: no cloud badges and a "No default emulator"
        // chip. Both are honest fallbacks, neither is worth an error line.
      });
  });

  $effect(() => {
    if (focusIndex > visible.length - 1) focusIndex = Math.max(0, visible.length - 1);
  });

  async function checkLibraryPath() {
    try {
      const path = await api.getLibraryPath();
      showLibraryBanner = path.trim() === '';
    } catch {
      // Unreadable config: leave the banner as-is rather than nag with a
      // guess — install errors will surface the real problem if there is one.
    }
  }

  async function selectPlatform(id: number) {
    activePlatform = id;
    search = '';
    const g = await api.listGames(id);
    if (activePlatform !== id) return; // superseded by a newer selection
    games = g;
    focusIndex = 0;
  }

  function openDetails(game: GameSummary, mode: CloudMode = 'overview') {
    detailsCloudMode = mode;
    detailsGame = game;
    noteViewed(game.path_cover_large);
  }

  function closeDetails() {
    detailsGame = null;
    detailsCloudMode = 'overview';
  }

  /** Design §6: the hover primary is Install when not installed, Play when
   *  installed. Both report their failure inline rather than silently. */
  async function primary(game: GameSummary) {
    installError = null;
    try {
      if (isInstalled(game, activePlatformName)) await api.launchGame(game.id);
      else await api.installGame(game.id);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      installError = message;
      if (message.includes('library folder')) showLibraryBanner = true;
    }
  }

  // Design §3: a card becomes the background only after the pointer has
  // rested on it for more than half a second.
  const hover = createHoverViewed();

  /** Design §3: `Ctrl+F` focuses the current view's search box. */
  export function focusSearch() {
    searchEl?.focus();
    searchEl?.select();
  }

  export function handleNav(action: NavDirection | 'accept' | 'back') {
    if (action === 'back') {
      if (detailsGame) closeDetails();
      return;
    }
    if (action === 'accept') {
      if (!detailsGame) {
        const game = visible[focusIndex];
        if (game) openDetails(game);
      }
      return;
    }
    if (detailsGame) return; // grid navigation is suspended while the overlay is open
    focusIndex = moveFocus(focusIndex, action, grid?.columns() ?? 1, visible.length);
    grid?.element()?.children[focusIndex]?.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
  }

  function onKey(e: KeyboardEvent) {
    if (!active) return;
    if (document.activeElement === searchEl) return;
    const map: Record<string, NavDirection> = {
      ArrowUp: 'up', ArrowDown: 'down', ArrowLeft: 'left', ArrowRight: 'right',
    };
    const action = map[e.key];
    if (action) {
      e.preventDefault();
      handleNav(action);
    }
  }

  function onSizeChange(e: Event) {
    const size = (e.currentTarget as HTMLSelectElement).value as CardSize;
    setCardSize('server', size).catch(() => {
      // Applied for this session; a failed save is not worth an error line.
    });
  }

  async function saveLibraryPath() {
    libraryPathError = null;
    libraryPathSaving = true;
    try {
      await api.setLibraryPath(libraryPathInput.trim());
      showLibraryBanner = false;
    } catch (err) {
      libraryPathError = err instanceof Error ? err.message : String(err);
    } finally {
      libraryPathSaving = false;
    }
  }
</script>

<svelte:window onkeydown={onKey} />

<section data-testid="server-section" class="server">
  {#if showLibraryBanner}
    <div data-testid="library-path-banner" class="library-banner">
      <span>Set a library folder to install games.</span>
      <input
        data-testid="library-path-input"
        bind:value={libraryPathInput}
        placeholder="/path/to/library"
        disabled={libraryPathSaving}
      />
      <button data-testid="library-path-save" disabled={libraryPathSaving || !libraryPathInput.trim()} onclick={saveLibraryPath}>
        {libraryPathSaving ? 'Saving…' : 'Save'}
      </button>
      {#if libraryPathError}<span class="banner-error" role="alert">{libraryPathError}</span>{/if}
    </div>
  {/if}

  {#if !session.connected}
    <div data-testid="server-offline" class="offline">
      Not connected to {hostOf(session.serverUrl)}
      <button data-testid="server-retry" onclick={() => retry()}>Retry</button>
    </div>
  {:else}
    <div class="columns">
      <nav data-testid="server-rail" class="rail" aria-label="Server platforms">
        {#each platforms as p (p.id)}
          <button
            data-testid={`platform-btn-${p.id}`}
            data-rail={`server-rail-${p.id}`}
            class="rail-item"
            class:active={p.id === activePlatform}
            aria-current={p.id === activePlatform ? 'page' : undefined}
            onclick={() => selectPlatform(p.id)}
          >
            <span class="rail-label">{p.name}</span>
            <span class="rail-count">{p.rom_count}</span>
          </button>
        {/each}
      </nav>

      <div class="body">
        <header data-testid="server-platform-header" class="platform-header">
          <h2>{activePlatformName}</h2>
          <p data-testid="server-platform-counts" class="counts">
            {platformCountsLine(activePlatformRow?.rom_count ?? 0, installedCount)}
          </p>
          <div class="chips">
            <!-- Task 6 mounts the firmware and emulator chips here. -->
          </div>
        </header>

        <div class="toolbar">
          <input
            data-testid="server-search"
            class="search"
            type="search"
            placeholder="Search this platform"
            aria-label="Search this platform"
            bind:this={searchEl}
            bind:value={search}
          />
          <label class="control">
            <span>Size</span>
            <select data-testid="server-size" value={uiSettings.cardSizeServer} onchange={onSizeChange}>
              {#each CARD_SIZES as option (option)}
                <option value={option}>{cardSizeLabel(option)}</option>
              {/each}
            </select>
          </label>
        </div>

        {#if installError}
          <p data-testid="server-error" class="error" role="alert">{installError}</p>
        {/if}

        {#if visible.length === 0}
          <p data-testid="server-empty" class="empty">
            {search.trim() === ''
              ? 'This platform has no games'
              : `No games match “${search.trim()}”`}
          </p>
        {:else}
          <CardGrid bind:this={grid} gridId="server-grid" size={uiSettings.cardSizeServer}>
            {#each visible as game, i (game.id)}
              <GameCard
                testId={`game-card-${game.id}`}
                badgeId={game.id}
                title={game.name}
                platform={activePlatformName}
                coverUrl={game.path_cover_small}
                installed={isInstalled(game, activePlatformName)}
                updateLabel={updates.labelFor(game.id)}
                {cloudPlatforms}
                focused={i === focusIndex}
                onOpen={() => openDetails(game)}
                onPrimary={() => primary(game)}
                onCloud={() => openDetails(game, 'save')}
                onHoverStart={() => hover.start(game.path_cover_large)}
                onHoverEnd={hover.end}
              />
            {/each}
          </CardGrid>
        {/if}
      </div>
    </div>
  {/if}

  {#if detailsGame}
    {#key detailsGame.id}
      <Details
        subject={fromSummary(detailsGame, activePlatformName)}
        initialCloudMode={detailsCloudMode}
        onClose={closeDetails}
        onLibraryPathUnset={() => { showLibraryBanner = true; }}
      />
    {/key}
  {/if}
</section>

<style>
  .server {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  .columns {
    display: flex;
    align-items: stretch;
    flex: 1 1 auto;
    min-height: 0;
  }

  .rail {
    flex: 0 0 220px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 16px 8px;
    box-sizing: border-box;
    border-right: 1px solid var(--border);
    overflow-y: auto;
  }

  .rail-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    font: inherit;
    font-size: 13px;
    text-align: left;
    padding: 7px 10px;
    border: none;
    border-radius: var(--r-row);
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
  }

  .rail-item:hover {
    background: var(--surface);
    color: var(--text-h);
  }

  .rail-item.active {
    background: var(--surface);
    color: var(--text-h);
    font-weight: 600;
  }

  .rail-label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .rail-count {
    flex: none;
    font-size: 11px;
    color: var(--text-muted);
  }

  .body {
    flex: 1 1 auto;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .platform-header {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 8px 14px;
    padding: 16px 24px 0;
    width: 100%;
    max-width: 1920px;
    margin: 0 auto;
    box-sizing: border-box;
  }

  .platform-header h2 {
    margin: 0;
    font-size: 20px;
    font-weight: 600;
    color: var(--text-h);
  }

  .counts {
    margin: 0;
    font-size: 12px;
    color: var(--text-muted);
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin-left: auto;
  }

  .toolbar {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 24px 0;
    width: 100%;
    max-width: 1920px;
    margin: 0 auto;
    box-sizing: border-box;
  }

  .search {
    flex: 1 1 240px;
    min-width: 120px;
    font: inherit;
    padding: 6px 10px;
    border-radius: var(--r-control);
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text-h);
  }

  .control {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    color: var(--text-muted);
  }

  .control select {
    font: inherit;
    font-size: 12px;
    padding: 5px 8px;
    border-radius: var(--r-control);
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text-h);
  }

  .empty {
    padding: 40px 24px;
    color: var(--text-muted);
    font-size: 14px;
  }

  .error {
    margin: 8px 24px 0;
    color: var(--danger);
    font-size: 12px;
  }

  .library-banner {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 10px;
    padding: 10px 24px;
    background: var(--surface);
    color: var(--text-h);
    font-size: 13px;
  }

  .library-banner input {
    flex: 1 1 240px;
    min-width: 160px;
    font: inherit;
    padding: 6px 8px;
    border-radius: var(--r-control);
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text-h);
  }

  .library-banner button {
    font: inherit;
    padding: 6px 14px;
    border-radius: var(--r-control);
    border: none;
    background: var(--primary);
    color: #fff;
    cursor: pointer;
    white-space: nowrap;
  }

  .library-banner button:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .banner-error {
    color: var(--danger);
    flex-basis: 100%;
  }

  .offline {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 40px 24px;
    color: var(--text);
    font-size: 14px;
  }

  .offline button {
    font: inherit;
    padding: 6px 14px;
    border-radius: var(--r-row);
    border: none;
    background: var(--primary);
    color: #fff;
    cursor: pointer;
  }
</style>
```

The unused `installed` import is deliberate in the list above — remove it if `npm run check` flags it; `isInstalled` reads the store internally.

- [ ] **Step 6: Type-check and run the unit suite**

Run from `rewrite/app`: `npm run check && npx vitest run`
Expected: PASS. Remove any import svelte-check reports as unused.

- [ ] **Step 7: Confirm the two heaviest Server groups still pass**

Run from `rewrite/`: `scripts/e2e.sh library install`
Expected: PASS. `library` proves the rail entries (`platform-btn-<id>`), the card ids and the `.focused` class survive; `install` proves a card click still opens Details and that `installed-badge-<id>` still appears after an install.

- [ ] **Step 8: Commit**

```bash
cd rewrite
git add app/src/lib/Server.svelte app/src/lib/server
git commit -m "rewrite: rebuild the Server view as a rail, header and card grid"
```

---

### Task 6: The Server platform header's firmware and emulator chips

**Files:**
- Modify: `app/src-tauri/src/firmware_service.rs:199-253` (extract `spawn_for_platform` from `spawn_for_game`)
- Modify: `app/src-tauri/src/commands.rs` (append the two commands and their test module)
- Modify: `app/src-tauri/src/lib.rs:325-330` (handler registration)
- Modify: `app/src/lib/api.ts` (type + two wrappers)
- Modify: `app/src/lib/server/header.ts`, `app/src/lib/server/header.test.ts` (the firmware chip's label)
- Modify: `app/src/lib/Server.svelte` (the `.chips` block from Task 5)

**Interfaces:**
- Consumes: `grid_core::romm::RommClient::firmware(platform_id)`; `state.session`, `state.install`, `state.firmware` on `AppState`; `default_entry_for_platform` and `load_profiles` inside `firmware_service.rs`; `platformCountsLine`, `emulatorChipLabel` (Task 5).
- Produces, used by Tasks 7 and 8:
  - Rust: `pub struct PlatformFirmwareStatus { pub file_count: u32, pub has_default_emulator: bool }` (Serialize) in `app/src-tauri/src/commands.rs`; commands `platform_firmware_status(platform_id: i64) -> PlatformFirmwareStatus` and `install_firmware_for_platform(platform_id: i64, platform: String) -> ()`.
  - `FirmwareService::spawn_for_platform(self: &Arc<Self>, session: Arc<SessionManager>, platform: String, platform_id: i64, trigger: FirmwareTrigger)`.
  - TS: `export type PlatformFirmwareStatus = { file_count: number; has_default_emulator: boolean }`, `api.platformFirmwareStatus(platformId)`, `api.installFirmwareForPlatform(platformId, platform)`.
  - `lib/server/header.ts`: `export function firmwareChipLabel(status: PlatformFirmwareStatus | null): string`, `export function firmwareInstallable(status: PlatformFirmwareStatus | null): boolean`.
  - Test ids: `server-firmware-chip`, `server-firmware-install`, `server-emulator-chip`.

- [ ] **Step 1: Write the failing chip-label test**

Append to `app/src/lib/server/header.test.ts`:

```ts
import { firmwareChipLabel, firmwareInstallable } from './header';

describe('firmwareChipLabel', () => {
  it('says nothing is known while the status is still loading', () => {
    expect(firmwareChipLabel(null)).toBe('Firmware: checking…');
  });
  it('says plainly when the server offers none', () => {
    expect(firmwareChipLabel({ file_count: 0, has_default_emulator: true })).toBe(
      'No server firmware',
    );
  });
  it('counts the files, singularising one', () => {
    expect(firmwareChipLabel({ file_count: 1, has_default_emulator: true })).toBe(
      'Firmware: 1 file',
    );
    expect(firmwareChipLabel({ file_count: 4, has_default_emulator: true })).toBe(
      'Firmware: 4 files',
    );
  });
  it('names the blocker when there is nowhere to put the firmware', () => {
    expect(firmwareChipLabel({ file_count: 4, has_default_emulator: false })).toBe(
      'Firmware: 4 files — no default emulator',
    );
  });
});

describe('firmwareInstallable', () => {
  it('needs both files on the server and somewhere to put them', () => {
    expect(firmwareInstallable({ file_count: 4, has_default_emulator: true })).toBe(true);
    expect(firmwareInstallable({ file_count: 0, has_default_emulator: true })).toBe(false);
    expect(firmwareInstallable({ file_count: 4, has_default_emulator: false })).toBe(false);
    expect(firmwareInstallable(null)).toBe(false);
  });
});
```

- [ ] **Step 2: Run it to verify it fails**

Run from `rewrite/app`: `npx vitest run src/lib/server/header.test.ts`
Expected: FAIL — `No "firmwareChipLabel" export is defined on the "./header" module`.

- [ ] **Step 3: Add the firmware label helpers**

Append to `app/src/lib/server/header.ts`:

```ts
import type { PlatformFirmwareStatus } from '../api';

/**
 * The `server-firmware-chip` text (design §6: "firmware status chip with an
 * Install action when the server offers firmware"). `null` is the state
 * before the status command answers — named rather than blank so the chip
 * does not appear and then jump.
 */
export function firmwareChipLabel(status: PlatformFirmwareStatus | null): string {
  if (status === null) return 'Firmware: checking…';
  if (status.file_count === 0) return 'No server firmware';
  const files = status.file_count === 1 ? '1 file' : `${status.file_count} files`;
  if (!status.has_default_emulator) return `Firmware: ${files} — no default emulator`;
  return `Firmware: ${files}`;
}

/** Whether the chip offers its Install action: the server has firmware AND
 *  the platform has an emulator whose profile says where it goes. */
export function firmwareInstallable(status: PlatformFirmwareStatus | null): boolean {
  return status !== null && status.file_count > 0 && status.has_default_emulator;
}
```

- [ ] **Step 4: Write the failing Rust test**

Append a NEW test module at the tail of `app/src-tauri/src/commands.rs`:

```rust
#[cfg(test)]
mod platform_firmware_tests {
    use super::*;

    #[test]
    fn a_platform_with_no_files_and_no_emulator_offers_nothing() {
        let status = PlatformFirmwareStatus {
            file_count: 0,
            has_default_emulator: false,
        };
        // The DTO is the whole contract: the frontend decides what to show.
        // Serialization is asserted because the field names are the API.
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, r#"{"file_count":0,"has_default_emulator":false}"#);
    }

    #[test]
    fn a_platform_with_files_and_an_emulator_serializes_both_flags() {
        let status = PlatformFirmwareStatus {
            file_count: 4,
            has_default_emulator: true,
        };
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, r#"{"file_count":4,"has_default_emulator":true}"#);
    }
}
```

- [ ] **Step 5: Run it to verify it fails**

Run from `rewrite/`: `cargo test -p app platform_firmware_tests`
Expected: FAIL to compile — `cannot find struct 'PlatformFirmwareStatus' in this scope`.

- [ ] **Step 6: Add `spawn_for_platform` to the firmware service**

In `app/src-tauri/src/firmware_service.rs`, add this method directly before `spawn_for_game`, and change `spawn_for_game`'s body to delegate to it:

```rust
    /// Installs a platform's server firmware into the platform's default
    /// emulator. The whole of [`Self::spawn_for_game`]'s body, minus the
    /// registry row: `spawn_for_game` only ever read `record.platform` off
    /// that row, so the two now share one implementation.
    ///
    /// Returns immediately, and silently, whenever there is nothing to do:
    /// no connected session, no default emulator for that platform, no
    /// config entry by that name, a [`FirmwareTrigger::Launch`] pass for a
    /// `(directory, platform)` pair that already completed one (D19), or a
    /// pass already running for that emulator directory. Never fails and
    /// never blocks the caller.
    pub fn spawn_for_platform(
        self: &Arc<Self>,
        session: Arc<SessionManager>,
        platform: String,
        platform_id: i64,
        trigger: FirmwareTrigger,
    ) {
        let Some(client) = session.client() else {
            return;
        };
        let config_path = Config::default_path();
        let Ok(config) = Config::load(&config_path) else {
            return;
        };
        let profiles = load_profiles();
        let Some(entry) = default_entry_for_platform(&config, &platform, profiles) else {
            return;
        };
        let dir = emulator_dir_of(entry);
        // D19, before the in-flight claim: a launch pass for a directory
        // that already completed one this process is a pure re-download.
        if !self.should_run(&dir, platform_id, trigger) {
            return;
        }
        if !self.try_begin(&dir, Some(platform_id)) {
            return;
        }
        let guard = FirmwareGuard::new(self.clone(), dir.clone(), Some(platform_id));
        let service = self.clone();
        let config_dir = config_dir_of(&config_path);
        tauri::async_runtime::spawn(async move {
            let _guard = guard;
            let ctx = GameFirmwareContext {
                platform: &platform,
                platform_id,
                config: &config,
                profiles,
                config_dir: &config_dir,
            };
            let warnings = install_for_game(&client, &ctx).await;
            if !warnings.is_empty() {
                // D14: warnings are logged, never surfaced as a dialog —
                // the install already succeeded. Local paths and platform
                // ids only; grid-core builds no URL into them.
                tracing::warn!("firmware for {platform}: {warnings}");
            }
            // Completion means "the pass finished", warnings included: a
            // warning is a per-file outcome, and re-downloading the whole
            // set on the next launch would not change it.
            service.mark_completed(&dir, platform_id);
        });
    }
```

and replace `spawn_for_game`'s body (everything after its signature) with:

```rust
        let Some(platform_id) = platform_id_for(&install.platform_ids(), &record.platform) else {
            return;
        };
        self.spawn_for_platform(session, record.platform.clone(), platform_id, trigger);
```

`spawn_for_game` keeps its `install: Arc<InstallService>` parameter — it is what resolves the platform id. If the compiler now reports `install` unused beyond that line, it is still used; if `record` triggers an unused-clone warning, drop the `.clone()`.

- [ ] **Step 7: Add the two commands**

In `app/src-tauri/src/commands.rs`, add the DTO and the commands after `open_server_page`:

```rust
/// What the Server platform header's firmware chip needs (design §6).
/// Deliberately two plain flags rather than a rendered sentence: the chip's
/// wording is the frontend's (`lib/server/header.ts`), and a count is what
/// the backend can honestly report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlatformFirmwareStatus {
    /// How many firmware files the server lists for this platform.
    pub file_count: u32,
    /// Whether the platform has a default emulator — i.e. somewhere for
    /// that firmware to be installed to. Without one there is nothing to
    /// install into, so the chip offers no action.
    pub has_default_emulator: bool,
}

/// `GET /api/firmware?platform_id=<id>` plus the local default-emulator
/// check. Read-only: nothing is downloaded here.
#[tauri::command]
pub async fn platform_firmware_status(
    state: State<'_, AppState>,
    platform_id: i64,
    platform: String,
) -> Result<PlatformFirmwareStatus, String> {
    let file_count = match state.session.client() {
        // Offline: report "no firmware" rather than an error. The chip is
        // an affordance, not a task, and the Server view is already
        // showing its own offline state when this can happen.
        None => 0,
        Some(client) => client.firmware(platform_id).await.map_err(err)?.len() as u32,
    };
    let has_default_emulator = tokio::task::spawn_blocking(move || {
        let Ok(config) = Config::load(&Config::default_path()) else {
            return false;
        };
        let profiles = grid_core::launch::profiles::load_profiles();
        crate::firmware_service::default_entry_for_platform(&config, &platform, profiles).is_some()
    })
    .await
    .map_err(|e| format!("platform_firmware_status did not finish: {e}"))?;
    Ok(PlatformFirmwareStatus {
        file_count,
        has_default_emulator,
    })
}

/// The firmware chip's Install action. Fire-and-forget, exactly like the
/// per-game and per-emulator triggers: the pass runs in the background,
/// logs its warnings, and never reports back through this command.
#[tauri::command]
pub fn install_firmware_for_platform(
    state: State<'_, AppState>,
    platform_id: i64,
    platform: String,
) -> Result<(), String> {
    state.firmware.spawn_for_platform(
        state.session.clone(),
        platform,
        platform_id,
        crate::firmware_service::FirmwareTrigger::Install,
    );
    Ok(())
}
```

`default_entry_for_platform` is currently private to `firmware_service.rs`; change its declaration there from `fn default_entry_for_platform` to `pub(crate) fn default_entry_for_platform`.

- [ ] **Step 8: Register both commands**

In `app/src-tauri/src/lib.rs`, add to the `generate_handler!` list after `commands::open_server_page,`:

```rust
            commands::platform_firmware_status,
            commands::install_firmware_for_platform,
```

- [ ] **Step 9: Run the Rust tests**

Run from `rewrite/`: `cargo test -p app`
Expected: PASS, including `platform_firmware_tests`.

- [ ] **Step 10: Add the TypeScript wrappers**

In `app/src/lib/api.ts`, add the type beside `Rpcs3FirmwareStatus`:

```ts
/// The Server platform header's firmware chip input (design §6). See
/// `app/src-tauri/src/commands.rs`'s `PlatformFirmwareStatus`.
export type PlatformFirmwareStatus = { file_count: number; has_default_emulator: boolean };
```

and the two wrappers beside `installPs3Firmware`:

```ts
  platformFirmwareStatus: (platformId: number, platform: string) =>
    invoke<PlatformFirmwareStatus>('platform_firmware_status', { platformId, platform }),
  installFirmwareForPlatform: (platformId: number, platform: string) =>
    invoke<void>('install_firmware_for_platform', { platformId, platform }),
```

- [ ] **Step 11: Mount both chips in the Server header**

In `app/src/lib/Server.svelte`, add to the imports:

```svelte
  import { emulatorChipLabel, firmwareChipLabel, firmwareInstallable, platformCountsLine } from './server/header';
  import type { PlatformFirmwareStatus } from './api';
```

(replacing the existing `platformCountsLine`-only import), add the state and the effect after `defaultEmulators`:

```svelte
  let firmware = $state<PlatformFirmwareStatus | null>(null);
  let firmwareRequested = $state(false);

  // One status call per platform selection. Reset to `null` first so the
  // chip reads "checking…" rather than the previous platform's answer.
  $effect(() => {
    const id = activePlatform;
    const name = activePlatformName;
    if (id === null || name === '') return;
    firmware = null;
    firmwareRequested = false;
    api
      .platformFirmwareStatus(id, name)
      .then((status) => {
        if (activePlatform === id) firmware = status;
      })
      .catch(() => {
        // Unreachable or refused: leave the chip at "checking…" rather than
        // claiming the server has no firmware when we simply do not know.
      });
  });

  function installFirmware() {
    const id = activePlatform;
    if (id === null) return;
    firmwareRequested = true;
    api.installFirmwareForPlatform(id, activePlatformName).catch(() => {
      firmwareRequested = false;
    });
  }
```

and replace the `.chips` placeholder with:

```svelte
          <div class="chips">
            <span data-testid="server-firmware-chip" class="chip">
              {firmwareChipLabel(firmware)}
              {#if firmwareInstallable(firmware)}
                <button data-testid="server-firmware-install" onclick={installFirmware} disabled={firmwareRequested}>
                  {firmwareRequested ? 'Installing…' : 'Install'}
                </button>
              {/if}
            </span>
            <button data-testid="server-emulator-chip" class="chip link" onclick={onOpenEmulators}>
              {emulatorChipLabel(defaultEmulators[activePlatformName] ?? '')}
            </button>
          </div>
```

Append the chip styles:

```svelte
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font: inherit;
    font-size: 11px;
    padding: 4px 10px;
    border-radius: var(--r-chip);
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--text-muted);
  }

  .chip.link {
    cursor: pointer;
  }

  .chip.link:hover {
    color: var(--text-h);
    border-color: var(--primary);
  }

  .chip button {
    font: inherit;
    font-size: 11px;
    padding: 2px 8px;
    border: none;
    border-radius: var(--r-pill);
    background: var(--primary);
    color: #fff;
    cursor: pointer;
  }

  .chip button:disabled {
    opacity: 0.6;
    cursor: default;
  }
```

- [ ] **Step 12: Run the checks**

Run from `rewrite/app`: `npm run check && npx vitest run`
Expected: PASS.

- [ ] **Step 13: Confirm the firmware group still passes**

Run from `rewrite/`: `scripts/e2e.sh firmware`
Expected: PASS. The chip only reads the server; the per-game firmware path this group exercises must be unchanged by the `spawn_for_platform` extraction.

- [ ] **Step 14: Format, lint and commit**

```bash
cd rewrite
cargo fmt
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p app --all-targets --features e2e -- -D warnings
cargo test --workspace
git add app/src-tauri/src/firmware_service.rs app/src-tauri/src/commands.rs \
        app/src-tauri/src/lib.rs app/src/lib/api.ts app/src/lib/server \
        app/src/lib/Server.svelte
git commit -m "rewrite: add the Server platform header firmware and emulator chips"
```

---

### Task 7: `Ctrl+F` focuses the active view's search

**Files:**
- Modify: `app/src/lib/shell.ts` (a new pure predicate + its test)
- Modify: `app/src/lib/shell.test.ts`
- Modify: `app/src/lib/Shell.svelte:43-66` (`chordBlocked` / `onKeydown`), `:187-189` (the Server mount)

**Interfaces:**
- Consumes: `Library.focusSearch()` and `Server.focusSearch()` (Tasks 4 and 5); `Shell.show(view)`.
- Produces, used by Task 8:
  - `app/src/lib/shell.ts`: `export function isSearchChord(e: { key: string; ctrlKey: boolean; metaKey: boolean; altKey: boolean; shiftKey: boolean }): boolean`.
  - `Server.svelte` receives `onOpenEmulators={() => (view = 'emulators')}` from Shell.

- [ ] **Step 1: Write the failing test**

Append to `app/src/lib/shell.test.ts` (and add `isSearchChord` to its import from `./shell`):

```ts
describe('isSearchChord (design section 3)', () => {
  const chord = (over: Partial<KeyboardEvent>) => ({
    key: 'f', ctrlKey: false, metaKey: false, altKey: false, shiftKey: false, ...over,
  });

  it('matches Ctrl+F and Cmd+F, in either case', () => {
    expect(isSearchChord(chord({ ctrlKey: true }))).toBe(true);
    expect(isSearchChord(chord({ metaKey: true }))).toBe(true);
    expect(isSearchChord(chord({ key: 'F', ctrlKey: true }))).toBe(true);
  });
  it('ignores the bare key', () => {
    expect(isSearchChord(chord({}))).toBe(false);
  });
  it('ignores chords carrying Alt or Shift, which belong to other bindings', () => {
    expect(isSearchChord(chord({ ctrlKey: true, altKey: true }))).toBe(false);
    expect(isSearchChord(chord({ ctrlKey: true, shiftKey: true }))).toBe(false);
  });
  it('ignores every other key', () => {
    expect(isSearchChord(chord({ key: 'g', ctrlKey: true }))).toBe(false);
    expect(isSearchChord(chord({ key: '1', ctrlKey: true }))).toBe(false);
  });
});
```

- [ ] **Step 2: Run it to verify it fails**

Run from `rewrite/app`: `npx vitest run src/lib/shell.test.ts`
Expected: FAIL — `isSearchChord is not a function`.

- [ ] **Step 3: Add `isSearchChord`**

Append to `app/src/lib/shell.ts`:

```ts
/**
 * Design §3: "`Ctrl+F` focuses the current view's search". Meta is accepted
 * alongside Ctrl so the same accelerator works on macOS, and Alt/Shift
 * variants are left alone so this never steals another binding.
 */
export function isSearchChord(e: {
  key: string;
  ctrlKey: boolean;
  metaKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
}): boolean {
  if (!(e.ctrlKey || e.metaKey) || e.altKey || e.shiftKey) return false;
  return e.key.toLowerCase() === 'f';
}
```

- [ ] **Step 4: Run it to verify it passes**

Run from `rewrite/app`: `npx vitest run src/lib/shell.test.ts`
Expected: PASS.

- [ ] **Step 5: Dispatch the chord in `Shell.svelte`**

In `app/src/lib/Shell.svelte`, add `isSearchChord` to the `./shell` import, then insert this in `onKeydown` directly after the `Escape` block and before the `Ctrl+1..5` guard:

```svelte
    // Design §3: Ctrl+F focuses the active view's search box. Ignored while
    // a dialog owns the screen (the details popup has its own focus) and
    // while the caret already sits in a text control, where Ctrl+F may be
    // an editor chord.
    if (isSearchChord(e)) {
      if (chordBlocked()) return;
      if (view === 'library') {
        e.preventDefault();
        library?.focusSearch();
      } else if (view === 'server') {
        e.preventDefault();
        server?.focusSearch();
      }
      return;
    }
```

`chordBlocked()` already returns `true` when `document.querySelector('[role="dialog"]')` matches — the Details popup's panel carries `role="dialog"` (`Details.svelte:314`) — and when focus is in an `INPUT`/`TEXTAREA`/`SELECT`, which includes the search box itself.

- [ ] **Step 6: Wire the Server view's emulator chip**

In the same file, change the Server mount to pass the navigation callback:

```svelte
<div data-testid="server-view" class="view" hidden={view !== 'server'}>
  <Server active={view === 'server'} onOpenEmulators={() => (view = 'emulators')} bind:this={server} />
</div>
```

- [ ] **Step 7: Run the checks**

Run from `rewrite/app`: `npm run check && npx vitest run`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
cd rewrite
git add app/src/lib/shell.ts app/src/lib/shell.test.ts app/src/lib/Shell.svelte
git commit -m "rewrite: focus the active view's search with Ctrl+F"
```

---

### Task 8: E2E — the new structure, and cases for what it added

**Files:**
- Modify: `e2e/specs/library.spec.ts`, `e2e/specs/updates.spec.ts`, and whichever other spec files still fail
- Create: `e2e/specs/library-grid.spec.ts` (added to the `library` stage group)
- Modify: `rewrite/scripts/e2e.sh:48-64` (the `library` group's spec list)

**Interfaces:**
- Consumes: every id Tasks 3–7 produced — `library-rail`, `library-rail-all`, `library-rail-recent`, `library-rail-updates`, `library-rail-platform-<slug>`, `library-rail-count-<key>`, `library-search`, `library-sort`, `library-size`, `library-grid`, `library-card-<id>`, `library-empty`, `library-update-badge-<id>`, `server-rail`, `platform-btn-<id>`, `server-platform-header`, `server-platform-counts`, `server-firmware-chip`, `server-emulator-chip`, `server-search`, `server-size`, `server-grid`, `game-card-<id>`, `installed-badge-<id>`, `card-primary-<id>`, `card-details-<id>`, `card-cloud-<id>`, `card-more-<id>`.
- Produces: nothing new. This task's deliverable is a green suite.

- [ ] **Step 1: Fix the one text assertion the redesign changes**

D-UI-9 renames the update tag from "Update Available" to the word `UPDATE`. In `e2e/specs/updates.spec.ts:179`, change

```ts
    await expect($(testId('library-update-badge-801'))).toHaveText('Update Available');
```

to

```ts
    // D-UI-9: the badge is a corner tag now, not a caption line.
    await expect($(testId('library-update-badge-801'))).toHaveText('UPDATE');
```

Every other `library-update-badge-<id>` assertion in that file is existence-only and needs no change.

- [ ] **Step 2: Add the new Library/Server spec**

Create `e2e/specs/library-grid.spec.ts`:

```ts
import {
  APP_START_TIMEOUT,
  FIXTURE_TOKEN,
  mockUrl,
  TRANSITION_TIMEOUT,
} from '../helpers/env.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/**
 * Stage `library`, second spec: the redesigned rails, toolbars and card
 * chrome (design §5, §6, D-UI-2/3/9). Runs after `library.spec.ts` in the
 * same group, so the app is already connected with the base fixtures —
 * platform 1 (SNES) holds roms 101/102/103, platform 2 (Arcade) holds 201
 * and 301. Nothing is installed in this group, so the Library grid is
 * empty and its empty states are what there is to assert.
 */
describe('library and server chrome', () => {
  before(async () => {
    await $(testId('connect-server-url')).waitForExist({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the connect form never appeared — the app did not reach a usable state',
    });
    // The group's first spec already connected; this process re-uses the
    // stored session, so the form may never appear. Tolerate both.
    if (await $(testId('connect-submit')).isExisting()) {
      await $(testId('connect-server-url')).setValue(mockUrl());
      await $(testId('connect-secret')).setValue(FIXTURE_TOKEN);
      await $(testId('connect-submit')).click();
    }
    await $(testId('platform-btn-1')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the Server rail never rendered a platform entry after connecting',
    });
  });

  it('gives each Server rail entry both its old test id and its new rail id', async () => {
    // Design §11 adds `server-rail-<id>`; every existing spec still clicks
    // `platform-btn-<id>`. Both live on one element.
    await expect($(testId('platform-btn-1'))).toHaveAttribute('data-rail', 'server-rail-1');
    await expect($(testId('platform-btn-2'))).toHaveAttribute('data-rail', 'server-rail-2');
  });

  it('heads the selected platform with its name and counts', async () => {
    await $(testId('platform-btn-1')).click();
    await $(testId('game-card-101')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('server-platform-header'))).toBeDisplayed();
    await expect($(testId('server-platform-counts'))).toHaveText('3 games · 0 installed');
    await expect($(testId('server-firmware-chip'))).toBeDisplayed();
    await expect($(testId('server-emulator-chip'))).toBeDisplayed();
  });

  it('filters the Server grid client-side from its search box', async () => {
    await $(testId('platform-btn-1')).click();
    await $(testId('game-card-101')).waitForExist({ timeout: TRANSITION_TIMEOUT });

    await $(testId('server-search')).setValue('chrono');
    await $(testId('game-card-101')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      reverse: true,
      timeoutMsg: 'Super Mario World survived a search for "chrono"',
    });
    await expect($(testId('game-card-102'))).toExist();

    await $(testId('server-search')).setValue('');
    await $(testId('game-card-101')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'clearing the search never restored the full platform list',
    });
  });

  it('Ctrl+F focuses the active view search box', async () => {
    await $(testId('nav-server')).click();
    await browser.keys(['Control', 'f']);
    const focused = await browser.execute(
      () => document.activeElement?.getAttribute('data-testid') ?? '',
    );
    expect(focused).toBe('server-search');

    await $(testId('nav-library')).click();
    await browser.keys(['Control', 'f']);
    const libraryFocused = await browser.execute(
      () => document.activeElement?.getAttribute('data-testid') ?? '',
    );
    expect(libraryFocused).toBe('library-search');
  });

  it('shows the Library rail with its three fixed entries and their counts', async () => {
    await $(testId('nav-library')).click();
    await $(testId('library-rail')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('library-rail-all'))).toExist();
    await expect($(testId('library-rail-recent'))).toExist();
    await expect($(testId('library-rail-updates'))).toExist();
    await expect($(testId('library-rail-count-all'))).toHaveText('0');
  });

  it('gives each Library rail entry its own empty state, verbatim', async () => {
    await $(testId('nav-library')).click();
    await $(testId('library-rail-all')).click();
    await expect($(testId('library-empty'))).toHaveText('No games installed');

    await $(testId('library-rail-recent')).click();
    await expect($(testId('library-empty'))).toHaveText('Nothing played in the last 30 days');

    await $(testId('library-rail-updates')).click();
    await expect($(testId('library-empty'))).toHaveText('Everything is up to date');

    await $(testId('library-rail-all')).click();
  });

  it('remembers each grid card size across a view switch', async () => {
    await $(testId('nav-server')).click();
    await $(testId('server-size')).selectByAttribute('value', 'large');
    await $(testId('nav-library')).click();
    await $(testId('library-size')).selectByAttribute('value', 'small');

    await $(testId('nav-server')).click();
    await expect($(testId('server-size'))).toHaveValue('large');
    await $(testId('nav-library')).click();
    await expect($(testId('library-size'))).toHaveValue('small');
  });
});
```

- [ ] **Step 3: Add the spec to the `library` stage group**

In `rewrite/scripts/e2e.sh`, change the `library` line of `STAGE_GROUPS` from

```bash
  "library:specs/library.spec.ts"
```

to

```bash
  "library:specs/library.spec.ts specs/library-grid.spec.ts"
```

The two specs share one data directory and one mock server, exactly as the `install` and `images` pairs already do.

- [ ] **Step 4: Prove the card-centre click is still safe**

Add this case to `e2e/specs/library.spec.ts`, after the "ArrowRight moves the focused card" test:

```ts
  // The hover overlay (D-UI-9) raises a centred Install button and a bottom
  // action row over the cover. WebdriverIO clicks an element's CENTRE, so
  // if either sat under that point every `game-card-<id>` click in the
  // suite would install instead of opening Details. `cards/size.ts` keeps
  // the band around the card root's centre free; this is that contract's
  // end-to-end check.
  it('opens Details, not the hover action, when the card itself is clicked', async () => {
    await $(testId('platform-btn-1')).click();
    await $(testId('game-card-103')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await $(testId('game-card-103')).click();

    await $(testId('details-panel')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'clicking the card centre did not open the details popup',
    });
    // Nothing was queued: the click never reached the Install action.
    expect(await $(testId('details-install')).isExisting()).toBe(true);
    await $(testId('details-close')).click();
  });
```

- [ ] **Step 5: Run the two most affected groups**

Run from `rewrite/`: `scripts/e2e.sh library updates`
Expected: PASS.

- [ ] **Step 6: Run every group**

Run from `rewrite/`: `scripts/e2e.sh`
Expected: all 15 groups PASS (`connect`, `connect-restore`, `library`, `install`, `downloads`, `emulators`, `launch`, `emulator-catalog`, `cloud-saves`, `images`, `ps3-install`, `content`, `native`, `firmware`, `updates`).

- [ ] **Step 7: Triage each failure before touching anything**

For every failing group, decide which of two kinds it is, and say which in the commit message:

- **Test breakage** — the spec drives the old grid: it waits for a caption element that is now a badge, asserts a class the card no longer carries, or clicks a control the toolbar moved. Fix the spec.
- **Product breakage** — the app genuinely stopped doing something (a cover never loads, an install never starts, a card click no longer opens Details). Do NOT weaken the spec. Fix the component.

Two traps specific to this plan:
- A card's `<img>` is now nested one level deeper (`.card > .cover > img`). The specs' `${testId('game-card-101')} img` selectors are descendant selectors, so they still match — but a spec using a **child** combinator would not. Search for `> img` before assuming.
- `installed-badge-<id>` moved from a text pill to a bare dot with no text. Any `toHaveText` on it must become an existence check.

- [ ] **Step 8: Re-run until green**

Run from `rewrite/`: `scripts/e2e.sh <the groups you changed>`, then `scripts/e2e.sh` once more in full.
Expected: all 15 groups PASS in one uninterrupted run.

- [ ] **Step 9: Commit**

```bash
cd rewrite
git add e2e/ scripts/e2e.sh
git commit -m "rewrite: cover the redesigned Library and Server views in E2E"
```

(If step 7 required a component fix, add that file to the same commit and name the behavior in the subject.)

---

### Task 9: Documentation

**Files:**
- Modify: `SPEC.md:29-30` (the Library and Server bullets)
- Modify: `rewrite/README.md` (the residual manual checklist)
- Modify: `docs/porting/03-library-install.md` (a `last_played_at` note in the registry section)

**Interfaces:**
- Consumes: nothing. Documentation only.
- Produces: nothing code reads.

- [ ] **Step 1: Update the SPEC.md Library and Server bullets**

In `SPEC.md`, replace the `- **Library** …` and `- **Server** …` bullets (lines 29-30) with:

```markdown
- **Library** contains a 220px filter rail — All games, Recent (played in the last 30
  days), Updates, then each installed platform, every entry with a count — beside a
  toolbar (search by title, sort by Recently played / Recently installed / Title /
  Platform, card size Small / Medium / Large) and one grid of cover cards. Each rail
  entry has its own empty state. Clicking a card opens the details sub view; the hover
  Play launches directly. The card size is remembered as `ui.card_size_library`.
- **Server** mirrors Library: a rail of the server's platforms with ROM counts, a
  platform header (name, ROM count, installed count, a firmware chip with an Install
  action when the server offers firmware, and the platform's default emulator chip
  linking to the Emulators view), a search box that filters the loaded platform list,
  and a grid whose not-installed cards render dimmed until hovered. The hover action is
  Install for a game that is not installed and Play for one that is. The card size is
  remembered as `ui.card_size_server`. Downloads and installs are queued and handled in
  the background.
```

Append this to the same list, after the Appearance bullet:

```markdown
- **Cards** (both grids) scale on hover with a gradient overlay, a centred Play or
  Install, and an action row of Details, Cloud sync and More. Badges: an installed dot
  top-right, an `UPDATE` tag top-left, a cloud icon bottom-right when the platform has
  cloud sync configured, and a short platform name bottom-left. `Ctrl+F` focuses the
  current view's search box.
```

- [ ] **Step 2: Add the manual checklist rows**

In `rewrite/README.md`, append to the "Residual manual checklist" list, after the "Server menu" bullet:

```markdown
- **Recently played**: launch an installed game, quit it, and confirm the Library rail's
  Recent count includes it and the "Recently played" sort puts it first; confirm the
  stamp survives updating that game (an update must not reset it) and a relaunch of the
  app.
- **Platform firmware chip**: on the Server view, select a platform the server holds
  firmware for and confirm the chip counts the files. With a default emulator set,
  press Install and confirm the firmware lands in that emulator's firmware directory;
  with the platform set to (none), confirm the chip reads "no default emulator" and
  offers no button.
- **Card sizes at width**: with the window maximised on a wide display, confirm the grid
  fills to at most 1920px and stays centred, and that Small / Medium / Large change the
  column count rather than stretching the covers.
```

- [ ] **Step 3: Note the new registry column in the porting doc**

In `docs/porting/03-library-install.md`, append this to the section describing the installed-game record (the "Registry key inside config" table around line 44 and the persistence discussion at line 263):

```markdown
### `last_played_at` (rewrite only)

The Rust rewrite's SQLite registry carries one column the Python
`installed_games` record never had: `last_played_at`, epoch seconds of the last
launch, `0` for a game never launched through GRID. It is written only when a
launch has actually spawned a process, and never by the install upsert — an
update or a reinstall keeps the stamp. Two surfaces read it, both in the
redesigned desktop Library view: the rail's "Recent" entry (played within the
last 30 days) and the "Recently played" sort. Nothing in the install pipeline
depends on it; a port that omits it loses those two orderings and nothing else.
```

- [ ] **Step 4: Commit**

```bash
cd /home/six/Documents/Programming/grid-launcher
git add SPEC.md rewrite/README.md docs/porting/03-library-install.md
git commit -m "rewrite: document the redesigned Library and Server views"
```

---

## Self-review

**Spec coverage.**

| Spec requirement | Task |
|---|---|
| §5 rail: All games / Recent / Updates / PLATFORMS with counts, 220px, persists per session | 2 (`rail.ts`), 4 (`selection.svelte.ts`, markup) |
| §5 toolbar: search, sort (four modes), card size | 2 (`sort.ts`, `size.ts`), 4 |
| §5 grid: `auto-fill minmax`, 3:4 fallback, one-line ellipsised title under the card | 3 (`CardGrid`, `GameCard`, `CARD_COVER_RATIO`) |
| §5 empty state per rail item, four exact strings | 2 (`emptyText`), 4, 8 (asserted) |
| §5 card click opens details, hover Play launches | 3, 4 |
| §6 rail of server platforms with ROM counts | 5 |
| §6 platform header: name, ROM count, installed count, firmware chip + Install, emulator chip → Emulators | 5 (counts), 6 (chips) |
| §6 grid: installed dot, UPDATE tag, 60% until hover, Install/Play hover primary | 3 (card), 5 (wiring) |
| §6 client-side search | 2 (`titleContains`), 5 |
| §3 `Ctrl+F` focuses the current view's search | 7 |
| §11 `library-rail-<key>`, `server-rail-<id>` | 2, 4, 5, 8 |
| §11 `platform-btn-<id>`, `game-card-<id>`, `library-card-<id>`, `library-update-badge-<id>` survive | 3, 4, 5, 8 |
| D-UI-2 rail + one grid + toolbar | 4 |
| D-UI-3 Server mirrors Library | 5, 6 |
| D-UI-7 grids to 1920px (lists still 1100px) | 3 (`CardGrid`) |
| D-UI-9 hover 1.05 + gradient + centred primary + action row; four badges; size per view, remembered | 1 (config), 2 (badges/size), 3 (card), 4, 5 |
| D-UI-9 Favourite | **deferred**, stated in the header — no backend exists |
| Recent needs a last-played timestamp | 1 (`last_played_at`, migration, launch stamp) |
| Docs (SPEC.md, README checklist, porting 03) | 9 |

**Placeholder scan.** No "TBD", no "similar to Task N", no "add error handling". Every code step carries the code. The one forward reference — Task 1 Step 15's note that `normalizeCardSize` arrives in Task 2 — is resolved inside that same step by inlining `'medium'`, and Task 2 Step 17 replaces it with the normalizer; both spellings are written out.

**Type consistency.** `CardSize` is `CardSizeName` from `api.ts` throughout (Tasks 1–5). `RailKey` is the same union in `rail.ts`, `selection.svelte.ts` and `Library.svelte`. `PlatformFirmwareStatus` has the same two fields in Rust (Task 6 Step 7), TypeScript (Step 10) and the header helpers (Step 3). `setCardSize(view, size)` is declared in Task 2 and called with `'library'` / `'server'` in Tasks 4 and 5. `focusSearch()` is exported by both views (Tasks 4, 5) and called in Task 7. `initialCloudMode` is added in Task 3 and passed in Tasks 4 and 5. `spawn_for_platform`'s signature in Task 6 Step 6 matches its call in Step 7.
