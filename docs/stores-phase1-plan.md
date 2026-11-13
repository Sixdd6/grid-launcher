# Game stores — Phase 1 (GOG) work plan

Status: planned 2026-11-13. Read with `docs/external-stores-plan.md` (user decisions) and the
approved design cards `docs/design/system/components/GameStores/README.md` and
`TVStores/README.md`, which are the spec for every screen and state (generator sources:
`docs/design/prototype/stores.js`, `stores_bk.js`, `tvstores.js`, `tvstores_bk.js`).

Phase 0 gives us `GameKey`/`StoreKind` (`stores/mod.rs`); registry v11 rows (`upsert_store`,
`find_key`, `remove_key`, `store_rows`; `store_meta` is never serialized to IPC);
`JobPayload::Store(StoreJob)` with a runner that has `download` and `finalize` halves; and
`launch_game_key` → `LaunchService::launch_key`.

Gaps: backups know only rom ids (`BackupItemKey::Game(i64)`, `PackGame.source.rom_id`,
`BackupRequest.rom_ids`, `DriveGame.rom_id`); Downloads rows have no phase field; only the
`main` window has IPC capabilities (keep it so: the login window stays unprivileged).

## Step 0 — contracts skeleton (one commit before fan-out)

Every Rust type below, the `goggame` parser, the row builder and the meta struct, the queue enums
and `JobPhase` plumbing, the pack struct, `Config.stores`, every Tauri command registered in
`lib.rs` (stub bodies returning an error "not implemented"), the `api.ts` types and methods, and
`check_secret_hygiene.sh` allowing `expose_secret` in `stores/gog/auth.rs` and `stores/gog/api.rs`.
After it lands, `api.ts`, `config.rs`, `stores/mod.rs` and `stores/gog/{goggame,meta,row}.rs` are
frozen: changes go through the lead.

### grid-core

