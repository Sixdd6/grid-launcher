# Game stores — Phase 4 (Steam) work plan

Status: planned 2026-11-14. Not started. Read it with:
- `docs/external-stores-plan.md` for the user decisions;
- `docs/stores-phase3-plan.md` and `docs/stores-phase2-plan.md`, the templates. Their spike
  results, contract deviations and integration outcomes are the lessons this plan builds on;
- the approved design cards `docs/design/system/components/GameStores/README.md` and
  `TVStores/README.md`, and their sources `docs/design/prototype/{stores,tvstores,stores_bk,tvstores_bk}.js`.
  The design is the spec.

What Phase 3 left on main (bc0d85a):
- `StoreBackend` (`Send + Sync + 'static`), `StoreShared`, `JobCtx`, `StoreLaunchRouter`,
  `prepare_login`, and the three backends `stores_service/{gog,epic,amazon}.rs`;
- `marks.ts` `storeCaps` with `hasTool`, `hasCloud`, `canBackup`, `hasInstaller`,
  `hasVersions`, `hasLanguages`, `hasDlc` and `readableVersions`. Steam's row is already
  all `false`, and every "tool ⇒ cloud/DLC" path already reads a capability;
- `noBackupReason('steam')` ("Steam games can’t be backed up here. Steam has its own Backup
  and restore."), `storesLede()`'s Steam sentence, `steamStartNotice`, "Launches through
  Steam" (desktop `details.ts`, TV `storeWords.ts`) and the hidden Folder fact;
- the e2e pattern: `mock-amazon.mjs`, `fake-tools/nile.mjs`, `seed/amazon-seed.mjs` and
  `amazon-secrets.spec.ts`.

Already Steam-aware on main (Phase 0):
- `StoreKind::Steam`, "Steam" and the `steam:` key; `launches_natively()` is `false`;
- `InstalledGame::is_native()` is `false` for a Steam row, so
  `LaunchService::launch_key` refuses it with `STEAM_LAUNCH_UNSUPPORTED`;
- `LaunchService::register_watched(key, title, platform, emulator_name, pid, alive)` and
  `sessions::AliveCheck` (`FnMut() -> bool`, called on every reap under the session
  lock: it must be cheap and must not block). It has one unit test and no caller;
- `backup::local::REASON_STORE` refuses Steam rows;
- `launch::compat::steam_roots(home)` (native, `~/.local/share/Steam`, Flatpak) for
  Proton discovery;
- `check_secret_hygiene.sh` already allows the fixture marker `FAKE-STEAM-KEY-not-real`.

What is still missing: any Steam backend (`StoresService` has no `steam`); `'steam'` in
`ENABLED_STORES`; the key's keyring slot; Steam's Settings card; and every Steam path in
the Library, Details, Downloads and TV.

**In scope:**
- Settings › Stores › Steam: the Steam client check, the account picker, the Web API key
  (keyring only), Test, Connect/Save, Remove key, Disconnect;
- the owned list from `IPlayerService/GetOwnedGames`;
- installed games from Steam's own files, including games installed outside GRID, games
  only;
- Install in Steam, Uninstall in Steam…, Verify in Steam and Play, all through `steam://`
  links to the Steam client;
- install progress from Steam's app manifests, and a watched session while the game runs;
- every Steam state in the design (parity inventory below).

**Out of scope:**
- cloud saves (Steam Cloud is Steam's; GRID shows no cloud UI for Steam, and the design
  has no Steam cloud sentence to add);
- drive backups (user decision; Details says why);
- DLC, updates and repair by GRID (Steam updates and validates its own games; GRID shows
  no UPDATE tag for Steam);
- Game Settings (the design gives Steam games none);
- a store tool (no Heroic tool for Steam; nothing is downloaded);
- non-Steam shortcuts, mods, Family Sharing libraries, Steam profile URLs and vanity names
  (the design asks for a SteamID64 only);
- macOS (GRID ships no macOS build). The root finder keeps the macOS path so a later
  universal build needs no new code, but it is not tested.

## Research (2026-11-14)

Sources are listed at the end of each part. No real Steam account or real Web API key was
used. The probes below sent only the fixture key `FAKE-STEAM-KEY-not-real` or an all-zero
32-digit key, or no key.

### The Web API

**Owned games.** `GET https://api.steampowered.com/IPlayerService/GetOwnedGames/v1/`.
- Parameters: `key` (string), `steamid` (uint64), `include_appinfo` (bool: name and
  icon), `include_played_free_games` (bool: free games are left out by default),
  `appids_filter`, `include_free_sub` (bool), `language`, `include_extended_appinfo`
  (bool: adds `capsule_filename`, `sort_as` and capabilities; needs `include_appinfo`),
  `skip_unvetted_apps` (bool, optional).
- GRID sends `include_appinfo=1`, `include_played_free_games=1`, `include_free_sub=0`,
  `skip_unvetted_apps=0`, `format=json`.
- The answer is `{"response": {"game_count": N, "games": [{"appid", "name",
  "playtime_forever", "img_icon_url", "rtime_last_played", …}]}}`.
- **Private game details give `{"response": {}}`** with HTTP 200: no `game_count` key at
  all. A public profile that owns nothing has `game_count: 0`. GRID tells them apart by
  whether `game_count` is present. Lutris uses the same rule, and its words match the
  design: "An empty library almost always means the separate Game details setting is
  private, even when the profile itself is public."
- Valve's own docs say only that the call answers "if their owned games/game details are
  visible to you". The `{}` shape comes from community reports (Valve wiki Feedback page).
  Spike 2 records it with the user's key at the end.

**A rejected key (probed 2026-11-14 from this machine):**

| call | key | status | body |
|---|---|---|---|
| `IPlayerService/GetOwnedGames/v1` | `FAKE-STEAM-KEY-not-real` | **401** Unauthorized | HTML "Access is denied. Retrying will not help. Please verify your `key=` parameter." |
| `IPlayerService/GetOwnedGames/v1` | none | **401** | the same |
| `IPlayerService/GetOwnedGames/v1` | 32 zeros (well formed, not a key) | **401** | the same |
| `IPlayerService/GetOwnedGames/v1` | 32 zeros, `steamid=123` | **401** | the same (the key is checked first) |
| `ISteamUser/GetPlayerSummaries/v2` | `FAKE-STEAM-KEY-not-real` / 32 zeros | **403** Forbidden | the same text, "Forbidden" |

- So **GetOwnedGames answers 401, not the design's "403 Forbidden"**. GRID treats 401 and
  403 alike as "Steam rejected the key". The design's words name the status; see
  Question 1.
- The body is HTML, and it is never shown or logged. GRID reads only the status.
- No `Retry-After` or rate headers were sent.

**Who the account is.** `GET https://api.steampowered.com/ISteamUser/GetPlayerSummaries/v2/?key=…&steamids=<id>`
gives `{"response": {"players": [{"steamid", "personaname", "communityvisibilitystate", …}]}}`.
GRID calls it only when `loginusers.vdf` doesn't name the chosen SteamID64 (a manual
entry), to get the persona name the card shows. An empty `players` list means no such
account; GRID shows that as "That isn’t a SteamID64…".

**Validating cheaply.** Test and Save make **one** `GetOwnedGames` call. Its answer
decides every result in the design: works (N games), rejected (401/403), private (`{}`),
no network (connect error, timeout, or 5xx/429). The SteamID64's form (17 digits, starting
`7656`) is checked locally first, with no request. A manual SteamID64 that is not in
`loginusers.vdf` adds the one `GetPlayerSummaries` call.

**Vanity URLs.** `ISteamUser/ResolveVanityURL` exists, but the design accepts only a
SteamID64 ("That isn’t a SteamID64. It has 17 digits and starts with 7656."), so GRID
never calls it.

**Rate limit and terms (Steam Web API Terms of Use).**
- "You are limited to one hundred thousand (100,000) calls to the Steam Web API per day."
  GRID makes about one owned-games call per startup, Refresh, Test and Save, plus one
  metadata batch per 100 new games. That is nowhere near the limit.
- The terms bind the key's owner. Here that is the user, whose own key lists the user's
  own games, only when the user asks. That matches the terms.
- The terms require: keep the key private (GRID: keyring only, sent only to Steam); don't
  imply endorsement by Valve (the store mark stays a monogram tile, never a logo; no
  "official" wording); give data "as is".
- They also ask apps that keep nonpublic user data to say what they store and where. The
  Settings help line already says the key stays in the OS keyring and goes only to Steam.
  GRID stores nothing nonpublic beyond that (the owned list needs public game details).

**Keeping the key out of URLs that get printed.** The key can only go in the query
string. The plan:
1. `stores/steam/api.rs` is the only file that reads the key (`expose_secret`). It builds
   the request with `RequestBuilder::query(&[("key", …)])` at send time. No `Url` with the
   key is ever kept, returned or formatted.
