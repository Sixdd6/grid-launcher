# Game stores — Phase 2 (Epic Games Store) work plan

Status: planned 2026-11-13. Read it with three other documents:
- `docs/external-stores-plan.md`, for the user decisions;
- `docs/stores-phase1-plan.md`, for the Phase 1 template and its lessons;
- the approved design cards `docs/design/system/components/GameStores/README.md` and
  `TVStores/README.md`. Their generator sources are `docs/design/prototype/stores.js`,
  `stores_bk.js`, `tvstores.js` and `tvstores_bk.js`. The design is the spec for every
  screen and state.

What Phase 1 left on main (c441b5a and later):
- the shared `stores::{tools, paths, progress, runner, art}`;
- `GameKey` and the registry store rows (v12);
- `StoreJob` with `download`/`finalize` and `JobPhase`;
- the `store-login` window (`store_auth.rs`);
- every store Tauri command, with a `source` argument;
- the `StoreStatus`/`StoreGame` IPC model;
- the desktop and TV store UI, with Epic, Amazon and Steam hidden by
  `gamestores/marks.ts` `ENABLED_STORES = ['gog']`;
- `NativeLaunch::redact`.

The gaps that are GOG-only today:
- `StoresService` (one `GogApi`, one `GogState`, `GogConfig`);
- `commands/stores.rs` (`require_gog`, `GameKey::store(StoreKind::Gog, …)`);
- `store_auth::classify`, which knows only the `on_login_success` redirect;
- `runner::run_tool`. It has stdin null, only `parse_gogdl_line`, and drops every other
  line;
- `is_store_art_url`, which knows only GOG hosts;
- `ToolStatus.name: 'gogdl'`;
- `StoreGame.origin`;
- the GOG-only words in `gamestores/*.ts` and `tv/stores/storeWords.ts`.

What is in scope:
- Epic login, the owned list, install (with DLC), update, verify and repair, uninstall,
  launch and legendary cloud saves;
- the design's Epic states (see the parity inventory below).

What is out of scope:
- Epic games on backup drives (user decision; Details says why);
- importing existing legendary/Heroic/EGL installs (`import`, `egl-sync`);
- the EOS overlay;
- macOS-native Epic builds (GRID ships no macOS build yet);
- Unreal Engine marketplace items;
- games managed by a third-party launcher. These show as **Not supported**, as the
  design says.

## Research (2026-11-13)

### legendary release

- **Pin `0.21.1` "Lowlife".** It was published 2026-09-08 and is the latest stable
  release, not a prerelease.
  - The repo moved: `derrod/legendary` now redirects to `legendary-gl/legendary`.
  - master HEAD 170964e (2026-11-04) has dependency bumps only.
  - License GPL-3.0 (repo `LICENSE`).
- Assets are under `https://github.com/legendary-gl/legendary/releases/download/0.21.1/`.
  - The project publishes no SHA256SUMS. The hashes below were **computed from the
    downloaded assets** (sha256sum; the files were never run). They match GitHub's
    per-asset `digest` in the releases API.

  | asset | os/arch | size | sha256 | kind |
  |---|---|---|---|---|
  | legendary_linux_x64 | linux/x86_64 | 3093622 | dbed33bbe96031e65858233e4badf0a1d401bb6cb0cc995d1237cf3a0c826a45 | Python zipapp |
  | legendary_linux_arm64 | linux/aarch64 | 2999455 | e901cb52ada10d5cef6ffb2a0985099958124457bfb455a571023d1e042143d9 | Python zipapp |
  | legendary_windows_x64.exe | windows/x86_64 | 17654404 | 6be77857dc0a6dd33ea7442ce8b5291944ec6da53757f81ca95b57ce794baf5b | PyInstaller |
  | legendary_windows_arm64.exe | windows/aarch64 | 17147114 | 5ca178d75713cb92ee24daa5df1905c75f05b8b403383669ca7aa32a5809fba3 | PyInstaller (digest from the API; not downloaded) |
  | legendary_macOS_x64 | macos/x86_64 | 15566864 | 3dfab50277284b5bb0104f8710e4ff1d9ff00c992e44f5843fd0c977c5cc99c4 | PyInstaller |
  | legendary_macOS_arm64 | macos/aarch64 | 15275984 | d87978321dba9cb731fab40c72f0a30bed55baca8b5341ab21024c5733cd837e | PyInstaller |

  `legendary_macOS.zip` (1278af67…3b06) is not used.
- **The Linux assets are zipapps** (since 0.21.0).
  - They start with `#!/usr/bin/env python3` and need Python ≥ 3.10.
  - They bundle requests, urllib3, certifi, filelock and pycryptodomex. Native
    `Cryptodome` `.so` files unpack into `$XDG_CACHE_HOME/legendary/vendored`.
  - There is no pywebview.
  - This is the same situation as gogdl. Reuse `AssetLauncher::Python3` and the Python 3.10
    check, and set `XDG_CACHE_HOME=<data>/stores/legendary/cache`.
- **Heroic pins the same upstream `0.21.1`** in `meta/downloadHelperBinaries.ts`
  (commit 87a61d0, 2026-09-09), with no checksum check. Its own fork
  (`Heroic-Games-Launcher/legendary`, last tag 0.20.43) is archived; 0.21.1 backports
  the fork's changes.

### Login (Heroic's way, caught in Rust)

- Heroic loads `https://www.epicgames.com/id/login?responseType=code` in a webview.
  - Its user agent is `Mozilla/5.0 (Windows NT 10.0; Win64; x64) EpicGamesLauncher`.
  - After sign-in, Epic sends an EGL user agent to
    **`https://localhost/launcher/authorized?code=<authorizationCode>`**: a navigation
    whose URL carries the code.
  - Heroic reads `code` and runs `legendary auth --code <code>`.
  - Source: Heroic `src/frontend/screens/WebView/index.tsx`,
    `src/backend/storeManagers/legendary/user.ts`.
- **GRID catches it exactly as it catches GOG's redirect.**
  - `on_navigation`, with `on_page_load` as the fallback, matches scheme, host, port and
    path (`/launcher/authorized`), reads `code` into a `SecretString`, and **refuses the
    navigation**.
  - So nothing on the user's port 443 ever receives the code.
  - The code never crosses IPC.
- legendary's own flow has a fallback. `https://legendary.gl/epiclogin` sends a 302 to
  `https://www.epicgames.com/id/login?redirectUrl=https%3A//www.epicgames.com/id/api/redirect%3FclientId%3D34a02cf8f4414e29b15921876da36f9a%26responseType%3Dcode`.
  - That ends on a **200 JSON page**, not a 302:
    `{"warning":"Do not share this code with any 3rd party service…","redirectUrl":…,"authorizationCode":"…","exchangeCode":null,"sid":null}`.
  - Catching it needs the page body, so it is only the spike-2 fallback (see Spikes).
- **Keep the code off argv.** `legendary auth --disable-webview` with no `--code`
  calls `webbrowser.open(…)`, then reads the code from **stdin**: the bare code, or the
  JSON blob.
  - Set `BROWSER=true` so no browser opens.
  - legendary exits 0 even when login fails. Failure is the stderr line
    `ERROR: Logging in …`.
  - Source: `legendary/cli.py` `auth()`, lines 93–207.
- **Tokens.**
  - `user.json` in the config dir holds the raw Epic OAuth response.
  - legendary sets **no file mode**, so GRID makes the folder's files private after every
    run.
  - `auth --delete` only deletes `user.json`; it does not revoke the session on the
    server.
  - Other secret-bearing files: `tmp/*.ovt` (ownership tokens) and `version.json`
    (cached client credentials).
  - All of these are under the tool folder that Disconnect deletes.

### Commands and outputs GRID uses