```rust
// stores/mod.rs adds: pub mod tools; pub mod paths; pub mod progress; pub mod runner; pub mod art;
//                     pub mod gog; pub mod offline;
#[derive(Debug, thiserror::Error)]
pub enum StoreError {               // Display text is GRID's own words; never tool output
    #[error("not signed in")] NotSignedIn,
    #[error("login expired")] LoginExpired,
    #[error("{0} isn't installed")] ToolMissing(&'static str),
    #[error("{tool} stopped: {reason}")] ToolFailed { tool: &'static str, reason: String },
    #[error("couldn't reach {0}")] Network(String),
    #[error("{0}")] Refused(String),
    #[error(transparent)] Library(#[from] LibraryError),
}

// stores/paths.rs
pub fn tool_dir(data_dir: &Path, tool: StoreTool) -> PathBuf;   // <data>/stores/<tool>, 0700, files 0600
pub fn ensure_private_dir(dir: &Path) -> io::Result<()>;
pub fn remove_tool_dir(data_dir: &Path, tool: StoreTool) -> io::Result<()>;

// stores/tools.rs
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)] #[serde(rename_all = "lowercase")]
pub enum StoreTool { Gogdl, Innoextract }   // Innoextract only if the Windows-installer spike says so
pub struct PinnedAsset { pub os: &'static str, pub arch: &'static str, pub url: &'static str,
                         pub file_name: &'static str, pub sha256: &'static str, pub size: u64 }
pub struct ToolPin { pub tool: StoreTool, pub version: &'static str, pub license: &'static str,
                     pub source_url: &'static str, pub assets: &'static [PinnedAsset] }
pub fn pin(tool: StoreTool) -> &'static ToolPin;
pub fn host_asset(tool: StoreTool) -> Option<&'static PinnedAsset>;
pub fn installed_path(data_dir: &Path, tool: StoreTool) -> Option<PathBuf>;  // E2E: GRID_LAUNCHER_E2E_TOOL_<TOOL> (feature e2e)
pub async fn install_tool(http: &reqwest::Client, data_dir: &Path, tool: StoreTool,
    progress: &mut (dyn FnMut(u64, u64) + Send)) -> Result<PathBuf, StoreError>; // SHA mismatch => file deleted, Refused(..)

// stores/runner.rs  (the ONLY place a tool process is spawned; lines never logged)
pub struct ToolCommand { pub program: PathBuf, pub args: Vec<String>, pub env: Vec<(String, String)>, pub cwd: Option<PathBuf> }
pub struct ToolExit { pub code: Option<i32>, pub stdout: SecretString } // stdout kept only for JSON parse
pub fn run_tool(cmd: &ToolCommand, cancel: &AtomicBool,
    on_progress: &mut dyn FnMut(progress::ToolProgress)) -> Result<ToolExit, StoreError>;
// stores/progress.rs
pub struct ToolProgress { pub done: u64, pub total: u64, pub speed_bps: f64, pub phase: JobPhase }
pub fn parse_gogdl_line(line: &str) -> Option<ToolProgress>;

// library/queue.rs
pub enum StoreOp { Install, Update, Repair, Dlc, Installer, Restore }   // kinds: store_install|store_update|store_repair|store_dlc|store_installer|store_restore
pub enum BackupItemKey { Game(i64), Store(GameKey), Emulator{..}, Firmware(String) } // kind backup_store_game
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)] #[serde(rename_all = "snake_case")]
pub enum JobPhase { #[default] None, Downloading, Installing, Verifying, Unpacking, InstallerSilent,
    InstallerWizard, CopyingInstaller, RunningInstaller, CopyingFromDrive, Checksum }
pub trait StoreProgressSink { fn progress(&mut self, done: u64, total: u64); fn phase(&mut self, phase: JobPhase); }
pub trait StoreJobRunner: Send + Sync {          // finalize signature changes
    fn download(&self, cancel: &AtomicBool, progress: &mut dyn FnMut(u64, u64, f64)) -> Result<(), LibraryError>;
    fn finalize(&self, sink: &mut dyn StoreProgressSink) -> Result<String, LibraryError>;
}
pub struct StoreJob { pub source: StoreKind, pub store_id: String, pub op: StoreOp,
                      pub title: String, pub drive: Option<DriveRef>, pub runner: Arc<dyn StoreJobRunner> }
pub struct DriveRef { pub key: String, pub name: String, pub label: String }
// InstallService:
pub fn on_install_phase(&self, id: u64, phase: JobPhase);
pub fn set_wizard_allowed(&self, f: Arc<dyn Fn() -> bool + Send + Sync>); // app: !tv_mode
// LibraryError additions (Display = the design's "Failed ·" text):
InstallerUnrecognised { file: String }, InstallerDlcBaseMissing { base_title: String, base_gog_id: String },
InstallerExit(i32), InstallerNeedsWizard { kept: bool }, InstallerRefused(String)

// stores/gog/goggame.rs  (fully implemented + tests in Step 0)
pub struct GogGameInfo { pub game_id: String, pub root_game_id: String, pub name: String,
    pub build_id: String, pub client_id: String, pub language: String, pub play_tasks: Vec<PlayTask> }
pub struct PlayTask { pub is_primary: bool, pub path: String, pub arguments: String, pub working_dir: String, pub category: String }
pub fn parse_info(bytes: &[u8]) -> Option<GogGameInfo>;
pub fn find_info_files(dir: &Path) -> Vec<PathBuf>;          // goggame-*.info
impl GogGameInfo { pub fn is_dlc(&self) -> bool; pub fn primary_task(&self) -> Option<&PlayTask>; }

// stores/gog/meta.rs  (store_meta JSON; never a token)
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct GogMeta { pub origin: GogOrigin, pub platform: GogPlatform, pub version: String, pub build_id: String,
    pub language: String, pub installer: Vec<String> /*abs paths, offline only*/, pub installer_bytes: u64,
    pub dlc: Vec<GogDlcMeta>, pub cloud_saves: bool, pub save_locations: Vec<SaveLocation>,
    pub last_sync: i64, pub exe_user_pinned: bool }
#[serde(rename_all="lowercase")] pub enum GogOrigin { #[default] Gogdl, Offline, Drive }
#[serde(rename_all="lowercase")] pub enum GogPlatform { #[default] Linux, Windows }
pub struct GogDlcMeta { pub id: String, pub title: String, pub installer: Vec<String> }
pub struct SaveLocation { pub name: String, pub path: String }

// stores/gog/row.rs
pub fn gog_row(info: &GogGameInfo, install_dir: &Path, meta: &GogMeta,
               product: Option<&GogProduct>, wineprefix: Option<&Path>) -> InstalledGame; // platform "GOG", slug linux|win, native cols from primary task

// stores/gog/mod.rs  (package A)
pub struct GogEndpoints { pub auth: Url, pub embed: Url, pub api: Url, pub content: Url, pub cloud: Url } // ::real(), ::e2e(base)
pub struct GogService;  // Clone, Send+Sync
impl GogService {
  pub fn new(data_dir: PathBuf, http: reqwest::Client, ep: GogEndpoints) -> Self;
  pub fn login_url(&self) -> Url;
  pub fn code_from_redirect(&self, url: &Url) -> Option<SecretString>;      // embed.gog.com/on_login_success?code=
  pub async fn complete_login(&self, code: SecretString) -> Result<GogAccount, StoreError>; // gogdl auth --code
  pub async fn account(&self) -> Result<Option<GogAccount>, StoreError>;     // Err(LoginExpired) on refresh failure
  pub fn disconnect(&self) -> io::Result<()>;
  pub async fn owned(&self) -> Result<Vec<GogProduct>, StoreError>;          // DLC folded into base.dlc
  pub async fn install_options(&self, id: &str) -> Result<GogInstallOptions, StoreError>; // gogdl info
  pub fn install_job(&self, req: GogInstallRequest, ctx: GogJobCtx) -> StoreJob;
  pub fn update_job(&self, row: &InstalledGame, ctx: GogJobCtx) -> StoreJob;
  pub fn repair_job(&self, row: &InstalledGame, ctx: GogJobCtx) -> StoreJob;
  pub fn dlc_job(&self, row: &InstalledGame, dlc_id: &str, ctx: GogJobCtx) -> StoreJob;
  pub fn remove_dlc(&self, registry: &Registry, row: &InstalledGame, dlc_id: &str) -> Result<(), StoreError>;
  pub fn uninstall(&self, registry: &Registry, store_id: &str) -> Result<(), StoreError>;
  pub async fn check_updates(&self, rows: &[InstalledGame]) -> Result<Vec<GogUpdate>, StoreError>;
  pub async fn cloud_state(&self, row: &InstalledGame) -> Result<CloudState, StoreError>;
  pub async fn cloud_sync(&self, registry: &Registry, row: &InstalledGame, mode: SyncMode) -> Result<CloudState, StoreError>;
}
pub struct GogJobCtx { pub registry: Arc<Registry>, pub library: PathBuf, pub default_compat_tool: String }
pub struct GogAccount { pub username: String, pub user_id: String }
pub enum SyncMode { Auto, PreferUpload, PreferDownload }
pub enum CloudState { Off, None, Synced { at: i64 }, Conflict(SaveConflict) }
pub struct SaveConflict { pub local_at: i64, pub local_files: u32, pub local_bytes: u64, pub cloud_at: i64 }

// stores/offline/mod.rs  (package B)
pub enum InstallerKind { LinuxSh, WindowsExe { parts: Vec<PathBuf> }, MacPkg }
pub struct Inspection { pub path: PathBuf, pub kind: InstallerKind, pub info: Option<GogGameInfo>,
    pub version: String, pub bytes: u64, pub verdict: Verdict }
pub enum Verdict { Install, Update { installed_version: String }, Dlc { base_title: String },
    Refused { title: String, body: String } }
pub fn inspect(path: &Path, registry: &Registry, tools_dir: &Path) -> Result<Inspection, StoreError>;
pub fn detect_in_dir(dir: &Path) -> Option<PathBuf>;                       // RomM archive: installer inside?
pub fn install_job(insp: Inspection, opts: OfflineOpts, ctx: GogJobCtx, wizard_allowed: Arc<dyn Fn() -> bool + Send + Sync>) -> StoreJob;
pub fn run_into(insp: &Inspection, dest: &Path, prefix: Option<&Path>, compat: &str,
    wizard_allowed: bool, sink: &mut dyn StoreProgressSink) -> Result<GogGameInfo, LibraryError>; // shared by RomM path and drive restore
pub struct OfflineOpts { pub install_dir: Option<PathBuf>, pub compat_tool: String }

// backup/pack.rs (package C fills; struct in Step 0)
pub struct Pack { /* existing */ #[serde(default, skip_serializing_if = "Vec::is_empty")] pub store_games: Vec<PackStoreGame> }
pub struct PackStoreGame { pub source: StoreKind /*gog*/, pub store_id: String, pub name: String,
    pub platform: GogPlatform, pub layout: StoreLayout /*"installer"|"installed"*/, pub version: String,
    pub summary: String, pub companies: Vec<String>, pub genres: Vec<String>, pub release_date: String,
    pub size: u64, #[serde(default)] pub installer: Vec<PackFile>, #[serde(default)] pub dlc_installers: Vec<PackStoreDlc>,
    #[serde(default)] pub installed: Option<PackInstalled>, #[serde(default)] pub saves: Option<PackSaves>,
    #[serde(default)] pub assets: PackAssets, pub meta: serde_json::Value, pub added: String,
    #[serde(flatten)] pub extra: BTreeMap<String, serde_json::Value> }
// paths: stores/gog/<component(name)>-<id>/{installer,installed,saves,assets}; FORMAT stays 1
// (older builds keep `store_games` through Pack.extra)
impl Pack { pub fn find_store_game(&self, key: &GameKey) -> Option<&PackStoreGame>; pub fn upsert_store_game(..); pub fn remove_store_game(..); }

// config.rs
pub struct StoresConfig { pub gog: GogConfig }
pub struct GogConfig { pub connected: bool, pub username: String, pub install_dir: String /*"" = <library>/GOG*/, pub cloud_saves: bool }
```

