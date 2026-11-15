# RomM as an optional storefront

Status: decisions settled 2026-11-15; plan written 2026-11-15. Open questions Q1–Q8 (end of
the plan) wait for the user's answers. They are not decided.

GRID stops being "a launcher for RomM" with stores beside it. RomM becomes one optional
storefront next to GOG, Epic, Amazon and Steam. Its engine (platforms, emulators,
firmware, per-emulator cloud saves, relink, drive packs) stays; what changes is how it is
reached, set up and shown.

## User decisions (2026-11-15)

1. **Browsing lives in the Library.** The Library rail gets a RomM group, as the stores have:
   Discover, then one row per platform, with installed and server games together. The
   Server tab is removed (desktop and TV). Without RomM connected the group is not shown
   (but see 5).
2. **First run is a neutral welcome.** Pick the library folder, then optionally connect any
   storefronts (RomM, GOG, Epic, Amazon, Steam), or skip and add them later in Settings.
   No RomM-first Connect screen.
3. **Systems stays RomM-only.** Choose your systems and Settings › Systems keep using the
   server's platform list and are unavailable without RomM.
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
  `app/src/lib/gamestores/rail.ts` (store groups: fixed Installed/Available rows),
  `Library.svelte` assembles both; TV `tv/tabs/library/*`, `tv/stores/libraryStores.ts`.
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

## Plan

Order: **Step 0** (contracts, below) → **D1 ∥ D2** (design, desktop and TV) → user sign-off →
**S** (SPEC/ARCHITECTURE) ∥ **Step 1** (foundation) → **Steps 2 ∥ 3 ∥ 4 ∥ 5 ∥ 6** in parallel
worktrees → **Step 7** (e2e migration) → **Step 8** (legacy removal, full gate, full e2e
suite). Every step ends with the gate (CLAUDE.md). Each step commits with
`git commit --only <its paths>`.

The work is additive first. Step 1 adds the new RomM status, commands and frontend stores
*beside* the old ones, and the old commands become thin wrappers over the new state. This
keeps the gate green after every step. Each UI step deletes the old screens it replaces.
Step 8 deletes the old commands and fields once nothing uses them.

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
    /// connection != NotConnected || installed_count > 0 (decision 5).
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

