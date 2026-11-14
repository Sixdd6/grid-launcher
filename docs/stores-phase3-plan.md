# Game stores — Phase 3 (Amazon Games / Prime Gaming) work plan

Status: planned 2026-11-13, not started. Read it with:
- `docs/external-stores-plan.md` for the user decisions;
- `docs/stores-phase2-plan.md`, the template. Its spike results, contract deviations and
  integration outcome are the lessons this plan builds on;
- the approved design cards `docs/design/system/components/GameStores/README.md` and
  `TVStores/README.md`, and their sources `docs/design/prototype/{stores,tvstores,stores_bk,tvstores_bk}.js`.
  The design is the spec.

What Phase 2 left on main (2eb9d87):
- `StoreBackend` and `stores_service/{mod,gog,epic}.rs`, with `StoreShared` and `JobCtx`;
- `LoginSpec` / `LoginWindowSpec` and `classify` with the loopback rule;
- `LineParser`, `ToolSignal`, `ToolExit::read_stdout` and `ToolCommand.{stdin,private_tree}`;
- `make_tree_private` and `is_store_art_url(url, extra_origins)`;
- `StoreLaunchPrep` and `StoreLaunchHook`;
- `StoreUpdateSource`, with updates keyed by `GameKey`;
- `StoreJobRunner::detail()`;
- `marks.ts` `storeCaps`. Its Amazon row already reads hasTool on, everything else off,
  and the no-backup sentence is already there;
- the e2e pattern: `mock-epic.mjs`, `fake-tools/legendary.mjs` and `seed/epic-seed.mjs`.

Already Amazon-aware on main:
- `StoreKind::Amazon`, "Amazon Games" and the `amazon:` key. A product id holds dots, and
  only the first `:` splits the key;
- `STORE_MARKS.amazon` and `TOOLS.amazon = nile`;
- `storeWords.loginHost('amazon')`;
- `backup::local::REASON_STORE` refuses Amazon rows, and the registry treats them as
  native.

What is still missing: `StoreTool::Nile`; any Amazon backend (`StoresService::backend(Amazon)`
is `None`); `'amazon'` in `ENABLED_STORES`; `'nile'` in the `ToolStatus.name` /
`StoreGame.origin` unions.

**In scope:**
- Amazon login (nile device registration);
- the owned list (Prime Gaming claims and Amazon Games purchases);
- install, update, verify and repair, uninstall and launch;
- every Amazon state in the design (parity inventory below).

**Out of scope:**
- cloud saves (Amazon has none in nile; `hasCloud` stays off);
- drive backups (user decision; Details says why);
- DLC (nile has no DLC model; an add-on entitlement is just another product);
- `nile import` of existing installs;
- running fuel.json `PostInstall` steps (Heroic runs them on Windows hosts only; see
  Question 2);