### Tauri commands and events

camelCase arguments, snake_case JSON. `key` is the game-key string.

| Command | Args | Returns |
|---|---|---|
| `stores_status` | — | `StoreStatus[]` (GOG only in Phase 1) |
| `store_library` | `source` | `StoreLibrary` |
| `store_refresh` / `store_check_updates` | `source` | `void` (results come as events) |
| `store_connect` | `source` | `void`; opens the login window; result is a `store-login` event |
| `store_cancel_login` / `store_disconnect` / `store_install_tool` | `source` | `void` |
| `store_login_type` | `text` | `void` (TV on-screen keyboard to the page) |
| `store_set_install_dir` | `source, path` | `void` |
| `store_set_cloud_saves` | `source, on` | `void` |
| `store_install_options` | `key` | `StoreInstallOptions` |
| `store_install` | `request: StoreInstallRequest` | `void` |
| `store_update` / `store_repair` / `store_uninstall` | `key` | `void` |
| `store_install_dlc` / `store_remove_dlc` | `key, dlcId` | `void` |
| `store_game_settings` | `key` | `StoreGameSettings` |
| `store_set_game_settings` | `key, settings` | `void` |
| `store_cloud_state` / `store_cloud_sync` | `key` | `StoreCloudState` |
| `store_cloud_resolve` | `key, choice: 'upload'\|'download'` | `StoreCloudState` |
| `offline_inspect` | `path` | `InstallerInspection` |
| `offline_install` | `path, compatTool, installDir?` | `void` |
| `plan_store_backup` | `key /*drive*/, gameKeys` | `StoreBackupPlan` |
| `start_store_backup` | `key, gameKeys` | `BackupQueued` |
| `drive_store_games` | `key` | `DriveStoreGame[]` |
| `install_store_from_drive` | `key, gameKey` | `void` |
| `remove_store_from_drive` | `key, gameKeys` | `number` |

`launch_game_key`, store branch: sync cloud saves before launch; on a conflict it rejects with
`{code:'store_cloud_conflict', message}` (the UI prompts, then calls `store_cloud_resolve`); it
uploads after the session ends.

Events: `stores-changed` (`StoreStatus[]`), `store-login`
(`{source, outcome:'connected'|'cancelled'|'failed', username, error}`; a network error stays in
the login window), `store-notice` (`StoreNotice`). Existing events are reused: `downloads`
snapshots, `installed-changed`, `updates-changed`, `drives-changed`.

### TypeScript (`api.ts`)