**0.3 Rail keys for the RomM group.**
- `romm:discover`: Discover (test id `library-rail-romm-discover`).
- `platform:<slug>`: one row per platform (test id `library-rail-platform-<slug>`). The key
  stays `platform:` so `matchesRail` and selection keep their shape. What changes is how
  `<slug>` is resolved, in a new pure `app/src/lib/romm/platformKey.ts`:
  1. a server platform: its `slug`;
  2. an installed RomM row: `platform_slug` when not empty; otherwise the slug of the
     server platform whose `platformLabel` equals the row's `platform` (when connected);
     otherwise `platformSlug(row.platform)` (today's name token).
  This keeps an unlinked or pre-v8 row in its platform's row while connected. When not
  connected, it gets a row of its own name.
- The rows are the union of the server platforms (`rom_count > 0`, not the Emulators shelf)
  and the installed RomM platforms. They are sorted by label. The label is the server's
  `platformLabel` when known, else the row's `platform`.
- `RailKey` (`app/src/lib/library/rail.ts`) gains `'romm:discover'`. The top section becomes
  All games, Recent, Updates only. The platform rows move into the group.
- A platform view's grid is a merge (pure, `app/src/lib/romm/platformGames.ts`): the
  server's `GameSummary[]` for the platform, each matched to an installed row by `rom_id`
  (and `source_server` empty or equal to the live server). It adds the installed rows that
  match no server game (unlinked rows, rows from another server, games gone from the
  server). Not connected: installed rows only. The Discover feed and `Catalog`
  (`app/src/lib/server/catalog.svelte.ts`) are reused unchanged and move from the Server
  view into the Library.
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
  keyring. Their keyring credential is left in place (Q5).
- The Python importer (`crates/grid-core/src/import_python.rs` `plan`) sets
  `romm_store_migrated = true`, `setup.welcomed = false`, `stores.romm.connected = false`
  explicitly. An imported user sees the welcome with the library folder and the RomM
  server and username filled in (Q6).

**0.5 Startup phases** (`app/src/lib/shell.ts` `SessionPhase`, `stores/session.svelte.ts`).
- `'loading' | 'welcome' | 'setup' | 'shell'`. `'none'` is renamed `'welcome'` and is entered
  only while `setup.welcomed` is false (new command `welcome_status`). It is never entered
  because of the RomM state.
- `'setup'` (Choose your systems) follows a *successful RomM connect* when
  `setup_status().needed` (today's rule). From the welcome it comes before the shell. From
  the Settings card, see Q2.
- Startup: `welcome_status` → `romm_restore` (only when `stores.romm.connected`; it runs in
  the background after the shell is up, so an unreachable server never holds the window) →
  shell.
- Disconnect, a 401, an unreachable server: the shell stays. Only `RommStatus` changes.
  Nothing returns to a Connect screen. Connect, `connectToServer()` and the `none`
  branch in `App.svelte` and `TvShell.svelte` go away.
- Where GRID opens (`shell.ts` `initialView`, which desktop and TV share): it now returns
  `{ view: 'library', rail: RailKey | null, drive: string | null }`.
  - Automatic: `romm:discover` when RomM is connected at startup, else All games (else the
    first Backup drive when RomM was never connected).
  - `discover`: `romm:discover`. This shows the Discover "not connected" panel when RomM is
    off.
  - `library`: All games / first drive, as today.
  - The stored values (`automatic`, `discover`, `library`) do not change, so no migration.
- `VIEWS` becomes `['library','downloads','settings']`, and `Ctrl+1..3`.

**0.6 IPC commands** (`commands/romm.rs`, registered in `app/src-tauri/src/lib.rs`).

| Old | New | Notes |
|---|---|---|
| `restore_session` → `RestoreOutcome` | `romm_restore` → `RommStatus` | Probes only when `stores.romm.connected`. Sets `server_url` for image filtering whenever the config has one, even when not connected. Spawns connected jobs on success. |
| `connect` → `SessionState` | `romm_connect(server_url, username, secret, use_token)` → `RommStatus` | Sets `stores.romm.connected = true` in the same save. `UsernameMismatch` is unchanged. |
| `retry_connect` | `romm_reconnect` → `RommStatus` | Re-probes with the stored credential (Unreachable → Retry). `Err` when none is stored. |
| `disconnect` | `romm_disconnect` → `RommStatus` | Drops the client, clears the keyring, sets `connected = false`, clears updates. Keeps `server_url`/`username` for prefill. Stays in the shell. |
| `use_without_server` | removed (Step 8) | |
| — | `romm_status` → `RommStatus` | Pull, for stores that mount late. |
| — | `welcome_status` → `{ needed: bool }`, `finish_welcome()` | In `commands/setup.rs`. |
| `open_server_page` | unchanged | The card's "Open in browser". |
| event `session-unauthorized` | event `romm-changed` (`RommStatus`) | Emitted on every connection change, including a 401. |

Steps 1–7 keep the old commands as wrappers over the new state. `restore_session` maps
not-connected-and-welcomed to the `offline` kind, so the old shell still opens. Step 8
deletes them, together with `RestoreOutcome`, `SessionState`'s IPC use, `SessionManager::use_offline`
and `SESSION_UNAUTHORIZED_EVENT`.

**0.7 Frontend stores** (Step 1).
- `stores/romm.svelte.ts` holds `romm.status: RommStatus` (pulled once, then `romm-changed`),
  plus `connect`, `reconnect` and `disconnect`.
- `session.svelte.ts` keeps `session.phase`. `session.connected` becomes a getter for
  `romm.status.connection === 'connected'`, so the ~30 readers do not change.
- `session.offline`, `lastError` and `rejected` stay until Step 8, but are no longer
  written to by the new code.

**0.8 Pure modules** (Step 1, used by desktop and TV): `app/src/lib/romm/`:
- `platformKey.ts`;
- `rail.ts`: `rommRailGroup(status, platforms, installedRows, updates)` →
  `{ heading:'ROMM', flag, rows: [discover, ...platforms] } | null`. `flag` is `!` for
  expired or unreachable and `off` for not connected, like `storeFlag`;
- `platformGames.ts`;
- `words.ts`: card pill, account line, needs-RomM reasons per feature ("Cloud saves need
  RomM", …, exact words from D1);
- `RommSignInForm.svelte` (desktop) and `signIn.ts`: the URL / token-or-password form taken
  out of `Connect.svelte` and `connectForm.ts`. The welcome and the card's Edit/Reconnect
  dialog use it.

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
  - step 2: "Connect your games": RomM (inline `RommSignInForm`), GOG, Epic, Amazon (their
    login windows, `stLoginOpen`) and Steam (the `stSteamCard` form). Each row shows its
    connected state;
  - Continue / Skip for now.
  - The Python-import variant is filled in. A RomM connect moves on to Choose your
    systems.
  - The theme toggle stays in the corner.
- **Library › RomM group** (rail, beside `stRailHTML`):
  - heading with the RomM mark and the `!`/`off` flag (connected, unreachable, sign-in
    expired, not connected with games installed);
  - Discover, then platform rows with their counts. The designer proposes the count
    (recommended: the server's ROM count when connected, the installed count when not;
    installed games marked on covers);
  - Discover mounted in the Library (`discover.js`).
- **The platform view**: the Server platform header from `serverhead.js` (name, counts,
  firmware and emulator chips, Backup all N…, Select), merged covers (Play for installed,
  Install for the rest) and search.
  - Panels for unreachable ("Can't reach RomM · Retry"), sign-in expired ("Sign in
    again…") and not connected ("RomM isn't connected · Connect RomM…", installed games
    still listed).
  - Discover's own panel for those states.
  - Unlinked rows: a "Not linked" tag, and Link needs RomM.
- **Settings › Game stores › RomM card** (beside `stStoreCard` in `renderStoresPane`):
  - server, user, pill (Connected / Can't reach / Sign-in expired / Not connected), last
    error;
  - Edit (sign-in dialog, filled in), Reconnect, Disconnect (confirm: "Installed RomM games
    stay and still play. Cloud saves, firmware downloads and updates stop."), Open in
    browser, Connect… when not connected.
  - The Connection pane is removed from `panes.js`. The pane label follows Q4.
- **"Needs RomM" states**:
  - Cloud saves (`panes.js` cloud) and RetroAchievements (`panes.js` ra): a panel or
    banner that names RomM and links the card. The local RA token stays editable if the
    designer finds that sound;
  - Systems (`panes_systems.js` `renderSystems`): per Q1;
  - Details: Saves tab, Achievements, Link, Update.
- **Removals**: the Server pill and the view in the top bar (applayout's top bar), the chip
  and its menu, the Server view's offline panel (GameBackups `bk-srv-off`), and the Connect
  screen.
- **Open GRID on** (`panes.js` Appearance): "Automatic: Discover when RomM is connected,
  else Library", "Discover", "Library", help text "Here and in TV mode".
- Words: every "Server › …" and "needs a server" string becomes RomM wording (empty drive
  text, backup refusals, the setup card "opens on Discover").

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
  browser, storefront rows, the on-screen keyboard for the RomM form, View to paste);
- three tabs (Library, Downloads, Settings; LB/RB) with **Library as the default tab**;
- the RomM group in the TV Library rail (`libRailHTML`) with Discover's shelves and
  platform pages moved from `srvPaint`;
- the RomM card in TV Settings › Game stores (`tvstores.js` stores pane);
- needs-RomM states on the TV Cloud saves, RetroAchievements and Systems panes;
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
- Main Sections (Server section folded into Library);
- Settings (Connection removed, Game stores card), Appearance;
- First Run Setup (the welcome; the 401 paragraph);
- TV mode Screens and Startup;
- Game backups ("Backing up needs a server").

ARCHITECTURE gets `commands/romm.rs`, `lib/romm/`, `stores/romm.svelte.ts`, and the
removed `Server.svelte`, `Connect.svelte` and `ConnectionPage.svelte`. Done: no SPEC
sentence still describes the chip, offline mode, Connect, or the Server view.

### Step 1 — foundation (one implementer, sequential; blocks Steps 2–6)
Files:
- `crates/grid-core/src/session.rs`, `crates/grid-core/src/config.rs`,
  `crates/grid-core/src/import_python.rs`, `crates/grid-core/tests/session.rs`;
- `app/src-tauri/src/commands.rs` (session commands become wrappers that call into
  `commands/romm.rs`), new `app/src-tauri/src/commands/romm.rs`,
  `app/src-tauri/src/commands/setup.rs` (welcome commands), `app/src-tauri/src/lib.rs`
  (register, change hook → `romm-changed`), `app/src-tauri/src/relink_service.rs` (only if
  the connected-jobs hook needs it);
- `app/src/lib/api.ts`, `app/src/lib/shell.ts`, `app/src/lib/stores/session.svelte.ts`,
  new `app/src/lib/stores/romm.svelte.ts`, new `app/src/lib/romm/*`;
- `app/src/lib/connectForm.ts` (moves to `romm/signIn.ts`; leave a re-export until Step 5).

Done:
- contracts 0.2–0.8 exist;
- old UI still builds and runs through the wrappers;
- `romm-changed` fires on connect, disconnect, reconnect, a 401 and restore.

Tests:
- Rust unit (config): migration matrix — connected / offline / imported / fresh / partly
  filled; idempotent; `offline` dropped on save; round-trip of an already-migrated config.
- Rust unit (session): state changes for each outcome; the 401 sets `Expired` and keeps the
  keyring; disconnect keeps `server_url`.
- `crates/grid-core/tests/session.rs`: replace the offline tests (lines ~730–830) with
  `connected = false` tests (no probe, no keyring read).
- vitest: `romm/platformKey`, `romm/rail`, `romm/platformGames` (installed+server merge,
  unlinked, other server, disconnected), `romm/words`, `shell.initialView` (new matrix),
  `stores/romm` reducer.

### Step 2 — Game backups without RomM (decision 8) — parallel after Step 1
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
  - Firmware and emulator builds without RomM: per Q7.
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
  (archive staged, art warnings).
- Integration (`crates/grid-core/tests/`): backup with no client, then install from that
  pack with no client. Round trip, same `PackSource`.
- vitest: `details/actions`, `tv/backupActions` reasons.
- e2e `backups-e`: not connected, Library row → Backup to drive → written → install back on
  a fresh data dir.

### Step 3 — Desktop Library: RomM group, Discover and platforms; Server view removed — parallel after Step 1
Files:
- `app/src/lib/Library.svelte`, `app/src/lib/Shell.svelte`, `app/src/lib/library/rail.ts`
  (+ test), `app/src/lib/library/selection.svelte.ts`, `app/src/lib/views/fold.ts`;
- `app/src/lib/discover/*`, `app/src/lib/server/*` (the `Catalog` and header modules stay;
  move them if useful);
- new `app/src/lib/romm/desktop/*.svelte` (RomM rail group, platform header, platform
  view, state panels);
- delete `app/src/lib/Server.svelte`;
- `app/src/lib/settings/openOn.ts` (+ test), `app/src/lib/settings/AppearancePage.svelte`
  (Open GRID on words; drop the Server card size);
- `app/src/lib/backups/drives.ts` (the `card_size_server` reader);
- `app/src/lib/gamestores/devMock.ts`;
- e2e: new `e2e/specs/romm-library.spec.ts` (+ group).

Done:
- top bar shows three views, no chip or menu, `Ctrl+1..3`;
- Library rail: All/Recent/Updates, the RomM group per `in_rail`, Backup drives, stores
  (the order follows Q3);
- Discover and platform views work as Server's did (install, play, select, Backup all N…,
  firmware and emulator chips, folding header, search);
- Library's `backupAction(session.connected)` call drops its argument;
- the state panels match D1;
- every D1 Library inventory item is screenshot-checked.

Tests:
- vitest: `library/rail` (no platform rows at the top, `romm:discover` key),
  `settings/openOn`, `views/fold` placeholders;
- e2e `romm-library`: connected → group, Discover, platform merge (installed + server);
  disconnect → group stays with installed only and the `off` flag; never connected and no
  RomM rows → no group.

### Step 4 — Desktop Settings: RomM card, Connection removed, needs-RomM states — parallel after Step 1
Files:
- `app/src/lib/Settings.svelte`, `app/src/lib/settings/pages.ts` (+ test; drop
  `connection`, label per Q4), `app/src/lib/settings/StoresPage.svelte`;
- new `app/src/lib/settings/RommCard.svelte`, `app/src/lib/settings/rommCard.ts` (+ test);
- delete `ConnectionPage.svelte`, `connection.ts` and its test;
- `CloudSavesPage.svelte`, `RetroAchievementsPage.svelte`,
  `app/src/lib/systems/SystemsPage.svelte`, `app/src/lib/systems/systemsStore.svelte.ts`
  (Q1);
- needs-RomM lines in `app/src/lib/details/*`. **Not** `Details.svelte` if Step 3 touches it.
  Today it does not need to; if it must, run Step 4 after Step 3;
- e2e: new `e2e/specs/settings-romm.spec.ts` (+ group).

Done:
- the card matches D1 in every connection state;
- Edit and Reconnect use `RommSignInForm`;
- Disconnect confirms, stays in the app, and the Library group stays while installed RomM
  games remain;
- the three pages show their needs-RomM states;
- no link to `connection` remains.

Tests:
- vitest: `rommCard` (pill, lines, actions per state), `settings/pages`, Systems rows
  without RomM (Q1);
- e2e `settings-romm`: edit server, disconnect/reconnect, 401 → "Sign-in expired" →
  Sign in again.

### Step 5 — Desktop welcome — parallel after Step 1
Files:
- `app/src/App.svelte`;
- new `app/src/lib/Welcome.svelte` and `app/src/lib/welcome.ts` (+ test);
- delete `Connect.svelte`, `connectPrefill.ts` and their tests;
- `connectFlow.ts` (only the library-folder helpers survive; rename to `welcomeFlow.ts`);
- `app/src/lib/setup/*` (ChooseSystems' "Disconnect" footer goes; its words);
- `app/src/lib/stores/pythonImport.svelte.ts` (the welcome shows the notice);
- `app/src/lib/settings/SteamCard.svelte` is **read-only** here: import it; if it needs
  changes, the change belongs to Step 4;
- e2e: rewrite `e2e/specs/connect.spec.ts`, `first-run-setup.spec.ts` and
  `python-import.spec.ts` (their groups test exactly this screen).

Done:
- a fresh start shows the welcome;
- folder → storefronts → Continue/Skip → shell;
- a RomM connect leads to Choose your systems when setup is needed;
- an imported config is filled in;
- connected and offline users never see it.

Tests:
- vitest: `welcome` (step rules, Continue enabled, what Skip writes);
- e2e as listed.

### Step 6 — TV mode — parallel after Step 1 (all files under `app/src/lib/tv/`)
Files:
- `TvShell.svelte`, `TvStage.svelte`, `nav.svelte.ts` (default tab `library`; drop
  `server`), `input/press.ts` (`TvTab`), `topbar.ts`, `TvTopBar.svelte`, `hints.ts`,
  `TvHintBar.svelte`, `overlays/menu.ts` and `TvMenu.svelte` (Reconnect goes), delete
  `reconnect.ts`;
- `tabs/library/*` (RomM group, Discover and platform screens moved in from
  `tabs/server/*`, which is then deleted);
- `startup/*` (`TvConnect` → `TvWelcome`; `connectPage.ts` → `welcomePage.ts`);
- `tabs/settings/panes/connection.ts` (delete), `tabs/settings/TvSettings.svelte`,
  `panes/cloud.ts`, `panes/ra.ts`, `panes/systems.ts`;
- `stores/TvStoresPane.svelte` (+ RomM card), `stores/libraryStores.ts`;
- `pages/TvSearchAll.svelte`, `pages/TvDetails.svelte`, `ui/TvGameCover.svelte`, as needed
  for the needs-RomM lines;
- e2e: rewrite `tv-server.spec.ts` → `tv-romm.spec.ts`, and update `tv-startup-*` and
  `tv-shell-*` (their groups).

Done:
- every D2 inventory item is screenshot-checked at 1080p;
- the TV checklist items D2 marked are re-pointed;
- no flow tells the user to finish in desktop mode.

Tests:
- vitest: `nav` default tab, `topbar`, `hints`, `menu`, the TV library rail with the RomM
  group, the welcome page rows, the TV RomM card;
- e2e: `tv-romm`, `tv-startup`, `tv-shell`.

**Parallel file check (Steps 2–6).**
- Disjoint: 2 (backend backups + backups TS), 3 (Library/Shell/discover/server/openOn),
  4 (settings/systems), 5 (App/Welcome/setup), 6 (tv/**).
- Shared files, each owned by one step:
  - `api.ts`, `session.svelte.ts`, `shell.ts` and `commands.rs` belong to Step 1; Steps 2–6
    do not edit them;
  - `Details.svelte` is owned by Step 3 if needed;
  - `scripts/e2e.sh`: each step adds only its own group line. A conflict there is
    mechanical.
- Merge order: 2, 3, 4, 5, 6, running `npm run check` and the gate after each merge (the
  `SettingsPage` union losing `connection` is the one type-level link between 3 and 4).

### Step 7 — e2e migration (one implementer, after Steps 2–6 merge)
Files:
- new `e2e/helpers/romm.ts`:
  - `connectRomm()`: through the welcome on a fresh data dir, else through
    Settings › Game stores;
  - `openRommDiscover()`;
  - `openRommPlatform(slug)`: by `library-rail-platform-<slug>`;
- `e2e/helpers/server.ts` (delete or turn into a re-export), `e2e/helpers/openOn.ts`,
  `e2e/helpers/tv.ts`, `gog.ts`, `epic.ts`, `amazon.ts`, `libraryMove.ts`, `backups.ts`,
  `env.ts`;
- every spec listed under "e2e impact" below; `scripts/e2e.sh` (group comments: Connect →
  welcome; python-import ordering note).

Done: every group passes on its own.

### Step 8 — legacy removal, full gate, full suite
Files:
- `app/src-tauri/src/commands.rs`, `lib.rs`, `crates/grid-core/src/session.rs` (delete
  `connect`/`restore_session`/`retry_connect`/`disconnect`/`use_without_server` wrappers,
  `RestoreOutcome`, `use_offline`, `SESSION_UNAUTHORIZED_EVENT`);
- `api.ts`, `shell.ts` (`applyRestore`, `applyRetry`, `offlineSession`, `chipLabel`),
  `session.svelte.ts` (`offline`, `lastError`, `rejected`, `connectToServer`);
- re-exports left by Steps 1, 5 and 7.

Then:
- the gate;
- `scripts/ctr.sh wincheck` (only if a step added `cfg(windows)` code);
- the **full e2e suite** `scripts/e2e.sh` (all default groups);
- an api-tester pass on connect, restore, disconnect, platforms and games against the
  live RomM as `tester` (affected endpoints, per the `api-verification` skill);
- a native `.rpm` build for the user's own test.

### Migration and compatibility
- **Connected users:** the migration sets `stores.romm.connected = true` and
  `setup.welcomed = true`. Startup restores as today, with no new sign-in, and they land in
  the Library on RomM › Discover (Automatic). A stored 401 shows "Sign-in expired" on the
  card and the group instead of the Connect form.
- **Offline users:** `connected = false`, `welcomed = true`. They go straight to the
  shell with no probe and no keyring read. The RomM card shows Not connected with the
  stored server and username filled in. The RomM group shows while they have installed
  RomM games. Their keyring credential: Q5.
- **Python import:** unchanged import. It lands on the welcome, filled in (Q6). The import
  notice shows over the welcome, as it does over Connect today.
- **Drive packs:**
  - `gridpack.json` and `PackSource` do not change.
  - Install from a drive already works without a server, and rows keep `source_server`.
  - Packs written without RomM (Step 2) have the same identity as packs written with it.
  - Drive rows stay in Library › Backup drives.
- **Unlinked rows** (`rom_id` None) and rows from another server:
  - they appear in their platform row of the RomM group (key rule 0.3) and in All games;
  - Link to server game, cloud saves and updates show needs-RomM when RomM is off;
  - connect-time relink (`relink_service::spawn_connected_jobs`) is unchanged.
- **Downgrade:** an older build reading a migrated config sees `offline` absent and
  `server_url` set. It probes if a credential is stored, else shows Connect. That is
  acceptable and nothing is lost.
- **Image cache:** `SessionManager::server_url` is set from config whenever one is stored,
  so cached RomM art keeps showing while RomM is off (today only offline mode set it
  without a probe).

### Risks
- **e2e churn.** 42 specs type into the Connect form (`connect-server-url`, 95 uses) and
  about 40 open `platform-btn-<id>` in the Server rail. Step 7 does this through one helper
  and runs alone. Until it lands, those old groups fail; only the new per-step specs are run.
- **Large platforms.** The merged platform grid must stay as fast as Server's
  (`large-platform`, `large-platform-perf`). `platformGames.ts` must be O(n) with a
  `Map<rom_id,row>`, and the grid keeps `$state.raw`.
- **Proton without RomM.** Decision 3 makes Systems RomM-only. But the Windows (Proton)
  default, which GOG, Epic and Amazon Windows games need, lives in Systems (Q1).
- **401 mid-session no longer leaves the shell.** Every RomM call site must already handle
  `client()` returning None mid-flow (they gate on it). Watch the Details pages and the
  first-run setup card for stale "connected" assumptions.
- **Keyring prompts.** `romm_restore` must keep the rule "not connected → no keyring read".
  Otherwise offline users get a secret-service unlock prompt at every start.
- **Rail key collisions.** A server slug can equal another platform's name token (the
  third fallback in 0.3). `platformKey.ts` tests must cover it. A server slug wins.
- **TV checklist drift.** `docs/tv-mode/checklist.md` items about the Server tab, Connect
  and the offline hint must be re-pointed in D2, not silently left.

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
- **Chip, offline and "Use without a server"** (`session-chip`, `session-retry`,
  `session-disconnect`, `connect-offline`, `tv-connection`): connect-restore-c, open-on-a/b/c/d,
  images-a/b, backups-c/d, tv-backups-c, tv-details-c, tv-server, tv-startup-a/c,
  tv-shell-a, tv-gog-b, tv-settings, gog-offline, epic-library; helpers `openOn.ts`, `env.ts`.
- **Connection page:** settings-screens, updates, connect.
- **New specs:** romm-library (Step 3), settings-romm (4), backups-e (2), tv-romm (6).
  Rewritten: connect → welcome, first-run-setup, python-import (5), tv-startup-*, tv-shell-*
  (6), open-on-a..d (7).
- **Final:** Step 8 runs the full default suite (`scripts/e2e.sh`, all groups except
  `OPT_IN_GROUPS`) and the opt-in `large-platform-perf` once for the merged grid.

### Open questions for the user (not decided; ask in chat)
- **Q1. Systems without RomM.** (a) The whole pane is a needs-RomM panel. (b) The per-system
  rows are replaced by a needs-RomM panel, but the **Windows (Proton)** row and **Other
  emulators** stay, because store games need Proton. *Recommended: (b).*
- **Q2. Choose your systems after connecting RomM later from Settings.** (a) Show the
  full-screen picker by today's rule (setup never finished, no emulator), and Skip returns
  to Settings. (b) Only from the welcome; later connects use Settings › Systems › Install
  recommended. *Recommended: (a).*
- **Q3. Where the RomM group and card sit.** (a) First: the group right under Updates (where
  the platform rows are today), and the first card on Game stores. (b) Alphabetical among
  the stores (Amazon, Epic, GOG, RomM, Steam). *Recommended: (a).*
- **Q4. The Settings pane label.** (a) Rename "Stores" to "Game stores" (decision 4's wording
  and SPEC's heading). (b) Keep "Stores". *Recommended: (a).*
- **Q5. Offline users' stored RomM credential.** (a) Keep it, unused, until a new connect
  replaces it; the card shows Not connected, filled in. (b) Delete it during migration.
  (c) The card offers "Reconnect with saved sign-in" when one is stored. *Recommended: (a).*
- **Q6. Python-import users.** (a) The welcome, with the library folder and the RomM server
  and username filled in. (b) Straight to the shell, with the RomM card asking to sign in.
  *Recommended: (a).*
- **Q7. Backups without RomM: emulators and firmware.** (a) Games from the installed copy;
  emulator builds from GRID's catalog still copied (they don't come from RomM);
  server-hosted builds and firmware skipped with "needs RomM". (b) Games only.
  *Recommended: (a).*
- **Q8. Discover when RomM is off but the group shows.** (a) Keep the Discover row with a
  "RomM isn't connected · Connect RomM…" panel, like a store's Available row. (b) Hide
  Discover until RomM connects. *Recommended: (a).*
