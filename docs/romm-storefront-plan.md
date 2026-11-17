# RomM as an optional storefront

Status: decisions settled 2026-11-15; plan written 2026-11-15; revised 2026-11-15 after the
user answered Q1–Q8 (decision 3 reversed, decisions 9–12 added). One open question
remains (end of the plan). It is not decided.

> **Note 2026-11-17: Discover and Open GRID on removed for now.** The user took the
> Discover view out of the app, in desktop and TV mode, until the Discover rework (see
> "After this rework: Discover"). The RomM rail group has no Discover row (connected:
> Installed, then the platform rows; connecting at startup: Installed only); a stale
> `romm:discover` selection falls back to RomM › Installed, or All games. Settings ›
> Appearance › Open GRID on is hidden and GRID always opens on Library › All games; the
> `[ui] open_on` value is still read and written, and ignored. The desktop "Open Discover"
> button, TV Menu › Discover and TV's Search everything are gone too. Discover's code
> (`app/src/lib/discover/`, `TvRommDiscover.svelte`, `discoverRows.ts`, `searchAll.ts`,
> `romm::discover`, `commands/discover.rs`) stays dormant with its unit tests; the e2e
> `discover` and `open-on` groups were removed. Where this plan describes Discover or
> Open GRID on below, read it as the state before 2026-11-17.

GRID stops being "a launcher for RomM" with stores beside it. RomM becomes one optional
storefront next to GOG, Epic, Amazon and Steam. Its engine (platforms, emulators,
firmware, per-emulator cloud saves, relink, drive packs) stays; what changes is how it is
reached, set up and shown. Systems (emulators and firmware) becomes a general emulation
setup tool that works without RomM.

## User decisions (2026-11-15)

1. **Browsing lives in the Library.** The Library rail gets a RomM group, as the stores have.
   The Server tab is removed (desktop and TV). The group's rows are set by decision 11.
2. **First run is a neutral welcome.** Pick the library folder, then optionally connect any
   storefronts (RomM, GOG, Epic, Amazon, Steam), or skip and add them later in Settings.
   No RomM-first Connect screen.
3. **Systems is a general emulation setup tool, separate from RomM** (replaces the earlier
   "Systems stays RomM-only"). Settings › Systems and Choose your systems work without
   RomM, from a platform list GRID ships itself. When RomM is connected, its platforms are
   merged in. Each firmware install offers **Install from RomM** (needs RomM) and
   **Locate files** (pick files on disk; GRID verifies size and MD5 and routes them into
   the emulator's folders). Bringing your own ROMs (play local ROMs, install ROMs from a
   Backup drive without RomM) comes later and is out of scope here, but the platform
   model must not block it.
4. **RomM is a card on Settings › Game stores** (server, user, status, Edit, Reconnect,
   Disconnect, Open in browser). The Connection page goes. Cloud saves and
   RetroAchievements stay their own pages and say where they need RomM.
5. **Disconnect RomM acts like a store disconnect.** GRID deletes the RomM login and stays in
   the app. Installed RomM games stay and still play; cloud saves, firmware downloads and
   updates stop. The RomM group stays in the Library while installed RomM games remain.
6. **No server chip, no offline mode.** The top-bar server chip and menu, and the distinct
   "Offline · no server" mode, are removed. RomM's status (connected, unreachable, sign-in
   expired) shows on its Library group and its Settings card, like the stores; Reconnect
   lives there.
7. **One RomM server**, as today. Multi-server can come later.
8. **Backups of installed RomM games work without RomM**, from the installed copy, as GOG
   games do. With RomM connected it works as today.
9. **Choose your systems after a later RomM connect** (2026-11-15, the planner's
   recommendation, the user had no preference). The full-screen picker appears after the
   first successful RomM connect by today's rule (setup never finished or skipped, no
   emulator, no game), wherever the connect happens. From the Settings card, Skip and Done
   return to Settings › Game stores. The welcome without RomM does **not** offer an
   emulator step (settled by the planner, see 0.11).
10. **Alphabetical order** (2026-11-15). The RomM group in the Library rail and the RomM
    card on Settings › Game stores sort among the stores by name: Amazon, Epic, GOG, RomM,
    Steam. The same order applies to the welcome's storefront rows.
11. **The RomM group follows the stores' pattern** (2026-11-15).
    - RomM not connected (with installed RomM games): the group has an **Installed** row
      only. Discover and the platform rows are hidden.
    - RomM connected: **Installed**, then **Discover**, then one row per platform (server
      and installed games merged).
    - Installed RomM games stay mixed into All games, Recent and Updates in every state.
    - This replaces the earlier "Discover panel when RomM is off" recommendation.
12. **The planner's recommendations for the earlier Q4–Q7** (2026-11-15):
    - the Settings pane is renamed "Game stores";
    - offline users keep their stored RomM credential, unused, until a new connect
      replaces it;
    - Python-import users get the welcome, filled in;
    - Backups without RomM include games from the installed copy plus catalog emulator
      builds; server-hosted emulator builds and firmware are skipped as "needs RomM".

## What already exists (code map, main c7b1107)

- "Use without a server" (`Config.offline`, `session.rs` `use_offline`/`restore` →
  `Offline`, `connectFlow.ts` `useWithoutServer`, TV `connectPage.ts`) already runs GRID
  with no server: Library, drives, Downloads and all four stores work.
- Every RomM feature already gates on `SessionManager::client()` returning `None`
  (about 45 call sites); the UI needs consistent "needs RomM" states, not new gates.
- Top-level views: `app/src/lib/shell.ts` `VIEWS`, `Shell.svelte` (chip, server menu),
  `Server.svelte` (Discover is its first rail row); TV `TvStage.svelte`, default tab
  `server` in `tv/nav.svelte.ts`, `tv/tabs/server/TvServer.svelte`, `tv/hints.ts`,
  `tv/overlays/menu.ts`.
- Library rail: `app/src/lib/library/rail.ts` (PLATFORMS rows from installed RomM rows),
  `app/src/lib/gamestores/rail.ts` (store groups: fixed Installed/Available rows, keys
  `store:<source>:installed|available`), `Library.svelte` assembles them in the order top
  rows → platforms → Backup drives → stores; `gamestores/marks.ts` `STORE_ORDER` is
  already alphabetical (`amazon, epic, gog, steam`). TV `tv/tabs/library/*`,
  `tv/stores/libraryStores.ts`.
- Settings: `settings/ConnectionPage.svelte` + `connection.ts`; `settings/StoresPage.svelte`
  + `gamestores/settings.ts` (cards from `StoreStatus`, fixed to four sources); TV
  `tv/tabs/settings/panes/connection.ts`, `tv/stores/TvStoresPane.svelte`.
- Startup: `App.svelte` phases (`none` → Connect, `setup` → ChooseSystems, `shell`);
  `Connect.svelte`; `stores/session.svelte.ts` (`withSetupCheck` only when connected;
  Disconnect returns to Connect); `shell.ts` `initialView`; TV `TvShell.svelte`,
  `tv/startup/*`.
- Backend: `SessionManager` (one live `RommClient`, also owns the image cache, host-filtered
  to the server URL); `StoresService` is independent of it. `StoreKind` has no RomM variant
  by design; `GameSource { Romm, Store(_) }`; RomM rows key on `(title, platform)` with an
  optional `rom_id`. RomM does not fit `StoreBackend` literally (platforms, emulators,
  firmware, saves, login by URL + token); the expected shape is a RomM storefront facade
  over the existing engine.
- Backups: `commands/backups.rs` `NEEDS_SERVER`; `plan_backup`/`start_backup` need a client;
  `SourceChoice::Local` exists; store-game local backups in `library/backup_job.rs`.
- **Systems and firmware today are server-driven:**
  - Settings › Systems (`systems/systemsStore.svelte.ts`) loads `listPlatforms()` and
    `setupPlatformOptions()`; both fetch the server's platform list.
    `commands/setup.rs` `setup_platform_options` → `grid_core::launch::setup::platform_setup(&[Platform], …)`.
    TV: `tv/systemsTv.svelte.ts`, `tv/pages/TvSystemPage.svelte`,
    `tv/tabs/settings/panes/systems.ts`, `tv/startup/choose.ts`.
  - `commands.rs` `list_platforms` fills three label-keyed maps on every fetch:
    `InstallService::set_platform_ids` (label → server id; read only by
    `firmware_service.rs` to fetch firmware from the server),
    `InstallService::set_platform_slugs` and the process-wide
    `grid_core::launch::set_platform_slugs` (label → slug; read by launch core
    resolution, firmware routing, cloud ops). Without a fetch they are empty.
  - `config.default_emulators`, `retroarch_cores` and `setup.pending` are keyed by the
    platform **label** (`Platform::label()`: display name, else name, else slug), the same
    string installed rows carry in `platform`.
  - Emulator profiles (`emulator-autoprofiles.json`) match platforms by **name keywords**
    (`platform_matches_keywords`, e.g. "playstation 2"), not by slug.
  - Firmware: the file list (name, size, MD5) comes from RomM (`GET /api/firmware`) or a
    drive's gridpack. `firmware::FirmwareSource` already abstracts the source
    (`ServerFirmware`, `PackFirmwareSource`); `install_platform_firmware_from` routes,
    verifies (`verify_download`: size then MD5) and writes the same way for both, and
    `firmware/manifest.rs` records what was installed. Firmware from a Backup drive
    already works without RomM (`firmware_service.rs` `spawn_from_pack`).

## Plan

Order: **Step 0** (contracts, below) → **D1 ∥ D2** (design, desktop and TV) → user sign-off →
**S** (SPEC/ARCHITECTURE) ∥ **Step 1** (foundation) → **Steps 2 ∥ 3 ∥ 4 ∥ 5 ∥ 6 ∥ 7** in
parallel worktrees → **Step 8** (e2e migration) → **Step 9** (legacy removal, full gate,
full e2e suite). Every step ends with the gate (CLAUDE.md). Each step commits with
`git commit --only <its paths>`.

The work is additive first. Step 1 adds the new RomM status, commands and frontend stores
*beside* the old ones, and the old commands become thin wrappers over the new state. This
keeps the gate green after every step. Each UI step deletes the old screens it replaces.
Step 9 deletes the old commands and fields once nothing uses them.

Step 7 (Systems and firmware without RomM) is its own parallel step: after Step 1 moves
the platform-map code into its own file (0.9) and adds the TS types (0.10), Step 7's files
are disjoint from Steps 2–6 (file check after Step 7).

### Step 0 — shared contracts (settled here, before any fan-out)