- macOS (GRID ships no macOS build);
- offers delivered as third-party keys (they are not Amazon entitlements and never reach
  nile's list).

## Research (2026-11-13)

Sources are pinned. **N/** is nile at tag v1.2.0, commit `1ed408ab51709e294034be7b1662288da840ff7f`:
`https://github.com/imLinguin/nile/blob/1ed408ab51709e294034be7b1662288da840ff7f/`.
**H/** is Heroic main at `89e06ad7b89cb7514ea3ce089ff1e2ca5e49dafa`:
`https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/blob/89e06ad7b89cb7514ea3ce089ff1e2ca5e49dafa/`.
No nile binary was run for this research.

### nile release

- **Pin `v1.2.0`** ("Robert Speedwagon"; `--version` prints `1.2.0 Robert Speedwagon`).
  - Published 2026-03-03. It is the latest stable release; the previous one is v1.1.2,
    from 2024-09-10.
  - License GPL-3.0 (`N/LICENSE.md`).
  - **Heroic pins the same `v1.2.0`** (`H/meta/downloadHelperBinaries.ts` L32, asset map
    L163-175) and does no hash check.
- Assets are under `https://github.com/imLinguin/nile/releases/download/v1.2.0/`. Each
  SHA-256 was computed from the downloaded file (never run) and matches GitHub's
  per-asset `digest`.

  | asset | os/arch | size | sha256 |
  |---|---|---|---|
  | nile_linux_x86_64 | linux/x86_64 | 11113264 | d73519514df9d52c50c6feec629373cc3a0d9504e087b6c7372a8ca8a95e7bfe |
  | nile_linux_arm64 | linux/aarch64 | 10847400 | 197859c629c47e4e4d7ed1d2a290b7bef1e778f891ff8ca42ba65dbb020ba751 |
  | nile_windows_x86_64.exe | windows/x86_64 | 12386046 | 6531790c59f78cea4a8743bf0582d5afda7fb887f5c143391d7339ad0f42ab88 |
  | nile_macOS_x86_64 | macos/x86_64 | 10002608 | 93ef4a5d29a378901569e3a854c289d356a0be3503700ebf7ee70586d401b0b0 |
  | nile_macOS_arm64 | macos/aarch64 | 8348560 | 6cc74eb19117c8c309b0b94657e1e267642a3b4840d7386a133dcf6f3bffeab0 |

  There is no Windows arm64 asset. Heroic maps none either, so that host gets "nile isn't
  available for this computer".
- **Every asset is a frozen PyInstaller `--onefile` binary** (Python 3.8 bundled;
  `N/.github/workflows/build.yaml`). None is a zipapp, so:
  - all five use `AssetLauncher::Direct`, and Linux needs no Python check;
  - each run unpacks into `$TMPDIR/_MEI*`;
  - the bootloader needs glibc ≥ 2.14, but the bundled libraries were built on Ubuntu
    22.04 and may need more. Spike 1 measures the floor.
  - Unreleased `main` has a zipapp (Python ≥ 3.11) and looks broken
    (`self_update.py` imports a deleted module). Do not track `main`.

### Login (Heroic's way, caught in Rust)

Heroic's flow is in `H/src/backend/storeManagers/nile/user.ts` and
`H/src/frontend/screens/WebView/index.tsx`; nile's side is `N/nile/api/authorization.py`.

1. `nile auth --login --non-interactive` prints one JSON line on stdout:
   `{"client_id","code_verifier","serial","url"}`.
   - `url` is `https://amazon.com/ap/signin?…`. Its query holds
     `openid.return_to=https://www.amazon.com`, `openid.assoc_handle=amzn_sonic_games_launcher`,
     `openid.oa2.response_type=code`, PKCE S256, `openid.oa2.client_id=device:<client_id>`
     and `marketPlaceId=ATVPDKIKX0DER` (US).
   - The values are generated locally, with no request to Amazon.
   - The first run of any command except `--version` also tries to download Amazon's
     FuelSDK into `<config>/SDK/`. A failure there only logs a warning.
2. Amazon redirects to
   `https://www.amazon.com/?openid.assoc_handle=amzn_sonic_games_launcher&…&openid.oa2.authorization_code=<code>`.
   - Heroic reads `openid.oa2.authorization_code` from every navigation.
   - nile's own web login matches the prefix
     `https://www.amazon.com/?openid.assoc_handle=amzn_sonic_games_launcher`.
3. `nile register --code <code> --code-verifier <v> --serial <s> --client-id <c>`
   registers a device with Amazon ("AGSLauncher for Windows", device type
   `A2UMVHOX7UP4V7`), then runs a library sync.
   - Success is only the stderr line `INFO [AUTH_MANAGER]:\t Succesfully registered a device`
     (Amazon's typo) and the token file.
   - On failure it prints `ERROR … Failed to register a device` and **prints the response
     body to stdout**.
   - **Exit code 0 either way.**

**GRID's way:**
- The window loads the `url` from nile. GRID first checks that url's host: `amazon.com`
  or `www.amazon.com` over https, or the e2e base.
- GRID catches the finish navigation in `on_navigation`, with `on_page_load` as the
  fallback. The finish is scheme `https`, host `www.amazon.com`, path `/`, **and the query
  holds `openid.assoc_handle=amzn_sonic_games_launcher`**. The code goes into a
  `SecretString`, and the navigation is refused.
  - Amazon's real home page has the same path, so the assoc_handle check is new in
    `LoginSpec`. Without it, a click on the Amazon logo would read as a failed sign-in.
- The code never crosses IPC. Neither do `client_id`, `code_verifier` or `serial`: they
  stay in a `SecretString` in the Amazon service between steps 1 and 3.

**The code goes on argv; stdin is not used:**
- nile's parser is plain argparse with `parse_known_args`. It reads no environment
  variable, and `register` has no stdin path.
- The only stdin path is the interactive `nile auth --login`. It prints the URL, waits
  for Enter, calls `webbrowser.open`, then reads the redirect URL from stdin. GRID would
  have to keep that process open, with two-way pipes, for the whole time the window is
  open, and stop the browser from opening (`BROWSER=` trick, untested).
- The argv path has a precedent: gogdl's `auth --code` (Phase 1).
- The values stay on argv for about a second, visible only to the same user's processes,
  which could already read nile's token file.
- `ToolCommand`'s `Debug` already hides every argument. No GRID log, error or IPC payload
  carries them.
- Spike 2 checks that nile itself never echoes them. It does log
  `Registerring a device. ID: <client_id>` on stderr, which the runner drops.

**Tokens:**
- `<config>/current_user.json` is `{"name": <given name>, "user_id"}`, in plain text.
- `<config>/<md5(user_id)>.enc` holds the whole register response: bearer
  access/refresh tokens, `mac_dms`, customer and device info. It is encrypted with AES
  keyed by `sha256(user_id)`. Since `user_id` sits in plain text next to it, this hides
  the tokens but does not protect them, so GRID treats the file as plain text (AGENTS.md
  store exception).
- nile writes every file with the default umask.
- `auth --logout` refreshes the token, POSTs `/auth/deregister`, and deletes its files
  **only if Amazon answers OK**. So Disconnect always deletes the folder itself
  afterwards.
- Each sign-in registers a new device in the user's Amazon account.

### Commands and outputs GRID uses

| Function | Detail |
|---|---|
| Config dir | `NILE_CONFIG_PATH=<data>/stores/nile`. nile **appends `/nile`**, so its files land in `<data>/stores/nile/nile/`: `current_user.json`, `<md5>.enc`, `library.json`, `installed.json`, `syncpoint.raw`, `manifests/<product id>.raw`, `SDK/Amazon Games Services/{Legacy,AmazonGamesSDK}/`. Precedence is `NILE_CONFIG_PATH`, then `XDG_CONFIG_HOME`, then platformdirs (`N/nile/constants.py` L18-20). The binary sits beside it (`<data>/stores/nile/nile_linux_x86_64`). |
| Streams | Logging goes to **stderr** as `LEVEL [Name]:\t message`. JSON goes to **stdout**. **The exit code is 0 for almost every failure.** The exceptions are `library sync` when signed out (exit 1, `ERROR [CLI]: User not logged in`), uncaught tracebacks, and `launch`, which returns the game's exit code. Any `ERROR [` line is a failure. **`Download complete` is printed even after `Not enough space available` or `Unable to load manifest`**, so it proves nothing. |
| Account | `auth --status` gives `{"Username": "<given name>" \| "<not logged in>", "LoggedIn": bool}`. It is local only and does not check the token. |
| Login check | `library sync` refreshes an expired token first (`refresh_token`). A refused refresh logs `ERROR [AUTH_MANAGER]: Failed to refresh the token <Response [4xx]>`, which means expired. A `ConnectionError` gives the same prefix with the exception, which means network. Then `There was an error syncing library`. |
| Owned list | After `library sync`, GRID reads `library.json` itself, as Heroic does. `library list --json` crashes when the file is missing. Each entry has `id` (the entitlement), `product.id` (`amzn1.adg.product.<uuid>`: **the id every command takes**, and GRID's `store_id`), `product.sku`, `product.asin`, `product.title` (may be missing), `product.productDetail.iconUrl`, and `product.productDetail.details.{shortDescription, developer, publisher, genres[], releaseDate, backgroundUrl1, backgroundUrl2, logoUrl, screenshots[], videos[]}`. Heroic's art: cover `backgroundUrl2 \|\| iconUrl`; square, used for its grid cards, `iconUrl`. Every game is Windows-only. |
| Installed | There is no list command. GRID reads `installed.json`: `[{"id": <product id>, "version", "path", "size"}]`. |
| Size | `install --info --json <id>` prints `{"download_size": <bytes>}`. Files are stored as they download, so the disk size is the download size. |
| Install | `install --path <exact dir> <id>`. Heroic uses `--base-path`, but GRID picks the folder, as with Epic's `--game-folder`. It never prompts. `--max-workers` is ignored (6 threads are hardcoded). If `installed.json` already knows the game, its saved path wins. |
| Progress | stderr, once a second: `INFO [PROGRESS]:\t = Progress: 11.45 424234858/3705895264, Running for: 02:52:48, ETA: 22:16:42`, which gives **real bytes done/total**; then ` = Downloaded: …`, ` + Download\t- 12.34 MiB/s` and ` + Disk\t- …`. Other lines: `INFO [DOWNLOAD]:\t Game is up to date` and `ERROR [DOWNLOAD]:\t Not enough space available`. `Checksum error for <path>` goes to **stdout**. |
| Update | `update <id>`, an alias of install that downloads the diff against `manifests/<id>.raw`. `list-updates --json` prints a JSON array of product ids, using the network; with nothing installed it prints `[]` plus `ERROR [CLI]: No games installed`. **Versions are opaque version ids**, not readable numbers (Question 1). |
| Verify | `verify <id>` reloads the installed manifest, re-hashes and downloads bad or missing files, with the same progress format. It needs the network and gives **no repaired-file count**, so the result is "Verified". |
| Uninstall | `uninstall <id>` deletes the manifest's files and empty folders, removes the `installed.json` entry and the manifest. It **crashes if the manifest is missing**; Heroic then edits `installed.json` itself. |
| Launch | `launch --json --no-wine <id>` is a dry run. It prints `{"command":{"instruction":<abs exe>,"arguments":[…]},"game_directory":…,"env":{FUEL_DIR, AMAZON_GAMES_SDK_PATH, AMAZON_GAMES_FUEL_ENTITLEMENT_ID, AMAZON_GAMES_FUEL_PRODUCT_SKU, AMAZON_GAMES_FUEL_DISPLAY_NAME}}`, read from `<install>/fuel.json` (`Main.Command`, `Main.Args`, `Main.WorkingSubdirOverride`). **There is no token in argv or env.** `AMAZON_GAMES_FUEL_DISPLAY_NAME` is the user's given name: personal data, never logged. |
| Cloud saves | None (Heroic: "Amazon Games doesn't support cloud saves"). |
| Debug | `-d` turns on DEBUG logging. Never pass it: at DEBUG nile logs response bodies. |

How Heroic launches (`H/.../nile/games.ts` L299-422): nile spawns the game with
`--no-wine --wrapper '<umu/proton> waitforexitandrun'` (or `--wine`), and on Linux it
waits as a subreaper. **GRID instead owns the process**, as for Epic: it takes
`launch --json`, then `build_native_command` (umu-run/wine/native), so the session holds
the child. Heroic also passes `--override-exe`, a flag nile 1.2.0 doesn't define;
`parse_known_args` hands it to the game as an argument. Do not copy it.

### Limits

- Every Amazon game is a Windows build, run under GRID's compat tool off Windows.
  - Online games use FuelPump DRM. nile provides it through the five env variables and the
    SDK DLLs; the README says such games "might not work".
  - `FUEL_DIR` / `AMAZON_GAMES_SDK_PATH` are host paths, passed as Heroic passes them.
- nile's marketplace is hardcoded to US (`ATVPDKIKX0DER`) and the login host to
  `amazon.com`. No regional handling is documented. An account from another marketplace
  may sign in but see a different or empty library. GRID can't fix that; the empty panel
  covers it.
- nile issues to plan around:
  - a non-ASCII display name crashes nile on Windows (#68, open against 1.2.0);
  - no clean cancel (#54). The runner kills the process group;
  - a dropped connection can end an install as "complete" (#51);
  - `CERTIFICATE_VERIFY_FAILED` on some systems (#46).
- No account term found that GRID's use adds to beyond what Heroic and nile already do.

Sources:
- the N/ files `nile/{arguments,cli,constants}.py`, `nile/api/{authorization,library,self_update}.py`,
  `nile/downloading/{manager,progress,worker}.py`, `nile/utils/{config,launch,uninstall,weblogin}.py`,
  `README.md`, `.github/workflows/build.yaml`;
- `https://api.github.com/repos/imLinguin/nile/releases/tags/v1.2.0`;
- the H/ files `meta/downloadHelperBinaries.ts`, `src/backend/storeManagers/nile/{user,library,games,setup}.ts`,
  `src/common/types/nile.ts`, `src/frontend/screens/WebView/index.tsx`,
  `src/backend/utils/compatibility_layers.ts`;
- Lutris `lutris/services/amazon.py`;
- nile issues #22, #46, #51, #54, #68.

## Step 0 — contracts skeleton (one commit before fan-out)

Phase 2 generalised nearly everything, so Step 0 is small. It delivers the types below,
two frozen-file changes (`LoginSpec.finish_query` and the async login preparation), the
launch-hook router, the nile signal parser, `AmazonMeta` and the row builder with tests,
`[stores.amazon]`, the `api.ts` unions and the stub backend.

**Step 0 passes the gate and every `gog*`, `tv-gog`, `epic*` and `tv-epic` e2e group
before fan-out.** After it, these files are frozen and change through the lead: `api.ts`,
`config.rs`, `stores/{mod,runner,tools,progress,art}.rs`, `stores/amazon/{mod,meta,row}.rs`,
`stores_service/mod.rs` and `store_auth.rs`.

### grid-core

```rust
// stores/tools.rs
pub enum StoreTool { Gogdl, Legendary, Nile }       // name "nile", e2e env GRID_LAUNCHER_E2E_TOOL_NILE
static NILE_PIN: ToolPin;                            // v1.2.0, GPL-3.0, github.com/imLinguin/nile, the five assets above, all Direct

// stores/runner.rs + progress.rs
pub enum LineParser { Gogdl, Legendary, Nile }       // Nile: parse_nile_line + nile_signal on stderr and stdout lines
pub fn parse_nile_line(line: &str) -> Option<ToolProgress>;  // "= Progress: <pct> <done>/<total>" gives bytes; "+ Download\t- X MiB/s" gives speed
pub fn nile_signal(line: &str) -> Option<ToolSignal>;
//  "Failed to refresh the token <Response"  -> NotLoggedIn     "User not logged in"          -> NotLoggedIn
//  "Failed to refresh the token" + ConnectionError / requests.exceptions traceback          -> Network
//  "Failed to register a device"            -> LoginFailed     "Not enough space available"  -> NotEnoughSpace
//  "Game is not installed" / "No game is installed" -> NotInstalled
//  "Game is up to date"                      -> DownloadSize(0) (already current, as legendary's)
//  "Checksum error for" (stdout)             -> OtherError     any other "ERROR [" / "CRITICAL [" / traceback -> OtherError
//  except "No games installed" (list-updates with nothing installed): no signal
// Nothing new in ToolSignal. Every nile command sets private_tree = <data>/stores/nile
// and calls paths::ensure_private_dir on it before the run, so the folder is 0700 even
// while nile writes 0644 files.

// stores/art.rs
STORE_ART_DOMAINS += "media-amazon.com", "ssl-images-amazon.com", "images-amazon.com";  // spike 4 confirms

// stores/amazon/meta.rs  (store_meta JSON; never a token)
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct AmazonMeta { pub product_id: String, pub version: String /* installed.json version id */,
                        pub exe_user_pinned: bool }