| Function | Detail |
|---|---|
| Config dir | `LEGENDARY_CONFIG_PATH=<data>/stores/legendary` (used as is; otherwise `$XDG_CONFIG_HOME/legendary`). It writes `config.ini`, `user.json`, `installed.json`, `metadata/<app>.json`, `manifests/`, `tmp/`, `assets.json`, `entitlements.json`, `version.json`. |
| Streams | Logging goes to **stderr** as `[Name] LEVEL: msg`. JSON goes to **stdout**. **Exit codes are unreliable:** many failures log `ERROR:` and exit 0. |
| Account | `status --json --offline` gives `{"account","games_available","games_installed","egl_sync_enabled","config_directory"}`. `account` is `"<not logged in>"` when signed out. Without `--offline` it refreshes the login. |
| Owned list | `list --json -T` (`-T` adds third-party titles) gives an array of `{app_name, app_title, asset_infos:{Windows:{build_version,…}}, metadata:{description, developer, keyImages[{type,url}], categories, customAttributes{CloudSaveFolder, ThirdPartyManagedApp, CanRunOffline, FolderName…}, mainGameItem?, releaseInfo…}, dlcs:[…same…]}`. DLC are nested under their base. Platforms are the keys of `asset_infos`. UE items are left out unless `--include-ue`. |
| Installed | `list-installed --json` (offline) gives `{app_name, title, version, install_path, install_size, executable, launch_parameters, platform, is_dlc, can_run_offline, requires_ot, needs_verification, save_path, …}`. |
| Sizes | `info <app> --json` gives `manifest.download_size`, `manifest.disk_size`, `manifest.launch_exe` and `game.cloud_save_folder`. |
| Install | `install <app> -y --skip-sdl --skip-dlcs --platform Windows --base-path <parent> --game-folder <folder>`. **`-y` without `--skip-dlcs` installs every DLC.** The SDL prompt ignores `-y`, so pass `--skip-sdl`. A DLC is `install <dlc_app> -y --skip-sdl` with the base installed. Third-party titles are refused (exit 0). On `MemoryError` Heroic retries with `--max-shared-memory 5000`. |
| Progress | stderr, about 1/s, from `[DLManager] INFO: = Progress: 12.34% (123/1000), Running for 00:01:02, ETA: 00:08:10`, ` - Downloaded: 512.00 MiB, Written: 1024.00 MiB`, ` + Download\t- 20.00 MiB/s (raw) / 40.00 MiB/s (decompressed)`, ` + Disk\t- …`. Before it: `[cli] INFO: Download size: X MiB`. The counts are chunks, not bytes. |
| Update | `update <app> -y --skip-sdl`. An update is found when `list` `asset_infos.Windows.build_version` ≠ the installed `version`. |
| Verify/repair | `repair <app> -y --skip-sdl`. The verify step writes progress to **stdout**, with `\r`: `Verification progress: n/N (x.x%) [y MiB/s]`. The result is on stderr: `Verification finished successfully.` or `Verification failed, N file(s) corrupted, M file(s) are missing.` |
| Uninstall | `uninstall <app> -y --skip-uninstaller` deletes only the manifest's file list (and its DLC first). A DLC uninstall leaves the base. |
| Launch | `launch <app> --json --skip-version-check [--offline]` prints `LaunchParameters` on stdout: `{game_executable, game_directory, working_directory, game_parameters[], user_parameters[], egl_parameters[…,"-AUTH_PASSWORD=<exchange code>","-AUTH_TYPE=exchangecode","-epicapp=…","-epicovt=…"], environment{}, launch_command[], pre_launch_command}`. The exchange code is short-lived and single-use. **`--dry-run` logs the whole command line, code included, to stderr: never use it.** |
| Cloud saves | `list-saves <app>` prints manifest names `YYYY.MM.DD-HH.MM.SS.manifest` on stdout: that is the cloud time. `sync-saves <app> -y --save-path <p> --force-upload\|--force-download`. legendary decides from mtimes only (newer side wins; within 60 s counts as the same age), so GRID decides the direction itself, as for GOG. **A download cleans the save folder first** (`clean_dir=True`). `customAttributes.CloudSaveFolder` has `{InstallDir}`, `{EpicID}`, `{AppData}` (Local AppData), `{UserDir}` (Documents), `{UserSavedGames}`, `{LocalLow}`/`{Roaming}` (legacy). |
| Update notice | `config.ini [Legendary] disable_update_notice = true`. Keep the update check: it fetches legendary's remote config (EGL version, user agent, client credentials) from `api.legendary.gl`, which is how a legendary install keeps working when Epic changes. Heroic leaves it on. |
| Third party | `customAttributes.ThirdPartyManagedApp` (else `ThirdPartyManagedProvider`): `origin`/`the ea app` gives "the EA app"; `ubisoftconnect` gives "Ubisoft Connect". |

Sources for the table: legendary `cli.py`, `core.py` (`get_save_path`,
`check_savegame_state`, `get_launch_parameters`), `models/game.py`,
`downloader/mp/manager.py` lines 766–775, and `lfs/lgndry.py` lines 23–31. The 0.21.1
sources were read on the tag.

- **Linux.** Epic has no Linux builds: legendary accepts only `Windows`, `Win32` and `Mac`.
  Every Epic game runs under GRID's compat tool.
  - Heroic lets legendary spawn the game (`--no-wine --wrapper "<umu-run>"`, `STORE=egs`,
    `GAMEID=umu-<id>` from umu's database).
  - **GRID instead owns the process.** It takes `launch --json`, then builds argv through
    `build_native_command` (umu-run/wine/native), so the session holds the child.
- **Art.** Heroic's choices:
  - cover: `DieselGameBox` / `OfferImageWide`;
  - portrait: `DieselGameBoxTall` / `OfferImageTall`, then `DieselStoreFrontTall`;
  - logo: `DieselGameBoxLogo`.

  The hosts are `cdn1.epicgames.com` and `cdn2.unrealengine.com`, sampled from the public
  promotions feed. Library metadata uses the same CDN; spike 4 confirms it.
- **Limits.**
  - Epic has no public API. legendary poses as the Epic Games Launcher, with its client ID
    `34a02cf8f4414e29b15921876da36f9a`, user agent and label, which `api.legendary.gl` can
    update remotely.
  - Epic's code page warns not to share the code with a third party. In GRID the code goes
    only to the local legendary.
  - EA/Ubisoft-managed titles need those launchers inside the prefix, so GRID shows them as
    Not supported.
  - Anti-cheat titles depend on Proton; GRID does nothing special.
  - I found no Epic term that GRID's use adds to beyond what Heroic and legendary already
    do.

Sources:
- https://github.com/legendary-gl/legendary (README, `legendary/cli.py`, `legendary/core.py`,
  `legendary/api/egs.py`, `.github/workflows/build-base.yml`, `zipapp_main.py`)
- https://api.github.com/repos/legendary-gl/legendary/releases/tags/0.21.1
- https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher (`meta/downloadHelperBinaries.ts`,
  `src/backend/storeManagers/legendary/{games,library,user}.ts`,
  `src/frontend/screens/WebView/index.tsx`, `src/backend/save_sync.ts`,
  `src/backend/launcher.ts`, `src/backend/wiki_game_info/umu/utils.ts`)
- https://legendary.gl/epiclogin (302, checked with curl)

## Step 0 — contracts skeleton (one commit before fan-out)

Step 0 delivers these in one commit:
- every type below;
- the `stores_service` split, with the GOG backend moved behind the trait and its
  behaviour unchanged;
- the generalised `store_auth` rule;
- the runner extensions, fully implemented and with tests, since every package uses them;
- `EpicMeta` and the row builder with its tests;
- `Config.stores.epic`;
- `api.ts` types;
- `check_secret_hygiene.sh` allowing `expose_secret` in `stores/runner.rs` (writing stdin)
  and `stores/epic/launch.rs` (the exchange code into argv);
- the Epic stub service, so every Epic call returns `not_implemented`.

**Step 0 passes the gate and every `gog`, `gog-*` and `tv-gog` e2e group before fan-out.**
After it lands these files are frozen, and changes go through the lead: `api.ts`,
`config.rs`, `stores/mod.rs`, `stores/runner.rs`, `stores/epic/{meta,row}.rs` and
`stores_service/mod.rs`.

### grid-core

