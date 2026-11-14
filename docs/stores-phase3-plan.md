# Game stores — Phase 3 (Amazon Games / Prime Gaming) work plan

Status: planned 2026-11-13; spikes and Step 0 done 2026-11-13 (see "Spike results" and
"Contract deviations — Step 0"); package C delivered 2026-11-14, its `amazon` e2e group waiting for
package A (see "Package C outcome"). Read it with:
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

## Spike results (2026-11-13)

All five downloaded assets matched the SHA-256s in Research. The Linux x86_64 binary was
copied to a fresh folder and run only inside the build container (`scripts/ctr.sh run`;
Ubuntu 24.04, glibc 2.39), with a scratch `NILE_CONFIG_PATH`, `HOME` and `TMPDIR`, and
every request sent to a dead proxy (`HTTPS_PROXY=http://127.0.0.1:9`, connection refused).
One run used a black-hole proxy (`10.255.255.1:9`, no answer at all). No Amazon account
and no network login were used. Where a line could not be produced offline, it was read
from the v1.2.0 source (`N/`).

### Spike 1 — the asset, offline

- `--version` prints `1.2.0 Robert Speedwagon` (exit 0, about 0.35 s). It writes nothing
  and does not try the SDK.
- `--help` confirms the commands: `auth`, `register`, `library`, `install` (aliases
  `update`, `verify`, one parser), `launch`, `uninstall`, `import`, `list-updates`.
  - `auth`: `--login --status --gui --non-interactive --logout`;
  - `register`: `--code --client-id --code-verifier --serial`;
  - `library {list,sync}`: `--json --force --installed`;
  - `install`/`update`/`verify`: `--max-workers --base-path --path EXACT_PATH --info
    --json id`;
  - `launch`: `--bottle --wine-prefix --wine --json --no-wine --wrapper id` (no
    `--override-exe`, as Research says);
  - `uninstall id`; `list-updates --json`.
- **Every run except `--version` costs about 0.35 s** and first tries Amazon's FuelSDK.
  Against the refused proxy that gives one line, `WARNING [CLI]:<TAB> There was an error
  getting sdk`, and the command goes on. nile skips the fetch only when `<cfg>/SDK`
  exists, so it retries on every command until one succeeds.
  - **Against the black-hole proxy, `auth --status` hung until it was killed at 90 s**
    (exit 124). nile sets no timeout on any request. Step 0 therefore adds
    `ToolCommand.timeout` (see the deviations), and the Amazon code sets
    `QUICK_RUN_TIMEOUT` on every short command.
  - From the source: `get_sdk` writes straight into `SDK/`, so a download that fails
    halfway leaves a partial `SDK/` that nile never fetches again.
- **Files.** The first command creates `<cfg>/nile/` (0755) and `installed.json` (0644,
  `[]`); the id migration that runs on every start rewrites it. Precedence is confirmed:
  `NILE_CONFIG_PATH` wins over `XDG_CONFIG_HOME`, and nile appends `/nile` to either.
- **`auth --status`** signed out: `{"Username": "<not logged in>", "LoggedIn": false}`
  (exit 0). With a hand-written `current_user.json` and no token file it says
  `{"Username": "FakeGivenName", "LoggedIn": true}`: it reads only that file, so
  `LoggedIn: true` does not prove there is a token.
- **`library list --json`** with no `library.json`: an `AttributeError` traceback, exit 1
  (the crash is confirmed). With a `library.json` it prints the file, exit 0, signed out
  too.
- **`list-updates --json`**: nothing installed gives `[]` on stdout plus `ERROR [CLI]:<TAB>
  No games installed` (exit 0). Something installed and signed out gives a traceback,
  exit 1, and no `[]`.
- **`library sync`** signed out: `ERROR [CLI]:<TAB> User not logged in`, exit 1. With
  `current_user.json` but no token file: a traceback, exit 1.
- **`install --info`, `install` and `update`** signed out: with no `library.json`, a
  traceback; with one, `INFO [CLI]:<TAB> Found: Fogline` and then a traceback (the token
  read), exit 1.
- **`verify`** of a game that is not installed: `ERROR [DOWNLOAD]:<TAB> Unable to load
  manifest`, then `INFO [CLI]:<TAB> Download complete`, exit 0. This confirms that
  `Download complete` proves nothing.