// stores/amazon/mod.rs  (types in Step 0, bodies package A)
pub const AMAZON_CODE_PARAM: &str = "openid.oa2.authorization_code";
pub const AMAZON_ASSOC_HANDLE: (&str, &str) = ("openid.assoc_handle", "amzn_sonic_games_launcher");
pub struct AmazonEndpoints { pub finish: Url /* https://www.amazon.com/ */, pub login_hosts: Vec<Url>,
                             pub art_origins: Vec<Url> }   // ::real(), ::e2e(base): finish = <base>/, login host = base
pub struct AmazonLoginStart { pub url: Url, secret: SecretString /* client_id, code_verifier, serial as JSON */ }
impl Debug for AmazonLoginStart   // prints the url's host only
pub struct AmazonAccount { pub username: String }
pub struct AmazonGame { pub product_id: String, pub entitlement_id: String, pub title: String, pub cover: String,
    pub screenshots: Vec<String>, pub description: String, pub developer: String, pub year: String,
    pub genres: Vec<String> }                         // Serialize/Deserialize
pub struct AmazonInstalled { pub id: String, pub version: String, pub path: String, pub size: u64 }  // installed.json row
pub struct AmazonInstallRequest { pub product_id: String, pub title: String, pub install_dir: PathBuf, pub compat_tool: String }
pub struct AmazonUpdate { pub product_id: String }
pub struct AmazonJobCtx { pub registry: Arc<Registry>, pub library: PathBuf, pub default_compat_tool: String }
pub struct AmazonService;   // Clone, Send+Sync
impl AmazonService {
  pub fn new(data_dir: PathBuf, http: reqwest::Client, ep: AmazonEndpoints) -> Self;
  pub fn with_tool_override(self, program: PathBuf) -> Self;            // #[doc(hidden)], the ONE test override (Phase 2 lesson)
  pub fn tool(&self, args: Vec<String>) -> ToolCommand;                 // the ONE nile builder: env, parser, private_tree
  pub fn tool_installed(&self) -> bool;  pub async fn install_tool(&self, progress) -> Result<(), StoreError>;
  pub async fn start_login(&self) -> Result<AmazonLoginStart, StoreError>;          // auth --login --non-interactive; checks the url host
  pub async fn complete_login(&self, start: AmazonLoginStart, code: SecretString) -> Result<AmazonAccount, StoreError>; // register
  pub async fn saved_account(&self) -> Result<Option<AmazonAccount>, StoreError>;  // auth --status (local)
  pub async fn sync(&self) -> Result<Vec<AmazonGame>, StoreError>;                 // library sync, then read library.json; LoginExpired / Network
  pub fn owned_local(&self) -> Vec<AmazonGame>;                                     // library.json as it is (startup, offline)
  pub fn installed(&self) -> Vec<AmazonInstalled>;                                  // installed.json
  pub async fn download_size(&self, id: &str) -> Result<u64, StoreError>;          // install --info --json
  pub fn install_job(&self, req: AmazonInstallRequest, ctx: AmazonJobCtx) -> StoreJob;
  pub fn update_job(&self, row: &InstalledGame, ctx: AmazonJobCtx) -> StoreJob;
  pub fn verify_job(&self, row: &InstalledGame, ctx: AmazonJobCtx) -> StoreJob;
  pub fn uninstall(&self, registry: &Registry, id: &str) -> Result<(), StoreError>;
  pub async fn updates(&self) -> Result<Vec<AmazonUpdate>, StoreError>;           // list-updates --json
  pub async fn launch_prep(&self, row: &InstalledGame) -> Result<StoreLaunchPrep, StoreError>; // launch --json --no-wine
  pub async fn disconnect(&self) -> io::Result<()>;   // auth --logout (best effort, short timeout), then remove_tool_dir
}