```rust
// stores/tools.rs
pub enum StoreTool { Gogdl, Legendary }             // name "legendary", e2e env GRID_LAUNCHER_E2E_TOOL_LEGENDARY
static LEGENDARY_PIN: ToolPin;                       // 0.21.1, GPL-3.0, github.com/legendary-gl/legendary, the six assets above
                                                     // (linux Python3, windows/macos Direct); filled in Step 0

// stores/runner.rs  (implemented in Step 0)
pub enum LineParser { #[default] Gogdl, Legendary }  // picks the stderr progress parser and the signal matcher
pub struct ToolCommand { /* existing */ pub stdin: Option<SecretString>, pub parser: LineParser,
                         pub progress_stdout: bool /* also parse stdout, split on \r and \n (verify) */ }
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolSignal {                               // GRID's own words for known lines; the raw line is dropped
    LoginFailed, NotLoggedIn, MemoryError, NotEnoughSpace, ThirdPartyManaged, NotInstalled,
    VerifyOk, VerifyFailed { corrupted: u32, missing: u32 }, DownloadSize(u64), OtherError }
pub struct ToolExit { pub code: Option<i32>, pub stdout: SecretString, pub signals: Vec<ToolSignal> }
// After every Legendary run: paths::make_tree_private(tool_dir) (files 0600, dirs 0700; unix only).

// stores/progress.rs
pub fn parse_legendary_line(line: &str) -> Option<ToolProgress>;   // Progress %, Downloaded MiB, + Download speed, verify n/N
pub fn legendary_signal(line: &str) -> Option<ToolSignal>;

// stores/paths.rs
pub fn make_tree_private(dir: &Path) -> io::Result<()>;

// stores/art.rs  (generalised)
pub fn is_store_art_url(url: &str, extra_origins: &[Url]) -> bool; // GOG hosts + *.epicgames.com, *.unrealengine.com
                                                                    // + the e2e origins; https only except the e2e origins

// stores/epic/meta.rs  (store_meta JSON; never a token)
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct EpicMeta { pub app_name: String, pub version: String, pub platform: String /*"Windows"*/,
    pub dlc: Vec<EpicDlcMeta>, pub cloud_saves: bool, pub cloud_save_folder: String, pub save_path: String,
    pub last_sync: i64, pub can_run_offline: bool, pub exe_user_pinned: bool }
pub struct EpicDlcMeta { pub app_name: String, pub title: String }
impl EpicMeta { pub fn from_json(..); pub fn to_json(..); }

// stores/epic/mod.rs  (types in Step 0, bodies package A)
pub struct EpicEndpoints { pub login: Url, pub finish: Url /* https://localhost/launcher/authorized */,
                           pub art_origins: Vec<Url> }  // ::real(), ::e2e(base)
pub struct EpicGame { pub app_name: String, pub title: String, pub cover: String, pub screenshots: Vec<String>,
    pub description: String, pub developer: String, pub year: String, pub build_version: String,
    pub third_party: Option<String> /* "the EA app" | "Ubisoft Connect" */, pub cloud_save_folder: String,
    pub can_run_offline: bool, pub dlc: Vec<EpicGameDlc> }
pub struct EpicGameDlc { pub app_name: String, pub title: String }
pub struct EpicInstalled { /* list-installed row: app_name, title, version, install_path, install_size,
    executable, launch_parameters, is_dlc, can_run_offline, save_path */ }
pub struct EpicInstallOptions { pub download_bytes: u64, pub disk_bytes: u64, pub dlc: Vec<(EpicGameDlc, u64)> }
pub struct EpicInstallRequest { pub app_name: String, pub title: String, pub dlc: Vec<String>,
    pub install_dir: PathBuf, pub compat_tool: String }
pub struct EpicJobCtx { pub registry: Arc<Registry>, pub library: PathBuf, pub default_compat_tool: String }
pub struct EpicAccount { pub username: String }
pub struct EpicService;   // Clone, Send+Sync
impl EpicService {
  pub fn new(data_dir: PathBuf, http: reqwest::Client, ep: EpicEndpoints) -> Self;
  pub fn login_url(&self) -> Url;
  pub async fn complete_login(&self, code: SecretString) -> Result<EpicAccount, StoreError>;  // auth --disable-webview, code on stdin
  pub async fn account(&self) -> Result<Option<EpicAccount>, StoreError>;                     // status --json; Err(LoginExpired)
  pub fn disconnect(&self) -> io::Result<()>;                                                 // auth --delete, then remove_tool_dir
  pub async fn owned(&self) -> Result<Vec<EpicGame>, StoreError>;                             // list --json -T
  pub async fn installed(&self) -> Result<Vec<EpicInstalled>, StoreError>;                    // list-installed --json
  pub async fn install_options(&self, app: &str) -> Result<EpicInstallOptions, StoreError>;  // info --json (base + each DLC)
  pub fn install_job(&self, req: EpicInstallRequest, ctx: EpicJobCtx) -> StoreJob;
  pub fn update_job(&self, row: &InstalledGame, ctx: EpicJobCtx) -> StoreJob;
  pub fn repair_job(&self, row: &InstalledGame, ctx: EpicJobCtx) -> StoreJob;
  pub fn dlc_job(&self, row: &InstalledGame, dlc: &str, ctx: EpicJobCtx) -> StoreJob;
  pub fn remove_dlc(&self, registry: &Registry, row: &InstalledGame, dlc: &str) -> Result<(), StoreError>;
  pub fn uninstall(&self, registry: &Registry, app: &str) -> Result<(), StoreError>;
  pub fn updates(&self, rows: &[InstalledGame], owned: &[EpicGame]) -> Vec<EpicUpdate>;      // pure: build_version != meta.version
  // package B:
  pub async fn launch_prep(&self, row: &InstalledGame, offline: bool) -> Result<StoreLaunchPrep, StoreError>;
  pub async fn cloud_state(&self, row: &InstalledGame) -> Result<CloudState, StoreError>;
  pub async fn cloud_sync(&self, registry: &Registry, row: &InstalledGame, mode: SyncMode) -> Result<CloudState, StoreError>;
}
// CloudState / SyncMode / SaveConflict are shared: move them from stores/gog to stores/mod.rs
// (re-exported from gog so nothing breaks). SaveConflict.cloud_files becomes Option<u32>.

// stores/epic/row.rs  (implemented in Step 0)
pub fn epic_row(inst: &EpicInstalled, game: Option<&EpicGame>, meta: &EpicMeta, wineprefix: Option<&Path>) -> InstalledGame;
// platform "Epic Games", slug "win", store_id = app_name, native_executable_path = install_path + executable
// (absolute, the same path rules as gog_row), native_working_dir relative, launch params from list-installed,
// native_wineprefix <install dir>/prefix off Windows (Phase 1 integration item 9), companies = developer.

// launch/mod.rs + launch/native.rs
pub struct StoreLaunchPrep { pub executable: PathBuf, pub working_dir: PathBuf, pub game_args: Vec<String>,
    pub secret_args: Vec<String> /* egl_parameters: appended last, every position in NativeLaunch::redact */,
    pub env: Vec<(String, String)> }
impl Debug for StoreLaunchPrep  // prints counts only
pub trait StoreLaunchHook: Send + Sync {
    fn prepare<'a>(&'a self, game: &'a InstalledGame) -> BoxFuture<'a, Result<Option<StoreLaunchPrep>, LaunchError>>; }
impl LaunchService { pub fn set_store_launch_hook(&self, hook: Arc<dyn StoreLaunchHook>); }
// build_native_command gains `prep: Option<&StoreLaunchPrep>`: executable/cwd from prep; argv = runner…, exe,
// game_args, the row's launch parameters, secret_args.

// config.rs
pub struct StoresConfig { pub gog: GogConfig, #[serde(default)] pub epic: EpicConfig }
pub struct EpicConfig { pub connected: bool, pub username: String, pub install_dir: String /*"" = <library>/Epic Games*/,
                        pub cloud_saves: bool /*default true*/ }
```

### App (`app/src-tauri/src`)

- **`stores_service.rs` becomes `stores_service/`:**
  - `mod.rs` holds the facade `StoresService { gog: Arc<GogStore>, epic: Arc<EpicStore> }`, plus
    `statuses()` (both stores, STORE_ORDER), `backend(StoreKind) -> Option<Arc<dyn StoreBackend>>`,
    the emitter and registry plumbing, and `observe_downloads`, which fans out by source.
  - `gog.rs` is the current file, moved unchanged in behaviour.
  - `epic.rs` is the Epic backend (Step 0 stubs; package C fills it).
  - `StoreBackend` is the trait that the commands call. It has every method the store
    commands use today:
    - login, cancel and disconnect;
    - tool install;
    - refresh, library and updates;
    - install folder and cloud toggle;
    - install options, install, update, repair, uninstall, DLC add/remove;
    - game settings and executables;
    - cloud state, sync and resolve;
    - sync before launch, sync after a session;
    - `is_art_url`/`art`;
    - `login_spec()`.
  - GOG-only extras (offline installers, backups) stay on `GogStore`.
- **`store_auth.rs`** gets `LoginSpec { store_name, login_url, finish: Url /*scheme+host+port+path*/,
  code_param: &'static str, user_agent: Option<String> }`.
  - `classify(url, &LoginSpec)` replaces `classify(url, embed)`.
  - The GOG spec is `embed/on_login_success`.
  - The Epic spec is `finish = EpicEndpoints.finish` with `user_agent = Some(EGL UA)`.
  - Both refuse the finish navigation.
- **`commands/stores.rs`.**
  - `require_gog` becomes `backend_for(source)` and `backend_for_key(key)`, for Gog and Epic.
  - `offline_*` and the store backup commands stay GOG-only.
  - **No new Tauri commands and no new events.** `stores-changed`, `store-login`,
    `store-notice`, `downloads`, `installed-changed` and `updates-changed` are reused.
- `launch_game_key`'s store branch is unchanged in shape: it syncs before launch and
  uploads after exit through the row's backend. `lib.rs` installs the Epic
  `StoreLaunchHook`.

### TypeScript (`api.ts`)

```ts
ToolStatus.name: 'gogdl' | 'legendary';
StoreGame.origin: 'gogdl' | 'offline' | 'drive' | 'legendary' | null;
StoreCloudState.conflict.cloud_files: number | null;   // Epic's list-saves gives no file list
```

No new methods. `gamestores/marks.ts` `ENABLED_STORES = ['gog', 'epic']` is set by
package D, not Step 0, so Step 0 ships no half-visible Epic.

## Work packages (after Step 0)

### A — grid-core Epic store

- **Owns:** `stores/{tools,progress,paths,art}.rs` after Step 0,
  `stores/epic/{mod,auth,list,install,update}.rs`, `crates/grid-core/tests/epic_*.rs` and
  their fixtures.
- **Tool.** The pinned legendary install with its SHA-256 check (mismatch: delete, Refused).
  It runs as `python3 <zipapp>` on Linux.
  - Env: `LEGENDARY_CONFIG_PATH`, `XDG_CACHE_HOME`, `BROWSER=true`.
  - A first-run `config.ini` sets `disable_update_notice = true`.
  - `make_tree_private` runs after each run.
- **Auth.** `auth --disable-webview` with the code on stdin; `ToolSignal::LoginFailed`
  marks a failure. `account` comes from `status --json`. `disconnect` is
  `auth --delete` plus `remove_tool_dir`.
- **Owned list.** `list --json -T`:
  - Keep only items with a `Windows` asset.
  - Drop the `mods`/`applications`-only categories.
  - Fold DLC into its base.
  - Read `third_party` from `customAttributes`.
  - Covers and screenshots come from the keyImages order in Research.
  - `year` is the release year when present.
  - Parse with `serde_json`, never by logging.
  - Product metadata goes to the owned cache `<data_dir>/stores/cache/epic-owned.json`
    (no token in it).
- **Install options.** `info --json` for the base and each owned DLC.
- **Jobs.**
  - install: `--skip-dlcs`, then each picked DLC, inside one job's download half;
  - update and repair;
  - DLC add.
  - Progress comes from `parse_legendary_line`. The total is `info`'s `download_size`
    (or `DownloadSize`); speed comes from `+ Download`.
  - Phases: `Downloading`, then `Verifying` for repair.
  - **Success is exit 0, no `OtherError`/`LoginFailed` signal, and the app present in
    `list-installed` with the expected version.**
  - On `MemoryError`, retry once with `--max-shared-memory 5000`.
  - `finalize` writes the row with `epic_row`. The repair result goes to
    `DownloadEntry.detail`, e.g. "Verified · 2 files repaired" from `VerifyFailed`.
- **Uninstall.** `uninstall -y --skip-uninstaller`, then the leftover folder and
  `<install>/prefix` through `RemovalGuard`, as GOG does. `remove_dlc` is `uninstall <dlc> -y`.
- **Updates.** The pure comparison, with no network beyond the owned refresh.
- **Art.** Generalise `is_store_art_url` (Step 0 signature) and its tests.
- **Tests.**
  - JSON parsers on recorded-shape fixtures that carry `FAKE-STORE-TOKEN-not-real`;
  - argv builders (no `--code` anywhere, `--skip-dlcs` always present);
  - the progress and signal parsers;
  - SHA mismatch;
  - a `cfg(unix)` fake legendary script;
  - the exchange code never in Display/Debug.

### B — Epic launch and cloud saves

- **Owns:** `stores/epic/{launch,cloud}.rs`, `launch/{mod,native}.rs` (hook and prep),
  `crates/grid-core/tests/epic_cloud.rs`.