**0.1 Facade, not `StoreBackend`.** RomM does not get a `StoreBackend`
(`app/src-tauri/src/stores_service/mod.rs`). That trait is built around an owned list, a store
tool, a login window that catches a redirect code, `library()` with Installed/Available,
and store installs. RomM has none of these. It signs in with a URL plus a token or password,
and its games come per platform with emulators, firmware and saves. The facade is:
- `grid_core::session::SessionManager`: it owns the RomM connection state (new) and the
  live `RommClient` (as today);
- a new `app/src-tauri/src/commands/romm.rs`: the `romm_*` commands and the
  `romm-changed` event;
- on the frontend, a new `app/src/lib/stores/romm.svelte.ts` and pure modules under
  `app/src/lib/romm/`.
Everything that calls `SessionManager::client()` today (about 45 sites) is unchanged.

**0.2 Status shape: a sibling `RommStatus`, not a `romm` source in `StoreStatus`.**
`StoreStatus` is built around the store tool, the owned list and Steam, and
`StoreSource = Exclude<GameSource,'romm'>` is used in about 30 store modules. Adding RomM
there would force a RomM branch into every store function. The Settings list and the
Library rail instead take `RommStatus` plus `StoreStatus[]`.

Rust (`crates/grid-core/src/session.rs`):
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RommConnection { Connected, Unreachable, Expired, NotConnected }

/// RomM's card and rail group. No secret, ever.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct RommStatus {
    pub connection: RommConnection,
    /// Credential-free (`strip_userinfo`); "" when none was ever stored.
    pub server_url: String,
    pub username: String,
    /// Which credential kind the sign-in form asks for: the keyring's kind when
    /// known (connect, restore, a 401), else None (form defaults to token).
    pub auth_kind: Option<AuthKind>,
    /// Unreachable: the probe's secret-free reason. "" otherwise.
    pub last_error: String,
    /// Installed RomM rows (source Romm, not the hidden Emulators platform).
    pub installed_count: u32,
    /// connection != NotConnected || installed_count > 0 (decisions 5 and 11).
    pub in_rail: bool,
    /// A probe (restore, reconnect, connect) is running.
    pub busy: bool,
}
```
`SessionManager` keeps a `Mutex<RommState>` (connection, auth_kind, last_error, busy).
`fn status(&self, installed_count: u32) -> RommStatus` builds the status, and
`set_change_hook(Arc<dyn Fn() + Send + Sync>)` fires on every change. The app layer counts
the installed rows, because grid-core's session does not read the registry. The 401 hook
sets `Expired` and fires the change hook. It no longer emits `session-unauthorized` for a
screen change.

TS (`app/src/lib/api.ts`):
```ts
export type RommConnection = 'connected' | 'unreachable' | 'expired' | 'not_connected';
export type RommStatus = {
  connection: RommConnection; server_url: string; username: string;
  auth_kind: AuthKind | null; last_error: string;
  installed_count: number; in_rail: boolean; busy: boolean;
};
export const ROMM_CHANGED_EVENT = 'romm-changed'; // payload: RommStatus
```

**0.3 Rail keys and order (decisions 10 and 11).**
- Keys of the RomM group:
  - `romm:installed`: Installed (test id `library-rail-romm-installed`, count test id
    `library-rail-count-romm-installed`). Always present while the group shows. Its grid
    is every installed RomM row (source Romm, not the Emulators shelf), with the
    Library's sort and search.
  - `romm:discover`: Discover (test id `library-rail-romm-discover`). Only while
    `connection === 'connected'`.
  - `platform:<slug>`: one row per platform (test id `library-rail-platform-<slug>`). Only
    while `connection === 'connected'`. The key stays `platform:` so `matchesRail` and
    selection keep their shape.
- **Rows per state** (`rommRailGroup`, 0.8):
  - `connected`: Installed, Discover, then the platform rows.
  - `not_connected`, `unreachable`, `expired`: Installed only. The heading flag says why
    (`off` for not connected, `!` for unreachable or expired), and the Installed view shows
    a one-line banner: "Can't reach RomM · Retry" or "RomM sign-in expired · Sign in
    again…" (words from D1). No banner for not connected; the `off` flag is enough, as for
    a store. Rationale: Discover and the platform rows need a live server; showing them
    with no client would only show error panels, and decision 11 hides them when RomM is
    off.
  - not connected with no installed RomM rows (`in_rail` false): no group.
  - When the selected key disappears (RomM drops while a platform or Discover is open),
    selection falls back to `romm:installed`.
- `<slug>` is resolved in a new pure `app/src/lib/romm/platformKey.ts`:
  1. a server platform: its `slug`;
  2. an installed RomM row: `platform_slug` when not empty; otherwise the slug of the
     server platform whose `platformLabel` equals the row's `platform`; otherwise
     `platformSlug(row.platform)` (today's name token).
  This keeps an unlinked or pre-v8 row in its platform's row.
- The platform rows are the union of the server platforms (`rom_count > 0`, not the
  Emulators shelf) and the installed RomM platforms, sorted by label. The label is the
  server's `platformLabel` when known, else the row's `platform`.
- `RailKey` (`app/src/lib/library/rail.ts`) gains `` `romm:${'installed'|'discover'}` ``. The
  top section becomes All games, Recent, Updates only (installed RomM games stay in all
  three). The PLATFORMS section is removed; platform rows live only in the RomM group.
- **Rail order:** top rows → Backup drives → storefront groups in name order: Amazon, Epic,
  GOG, RomM, Steam. A new pure `app/src/lib/romm/order.ts` exports
  `STOREFRONT_ORDER: readonly ('romm' | StoreSource)[] = ['amazon','epic','gog','romm','steam']`,
  used by the desktop and TV rails, the Game stores pane and the welcome. `STORE_ORDER` in
  `gamestores/marks.ts` is unchanged.
- A platform view's grid is a merge (pure, `app/src/lib/romm/platformGames.ts`): the
  server's `GameSummary[]` for the platform, each matched to an installed row by `rom_id`
  (and `source_server` empty or equal to the live server). It adds the installed rows that
  match no server game (unlinked rows, rows from another server, games gone from the
  server). The Discover feed and `Catalog` (`app/src/lib/server/catalog.svelte.ts`) are
  reused unchanged and move from the Server view into the Library.
- `card_size_server` is no longer read. The RomM views use `card_size_library`. The key stays
  in `UiSettings` (and is ignored), so configs round-trip.

**0.4 Config migration** (`crates/grid-core/src/config.rs`).
- `server_url` and `username` **stay top-level**. The Python importer writes them, 29
  non-test sites read them, and an older GRID build reading the config still finds them.
- New `[stores.romm]`: `RommConfig { connected: bool }`, which is
  `StoresConfig.romm`, skipped while default. `connected` says whether GRID should probe at
  startup. It replaces the old meaning of `offline`.
- New `[setup] welcomed: bool` (`SetupState`): the welcome was finished or skipped.
- One-time migration in `Config::load`, marked by a new top-level
  `romm_store_migrated: bool` (false when absent; `Config::default()` sets it true, like
  `background_fade_inverted`):
  - `stores.romm.connected = !offline && !server_url.is_empty()`
  - `setup.welcomed = offline || (!server_url.is_empty() && !library_path.is_empty())`
  - `offline = false`
  - The field `offline` stays only so the migration can read it, with
    `skip_serializing_if = "is_false"`, so it leaves the file on the next save.
- Result: connected users probe as today and never see the welcome. Offline users go
  straight to the shell with RomM "Not connected", and nothing is probed or read from the
  keyring. Their keyring credential is left in place, unused, until a new connect
  replaces it (decision 12).
- The Python importer (`crates/grid-core/src/import_python.rs` `plan`) sets
  `romm_store_migrated = true`, `setup.welcomed = false`, `stores.romm.connected = false`
  explicitly. An imported user sees the welcome with the library folder and the RomM
  server and username filled in (decision 12).
- `default_emulators`, `retroarch_cores` and `setup.pending` keep their label keys; no
  migration (lookup rule in 0.9).

**0.5 Startup phases** (`app/src/lib/shell.ts` `SessionPhase`, `stores/session.svelte.ts`).
- `'loading' | 'welcome' | 'setup' | 'shell'`. `'none'` is renamed `'welcome'` and is entered
  only while `setup.welcomed` is false (new command `welcome_status`). It is never entered
  because of the RomM state.
- `'setup'` (Choose your systems) follows a *successful RomM connect* when
  `setup_status().needed` (today's rule), wherever the connect happens (decision 9).
  `session.setupReturn: 'welcome' | 'settings'` records where it came from:
  - from the welcome: Done/Skip go on to the shell (as today);
  - from the Settings card: Done/Skip return to the shell on Settings › Game stores.
  `romm.svelte.ts` `connect()` sets the phase and `setupReturn`; `App.svelte` and
  `TvShell.svelte` render it; ChooseSystems (Step 7) only emits done/skip.
- The welcome without RomM goes straight to the shell; it has no emulator step (0.11).
- Startup: `welcome_status` → `romm_restore` (only when `stores.romm.connected`; it runs in
  the background after the shell is up, so an unreachable server never holds the window) →
  shell. Before the shell, `system_platforms` warms the platform registries from the
  catalog (0.9), so launch and firmware routing work with no server.
- Disconnect, a 401, an unreachable server: the shell stays. Only `RommStatus` changes.
  Nothing returns to a Connect screen. Connect, `connectToServer()` and the `none`
  branch in `App.svelte` and `TvShell.svelte` go away.
- Where GRID opens (`shell.ts` `initialView`, which desktop and TV share): it now returns
  `{ view: 'library', rail: RailKey | null, drive: string | null }`.
  - Automatic: `romm:discover` when RomM is connected at startup, else All games (else the
    first Backup drive when RomM was never connected).
  - `discover`: `romm:discover` when RomM is connected at startup, else All games (the
    Discover row does not exist while RomM is off, decision 11).
  - `library`: All games / first drive, as today.
  - The stored values (`automatic`, `discover`, `library`) do not change, so no migration.
  - Restore runs after the shell is up, so "connected at startup" means
    `stores.romm.connected` in the config; if the probe then fails, selection falls back to
    `romm:installed` (0.3) or All games when the group is gone.
- `VIEWS` becomes `['library','downloads','settings']`, and `Ctrl+1..3`.

**0.6 IPC commands: RomM** (`commands/romm.rs` and `commands/welcome.rs`, registered in
`app/src-tauri/src/lib.rs`).

| Old | New | Notes |
|---|---|---|
| `restore_session` → `RestoreOutcome` | `romm_restore` → `RommStatus` | Probes only when `stores.romm.connected`. Sets `server_url` for image filtering whenever the config has one, even when not connected. Spawns connected jobs on success. |
| `connect` → `SessionState` | `romm_connect(server_url, username, secret, use_token)` → `RommStatus` | Sets `stores.romm.connected = true` in the same save. `UsernameMismatch` is unchanged. |
| `retry_connect` | `romm_reconnect` → `RommStatus` | Re-probes with the stored credential (Unreachable → Retry). `Err` when none is stored. |
| `disconnect` | `romm_disconnect` → `RommStatus` | Drops the client, clears the keyring, sets `connected = false`, clears updates. Keeps `server_url`/`username` for prefill. Stays in the shell. |
| `use_without_server` | removed (Step 9) | |
| — | `romm_status` → `RommStatus` | Pull, for stores that mount late. |
| — | `welcome_status` → `{ needed: bool }`, `finish_welcome()` | In a new `commands/welcome.rs` (not `commands/setup.rs`, which Step 7 owns). |
| `open_server_page` | unchanged | The card's "Open in browser". |
| event `session-unauthorized` | event `romm-changed` (`RommStatus`) | Emitted on every connection change, including a 401. |

Steps 1–8 keep the old commands as wrappers over the new state. `restore_session` maps
not-connected-and-welcomed to the `offline` kind, so the old shell still opens. Step 9
deletes them, together with `RestoreOutcome`, `SessionState`'s IPC use, `SessionManager::use_offline`
and `SESSION_UNAUTHORIZED_EVENT`.

**0.7 Frontend stores** (Step 1).
- `stores/romm.svelte.ts` holds `romm.status: RommStatus` (pulled once, then `romm-changed`),
  plus `connect`, `reconnect` and `disconnect`.
- `session.svelte.ts` keeps `session.phase` and gains `setupReturn` (0.5).
  `session.connected` becomes a getter for `romm.status.connection === 'connected'`, so the
  ~30 readers do not change.
- `session.offline`, `lastError` and `rejected` stay until Step 9, but are no longer
  written to by the new code.

**0.8 Pure modules** (Step 1, used by desktop and TV): `app/src/lib/romm/`:
- `platformKey.ts`;
- `order.ts` (`STOREFRONT_ORDER`, 0.3);
- `rail.ts`: `rommRailGroup(status, platforms, installedRows, updates)` →
  `{ heading:'ROMM', flag, rows } | null`. `rows` is `[installed]` unless connected, then
  `[installed, discover, ...platforms]`. `flag` is `!` for expired or unreachable and
  `off` for not connected, like `storeFlag`;
- `platformGames.ts`;
- `words.ts`: card pill, account line, banner lines, needs-RomM reasons per feature
  ("Cloud saves need RomM", "Install from RomM needs RomM", …, exact words from D1);
- `RommSignInForm.svelte` (desktop) and `signIn.ts`: the URL / token-or-password form taken
  out of `Connect.svelte` and `connectForm.ts`. The welcome and the card's Edit/Reconnect
  dialog use it.

**0.9 Platform source and model without a server** (Step 7; Step 1 does the file move).

*What GRID's bundled data covers (finding, main 19533de):*
- `platform-emulators.json` `recommended`: 19 RomM slugs → preferred standalone emulator
  profiles (psx, ps2, psp, ps3, psvita, ps4, ps4--1, ps5, ngc, wii, wiiu, 3ds,
  new-nintendo-3ds, switch, switch-2, xbox, xbox360, dc, saturn).
- `romm-platform-cores.json`: 75 RomM slugs → RetroArch cores (the 8-bit to 32-bit
  consoles, handhelds, arcade/CPS, computers, DOS, ScummVM, …).
- Together: **84 distinct slugs** (10 only in the first file, 65 only in the second, 9 in
  both). With `emulator-autoprofiles.json` (26 profiles) and `retroarch-core-list.json`,
  that is enough to say, per slug, which emulators GRID can install on this OS.
- **Missing:**
  1. **Display names.** Both files are keyed by slug only. Labels matter: config keys
     (`default_emulators`, `retroarch_cores`, `setup.pending`), the registry's `platform`
     column and profile keyword matching ("playstation 2") all use names.
  2. **Slugs for some profiles.** Pico-8 (`Pico-8` profile) and Sega Model 3 (Supermodel)
     have profiles but no slug in either file (`pico` in the cores file is the Sega Pico).
  3. **A firmware file table.** GRID ships no list of firmware file names, sizes or MD5s;
     today they come only from RomM or a gridpack. Locate files without RomM needs one.
  4. **Server ids.** `set_platform_ids` holds RomM's numeric ids; they exist only on a
     server and are read only to fetch firmware from it. No local substitute is needed.

*New bundled data (repo root, `include_str!` like the others):*
- `platforms.json`: `{ "_comment": …, "platforms": [ { "slug", "name", "aliases": [] } ] }`.
  `name` is **RomM's default platform name for that slug** (doc-research takes RomM's
  slug → name table for the server version GRID targets), so a user whose server has no
  custom names gets identical label keys with and without RomM. It lists every slug of the
  two existing files plus the profile-only systems (Pico-8, Sega Model 3 under RomM's
  slugs). `aliases` are other names RomM or the Python launcher used (for old config
  keys).
- `firmware-known.json`: per slug, the files emulators need: `{ "name", "size", "md5",
  "required": bool, "profiles": [..] }`. `size`/`md5` are 0/"" for per-console files
  that have no fixed hash (Switch `prod.keys`, Wii U `keys.txt`). Source: doc-research
  compiles it from published BIOS tables (libretro's System.dat for RetroArch cores, each
  standalone emulator's own docs) and confirms the licence before anything is copied.

*grid-core model* (new `crates/grid-core/src/systems/`, `mod.rs` + `catalog.rs` +
`merge.rs`):
```rust
/// Where a system row came from. A later "local ROMs" origin adds games, not a new
/// kind of platform: nothing below assumes a server id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlatformOrigin { Catalog, Server, Both, Installed }

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SystemPlatform {
    pub slug: String,
    /// The server's label when connected and known, else the catalog name, else the
    /// installed row's `platform`. The key for default_emulators and friends.
    pub label: String,
    /// Every name this slug has gone by (catalog name, aliases, server name and
    /// display_name): the lookup set for label-keyed config.
    pub aliases: Vec<String>,
    pub origin: PlatformOrigin,
    /// RomM's id; None without RomM. Only Install from RomM reads it.
    pub server_id: Option<i64>,
    /// RomM's ROM count; None without RomM.
    pub server_rom_count: Option<i64>,
    /// Installed games on this platform (any source with this slug).
    pub installed_count: u32,
}
```
- `systems::catalog()` parses `platforms.json` once.
- `systems::merge(catalog, server: Option<&[Platform]>, installed: &[(label, slug)]) -> Vec<SystemPlatform>`:
  join by slug; the server label wins; a server platform the catalog does not know is
  added (`Server`); an installed platform neither knows is added (`Installed`, slug =
  `platform_slug` or the name token); the Emulators shelf is dropped. Sorted by label.
- **Label lookup rule** (fixes the custom-name gap): a reader of `default_emulators`,
  `retroarch_cores` or `setup.pending` tries the current `label` first, then each alias of
  the same slug. Writers use the current label. A new pure
  `systems::lookup<'a, V>(map: &'a BTreeMap<String,V>, p: &SystemPlatform) -> Option<&'a V>`.
  `launch::selection` and `autoconfig::entry` call it where they key by name today; no
  config migration.