- **`uninstall`** of a game that is not installed: `ERROR [UNINSTALL]:<TAB> Game isn't
  installed`, exit 0 (not "Game is not installed"; `nile_signal` matches both).
- **`launch`**: an id not in the library gives `ERROR [CLI]: No game match` (exit 0); an
  empty `installed.json` gives `ERROR [CLI]:<TAB> No game is installed` (exit 0); no
  `library.json` gives a traceback (exit 1).
- **`auth --logout`** signed out: `ERROR [AUTH_MANAGER]:<TAB> You are not logged in`
  (exit 0). With `current_user.json` but no token: a traceback, exit 1. So Disconnect
  must delete the folder itself, as planned.
- **`_MEI*`.** Each run unpacks about 21 MB into `$TMPDIR/_MEIxxxxxx` (0700). A normal
  exit removes it, and so does SIGTERM to the process group (the runner's cancel).
  SIGKILL leaves it behind. The runner's SIGTERM, then SIGKILL after 3 s, is enough.
- **The glibc floor.** The libraries were extracted with a small CArchive parser (no
  execution) and read with `objdump -T`.
  - The bootloader needs `GLIBC_2.14` (x86_64) or `GLIBC_2.17` (arm64).
  - The bundled `libpython3.8.so.1.0` and `libgcc_s.so.1` need **`GLIBC_2.35`** on both
    architectures (the Ubuntu 22.04 build).
  - GRID's release build image is Debian 13 (glibc 2.41), so any computer that runs a
    released GRID meets nile's floor. On an older glibc, nile fails to start, and that
    shows as a tool error.

### Spike 2 — login without a network

- **`auth --login --non-interactive`**: exit 0, one JSON line on stdout,
  `{"client_id", "code_verifier", "serial", "url"}`, and only the SDK warning on stderr.
  - `client_id` is 94 hex characters (the serial and `#A2UMVHOX7UP4V7`, hex-encoded),
    `code_verifier` 43 characters, `serial` 32 hex characters. Every run gives new values.
  - Nothing goes to the network but the SDK fetch.
  - `url` is `https://amazon.com/ap/signin?…` (host `amazon.com`, no `www`), with the
    query keys `openid.ns`, `openid.claimed_id`, `openid.identity`,
    `openid.mode=checkid_setup`, `openid.oa2.scope=device_auth_access`, `openid.ns.oa2`,
    `openid.oa2.response_type=code`, `openid.oa2.code_challenge_method=S256`,
    `openid.oa2.client_id=device:<client_id>`, `language=en_US`,
    `marketPlaceId=ATVPDKIKX0DER`, `openid.return_to=https://www.amazon.com`,
    `openid.pape.max_auth_age=0`, `openid.assoc_handle=amzn_sonic_games_launcher`,
    `pageId=amzn_sonic_games_launcher` and `openid.oa2.code_challenge`.
- **`register` with fake values, Amazon unreachable: exit 1**, not 0. It ends in an
  uncaught `requests.exceptions.ProxyError` traceback (`Network`), and **stdout is
  empty**: with no response there is no body to print.
  - stderr carries `INFO [AUTH_MANAGER]:<TAB> Registerring a device. ID: <client_id>`
    (the runner drops it).
  - **grep: the code, the verifier and the serial appear in neither stream.** The
    client id appears only in that one stderr line.
  - From the source: when Amazon refuses (`response.ok` false), nile logs `ERROR
    [AUTH_MANAGER]:<TAB> Failed to register a device`, prints Amazon's response body on
    stdout and exits 0. The body is Amazon's error JSON; GRID keeps stdout only as a
    `SecretString` and never shows it.
- **The interactive `auth --login`** (rejected alternative): it prints `Login URL:
  <url>` and two prompts on stdout, waits for Enter, calls `webbrowser.open`
  (`BROWSER=true` stopped it), then reads the redirect URL from stdin. It accepted a fed
  redirect (`Got authorization code`) and went on to `register`. With EOF on stdin it
  ends in an `EOFError` traceback, exit 1.

### Spike 3 — launch dry run offline

- With a hand-made `library.json`, `installed.json`, `current_user.json` and a
  `fuel.json` that has a `//` comment and trailing commas, `launch --json --no-wine <id>`
  exits 0 and prints
  `{"command": {"instruction": "<abs exe>", "arguments": [...]}, "game_directory": "<install>",
  "env": {"FUEL_DIR", "AMAZON_GAMES_SDK_PATH", "AMAZON_GAMES_FUEL_ENTITLEMENT_ID",
  "AMAZON_GAMES_FUEL_PRODUCT_SKU", "AMAZON_GAMES_FUEL_DISPLAY_NAME"}}`.
  - `FUEL_DIR` and `AMAZON_GAMES_SDK_PATH` name `<cfg>/nile/SDK/Amazon Games
    Services/{Legacy,AmazonGamesSDK}` even when the SDK was never downloaded.
  - `AMAZON_GAMES_FUEL_DISPLAY_NAME` is `current_user.json`'s name (the fake name was on
    stdout). Signed out (no `current_user.json`) it still works, with an empty name.
  - It needs no token and no network, apart from the SDK attempt, which can hang (so
    launch prep sets the quick timeout).