- **Launch prep.**
  - Run `launch <app> --json --skip-version-check`. The stdout `SecretString` is parsed in
    `launch.rs` only.
  - `egl_parameters` become `secret_args`, all of them redacted. They are appended after
    the user's parameters.
  - A `Network` error with `can_run_offline` retries with `--offline` (no exchange code).
  - Other failures map to `LaunchError::Validation` with GRID's words ("Couldn't get a
    launch ticket from Epic Games: …").
  - `pre_launch_command` and `launch_command` are ignored, because GRID's compat tool
    wraps the game.
  - The early-exit message and the session record show `<redacted>`.
- **Save path, resolved by GRID** (pure `resolve_save_path(folder, install_dir, prefix,
  host, prefix_user)`):
  - `{InstallDir}`, `{EpicID}`, `{AppData}`, `{UserDir}`, `{UserSavedGames}`, `{LocalLow}`,
    `{Roaming}`;
  - prefix user `steamuser` under umu/Proton, the host user under plain wine, Windows known
    folders on a Windows host;
  - a case-insensitive walk of existing parts;
  - reuse of the Windows-token helpers in `backup/saves.rs` / `cloud` where they fit.

  The result is stored in `meta.save_path` and always passed as `--save-path`.
- **Cloud state and sync.** These mirror GOG:
  - The cloud time comes from `list-saves <app>`; the local side from mtimes, file count
    and bytes; then compare with `meta.last_sync`.
  - Auto: an upload or download is **forced**, or `Conflict` when both changed.
    `cloud_files`/`cloud_bytes` are `None`.
  - Before a forced download, copy the save folder to
    `<data>/stores/save-backups/epic/<app>/<epoch-ms>/` (newest 5 kept), because a
    legendary download cleans the folder.
  - Off Windows, if the prefix doesn't exist yet, skip the pre-launch download unless
    `prefix_user` is known (umu). The after-session sync then covers it.

### C — app services, login window, e2e harness

- **Owns:**
  - app side: `stores_service/epic.rs`, `store_auth.rs` (Epic spec, user agent, the
    navigation rule), the Epic branches in `commands/stores.rs`, `update_service.rs` store
    updates (by source), `lib.rs` after Step 0, `tauri.conf.json` (asset scope already
    covers `stores/art`);
  - e2e: `e2e/mock-romm/mock-epic.mjs` (+ test), `e2e/fake-tools/legendary.mjs` (+ test),
    `e2e/seed/epic-seed.mjs`, `e2e/helpers/epicLogin.ts`, the `scripts/e2e.sh` groups,
    `wdio.conf.ts`, the e2e env overrides, `e2e/specs/epic-login.spec.ts` and
    `epic-secrets.spec.ts`.
- **Login window.** The Epic `LoginSpec`, using the same window, incognito data dir and
  network-error page as GOG.
  - The navigation rule refuses **any** navigation to `localhost`/`127.0.0.1` that is not
    the finish URL. Pop-ups for third-party sign-in (Google, Apple, Xbox, …) open in the
    same login window, or are refused per spike 3.
  - Success runs `complete_login`, then the legendary install, then the owned list, as for
    GOG.
- **EpicStore.** Status, owned cache and refresh, store notices (background only, as
  agreed in Phase 1), install guards (signed in, tool ready, not running), cloud hooks
  before and after a session, and the `StoreLaunchHook` implementation through
  `EpicService::launch_prep`.
- **StoreGame for Epic.**
  - `platforms: ['windows']` and `installed_platform: 'windows'`;
  - `origin: 'legendary'`;
  - `not_supported` is the launcher name;
  - `download_bytes`/`disk_bytes` stay 0 until `info` has answered (cached);
  - `version` / `update_version` are build versions as legendary gives them.
- **Settings card.** "legendary 0.21.1 · installed"; license "legendary · GPL-3.0 ·
  github.com/legendary-gl/legendary"; the size comes from the host asset (3.1 MB on
  Linux x86_64). These are data from the pin. The prototype's "0.20.37 · 14 MB ·
  derrod" are placeholders.

### D — desktop UI

- **Owns:** `app/src/lib/gamestores/**` (pure `.ts` and `desktop/`),
  `stores/gameStores.svelte.ts`, `gamestores/fakeApi.ts` and `devMock.ts` (Epic fixtures
  from `stores.js` e1–e6), `settings/StoresPage.svelte`, the store parts of
  `Library.svelte`, `lib/notifications/*` store words,
  `e2e/specs/epic-library.spec.ts` and `epic-cloud.spec.ts`, and `helpers/epic.ts`.
- **Work.**
  - `ENABLED_STORES` gains `'epic'`.
  - Replace every `source === 'gog'` that means "a store with a tool/cloud" with the
    store's capability from `marks.ts`: `hasTool`, `hasCloud`, `canBackup` (GOG only) and
    `hasInstaller` (GOG only).
  - Store-named words come from `storeName()`.
  - The install dialog for Epic skips Version and Language.
  - Details lines: Not supported, and "can't be backed up".
- **Milestone D1:** the pure modules first, committed, so E imports them.

### E — TV UI

- **Owns:** `app/src/lib/tv/stores/*` and `e2e/specs/tv-epic.spec.ts`. It imports D1 and
  never edits it.
- **Work.** Port the Epic states from `tvstores.js`/`tvstores_bk.js`. Generalise
  `storeWords.ts` the same way, and add the Epic login host words.

**Order.** Step 0 comes first. A, B and C then start together (backend wave). D and E
follow, against `fakeApi` until the backend is merged. Merge in the order A, B, C, D, E,
then run the integration list and the full e2e suite on the merged tree.

## Parity inventory (Epic states in the prototype)

Every item is checked by screenshot against the prototype before its package is done
(`design-parity-handoff`). For D, from `stores.js` / GameStores:
1. Rail group "Epic Games" (mark E, hue 230) between Amazon Games and GOG, with Installed
   / Available counts. The heading badge is `!` (login expired, legendary missing or
   failed) or `off`. Available's count is a spinner while loading and `!` on an error.
2. Header "Epic Games · Installed" / "· Available", with the chips "Signed in as
   SamPlays", "Cloud saves: on", "Login expired · Sign in again" and "legendary not
   installed". Refresh is on Available. There is no Install from installer…, no Select and
   no Backup all.
3. Covers:
   - Starlight Courier: installed, cloud synced, cloud button;
   - Vector Bloom: installed, no cloud save, never played;
   - Glimmer Deep and Rustbelt Rally: available, Install;
   - Iron Orchard ("the EA app") and Northern Static ("Ubisoft Connect"): the **NOT
     SUPPORTED** tag and Details hover;
   - the installing percentage, QUEUED and the bar;
   - the UPDATE tag.
4. Panels:
   - "Loading your Epic Games games…" over skeletons;
   - nothing installed (Open Available);
   - everything installed (Refresh);
   - login expired (Sign in again…);
   - not connected (Connect Epic Games…);
   - "Epic Games needs legendary … It's <size>." · Install legendary (<size>);
   - "Installing legendary…";
   - "Couldn't install legendary: the download didn't match its checksum, so GRID deleted
     it." · Try again.
5. Install dialog: Windows only (no Version), compat tool, install folder, DLC you own with
   sizes, the space line and Not enough space, the footer.