*Platform registries without a server* (app layer). Step 1 moves `platform_id_entries`,
`platform_slugs_by_label`, `platform_slugs_for_install` and the three `set_*` calls out of
`commands.rs` `list_platforms` into a new `app/src-tauri/src/platform_registry.rs`
(`fn record(state, server: Option<&[Platform]>)`), with no behaviour change. Step 7 then
owns that file and changes `record` to:
- slugs (`launch::set_platform_slugs`, `InstallService::set_platform_slugs`): every
  `SystemPlatform` label and alias → slug, server entries winning;
- ids (`InstallService::set_platform_ids`): server platforms only (label → id); empty
  without RomM;
- called at startup with `None` (catalog only) and after every `list_platforms` with the
  server list. `InstallService` itself (`library/mod.rs`) does not change.

*Future: bring your own ROMs* (out of scope, recorded so this model does not block it).
A later step lets users add local ROM folders and install ROMs from a Backup drive
without RomM. It will add a game source (likely a `GameSource::Local` variant and
registry rows with that source), whose rows join `SystemPlatform` by slug like installed
RomM rows do. Systems, firmware and the label lookup above need no change for it; the
Library will get its own group for it then.

**0.10 IPC commands: Systems and firmware** (Step 7 implements; Step 1 adds the TS types
and wrappers to `api.ts` so Steps 4, 5 and 6 can import them).

| Command | Returns | Notes |
|---|---|---|
| `system_platforms()` (new) | `SystemPlatform[]` | `merge` over the catalog, the server list when a client is live (else none), and the registry's installed rows. Calls `platform_registry::record`. Never fails for lack of RomM. |
| `setup_platform_options()` (changed) | `PlatformSetup[]` | Takes its platforms from `system_platforms`, not `list_platforms`. Without RomM: catalog emulators only (server packages empty). `platform_setup` takes `&[SystemPlatform]`. |
| `install_firmware_for_platform(platform_id, …)` (unchanged) | as today | "Install from RomM". The frontend passes `SystemPlatform.server_id`; the button is disabled with "Needs RomM" when it is None or RomM is not connected. |
| `firmware_expected(slug, emulator)` (new) | `FirmwareExpected { files: { name, size, md5, required, installed, verified_by: 'romm' \| 'grid' \| 'none' }[] }` | The expected set: the server's firmware list for the platform when RomM is connected, unioned by name with `firmware-known.json` entries for this slug and emulator's profile. `installed` from `firmware/manifest.rs` (local-file installs are recorded the same way). |
| `locate_firmware_files(slug, emulator, paths: string[])` (new) | `LocateReport { installed: { file, as_name }[], unverified: { file, as_name }[], refused: { file, reason }[] }` | See validation below. |
| `locate_ps3_firmware(emulator, path)` (new) | as `install_ps3_firmware` | A picked `PS3UPDAT.PUP` handed to `rpcs3 --installfw` (`firmware/rpcs3.rs`), after its size/MD5 check against the expected set when one is listed. |

*Locate files: picking.* Desktop uses `@tauri-apps/plugin-dialog` `open({ multiple: true })`
for files, or a folder; TV uses its folder browser in a file mode (on-screen, no desktop
fallback). A folder is scanned (depth ≤ 3, regular files only, symlinks not followed);
files that match nothing are counted, not listed one by one.