2. Every `reqwest::Error` goes through `.without_url()` before it is turned into GRID's
   words (the RetroAchievements precedent, `retroachievements.rs`). A non-2xx status
   becomes a `SteamApiError` variant with **no** URL and no body.
3. The Steam HTTP client is built with `redirect::Policy::none()`: reqwest logs redirect
   targets at debug, and a 3xx from Steam is answered as "Couldn't reach
   api.steampowered.com". GRID's "debug prints" switch sets the global filter to `debug`,
   so this matters.
4. Spike 4 runs a request (with a 302 and an error) against a local server under a
   `TRACE` subscriber that captures every event, and greps for the key. If reqwest,
   hyper or hyper-util print the full URI at any level, the app's filter gains
   `reqwest=info,hyper=info,hyper_util=info` directives in `logging.rs` (the only way to
   be sure at runtime). The e2e secrets spec runs with debug prints on.
5. `SteamApiError`'s `Display` and `Debug` never carry a URL. A unit test formats every
   variant built from a request carrying `FAKE-STEAM-KEY-not-real` and greps.

**Metadata and art without a key.** `GET https://api.steampowered.com/IStoreBrowseService/GetItems/v1/?input_json=…`
works with no key (probed): `{"ids":[{"appid":N},…], "context":{"language":"english","country_code":"US"},
"data_request":{"include_basic_info":true,"include_release":true,"include_screenshots":true,"include_assets":true}}`.
- Per item it gives:
  - `success` (1 = found; **15 for apps with no store page**: Proton 9.0 2805730, Steam
    Linux Runtime 3.0 1628350, Steamworks Common Redistributables 228980 and Steam Linux
    Runtime 1070560 all answered 15);
  - `type` (0 game; Wallpaper Engine 431960 answered 6, software);
  - `name`, `basic_info.short_description`, `basic_info.developers[].name`,
    `release.steam_release_date` (unix seconds) and `screenshots.all_ages_screenshots[].filename`;
  - `assets.asset_url_format` (`steam/apps/<id>/${FILENAME}?t=…`) and
    `assets.library_capsule` (`library_600x900.jpg`, or `<sha1>/library_600x900.jpg` for
    newer games) and `assets.header`.
- Genres come only as tag ids (`tagids`), with no names. GRID leaves genres empty, so
  Overview leaves out the Genres fact (see "Choices taken" 6).
- The service is undocumented in Valve's partner docs, though Steam's own store uses it.
  A failure there is not fatal: the cover falls back to the plain path below, and the
  other fields stay empty.

**Art hosts (probed, no credential).** `library_600x900.jpg` for app 620 answered 200
`image/jpeg` (52764 bytes) on each of:
- `https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/620/library_600x900.jpg`
- `https://shared.fastly.steamstatic.com/store_item_assets/…`
- `https://shared.steamstatic.com/store_item_assets/…`
- `https://cdn.cloudflare.steamstatic.com/steam/apps/620/library_600x900.jpg`
- `https://steamcdn-a.akamaihd.net/steam/apps/620/library_600x900.jpg`

`https://cdn.akamai.steamstatic.com/steam/apps/620/header.jpg` answered 200 too. Newer apps
keep their art under a hash folder, which only `GetItems` names.

- **Cover rule:** `https://shared.akamai.steamstatic.com/store_item_assets/` +
  `asset_url_format` with `${FILENAME}` = `library_capsule`. Without `GetItems`, the
  plain `…/steam/apps/<id>/library_600x900.jpg`. When that 404s, the art cache's existing
  miss path gives the tile with the title, as for any missing cover.
- **Allowlist:** the domain `steamstatic.com` with its subdomains, plus the **exact host**
  `steamcdn-a.akamaihd.net`. `akamaihd.net` must never be a whole domain, since any Akamai
  customer serves from it. `is_store_art_url` gains an exact-host list beside
  `STORE_ART_DOMAINS`.

Sources:
- `https://partner.steamgames.com/doc/webapi/IPlayerService` (GetOwnedGames parameters,
  visibility);
- `https://steamapi.xpaw.me/IPlayerService#GetOwnedGames` (every parameter, incl.
  `include_extended_appinfo`, `skip_unvetted_apps`);
- `https://developer.valvesoftware.com/wiki/Steam_Web_API/Feedback` (the empty
  `response` for private profiles);
- `https://steamcommunity.com/dev/apiterms` (the limit and the terms);
- Lutris `lutris/services/steam.py` at `f9ef1bc4b5b6a3bb52e26ef973dc5b7d66965634`
  (the private-details message, `excluded_appids`);
- `https://cdn.jsdelivr.net/npm/steam-user@5.3.0/protobufs/generated/steammessages_player.steamclient.json`
  (`include_extended_appinfo`, `capsule_filename`);
- the probes above (curl, 2026-11-14).

### Steam's local files