6. Details:
   - Play / Install… / Cancel install / Not supported (disabled) / Update to … / Game
     Settings / Verify and repair / Uninstall (Confirm uninstall);
   - lines "Runs through <tool> · Windows version" and the cloud states;
   - "Epic Games games can't be backed up: legendary installs only from Epic's servers.";
   - "Needs the EA app. GRID can't install games that start through another company's
     launcher; install it from the Epic Games Launcher instead.";
   - "Your Epic Games login expired. Sign in again";
   - tabs Overview (DLC Install/Remove, facts), Media, and Cloud saves (synced, conflict
     with both sides, off, none, save location, "RomM cloud saves are off for store
     games.").
7. Launch: "Syncing cloud saves for …", "Launching …", "Cloud saves uploaded to Epic Games
   for …", the conflict prompt "Which save should … use?" and Don't launch.
8. Game Settings: executable "Set by Epic Games…", launch parameters, compat tool and
   prefix (read-only), and the cloud toggle.
9. Settings › Stores Epic card:
   - Connected: account, games you own, the legendary tool row, license, install folder,
     cloud saves, Show in Library, Refresh games, Disconnect… with its inline confirm
     ("…GRID deletes your Epic Games login and legendary from this computer…");
   - Login expired;
   - Not connected (tool row "Installs when you connect · <size>").
10. Login window: "Sign in to Epic Games — GRID · 🔒 www.epicgames.com", the network-error
    strip, a failed sign-in card with Try again, and cancel being silent.
11. Downloads rows:
    - "Downloading 45% · 41.2 MB/s", "Verifying files 45%", "Downloading 40% · checking its
      SHA-256" (tool);
    - "Installed · 24.0 GB", "Updated to …", "Verified · N files repaired";
    - failed: "Failed · legendary stopped: Epic's download server didn't answer" with Retry
      and Dismiss;
    - the strip "… · Epic Games · Downloading …".
12. Notification cards: connected, "Epic Games: N games found · M to install", legendary
    installing / installed / failed, queued, installed · Play, updated, verify result,
    update available · Show, cloud cards, uninstalled ("Its cloud saves stay in your Epic
    Games account."), disconnected, signed-out install · Sign in again, missing tool ·
    Install legendary.

For E, from `tvstores.js` / TVStores: items 1–4 as TV panels and folders; the game page
(buttons, left column with the backup refusal and Not supported A-explanation, LB/RB tabs
with the Cloud saves rows and confirms); the install page (no Version rows); the game
settings page; the Settings › Stores Epic page (connected, expired, not connected,
Disconnect confirm); signing in over TV mode (keyboard, Y, B); Downloads rows and the
A-list; the prototype menu states "Epic Games: loading owned games / legendary not
installed / legendary install failed".

## Spikes (run first, in the container: `scripts/ctr.sh run …`; no real Epic account)

1. **legendary asset.**
   - Run `python3 legendary_linux_x64 --help`, `auth --help`, `launch --help` and
     `sync-saves --help` from a scratch config dir.
   - Confirm the flags above, the Python 3.10 floor, where the vendored `.so` lands with
     `XDG_CACHE_HOME`, and that `config.ini`'s `disable_update_notice` is honoured.
   - Run `status --json --offline` signed out.
2. **Stdin login without a network.**
   - Run `auth --disable-webview` with `BROWSER=true`, `HTTPS_PROXY=http://127.0.0.1:9`,
     and a fake code on stdin.
   - Check that it reads stdin (no prompt left waiting), that no browser opens, that the
     code appears nowhere in its output, and which stderr line marks the failure.
   - If stdin can't work, fall back to `--code` on argv, as gogdl's `auth --code` does:
     `ToolCommand`'s Debug already hides args.
3. **Tauri login window with Epic's page** (manual, by the user, if a real login is
   wanted; otherwise structural only).
   - Check that the EGL user agent through `WebviewWindowBuilder::user_agent` gets the
     `localhost/launcher/authorized?code=` navigation, and that WebKitGTK reports it to
     `on_navigation`.
   - Check how third-party sign-in pop-ups behave (`on_new_window` or `window.open` in the
     same view).
   - Check hCaptcha in an ephemeral context.
   - Fallback if the user agent stops working: on the `/id/api/redirect` JSON page, run a
     GRID script with `webview.eval` that turns `authorizationCode` into a navigation to a
     sentinel URL that `on_navigation` catches and refuses.
4. **Art hosts.** From recorded `list --json` shapes (legendary test data / Heroic issues),
   confirm the keyImages hosts beyond `cdn1.epicgames.com` and `cdn2.unrealengine.com`.

## Spike results (2026-11-13)

All six downloaded assets (and `legendary_macOS.zip`) matched the SHA-256s in Research.
The Linux x86_64 zipapp was copied to a fresh folder and run only inside the build
container (`scripts/ctr.sh run`; Ubuntu 24.04, Python 3.12.3), with a scratch
`LEGENDARY_CONFIG_PATH`, `XDG_CACHE_HOME` and `HOME`, `BROWSER=true`, and every request
sent to a dead proxy (`HTTPS_PROXY=http://127.0.0.1:9`). No Epic account and no network
login were used.

### Spike 1 — the asset, offline

- `--version` prints `legendary version "0.21.1", codename "Lowlife"` (exit 0). The
  wheel's METADATA says `Requires-Python: >=3.10`, so the Python 3.10 floor stands. The
  zipapp also carries a mypyc `.so` built for CPython 3.14 (charset_normalizer); on 3.12 it
  falls back to pure Python and runs.
- The `--help` output confirms every flag in the table above:
  - `auth`: `--import --code --token --sid --delete --disable-webview`;
  - `launch`: `--offline --skip-version-check --json --dry-run --wrapper --wine
    --wine-prefix --no-wine --override-exe --language --override-username`;
  - `sync-saves`: `--skip-upload --skip-download --force-upload --force-download
    --save-path --disable-filters`;
  - `install` (aliases `download`, `update`, `repair`): `--base-path --game-folder
    --max-shared-memory --skip-sdl --skip-dlcs --with-dlcs --platform --repair
    --update-only --ignore-free-space --save-path`;
  - `list`: `--platform --include-ue -T --json --force-refresh`; `list-installed`:
    `--check-updates --json --show-dirs`; `info`: `--offline --json --platform`;
    `status`: `--offline --json`; `uninstall`: `--keep-files --skip-uninstaller`;
    `list-saves [<App Name>]`.
- `LEGENDARY_CONFIG_PATH` is used as is. The first command writes `config.ini`,
  `aliases.json`, `manifests/`, `metadata/` and `tmp/` there (`version.json` after an
  update check). **Every folder is 0755 and every file 0644**, so GRID must make the tree
  private after each run.
- The vendored native modules unpack on the first run of any command (even `--help`) into
  `$XDG_CACHE_HOME/legendary/vendored/Cryptodome/` (42 `.so` files), with
  `vendored.lock` beside it.
- The first run writes `config.ini` with `disable_update_check = false` and
  `disable_update_notice = true`. The notice default is `true` only for the Linux zipapp:
  it is `false` on the Windows and macOS PyInstaller builds (`is_windows_mac_or_pyi()`),
  so GRID's first-run `disable_update_notice = true` still matters there. The notice is
  also off whenever `--json` is used. `auth` forces it on, but it goes to stdout, which
  GRID does not read for `auth`. The flag itself could not be exercised offline: the
  notice needs a successful update check.
- Signed out, `status --json --offline` prints
  `{"account": "<not logged in>", "games_available": 0, "games_installed": 0,
  "egl_sync_enabled": false, "config_directory": "…"}` (exit 0). Without `--offline` it
  runs the update check first; with no network that is an uncaught
  `requests.exceptions.ProxyError` traceback and exit 1. So the signed-out check always
  uses `--offline`.
- `list-installed --json` prints `[]` (exit 0). `info <unknown> --json` prints
  `{"game": null, "install": null, "manifest": null}` (exit 0, no login needed).
- **Not logged in:** `list`, `list-saves`, `install`, `repair` and `sync-saves` die with
  an uncaught `ValueError: No saved credentials` traceback, exit 1, and no `ERROR:` line.
- **Not installed:** `launch` logs `[cli] ERROR: Game X is not currently installed!` and
  exits 1. `uninstall` logs `[cli] ERROR: Game X not installed, cannot uninstall!` and
  exits **0**.
- With no `LANG`, every run logs `[Core] WARNING: Getting locale failed: …, falling back
  to using en-US.` This is harmless and not a signal.
- From the 0.21.1 source (read, not run):
  - an install's space failure goes to **stdout**
    (` ! Failure: Not enough available disk space! …`), followed by
    `[cli] CRITICAL: Installation cannot proceed, exiting.` on stderr, exit 1;
  - a `MemoryError` from the download manager is uncaught (a traceback);
  - the verify step logs one `ERROR` line per bad file (`File does not match hash: …`,
    `File is missing: …`) before its result line;
  - the download manager's logger is `DLManager`.

### Spike 2 — stdin login, no network

- **stdin works; the `--code` fallback is not used.** With `auth --disable-webview` and
  `BROWSER=true`, legendary prints its prompt on stdout ("Please login via the epic web
  login!", the legendary.gl URL, then `Please enter the "authorizationCode" value from
  the JSON response: `) and reads one line. A bare code, the JSON blob, and a code with
  no trailing newline (stdin then closed) are all accepted. No browser opens. The zipapp
  has no pywebview, so it takes the stdin path even without `--disable-webview`.
- A fake code with Epic unreachable gives two lines, then **exit 0**:
  `[Core] ERROR: Logging in failed with ProxyError(…)` (or
  `ConnectionError(… NameResolutionError …)` for a host that doesn't resolve), then
  `[cli] ERROR: Login attempt failed, please see log for details.` No `user.json` is
  written. In every case the code appeared in neither stream (checked with grep).
- Empty stdin (EOF) gives an `EOFError` traceback and exit 1.
- **Before it reads stdin**, `auth` runs `check_for_updates(force=True)` with no error
  handling. With api.legendary.gl unreachable it dies with a
  `requests.exceptions.ProxyError` traceback, exit 1, and stdin is never read. So:
  - a login needs api.legendary.gl;
  - the runner must not fail when the tool closes stdin unread;
  - package A treats a non-zero exit, `OtherError` or `Network` as a failed login, not
    only `LoginFailed`.
- Method for cases 2b–2f: a wrapper script ran the zipapp with
  `LGDAPI.get_version_information` stubbed (and, for the DNS case, `EPCAPI._oauth_host`
  pointed at an unresolvable host). legendary's own `auth` code ran unchanged.
- `auth --delete` while signed out logs `[cli] INFO: User data deleted.` (exit 0).
- From the source: a code that the real Epic refuses logs
  `[EPCAPI] ERROR: Login to EGS API failed with errorCode: …`, then
  `[Core] ERROR: Logging in failed with InvalidCredentialsError(…)`, then the
  `Login attempt failed` line. None of these lines carries the code (it goes in the POST
  body). A success logs `[cli] INFO: Successfully logged in as "<name>"`.

### Spike 3 — not run

The Tauri login window with Epic's real page needs a manual check by the user with a real
account. Package C keeps the structural rule (the finish URL is refused, and so is any
other navigation to localhost).

### Spike 4 — art hosts

- One GET of Epic's public promotions feed (no credentials) found these keyImages hosts:
  `cdn1.epicgames.com` (115 images) and `cdn2.unrealengine.com` (36). Video thumbnails
  are on `media-cdn.epicgames.com`.
- Video entries (`featuredMedia`, `heroCarouselVideo`) use a non-http
  `com.epicgames.video://` scheme. The owned-list parser must skip them, and the art
  check refuses them.
- Image types seen: `OfferImageWide`, `OfferImageTall`, `Thumbnail`,
  `DieselStoreFrontWide`/`Tall`, `CodeRedemption_340x440`, `GalleryImage`.
- A HEAD of one `cdn1` and one `cdn2` image both answered 200 `image/jpeg` with no
  credentials.
- **Rule:** https on `epicgames.com` and `unrealengine.com` and their subdomains. This
  covers all three hosts. The library's `list --json` keyImages could not be seen without
  an account; they come from the same catalog service, and the rule admits any
  subdomain.

## Contract deviations — Step 0

Where the skeleton differs from, or adds to, the Step 0 block above:

- **grid-core runner.**
  - `ToolSignal::Network` is added. A network failure is either a `requests.exceptions.*`
    traceback line or `Logging in failed with ConnectionError/ProxyError/…`
    (`HTTP request for login failed` too). Package B's offline launch retry keys on it.
  - `ToolCommand.private_tree: Option<PathBuf>`: the runner itself makes that folder
    private (`make_tree_private`) when a run ends, however it ends, so no caller can
    forget it. Epic commands set it to `<data>/stores/legendary`.
  - `ToolExit::has(&signal)` is added. So is `ToolExit::read_stdout(parse)`: `runner.rs`
    is an allowed `expose_secret` file, so the Epic code (except `launch.rs`) reads stdout
    through it.
  - `LineParser::tool()` names the tool in errors ("legendary stopped: …").
  - With `LineParser::Legendary`, the signal matcher reads **stdout's lines as well as
    stderr's**, because the space failure is on stdout. Each signal is kept once, in
    arrival order. A stdout line over 8 KiB (a JSON answer) is skipped for matching; the
    whole stdout is still kept. Specific signals win: a caller checks them before
    `OtherError`, which also comes for any traceback and for any other `ERROR`/`CRITICAL`
    line (the per-file verify lines excepted).
  - Writing stdin tolerates a tool that exits first (spike 2a).
- **Progress.** legendary's counts are chunks or files, so `parse_legendary_line` reports
  a **fraction**: `done` out of `LEGENDARY_PROGRESS_TOTAL` (10 000), in hundredths of a
  percent. The job turns it into bytes with `info`'s `download_size`, or with the
  `DownloadSize` signal (`Download size is 0, …` gives `DownloadSize(0)`). The
  `Downloaded` line is not used. Verify progress gives a `Verifying` fraction with its
  speed. `ProgressMerger` also keeps a non-zero speed from a progress report.
- `tools::python_problem(tool, found)` takes the tool, so it says "legendary needs
  Python 3.10…".
- `LEGENDARY_PIN` is filled with all six assets (Linux `Python3`, Windows and macOS
  `Direct`).
- `EpicConfig` is not written while it is default (`skip_serializing_if`), so a GOG-only
  `config.toml` keeps its shape.
- `is_store_art_url(url, extra_origins)`: the GOG app passes its gamesdb origin, the Epic
  app passes `EpicEndpoints.art_origins` (empty for real Epic, the mock in e2e).
- `stores::epic`:
  - adds `EpicUpdate { app_name, version }` and the pure `epic::updates()` (implemented:
    `build_version` ≠ `meta.version`, and neither blank);
  - adds `EpicService::endpoints()` and `tool_dir()`, and the constants
    `EPIC_LOGIN_USER_AGENT` and `EPIC_CODE_PARAM`;
  - `EpicInstalled` deserializes from `list-installed --json` (unknown keys ignored, a
    `null` read as the default; `save_path` is a `String`);
  - the Step 0 `disconnect` only removes the tool folder;
  - every other body returns `not implemented`, and its jobs fail with it.
- `epic_row`:
  - `native_working_dir` is the executable's folder, relative (legendary's own
    `working_directory`);
  - `filesize_bytes` is legendary's `install_size`;
  - `first_release_date` is `EpicGame.year`.
- **Launch.**
  - `StoreLaunchPrep` lives in `launch::native` and is re-exported as
    `launch::StoreLaunchPrep`.
  - An empty `executable` or `working_dir` falls back to the row.
  - The hook is asked only for store rows (`game.is_store()`); `Ok(None)` launches the
    row as it is.
- **App backend trait (`stores_service::StoreBackend`).**
  - Errors are `String`: GRID's words, as the commands answer.
  - The methods that load the owned list take `self: Arc<Self>` and return a `'static`
    future: `install_options`, `install_job`, `dlc_job`.
  - The background starters are named `run_login_check`, `run_finish_login`,
    `run_install_tool` and `run_refresh`, so that they don't shadow `GogStore`'s own
    `&Arc<Self>` methods.
  - `library(updates)` takes store id → version. `UpdateService::store_versions(kind)`
    gives GOG's, and nothing for Epic until package C. `forget_store_update` and
    `clear_store_updates` take the store kind and only touch GOG's set.
  - The trait also carries `kind`, `status`, `load`, `observe_downloads`,
    `ensure_can_install`, `game_settings`, `apply_game_settings` (moved from
    `commands/stores.rs` into `gog.rs`), `windows_version`, `cloud_enabled` and
    `is_art_url`.
  - `StoreShared` holds the emitter, the registry, the art client and the backends
    (weakly), so any backend's `changed()` emits every store's status. Statuses come in
    the frontend's `STORE_ORDER` (epic, gog).
  - `JobCtx` is the neutral job context, with `.gog()` and `.epic()`.
  - `default_install_dir(config, configured, name)` is shared.
- **`store_auth`.**
  - `LoginWindowSpec { login: LoginSpec, source, data_dir, tv }`.
  - `GOG_LOGIN_SUCCESS_PATH` is public, and the GOG finish URL is `embed.join(it)`.
  - `is_login_redirect(url, finish)` matches the finish path exactly (a trailing `/`
    aside).
  - The window applies `LoginSpec.user_agent`.
- **`commands/stores.rs`.** `store_kind`, `store_key` and `store_row` work for any
  supported store (`UNSUPPORTED_STORE` otherwise). `launch_game_key` syncs through
  `stores.backend(kind)` for any store key.
- **Epic stub (`stores_service/epic.rs`).**
  - The status is always "not connected", with the legendary pin, and `in_rail` only
    with installed rows.
  - These answer `not implemented`: `begin_login`, `ensure_can_install`, `disconnect`,
    install options and install jobs, DLC, cloud state and sync.
  - `sync_before_launch` is `Ok`. Game Settings are real (every Epic game takes a compat
    tool). The login spec is real.
  - Its `StoreLaunchHook`, installed in `lib.rs`, refuses an Epic row with "Couldn't get
    a launch ticket from Epic Games: not implemented".
  - `GRID_LAUNCHER_E2E_EPIC_BASE` (feature `e2e`) feeds `EpicEndpoints::e2e`.
- **TypeScript.** Besides the three `api.ts` types, `gamestores/details.ts` leaves out
  the cloud file count when it is `null` (a one-line type fix, so that the check passes).

## Contract deviations — package A

No frozen file changed. Package A filled `stores/epic/mod.rs` and added
`stores/epic/{auth,list,install,update}.rs` and `crates/grid-core/tests/epic_store.rs`.
`tools`, `progress`, `paths` and `art` needed nothing beyond Step 0.

- **Additions to `EpicService`** (for package C):
  - `tool_installed()` and `install_tool(progress)`. The second is the pinned download
    with its SHA-256 check (a mismatch deletes the file: `Refused`), then the first-run
    `config.ini`.
  - `tool(args)` builds every legendary command: env `LEGENDARY_CONFIG_PATH`,
    `BROWSER=true` and (off Windows) `XDG_CACHE_HOME=<tool dir>/cache`;
    `LineParser::Legendary`; `private_tree`. It writes `config.ini`
    (`disable_update_notice = true`) before the first run. `tool_env()` is public.
  - `saved_account()` is `status --json --offline` (no network). Use it at startup when
    Epic can't be reached.
  - `set_owned(games)` seeds the kept owned list from the cache, and `game(app)` reads
    it. The jobs and `install_options` read titles, DLC, the third-party refusal and the
    expected build from it.
  - `with_tool_override(program)` (`#[doc(hidden)]`) runs a fake legendary in the tests.
  - `list::{read,write,remove}_owned_cache` and `OwnedCache { refreshed_at, games }`
    handle `<data>/stores/cache/epic-owned.json`. `EpicGame` and `EpicGameDlc` are
    `Serialize`/`Deserialize` for it. The caller writes the cache after `owned()`.
    `disconnect()` removes it.
- **Login.** Signed in means exit 0, no `Network`/`LoginFailed`/`OtherError` signal,
  and `user.json` present. The account then comes from `status --json --offline`.
  `Network` gives `StoreError::Network("Epic Games")` and `LoginFailed` gives "Epic Games
  didn't accept the sign-in. Try again."
- **Account.** It returns `None` without running legendary when there is no `user.json`.
  Otherwise it runs `status --json` (online). `NotLoggedIn`/`LoginFailed` give
  `LoginExpired`, and `Network` gives `Network`.
- **Owned list.** It keeps items that have a `Windows`/`Win32` asset, **and third-party
  titles, which have no asset** (`-T`). Without that rule, Not supported could never
  show.
  - It drops items whose categories are only `mods`/`applications`, mods without
    `games`, DLC listed on their own (`mainGameItem`), and DLC with no Windows asset.
  - The cover is portrait first (`DieselGameBoxTall`, `OfferImageTall`,
    `DieselStoreFrontTall`, then `DieselGameBox`, `OfferImageWide`), because the Library
    cards are portrait, like GOG's `vertical_cover`.
  - Screenshots are `Screenshot`/`GalleryImage`, else one wide image.
  - The year is the first plausible `releaseInfo` date.
  - Third-party names: `origin`/`the ea app` give "the EA app", and
    `ubisoftconnect`/`uplay` give "Ubisoft Connect". Any other value is shown as it is.
- **Install options.** The DLC come from `info`'s installable `owned_dlc`, then from the
  owned list, each once. A DLC whose `info` fails is offered with size 0. A third-party
  title is refused.
- **Jobs.**
  - An update and a repair also run `update`/`repair` for each installed DLC (from
    `meta.dlc`). This is because `--skip-dlcs`, which is always present, leaves the DLC
    alone. A DLC transfer that fails inside an install, update or repair does not fail
    the job. It becomes a warning line ("Couldn't install Night Route: …"), and only the
    DLC that `list-installed` shows go into `meta.dlc`.
  - Progress is in bytes from `info`'s download sizes for an install or a DLC. For an
    update or a repair, whose sizes are unknown up front, it is a fraction of the whole
    job (N × 10 000). The repair's `download_phase()` is `Verifying`.
  - The success rule: no failure signal on the base transfer (exit code included), and
    the app present in `list-installed` with a non-empty version. After an update, the
    version must be the owned `build_version`, or at least differ from the old one,
    unless legendary printed `DownloadSize(0)` (already up to date). A cached build that
    is stale must not fail a real update, so the check is not strictly "== expected".
  - `step_failure` maps the signals to GRID's words in this order: `NotLoggedIn` or
    `LoginFailed` give `LoginExpired`; `ThirdPartyManaged`; "there isn't enough disk
    space"; "Epic's download server didn't answer"; "it ran out of memory"; "the game
    isn't installed"; the exit code; "it reported an error".
- **Uninstall and DLC removal.** A missing legendary, or `NotInstalled` (legendary
  forgot the game, for example after a Disconnect), still removes the files. Any other
  failure stops before a file is touched.

**Left for integration:** the repair result ("2 files repaired") is what the job's
`finalize` returns, and the queue stores that as the completed entry's `error`. The
frontend reads `DownloadEntry.detail` (`Verified · <detail>`). One line in
`library/mod.rs` (store finalize: a completed `StoreOp::Repair` message goes to
`detail`) closes this. GOG's repair would also use it.

## Contract deviations — package B

Package B (launch and cloud saves) lives in `stores/epic/{launch,cloud}.rs` and
`tests/epic_cloud.rs`. No frozen file changed. `launch/{mod,native}.rs` needed nothing
beyond Step 0: the hook, the redacted positions and the `<redacted>` early-exit argv were
already there, and the session record holds no argv.

- **`epic/mod.rs`**, kept to what merges trivially with A: `pub mod cloud; pub mod
  launch;` and the bodies of `launch_prep`, `cloud_state` and `cloud_sync` (one-line
  delegations).
- **Its own legendary helper.** `launch.rs` has `legendary_command`, `run` and
  `tool_error`. They hold the env (`LEGENDARY_CONFIG_PATH`, `XDG_CACHE_HOME=<tool>/cache`,
  `BROWSER=true`), `LineParser::Legendary` and `private_tree`. `cloud.rs` shares them.
  **Integration:** swap them for A's helper, and its test override, when both are merged.
- **Test hook.** `epic::launch::set_tool_override_for_tests(data_dir, program)` is
  `#[doc(hidden)]` and keyed by data folder. The integration test needs it because it
  cannot reach a `cfg(test)` builder, and nothing in the app sets it. If A adds a public
  override, use A's and delete this one.
- **`launch_ticket_error(&StoreError) -> LaunchError`** gives GRID's words, "Couldn't get
  a launch ticket from Epic Games: …":
  - login expired: "your Epic Games login expired. Sign in again.";
  - network: "Epic Games didn't answer. Check your connection and try again.";
  - anything else: the error's own text.

  Package C's `StoreLaunchHook` should map `launch_prep` errors through it.
- **Launch prep details.**
  - legendary puts the installed launch parameters first in `game_parameters`, but the row
    already carries them. GRID drops that leading copy. It compares against legendary's
    `installed.json` value, or the row's when that is missing, so a parameter the user
    edited is not doubled.
  - The executable and working folder are legendary's. They are left empty, so the row's
    are used, when the user pinned an executable or legendary's path climbs out of the
    game folder.
  - legendary's `environment` is passed on without `WINEPREFIX`, `CX_BOTTLE`,
    `STEAM_COMPAT_DATA_PATH` and `PROTONPATH`.
  - The offline retry happens only on `StoreError::Network`. That comes from the
    `Network` signal, which wins over the `LoginFailed` line that follows it.
- **Save path.**
  - `resolve_save_path(folder, install_dir, epic_id, prefix, host, prefix_user)` takes
    `SaveHost::{Windows{profile, local_appdata, roaming, documents}, Wine}`.
  - Besides the plan's tokens it accepts `{UserProfile}` (legendary has it).
  - `..` may climb, because Epic writes `{AppData}/../LocalLow/…`, but never out of the
    user folder (or the game folder for `{InstallDir}`).
  - `{EpicID}` is the `account_id` from legendary's `user.json`. Only that field is read.
  - The Wine user comes from `save_prefix_user`:
    - an existing prefix says which user it has (`cloud::native::prefix_user`);
    - a prefix not made yet gets `steamuser` only under Proton (umu);
    - under plain Wine it stays unknown, so the path is `None`.
  - If GRID can't resolve the path, it uses `meta.save_path` from the last sync.
  - The row's compat kind uses the config's default only when the row names no tool
    (`Config::default_path()`).
- **The prefix-missing rule**, as implemented: with no save path, an Auto sync never
  downloads and returns the current state. An explicit "use the cloud save" is refused
  with "Start the game once so its Windows folder exists; then GRID can download its cloud
  saves."
- **Sync time.** `last_sync` is the later of now (after the run) and the newest local file,
  so the save just written never counts as a later change. A conflict's `cloud_files` and
  `cloud_bytes` are `None`.
- Local save folder copies go to `<data>/stores/save-backups/epic/<app>/<epoch-ms>/`.
  The newest five are kept.

## Contract deviations — package C

No frozen file changed. Where package C differs from, or adds to, the plan:

- **`EpicStore` has a seam.** `EpicStore::new(api: Arc<dyn EpicApi>, data_dir, config_path,
  shared)`; `RealEpic` forwards to `EpicService` and `tools::install_tool`, and the unit
  tests fake it (as `GogApi`). `EpicStore::real(...)` is unchanged, so `stores_service/mod.rs`
  is untouched.
- **legendary installs before the code is redeemed.** legendary itself redeems the code
  (`auth`, the code on stdin), so `finish_login` installs legendary first when it is missing
  (with the "Installing legendary (3 MB) for Epic Games…" card), then signs in, then loads the
  owned list. GOG signs in first. A legendary that can't install fails the sign-in with its
  own words (`store-login` `failed`) and keeps the Try again card.
- **The owned cache is the app's.** `EpicStore` writes `<data>/stores/cache/epic-owned.json`
  (metadata only), as `GogStore` writes `gog-owned.json`. Package A's `owned()` should not
  write a cache of its own at that path.
- **Status and games.** `in_rail` is "connected or expired, or any installed game" (a
  not-connected Epic with nothing installed has no rail group). A game with no
  `CloudSaveFolder` (and no resolved save path) has the cloud column `none` and never syncs.
  Available games carry `info`'s sizes once the install dialog has asked for them.
  `not_supported` is the launcher's name ("the EA app"); the frontend words the sentence.
  An install of a third-party title is refused with Details' sentence ("Needs the EA app.
  GRID can't install …"). The install request's platform must be `windows` or empty.
- **Launch hook.** Signed out (or expired), a game with `can_run_offline` asks for
  `launch_prep(row, offline = true)`; any other game is refused with "Sign in to Epic Games
  to play <title>." (or the expired words). Signed in, a `StoreError::Network` from
  `launch_prep(row, false)` retries once with `offline = true` when the game may. So package
  B's `launch_prep` need not retry itself (a second retry inside it is harmless). Every other
  failure is `LaunchError::Validation("Couldn't get a launch ticket from Epic Games: …")`,
  and `LoginExpired` also flips the card.
- **Updates by store.** `UpdateService.store_updates` is keyed by `GameKey` (Integration
  item 4). A `StoreUpdateSource` trait (in `update_service.rs`, implemented there for
  `GogStore` and `EpicStore`) gives the check its store; `spawn_store_check` is generic and
  runs one check per store at a time. `forget_store_update` and `clear_store_updates` take
  the `StoresService` facade and touch only their store's keys. Epic's check is the pure
  `epic::updates` over the owned list (fetched once per run if needed). `lib.rs` starts
  both stores' checks after the startup login check.
- **Commands.** `store_update` and `store_repair` refuse while the game runs ("Quit <title>
  before updating it." / "… before verifying its files."), for every store.
- **Notices.** Epic has its own `stores_service::epic::notice` (the GOG module's words with
  "Epic Games" and "legendary"). A repair's card reads the finished row's detail: "Starlight
  Courier: 2 damaged files were downloaded again", or "Verified …'s files" when none was.
- **Login window (`store_auth.rs`).** Besides refusing the finish URL, `classify` refuses
  (silently, `Redirect::Refused`) any navigation to a loopback host (`localhost`,
  `*.localhost`, `127.0.0.0/8`, `::1`, the unspecified addresses, IPv4-mapped forms) unless
  it is the login page's own origin (the e2e mock). This applies to GOG too. A pop-up
  (`window.open`, `on_new_window`) is denied and its http(s) page opens in the same login
  window, under the same rules; a pop-up to the finish URL finishes the sign-in. Spike 3
  (a real Epic sign-in) is still the user's manual check.
- **E2E harness.**
  - The fake legendary prints the token on stderr for every command, and on stdout as a
    text line or a `fake_session` key inside every JSON object it prints (an empty JSON
    array, e.g. `list-installed` with nothing installed, has nowhere to carry it).
    Package A's and B's parsers must ignore unknown keys and non-matching stdout lines.
  - It writes its files 0644, as legendary does; `epic-secrets` checks every file under
    `stores/legendary/` is private afterwards (`ToolCommand.private_tree`, package A).
  - Marker files in its config dir: `.fake-expired`, `.fake-download-fail`,
    `.fake-launch-network` (see the file's header). Its cloud is
    `stores/legendary/fake-cloud/<app>/` (`files/`, `manifests.json`).
  - Third-party values in the fixtures are `Origin` and `UbisoftConnect` (lowercased:
    `origin`, `ubisoftconnect`).
  - `e2e/fixtures-epic/owned.json` holds the six games; sizes are in MB, not the
    prototype's GB, so installs fit in the container.
  - Exported for the specs: `E2E_EPIC_URL`, `E2E_FAKE_LEGENDARY`, `E2E_EPIC_CONFLICT_KEY`
    (`epic:Starlight`) and `E2E_EPIC_ARGV_FILE` (the compat-tool stand-ins' argv log).
  - `.gitignore` gains the GOG and Epic mock request logs.

## Package C outcome (2026-11-13)

- `scripts/ctr.sh gate`: PASS. New unit tests: `stores_service::epic` (17), `store_auth`
  (the localhost rule, loopback hosts, the sink's verdicts), `update_service` (stores keyed
  by `GameKey`).
- Harness tests (`npm run test:mock` in `e2e/`, node --test): `mock-epic.test.mjs` 4/4,
  `legendary.test.mjs` 11/11.
- `scripts/ctr.sh e2e gog gog-library gog-cloud tv-gog`: every group passes (11 + 4, 7, 3,
  8 + 6).
- `scripts/ctr.sh e2e epic` on the Step 0 stubs: `epic-login` 10 of 13 pass (not connected,
  the user agent, Cancel, ✕, the network error, the localhost refusal, the pop-up in the same
  window, a denied sign-in, Install legendary, Disconnect). Blocked on package A
  (`complete_login`, `owned`): "a code legendary refuses", "signs in" and "lists the owned
  games". `epic-secrets` is not reached (the group stops at the first failing spec); it needs
  A (sign-in, install) and B (`launch_prep`, cloud saves).
- Not run: `wincheck` (no `cfg(windows)`/`cfg(unix)` code touched).

## E2E plan

- **Mock Epic** (`mock-romm/mock-epic.mjs`, one base for login and art).
  - `/id/login` is a form posting to `/id/login`. That 302s to
    `<base>/launcher/authorized?code=FAKE-E2E-EPIC-CODE`.
    - User `denied`: `?error=access_denied`, no code.
    - User `refused`: a code the fake legendary refuses.
  - `/launcher/authorized` must **never be served**: `epic-login` checks the request log.
  - `/art/<id>.jpg` serves the art.
  - `GRID_LAUNCHER_E2E_EPIC_BASE` (feature `e2e`) gives `EpicEndpoints::e2e`. Every other
    group gets the dead loopback address.
- **Fake legendary** (`fake-tools/legendary.mjs`, run directly through
  `GRID_LAUNCHER_E2E_TOOL_LEGENDARY`).
  - It implements:
    - `auth --disable-webview` (stdin), `auth --delete`;
    - `status --json`, `list --json -T`, `list-installed --json`, `info --json`;
    - `install`/`update`/`repair` with real-format progress on stderr;
    - `uninstall`;
    - `launch --json`, whose `egl_parameters` carry `-AUTH_PASSWORD=FAKE-STORE-TOKEN-not-real`
      and whose game is a script that writes its argv to a file;
    - `list-saves` and `sync-saves`.
  - Its "cloud" is a folder under its config dir.
  - **Every command prints `FAKE-STORE-TOKEN-not-real` on stdout and stderr**, and
    `user.json` holds it.
- **Seed** (`seed/epic-seed.mjs`): signed in as SamPlays; Starlight Courier and Vector
  Bloom installed; the owned list is the prototype's six games. A profile is used without
  a server. With `--cloud-conflict`, Starlight Courier's cloud save is newer than its
  `last_sync`, and the local save changed too.
- **Groups in `scripts/e2e.sh`:**

  | group | specs | checks |
  |---|---|---|
  | `epic` | `epic-login`, `epic-secrets` | sign-in through the window (code caught, `/launcher/authorized` never served, denied, refused, cancel); legendary install after connect; owned list. Secrets: after sign-in, install, launch and a cloud sync, no GRID log, forwarded line, IPC payload (recorded events plus `invoke` answers), session record or file in the data folder outside `stores/legendary/` holds the fake token; the game's argv file does hold it (the exchange code reached the game); the early-exit text shows `<redacted>` |
  | `epic-library` | `epic-library` | rail group and counts, panels, Not supported, install with a DLC, update, verify, uninstall, Settings card and Disconnect confirm |
  | `epic-cloud` | `epic-cloud` (mock flag `--cloud-conflict`) | synced line, Sync now, the conflict prompt at launch, both resolutions, the save-backups copy before a download |
  | `tv-epic` | `tv-epic` | TV folder, panels, game page tabs (go to a tab directly; LB/RB wrap), install page, Settings › Stores › Epic, sign-in following the login window as WebDriver's active window |

- **Existing specs to update** (lead, at integration):
  - `gog-library`, `tv-gog-a/b`, `tv-settings`, `tv-shell` and `tv-layout` wherever they
    count rail groups, store cards or Settings › Stores rows: Epic now shows when it is
    connected.
  - Seeds that don't connect Epic must leave it hidden. Check that a not-connected Epic
    with no installed games has no rail group.

## Gate requirements

- The gate (CLAUDE.md), including `check_secret_hygiene.sh`, with the two new
  `expose_secret` files only.
- `scripts/ctr.sh wincheck`. The runner's stdin and `make_tree_private` add
  `cfg(unix)`/`cfg(windows)` code.
- The **full** e2e suite on the merged tree, not only the Epic groups. Step 0 moves the
  GOG service, so every `gog*` and `tv-gog` group is a regression check.
- Fixtures use `FAKE-STORE-TOKEN-not-real` only.
- Each UI package ticks every item of its parity inventory.

## Integration list (expected, from the Phase 1 lessons)

1. **Notices once each.** The desktop's `StoreLayer` and TV's `storeEvents.ts` handle Epic
   too. The backend `store-notice` carries only background events.
2. **Art.** `ensure_image`'s store branch accepts the Epic hosts (`is_store_art_url` with
   the e2e origin). TV (`TvStoreArt`) and the dialogs (`StoreCoverSm`) show Epic covers.
3. **Rail.** `library/rail.ts` lists no "Epic Games" platform row.
4. **Updates.** `UpdateService.store_updates` is keyed by `GameKey` (not by a bare GOG id),
   so the Library's Updates view counts Epic updates.
5. **Launch.** The Epic hook is installed in `lib.rs`. A Play from TV mode and from the
   desktop both go through `launch_game_key`. The session's `emulator_name` is the compat
   tool.
6. **Drive views.** Epic games never show Select, Backup to drive… or drive counts.
   `backup::local::REASON_STORE` still refuses them.
7. **TV specs** that list Settings panes or count stores (see the E2E plan).

## Package D outcome (desktop UI)

- **D1** (`feat(stores): D1 …`): `gamestores/marks.ts` has `ENABLED_STORES = ['epic', 'gog']`
  and `storeCaps(source)` with `hasTool`, `hasCloud`, `canBackup` (GOG), `hasInstaller`
  (GOG), `hasVersions` (GOG) and `hasLanguages` (GOG), plus `installedByTool(game)`
  (`origin` is the store's tool name, so legendary installs can Verify), `storeLauncher`,
  `storeOfPlatform` ("Epic Games" → `epic` for Downloads rows) and `storesLede()`.
  `hasStoreCloud` stays as an alias of `hasCloud`. Every `source === 'gog'` in the pure
  modules that meant a capability now reads it. TV (`tv/stores/*`, package E) imports
  these and still has its own `source === 'gog'` checks to convert.
- **Settings › Stores lede** names the stores this build shows: "GRID installs GOG and
  Epic games with the open-source tools Heroic Games Launcher uses; their logins stay in
  those tools' files in GRID's data folder." (the prototype's line, without Amazon and
  Steam).
- **Not connected account line**: "Not signed in · N games installed here" stays a
  store-with-offline-installers line (GOG), as in the prototype; Epic says "Not signed
  in".
- **Install dialog**: Version and Language show only for a store with versions /
  languages, whatever the backend sends, so Epic never shows them.
- **fakeApi / devMock**: both stores; the prototype's e1–e6 (app names are made up:
  `Starlight`, `VectorBloom`, `GlimmerDeep` with one DLC, `IronOrchard`,
  `NorthernStatic`, `RustbeltRally`), SamPlays, legendary 0.21.1 (3.1 MB), per-store
  scenarios (`scenario(name, 'epic')`), and new scenarios `ownedError`, `update`,
  `cloudConflict` (Epic's conflict has `cloud_files: null`).
- **E2E** (`helpers/epic.ts`, `specs/epic-library.spec.ts`, `specs/epic-cloud.spec.ts`):
  written against this plan; they find covers by title, never by app name, and accept
  either a connect form or a profile already in the Library. Pending integration with
  package C's mock, fake legendary, seed and `scripts/e2e.sh` groups. For integration:
  - `epic-cloud` needs the seed's conflict on **Starlight Courier** and the save-backups
    copy at `<data>/stores/save-backups/epic/<app>/<epoch-ms>/`. It covers the prompt's
    Don't launch and Use the cloud save; "Keep this computer's save" is covered by the
    GOG spec and vitest, because one seeded conflict allows one resolution.
  - `epic-library`'s update step runs only if the seed gives an installed game a newer
    build.
- **Parity** (dev mock beside the prototype, screenshots in the worktree's
  `ctr-out/parity-epic/`, not committed): rail group and counts, Installed and Available
  headers, Not supported covers and Details, installed Details, install dialog (no
  Version, no Language, DLC), every Available panel (loading, empty, all installed,
  expired, not connected, legendary missing / installing / failed), cloud conflict tab,
  Settings card (connected, Disconnect confirm, expired, not connected), the failed
  Downloads row and the strip. The Library's default Sort ("Title") is the shipped app's,
  not an Epic difference. The login window (item 10) and backend card texts (item 12)
  belong to package C.

## Questions for the user

None. Two choices were taken here and can be reversed on request:
- legendary keeps its update check to `api.legendary.gl`, as in Heroic, because it carries
  Epic client changes. Only the update notice is off.
- The Epic install folder defaults to `<library>/Epic Games`, the store's platform name,
  as GOG uses `<library>/GOG`.