```ts
export type StoreSource = Exclude<GameSource, 'romm'>;
export type ToolStatus = { name: 'gogdl'; version: string; state: 'not_installed'|'installing'|'installed'|'failed';
  size_bytes: number; error: string; license: string; source_url: string; done: number; total: number };
export type StoreStatus = { source: StoreSource; connection: 'connected'|'expired'|'not_connected'; username: string;
  tool: ToolStatus; owned: 'idle'|'loading'|'ready'|'error'; owned_error: string; owned_count: number;
  installed_count: number; installed_bytes: number; refreshed_at: number; cloud_saves: boolean;
  install_dir: string; in_rail: boolean; last_error: string };
export type StoreDlc = { id: string; title: string; download_bytes: number; installed: boolean };
export type StoreGame = { key: string; source: StoreSource; store_id: string; title: string; installed: boolean;
  cover: string; screenshots: string[]; description: string; developer: string; genres: string[]; year: string;
  platforms: ('linux'|'windows')[]; installed_platform: 'linux'|'windows'|null; download_bytes: number; disk_bytes: number;
  version: string; update_version: string; origin: 'gogdl'|'offline'|'drive'|null; not_supported: string;
  dlc: StoreDlc[]; cloud: 'off'|'none'|'synced'|'conflict'|'unknown'; last_sync: number; install_dir: string; compat_tool: string };
export type StoreLibrary = { status: StoreStatus; installed: StoreGame[]; available: StoreGame[] };
export type StoreInstallOptions = { versions: { platform: 'linux'|'windows'; recommended: boolean; download_bytes: number; disk_bytes: number }[];
  languages: string[]; default_language: string; dlc: StoreDlc[]; install_dir: string; free_bytes: number; volume: string; compat_tool: string };
export type StoreInstallRequest = { key: string; platform: 'linux'|'windows'; language: string; dlc_ids: string[]; install_dir: string; compat_tool: string };
export type StoreGameSettings = { install_dir: string; executable: string; exe_user_pinned: boolean; launch_parameters: string;
  compat_tool: string; wineprefix: string; cloud_saves: boolean };
export type StoreCloudState = { state: 'off'|'none'|'synced'|'syncing'|'conflict'; synced_at: number; locations: string[];
  conflict: { local_at: number; local_files: number; local_bytes: number; cloud_at: number } | null };
export type InstallerInspection = { path: string; kind: 'linux_sh'|'windows_exe'|'mac_pkg'; key: string | null; title: string;
  cover: string; version: string; bytes: number;
  verdict: { kind: 'install' } | { kind: 'update'; installed_version: string } | { kind: 'dlc'; base_title: string }
         | { kind: 'refused'; title: string; body: string } };
export type StoreNoticeAction = { kind: 'play'|'open_downloads'|'try_again'|'sign_in'|'install_tool'|'show_updates'|'install_base'|'open_settings';
  key?: string; title?: string };
export type StoreNotice = { source: StoreSource | null; message: string; detail: string; error: boolean; action: StoreNoticeAction | null };
// DownloadEntry gains:
phase: 'none'|'downloading'|'installing'|'verifying'|'unpacking'|'installer_silent'|'installer_wizard'|'copying_installer'|'running_installer'|'copying_from_drive'|'checksum';
failure: { code: 'installer_unrecognised'|'installer_dlc_base_missing'|'installer_exit'|'installer_needs_wizard'; base_title: string; base_gog_id: string; kept: boolean } | null;
// DownloadKind gains: 'store_repair'|'store_dlc'|'store_installer'|'store_restore'|'store_tool'|'backup_store_game'
export type DriveStoreGame = { key: string; name: string; platform: 'linux'|'windows'; layout: 'installer'|'installed';
  version: string; size: number; cover: string; installed: boolean; added: string; summary: string };
export type StoreBackupPlan = { drive: string; rows: { key: string; name: string; layout: 'installer'|'installed'; detail: string;
  bytes: number; windows_prefix_note: boolean; already: boolean; refused: string }[]; bytes_to_write: number; free_bytes: number; fits: boolean };
export const STORES_CHANGED_EVENT = 'stores-changed', STORE_LOGIN_EVENT = 'store-login', STORE_NOTICE_EVENT = 'store-notice';
```

Each command gets a camelCase `api.*` method (`storeInstall`, `offlineInspect`, …). Phase 1 hides
Epic, Amazon and Steam (no cards, no rail groups).

## Work packages (after Step 0)

- **A — grid-core GOG online.** Owns `stores/{tools,paths,progress,runner,art}.rs`,
  `stores/gog/{mod,auth,api,install,update,cloud}.rs`, `crates/grid-core/tests/gog_*.rs` and
  fixtures. Pinned gogdl install (SHA-256), auth and refresh (bearer only in memory as
  `SecretString`), owned list and art through GOG HTTP APIs, install options from `gogdl info`,
  install/update/repair/DLC runners with progress and phases, uninstall, update check
  (content-system builds vs `meta.build_id`), save-sync wrapper with GRID's own conflict
  detection. Tests: parsers with `FAKE-STORE-TOKEN-not-real`, argv builders, wiremock API tests,
  a `cfg(unix)` fake gogdl script, SHA mismatch deletes the file, the bearer never in
  Display/Debug.
- **B — offline installers and RomM installer archives.** Owns `stores/offline/*`,
  `library/specials/gog_installer.rs`, `launch/native.rs` edits, the `finalize_native_base` region
  and retry path in `library/mod.rs`, and `library/queue.rs` after Step 0 (keep the download on
  `InstallerNeedsWizard`; retry without downloading again). `.sh` read as zip (`data/noarch`,
  never run); `.exe` silent via umu/wine, native on Windows, wizard only when `wizard_allowed()`;
  identity from `goggame-*.info`, DLC by `rootGameId`; `.pkg` refused plus the other refusals;
  installer path recorded in `meta.installer`; a RomM archive with an installer stays a RomM row.
- **C — GOG backups.** Owns the store parts of `backup/{pack,plan,local,saves}.rs`, new
  `backup/store.rs`, `library/{backup_job,drive_restore,drive_install}.rs`, `finalize_backup` and
  restore dispatch in `library/mod.rs`, the store commands in the app's backup commands and
  `drive_service.rs`. Installer layout when every `meta.installer` path still exists with the same
  size, else installed copy plus saves (no Wine prefix); refusals while running or installing;
  restore via `offline::run_into` or copy back as a `StoreOp::Restore` job with `drive`; restored
  rows get `origin: drive` (or `offline` after an installer restore). Merges after B.