*Locate files: validation* (pure, `crates/grid-core/src/firmware/local.rs`, plus a
`LocalFirmwareSource: FirmwareSource` so writes go through `install_platform_firmware_from`
with the same routing, zip keep/extract rule, verification and manifest as RomM and
gridpack installs):
1. Each path: canonicalized; must be a regular file; larger than
   `PACK_FIRMWARE_MAX_BYTES` (2 GiB) → refused before reading.
2. Size and MD5 are computed once per file (streamed, not held twice).
3. Match against the expected set (`firmware_expected`):
   - MD5 and size match an entry → installed under the entry's `name` (a renamed dump,
     e.g. `scph5501 (1).bin`, still lands as `scph5501.bin`); the `FirmwareRecord` handed
     to the source carries that size and MD5, so `verify_download` re-checks it;
   - name matches an entry that has a hash, but size or MD5 differ → refused: "This
     isn't the <name> GRID knows (size or MD5 differs)";
   - name matches an entry with no hash (per-console keys) → per the open question
     below (recommended: installed, listed as "unverified");
   - an archive (`.zip`, `.7z`, `.rar`) that matches no entry as a whole → its members are
     matched one by one (the existing extractor, into a temp dir) and installed members
     are written as raw files;
   - no match → refused: "GRID doesn't know this file for <system>".
4. Targets come from the emulator's profile `firmware_directories`
   (`routing::targets_for_entry`, RetroArch/Cemu shaping) as for RomM firmware. No
   emulator installed for the system → the command refuses before reading any file.
5. Nothing about file contents is logged; paths are logged at debug only. (Firmware is not
   a secret under the hard rules, but key files are personal.)