**Where Steam is.**
- **Linux**, in this order (steamlocate 2.1.1's list, `src/locate/linux.rs`):
  - Flatpak: `~/.var/app/com.valvesoftware.Steam/.local/share/Steam`, `…/.steam/steam`,
    `…/.steam/root`;
  - `$XDG_DATA_HOME/Steam` (default `~/.local/share/Steam`);
  - `~/.steam/steam`, `~/.steam/root`, `~/.steam/debian-installation`;
  - Snap: `$SNAP_USER_DATA` or `~/snap` + `steam/common/.local/share/Steam`,
    `…/.steam/steam`, `…/.steam/root`.

  A root counts when it has a `steamapps/` folder. Roots that point at the same folder
  through a symlink count once.
- **Windows:** `HKCU\Software\Valve\Steam\SteamPath` (Playnite's choice, `Steam.cs`), then
  steamlocate's `HKLM\SOFTWARE\Wow6432Node\Valve\Steam\InstallPath` (or the 64-bit
  key). The executable is `HKCU\…\SteamExe`, else `<SteamPath>\steam.exe`.
- **macOS:** `~/Library/Application Support/Steam` (kept, not tested).
- GRID's existing `launch::compat::steam_roots` (three Linux roots, for Proton) stays as
  it is. Package A's `steam::paths::steam_roots()` is the full list, and compat may move
  to it later.

**Library folders.** `<root>/steamapps/libraryfolders.vdf` is text KeyValues:
`"libraryfolders" { "0" { "path" "…" "label" "" "contentid" … "totalsize" … "apps" {
"<appid>" "<bytes>" … } } "1" { … } }`. Each `path` has its own `steamapps/`. GRID reads
every library, then every `steamapps/appmanifest_<appid>.acf` in each. It does not trust
the `apps` map alone, because Steam updates that map late.

**App manifests** (`appmanifest_<appid>.acf`, text KeyValues, root key `AppState`). Fields
GRID reads: `appid`, `name`, `StateFlags`, `installdir` (under `steamapps/common/`),
`SizeOnDisk`, `buildid`, `LastUpdated` (unix seconds), `LastPlayed`, `BytesToDownload`,
`BytesDownloaded`, `BytesToStage`, `BytesStaged`.

**`StateFlags` is a bit set** (steamlocate's `StateFlag`; game-scanner's `SteamAppState`):

| bit | value | name |
|---|---|---|
| 0 | 1 | Uninstalled |
| 1 | 2 | UpdateRequired |
| 2 | 4 | FullyInstalled |
| 3 | 8 | Encrypted |
| 4 | 16 | Locked |
| 5 | 32 | FilesMissing |
| 6 | 64 | AppRunning |
| 7 | 128 | FilesCorrupt |
| 8 | 256 | UpdateRunning |
| 9 | 512 | UpdatePaused |
| 10 | 1024 | UpdateStarted |
| 11 | 2048 | Uninstalling |
| 12 | 4096 | BackupRunning |
| 16 | 65536 | Reconfiguring |
| 17 | 131072 | Validating |
| 18 | 262144 | AddingFiles |
| 19 | 524288 | Preallocating |
| 20 | 1048576 | Downloading |
| 21 | 2097152 | Staging |
| 22 | 4194304 | Committing |
| 23 | 8388608 | UpdateStopping |

GRID's reading:
- **installed** = bit 4 set and bit 2048 clear. A game with an update queued (4|2 = 6) or
  running (e.g. 4|2|256|1024) is still installed and still plays through Steam. Playnite
  uses the same FullyInstalled test (`SteamLocalService.cs`).
- **installing** = bit 4 clear and any of 2, 256, 1024, 1048576, 524288, 2097152, 4194304,
  or 512 (paused). A first install that is paused stays "Installing in Steam" at its
  percentage, as Steam shows it.
- **progress** = `BytesDownloaded / BytesToDownload`, falling back to
  `BytesStaged / BytesToStage`; 0 when both totals are 0.
- **gone** = the file is missing, or bit 1 / 2048 is set.

**Accounts on this computer.** `<root>/config/loginusers.vdf`: `"users" { "<SteamID64>" {
"AccountName" "PersonaName" "RememberPassword" "MostRecent" "Timestamp" … } }`. Lutris's
`get_active_steamid64` reads the same file.
- The picker lists each SteamID64 with its **PersonaName** ("SamPlays").
- "signed in to Steam on this computer" is the user with `MostRecent "1"`; every other
  user "has signed in on this computer before".
- **`AccountName` is the Steam login name, half of a credential.** GRID never reads it
  into any struct, log or payload. The parser keeps only the keys it needs.

**Last played.** `GetOwnedGames` gives `rtime_last_played`, and the manifest's `LastPlayed`
is local. GRID stamps `last_played_at` with the newest of those and its own session stamp
(`Registry::touch_last_played_key` only ever moves it forward). So "Deep Field Radio,
1 week ago", a game played in Steam itself, shows right without GRID having launched it.

**A text VDF/ACF parser.**
- **steamlocate 2.1.1** (MIT; deps `keyvalues-parser` 0.2, `keyvalues-serde` 0.2, `crc`,
  serde, and `winreg` 0.56 on Windows only, which is already in GRID's `Cargo.lock`).
  - `SteamDir::from_dir(path)` takes a root GRID found itself, which is how the e2e fake
    root plugs in.
  - `libraries()` / `Library::apps()` parse `libraryfolders.vdf` and every manifest into
    `App { app_id, name, install_dir, state_flags, size_on_disk, build_id, last_updated,
    bytes_to_download, bytes_downloaded, bytes_to_stage, bytes_staged, … }`.
  - Its Windows locate reads HKLM only, so GRID reads HKCU `SteamPath` first and uses
    steamlocate's locate after.
- `keyvalues-parser` 0.2 directly, for the files steamlocate doesn't model:
  `loginusers.vdf`, `registry.vdf` and `localconfig.vdf`.
- Spike 1 decides between steamlocate and a small hand parser: it parses real-format
  samples, a manifest Steam is still writing (truncated), and odd escapes in a `path`. If
  steamlocate fails a whole library on one bad manifest, GRID wraps it per file.

**Telling games from tools.**
- `appinfo.vdf` (`<root>/appcache/appinfo.vdf`) has each app's `common/type`
  (`Game`, `Tool`, `Application`, `Config`, `Demo`, `DLC`, …). But it is binary, its format
  changed in 2024 (v29, with a string table), and only third-party parsers read it. **Not
  used.**
- What others do:
  - Lutris drops a fixed list (`221410` Steam for Linux, `228980` Steamworks Common
    Redistributables, `1070560` Steam Linux Runtime). Installed games appear only when they
    are in the owned list.
  - Playnite drops `228980` and requires FullyInstalled.
  - Heroic has no Steam library manager; it only adds its own games to Steam as shortcuts.
- **GRID's rule** (`steam::filter`, pure, with tests):
  1. **A fixed denylist** of tool app ids, kept in code with a comment per id:
     - 228980 Steamworks Common Redistributables;
     - 1070560, 1391110, 1628350 and 4183110 (Steam Linux Runtime 1.0 scout, 2.0 soldier,
       3.0 sniper, 4.0);
     - 1493710 Proton Experimental, 2180100 Proton Hotfix, 1826330 Proton EasyAntiCheat
       Runtime, 1161040 Proton BattlEye Runtime;
     - every numbered Proton release Valve ships (858280, 930400, 961940, 996510, 1054830,
       1113280, 1245040, 1420170, 1580130, 1887720, 2348590, 2805730, plus newer ones
       found in spike 1's research);
     - 221410 Steam for Linux;
     - 250820 SteamVR, 323910 SteamVR Performance Test.
  2. **Name and folder patterns** for new releases: name `Proton …` or `Steam Linux
     Runtime…`, or `installdir` starting `SteamLinuxRuntime` or `Proton `.
  3. **The store type,** when known from the `GetItems` cache: keep `type` 0 (game) and 1
     (demo); drop every other type (software, tools, videos, …). When the type is unknown
     (offline before the first fetch, or `success` 15), rules 1–2 decide alone.
  4. Owned games go through rules 1 and 3 too, so owned software (Wallpaper Engine) stays
     out of Available, as "games only" asks.
- The cache of `GetItems` answers is `stores/cache/steam-apps.json`: type, name,
  description, developers, release, assets and screenshots per app id. It is refreshed
  for unknown ids only, 100 ids per request.

Sources:
- steamlocate 2.1.1 (crates.io source: `src/locate/{linux,windows,macos}.rs`,
  `src/app.rs`, `Cargo.toml`);
- `https://docs.rs/crate/game-scanner/1.0.4/source/src/steam/types.rs` (`SteamAppState`
  bits);
- Playnite `source/Libraries/SteamLibrary/{Steam.cs,SteamGameController.cs,Services/SteamLocalService.cs}`
  at `886468f65724e6fd104f3a09847a17ba57cfb430`;
- Lutris `lutris/services/steam.py` at `f9ef1bc4…`;
- `https://github.com/sonic2kk/steamtinkerlaunch/wiki/Appinfo` (the binary `appinfo.vdf`).

### Driving the Steam client

**`steam://` links** (Valve Developer Community "Steam browser protocol"; the wiki warns
that some entries are old):
- `steam://install/<appid>` opens Steam's install dialog (the user picks the library
  there);
- `steam://uninstall/<appid>` opens Steam's uninstall confirmation. The wiki's text ("deletes
  the specified apps' cache files") is out of date; Playnite uses it to uninstall;
- `steam://validate/<appid>` verifies the game's files;
- `steam://rungameid/<appid>` starts the game ("Same as run, but with support for mods and
  non-Steam shortcuts").

**How GRID opens them.** It runs the Steam executable with the link as an argument, never
a browser or `xdg-open` first:
- `steam -silent steam://rungameid/620`: Playnite's form. `-silent` keeps Steam's main
  window from popping up when Steam has to start;
- install, uninstall and validate omit `-silent`, because they must show Steam's dialog.

Which executable:
- **Linux native:** `steam` on `PATH` (usually `/usr/bin/steam`, the design's example),
  else `~/.steam/steam.sh`.
- **Flatpak:** `flatpak run com.valvesoftware.Steam <args>`, when the Flatpak root is the
  one in use. GRID finds it through the exported launcher
  (`/var/lib/flatpak/exports/bin/com.valvesoftware.Steam` or
  `~/.local/share/flatpak/exports/bin/…`), which it can run directly.
- **Snap:** `/snap/bin/steam`.
- **Windows:** `SteamExe` from HKCU.
- **Fallback:** `xdg-open <link>` (Linux) when no executable is found but a root is. The
  card then shows the root's path. The design's "Steam client found" needs a path, so
  Settings shows the executable or the Flatpak id (Question 3).

Further rules:
- When Steam isn't running, the executable starts it and hands it the link. Steam may first
  ask the user to sign in or update itself. GRID only waits; the watched session and the
  install watcher allow for it (below).
- Spawning: the child is detached with null stdio. Nothing it prints is read or logged
  (Steam's console output names accounts). The launch is a fire-and-forget spawn through
  one function, `steam::client::open(link)`. A second Steam process that only forwards the
  link exits at once.

**Install progress and completion.** After `steam://install/<id>`, GRID polls every 2 s
for `appmanifest_<id>.acf` in every library:
- it appears once the user confirms Steam's dialog;
- GRID reads the progress fields and the state above;
- the Downloads row reads "Installing in Steam N% · Managed by Steam";
- it completes when installed (bit 4, not uninstalling). Then: "Installed by Steam", the
  card "<title> installed · Play", the row upserted, `changed()`.

When no manifest appears within `STEAM_INSTALL_START_TIMEOUT` (15 min; the user closed the
dialog), or the manifest goes away before completion, the row ends Cancelled
("Cancelled"). Steam owns pause and cancel, so GRID's Cancel is disabled (design). The
watcher survives a restart: on startup, any manifest that is installing becomes a
Downloads row again (for a game installed inside Steam too).

**Uninstall and verify.**
- Uninstall: GRID opens the link and shows the design's toast. The background scan
  notices the manifest is gone, removes the row, and raises the uninstalled card (no cloud
  sentence).
- Verify: the link and the toast only. Steam shows its own progress.

**Is the game running (the watched session)?** `AliveCheck` must be cheap and
non-blocking. GRID's check:
- **Linux and macOS:** `<root>/../registry.vdf`, i.e. `~/.steam/registry.vdf` (Flatpak:
  `~/.var/app/com.valvesoftware.Steam/.steam/registry.vdf`). Its
  `Registry/HKCU/Software/Valve/Steam/RunningAppID` equals the app id, or
  `…/Steam/apps/<appid>/Running` is `"1"`. The file is a few KB; it is read only when its
  mtime changed since the last poll.
- **Windows:** `HKCU\Software\Valve\Steam\RunningAppID` (DWORD), or
  `HKCU\Software\Valve\Steam\Apps\<appid>\Running` (DWORD 1).
- **Linux fallback** when `registry.vdf` is missing: one pass over `/proc/*/cmdline` for
  Steam's `reaper SteamLaunch AppId=<appid>` wrapper.
- No public source documents `RunningAppID` (searched 2026-11-14). It is widely used, and
  the Windows value is the same key, but it waits for the user's own check (see "What
  waits for the user's machine"). The fallback covers it until then.

The check is a small state machine kept in the closure:
- **Starting:** alive until the game is first seen running, for at most
  `STEAM_START_GRACE` (180 s: Steam may have to start, sign in or finish an update first);
- **Running:** alive while seen;
- **Ended:** two misses in a row (5 s at the 2.5 s reap) end it.

A game never seen within the grace ends silently. The design has no words for it, so
nothing is shown beyond the session ending (the Playing page closes).

Playnite instead watches processes under the install folder. That fails for games whose
launcher runs elsewhere, and under Proton the process paths are Wine paths, so GRID does
not use it.

Sources:
- `https://developer.valvesoftware.com/wiki/Steam_browser_protocol` (via search results;
  the page itself answered 403 to the fetcher);
- Playnite `SteamGameController.cs` (`steam://install`, `steam://uninstall`,
  `steam.exe -silent "steam://rungameid/…"`, install and uninstall watchers) at
  `886468f6…`;
- `https://revuln.blogspot.com/2018/12/steam-browser-protocol-insecurity-when.html`
  (`run`/`rungameid` as the launch commands);
- `https://docs.rs/crate/game-detector/0.2.1/source/src/steam.rs` (HKCU/HKLM
  `SteamPath`).

### Steam Cloud

Steam syncs its own saves when the game starts and stops. GRID neither syncs nor shows
cloud state for Steam: `hasCloud` stays `false`, so no Cloud saves tab, chip, Details line,
Game Settings switch or Settings row appears. The design has no Steam cloud sentence, so
there is none to confirm or add.

## Step 0 — contracts skeleton (one commit before fan-out)

Steam is the first store with no tool, no login window and no GRID-run job, so Step 0 is
larger than Phase 3's.
- **What it adds:** the types below, the key slot, the queue's external store row, the
  backend shape, the Steam commands, the `api.ts` types, `[stores.steam]` and the stub
  backend.
- **Frozen-file changes:** `queue.rs` (one key and two methods), `art.rs` (an exact-host
  list), `secrets.rs` (the key slot), `commands.rs` (Steam routing in `launch_game_key`)
  and `config.rs`.

**Step 0 passes the gate and every `gog*`, `tv-gog`, `epic*`, `tv-epic`, `amazon*`,
`tv-amazon` and `launch` e2e group before fan-out.** After it, these files are frozen and
change through the lead: `api.ts`, `config.rs`, `secrets.rs`, `library/queue.rs`,
`stores/{mod,art}.rs`, `stores/steam/{mod,meta,row,filter}.rs`, `stores_service/mod.rs`
and `commands/stores.rs`.

### grid-core

```rust
// secrets.rs: a third independent keyring item under SERVICE "grid-launcher"
const STEAM_KEY_ACCOUNT: &str = "steam-web-api-key";
pub trait SteamKeyStore: Send + Sync {
    fn save(&self, key: &SecretString) -> Result<(), SecretError>;
    fn load(&self) -> Result<Option<SecretString>, SecretError>;
    fn clear(&self) -> Result<(), SecretError>;      // NoEntry is Ok
}
impl SteamKeyStore for KeyringStore {}  impl SteamKeyStore for MemoryStore {}  // own field, as RA's
// Test: saving or clearing it never touches the RomM credential or the RA token, and back.

// library/queue.rs: a store row the queue shows but does not drive (Steam installs)
JobKey::StoreExternal { source: StoreKind, store_id: String }   // job "store", kind "store_external",
                                                                // source_id = the game key; takes no slot,
                                                                // never waits, dedupes per product
pub fn admit_store_external(&mut self, source, store_id, title) -> Option<u64>; // None if one is live
pub fn finish_store_external(&mut self, id, outcome: ExternalOutcome /* Completed{detail} | Cancelled | Failed(text) */);
// set_progress works on it as on any row. request_cancel answers Ignored and retryable None
// (Steam owns pause/cancel). InstallService wraps both, as admit_external.

// stores/art.rs
STORE_ART_DOMAINS += "steamstatic.com";
const STORE_ART_HOSTS: [&str; 1] = ["steamcdn-a.akamaihd.net"];   // exact host only, never akamaihd.net

// stores/steam/mod.rs  (types in Step 0, bodies package A)
pub const STEAM_API: &str = "https://api.steampowered.com";
pub const STEAM_ART_BASE: &str = "https://shared.akamai.steamstatic.com/store_item_assets/";
pub struct SteamEndpoints { pub api: Url, pub art_base: Url, pub art_origins: Vec<Url> }  // ::real(), ::e2e(base)
pub struct SteamId(u64);                                       // FromStr: 17 digits, starts 7656
pub enum SteamApiError { Rejected { status: u16 }, Private, NoSuchAccount, Network }  // Display: GRID's words, never a URL
pub struct OwnedGames { pub games: Vec<OwnedGame> }            // only from a response with game_count
pub struct OwnedGame { pub appid: u32, pub name: String, pub last_played: i64 }
pub struct AppInfo { pub appid: u32, pub kind: Option<u8> /* store type */, pub name: String, pub description: String,
                     pub developers: Vec<String>, pub year: String, pub cover: String, pub screenshots: Vec<String> }
pub struct SteamClient { pub root: PathBuf, pub exe: ClientExe /* Path(PathBuf) | Flatpak | XdgOpen */, pub display: String }
pub struct LocalAccount { pub steam_id: SteamId, pub persona: String, pub most_recent: bool }  // never AccountName
pub struct ManifestState { pub appid: u32, pub name: String, pub library: PathBuf, pub install_dir: String,
                           pub flags: u32, pub size_on_disk: u64, pub build_id: u64, pub last_updated: i64,
                           pub last_played: i64, pub done: u64, pub total: u64 }
pub enum AppPhase { Installed, Installing, Gone }               // the StateFlags reading above
pub enum SteamLink { Install(u32), Uninstall(u32), Validate(u32), Run(u32) }  // Display: steam://…; Run adds -silent
pub struct SteamService;   // Clone, Send+Sync
impl SteamService {
  pub fn new(http: reqwest::Client /* redirect none */, ep: SteamEndpoints) -> Self;
  pub fn with_roots_override(self, roots: Vec<PathBuf>, exe: Option<PathBuf>) -> Self; // #[doc(hidden)], the ONE test/e2e override
  pub fn find_client(&self) -> Option<SteamClient>;                          // roots + executable
  pub fn local_accounts(&self, client: &SteamClient) -> Vec<LocalAccount>;   // loginusers.vdf
  pub fn scan(&self, client: &SteamClient) -> Vec<ManifestState>;            // every library, every manifest
  pub fn manifest(&self, client: &SteamClient, appid: u32) -> Option<ManifestState>;
  pub fn open(&self, client: &SteamClient, link: SteamLink) -> io::Result<()>;  // detached, null stdio
  pub fn alive_check(&self, client: &SteamClient, appid: u32) -> AliveCheck;   // the state machine above
  pub async fn owned(&self, key: &SecretString, id: SteamId) -> Result<OwnedGames, SteamApiError>;
  pub async fn persona(&self, key: &SecretString, id: SteamId) -> Result<String, SteamApiError>;
  pub async fn app_info(&self, ids: &[u32]) -> Vec<AppInfo>;                 // GetItems, no key; failures give []
}

// stores/steam/filter.rs  (implemented in Step 0)
pub fn is_game(appid: u32, name: &str, install_dir: &str, kind: Option<u8>) -> bool;  // rules 1–3 above
pub fn phase(flags: u32) -> AppPhase;   pub fn progress(m: &ManifestState) -> (u64, u64);

// stores/steam/meta.rs  (store_meta JSON; never the key)
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct SteamMeta { pub library: String, pub install_dir: String, pub build_id: u64,
                       pub outside: bool /* not installed through GRID's Install in Steam */ }

// stores/steam/row.rs  (implemented in Step 0)
pub fn steam_row(m: &ManifestState, info: Option<&AppInfo>, meta: &SteamMeta) -> InstalledGame;
// source steam, store_id = appid, platform "Steam", slug "steam", title = info name, else the
// manifest's name, else the appid; install_path = <library>/steamapps/common/<installdir>;
// filesize = SizeOnDisk; companies = developers; first_release_date = year. No native launch
// columns (is_native() is false, so LaunchService never builds a command for it).

// config.rs
pub struct StoresConfig { …, #[serde(default, skip_serializing_if = "SteamConfig::is_default")] pub steam: SteamConfig }
pub struct SteamConfig { pub connected: bool, pub steam_id: String /* not secret */, pub persona: String }
// No key field, ever: the hygiene script refuses a `steam_api_key`/`web_api_key` field.
```

### App (`app/src-tauri/src`)

- **The no-tool backend.** `SteamStore` implements `StoreBackend` like the others, with
  these answers:
  - `status`: `tool` = an empty `ToolStatus` (name `""`), so no tool UI shows;
  - `login_spec` / `prepare_login`: `Err("Steam connects in Settings › Stores.")`. The
    frontend never asks: `store_connect` for Steam goes to Settings, as in the prototype;
  - `run_install_tool`: does nothing;
  - `ensure_can_install` / `install_options` / `install_job` / `update_job` /
    `repair_job` / the DLC calls / `uninstall`: `Err("Steam installs, updates and
    uninstalls its games itself.")`. They are never reached from the UI;
  - `game_settings`: empty; `cloud_*`: `none`; `check_updates`: does nothing (Steam
    updates its own games); `launch_hook`: `None`;
  - `is_art_url`: the Steam hosts plus the e2e origin;
  - `disconnect`: real (below).

  A new trait method keeps the router honest: `fn launch_watched(self: Arc<Self>, key:
  GameKey, launch: Arc<LaunchService>) -> Option<BoxFut<'static, Result<GameSession,
  LaunchError>>>`, default `None`. Steam's opens `SteamLink::Run` and calls
  `register_watched` with its `alive_check`.
- **`launch_game_key`** (`commands.rs`). Before `launch.launch_key`, when the key's
  backend answers `launch_watched`, that answer is the session. The `last_played_at` stamp
  after it stays as it is. This is the only change outside the store files, and the
  desktop and TV launch paths both go through it.
- **`StoreStatus` gains `steam: Option<SteamCard>`** (`None` for every other store):
  - `client_path: String` (`""` = Steam not found);
  - `steam_id: String`;
  - `key: 'ok' | 'none' | 'bad' | 'private'`;
  - `key_tail: String` (Question 2; `""` until decided);
  - `accounts: Vec<{persona, steam_id, most_recent}>`.

  `username` is the persona. `connection` is `connected` / `not_connected` (Steam has no
  expired state; a bad key is `key: 'bad'`).
- **New commands** (`commands/stores.rs`, Steam only):
  - `steam_test(steam_id: String, key: Option<String>) -> SteamTestResult`. `key` is
    `None` to test the saved key. The plain `String` is wrapped in a `SecretString` on the
    first line and never read again (the `set_retroachievements_credentials` precedent).
    The result is `ok{owned_count} | bad_key{status} | private | bad_steam_id | network`.
  - `steam_save(steam_id: String, key: Option<String>) -> SteamTestResult`. It runs the
    same test; on `ok` or `private` it saves the key to the keyring (when given), writes
    `[stores.steam]`, scans, refreshes, then emits `changed` and the connected notice.
  - `steam_remove_key()`: clears the keyring slot; the store stays connected with
    `key: 'none'`.
  - `steam_recheck_client()`: finds the client again and scans.
  - `steam_open(key: String, action: 'install' | 'uninstall' | 'validate')`. `install`
    also admits the external Downloads row and starts the watcher.
  - No answer ever carries the key. Disconnect is the existing `store_disconnect`.
- **`StoresService`** gains `steam: Arc<SteamStore>`, registered last (`STORE_ORDER`
  amazon, epic, gog, steam). `SteamStore::real(...)` takes `Arc<dyn SteamKeyStore>` (the
  `KeyringStore` in `lib.rs`; `MemoryStore` in tests).
- **`stores_service/steam.rs`** is the stub:
  - status reads the config and `find_client()`;
  - `in_rail` = connected;
  - every Steam command answers `not implemented`, except `steam_test`, which returns
    `bad_steam_id` for a bad form;
  - e2e (feature `e2e`): `GRID_LAUNCHER_E2E_STEAM_BASE` feeds `SteamEndpoints::e2e`,
    `GRID_LAUNCHER_E2E_STEAM_ROOT` and `GRID_LAUNCHER_E2E_STEAM_EXE` feed
    `with_roots_override`. The e2e build reads no real Steam root.
- `update_service.rs`: no Steam source, and nothing to clear on Disconnect.

### TypeScript (`api.ts`)

```ts
StoreStatus.steam: SteamCard | null;
type SteamCard = { client_path: string; steam_id: string; key: 'ok' | 'none' | 'bad' | 'private';
                   key_tail: string; accounts: { persona: string; steam_id: string; most_recent: boolean }[] };
type SteamTestResult = { kind: 'ok'; owned_count: number } | { kind: 'bad_key'; status: number }
                     | { kind: 'private' } | { kind: 'bad_steam_id' } | { kind: 'network' };
StoreGame.origin: … | 'steam' | 'steam_outside';      // installed through GRID's Install in Steam / found
steamTest(steamId, key?), steamSave(steamId, key?), steamRemoveKey(), steamRecheckClient(),
steamOpen(key, action)
```

`ENABLED_STORES` gains `'steam'` in package D, not in Step 0. `StoreCaps` gains
`managedByClient` (Steam only) in Step 0, all callers `false` until D.

**Hygiene** (`check_secret_hygiene.sh`):
- `expose_secret` is allowed in **one** new file, `stores/steam/api.rs`;
- `STEAM_KEY_ACCOUNT` / `steam-web-api-key` may appear only in `secrets.rs`, as RA's
  slot;
- a field named `steam_api_key`, `web_api_key` or `steam_key` fails anywhere outside
  `secrets.rs`;
- the store-log rule covers `stores/steam/` and `stores_service/steam.rs`;
- a new rule fails any `format!`/log macro in `stores/steam/` that names `url` or `Url`
  together with `key`.

## Work packages (after Step 0)

Three packages, as in Phase 3. There is no tool, so the grid-core package carries the web
client, the local files and the client driver; the app package carries the Settings flow
and the harness.

### A — grid-core Steam store

- **Owns:** `stores/steam/{mod,api,paths,local,client,watch}.rs` (`meta`/`row`/`filter`
  belong to Step 0), `crates/grid-core/tests/steam_store.rs` and its fixtures under
  `crates/grid-core/tests/fixtures/steam/` (real-format VDF/ACF, fake ids, the fake key
  only), `Cargo.toml` (steamlocate, keyvalues-parser; `winreg` for `cfg(windows)`).
- **api.rs:**
  - `owned` (GetOwnedGames), `persona` (GetPlayerSummaries) and `app_info` (GetItems,
    batches of 100, no key);
  - the key goes in only through `query()` at send time; redirects are off; errors go
    through `.without_url()`; status-only errors, as in Research;
  - 401/403 give `Rejected{status}`; `{}` gives `Private`; empty `players` gives
    `NoSuchAccount`; anything else (connect, timeout, 3xx, 429, 5xx, bad JSON) gives
    `Network`;
  - timeouts: 20 s per request.
- **paths.rs / local.rs:**
  - the root list (Research) with symlink dedup; HKCU `SteamPath`/`SteamExe` behind
    `cfg(windows)`;
  - the executable finder (PATH `steam`, Flatpak export, `/snap/bin/steam`, `steam.sh`);
  - `loginusers.vdf` (SteamID64, PersonaName, MostRecent only) and libraries + manifests
    (steamlocate or the spike 1 fallback);
  - one bad manifest skips that app, never the library;
  - a manifest Steam is still writing (a parse error) counts as "unchanged since the last
    scan" for the watcher.
- **client.rs:** `open(link)`: argv per `ClientExe` (`-silent` only for `Run`); detached,
  `Stdio::null()` ×3, never waited on in the caller's thread (a reaper thread waits, so no
  zombie). An exec error gives `io::Error`, which the app words.
- **watch.rs:**
  - `alive_check`: `registry.vdf` with an mtime cache, then the `/proc` fallback; the
    Windows registry behind `cfg(windows)`; the Starting/Running/Ended machine with
    injectable clocks for tests;
  - `InstallWatch`: a pure step function `(previous, Option<ManifestState>, elapsed) ->
    WatchEvent { Progress(done, total) | Installed | Cancelled | Waiting }`, so the app
    drives the polling loop and the tests drive the function.
- **Tests:**
  - parsers on the fixture files (several libraries, a Flatpak-style root, odd escapes, a
    truncated manifest, StateFlags 4/6/1026/1542/2048);
  - the filter table (every denylisted id, the patterns, store types 0/1/6/13, unknown);
  - `SteamId` parsing;
  - a mock HTTP server for every API outcome, asserting the key reaches the server and
    never appears in any `Display`/`Debug` of the result or error;
  - `SteamLink` argv per OS shape;
  - the alive machine (grace, two misses);
  - a `cfg(unix)` fake `steam` script that records its argv.

### C — app services, Settings flow, e2e harness

- **Owns:**
  - app: `stores_service/steam.rs`, the Steam commands in `commands/stores.rs` after Step
    0, `lib.rs` (the keyring slot into `SteamStore::real`, the startup scan);
  - e2e: `e2e/mock-romm/mock-steam.mjs` (+ test), `e2e/fake-tools/steam.mjs` (+ test),
    `e2e/seed/steam-seed.mjs`, `e2e/fixtures-steam/` (a fake Steam root template),
    `e2e/helpers/steamSetup.ts`, the `scripts/e2e.sh` groups, `wdio.conf.ts`,
    `.gitignore` (the mock request log), and `e2e/specs/steam-connect.spec.ts` and
    `steam-secrets.spec.ts`.
- **SteamStore** is built on `AmazonStore`'s shape, without the tool and login parts:
  - a `SteamApi` seam (`RealSteam` forwards to `SteamService`; unit tests fake it);
  - the key comes from the `SteamKeyStore` when needed, kept in memory as a `SecretString`
    after the first load, and dropped on Remove key and on Disconnect;
  - background-only notices (its own `notice` module).
- **The owned list and installed rows.**
  - The owned list: `GetOwnedGames`, filtered (rule 1, then rule 3 when the type is known).
  - **One owner:** `stores/cache/steam-owned.json` (app ids, names, `rtime_last_played`,
    refreshed time), written and deleted only here (Phase 2 lesson), plus
    `steam-apps.json` for `GetItems` answers. Startup reads both; Refresh rewrites them.
  - **Installed rows are registry rows** (source `steam`), reconciled by every scan:
    - an installed game that passes the filter is upserted with `steam_row`;
    - a row whose manifest is gone is deleted, with the uninstalled card when GRID saw it
      installed in this run;
    - `outside` is set unless GRID's watcher saw the install;
    - `last_played_at` is moved forward from `LastPlayed` / `rtime_last_played`.
  - **When the client is missing:** the rows stay (so "Check again" brings back last
    played), but `library().installed` is empty and the status shows the client missing.
  - **The scan runs:** at startup, after Save, on Refresh and Check again, every 15 s in
    the background (only libraries whose `steamapps/` mtime changed), and every 2 s per
    live install watch.
- **Key states** (`SteamCard.key`): `none` (no key in the keyring), `bad` (the last call
  answered `Rejected`), `private`, `ok`. A refresh that answers `Network` keeps the last
  state and the cached list. Each state flips the Available panel and the "Installed games
  only" pill, as in the design.
- **Settings flow:**
  - **Test:** `steam_test`, with "Asking Steam…" while it waits;
  - **Save / Connect Steam:** `steam_save`. Connect needs a key Steam answers for (`ok`
    or `private`), because the user decision requires both. Afterwards Remove key leaves
    the store connected with "Installed games only", as the design shows;
  - a Save that only changes the SteamID64 keeps the saved key (`key: None`);
  - a keyring error gives "Couldn't save your key in the OS keyring: <keyring's text>"
    (the text carries no secret).
- **Install / uninstall / verify** (`steam_open`):
  - **install:**
    - refuses with "Steam isn’t installed. Install Steam, then try again." when the client
      is missing;
    - else opens the link, shows the design's toast, admits the external row, and starts
      the watch loop (a tokio task, 2 s);
    - progress goes to `set_progress`; the end goes to `finish_store_external`
      (`Completed{"Installed by Steam"}` / `Cancelled`), then the "installed · Play" card;
  - **uninstall** and **verify:** open the link, show the toast; the scan does the rest.
- **Launch** (`launch_watched`): not installed is an error; the client missing gives
  "Steam isn’t installed. Install Steam, then try again."; then `open(Run)`, then
  `register_watched(key, title, "Steam", "Steam", 0, alive_check)`. `AlreadyRunning` is as
  for any game. The "Starting <title> through Steam…" toast is the frontend's
  (`steamStartNotice`).
- **Disconnect:**
  - clears the keyring slot (a `NotFound` is fine; any other error stops it with
    "Couldn't delete your Web API key from the OS keyring: …");
  - deletes both cache files and every Steam registry row (only rows, never files: "they
    stay installed in Steam");
  - clears `[stores.steam]` and the in-memory key, stops the watches (live external rows
    end Cancelled), and emits `changed`.

  The rail group goes (design decision 5's Steam exception).
- **E2E harness:**
  - **`mock-steam.mjs`** (one base): `/IPlayerService/GetOwnedGames/v1/`,
    `/ISteamUser/GetPlayerSummaries/v2/`, `/IStoreBrowseService/GetItems/v1/` and
    `/art/<appid>.jpg`.
    - Key rule: `FAKE-STEAM-KEY-not-real` is good; `FAKE-STEAM-KEY-not-real-revoked`
      gives 401 with Steam's HTML; any other key gives 401.
    - SteamID rules: `76561198000000001` lists the fixture games; `…0002` gives `{}`
      (private); `…0009` gives an empty `players` list.
    - A `.fail` switch gives a 503.
    - It logs requests (key masked in its own log) and **refuses any request that has a
      `Referer`, a cookie or a redirect follow**.
    - Assets point at `{BASE}/art/`. GetItems answers `success: 15` for the tool ids.
  - **`fake-tools/steam.mjs`** (through `GRID_LAUNCHER_E2E_STEAM_EXE`) records its argv
    to `E2E_STEAM_ARGV_FILE` and acts on the link in the fake root:
    - `install/<id>`: writes the manifest 1026 with growing `BytesDownloaded` over ~6 s,
      then 4;
    - `uninstall/<id>`: deletes the manifest;
    - `validate/<id>`: records only;
    - `-silent rungameid/<id>`: writes `RunningAppID` = id to `registry.vdf`, starts a
      sleep process, and sets it back to 0 after `E2E_STEAM_RUN_SECS` (default 4).

    Markers in the fake root: `.fake-install-cancel` (no manifest ever), `.fake-no-run`
    (`RunningAppID` never set). It prints `FAKE-STEAM-KEY-not-real` on stdout and stderr,
    to prove GRID reads neither.
  - **The fake root** (`fixtures-steam/`): two libraries.
    - Installed: Moonlit Ferry (StateFlags 4), Deep Field Radio and Old Brick Road (4,
      `outside`), plus Proton 9.0 (2805730), Steam Linux Runtime 3.0 (1628350) and
      Steamworks Common Redistributables (228980), which must never show.
    - `loginusers.vdf`: SamPlays (`…0001`, MostRecent 1) and couchguest (`…0002`), each
      with an `AccountName` of `FAKE-ACCOUNT-NAME-not-shown`, which must never show.
    - `registry.vdf` with `RunningAppID` 0.
  - **The seed** (`steam-seed.mjs`): connected as SamPlays with the key written to the
    run's gnome-keyring (through the app's own `steam_save` in a helper, so the seed
    writes no secret to a file); Quiet Comet, Harbor of Bells, Rime Assembly and Signal
    Lost owned and not installed. `--nokey`, `--private` and `--noclient` give the other
    states.

### D — UI (desktop and TV)

- **Owns:** `app/src/lib/gamestores/**`, `stores/gameStores.svelte.ts`, `fakeApi.ts` and
  `devMock.ts`, `settings/StoresPage.svelte` (+ a new `SteamCard.svelte`),
  `app/src/lib/tv/stores/*` (+ a new `TvSteamPage.svelte`), the Downloads row words,
  `lib/notifications/*` store words, `e2e/helpers/steam.ts`, and
  `e2e/specs/steam-library.spec.ts` and `tv-steam.spec.ts`.
- **Work:**
  - `ENABLED_STORES = ['amazon', 'epic', 'gog', 'steam']`; `storesLede()` gets the
    prototype's full line.
  - `managedByClient` drives every Steam difference from the inventory. Every path that
    assumes a tool or a GRID-run job is checked against it (Phase 3 lesson: Amazon found
    the "tool ⇒ cloud" paths; Steam will find the "store ⇒ tool" and "job ⇒ cancellable"
    ones):
    - Install in Steam (no dialog);
    - Installing in Steam (disabled, "Managed by Steam");
    - no Game Settings;
    - Verify in Steam / Uninstall in Steam… (no confirm of GRID's own);
    - Running in Steam (no Stop);
    - Downloads rows with a disabled Cancel; TV's A-list without Cancel;
    - the teal cover bar;
    - "Installed in Steam, outside GRID".
  - The Settings card: the client line, the account radios plus manual entry, the key
    row (saved: masked + Change… / Remove, with the tail per Question 2; not saved: a
    password field), the help with the steamcommunity.com/dev/apikey link (opens in the
    browser through the opener plugin), Test, Save / Connect Steam, Show in Library,
    Disconnect… with its inline confirm, and the five Test results in the design's words
    (the 401/403 words per Question 1).
  - The key input is cleared after Save, never kept in a store, never logged, and never
    in a URL or `localStorage`.
  - TV: Steam's page with the account sheet, "Enter a SteamID64…" through the keyboard,
    the key through the **masked** keyboard (`TvKeyboard` `mask`), and the results as
    notification cards.
  - `fakeApi`/`devMock` get the prototype's s1–s7, SamPlays, both accounts, the
    `/usr/bin/steam` client, a Quiet Comet install at 52%, and the scenarios `nokey`,
    `private`, `badkey`, `noclient`, `off`, plus Test results.
  - Available Steam games have no size (Steam's API gives none): the Size fact and the
    header total leave it out, never guessed. Installed games show `SizeOnDisk`.
- **Milestone D1** (pure modules: `managedByClient` paths, Steam words, Steam card model,
  Downloads line) lands first.

**Order:** Step 0, then A, C and D together; D works against `fakeApi` until C is merged.
Merge A, C, D, then run the integration list and the full e2e suite on the merged tree.

## Parity inventory (Steam states in the prototype)

Each item is checked by screenshot against the prototype before its package is done
(`design-parity-handoff`). Sources: `stores.js` (L24-26 store, L51-57 games, L77-80
availability, L101-118 rail, L120-147 covers, L165 states, L203-252 header and panels,
L413-422 install and play, L448-500 Details, L600-625 Downloads, L660-690 Settings card,
L762-848 actions), `stores_bk.js` L9, `tvstores.js` (L12-13, L32-37, L52-59, L73, L85-119,
L147-170, L212-222, L237-303, L352-415, L498-516, L532-551) and `tvstores_bk.js` L9.

Desktop (GameStores):
1. **Rail group** "STEAM" (mark S, `hsl(210 45% 30%)`), last, with Installed / Available
   counts:
   - the heading `!` when the client is missing or the key isn't `ok`;
   - the Available count `!` for no key, rejected key or private; a spinner while loading;
   - the Installed count `!` when the client is missing;
   - no group when not connected.
2. **Header:**
   - "Steam · Installed" with "3 games · 21.1 GB · in your Steam libraries";
   - "Steam · Available" with "N games you own and haven’t installed · updated 5 min ago",
     or "Games you own and haven’t installed" while blocked;
   - chip "Steam account: SamPlays" (opens Settings); no tool chip, no cloud chip;
   - Refresh on Available when not blocked; no Select, Backup all or Install from
     installer.
3. **Covers:**
   - Moonlit Ferry, Deep Field Radio and Old Brick Road installed (dot), Play;
   - Quiet Comet installing in Steam: "52%" tag, **teal** bottom bar, disabled "Installing
     in Steam" with the title "Managed by Steam";
   - Available covers with **Install in Steam**;
   - no UPDATE, SAVE CONFLICT or NOT SUPPORTED tag, and no cloud button.
4. **Panels:**
   - "Loading your Steam games…" with skeletons;
   - nothing installed ("No Steam games installed yet" / "Install one from Available.
     Games you install in Steam itself show here too." · Open Available);
   - everything installed (· Refresh);
   - **no key** ("Steam needs your Web API key to list the games you haven’t installed" ·
     Add your key in Settings…);
   - **key rejected** (err tone; "Steam answered 403 Forbidden. The key may have been
     revoked…" per Question 1 · Open Settings › Stores);
   - **private** (warn tone; the privacy path · Refresh / Open Settings › Stores; a
     Refresh that is still private gives the error toast "Steam still lists no games: the
     profile’s game details are private");
   - **Steam isn’t installed** (Installed view; · Check again; then the "Steam client
     found" toast);
   - not connected.
5. **Install:** no dialog. Install in Steam gives the toast "Steam opened to install
   <title>. Choose the library folder there; GRID shows the progress.", or "Steam isn’t
   installed. Install Steam, then try again." (error).
6. **Details:**
   - Play / Install in Steam / the "Installing in Steam · 52%" box with its bar and
     "Managed by Steam. Pause or cancel it in Steam." and a disabled Cancel;
   - Verify in Steam (toast "Steam is verifying <title>’s files. Watch it in Steam.") and
     Uninstall in Steam… (toast "Steam opened its uninstall window for <title>. It leaves
     the Library once Steam removes it.");
   - **no Game Settings**;
   - lines: last played, "Launches through Steam", "Installed in Steam, outside GRID",
     "Steam games can’t be backed up here. Steam has its own Backup and restore.";
   - chips: the store chip and Playing only (no version, no Windows/Linux chip);
   - Overview facts without Folder (and without Size for an Available game, and without
     Genres; see Choices 6); tabs Overview and Media.
7. **Launch:** "Starting <title> through Steam…"; the Playing chip while Steam runs it;
   Stop reads **"Running in Steam"**, and pressing it gives "<title> is running in Steam.
   Quit it from the game."
8. **Downloads:**
   - "Installing in Steam 52% · Managed by Steam" with a disabled Cancel (title "Managed
     by Steam") beside the queue;
   - Completed "Installed by Steam";
   - the strip prefers a non-Steam job;
   - the Steam row takes no slot.
9. **Notification cards:**
   - installed · Play;
   - Steam connected · N games owned / connected but private (error);
   - Steam disconnected ("Your key was deleted from the keyring.");
   - Web API key removed ("Steam’s Available list needs it.");
   - Steam client found;
   - uninstalled (no cloud sentence).
10. **Settings › Stores Steam card:**
    - head "SteamID64 76561198000000001 · SamPlays", pill Connected / Installed games only
      / Not connected ("Not set up");
    - the lede;
    - the client line found (`/usr/bin/steam`) / not installed · Check again;
    - the Account fieldset (two radios with notes, Enter a SteamID64 + field, the help
      line);
    - the Web API key row (saved / not saved), the help with the link;
    - the Test results (works · N games owned, rejected, private + help, not a SteamID64,
      no network, Asking Steam…);
    - Save / Connect Steam, Test, Show in Library, Disconnect…;
    - the inline confirm "Disconnect Steam? GRID forgets your SteamID64 and deletes your
      Web API key from the OS keyring. Steam games leave the Library; they stay installed
      in Steam."
11. **The Stores lede:** "…Steam games install and start through the Steam client."

TV (TVStores):
- items 1–4 as the TV folder and panels (Steam ×4 and Check again, "Install Steam in
  desktop mode…");
- X quick action Install in Steam;
- the game page buttons ("Installing in Steam · 52%" disabled, A says "Managed by Steam.
  Pause or cancel it in Steam."; Verify in Steam; Uninstall in Steam…; no Game settings);
- the left column with the Managed by Steam box, "Installed in Steam, outside GRID" and
  the no-backup line;
- the Playing page "Steam · running in Steam" with "Steam runs this game. Quit it from the
  game; GRID notices when it closes." and **no Stop game**;
- the install toast with "Steam’s window opens over TV mode.";
- Downloads rows ("Installing in Steam 67% · Managed by Steam"), the A-list without
  Cancel (subtitle "Managed by Steam: pause or cancel it in Steam"), "Installed by Steam";
- Settings › Stores row "SteamID64 …" with Connected / Installed games only / Not
  connected;
- **Steam's page:** Steam client, Account (sheet + Enter a SteamID64… through the
  keyboard), Web API key (masked keyboard; the row per Question 2), Test, Show in Library,
  Disconnect… (confirm box), Save / Connect Steam (Start); results as cards with the
  desktop's words;
- the disconnect card "Steam disconnected" / "Your key was deleted from the keyring".

## Spikes

Run them first, in the container (`scripts/ctr.sh run …`). There is no real Steam client
and no real key. Network probes send only `FAKE-STEAM-KEY-not-real` or no key.

1. **Parsing real formats:**
   - steamlocate 2.1.1 and keyvalues-parser on public real-format samples:
     - steamlocate's own test fixtures and snapshots;
     - `libraryfolders.vdf` and manifests from public bug reports (Proton, Lutris,
       steamlocate issues);
     - `loginusers.vdf` and `registry.vdf` shapes from public projects;
   - record: a manifest cut in half, a `path` with `\\` and a quote, a library whose
     folder is missing, a manifest with unknown keys, and whether steamlocate reads
     `BytesStaged`/`BytesToStage`;
   - pick steamlocate or a hand parser;
   - complete the Proton/runtime app id list from public sources (SteamDB lists).
2. **The Web API from the container:**
   - repeat the 401/403 probes (status, `Content-Type`, no `Retry-After`);
   - what a 3xx would be (none seen);
   - `GetItems` for 50 ids, including the tool ids (success 15) and a delisted game;
   - the cover rule on 20 app ids, old and new (hash folders), recording the misses;
   - the private `{}` and the owned shape wait for the user's key.
3. **Opening links:**
   - with a recording fake `steam`, `flatpak` and `xdg-open` on `PATH` and fake roots,
     check which argv each root layout produces (native, Flatpak, Snap, no executable);
   - check that GRID's spawn leaves no zombie and reads nothing;
   - check the `/proc/*/cmdline` fallback against a process started as `reaper
     SteamLaunch AppId=620 -- sleep 30`, and how long one pass takes with 500 processes.
4. **The key in logs:**
   - a request carrying `FAKE-STEAM-KEY-not-real` (200, 302, 401, a refused connection,
     a timeout) against a local server, under a `TRACE` subscriber for every target;
   - grep the captured output; decide whether `logging.rs` needs the `reqwest`/`hyper`
     directives.
5. **The real client** (manual, by the user, optional):
   - `steam://install`, `uninstall`, `validate` and `-silent rungameid` with Steam
     running, not running and signed out, on native and Flatpak Steam;
   - what `registry.vdf` (`RunningAppID`, `apps/<id>/Running`) shows while a native and
     a Proton game run;
   - the manifest's flags and bytes during an install, a pause and an update;
   - the user's key: Test works, the private toggle gives `{}`, and a revoked key's
     status.

   Without it, A keeps the rules above.

### What waits for the user's machine

Spike 5, plus:
- the real `GetOwnedGames` shape with `rtime_last_played`;
- `GetPlayerSummaries` for a manual SteamID64;
- Steam's dialogs appearing over TV mode (fullscreen GRID window);
- the Windows registry values (`SteamPath`, `SteamExe`, `RunningAppID`) on a Windows
  host.

## E2E plan

- **Mock Steam** and **fake steam** as in package C. Every other group gets the dead
  loopback base and an empty fake root, so no real Steam on the runner leaks in. The e2e
  build ignores the real roots.
- **Groups in `scripts/e2e.sh`:**

  | group | specs | checks |
  |---|---|---|
  | `steam` | `steam-connect`, `steam-secrets` | **Connect:** not connected → the card; client found (`fake steam` path); the two local accounts (persona only); a bad SteamID64 (local, no request in the mock log); Test with a revoked key (rejected words); private (`…0002`); no network (mock `.fail`); Connect with the good key → "Steam connected · 4 games owned", the rail group, the pill Connected; Remove key → Installed games only + the no-key panel; Save a new key; Disconnect confirm → the group goes, no Steam row left in the registry, the keyring slot empty (`secret-tool lookup` finds nothing), the fake root's files untouched. **Secrets:** after Connect, Test, Refresh, install, launch, uninstall and Disconnect, the fake key appears in **no** GRID log line (run with debug prints on), `stores-changed`/`store-notice` event, `invoke` answer, config file, registry DB, `stores/cache/*`, session record, Downloads snapshot or DOM text (the key field is empty after Save). The mock's log shows the key arrived only as the `key` query parameter of the API host, and never on an art or GetItems request. `FAKE-ACCOUNT-NAME-not-shown` appears nowhere, and neither does the fake steam's stdout token. |
  | `steam-library` | `steam-library` | rail counts (tools filtered: 3 installed, never Proton/runtime/redistributables), header chips, every panel (seed flags), covers by title, Install in Steam → the toast, the argv file shows `steam://install/<id>`, the teal bar and "Installing in Steam N%" on the cover, Details box and Downloads row with Cancel disabled, then "Installed by Steam" and the card; Details lines ("Installed in Steam, outside GRID", Launches through Steam, the no-backup line, no Game Settings); Play → argv `-silent steam://rungameid/<id>`, Playing chip, Stop reads Running in Steam, then the session ends when the fake clears `RunningAppID`, and `last_played_at` moved; Verify/Uninstall in Steam → argv and toasts, the game leaves Installed after the fake deletes its manifest; `.fake-install-cancel` → the row ends Cancelled; a manifest dropped into the fake root by the spec (a game installed in Steam itself) shows in Installed within the 15 s scan |
  | `tv-steam` | `tv-steam` | TV folder and panels, X Install in Steam, game page (disabled Installing in Steam with A's line), Playing page with no Stop, Downloads A-list without Cancel, Settings › Stores › Steam: the account sheet, Enter a SteamID64… and the masked key through the on-screen keyboard (the keyboard never shows the key in clear), Test results as cards, Connect Steam, Disconnect confirm |

- **Existing specs:**
  - every GOG, Epic and Amazon group must still pass with Steam not connected (no rail
    group);
  - `launch` still passes (the `launch_watched` default is `None`);
  - the `epic-library` / `amazon-library` lede assertions change once (the full
    prototype lede).

## Gate requirements

- The gate (CLAUDE.md), `check_secret_hygiene.sh` included, with the new rules from Step
  0. Add no `expose_secret` file beyond `stores/steam/api.rs`.
- **`scripts/ctr.sh wincheck`.** The HKCU reads, the Windows alive check, the
  `SteamExe` path and the `winreg`/steamlocate Windows deps are `cfg(windows)`; the
  `/proc` fallback and the Flatpak/Snap finder are `cfg(unix)`/`cfg(target_os = "linux")`.
  Run it on Step 0 and again on the merged tree.
- The **full** e2e suite on the merged tree. Step 0 touches `launch_game_key`, the queue
  and `StoreStatus`, so every store group and `launch` are regression checks.
- Fixtures use `FAKE-STEAM-KEY-not-real` (and the `-revoked` suffix form) only. The fake
  account name `FAKE-ACCOUNT-NAME-not-shown` is not a credential and matches the allowed
  `FAKE-…` form.
- D ticks every parity item, desktop and TV.

## Integration list (expected, from the Phase 2 and 3 outcomes)

1. **One key reader.** Only `stores/steam/api.rs` exposes the key; the app never formats
   it. Grep `expose_secret` and `SecretString::from` in the Steam files.
2. **One owned-list owner.** Only `stores_service/steam.rs` writes or deletes
   `stores/cache/steam-owned.json` and `steam-apps.json`. Disconnect deletes both.
3. **One opener and one override.** Every `steam://` spawn goes through
   `SteamService::open`; tests and e2e use `with_roots_override` only.
4. **The launch route.** Desktop and TV `launch_game_key` for a Steam key reach
   `launch_watched`; GOG, Epic and Amazon still reach `launch_key` and the router hook.
   `STEAM_LAUNCH_UNSUPPORTED` stays as the guard for any other caller.
5. **Watched session end.** The session-finished hooks run for a Steam session (no cloud
   upload: Steam's `sync_after_session` does nothing); the Playing page and chip clear.
6. **Downloads.** The external row takes no slot (a GOG install runs beside a Steam one);
   Cancel and Retry are not offered; the row survives a restart while Steam still
   installs.
7. **Filtered everywhere.** The Library's All games, Recent and search never show a
   tool/runtime row; no Steam registry row is ever written for a filtered app.
8. **No platform row.** `library/rail.ts` lists no "Steam" platform row.
9. **Art.** `ensure_image` accepts the Steam hosts and the e2e origin, and refuses
   `*.akamaihd.net` other than `steamcdn-a`.
10. **No store-tool assumptions.** Grep the UI for `storeTool(` / `hasTool` paths that
    would show "undefined not installed" or a tool row for Steam.
11. **TV specs** keep `switchToWindow(MAIN_WINDOW)`. No login window opens for Steam.
12. **Docs.** SPEC "Game stores" gets a Steam paragraph (the key's place, what GRID reads
    from Steam's files, the watched session); ARCHITECTURE gets `stores::steam`,
    `stores_service/steam.rs` and the `SteamKeyStore` slot; `external-stores-plan.md`
    gets its status line; this plan gets its outcomes.

## Choices taken (reversible on request)

1. **Connect needs a key Steam answers for.** The user decision requires the key, so
   Connect without one is refused ("Paste your key first"). The design's "Installed games
   only" state comes only from Remove key, or a key Steam later rejects.
2. **The SteamID64 is not secret.** It is the public profile number and lives in
   `[stores.steam]`. The key never does.
3. **One GetOwnedGames call is the test.** No `GetPlayerSummaries` unless the SteamID64 is
   not one of the local accounts.
4. **Steam games are registry rows**, reconciled from Steam's files, so last played,
   Recent, sort and search work as for other stores. Disconnect deletes the rows, not the
   games.
5. **Games only** = the denylist + name patterns + the store type (game or demo). Owned
   software (Wallpaper Engine, RPG Maker) is hidden too.
6. **No genres.** Steam's keyless metadata gives tag ids, not names. Overview leaves
   Genres out rather than calling another, rate-limited store API per game.
7. **No size for Available Steam games.** No Steam API GRID uses gives a download size.
8. **A game Steam never starts** (within 180 s) ends its session silently.
9. **No Steam updates in GRID.** Steam updates in the background; Library › Updates never
   lists a Steam game.

## User decisions (2026-11-14)

1. **Rejected key wording.** The message drops the status code: it says Steam rejected the
   key, with no "401"/"403". This overrides the design's "403 Forbidden" text.
2. **Saved key display.** Dots only ("Key saved" with dots). No part of the key ever leaves
   the keyring code; the design's "…3F2A" suffix is not shown.
3. **Flatpak/Snap client line.** "Steam client found · Flatpak (com.valvesoftware.Steam)"
   (Snap: its package name). Native installs keep showing the path.

Questions for the user: none open.