- **D — app services, login window, e2e harness.** Owns `stores_service.rs`, `store_auth.rs`,
  `commands/stores.rs`, the store branch of `launch_game_key` (sync before launch, upload after
  exit), `update_service.rs` store updates, `lib.rs` after Step 0, `app/src-tauri/Cargo.toml`,
  `tauri.conf.json`, capabilities; e2e: `e2e/mock-romm/mock-gog.mjs`,
  `e2e/fake-tools/{gogdl,installer,innoextract}.mjs`, `e2e/fixtures-gog/`, seeds, the `scripts/e2e.sh`
  stage and `wdio.conf.ts`, e2e-feature env overrides, `e2e/specs/gog-login.spec.ts` and
  `gog-secrets.spec.ts`. Login window: incognito data dir, no capability, redirect caught in Rust,
  `store-login` event; gogdl install right after connect; owned cache and refresh; `store-notice`
  for every notice on the GameStores list.
- **E — shared frontend model and desktop UI.** Owns `app/src/lib/gamestores/**` (pure `.ts`),
  `lib/stores/gameStores.svelte.ts`, `lib/gamestores/fakeApi.ts`, `Library.svelte`,
  `lib/library/rail.ts`, store parts of `Details.svelte` and `lib/details/*`,
  `lib/settings/StoresPage.svelte` and `pages.ts`, `lib/downloads/format.ts`,
  `lib/notifications/*`, GOG parts of `lib/backups/*`, `e2e/specs/gog-*.spec.ts`. Port each screen
  from `stores.js`/`stores_bk.js`, item by item from the GameStores checklist. Milestone E1: the
  pure modules and the rune store, committed first.
- **F — TV UI.** Owns new files under `lib/tv/**` and `e2e/specs/tv-gog-*.spec.ts`. Port from
  `tvstores.js`/`tvstores_bk.js`, item by item from the TVStores checklist. Imports E1, never edits it.

Order: Step 0; then A, B, C, D start together (backend wave); E and F after the backend merges
(or against `fakeApi`). Merge order A, B, C, D, E, F; full e2e on the merged tree.

Shared done criteria: the gate (incl. `check_secret_hygiene.sh`), fake markers only, UI packages
tick every Phase 1 checklist item, e2e specs pass against `mock-gog` and fake tools: `gog-login`,
`gog-library`, `gog-cloud`, `gog-offline`, `gog-romm-installer`, `gog-backups`, `tv-gog-*`,
`gog-secrets`.

## Spikes

1. gogdl release: pinned tag, upstream asset names per OS/arch, self-contained or not, SHA-256s
   and sizes.
2. gogdl CLI at that tag: `auth --code`, `info` JSON, download/update/repair flags, DLC flags,
   progress line format and stream; save-sync semantics (GRID decides conflicts itself from
   `meta.last_sync`, local mtimes and GOG cloudstorage timestamps, then forces a direction); where
   save paths come from.
3. Tauri 2 login window on WebKitGTK: `on_navigation` for the redirect; GRID chrome via a child
   webview (`unstable`) or a plain decorated window; separate data dir; cookie wipe on Disconnect.
4. TV on-screen keyboard and d-pad inside a third-party page via `webview.eval`.
5. Windows installers: identity before running (pinned innoextract `--list`?), silent flags, how
   "needs wizard" shows, `/DIR` mapping under umu, the prefix a DLC installer uses.
6. RomM pre-download hint: possible only from file names or multi-file ROMs; else after download.
7. Update identity for offline installs: version string vs content-system build ids.

## Spike results (2026-11-13)

Decisions taken from them (lead):
- **Pin heroic-gogdl v1.4.0** (has the chunk-cache integrity fix). Assets under
  `https://github.com/Heroic-Games-Launcher/heroic-gogdl/releases/download/v1.4.0/`; SHA-256 from
  GitHub's published asset digests:

  | asset | size | sha256 | kind |
  |---|---|---|---|
  | gogdl_linux_x86_64 | 983218 | 2aa2b59c83d3bfbe7b60ead91c6297b263b5d1e6c233a04b7409bafe26738f02 | Python zipapp |
  | gogdl_linux_arm64 | 975558 | 3df71cb07603b5d17fd914c96d17e4b381fad5a5705dc7dd3e3ff7a3856c6cc3 | Python zipapp |
  | gogdl_windows_x86_64.exe | 12471749 | 291b653e6dbf7a394a3342ed0ebcea2cbc83e5f66d030777d65ccdd60d18ab02 | PyInstaller |
  | gogdl_windows_arm64.exe | 12296193 | f3885d51c823d2faaf3b49647c63e799339afdc51fdbd0515e399ca08954630c | PyInstaller |
  | gogdl_macos_x86_64 | 11720560 | 5b7e68497907025549f83f960a6d325e53cf48f5cac63a4f1313318e50dd2de8 | PyInstaller |
  | gogdl_macos_arm64 | 11098928 | af167d29ca822f495ff677d03c40bb9d6dc5535dac1f30d71775c91fe844149c | PyInstaller |

  The Linux assets are zipapps (`#!/usr/bin/env python3`, Python ≥ 3.10; a bundled xdelta3 `.so`
  unpacks into `$XDG_CACHE_HOME/heroic_gogdl/vendored`). GRID already requires python3 ≥ 3.10
  for umu-launcher (`launch::umu`); reuse that check, and show the same "needs Python 3.10" tool
  state when it fails. Run Linux gogdl as `python3 <zipapp>`. License GPL-3.0 (repo `LICENSE`).
- **Running gogdl.** `gogdl --auth-config-path <data>/stores/gogdl/auth.json <cmd> …` — the flag
  is top-level and MUST come before the subcommand (after it, it is silently ignored). Env:
  `GOGDL_CONFIG_PATH=<data>/stores/gogdl` (manifests land in `…/heroic_gogdl/manifests/<id>`),
  and on Linux `XDG_CACHE_HOME=<data>/stores/gogdl/cache`. Logging goes to stderr as
  `[NAME] LEVEL: msg`; results to stdout. `auth` prints token JSON on stdout: never log or
  forward it; parse only what GRID needs (user id; the access token as `SecretString`).