**0.11 Welcome and emulators (decision 9, settled by the planner).** The welcome does not
offer an emulator step without RomM. Reasons: until bring-your-own-ROMs lands, a user
without RomM has no ROMs to play, so an emulator picker at first run would be setup for
nothing; store games need only the Windows (Proton) default, which is in Settings ›
Systems and now works without RomM. With RomM connected in the welcome, Choose your
systems follows as today (pre-checked from the server's platforms). Revisit when
bring-your-own-ROMs adds local games; the picker can then pre-check the systems of the
games found.

**0.12 Settings › Systems without RomM (settled by the planner; D1 draws it).**
- Rows: the `SystemPlatform`s that have at least one emulator installable on this OS
  (standalone profile or RetroArch core), plus any with a configured emulator or an
  installed game, sorted by label. The Windows (Proton) row and Other emulators stay as
  today. With RomM connected, server platforms with ROMs are added even when GRID has no
  emulator for them (as today), and rows may show the server's ROM count.
- Each firmware block shows the expected files (`firmware_expected`) and two actions:
  **Install from RomM** (disabled with "Needs RomM" when RomM is off or the platform is
  not on the server) and **Locate files…**. After Locate files, a short report lists
  installed, unverified and refused files.
- Choose your systems (after a RomM connect) is unchanged except that its rows come from
  `setup_platform_options` over `SystemPlatform`, and its footer loses "Disconnect".

### Step D1 — design, desktop (designer)
The prototype is the spec. This step comes before any code. The new card is
**RommStorefront**, built in `docs/design/prototype/applayout.py` on top of the GameStores
card (`st`), the same way GameStores is built on GameBackups:
- new `docs/design/prototype/romm.js` and `romm.css`;
- output `docs/design/system/components/RommStorefront/preview.html` and `README.md`. The
  README has a full inventory of states (memory: design parity handoff).

It covers:
- **Welcome** (replaces `fresh.js` `freshShow('connect')`):
  - step 1: the library folder (required, Browse…);
  - step 2: "Connect your games": Amazon, Epic, GOG (their login windows, `stLoginOpen`),
    RomM (inline `RommSignInForm`) and Steam (the `stSteamCard` form), in that
    alphabetical order. Each row shows its connected state;
  - Continue / Skip for now. No emulator step without RomM (0.11).
  - The Python-import variant is filled in. A RomM connect moves on to Choose your
    systems.
  - The theme toggle stays in the corner.
- **Library › RomM group** (rail, beside `stRailHTML`), placed alphabetically among the
  store groups (Amazon, Epic, GOG, RomM, Steam) after Backup drives:
  - heading with the RomM mark and the `!`/`off` flag;
  - **connected**: Installed, Discover, then platform rows with their counts. The designer
    proposes the platform count (recommended: the server's ROM count; installed games
    marked on covers);
  - **not connected, unreachable, sign-in expired**: Installed only, with the flag, and the
    Installed view's banner for unreachable ("Can't reach RomM · Retry") and expired
    ("Sign in again…");
  - the Installed view (installed RomM games, Library sort and search);
  - Discover mounted in the Library (`discover.js`);
  - All games, Recent and Updates still include installed RomM games.
- **The platform view**: the Server platform header from `serverhead.js` (name, counts,
  firmware and emulator chips, Backup all N…, Select), merged covers (Play for installed,
  Install for the rest) and search. Unlinked rows: a "Not linked" tag.
- **Settings › Game stores › RomM card** (beside `stStoreCard` in `renderStoresPane`),
  placed alphabetically among the store cards:
  - server, user, pill (Connected / Can't reach / Sign-in expired / Not connected), last
    error;
  - Edit (sign-in dialog, filled in), Reconnect, Disconnect (confirm: "Installed RomM games
    stay and still play. Cloud saves, firmware downloads and updates stop."), Open in
    browser, Connect… when not connected;
  - a connect from the card that needs setup leads to Choose your systems; Skip/Done return
    to this pane (decision 9).
  - The Connection pane is removed from `panes.js`. The pane label is "Game stores".
- **Settings › Systems as a general tool** (`panes_systems.js` `renderSystems`):
  - the page without RomM: rows per 0.12, no needs-RomM panel; the Windows (Proton) row and
    Other emulators as today;
  - the page with RomM: server platforms merged in (ROM count where known);
  - the firmware block: expected files with installed state, **Install from RomM** (normal;
    disabled "Needs RomM" when off or not on the server) and **Locate files…**;
  - the Locate files report (installed / unverified / refused, with reasons) and the "no
    emulator installed for this system yet" state;
  - PS3: Locate PS3UPDAT.PUP… beside Install from RomM;
  - Choose your systems without its "Disconnect" footer.
- **"Needs RomM" states**:
  - Cloud saves (`panes.js` cloud) and RetroAchievements (`panes.js` ra): a panel or
    banner that names RomM and links the card. The local RA token stays editable if the
    designer finds that sound;
  - Details: Saves tab, Achievements, Link, Update.
- **Removals**: the Server pill and the view in the top bar (applayout's top bar), the chip
  and its menu, the Server view's offline panel (GameBackups `bk-srv-off`), the Connect
  screen, and the Library's PLATFORMS section.
- **Open GRID on** (`panes.js` Appearance): "Automatic: Discover when RomM is connected,
  else Library", "Discover (when RomM is connected)", "Library", help text "Here and in TV
  mode".
- Words: every "Server › …" and "needs a server" string becomes RomM wording (empty drive
  text, backup refusals, the setup card "opens on Discover"). Write "Backup" as one word.

Done when the card is rebuilt
(`python3 -I docs/design/prototype/applayout.py docs/design/system/components`), published
to the design-system artifact, and every inventory item is reachable in the preview.

### Step D2 — design, TV (designer, in parallel with D1; disjoint files)
The new card is **TVRommStorefront**, built in `docs/design/prototype/tvmode.py` on top of
TVStores:
- new `tvromm.js` and `tvromm.css`;
- output `docs/design/system/components/TVRommStorefront/{preview.html,README.md}`.

It covers:
- the TV welcome (replaces `connectPage` / `startConnect` in `tvmode.js`: the folder
  browser, storefront rows in alphabetical order, the on-screen keyboard for the RomM form,
  View to paste);
- three tabs (Library, Downloads, Settings; LB/RB) with **Library as the default tab**;
- the RomM group in the TV Library rail (`libRailHTML`), alphabetical among the stores:
  Installed only when RomM is off (with the flag and the Installed banner), and Installed,
  Discover's shelves and platform pages (moved from `srvPaint`) when connected;
- the RomM card in TV Settings › Game stores (`tvstores.js` stores pane), alphabetical;
- TV Settings › Systems without RomM, the firmware block's Install from RomM / Locate
  files…, the TV file browser in file mode for Locate files, and the Locate report;
- needs-RomM states on the TV Cloud saves and RetroAchievements panes;
- removal of the top bar's connection text and dot, the hint bar's offline line
  (`NO_SERVER_LINE`), and Menu › Reconnect;
- Open GRID on in TV Appearance.

The README lists which `TVMode/README.md` "Decided" items and which
`docs/tv-mode/checklist.md` items it replaces. D2 edits those two files to mark them
superseded and point to the new card. No desktop-mode fallback anywhere (memory).

**Sign-off.** The orchestrator shows the user both cards and their inventories. Step 1
starts only after the user approves.

### Step S — SPEC.md and ARCHITECTURE.md (after sign-off; parallel with Step 1)
Files: `SPEC.md`, `ARCHITECTURE.md`. Rewrite these SPEC sections to the decisions and the
approved design:
- Overview, Top Bar (views, Where GRID opens, offline mode removed);
- Main Sections (Server section folded into Library; the RomM group's rows per state);
- Settings (Connection removed, Game stores card, Systems as a general tool with Install
  from RomM / Locate files), Appearance;
- First Run Setup (the welcome; Choose your systems after a RomM connect; the 401
  paragraph);
- TV mode Screens and Startup;
- Game backups ("Backing up needs a server").
- A short "Later: bring your own ROMs" note under Systems (0.9).

ARCHITECTURE gets `commands/romm.rs`, `commands/welcome.rs`, `commands/systems.rs`,
`platform_registry.rs`, `grid_core::systems`, `firmware/local.rs`, `lib/romm/`,
`stores/romm.svelte.ts`, the two new bundled JSON files, and the removed `Server.svelte`,
`Connect.svelte` and `ConnectionPage.svelte`. Done: no SPEC sentence still describes the
chip, offline mode, Connect, the Server view, or Systems as RomM-only.

### Step 1 — foundation (one implementer, sequential; blocks Steps 2–7)
Files:
- `crates/grid-core/src/session.rs`, `crates/grid-core/src/config.rs`,
  `crates/grid-core/src/import_python.rs`, `crates/grid-core/tests/session.rs`;
- `app/src-tauri/src/commands.rs` (session commands become wrappers that call into
  `commands/romm.rs`; `list_platforms` calls `platform_registry::record`), new
  `app/src-tauri/src/commands/romm.rs`, new `app/src-tauri/src/commands/welcome.rs`, new
  `app/src-tauri/src/platform_registry.rs` (pure move, 0.9), `app/src-tauri/src/lib.rs`
  (register, change hook → `romm-changed`), `app/src-tauri/src/relink_service.rs` (only if
  the connected-jobs hook needs it);
- `app/src/lib/api.ts` (RomM types and commands, plus the Systems types and wrappers of
  0.10), `app/src/lib/shell.ts`, `app/src/lib/stores/session.svelte.ts`,
  new `app/src/lib/stores/romm.svelte.ts`, new `app/src/lib/romm/*`;
- `app/src/lib/connectForm.ts` (moves to `romm/signIn.ts`; leave a re-export until Step 5).

Done:
- contracts 0.2–0.8 exist; 0.9's file move is done; 0.10's TS types and wrappers exist;
- old UI still builds and runs through the wrappers;
- `romm-changed` fires on connect, disconnect, reconnect, a 401 and restore.

Tests:
- Rust unit (config): migration matrix — connected / offline / imported / fresh / partly
  filled; idempotent; `offline` dropped on save; round-trip of an already-migrated config.
- Rust unit (session): state changes for each outcome; the 401 sets `Expired` and keeps the
  keyring; disconnect keeps `server_url`.
- Rust unit (`platform_registry`): the moved maps are byte-identical to before (the
  existing `commands.rs` tests move with them).
- `crates/grid-core/tests/session.rs`: replace the offline tests (lines ~730–830) with
  `connected = false` tests (no probe, no keyring read).
- vitest: `romm/platformKey`, `romm/rail` (Installed only for not connected, unreachable
  and expired; Installed, Discover, platforms for connected; no group when `in_rail` is
  false), `romm/order`, `romm/platformGames` (installed+server merge, unlinked, other
  server), `romm/words`, `shell.initialView` (new matrix, `discover` with RomM off →
  All games), `stores/romm` reducer (connect → `setup` with `setupReturn`).

### Step 2 — Game backups without RomM (decisions 8 and 12) — parallel after Step 1
Files:
- `app/src-tauri/src/commands/backups.rs`;
- `crates/grid-core/src/library/backup_job.rs`, `crates/grid-core/src/backup/local.rs`,
  `crates/grid-core/src/backup/plan.rs`, and the `InstallService` file that defines
  `start_backup_on`;
- `app/src/lib/details/actions.ts` (keep `backupAction`'s signature; stop gating on it),
  `app/src/lib/tv/backupActions.ts`, `app/src/lib/backups/*` (dialog words);
- e2e: new `e2e/specs/backups-e.spec.ts` (+ group entry in `scripts/e2e.sh`).
- **Not** `Library.svelte` and **not** `api.ts`. If a type must change, add it to Step 1
  instead.

The concrete change:
- **`plan_backup` / `start_backup`:** `client: Option<Arc<RommClient>>` instead of
  `.ok_or(NEEDS_SERVER)?`. `NEEDS_SERVER` goes.
  - With a client: today's path, unchanged.
  - Without one: a new `plan_games_local(registry, request)` builds `PlanGame`s from the
    installed rows named in `request.rom_ids` (`platform_ids` mean server platforms and are
    ignored; the frontend already sends rom ids for "Backup all N…"). Sizes come from the
    local scan that `local_states` already does. `use_local` is forced on.
  - A game that is not eligible locally is shown unavailable with its `local.rs` reason plus
    "Connect RomM to back it up from the server".
  - Emulators and firmware without RomM (decision 12): emulator builds from GRID's catalog
    are still copied; server-hosted emulator builds and firmware are skipped and listed as
    "needs RomM".
- **`InstallService::start_backup_on(client: Option<…>, server: String, …)`**:
  `BackupJob::new` takes the pack identity's `server` explicitly instead of deriving it
  from the client.
- **`backup_job.rs` `download_local_game`:** with no client, a serverless branch:
  - metadata comes from the registry row (`pack_game_from_row`: title, platform name and
    slug, summary, genres, regions, languages, companies, release date, rating, tags,
    revision);
  - art comes only from the image cache (the row's cached cover paths and any cached
    screenshots/fanart; a missing image is a warning, never a failure);
  - no `exact_copy`, because there is no server file list: the scanned tree is always
    archived (the existing non-exact path, which install-from-drive already reads);
  - no `game_has_server_update`; the version comes from `rom_file_name`.
- **Pack identity with no server:** `PackSource { server, rom_id }`:
  - `rom_id` is the row's `rom_id`. Rows without one are already ineligible
    (`REASON_NO_ROM_ID`).
  - `server` is the row's `source_server` when set (a game installed from a drive), else
    the config's `server_url` (credential-free; Disconnect keeps it), else the game is
    ineligible: "GRID doesn't know which RomM server <title> came from".
  - `fs_name = row.rom_file_name`, `server_updated_at = row.server_updated_at`, `igdb_id`
    when the row has one.
  - `local::locate` gets that identity server as `live_server`, so its other-server check
    compares against the game's own server.
  - The same identity, written while connected, is byte-identical. A pack written without
    RomM therefore installs and dedupes exactly like one written with it.
- `set_drive_os` (adding a computer) still needs RomM. Only the words change.

Tests:
- Rust unit: `plan_games_local`; identity choice (source_server > config > ineligible);
  `pack_game_from_row`; the serverless `download_local_game` with a tempdir library
  (archive staged, art warnings); the plan without a client keeps catalog emulators and
  lists server builds and firmware as needs-RomM.
- Integration (`crates/grid-core/tests/`): backup with no client, then install from that
  pack with no client. Round trip, same `PackSource`.
- vitest: `details/actions`, `tv/backupActions` reasons.
- e2e `backups-e`: not connected, Library row → Backup to drive → written → install back on
  a fresh data dir.

### Step 3 — Desktop Library: RomM group, Installed, Discover and platforms; Server view removed — parallel after Step 1
Files:
- `app/src/lib/Library.svelte`, `app/src/lib/Shell.svelte`, `app/src/lib/library/rail.ts`
  (+ test), `app/src/lib/library/selection.svelte.ts`, `app/src/lib/views/fold.ts`;
- `app/src/lib/discover/*`, `app/src/lib/server/*` (the `Catalog` and header modules stay;
  move them if useful);
- new `app/src/lib/romm/desktop/*.svelte` (RomM rail group, Installed view banner,
  platform header, platform view);
- delete `app/src/lib/Server.svelte`;
- `app/src/lib/settings/openOn.ts` (+ test), `app/src/lib/settings/AppearancePage.svelte`
  (Open GRID on words; drop the Server card size);
- `app/src/lib/backups/drives.ts` (the `card_size_server` reader);
- `app/src/lib/gamestores/devMock.ts`;
- e2e: new `e2e/specs/romm-library.spec.ts` (+ group).

Done:
- top bar shows three views, no chip or menu, `Ctrl+1..3`;
- Library rail: All/Recent/Updates, Backup drives, then storefront groups in
  `STOREFRONT_ORDER` (Amazon, Epic, GOG, RomM, Steam); no PLATFORMS section;
- the RomM group per 0.3: Installed only when RomM is off; Installed, Discover, platforms
  when connected; selection falls back to Installed when rows disappear;
- Discover and platform views work as Server's did (install, play, select, Backup all N…,
  firmware and emulator chips, folding header, search);
- Library's `backupAction(session.connected)` call drops its argument;
- the banners match D1;
- every D1 Library inventory item is screenshot-checked.

Tests:
- vitest: `library/rail` (no platform rows at the top, `romm:installed` and
  `romm:discover` keys, storefront order), `settings/openOn`, `views/fold` placeholders;
- e2e `romm-library`: connected → group with Installed, Discover and the platform merge
  (installed + server), RomM between GOG and Steam; disconnect → group shows Installed
  only with the `off` flag, the installed game is still in All games and Recent; server
  stopped → Installed only, `!` flag, Retry banner; never connected and no RomM rows → no
  group.

### Step 4 — Desktop Settings: RomM card, Connection removed, needs-RomM states — parallel after Step 1
Files:
- `app/src/lib/Settings.svelte`, `app/src/lib/settings/pages.ts` (+ test; drop
  `connection`, label "Game stores"), `app/src/lib/settings/StoresPage.svelte` (cards in
  `STOREFRONT_ORDER`);
- new `app/src/lib/settings/RommCard.svelte`, `app/src/lib/settings/rommCard.ts` (+ test);
- delete `ConnectionPage.svelte`, `connection.ts` and its test;
- `CloudSavesPage.svelte`, `RetroAchievementsPage.svelte`;
- needs-RomM lines in `app/src/lib/details/*`. **Not** `Details.svelte` if Step 3 touches it.
  Today it does not need to; if it must, run Step 4 after Step 3;
- **Not** `app/src/lib/systems/*` (Step 7);
- e2e: new `e2e/specs/settings-romm.spec.ts` (+ group).

Done:
- the card matches D1 in every connection state and sits between GOG and Steam;
- Edit and Reconnect use `RommSignInForm`;
- a connect from the card that needs setup opens Choose your systems, and Skip/Done
  return to Game stores (through `romm.connect()` and `setupReturn`, 0.5);
- Disconnect confirms, stays in the app, and the Library group stays (Installed only) while
  installed RomM games remain;
- Cloud saves and RetroAchievements show their needs-RomM states;
- no link to `connection` remains.

Tests:
- vitest: `rommCard` (pill, lines, actions per state), `settings/pages`;
- e2e `settings-romm`: edit server, disconnect/reconnect, 401 → "Sign-in expired" →
  Sign in again; connect on a fresh profile with setup needed → picker → Skip → back on
  Game stores.

### Step 5 — Desktop welcome — parallel after Step 1
Files:
- `app/src/App.svelte` (phases; renders ChooseSystems for `setup` and honours
  `setupReturn`);
- new `app/src/lib/Welcome.svelte` and `app/src/lib/welcome.ts` (+ test);
- delete `Connect.svelte`, `connectPrefill.ts` and their tests;
- `connectFlow.ts` (only the library-folder helpers survive; rename to `welcomeFlow.ts`);
- `app/src/lib/stores/pythonImport.svelte.ts` (the welcome shows the notice);
- `app/src/lib/settings/SteamCard.svelte` is **read-only** here: import it; if it needs
  changes, the change belongs to Step 4;
- **Not** `app/src/lib/setup/*` (Step 7): App.svelte keeps mounting ChooseSystems as today;
- e2e: rewrite `e2e/specs/connect.spec.ts`, `first-run-setup.spec.ts` and
  `python-import.spec.ts` (their groups test exactly this screen).

Done:
- a fresh start shows the welcome;
- folder → storefronts (alphabetical) → Continue/Skip → shell, with no emulator step
  without RomM;
- a RomM connect leads to Choose your systems when setup is needed;
- an imported config is filled in;
- connected and offline users never see it.

Tests:
- vitest: `welcome` (step rules, row order, Continue enabled, what Skip writes);
- e2e as listed.

### Step 6 — TV mode — parallel after Step 1 (files under `app/src/lib/tv/`, except Systems)
Files:
- `TvShell.svelte`, `TvStage.svelte`, `nav.svelte.ts` (default tab `library`; drop
  `server`), `input/press.ts` (`TvTab`), `topbar.ts`, `TvTopBar.svelte`, `hints.ts`,
  `TvHintBar.svelte`, `overlays/menu.ts` and `TvMenu.svelte` (Reconnect goes), delete
  `reconnect.ts`;
- `tabs/library/*` (RomM group with Installed, Discover and platform screens moved in from
  `tabs/server/*`, which is then deleted);
- `startup/*` except `startup/choose.ts` (`TvConnect` → `TvWelcome`; `connectPage.ts` →
  `welcomePage.ts`);
- `tabs/settings/panes/connection.ts` (delete), `tabs/settings/TvSettings.svelte`,
  `panes/cloud.ts`, `panes/ra.ts`;
- `stores/TvStoresPane.svelte` (+ RomM card, alphabetical), `stores/libraryStores.ts`;
- `pages/TvSearchAll.svelte`, `pages/TvDetails.svelte`, `ui/TvGameCover.svelte`, as needed
  for the needs-RomM lines;
- **Not** `systemsTv.svelte.ts`, `pages/TvSystemPage.svelte`, `pages/TvEmulatorForm.svelte`,
  `tabs/settings/panes/systems.ts` or `startup/choose.ts` (Step 7). If `TvSettings.svelte`
  must change how it mounts the Systems pane, Step 6 owns that one line;
- e2e: rewrite `tv-server.spec.ts` → `tv-romm.spec.ts`, and update `tv-startup-*` and
  `tv-shell-*` (their groups).

Done:
- every D2 inventory item outside Systems is screenshot-checked at 1080p;
- the TV checklist items D2 marked are re-pointed;
- no flow tells the user to finish in desktop mode.

Tests:
- vitest: `nav` default tab, `topbar`, `hints`, `menu`, the TV library rail with the RomM
  group (Installed only when off; full when connected; alphabetical), the welcome page
  rows, the TV RomM card;
- e2e: `tv-romm`, `tv-startup`, `tv-shell`.

### Step 7 — Systems and firmware without RomM (decision 3) — parallel after Step 1
Files:
- repo root: new `platforms.json`, new `firmware-known.json` (both from a doc-research
  report run before this step: RomM's slug → name table; published BIOS tables with their
  licences);
- grid-core: new `crates/grid-core/src/systems/{mod.rs,catalog.rs,merge.rs}`,
  `crates/grid-core/src/lib.rs` (`pub mod systems;`), `launch/setup.rs`
  (`platform_setup` over `&[SystemPlatform]`), `launch/selection.rs` and
  `autoconfig/entry.rs` (label lookup through `systems::lookup`), `firmware/mod.rs`
  (`LocalFirmwareSource`), new `firmware/local.rs`, `firmware/manifest.rs` (local
  installs recorded), `firmware/rpcs3.rs` (a picked PUP);
- app layer: `app/src-tauri/src/platform_registry.rs` (after Step 1's move),
  `app/src-tauri/src/commands/setup.rs`, new `app/src-tauri/src/commands/systems.rs`
  (`system_platforms`, `firmware_expected`, `locate_firmware_files`,
  `locate_ps3_firmware`), `app/src-tauri/src/firmware_service.rs`,
  `app/src-tauri/src/lib.rs` (its registration lines and the startup `record(None)` call
  only);
- desktop: `app/src/lib/systems/*`, `app/src/lib/setup/*` (ChooseSystems rows from
  `SystemPlatform`; the "Disconnect" footer goes), `app/src/lib/emulators/defaults.ts`;
- TV: `app/src/lib/tv/systemsTv.svelte.ts`, `tv/pages/TvSystemPage.svelte`,
  `tv/pages/TvEmulatorForm.svelte` (if needed), `tv/tabs/settings/panes/systems.ts`,
  `tv/startup/choose.ts`, and the TV file browser's file mode (its current file, read
  first; if it is shared with Step 6's welcome folder browser, Step 7 adds the file mode
  only after Step 6 merges);
- e2e: new `e2e/specs/systems-local.spec.ts` (+ group; desktop and TV stages), a fixture
  folder of fake firmware files whose MD5s match a test-only `firmware-known` override
  (the e2e build reads it from an env var, as other e2e hooks do). Never real BIOS files.
- **Not** `api.ts` (Step 1 added the types), **not** `commands.rs`, **not**
  `library/mod.rs`.

Done:
- with RomM never connected: Settings › Systems lists the catalog systems per 0.12;
  choosing and installing an emulator works; the Windows (Proton) row works;
- a game's launch resolves its RetroArch core by slug with no server (registries warmed
  from the catalog);
- Locate files installs a matching file under its known name into the emulator's folders,
  marks it installed, refuses a mismatch with its reason, and handles a folder and a zip;
- Install from RomM is disabled with "Needs RomM" when RomM is off, and works as today
  when connected (server firmware list merged into the expected set);
- with RomM connected: server platforms are merged; a custom server name still finds the
  emulator set under the catalog name (label lookup);
- every D1/D2 Systems inventory item is screenshot-checked (desktop, and TV at 1080p).

Tests:
- Rust unit (`systems`): catalog parses and covers every slug of `platform-emulators.json`
  and `romm-platform-cores.json`; every profile with keywords matches at least one catalog
  name; `merge` (catalog only, server only, both, server label wins, unknown server slug,
  installed-only row, Emulators shelf dropped); `lookup` (label, alias, custom name).
- Rust unit (`firmware/local`): MD5 match under another name, size mismatch, MD5 mismatch,
  no-hash entry, unknown file, oversize file refused before read, folder scan depth,
  archive whole vs members, no emulator installed → refused.
- Rust unit (`platform_registry`): `record(None)` fills slugs from the catalog and leaves
  ids empty; `record(Some)` lets server entries win.
- Integration (`crates/grid-core/tests/`): Locate files through
  `install_platform_firmware_from(LocalFirmwareSource)` into a tempdir RetroArch layout,
  then the manifest says installed.
- vitest: `systems/rows` without RomM, the firmware block's actions per state, the Locate
  report reducer, TV `panes/systems`, `setup/choices` over `SystemPlatform`.
- e2e `systems-local`: no RomM → Systems rows → Locate files with the fixture folder →
  installed and refused lines; TV stage the same through the TV file browser.

**Parallel file check (Steps 2–7).**
- Disjoint: 2 (backend backups + backups TS), 3 (Library/Shell/discover/server/openOn),
  4 (settings pages and card, not systems), 5 (App/Welcome/import notice), 6 (tv/** except
  the Systems files), 7 (systems, setup, firmware, platform registry, the two JSON files,
  the TV Systems files).
- Shared files, each owned by one step:
  - `api.ts`, `session.svelte.ts`, `shell.ts` and `commands.rs` belong to Step 1; Steps 2–7
    do not edit them;
  - `Details.svelte` is owned by Step 3 if needed;
  - `TvSettings.svelte` is owned by Step 6;
  - `app/src-tauri/src/lib.rs`: Step 7 adds only its command registrations and one startup
    call; nothing else in 2–6 edits it. A conflict there is mechanical;
  - `scripts/e2e.sh`: each step adds only its own group line. A conflict there is
    mechanical.
- Merge order: 2, 3, 4, 5, 6, 7, running `npm run check` and the gate after each merge (the
  `SettingsPage` union losing `connection` is the one type-level link between 3 and 4;
  Step 7 compiles against Step 1's `api.ts` types, so it does not depend on 2–6).

### Step 8 — e2e migration (one implementer, after Steps 2–7 merge)
Files:
- new `e2e/helpers/romm.ts`:
  - `connectRomm()`: through the welcome on a fresh data dir, else through
    Settings › Game stores;
  - `openRommInstalled()`, `openRommDiscover()`;
  - `openRommPlatform(slug)`: by `library-rail-platform-<slug>`;
- `e2e/helpers/server.ts` (delete or turn into a re-export), `e2e/helpers/openOn.ts`,
  `e2e/helpers/tv.ts`, `gog.ts`, `epic.ts`, `amazon.ts`, `libraryMove.ts`, `backups.ts`,
  `env.ts`;
- every spec listed under "e2e impact" below, including `systems-local` (re-pointed from
  "Use without a server" to the welcome's Skip); `scripts/e2e.sh` (group comments: Connect
  → welcome; python-import ordering note).

Done: every group passes on its own.

### Step 9 — legacy removal, full gate, full suite
Files:
- `app/src-tauri/src/commands.rs`, `lib.rs`, `crates/grid-core/src/session.rs` (delete
  `connect`/`restore_session`/`retry_connect`/`disconnect`/`use_without_server` wrappers,
  `RestoreOutcome`, `use_offline`, `SESSION_UNAUTHORIZED_EVENT`);
- `api.ts`, `shell.ts` (`applyRestore`, `applyRetry`, `offlineSession`, `chipLabel`),
  `session.svelte.ts` (`offline`, `lastError`, `rejected`, `connectToServer`);
- re-exports left by Steps 1, 5 and 8.

Then:
- the gate;
- `scripts/ctr.sh wincheck` (only if a step added `cfg(windows)` code; Step 7's file picker
  and path canonicalization are the likeliest);
- the **full e2e suite** `scripts/e2e.sh` (all default groups);
- an api-tester pass on connect, restore, disconnect, platforms, games and firmware
  (list and content) against the live RomM as `tester` (affected endpoints, per the
  `api-verification` skill);
- a native `.rpm` build for the user's own test.

### Migration and compatibility
- **Connected users:** the migration sets `stores.romm.connected = true` and
  `setup.welcomed = true`. Startup restores as today, with no new sign-in, and they land in
  the Library on RomM › Discover (Automatic). A stored 401 shows "Sign-in expired" on the
  card and the group (Installed only) instead of the Connect form.
- **Offline users:** `connected = false`, `welcomed = true`. They go straight to the
  shell with no probe and no keyring read. The RomM card shows Not connected with the
  stored server and username filled in. The RomM group shows Installed only while they have
  installed RomM games. Their keyring credential stays, unused (decision 12). Settings ›
  Systems now works for them.
- **Python import:** unchanged import. It lands on the welcome, filled in (decision 12).
  The import notice shows over the welcome, as it does over Connect today.
- **Systems config:** `default_emulators`, `retroarch_cores` and `setup.pending` keep their
  label keys. A key written under a server label still resolves without RomM when the
  label equals the catalog name or one of its aliases; a key under a custom server name
  resolves through the slug's aliases only while RomM is connected (the server supplies
  that alias). Without RomM the row shows "no emulator chosen" until the user picks one,
  which is written under the catalog label. Nothing is deleted.
- **Drive packs:**
  - `gridpack.json` and `PackSource` do not change.
  - Install from a drive already works without a server, and rows keep `source_server`.
  - Packs written without RomM (Step 2) have the same identity as packs written with it.
  - Firmware from a drive (`spawn_from_pack`) is unchanged.
  - Drive rows stay in Library › Backup drives.
- **Unlinked rows** (`rom_id` None) and rows from another server:
  - while connected, they appear in their platform row of the RomM group (key rule 0.3);
    they are always in Installed and All games;
  - Link to server game, cloud saves and updates show needs-RomM when RomM is off;
  - connect-time relink (`relink_service::spawn_connected_jobs`) is unchanged.
- **Downgrade:** an older build reading a migrated config sees `offline` absent and
  `server_url` set. It probes if a credential is stored, else shows Connect. Firmware
  installed through Locate files is in the manifest under its known name, which an older
  build reads as installed. That is acceptable and nothing is lost.
- **Image cache:** `SessionManager::server_url` is set from config whenever one is stored,
  so cached RomM art keeps showing while RomM is off (today only offline mode set it
  without a probe).

### Risks
- **e2e churn.** 42 specs type into the Connect form (`connect-server-url`, 95 uses) and
  about 40 open `platform-btn-<id>` in the Server rail. Step 8 does this through one helper
  and runs alone. Until it lands, those old groups fail; only the new per-step specs are run.
- **Large platforms.** The merged platform grid must stay as fast as Server's
  (`large-platform`, `large-platform-perf`). `platformGames.ts` must be O(n) with a
  `Map<rom_id,row>`, and the grid keeps `$state.raw`.
- **Label drift between the catalog and RomM.** Config is keyed by label; a server with
  custom names, or a RomM release that renames a platform, makes the label differ from
  the catalog name. The alias lookup (0.9) covers it while connected; tests must include
  a custom-named platform. `platforms.json` names must come from RomM's own table, not be
  typed by hand.
- **Catalog drift.** RomM adds slugs over time. A server platform the catalog does not
  know still shows while connected (`Server` origin) and gets keyword-matched emulators,
  but not without RomM. A unit test ties the catalog to the two existing slug files, so a
  new recommendation cannot land without a catalog entry.
- **Firmware table data.** `firmware-known.json` is new data with real hashes. Wrong
  entries refuse good files. It needs a doc-research source per entry and a licence check
  (libretro's System.dat, emulator docs). Fixtures use fake files and a test-only table;
  no real BIOS file or hash goes into a fixture.
- **Locate files input.** User-picked paths: canonicalize, regular files only, size cap
  before reading, no symlink following in folder scans, temp-dir extraction for archives
  (the existing extractor's path rules). Key files are personal: never log contents.
- **TV file browser.** Locate files on TV needs a file mode in the TV browser; if that
  browser is shared with Step 6's welcome, Step 7 waits for Step 6's merge for that file.
- **Startup cost.** `system_platforms` at startup reads the registry and parses two small
  JSON files; it must not block the window or touch the keyring.
- **401 mid-session no longer leaves the shell.** Every RomM call site must already handle
  `client()` returning None mid-flow (they gate on it). Watch the Details pages, the
  first-run setup card, and a platform view open when the rows disappear (selection falls
  back to Installed).
- **Keyring prompts.** `romm_restore` must keep the rule "not connected → no keyring read".
  Otherwise offline users get a secret-service unlock prompt at every start.
- **Rail key collisions.** A server slug can equal another platform's name token (the
  third fallback in 0.3). `platformKey.ts` tests must cover it. A server slug wins.
- **TV checklist drift.** `docs/tv-mode/checklist.md` items about the Server tab, Connect,
  the offline hint and RomM-only Systems must be re-pointed in D2, not silently left.

(The earlier "Proton without RomM" risk is gone: Systems, with its Windows (Proton) row,
now works without RomM.)

### e2e impact
- **Connect screen** (`connect-*` test ids): connect, connect-restore-a/b/c, python-import,
  first-run-setup, layout-migration, library, library-grid, install-a/b/c, downloads,
  emulators, emulator-catalog, server-emulator, launch, cloud-saves, images-a/b,
  ps3-install, content, native, firmware, updates, systems, large-platform(-perf),
  library-move-c, backups-a/c/d, discover, open-on-a, settings-screens, ui-screens,
  gog-library, epic-library, amazon-library, and TV: tv-startup-a/c/d, tv-details-a,
  tv-backups-a/e, tv-gog-a/b, tv-amazon, tv-epic, tv-steam, tv-systems, tv-server,
  tv-large-platform, tv-layout, tv-settings; helpers `gog.ts`, `epic.ts`, `tv.ts`,
  `libraryMove.ts`.
- **Server view and rail** (`platform-btn-*`, `server-rail-discover`,
  `server-platform-header`, `server-grid`, `server-section`): the specs above plus
  open-on-a, discover, library-move-a/d, gog-romm-installer, backups-b, tv-backups-a/e;
  helper `server.ts`.
- **Library PLATFORMS section** (`library-rail-platform-*` at the top of the rail today):
  library, library-grid, install-*, launch, layout-migration, library-move-*, backups-*,
  tv-layout. Those rows now exist only inside the RomM group and only while connected;
  specs that open a platform with RomM off move to `library-rail-romm-installed`.
- **Chip, offline and "Use without a server"** (`session-chip`, `session-retry`,
  `session-disconnect`, `connect-offline`, `tv-connection`): connect-restore-c, open-on-a/b/c/d,
  images-a/b, backups-c/d, tv-backups-c, tv-details-c, tv-server, tv-startup-a/c,
  tv-shell-a, tv-gog-b, tv-settings, gog-offline, epic-library; helpers `openOn.ts`, `env.ts`.
- **Connection page:** settings-screens, updates, connect.
- **Systems and firmware** (Step 7 changes the data source and the firmware block):
  systems, firmware, emulators, emulator-catalog, server-emulator, ps3-install,
  first-run-setup (ChooseSystems footer), tv-systems. They keep their RomM path (Install
  from RomM) and gain assertions for the two firmware buttons; the mock server's
  `firmware.json` fixtures stay.
- **Store order:** store-rail and Game stores assertions that check group or card order
  (gog-library, epic-library, amazon-library, settings-screens, tv-settings) see RomM
  between GOG and Steam.
- **New specs:** romm-library (Step 3), settings-romm (4), backups-e (2), tv-romm (6),
  systems-local (7). Rewritten: connect → welcome, first-run-setup, python-import (5),
  tv-startup-*, tv-shell-* (6), open-on-a..d (8).
- **Final:** Step 9 runs the full default suite (`scripts/e2e.sh`, all groups except
  `OPT_IN_GROUPS`) and the opt-in `large-platform-perf` once for the merged grid.

### Locate files: unverifiable firmware (user decision 13, 2026-11-15)

Checksum-matched files install as verified. A file whose name matches a known entry that
has no fixed checksum (per-console keys such as Switch `prod.keys`, Wii U `keys.txt`)
installs as "Installed · not verified". Files that match nothing are refused.

### Design sign-off (user decisions 14–17, 2026-11-15)

The user approved the RommStorefront (desktop) and TVRommStorefront (TV) prototypes,
including the designers' own calls (startup Connecting state, one welcome button that reads
Skip for now until something connects and then Continue, Choose your systems after
Continue, More systems group, Cloud saves and RetroAchievements forms editable without
RomM). Where the two designers differed:

14. **Platform row count is RomM's game count only.** Installed games RomM doesn't have
    show in the grid but are not counted. (The desktop prototype is changed to match.)
15. **Two cover tags:** NOT LINKED for an installed game with no RomM link, NOT ON ROMM for
    one from another server or removed from this one. (The TV prototype gains NOT ON ROMM.)
16. **Never connected → GRID opens on All games** (Automatic and Library), not the first
    Backup drive. (The TV prototype is changed to match.)
17. Where the two prototypes' words differ, desktop (D1) wins; `romm/words.ts` is shared.

No open questions remain.

## Step 1 outcome (landed 2026-11-15; Steps 2–7 build on this)

Commits a08ae6b, b251888, 480dae9. Gate PASS; e2e python-import, connect, connect-restore,
open-on, first-run-setup, tv-startup, tv-settings, install pass through the wrappers.

**Frozen for Steps 2–7:** `api.ts`, `shell.ts`, `stores/session.svelte.ts`, `commands.rs`.
Anything missing there comes back to the orchestrator, not edited in a parallel step.

- Rust (`session.rs`): `RommConnection { Connected, Unreachable, Expired, NotConnected }`
  (snake_case); `RommStatus { connection, server_url, username, auth_kind, last_error,
  installed_count, in_rail, busy }`; `SessionManager::{set_change_hook, status, prime,
  restore_romm, reconnect}`; `connect` saves `stores.romm.connected = true`; `disconnect`
  saves false and keeps `server_url`/`username`; a 401 sets Expired, keeps the keyring.
  "Connecting" = `busy && connection == not_connected && server_url != ""`.
- Config: `StoresConfig.romm: RommConfig { connected }`, `SetupState.welcomed`,
  `romm_store_migrated`; `offline` written only while true and cleared by the migration.
- Commands: `romm_status`, `romm_restore`, `romm_connect`, `romm_reconnect`,
  `romm_disconnect` (`commands/romm.rs`, `ROMM_CHANGED_EVENT`); `welcome_status`,
  `finish_welcome` (`commands/welcome.rs`). Legacy wrappers kept: `connect`,
  `restore_session`, `retry_connect`, `disconnect`, `use_without_server`.
  `session-unauthorized` still emitted beside `romm-changed` until Step 9.
- TS (`api.ts`): `RommConnection`, `RommStatus`, `ROMM_CHANGED_EVENT`, `WelcomeStatus`, the
  0.10 Systems types, `InstalledGame.source_server?`; wrappers `rommStatus`, `rommRestore`,
  `rommConnect(serverUrl, username, secret, useToken)`, `rommReconnect`, `rommDisconnect`,
  `welcomeStatus`, `finishWelcome`, `systemPlatforms`, `firmwareExpected(slug, emulator)`,
  `locateFirmwareFiles(slug, emulator, paths)`, `locatePs3Firmware(emulator, path)` (the
  last four's Rust commands come in Step 7).
- `shell.ts`: `SessionPhase = 'loading'|'welcome'|'none'|'setup'|'shell'` (`none` = legacy
  Connect), `SetupReturn`, `initialPlace(rommConnected, openOn)` → `{ view: 'library',
  rail: 'romm:discover'|'all', drive: null }`, `STOREFRONT_VIEWS`, `storefrontViewForDigit`.
  Legacy `VIEWS`/`initialView` kept until Step 9.
- `stores/romm.svelte.ts`: `romm.status`, `romm.loaded`; `initRomm`, `refreshRomm`,
  `restoreRomm`, `reconnectRomm`, `disconnectRomm`, `connectRomm(url, user, secret,
  useToken, from)` (returns error text or null; `from='settings'` sets
  `session.setupReturn='settings'` when setup is needed).
- `stores/session.svelte.ts`: `session` is a class instance (don't spread it) with
  `setupReturn`, `start()` (welcome, or the shell plus a background `restoreRomm`),
  `finishWelcome()`.
- `lib/romm/`: `platformKey` (`platformKeyer`, `serverPlatformSlug`, `platformRailKey`),
  `order` (`STOREFRONT_ORDER`), `rail` (`rommRailGroup(status, platforms|null,
  installedRows)` → `{ heading, flag, rows }|null`, `RommRailKey`, `rommSelectionFallback`,
  …), `platformGames` (`platformGames(serverGames, installedRows, liveServer)`,
  `platformGameCounts`, `localTagLabel`), `words` (all RommStorefront README strings),
  `signIn` (`signInError`, …; `connectForm.ts` re-exports).
- `romm/platformKey.ts` and `romm/rail.ts` import `platformSlug` (`library/rail.ts`) and
  `platformLabel` (`server/platformLabel.ts`): keep those exports or re-export if moved.
- **`RommSignInForm.svelte` is owned by Step 4** (in `lib/romm/`); Step 5 starts after Step 4
  merges and reuses it.
- Step 7 owns `platform_registry.rs` (`record(state, None)` returns early today) and adds
  the `system_platforms` startup warm-up in `lib.rs`.

## Leftovers for Step 9 (collected from Steps 2–7)

Each item is marked with how Step 9 settled it.

- ~~**Step 8 first:** `romm-library-b` still types into the removed Connect form; check
  `settings-romm` for leftover `connect-*` ids.~~ Done in Step 8 (verified: it connects with
  `connectRomm()`; `settings-romm` has no `connect-*` ids).
- ~~api.ts: move `BackupPlan.without_romm`, `unavailable` and `PlanUnavailable` into
  `api.ts`.~~ Done: `BackupPlan` carries them (and `firmware_skipped`); `RommOffFields` is gone.
- ~~`settings/BackupDrivesPage.svelte`: "The server's firmware for the systems you backup"
  → "Firmware from RomM for the systems you backup".~~ Done.
- ~~Backup firmware line without RomM reads "Needs RomM · skipped" with no system names.~~
  Done: `plan::firmware_skipped_systems` names the systems of the games being written that
  `firmware-known.json` lists required firmware for; desktop reads "Needs RomM · skipped:
  PlayStation 2". TV's Firmware row keeps "Needs RomM" (TVRommStorefront has no names).
- Desktop: "Open Config Folder" left with the Connection page (ask the user where it goes).
  **Not done: waiting on the user's decision.**
- ~~TV Backup page: pass the unlinked rows to `unavailableRows`.~~ Done: RomM › Installed's
  Backup all N… passes them (and counts them, as desktop does).
- ~~e2e: a TV Can't reach state.~~ Done: `tv-romm-b` (the `tv-romm` group's second launch).
- ~~Remove the legacy `'none'` phase branches in `App.svelte` and `TvShell.svelte`.~~ Done.
- ~~Step 7: move `commands/systems.rs` to `pub mod systems;`; add `SystemPlatform.name` and
  `LocateReport.unmatched` to `api.ts`; `FirmwareExpectedFile` region and `FolderEntry`
  size; `Settings.svelte` passes `onConnectRomm`; TV Choose your systems' "RomM is
  connected." sub-line.~~ Done: the module mount, both fields (the local extended types are
  gone), `region` (grid-core `firmware::known::region_of`, file lines "Required · Japan")
  and `size` (TV file browser rows "512 KB"), `onConnectRomm` required (no rail-click
  fallback). The TV sub-line had already landed in Step 6 (2b58f1e).
- ~~Step 8 (Systems e2e): `first-run-setup` PS2 `firmware`; `systems.spec` old words.~~ Done in
  Step 8 (verified).
- ~~Remove `session-unauthorized`, the legacy wrappers, `VIEWS`/`initialView`, `'none'`
  phase.~~ Done.

## Step 9 outcome (landed 2026-11-15)

Commits 0acf6a4 (removal and leftovers) and 45ef0e5 (e2e). `scripts/ctr.sh gate` PASS;
`scripts/ctr.sh wincheck` PASS; the full suite `scripts/ctr.sh e2e` on a fresh build: all
96 stages passed, no retries. Not run here: the api-tester pass and the native `.rpm`
build the Step 9 list names (the orchestrator's call).

**Removed** (each proven unused by grep, then by the gate's build and tests):
- Rust: the `connect`, `restore_session`, `retry_connect`, `disconnect` and
  `use_without_server` commands and their `lib.rs` registrations;
  `SESSION_UNAUTHORIZED_EVENT` and the unauthorized hook (`SessionUnauthorized`,
  `UnauthorizedHook`, `set_unauthorized_hook`); `RestoreOutcome`,
  `SessionManager::{restore, retry, use_offline}` and `legacy_outcome`;
  `commands/romm.rs` `disconnect_now` (folded into `romm_disconnect`); the `#[path]` mount
  of `commands/systems.rs`. A 401 mid-session still sets Expired and fires `romm-changed`.
- TS: `api.ts` `SessionState`, `SessionUnauthorized`, `RestoreOutcome`,
  `SESSION_UNAUTHORIZED_EVENT` and the `connect`/`restoreSession`/`retryConnect`/
  `disconnect`/`useWithoutServer` wrappers; `shell.ts` `VIEWS`, `viewForDigit`,
  `initialView`, `ShellSession`, `applyRestore`, `applyRetry`, `offlineSession`,
  `applyUnauthorizedEvent`, `chipLabel` and the `'none'` phase (`View` is now the
  storefront views); `session.svelte.ts` `offline`, `lastError`, `rejected`, `busy`, the
  no-op setters and the legacy `restore`/`connect`/`retry`/`disconnect`/`useOffline`/
  `connectToServer`/`listenUnauthorized`; the `'server'` view in `Shell.svelte` and
  `uiMode.ts`, and `uiMode.ts`' `'connection'` page mapping; `App.svelte`'s and
  `TvShell.svelte`'s `'none'` branches (TvShell's `welcomeNeeded` probe with them);
  `firstLibraryDriveKey` and `drives.firstLibraryDrive` (the offline shell's first drive);
  the dev mock's `restore_session`; `e2e/helpers/server.ts`.
- Tests of removed code went with it or were moved onto the API that replaced it:
  `crates/grid-core/tests/session.rs` now checks `restore_romm`/`reconnect` through
  `RommStatus` (each legacy outcome's case kept: Not connected with the stored server and
  user, Connected, Expired with its auth kind and no secret on the wire, Unreachable with
  its error, a 403 not Expired, a mid-session 401 changing the status once, a 401 on a
  replaced client doing nothing); `shell.test.ts` keeps `viewLabel`, `hostOf`,
  `initialPlace` and the `Ctrl+<n>` keys; `uiMode.test.ts` loses the `'server'` view case;
  `App.svelte.test.ts` renders the toast in the welcome phase instead of `'none'`.

**Behaviour changed by design:** `backups-e` now expects "Needs RomM · skipped: PlayStation
2" (it read "Needs RomM · skipped"): its games are PlayStation 2 games, whose BIOS
`firmware-known.json` lists as required (RommStorefront README, "Needs RomM · skipped: Sega
Saturn").

**New checks:** `tv-romm-b` (Can't reach after a connect: the rail flag, no Discover or
platforms, the Installed banner and a failing Retry, the Game stores pill, the RomM page's
line and Reconnect, then Reconnect with the server back); `systems-local` asserts a file
line's region ("Required · North America"); `tv-systems-local` asserts a file row's size.

**Held for the user:** "Open Config Folder" placement, and the backend install path for
server-hosted emulator files that don't follow `docs/server-emulator-packages.md` naming
(now unreachable from the UI; left as is).

## After this rework: Discover

The user wants Discover reworked once this rework lands (2026-11-15). The trigger: when
RomM has nothing released in the last six months for the user's systems, the New releases
coverflow (Discover's first spot) is only an empty note; something else should take that
spot. Design it as its own step after Step 9, not inside Steps 3/6.

## Decisions after Step 8 (2026-11-15)

- **Open Config Folder** moves to Settings › Library, under the library folder row (no General page).
- **Emulators platform:** when the server has an Emulators platform, it gets a row in the
  RomM group like any other platform (desktop and TV). Its view installs the server's
  emulator files through the existing server-emulator path, which stays (both named-to-rule
  builds and other files). RomM › Installed and All games still leave emulator files out.
  Designed first, built after Step 9.
- **Server build beside catalog copy:** keep both; the user picks per system in Settings › Systems.
- **"How to name builds"** links to `docs/server-emulator-packages.md` on GitHub.

## Emulators row and Config folder outcome (built 2026-11-15)

Built from the RommStorefront and TVRommStorefront cards' "RomM › Emulators" and
"Settings › Library: Open config folder" sections.

- **Backend.** `launch::emulators_view` sorts every Emulators-platform file into
  Emulators, Other files and Other builds (pure, unit-tested). Emulators install through
  the existing `install_server_emulator` (kept, as is the whole-folder plain install of
  `install(rom_id)`); an Other file installs one file through the same download path as a
  plain file (`install_server_file`, `JobTarget::ServerFile`, `games/Emulators/<folder>`,
  unpacked when GRID knows the archive, never registered, no registry row: installed
  state is read from the disk). A wrong-processor build fails with
  `FailureCode::WrongMachine` (what it is built for), which the row and the card word.
  Commands: `commands/romm_emulators.rs`, `config_folder_path`.
- **Frontend.** `lib/romm/emulators.ts` (rail row, row states, words),
  `lib/romm/emulators.svelte.ts` (the shared list and actions),
  `RommEmulatorsView.svelte`, `TvRommEmulators.svelte`, `TvRommOtherBuilds.svelte`;
  Settings › Library's Config folder block.
- **Choices the cards leave open:** the rail row shows without a count while its list
  loads or failed; a single-system folder names the server's platform for its slug
  ("Sony PlayStation 2"), a `multi` folder the profile's systems; the catalog-copy line
  names the entry the build registers as (the profile's full name plus " (server)").