- **There is no working-folder key.** nile applies `WorkingSubdirOverride` to the cwd of
  its own launch only and never prints it; `game_directory` is the install root. So
  launch prep leaves `working_dir` empty, and the row's (from `read_fuel`) is used.
- `--json` without `--no-wine` prints the same thing (`--json` returns before any wine
  logic). Without `--json`, nile tries to start the `.exe` itself.
- `fuel.json` is read with `json5` (comments and trailing commas are fine), and
  **`SchemaVersion` is required**: without it, a `KeyError` traceback, exit 1. GRID's
  `read_fuel` does not require it.
- `uninstall` with the manifest missing: an `AttributeError` traceback, exit 1;
  `installed.json` and the files are untouched (confirms Research).

### Spike 4 — art hosts

- Public pages only. Junk Store's Amazon game pages, built from Amazon's catalog, use
  `m.media-amazon.com` for every image. Besiege and Disney Pixar Cars each have a 400×225
  PNG icon (16:9), a 512×288 banner, and a 1080×1440 (3:4, portrait) cover, all on that
  host and fetched with no credentials.
- Which `library.json` field carries the portrait image could not be seen without an
  account. Heroic's and BannerHub's code use `iconUrl` as the card art and
  `backgroundUrl1`/`backgroundUrl2` as the hero. If `iconUrl` is the 16:9 icon, it does
  not suit a portrait card; package A keeps the planned order until the user's
  `library.json` shows the fields.
- **Rule:** https on `media-amazon.com`, `ssl-images-amazon.com` and `images-amazon.com`
  and their subdomains.

### Spike 5 — not run

Spike 5 is manual (the user, a real account). It is left for the user's final login test.

### What waits for a real account (the user's final login test)

- Spike 5: WebKitGTK's user agent on Amazon's page, captcha and 2-step in the ephemeral
  context, the finish hop reaching `on_navigation`, and the pop-ups.
- `register`'s real lines: success, and the refusal body on stdout.
- `library sync` succeeding, and the expired-refresh line (`Failed to refresh the token
  <Response [4xx]>`).
- The real `library.json` art fields (the spike 4 portrait question).
- Real `install`/`update`/`verify` progress, the `installed.json` version format and real
  `list-updates --json` output.
- The SDK download succeeding (the FUEL DLLs), and `auth --logout` deregistering.

## Contract deviations — Step 0

Where the skeleton differs from, or adds to, the Step 0 block above:

- **grid-core runner.**
  - `ToolCommand.timeout: Option<Duration>` and `runner::TIMED_OUT` are new (spike 1:
    nile can hang for ever). A run past its timeout is stopped like a cancel (the process
    group, SIGTERM then SIGKILL) and fails with `StoreError::ToolFailed { tool, reason:
    TIMED_OUT }`. `None` (the default) keeps every GOG and Epic run as it was.
  - `LineParser::Nile` reads stdout's lines for signals too (as legendary's), through a
    private `reads_stdout()`.
- **`nile_signal`**, from the spikes:
  - `Failed to refresh the token` followed by anything but `<Response` is `Network`.
    nile logs the `requests` exception's text, which does not name the class.
  - `Game isn't installed` (uninstall) is `NotInstalled`, beside the two `launch` lines.
  - A `requests.exceptions.<network error>` traceback line is `Network` (`register`
    offline).
  - The `Traceback` line gives `OtherError`. The exception line itself
    (`AttributeError: …`) and PyInstaller's `[PYI-…:ERROR]` line are not signals.
  - `You are not logged in` (`auth --logout`) is `NotLoggedIn`.