- **`info <id> --platform X [--lang L] --with-dlcs`**: one JSON line on stdout:
  `{"size":{"*":{download_size,disk_size},"<lang>":{…}},"dlcs":[{title,id,size}],"buildId",
  "languages":[…],"folder_name","dependencies","versionEtag","versionName",…}`; a language's size is
  `size["*"] + size[lang]`. `--platform linux` returns a flat shape (download_size, disk_size,
  dlcs, languages, folder_name, versionName).
- **download/update/repair**: `--path` (download appends the build's install directory; update and
  repair take the game dir), `--support <dir>`, `--platform`, `--lang`, `--with-dlcs --dlcs a,b`
  (`--dlcs` does nothing without `--with-dlcs`; default no DLC), `--max-workers`. Progress on
  stderr about once a second: `[PROGRESS] INFO: = Progress: 12.34 123456/1000000, Running for:
  00:01:02, ETA: 00:07:30` plus `= Downloaded: … MiB, Written: … MiB` and `+ Download\t- …` /
  `+ Disk\t- …` lines. Exit 2 = not enough disk space; 1 = no builds / platform unsupported;
  `-signum` on signals. **A fatal worker error exits 0 without writing the manifest: success = exit
  0 AND the manifest was rewritten.** The executor uses 1 GiB of `/dev/shm`.
- **Cloud saves.** Save locations come from
  `https://remote-config.gog.com/components/galaxy_client/clients/{clientId}?component_version=2.0.45`
  → `content.{Windows|MacOS}.cloudStorage.{enabled, locations:[{name, location}]}` with variables
  `<?INSTALL?>`, `SAVED_GAMES`, `APPLICATION_DATA_LOCAL(_LOW)`, `APPLICATION_DATA_ROAMING`,
  `DOCUMENTS`, `APPLICATION_SUPPORT`; `clientId` from the gogdl manifest or `goggame-<id>.info`.
  `save-sync <path> <id> --os <os> --ts <float> --name <location name>` with direction flags
  `--skip-download|--skip-upload|--force-upload|--force-download` prints the new timestamp on
  stdout and reports nothing structured (it exits 0 even on upload/download errors and on
  conflicts). **GRID decides the direction itself** (from `meta.last_sync`, local mtimes and the
  cloudstorage listing) and always passes `--force-upload` or `--force-download`.
  `--force-download` deletes local files missing in the cloud: copy the local saves aside first.
- **Windows installers.** Silent flags: `/SP- /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /NOCANCEL
  /NOICONS /LANG=<inno lang> /DIR="<path>" /LOG="<path>\grid-install.log"`; under Wine/umu use
  a `Z:\…` path (backslashes). Exit 0 ok; 2 and 5 = cancelled; 1, 3, 4, 7, 8 = failed → wizard
  offer (desktop) / `InstallerNeedsWizard` (TV). "Needs a wizard" mostly shows as a **hang**: treat
  the process as stalled when neither the log nor the target dir has grown for 120 s, then kill
  it (`wineserver -k` in that prefix) and treat it as needing the wizard. No overall timeout.
  **A DLC installer runs in the base game's prefix with the base game's `/DIR`.**
- **No innoextract.** 1.9 is too old for current Inno versions and has no macOS build. For identity
  before running, try the `innospect` crate (Apache-2.0, pure Rust) to read `goggame-*.info`
  (gameId, buildId) and languages; when it fails, fall back to the file name, then read
  `goggame-*.info` from the install dir after the run. Drop `StoreTool::Innoextract`.
- **Update identity for offline installs.** The file name's number is not a build id.
  `goggame-<id>.info` `buildId` (or `goggame-<id>.id`) matches content-system
  `builds?generation=2`; store it in `meta.build_id`. Windows: compare the default build's
  `build_id`. Linux installers: compare `api.gog.com/products/{id}?expand=downloads`
  `installers[os=linux].version` with the version in `gameinfo`. An offline install with no build
  id never shows an update.

## Contract deviations (Step 0)

Where the skeleton differs from, or adds to, the Step 0 block above:

- `StoreTool` is `Gogdl` only (spike results). `PinnedAsset` gains `launcher: AssetLauncher`
  (`Direct` | `Python3`: Linux gogdl runs as `python3 <zipapp>`). `StoreTool::name()` gives
  the folder/serde name. The pins hold no assets yet; package A fills them.
- `StoreError::not_implemented()` and `stores::NOT_IMPLEMENTED` exist for the stubs only.
- `GogEndpoints::e2e(base: Url)` points all five hosts at one mock server.
- `gog_row` needs `GogProduct`, so `stores/gog/mod.rs` defines it (`id, title, cover,
  screenshots, description, developer, publisher, genres, release_date, platforms, dlc:
  Vec<GogProductDlc>`), plus `GogInstallOptions { versions: Vec<GogVersionOption>, languages,
  default_language, dlc: Vec<GogDlcOption> }`, `GogInstallRequest { store_id, title, platform,
  language, dlc_ids, install_dir: PathBuf, compat_tool }` and `GogUpdate { store_id, version,
  build_id }`. Package A may add fields; it must not rename these.
- `gog_row`: `store_id` is `info.game_id`; the title is the product's, else the info name;
  `native_executable_path` is absolute (`install_dir` + the primary task path, both separators);
  a task path that is absolute, has a drive letter or climbs out (`..`) is not used (left blank).
  `native_working_dir` is relative with `/`. `native_launch_source` is `user` when
  `meta.exe_user_pinned`, else `sidecar` (the caller carries a pinned executable over).
  `extracted_dir` and `native_game_dir` are both the install folder. `installed_at` is now;
  `filesize_bytes` stays 0 for the caller. Companies are developer then publisher, deduped.
- `GogMeta::from_json` / `to_json` and `GogPlatform::slug()` (`linux` | `win`) added.
- `goggame::parse_info` keeps only `FileTask`s with a path (a task without `type` counts);
  ids may be JSON numbers; a UTF-8 BOM is allowed. `primary_task` falls back to the first
  `game` task. `find_info_files` is top-level only and skips files over 1 MiB.