// stores/amazon/row.rs  (implemented in Step 0)
pub fn amazon_row(inst: &AmazonInstalled, game: Option<&AmazonGame>, meta: &AmazonMeta, exe: Option<&FuelMain>,
                  wineprefix: Option<&Path>) -> InstalledGame;
// platform "Amazon Games", slug "win", store_id = product id, filesize = installed size,
// native_executable_path from fuel.json Main.Command (absolute, `\` → `/`, the same path
// rules as epic_row), working dir Main.WorkingSubdirOverride or the exe's folder, launch
// params Main.Args, native_wineprefix <install>/prefix off Windows, companies = developer,
// first_release_date = year.
pub struct FuelMain { pub command: String, pub args: Vec<String>, pub working_subdir: String }
pub fn read_fuel(install: &Path) -> Option<FuelMain>;   // fuel.json is JSON5: comments and trailing commas → `json5` crate
                                                        // (or a strip-then-serde_json pass; package A picks, spike 3 shows real files)

// config.rs
pub struct StoresConfig { pub gog, pub epic, #[serde(default, skip_serializing_if = "is_default")] pub amazon: AmazonConfig }
pub struct AmazonConfig { pub connected: bool, pub username: String, pub install_dir: String /* "" = <library>/Amazon Games */ }
```

### App (`app/src-tauri/src`)

- **`store_auth.rs`:**
  - `LoginSpec` gains `finish_query: Option<(&'static str, &'static str)>`. A URL is the
    finish only when the scheme, host, port and path match **and** that query pair is
    present.
  - GOG and Epic pass `None`, so their behaviour doesn't change.
  - The Amazon spec: `finish = endpoints.finish`, `finish_query = Some(AMAZON_ASSOC_HANDLE)`,
    `code_param = AMAZON_CODE_PARAM`, `user_agent = None` (Spike 5 may change it).
  - Its `login_url` is the per-attempt URL nile printed.
- **`StoreBackend::prepare_login(self: Arc<Self>) -> BoxFut<'static, Result<LoginSpec, String>>`.**
  - It replaces the `begin_login()` + `login_spec()` pair in `store_connect`.
  - Its default body is `begin_login()?; Ok(login_spec())`, so GOG and Epic are
    untouched.
  - Amazon's version installs nile if it is missing (see "Choices taken" 1), runs
    `start_login`, keeps the `AmazonLoginStart` in the store, and returns the spec.
  - `run_finish_login(code)` takes the kept start.
- **Launch-hook router.**
  - `LaunchService` has **one** hook slot, and Phase 2 filled it with `EpicStore`.
  - `stores_service/mod.rs` gets `StoreLaunchRouter(Weak<StoreShared>)`, which implements
    `StoreLaunchHook`. It hands `game.source`'s store to that backend's
    `launch_hook()` (a new trait method, default `None`).
  - `install_launch_hook` installs the router. This replaces a Phase 2 integration item
    that would otherwise fail silently: Amazon rows would launch with no FUEL env.
- **`StoresService`** gains `amazon: Arc<AmazonStore>`, registered in `STORE_ORDER`.
  `backend(Amazon)` returns it, and `JobCtx::amazon()` is added.
- **`stores_service/amazon.rs`** is the stub:
  - its status is always "not connected", with the nile pin;
  - `in_rail` only when installed rows exist;
  - every action answers `not implemented`;
  - Game Settings are real (compat tool, executable);
  - the login spec is real;
  - `GRID_LAUNCHER_E2E_AMAZON_BASE` (feature `e2e`) feeds `AmazonEndpoints::e2e`.
- **`commands/stores.rs`:** `store_kind`, `store_key` and `store_row` accept `amazon`.
  The `store_key("amazon:x").is_err()` test flips. **No new commands or events.**
- `update_service.rs` gets a `StoreUpdateSource` stub for `AmazonStore`, which reports no
  updates until package C.

### TypeScript (`api.ts`)

```ts
ToolStatus.name: 'gogdl' | 'legendary' | 'nile';
StoreGame.origin: 'gogdl' | 'offline' | 'drive' | 'legendary' | 'nile' | null;
```

No new methods. `ENABLED_STORES` gains `'amazon'` in package D, not in Step 0.

`check_secret_hygiene.sh` allows `expose_secret` in **one** new file,
`stores/amazon/auth.rs`. It is the only place that reads `AmazonLoginStart`'s secret and
the code, and the only place that writes them into the `register` argv.

## Work packages (after Step 0)

Three packages, not five: Amazon has no cloud saves and no DLC, and its launch prep is
small enough to sit in A. The desktop and TV UI changes are mostly data and words, so
they are one package.

### A — grid-core Amazon store

- **Owns:** `stores/amazon/{mod,auth,list,install,launch}.rs` (`meta.rs`/`row.rs` belong
  to Step 0), `crates/grid-core/tests/amazon_store.rs` and its fixtures.
- **Tool:**
  - the pinned download with the SHA-256 check (a mismatch deletes it: `Refused`);
  - env `NILE_CONFIG_PATH=<tool dir>`;
  - `LineParser::Nile`, `private_tree`, `ensure_private_dir` before each run;
  - never `-d`.
- **Auth:**
  - `start_login`: parse the JSON with `read_stdout`, and refuse a URL that is not
    `https://{www.,}amazon.com/ap/signin` (or the e2e base).
  - `complete_login`: `register` with the four values in argv.
  - **Signed in** means no `LoginFailed`/`Network`/`OtherError` signal, `current_user.json`
    has a `user_id`, and `auth --status` says `LoggedIn: true`. The username comes from
    there.
  - `LoginFailed` gives "Amazon didn't accept the sign-in. Try again."
- **Owned list:**
  - parse `library.json` with serde, ignoring unknown keys and nulls;
  - dedup by `product.id`, then by `product.asin`; a missing title falls back to the id;
  - cover = `iconUrl`, then `backgroundUrl2`, then `backgroundUrl1` (Heroic's card
    image). Spike 4 checks whether it suits a portrait card;
  - screenshots = `details.screenshots`, else `backgroundUrl1`;
  - year from `releaseDate`.
  - `sync()` maps a signed-out sync (exit 1) and `NotLoggedIn` to `LoginExpired`, and
    `Network` to `Network`.
- **Jobs:**
  - install is `install --path <install_dir>/<title with nile's banned characters
    stripped> <id>`;
  - progress in bytes from `parse_nile_line`; total from `download_size`;
  - **Success** means no failure signal, the id present in `installed.json` with a
    non-empty version, and `fuel.json` readable. `Download complete` is ignored.
  - **Update** passes on `DownloadSize(0)` (already current) or on a changed version.
  - **Verify** is phase `Verifying`, and its detail stays `""` ("Verified").
  - After an install that saw a `Network` signal and still "succeeded" (#51), the job runs
    `verify` once before finalize.
  - `finalize` writes the row with `amazon_row`.
  - Cancel kills the group (as Epic). A cancelled first install removes its folder through
    `RemovalGuard`, the same as Epic's.
- **Uninstall:** `uninstall <id>`, then the leftover folder and `<install>/prefix` through
  `RemovalGuard`.
  - A missing nile, `NotInstalled`, or a traceback (missing manifest) still removes the
    files.
  - It also drops the id from `installed.json`. Heroic does the same; the file holds no
    token.
  - Any other failure stops before a file is touched.
- **Updates:** `list-updates --json`, keeping only installed ids. The `No games installed`
  line is not an error.
- **Launch prep:**
  - `launch --json --no-wine <id>`, its stdout parsed in `launch.rs`;
  - `executable`/`working_dir` from the dry run, left empty (so the row's are used) when
    the user pinned an exe;
  - `game_args` = `arguments` minus the row's own launch params (no doubling, the Epic
    rule);
  - `env` = the five FUEL vars. **No `secret_args`.** `StoreLaunchPrep`'s Debug prints
    counts only, so the display name is never printed.
  - It needs no network. Signed out, it still works, because nile reads only the local
    files. Spike 3 confirms this.
  - Failures give `LaunchError::Validation("Couldn't start <title> through nile: …")`.
- **Tests:**
  - parsers on recorded-shape fixtures carrying `FAKE-STORE-TOKEN-not-real`;
  - argv builders (`--path` always, never `-d`, never `--base-path`);
  - progress and signal parsers;
  - the URL host check;
  - SHA mismatch;
  - a `cfg(unix)` fake nile;
  - `AmazonLoginStart` never in Display/Debug.

### C — app services, login window, e2e harness

- **Owns:**
  - app: `stores_service/amazon.rs`, the Amazon branches of `commands/stores.rs`, the
    Amazon `StoreUpdateSource` in `update_service.rs`, and `lib.rs` after Step 0;
  - e2e: `e2e/mock-romm/mock-amazon.mjs` (+ test), `e2e/fake-tools/nile.mjs` (+ test),
    `e2e/seed/amazon-seed.mjs`, `e2e/fixtures-amazon/library.json`,
    `e2e/helpers/amazonLogin.ts`, the `scripts/e2e.sh` groups, `wdio.conf.ts`,
    `.gitignore` (the mock request log), `e2e/specs/amazon-login.spec.ts` and
    `amazon-secrets.spec.ts`.
- **AmazonStore** is built on `EpicStore`, without the cloud and DLC parts:
  - an `AmazonApi` seam (`RealAmazon` forwards to the service, and unit tests fake it);
  - status;
  - **the owned list's only source is nile's `library.json`.** There is no
    `amazon-owned.json`; startup reads `owned_local()` (Phase 2 lesson: one owned-list
    owner);
  - background-only notices (its own `notice` module, with Epic's words and "Amazon Games"
    / "nile");
  - install guards (signed in, tool ready, not running);
  - the `launch_hook`.
- **Login:** `prepare_login` as in Step 0.
  - The window, incognito data folder, network-error page, loopback rule and pop-up rule
    are Phase 2's.
  - Success runs `complete_login`, then a sync, then "Connected to Amazon Games as …".
    After an expired login it reads "Signed in to Amazon Games as …".
  - nile's register already syncs once; GRID's sync after it is the one that counts.
  - A failure keeps the window's Try again card.
- **Launch hook:** signed in or not, Amazon games launch (nile needs no ticket). A
  `launch_prep` failure is a `Validation` error with nile's words. There is no offline
  retry.
- **StoreGame for Amazon:**
  - `platforms: ['windows']`, `origin: 'nile'`;
  - sizes from `download_size`, cached once the install dialog asks; disk = download;
  - `version` is the installed version id; `update_version` follows Question 1;
  - `not_supported` is always null.
- **Settings card data:** "nile 1.2.0 · installed"; "nile · GPL-3.0 ·
  github.com/imLinguin/nile"; size from the host asset (11 MB on Linux x86_64). The
  prototype's "1.1.2 · 9 MB" are placeholders.
- **Disconnect:** `disconnect()`, which deregisters as a best effort and then deletes
  `stores/nile/`. The config is cleared. Installed games stay, and still play through the
  row's exe and the compat tool, with no FUEL env. That matches the design's "stay and
  still play"; a DRM title may then refuse to start.

### D — UI (desktop and TV)

- **Owns:** `app/src/lib/gamestores/**`, `stores/gameStores.svelte.ts`, `fakeApi.ts` and
  `devMock.ts`, `settings/StoresPage.svelte`, `app/src/lib/tv/stores/*`,
  `lib/notifications/*` store words, `e2e/helpers/amazon.ts`,
  `e2e/specs/amazon-library.spec.ts` and `tv-amazon.spec.ts`.
- **Work:**
  - `ENABLED_STORES = ['amazon', 'epic', 'gog']`;
  - `storesLede()` gets the prototype's line ("GOG, Epic and Amazon games");
  - `fakeApi`/`devMock` get the prototype's a1–a3 (Fogline installed; Kettle Drum and
    Lowsun available), account `sam.plays`, nile 1.2.0 (11 MB), and the per-store
    scenarios. The prototype's own default for Amazon is **expired**;
  - check every path that assumes "a tool store has cloud saves". Amazon is the first
    `hasTool && !hasCloud` store: no Cloud saves tab, chip, Game Settings toggle or
    Settings row; Disconnect reads "they can't install or update"; uninstall has no
    cloud sentence; launch has no sync toasts;
  - the update button and version fact follow Question 1;
  - the install dialog shows no Version, Language or DLC;
  - TV is the same port through `storeWords.ts` (`loginHost('amazon')` already exists).
- **Milestone D1** (pure modules) lands first, as in Phase 2.

**Order:** Step 0, then A, C and D together; D works against `fakeApi` until C is merged.
Merge A, C, D, then run the integration list and the full e2e suite on the merged tree.

## Parity inventory (Amazon states in the prototype)

Each item is checked by screenshot against the prototype before its package is done
(`design-parity-handoff`). Sources: `stores.js` (L13-17 store, L48-50 games, L170-190
states, L212-241 header and panels, L455-470 Details, L637-660 Settings card, L700-730
login), `stores_bk.js` L8, `tvstores.js` (L9, L29-31, L531, L541) and `tvstores_bk.js` L8.

Desktop (GameStores):
1. **Rail group** "Amazon Games" (mark A, hue 32): first, above Epic Games, with Installed
   / Available counts. In the prototype's starting state the heading badge is `!` (expired).
   It also has `off`, a spinner while loading, and `!` on an error.
2. **Header** "Amazon Games · Installed" / "· Available". Chips: "Signed in as sam.plays",
   **no "Cloud saves" chip**, "Login expired · Sign in again", "nile not installed".
   Refresh is on Available. No Install from installer…, Select or Backup all.
3. **Covers:** Fogline installed (last played); Kettle Drum and Lowsun available, Install;
   the installing percentage, QUEUED and bar; an UPDATE tag (Question 1). There is no NOT
   SUPPORTED tag and no SAVE CONFLICT tag.
4. **Panels:**
   - "Loading your Amazon Games games…";
   - nothing installed;
   - everything installed;
   - **login expired** ("Your Amazon Games login expired" / "Sign in again to see the
     games you own and install them. Installed games still play." · Sign in again…), the
     prototype's default;
   - not connected (Connect Amazon Games…);
   - nile missing, installing, and failed checksum.
5. **Install dialog:** Windows only (no Version, no Language, no DLC), compat tool,
   install folder, the space line and Not enough space, the footer.
6. **Details:**
   - Play / Install… (disabled when not connected or expired) / Cancel install / Update /
     Game Settings / Verify and repair / Uninstall, then Confirm uninstall;
   - "Runs through <tool> · Windows version";
   - "Amazon Games games can't be backed up: nile installs only from Amazon's servers.";
   - "Your Amazon Games login expired. Sign in again" (Available game while expired);
   - "Connect Amazon Games in Settings › Stores to install.";
   - tabs Overview and Media only, **no Cloud saves tab**.
7. **Launch:** "Launching …" with no cloud toasts; Stop gives "Stopped …".
8. **Game Settings:** executable, launch parameters, compat tool and prefix, **no cloud
   toggle**.
9. **Settings › Stores Amazon card:**
   - connected: Account, Games you own, Store tool, License, Install folder, **no Cloud
     saves row**, Show in Library / Refresh games / Disconnect…;
   - the Disconnect confirm: "…GRID deletes your Amazon Games login and nile from this
     computer. The N installed games stay and still play; they can't install or update
     until you connect again.";
   - expired (warning paragraph, Sign in again…);
   - not connected (lede "GRID installs with nile (nile, GPL-3.0, <size>)…", tool row
     "Installs when you connect · <size>").
10. **Login window:** "Sign in to Amazon Games — GRID · 🔒 www.amazon.com" (the URL strip
    reads `www.amazon.com/ap/signin`), "Loading www.amazon.com…", "Signed in. Closing…",
    the network-error strip, the failed sign-in card with Try again, and a silent cancel.
    Then the toast "Signed in to Amazon Games as sam.plays" after expired, or "Connected to
    …" otherwise.
11. **Downloads rows:** "Downloading 45% · 41.2 MB/s", "Verifying files 45%", the tool's
    "checking its SHA-256", "Installed · 7.0 GB", "Updated", "Verified", failed with Retry
    and Dismiss, and the strip "… · Amazon Games · Downloading …".
12. **Notification cards:**
    - connected / signed in again;
    - "Amazon Games: N games found · M to install";
    - nile installing / installed / failed;
    - queued; installed · Play; updated; verified; update available · Show;
    - uninstalled, **with no cloud sentence**;
    - disconnected;
    - signed-out install · Sign in again;
    - missing tool · Install nile.

TV (TVStores):
- items 1–4 as TV folders and panels;
- the game page: buttons; the left column with the no-backup line; LB/RB tabs without
  Cloud saves;
- the install page (no Version, no DLC rows);
- the game settings page without the cloud row;
- Settings › Stores › Amazon (connected, expired, not connected, Disconnect confirm);
- **"Show: sign in to Amazon Games"**: the sign-in layer over TV mode with the keyboard,
  Y and B;
- the prototype menu's "Amazon Games: connected" (from its expired default);
- Downloads rows and the A-list;
- the disconnect card.

## Spikes

Run them first, in the container (`scripts/ctr.sh run …`). No real Amazon account. Use a
scratch `NILE_CONFIG_PATH`/`HOME`/`TMPDIR` and `HTTPS_PROXY=http://127.0.0.1:9`.

1. **The asset, offline:**
   - `--version`, `--help`, and `--help` of `auth`, `register`, `library`, `install`,
     `verify`, `update`, `uninstall`, `launch` and `list-updates`;
   - `auth --status` signed out;
   - `library list --json` with no library (confirm the crash);
   - `list-updates --json` with nothing installed;
   - the SDK download against a dead proxy: a warning, no hang, and how long it takes;
   - where the files land (`<cfg>/nile/`) and their modes;
   - the `_MEI*` folder under `TMPDIR`;
   - **the glibc floor**: extract libpython3.8 with `pyi-archive_viewer` (no execution),
     then `objdump -T`. Compare with GRID's oldest supported distro.
2. **Login without a network:**
   - `auth --login --non-interactive`: the JSON keys and URL shape, and whether it touches
     the network;
   - `register` with fake values: the lines, the exit code, and that **neither the code
     nor the verifier appears in either stream** (grep). Also whether the failure prints a
     body on stdout when there is no response (traceback?).
   - Informational: the interactive `auth --login` stdin path, to record it as the
     rejected alternative.
3. **Launch dry run offline:**
   - a hand-made `library.json` entry, `installed.json`, `fuel.json` (with a comment and
     a trailing comma) and `current_user.json` with a fake name;
   - run `launch --json --no-wine <id>`: the output shape, that it needs no token or
     network, and `--json` without `--no-wine` on a host with no wine.
   - From the 1.2.0 source: `uninstall` with a missing manifest.
4. **Art hosts.** From public sources only (recorded `library.json` shapes in Heroic or
   Lutris issues, and Prime Gaming's public pages): the image hosts and the aspect of
   `iconUrl` vs `backgroundUrl2` for a portrait card.
5. **The Tauri login window with Amazon's real page.** This is manual, by the user, with
   a real account, and optional:
   - that WebKitGTK's own user agent is accepted;
   - captcha and 2-step in an ephemeral context;
   - that the `www.amazon.com/?openid.assoc_handle=…&openid.oa2.authorization_code=` hop
     reaches `on_navigation`;
   - which pop-ups appear.

   Without it, C keeps the structural rule.

## E2E plan

- **Mock Amazon** (`mock-romm/mock-amazon.mjs`, one base for login and art):
  - `/ap/signin` is a form. It 302s to
    `<base>/?openid.assoc_handle=amzn_sonic_games_launcher&openid.oa2.authorization_code=FAKE-E2E-AMAZON-CODE`;
  - the `denied` user gets the same redirect with no code; the `refused` user gets a code
    the fake nile refuses;
  - a link to `<base>/` **without** the assoc_handle must load as an ordinary page (the
    `finish_query` rule);
  - `/` with the assoc_handle must **never be served** (the request log checks);
  - `/art/<id>.jpg` serves the art.
  - Every other group gets the dead loopback base.
- **Fake nile** (`fake-tools/nile.mjs`, through `GRID_LAUNCHER_E2E_TOOL_NILE`) implements:
  - `--version`;
  - `auth --login --non-interactive` (url = `<GRID_LAUNCHER_E2E_AMAZON_BASE>/ap/signin?…`,
    and a `code_verifier` of `FAKE-STORE-TOKEN-not-real`);
  - `register` (writes `current_user.json` and a `.enc` holding the fake token; refuses
    the `refused` code with nile's real lines);
  - `auth --status`, `auth --logout`;
  - `library sync` (writes `library.json` from `fixtures-amazon/library.json`);
  - `install --info --json`, `install --path` / `update` / `verify` with real-format
    progress on stderr, writing `installed.json`, a manifest and a `fuel.json` whose
    `Main.Command` is the argv-recording game script;
  - `uninstall`, `list-updates --json`, `launch --json --no-wine`.

  Rules:
  - **Every command prints `FAKE-STORE-TOKEN-not-real`** on stderr, and on stdout as a
    text line or a `fake_session` key in every JSON object.
  - It writes 0644 and exits 0 on failures, as nile does.
  - Markers in its config dir: `.fake-expired` (refresh refused), `.fake-download-fail`,
    `.fake-no-space`, `.fake-update` (Fogline listed in `list-updates`).
- **Seed** (`seed/amazon-seed.mjs`):
  - signed in as `sam.plays`;
  - Fogline installed; Kettle Drum and Lowsun available;
  - no server profile;
  - `--expired` starts in the prototype's expired state.
- **Groups in `scripts/e2e.sh`:**

  | group | specs | checks |
  |---|---|---|
  | `amazon` | `amazon-login`, `amazon-secrets` | **Login:** nile installs before the window; the code is caught and the assoc_handle `/` is never served; the plain `/` loads; denied, refused and cancel; owned list. **Secrets:** after sign-in, install, update, verify, launch and Disconnect, no GRID log, forwarded line, IPC payload (recorded events plus `invoke` answers), session record or file outside `stores/nile/` holds the fake token, `FAKE-E2E-AMAZON-CODE` or the display name. Every file under `stores/nile/` is 0600 and every folder 0700. Disconnect removes `stores/nile/` even when `auth --logout` fails. The game's env file shows the FUEL vars reached the game. |
  | `amazon-library` | `amazon-library` | rail group and counts, header chips (no cloud chip), every panel including expired, install (no DLC/Version), update, verify, uninstall, Details with no Cloud saves tab, Settings card with no Cloud row, the Disconnect confirm wording |
  | `tv-amazon` | `tv-amazon` | TV folder, panels, game page tabs (no Cloud saves), install page, Settings › Stores › Amazon, sign-in over TV mode with the login window as WebDriver's active window, then back to `MAIN_WINDOW` |

- **Existing specs:**
  - GOG and Epic groups must still pass with Amazon not connected (hidden: no rail
    group).
  - `store_key("amazon:x")` tests flip.
  - `epic-login` still passes through the router hook.

## Gate requirements

- The gate (CLAUDE.md), `check_secret_hygiene.sh` included. Add no `expose_secret` file
  beyond `stores/amazon/auth.rs`.
- `scripts/ctr.sh wincheck`. `ensure_private_dir` before runs and the router touch
  `cfg(unix)` paths, and the nile pin adds a Windows asset.
- The **full** e2e suite on the merged tree. Step 0 changes `LoginSpec`, `store_connect`
  and the launch-hook slot, so every GOG and Epic group is a regression check.
- Fixtures use `FAKE-STORE-TOKEN-not-real` only. The e2e code `FAKE-E2E-AMAZON-CODE` is a
  login code, not a token, and is already of the allowed `FAKE-…` form. If the hygiene
  regex flags it, rename it to `FAKE-STORE-TOKEN-not-real-code`.
- D ticks every parity item, desktop and TV.

## Integration list (expected, from the Phase 2 outcome)

1. **One nile helper and one test override.** `launch.rs`, `install.rs` and `auth.rs` all
   use `AmazonService::tool()` and `with_tool_override`. Phase 2 had to merge B's
   duplicate legendary helper.
2. **One owned-list owner.** Only nile's `library.json`, read by grid-core; the app keeps
   it in memory only. Grep that no `amazon-owned.json` appears.
3. **The launch router.** Epic launches still get their ticket, Amazon launches get the
   FUEL env, and GOG gets `Ok(None)`. Check both from desktop and TV `launch_game_key`.
4. **Parsers tolerate the fake's extra output** (the `fake_session` keys and token
   lines): first JSON value on stdout, unknown keys ignored.
5. **Notices once each.** `StoreLayer` and `storeEvents.ts` handle Amazon, and the backend
   `store-notice` carries only background events.
6. **Art.** `ensure_image` accepts the Amazon hosts and the e2e origin; `TvStoreArt` and
   `StoreCoverSm` show Amazon covers.
7. **No platform row.** `library/rail.ts` lists no "Amazon Games" platform row.
8. **Updates.** The Amazon `StoreUpdateSource` keys by `GameKey`, and the Library's
   Updates view counts Amazon updates. `lib.rs` starts its check after the startup login
   check.
9. **Store Details Escape.** Verify/Update buttons vanish when pressed. The Phase 2 fix
   (window-level Escape when the focus is on the body) must cover Amazon Details too.
10. **No-cloud paths.** Grep the UI for `hasCloud` and `source === 'epic'` left over: Amazon
    is the first tool store without cloud saves, so any path that assumed "tool ⇒ cloud"
    shows up here.
11. **TV specs** keep `switchToWindow(MAIN_WINDOW)` after the login window.
12. **Docs.** SPEC "Game stores" gets an Amazon paragraph, ARCHITECTURE gets
    `stores::amazon` and `stores_service/amazon.rs`, `external-stores-plan.md` gets its
    status line, and this plan gets its outcomes.

## Choices taken (reversible on request)

1. **nile installs before the login window opens.** nile generates the login URL and its
   PKCE values, so it must exist first. On Connect: the "Installing nile (11 MB) for
   Amazon Games…" card and Downloads row, then the window. The alternative was GRID
   building the URL itself from nile's constants; that copies Amazon's device protocol
   into GRID, against "nile as it is".
2. **The login code goes on argv** (see Research). nile has no other path short of
   holding an interactive process open.
3. **The install folder defaults to `<library>/Amazon Games`**, like `<library>/Epic Games`.
4. **The owned list is nile's own `library.json`**, with no GRID copy.
5. **Disconnect deregisters the device as a best effort.** It then deletes
   `stores/nile/`, whatever Amazon answered.

## User decisions (2026-11-13)

1. **Update wording.** For Amazon only, the button reads **"Update"** with no version;
   the cover keeps its UPDATE tag, the update card reads "<game>: an update is
   available", and Overview leaves out the Version fact.
2. **Post-install steps.** fuel.json `PostInstall` steps are skipped everywhere in
   Phase 3, on Windows hosts too.

Questions for the user: none open.