- **`AmazonService`.**
  - `tool()` returns `Result<ToolCommand, StoreError>`, as Epic's does: `ToolMissing`
    when nile is not installed, and `Refused` when the folder can't be made private.
  - These are real in Step 0, not stubs: `tool`, `tool_env`, `tool_installed`,
    `install_tool` (returns the path, as Epic's), `tool_dir`, `config_dir`
    (`<data_dir>/stores/nile/nile`) and `endpoints`.
  - `QUICK_RUN_TIMEOUT` (90 s) is the deadline package A sets on every short run.
  - `AmazonEndpoints::allows_login_url` (the login-URL host check) and
    `AMAZON_SIGNIN_PATH` are implemented. The e2e finish is the base's root.
  - `AmazonLoginStart::new(url, secret)` and `secret() -> &SecretString` are public;
    only `auth.rs` may call `expose_secret` on it.
- **`row.rs`.**
  - `read_fuel` strips comments and trailing commas, then uses `serde_json`, so no new
    crate. It also has `parse_fuel` (the pure part), `FUEL_FILE` and `join_args`.
  - `amazon_row`: the title falls back to the product id, and `genres` is set too.
  - The launch parameters are `join_args(Main.Args)`: single-quoted where needed, so
    they split back unchanged through `launch::template::split_template`. Windows `\`
    paths survive.
  - The working folder is `WorkingSubdirOverride` when it is a safe relative path, else
    the executable's folder.
- **`AmazonConfig`** derives `Default`. It has no cloud field, and it is not written while
  default.
- **App trait.**
  - `StoreBackend` is now `Send + Sync + 'static`.
  - `prepare_login` has the planned default. `login_spec()` stays on the trait: when the
    window is already open, `store_connect` only focuses it and asks `login_spec()`
    (the spec is unused then).
  - `launch_hook(self: Arc<Self>) -> Option<Arc<dyn StoreLaunchHook>>`, default `None`;
    `EpicStore` and `AmazonStore` return `Some(self)`.
  - `StoreShared::backend(kind)` (the registered backends) feeds
    `StoreLaunchRouter::new(&Arc<StoreShared>)`.
- **`store_auth`**: a public `is_finish(url, spec)` (`is_login_redirect` plus
  `finish_query`); `classify` uses it.
- **The Amazon stub** (`stores_service/amazon.rs`):
  - `cloud_state` answers `off`, and `cloud_sync` answers "Amazon Games has no cloud
    saves." (not `not implemented`);
  - the DLC calls answer `not implemented`;
  - the login spec's `login_url` is the kept start's URL, else `<login host>/ap/signin`;
  - `login_cancelled` and `login_redirect_failed` drop the kept start;
  - its launch hook refuses an Amazon row with "Couldn't start <title> through nile: not
    implemented".
- **Updates.** `update_service.rs` has a `StoreUpdateSource` for `AmazonStore` that is
  never `ready()`. `lib.rs` already spawns its check after the startup login check
  (integration item 8); it does nothing until package C.
- **The hygiene script** allows `expose_secret` in `stores/amazon/auth.rs`, which does
  not exist yet.

- **Results.** `scripts/ctr.sh gate` PASS; `scripts/ctr.sh wincheck` PASS;
  `scripts/ctr.sh e2e gog gog-library gog-cloud tv-gog epic epic-library epic-cloud tv-epic launch`:
  12 spec files passed, `== e2e: all stages passed`. GOG and Epic are unchanged through
  `prepare_login`, the `finish_query` rule and the launch router.

## Contract deviations — package A

Package A filled `stores/amazon/{mod,auth,list,install,launch}.rs` and added
`crates/grid-core/tests/amazon_store.rs` (a `cfg(unix)` fake nile that prints
`FAKE-STORE-TOKEN-not-real` on both streams and exits 0 on failures, as nile does). Where
it differs from, or adds to, the section A and Step 0 blocks:

- **Frozen files (the glibc floor).** `runner.rs` gains `ToolSignal::SystemTooOld`, and
  `progress.rs`'s `nile_signal` maps any line naming `GLIBC_…` plus `not found` to it,
  before every other rule (PyInstaller's `[PYI-…:ERROR] Failed to load Python shared
  library …` line and the bootloader's own loader error). Nothing else changed there.
- **`AmazonService::run` (crate-private) is every nile run's one path.** It wraps
  `run_tool` and turns a nile that can't start into
  `StoreError::Refused(NILE_SYSTEM_TOO_OLD)`, "nile can't run on this computer: it needs
  a newer Linux system (glibc 2.35 or later)." That covers a `SystemTooOld` signal, and on
  Linux an exec error for the installed file (for example a missing ELF loader). nile's
  raw stderr is never in it. Settings and the store panel can show the text as it is, or
  match the public constant.
- **New public items:** `NILE_SYSTEM_TOO_OLD`, `SYNC_TIMEOUT` (300 s, `library sync`),
  `LOGOUT_TIMEOUT` (30 s), `AmazonService::has_login()` (`current_user.json` names a user
  id), `install::folder_name(title, id)`, `install::UNINSTALL_TIMEOUT` (600 s),
  `launch::launch_error(title, &err)` and `launch::FUEL_ENV`, plus `auth::LOGIN_REFUSED`
  ("Amazon didn't accept the sign-in. Try again.") and `auth::CURRENT_USER_FILE`.
- **`start_login` first deletes nile's saved login** (`current_user.json` and the
  `*.enc` token file). nile refuses both `auth --login` and `register` with "You are
  already logged in" while a user id is saved, even after it has expired (`N/nile/cli.py`
  `handle_auth`, `handle_register`). The library, `installed.json` and the manifests
  stay. So a "Sign in again" that the user cancels leaves nile signed out:
  `saved_account()` answers `None`, while `[stores.amazon] connected` is still true.
  Package C shows that as the expired state.
- **`complete_login` success** = no `LoginFailed` signal, and nile wrote
  `current_user.json` with a user id (the folder was cleared before the run), then
  `auth --status` says `LoggedIn`. An `OtherError` from nile's library sync after a good
  registration does not fail the login (it departs from section A's "no `OtherError`"):
  GRID's own sync comes next and reports it. `Network` counts only when no user was
  saved.
- **Signed out, nothing runs.** `saved_account()` is `None` without running nile when
  there is no saved login; `sync()` is `NotSignedIn`; `updates()` with nothing installed
  is empty without a run; every job fails with `NotSignedIn` ("not signed in") before
  nile starts.
- **`sync()`** also answers `LoginExpired` when nile's token file is gone (nile then
  crashes with a traceback). Its timeout is `SYNC_TIMEOUT`, not `QUICK_RUN_TIMEOUT`,
  because a first sync pages through the whole library. Every other short command uses
  `QUICK_RUN_TIMEOUT`.
- **Jobs.** The success rule, the #51 verify-once, the update rule and "Verified" are as
  planned. The bar's total is `install --info`'s size until nile's first `= Progress`
  line, then nile's own byte total. A cancelled first install removes its new folder
  through the removal guard inside the job (`download`), because grid-core has no
  other hook for it. fuel.json `PostInstall` is never read.
- **Uninstall** also deletes `manifests/<id>.raw` whenever GRID has to drop the id from
  `installed.json` itself. Otherwise a reinstall would download only the files that
  changed against the stale manifest. A non-zero exit (nile's traceback for a missing
  manifest), `NotInstalled`, a missing nile and `NILE_SYSTEM_TOO_OLD` all still remove
  the files. Exit 0 with any other error line, or the timeout, stops before a file is
  touched.
- **Launch prep** leaves `working_dir` empty (spike 3), and the executable empty when it
  falls outside `game_directory`. After Disconnect, nile's folder is gone, so
  `launch_prep` fails (nile has no `library.json`). Package C's hook should then launch
  without the FUEL env ("stay and still play"), not refuse the launch.
- **Install folder.** `AmazonInstallRequest.install_dir` stays the game's own folder.
  Package C builds it as `<install base>/<install::folder_name(title, id)>`: nile's
  banned characters removed, then outer spaces and dots, falling back to the id.
- **Disconnect** runs `auth --logout` only when a login is saved, then always deletes
  `stores/nile/`.

- **Results.** `scripts/ctr.sh gate` PASS (`== gate: PASS`), and `scripts/ctr.sh wincheck`
  PASS. That includes 37 Amazon/nile unit tests and the 7 `amazon_store` tests: login,
  sync and expiry, install/update/verify/launch/uninstall, failed installs and #51, a
  cancelled first install, uninstall after nile forgot the game and after Disconnect, and
  the launch-prep error. The fake glibc-loader and exec-error binaries are in
  `stores::amazon::tests`. Package A has no e2e group of its own (`amazon*` belong to
  C/D).

## Contract deviations — package C

Where package C differs from, or adds to, the plan. One frozen file changed, in a test only
(`stores_service/mod.rs`, see the launch hook).

- **`AmazonStore` has a seam.** `AmazonStore::new(api: Arc<dyn AmazonApi>, data_dir,
  config_path, shared)`; `RealAmazon` forwards to `AmazonService`, and the unit tests fake it
  (as `EpicApi`). `AmazonStore::real(...)` keeps its Step 0 signature. Besides the service's
  calls the seam has `owned_local_at()` (`library.json`'s mtime, for "Refreshed …" at startup).
- **One owned-list owner.** Startup reads `owned_local()` when `[stores.amazon] connected`;
  every refresh is `sync()`. GRID writes no owned-list file (no `stores/cache/amazon-*`); the
  list lives in memory and goes with nile's folder on Disconnect.
- **Login.** `prepare_login` sets the login phase, installs nile when it is missing (the
  "Installing nile (11 MB) for Amazon Games…" card, then "nile installed", or the
  `tool_failed` card with Install nile), runs `start_login` and keeps the `AmazonLoginStart`.
  Any failure there returns `Err` (GRID's words) to `store_connect`, which the frontend shows
  as "Couldn't open the sign-in window: …"; no window opens and no `store-login` event is sent.
  `run_finish_login` takes the kept start (no start: `failed`, "Amazon Games returned an
  error. Try again."), runs `complete_login`, emits `connected`, then syncs ("Amazon Games: N
  games found · M to install"); a failure emits `failed` with the error's words. Cancel, a
  window that fails to open and a code-less redirect drop the kept start. The inherent method
  is `prepare_sign_in` (a same-named inherent method would shadow the trait's in tests).
- **Login check.** `saved_account()` (local) decides signed in / signed out; an expired login
  shows up in the sync after it (`LoginExpired` flips the card to Expired and raises the
  background "Your Amazon Games login expired…" card). A network failure keeps the state.
- **Disconnect** is sync on the trait (`store_disconnect` runs it in `spawn_blocking`), so
  it blocks on the service's async `disconnect()` on that blocking thread (a runtime of its
  own in unit tests). A `NotFound` is fine; any other error stops it with "Couldn't delete
  Amazon Games's login: …". Then the login window data, the config (`connected`,
  `username`) and the in-memory state go.
- **Launch hook.** Not connected (after a Disconnect, when nile and its files are gone) or
  nile missing: `Ok(None)`, the row launches as it is. Otherwise `launch_prep`, connected or
  expired; an error is `LaunchError::Validation("Couldn't start <title> through nile: <error>")`,
  with no retry. The frozen `stores_service/mod.rs` router test now expects `Ok(None)` for its
  not-connected Amazon row (it expected Step 0's refusal).
- **Installs.** `ensure_can_install` as Epic's. The install dialog asks `download_size` and
  answers one Windows version with download = disk = that size (no languages, no DLC); the
  size is kept for the Available card. `install_job` refuses a platform other than `windows`
  and **passes the game's own folder** as `AmazonInstallRequest.install_dir`
  (`<install base>/<sanitize_component(title)>`, or the dialog's folder), as Epic does; A's
  `install --path` must use it as is (the Step 0 doc comment), not append the title again.
- **Updates.** `StoreUpdateSource for AmazonStore`: ready when connected with nile ready;
  `find` is `list-updates`, keeping ids that have a row; each update carries
  `AMAZON_UPDATE_MARK` (`"latest"`) as its version and build, since nile's versions are
  opaque. **`StoreGame.update_version` is `"latest"` for an Amazon game with an update** (D
  only tests it for non-empty; User decisions 1: the button reads "Update"). A finished
  update hides its UPDATE tag until the next check. The card is "Fogline: an update is
  available" (User decisions 1, word for word).
- **StoreGame / status.** `origin: nile`, `platforms: ['windows']`, `cloud: 'none'`, no DLC,
  `not_supported` empty, `version` the installed version id; `in_rail` is "connected or
  expired, or any installed game" (Epic's); `cloud_saves` always false. `cloud_state`
  answers `none` (Step 0 said `off`), `cloud_sync` "Amazon Games has no cloud saves.", the DLC
  calls "Amazon Games games have no DLC in GRID.".
- **Notices** (`stores_service::amazon::notice`): Epic's words with "Amazon Games" / "nile",
  no cloud cards; a verify reads "Verified <title>'s files" (nile gives no count).
- **E2E harness.**
  - The fake nile prints the token on stderr for every command, and on stdout as a text line,
    a `fake_session` key in every JSON object, or a text line before a JSON array
    (`list-updates`, `library list`). A's parsers must take the first JSON value on stdout.
  - `register` succeeds only with `FAKE-E2E-AMAZON-CODE` **and** the `code_verifier`
    (`FAKE-STORE-TOKEN-not-real`), `serial` and `client_id` that `auth --login` printed, so the
    kept start's round trip is checked. It refuses `-d`, `--base-path`, a new install without
    `--path` and the interactive `auth --login` with exit 2.
  - It writes 0644 files and default-umask folders; markers in `stores/nile/nile/`:
    `.fake-expired`, `.fake-download-fail`, `.fake-no-space`, `.fake-update`,
    `.fake-logout-fail`.
  - `fixtures-amazon/library.json` is nile's shape plus a `fake` key per entry (stripped from
    the `library.json` it writes), with art at `{BASE}`; Fogline has a second entitlement of
    the same product, so the owned list is 3 games only if A dedups.
  - Product ids: Fogline `amzn1.adg.product.5b0f1a2c-3d4e-4f50-8a6b-0000000000a1`, Kettle Drum
    `…a2`, Lowsun `…a3` (exported from `helpers/amazonLogin.ts`).
  - The seed takes `E2E_AMAZON_URL` (e2e.sh passes it) for `library.json`'s art; its signed-in
    profile sets `.fake-update` (an update for Fogline at startup); `--expired` or a spec name
    ending in `-expired` adds `.fake-expired` (the prototype's default state). Exported for the
    specs: `E2E_AMAZON_URL`, `E2E_FAKE_NILE`, `E2E_AMAZON_ARGV_FILE`, `E2E_AMAZON_ENV_FILE`
    (the compat-tool stand-ins' argv and FUEL env logs).
  - The display name is the account name GRID shows (status, config), so the secrets spec
    checks that no log or payload names `AMAZON_GAMES_FUEL_DISPLAY_NAME`, and that the
    variable reached the game.
  - `e2e/package.json` `test:mock` runs the two new harness tests (outside the Owns list).

## Package C outcome (2026-11-14)

- `scripts/ctr.sh gate`: PASS. New unit tests: `stores_service::amazon` (17: login with
  nile installed first, failures, cancel, startup from `library.json`, expired sync,
  Disconnect, install guards and options, library mapping, status, launch hook, Downloads
  cards, notices, refresh counts), `update_service` (`amazon_updates`).
- Harness tests (`node --test`, in the container): `nile.test.mjs` 13/13,
  `mock-amazon.test.mjs` 4/4.
- `scripts/ctr.sh e2e amazon gog tv-gog epic tv-epic`: `gog`, `tv-gog`, `epic` and `tv-epic`
  pass (GOG and Epic are unchanged with Amazon registered; `epic-login` still runs through the
  router hook). `amazon` on package A's Step 0 stubs: `amazon-login` 3 of 12 pass (not
  connected, Install nile, Disconnect). The other 9 wait for A: every login-window step needs
  `start_login`, since `store_connect` gets "not implemented" before the window opens. The
  refused code and the sign-in also need `complete_login`, and the owned list needs `sync`.
  `amazon-secrets` is not reached (the group stops at the first failing spec). It needs A's
  sign-in, install, update, verify, launch prep and disconnect.
- Not run: `wincheck` (no `cfg(windows)`/`cfg(unix)` code touched). `amazon-library` and
  `tv-amazon` are package D's specs; their groups skip until those files exist.

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