- Queue: `JobFailure { code: FailureCode, base_title, base_gog_id, kept }` with
  `JobFailure::from_error(&LibraryError)`; the queue sets `failure` on any `Failed`
  transition. `InstallerRefused` and other errors give `failure: null`. `QueueState::set_phase`
  added; `phase` is never reset by the queue.
- `InstallService::enqueue_store_job(job)` drops its `title` argument (the job carries
  `title`; `drive` fills the row's drive fields). `InstallService::wizard_allowed()` returns the
  installed check (default: yes).
- `InstallerNeedsWizard` Display: "This installer needs its setup window. Install it in desktop
  mode." plus " The download was kept." when `kept`. The UI words the offline-installer variant
  ("The installer was kept.") from `failure`.
- A `BackupItemKey::Store` row has `rom_id` 0 and its game key in `source_id`.
- `Pack::paths()` includes store games, so a bad store path makes a pack unreadable like any
  other. `backup::pack::store_game_dir_rel(source, name, id)` gives `stores/<source>/<name>-<id>`.
  `PackStoreGame::key()` and `paths()` added; `meta` defaults to `null`.
- `GogConfig::cloud_saves` defaults to `true`; `[stores]` is not written while default.
- `app/src-tauri/src/commands/stores.rs` mirrors every `api.ts` store type in Rust (enum-like
  strings are `String` where the TS union is wide). `#![allow(dead_code)]` until package D
  builds them. Event constants live there.
- `api.ts` adds `StorePlatform`, `JobPhase`, `JobFailure` and `StoreLoginEvent` aliases.

### Package A

- `GogEndpoints` gains `library` (galaxy-library), `gamesdb` and `remote_config` (Galaxy remote
  config); `e2e(base)` points them at the mock too. The paths GRID uses on each host do not
  overlap: `auth`, `on_login_success`, `userData.json`, `user/data/games`,
  `users/{uid}/releases`, `products?ids=`, `products/{id}`,
  `platforms/gog/external_releases/{id}`, `products/{id}/os/windows/builds`,
  `components/galaxy_client/clients/{clientId}`, `v1/{uid}/{clientId}`.
- `ToolCommand`'s `Debug` no longer derives: it prints the program and the argument and
  variable counts only (a login code is an argument). `ToolExit::success()` added.
- `tools`: `asset_for`, `tool_command` (wraps a zipapp as `python3 <file>` after the Python
  3.10 check, `python_problem` words it), `StoreTool::e2e_env` (feature `e2e`). An asset counts
  as installed when its file has the pinned size. With `GRID_LAUNCHER_E2E_TOOL_GOGDL` set (e2e
  feature only) the fake runs directly and `install_tool` returns it without downloading.
- `paths`: `make_private_file`, `make_private_executable`, `save_backups_dir`
  (`<data>/stores/save-backups/`, outside the tool folder so Disconnect keeps save copies).
- `progress`: a `+ Download` line is a speed-only report (`done == total == 0`);
  `ProgressMerger` folds it into the byte report.
- A fresh install runs `gogdl update --path <install_dir>` (after deleting a stale manifest),
  not `download --path <parent>`: `download` appends GOG's folder name, and
  `GogInstallRequest.install_dir` is the game's own folder. Success = exit 0 and the manifest
  (`manifests/<id>`, or `<dir>/.gogdl-linux-manifest`) written during the run; a repair that
  finds nothing broken leaves the manifest alone, so a repair needs only exit 0 and the manifest.
- A Windows build on Linux gets the prefix `<install_dir>/prefix` (`WINE_PREFIX_DIR`, as RomM
  native games and drive restores). A Linux build launches `start.sh` when it has one, with a
  synthesized `GogGameInfo` (name from `gameinfo`) when there is no `goggame-*.info`.
- `uninstall`: `unins000.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART` through
  `build_native_command` (native on Windows; skipped on Linux when the row has no compat tool),
  stopped after 120 s; then the folder through `RemovalGuard` with the game's parent folder as
  the library root plus every configured root. A refused folder stays (logged, path only) and
  the row still goes, like a RomM uninstall.
- `remove_dlc` runs gogdl `update` with the remaining DLC list (blocking) for gogdl installs;
  offline/drive rows only drop the meta entry.
- Cloud saves: Windows builds only (remote-config has no Linux locations). The game-scope token
  comes from `gogdl auth --client-id --client-secret` (secret from gogdl's manifest, else the
  default Windows build's meta). Local copies before a forced download go to
  `<data>/stores/save-backups/gog/<id>/<epoch-ms>/<location>/`, newest 5 kept.
- `auth::read_stdout` is the one place GOG code reads a tool's kept stdout (a JSON parse).

### Package E

- `api.ts` is unchanged: no type or method was missing for the desktop UI.
- **Who raises which card.** The frontend words the cards for the commands it calls and their
  answers: added to download queue (install dialog, offline installer), refusals, Launching /
  Syncing cloud saves, the cloud conflict choices, uninstalled, disconnected, and the sign-in
  outcome from `store-login` ("Connected to GOG as …", "Couldn’t sign in to GOG: …" · Try again).
  Package D's `store-notice` carries the asynchronous ones only (tool installing / installed,
  owned list loaded, installed / updated / verified, update available, cloud upload after a game
  closes, the offline installer's wizard fallback). D must not also send `store-notice` for the
  ones above, or they show twice. Store rows (`job: 'store'`) earn no card from the Downloads
  diff.
- RomM installs that run a GOG installer from their archive are worded by the frontend from the
  row: the wizard card on the move to `phase: 'installer_wizard'`, the failure card from
  `failure` (`install_base` carries `title` and, when known, `key: 'gog:<base_gog_id>'`), and
  the generic "Couldn’t install" card is not shown for a row with `failure`.
- `launch_game_key`'s conflict refusal is read from `{ code: 'store_cloud_conflict' }`; the
  prompt then reads `store_cloud_state` for the two sides.
- `StoreGame.cover` and `screenshots` go through `ensure_image` like RomM art (a URL or a data
  URL); a backend that answers a cached path should answer the URL instead.
- `DownloadEntry.detail` is read for: a store tool's completed version ("Installed · 1.4.0"), a
  repair's result ("Verified · 2 files repaired"), and a GOG backup row's layout ("Installer
  files and saves"). An update's new version comes from the game's library entry.
- The store rail rows' keys are `store:<source>:installed|available`; `RailKey` gained
  `` `store:${string}` ``. Settings gained the `stores` page (and TV mode's `TV_PANES` the
  `stores` pane, which package F fills).
- Shared components gained optional props only: `RailPane` (store heading mark, flag, indent,
  spinner / `!` counts), `GameCard` (extra tags, progress bar, primary label, store corner tag,
  no cloud button, faded art), the toasts (`desktopAction`, the desktop card's one button).
- Desktop components live in `app/src/lib/gamestores/desktop/`; package F imports only the pure
  modules in `app/src/lib/gamestores/*.ts` and `stores/gameStores.svelte.ts`. TV mode shows
  `store-notice` its own way through `onStoreNotice` (the desktop's wiring is in `StoreLayer`).
- Dev only: `VITE_FAKE_STORES=1 npm run dev` runs the desktop UI in a browser against
  `gamestores/fakeApi.ts` (`gamestores/devMock.ts`, Tauri's IPC mock); `window.__gridFake`
  switches the prototype's states. `main.ts` imports it only under `import.meta.env.DEV`.

## Integration list (lead, after all packages merge)

From packages E and F, to fix once B, C and D are on main:
1. **Notices, once each.** The desktop shell's `store-notice`/`store-login` subscription must be
   desktop-only (TV subscribes itself). `store-notice` carries only background notices (D agreed).
   The shared notifier must skip `installer_needs_wizard` in TV mode (TV shows its own "Couldn't
   install X silently" card with Retry). Check the backup-queued card isn't doubled by a backend
   notice.
2. **Library rail (E):** `library/rail.ts` must leave store rows out of the Platforms list (no
   "GOG" platform row; the GOG group covers it).
3. **Backend:** `set_wizard_allowed` false in TV mode; controller input still reaches GRID while
   the login window has focus; the launch conflict rejects with code `store_cloud_conflict`.
4. **RomM installer-archive line** on Details / the TV game page ("Installed by running its GOG
   installer") needs a field: add one (e.g. `InstalledGame.installed_by_installer`) at
   integration. The pre-install line ("Its archive holds a GOG installer") can't be known before
   download (spike 6); drop it from the build and tell the user.
5. **Drive rail count** must include GOG games (`pack.game_count` or the UI sum).
6. **Store covers:** check `StoreGame.cover` is a URL GRID can show through the store art
   allowlist; TV draws store art with a plain `<img>`.
7. Run every `gog-*` and `tv-gog-*` e2e spec (add a `tv-gog` stage group in `scripts/e2e.sh`).

### Package C

No frozen type changed (`Pack`, `PackStoreGame`, `StoreLayout`, `api.ts` as in Step 0).
Additions and choices the other packages should know:
- The five store backup commands keep their IPC arguments and gain `State`/`AppHandle`;
  their bodies live in a new `app/src-tauri/src/commands/store_backups.rs`.
- `InstallService` gains `set_store_running_check(Fn(&GameKey) -> bool)` (the store
  backup commands install it from the launch service's sessions on each call; package D
  may move that wiring into `lib.rs`), `store_running`, `store_job_live` (a live
  `job == "store"` row for the key: the "being installed or updated" refusal),
  `store_backup_busy` (a live `backup_store_game` row: for D's uninstall/update guards),
  `plan_store_backup`, `start_store_backup` and `install_store_from_drive`.
  `BackupItem::Store(StoreItem)`; `BackupJob.client` is now `Option` (a store backup
  needs no server; retry too).
- Restore: `drive_restore::store_restore_job(drive, game, StoreRestoreCtx, InstallerRun)`.
  The installer step is a seam (`InstallerRun`); production uses
  `offline_installer` = `offline::inspect(staged, registry, <config dir>/stores)` then
  `offline::run_into`. DLC installers run after the base game into the same folder and
  prefix; a DLC failure is a warning. An installer restore records origin `offline`, no
  `installer` paths and `installer_bytes` 0 (the staged copy is deleted); an installed
  copy records origin `drive`. Both clear `exe_user_pinned` (the pinned path is gone).
- A restored Windows GOG game (off Windows) gets `<install dir>/prefix`
  (`backup::store::STORE_PREFIX_DIR`) as its Wine prefix. A backup leaves out the row's
  `native_wineprefix` when it is inside the install folder, and a `prefix/` holding
  `drive_c` or `pfx`. Package A: put GOG prefixes there too, or anywhere outside the
  install folder.
- Save folders: `meta.save_locations` (GOG's `<?VAR?>` paths) become Windows-token
  folders, and `<?INSTALL?>\…` for a folder inside the install folder; the cloud-save
  folders GRID's config knows for the title are added for a Windows game.
- A store job's download half has no phase sink, so a restore row shows
  `RunningInstaller` / `Installing` only from finalize; while it copies off the drive
  its phase is `none` (the row's kind `store_restore` with `drive_label` tells the UI).
- `DriveStoreGame.cover` is the pack path of the cover; `drive_asset` serves a store
  game's art under the cache key `gridpack://<drive>/<rel>`. A backup's cover is the
  row's `cover_large_path`/`cover_small_path` (a local file or the image cache's copy).
- Drive summaries count store games in `game_count`; `Pack::pack_bytes` includes them.
- `drive_copy::copy_targets` no longer needs a `Send` progress callback.
